// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useQueryClient } from "@tanstack/react-query";
import { FileSpreadsheet } from "lucide-react";
import { useEffect, useEffectEvent, useRef, useState } from "react";
import { Button, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { Dialog } from "../../components/Dialog";
import { Select, type SelectOption } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { useToasts } from "../../components/Toasts";
import { useProjects } from "../../data/board";
import { useCall, useOp } from "../../data/hooks";
import { opKey } from "../../data/queries";
import { skippedReason, useFailureToast } from "../../data/safety";
import { type CsvColumn, type CsvColumnMapping, type CsvDateOrder, type CsvEncoding, type CsvImportResult, type CsvUnit } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { chooseCsv, choosesFiles, importCsv, type CsvImportRequest, type ImportSource } from "../../ipc/transfer";
import { readPref, writePref } from "../../prefs/store";
import styles from "./CsvImport.module.css";

/** The person's last preset, kept with their own preferences, not the workspace's. */
const PRESET_PREF = "csvImport.preset";
/** How long the check waits after a change before it runs again. */
const RECHECK_MS = 250;
/** Rows with problems shown; the rest are counted. */
const PROBLEMS_SHOWN = 100;

/** The built-in targets, in the order they are offered. */
const TARGETS = [
  "title",
  "description",
  "status",
  "type",
  "priority",
  "labels",
  "fix_version",
  "start_date",
  "due_date",
  "estimate",
  "spent",
  "comment",
  "key",
  "parent",
  "blocks",
  "blocked_by",
  "relates",
  "duplicates",
  "duplicated_by",
  "clones",
  "cloned_by",
  "relations",
] as const;

const UNITS: readonly CsvUnit[] = ["seconds", "minutes", "hours", "days", "text"];
const ENCODINGS: readonly CsvEncoding[] = ["utf8", "utf16le", "utf16be", "windows1252", "latin1"];
/** Encoding names are those of the standards, in any language. */
const ENCODING_NAMES: Readonly<Record<CsvEncoding, string>> = {
  utf8: "UTF-8",
  utf16le: "UTF-16LE",
  utf16be: "UTF-16BE",
  windows1252: "Windows-1252",
  latin1: "ISO-8859-1",
};
const SEPARATORS = [
  [",", "csv-separator-comma"],
  [";", "csv-separator-semicolon"],
  ["\t", "csv-separator-tab"],
  ["|", "csv-separator-pipe"],
] as const;
const DATE_ORDERS: readonly CsvDateOrder[] = ["dmy", "mdy", "ymd"];

/** A column's place, as the person chose it. */
interface Choice {
  to: string;
  unit?: CsvUnit | undefined;
}

/** What the person set: the parts of the import's input the dialog changes. */
export interface CsvSettings {
  project: string;
  preset: string;
  encoding: CsvEncoding | "auto";
  separator: string;
  dateOrder: CsvDateOrder | "auto";
  columns: Readonly<Record<string, Choice>>;
  createMissing: boolean;
  skipInvalid: boolean;
}

/** The import's input for `settings`; a check lists only the rows with problems. */
export function requestOf(settings: CsvSettings, dryRun: boolean): CsvImportRequest {
  const format = {
    ...(settings.encoding === "auto" ? {} : { encoding: settings.encoding }),
    ...(settings.separator === "auto" ? {} : { separator: settings.separator }),
    ...(settings.dateOrder === "auto" ? {} : { date_order: settings.dateOrder }),
  };
  const columns: CsvColumnMapping[] = Object.entries(settings.columns).map(([column, choice]) => ({
    column,
    to: choice.to,
    ...(choice.unit === undefined ? {} : { unit: choice.unit }),
  }));
  return {
    project: settings.project,
    ...(settings.preset === "" ? {} : { preset: settings.preset }),
    ...(Object.keys(format).length > 0 ? { format } : {}),
    ...(columns.length > 0 ? { columns } : {}),
    create_missing: settings.createMissing,
    skip_invalid: settings.skipInvalid,
    only_problems: true,
    dry_run: dryRun,
  };
}

/** The mapping of a checked file, as a preset keeps it: every column that goes somewhere. */
export function presetColumns(columns: readonly CsvColumn[]): CsvColumnMapping[] {
  const seen = new Set<string>();
  return columns.flatMap((column) => {
    if (column.to === undefined || seen.has(column.name)) {
      return [];
    }
    seen.add(column.name);
    return [{ column: column.name, to: column.to, ...(column.unit === undefined ? {} : { unit: column.unit }) }];
  });
}

type Step = { kind: "choose" } | { kind: "mapping"; source: ImportSource } | { kind: "importing"; source: ImportSource };

/**
 * Import from CSV: choose a file (in the native chooser; in a browser, name
 * one in the exports folder), then see how it reads: the encoding,
 * separator and date order found, each column with examples and where it
 * goes, and every row with problems. Each change checks the file again.
 * The import runs as one change; its toast offers Undo, which moves the
 * new tasks to the trash. A mapping can be kept as a preset of the
 * workspace; the last preset used is the person's own preference.
 */
export function CsvImportDialog({ project: initialProject, onClose }: { project?: string | undefined; onClose: () => void }) {
  const t = useT();
  const { locale } = useLocale();
  const call = useCall();
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const fail = useFailureToast();
  const projects = useProjects();
  const active = (projects.data?.items ?? []).filter((one) => one.state === "active");
  const [step, setStep] = useState<Step>({ kind: "choose" });
  const [name, setName] = useState("");
  const [result, setResult] = useState<CsvImportResult | null>(null);
  const [checking, setChecking] = useState(false);
  const [presetName, setPresetName] = useState("");
  const remembered = readPref(PRESET_PREF);
  const [settings, setSettings] = useState<CsvSettings>({
    project: initialProject ?? "",
    preset: typeof remembered === "string" ? remembered : "",
    encoding: "auto",
    separator: "auto",
    dateOrder: "auto",
    columns: {},
    createMissing: true,
    skipInvalid: false,
  });
  const project = settings.project !== "" ? settings.project : (active[0]?.key ?? "");
  const fields = useOp("field.list", { project }, { enabled: project !== "" });
  const presets = useOp("csv_preset.list", {});
  // A remembered preset that was deleted since: detect again.
  const presetItems = presets.data?.items;
  const preset = settings.preset !== "" && presetItems !== undefined && !presetItems.some((one) => one.name === settings.preset) ? "" : settings.preset;
  const list = (names: readonly string[]) => new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(names);

  const source = step.kind === "choose" ? null : step.source;
  const request = requestOf({ ...settings, preset, project }, true);
  const requestKey = JSON.stringify(request);
  const latest = useRef(0);
  const answered = useEffectEvent((run: number, checked: CsvImportResult | null, error: unknown) => {
    if (latest.current !== run) {
      return;
    }
    setChecking(false);
    if (checked !== null) {
      setResult(checked);
      return;
    }
    fail(error);
    // A file that can't be read: start over.
    if (result === null) {
      setStep({ kind: "choose" });
    }
  });
  useEffect(() => {
    if (source === null || step.kind !== "mapping" || project === "" || presetItems === undefined) {
      return;
    }
    const run = latest.current + 1;
    latest.current = run;
    const wanted = JSON.parse(requestKey) as CsvImportRequest;
    const timer = window.setTimeout(() => {
      setChecking(true);
      importCsv(call, source, wanted).then(
        (checked) => {
          answered(run, checked, null);
        },
        (error: unknown) => {
          answered(run, null, error);
        },
      );
    }, RECHECK_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [source, requestKey, step.kind, project, call, presetItems]);

  const change = (patch: Partial<CsvSettings>) => {
    setSettings((now) => ({ ...now, ...patch }));
  };
  const choose = () => {
    chooseCsv().then((chosen) => {
      if (chosen !== null) {
        setStep({ kind: "mapping", source: chosen });
      }
    }, fail);
  };
  const run = () => {
    if (step.kind !== "mapping" || result === null) {
      return;
    }
    const chosen = step.source;
    setStep({ kind: "importing", source: chosen });
    importCsv(call, chosen, requestOf({ ...settings, preset, project }, false)).then(
      (imported) => {
        writePref(PRESET_PREF, preset === "" ? (imported.preset ?? "") : preset);
        const group = imported.group;
        toasts.show({
          tone: "info",
          text: t("csv-imported", { count: imported.counts.created, project: imported.project }),
          action:
            group === undefined
              ? undefined
              : {
                  label: t("toast-undo"),
                  run: () => {
                    call("history.revert", { groups: [group] }).then((reverted) => {
                      const skipped = reverted.skipped[0];
                      if (skipped) {
                        toasts.show({ tone: "error", text: t("history-not-reverted"), detail: skippedReason(t, skipped) });
                      }
                    }, fail);
                  },
                },
        });
        onClose();
      },
      (error: unknown) => {
        fail(error);
        setStep({ kind: "mapping", source: chosen });
      },
    );
  };
  const savePreset = () => {
    const presetTitle = presetName.trim();
    if (presetTitle === "" || result === null) {
      return;
    }
    const format = request.format;
    call("csv_preset.save", { name: presetTitle, columns: presetColumns(result.columns), ...(format === undefined ? {} : { format }) }).then((saved) => {
      queryClient.setQueryData(opKey("csv_preset.list", {}), { items: saved.items });
      toasts.show({ tone: "info", text: t("csv-preset-saved", { name: presetTitle }) });
      change({ preset: presetTitle, columns: {} });
      setPresetName("");
    }, fail);
  };

  const fieldNames = (fields.data?.fields ?? []).map((field) => field.name);
  const targetOptions: SelectOption<string>[] = [
    { id: "ignore", label: t("csv-target-ignore") },
    ...TARGETS.map((target) => ({ id: target, label: t(`csv-target-${target}`) })),
    ...fieldNames.map((field) => ({ id: `field.${field}`, label: t("csv-target-field", { name: field }) })),
  ];
  const unitOptions: SelectOption<CsvUnit>[] = UNITS.map((unit) => ({ id: unit, label: t(`csv-unit-${unit}`) }));
  const presetOptions: SelectOption<string>[] = [
    { id: "", label: result?.preset !== undefined && preset === "" ? t("csv-preset-detected", { name: result.preset }) : t("csv-preset-auto") },
    ...(presets.data?.items ?? []).map((preset) => ({ id: preset.name, label: preset.built_in === true ? t(`csv-preset-${preset.name}`) : preset.name })),
  ];
  const found = result?.format;
  const encodingOptions: SelectOption<CsvEncoding | "auto">[] = [
    { id: "auto", label: t("csv-encoding-auto", { found: found ? ENCODING_NAMES[found.encoding] : "…" }) },
    ...ENCODINGS.map((encoding) => ({ id: encoding, label: ENCODING_NAMES[encoding] })),
  ];
  const separatorName = (sep: string) => {
    const known = SEPARATORS.find(([each]) => each === sep);
    return known ? t(known[1]) : sep;
  };
  const separatorOptions: SelectOption<string>[] = [
    { id: "auto", label: t("csv-separator-auto", { found: found ? separatorName(found.separator) : "…" }) },
    ...SEPARATORS.map(([sep, label]) => ({ id: sep, label: t(label) })),
  ];
  const dateOptions: SelectOption<CsvDateOrder | "auto">[] = [
    { id: "auto", label: found?.date_order === undefined ? t("csv-date-auto") : t("csv-date-auto-found", { found: t(`csv-date-${found.date_order}`) }) },
    ...DATE_ORDERS.map((order) => ({ id: order, label: t(`csv-date-${order}`) })),
  ];
  const projectOptions: SelectOption<string>[] = active.map((one) => ({ id: one.key, label: t("csv-project-option", { key: one.key, name: one.name }) }));
  const problemRows = (result?.rows ?? []).filter((row) => (row.problems ?? []).length > 0);
  const busy = step.kind === "importing";
  const canImport = step.kind === "mapping" && result !== null && !checking && !result.blocked && result.counts.created > 0;

  return (
    <Dialog
      title={initialProject === undefined ? t("csv-title") : t("csv-title-into", { project: initialProject })}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose} isDisabled={busy}>
            {t("dialog-cancel")}
          </Button>
          {result && step.kind !== "choose" && (
            <Button className="btn primary" onPress={run} isDisabled={!canImport}>
              {t("csv-import-start", { count: result.counts.created })}
            </Button>
          )}
        </>
      }
    >
      <div className={styles.form} data-testid="csv-import">
        {step.kind === "choose" && (
          <>
            <p className="muted">{t("csv-choose-about")}</p>
            {initialProject === undefined && projectOptions.length > 0 && (
              <Select label={t("csv-project")} options={projectOptions} value={project} onChange={(key) => { change({ project: key }); }} />
            )}
            {choosesFiles() ? (
              <div>
                <Button className="btn" onPress={choose} isDisabled={project === ""}>
                  <FileSpreadsheet className="ic s" />
                  {t("csv-choose")}
                </Button>
              </div>
            ) : (
              <form
                className={styles.row}
                onSubmit={(event) => {
                  event.preventDefault();
                  if (name.trim() !== "" && project !== "") {
                    setStep({ kind: "mapping", source: { path: name.trim(), name: name.trim() } });
                  }
                }}
              >
                <span className="field grow">
                  <input aria-label={t("csv-file-name")} placeholder={t("csv-file-name")} value={name} onChange={(event) => { setName(event.target.value); }} />
                </span>
                <Button type="submit" className="btn" isDisabled={name.trim() === "" || project === ""}>
                  {t("csv-check")}
                </Button>
              </form>
            )}
          </>
        )}
        {source !== null && (
          <p className={classes("muted", styles.path)}>{t("csv-file", { name: source.name })}</p>
        )}
        {(checking || busy) && (
          <div role="status" className={styles.row}>
            <div className="meter grow" role="progressbar" aria-label={t(busy ? "csv-importing" : "csv-checking")}>
              <i style={{ inlineSize: "100%" }} />
            </div>
          </div>
        )}
        {source !== null && result && (
          <>
            <div className={styles.settings}>
              {initialProject === undefined && <Select label={t("csv-project")} options={projectOptions} value={project} onChange={(key) => { change({ project: key, columns: {} }); }} isDisabled={busy} />}
              <Select label={t("csv-preset")} options={presetOptions} value={preset} onChange={(preset) => { change({ preset, columns: {} }); }} isDisabled={busy} />
              <Select label={t("csv-encoding")} options={encodingOptions} value={settings.encoding} onChange={(encoding) => { change({ encoding }); }} isDisabled={busy} />
              <Select label={t("csv-separator")} options={separatorOptions} value={settings.separator} onChange={(separator) => { change({ separator, columns: {} }); }} isDisabled={busy} />
              <Select label={t("csv-date-order")} options={dateOptions} value={settings.dateOrder} onChange={(dateOrder) => { change({ dateOrder }); }} isDisabled={busy} />
            </div>
            <section aria-label={t("csv-columns")} className={styles.scroll}>
              <table className={styles.table}>
                <thead>
                  <tr>
                    <th scope="col">{t("csv-column")}</th>
                    <th scope="col">{t("csv-column-samples")}</th>
                    <th scope="col">{t("csv-column-target")}</th>
                    <th scope="col">{t("csv-column-unit")}</th>
                  </tr>
                </thead>
                <tbody>
                  {result.columns.map((column) => {
                    const to = column.to ?? "ignore";
                    const timed = to === "estimate" || to === "spent";
                    return (
                      <tr key={column.index} data-column={column.name}>
                        <td className="selectable">{column.name}</td>
                        <td className={classes("muted", "selectable", styles.samples)}>{column.samples?.join(" · ")}</td>
                        <td>
                          <Select
                            label={t("csv-column-target-of", { column: column.name })}
                            options={targetOptions}
                            value={targetOptions.some((option) => option.id === to) ? to : "ignore"}
                            onChange={(target) => {
                              change({ columns: { ...settings.columns, [column.name]: { to: target, unit: target === "estimate" || target === "spent" ? (column.unit ?? "text") : undefined } } });
                            }}
                            isDisabled={busy}
                          />
                        </td>
                        <td>
                          {timed && (
                            <Select
                              label={t("csv-column-unit-of", { column: column.name })}
                              options={unitOptions}
                              value={column.unit ?? "text"}
                              onChange={(unit) => {
                                change({ columns: { ...settings.columns, [column.name]: { to, unit } } });
                              }}
                              isDisabled={busy}
                            />
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </section>
            <div className={styles.settings}>
              <Switch isSelected={settings.createMissing} onChange={(createMissing) => { change({ createMissing }); }} isDisabled={busy}>
                {t("csv-create-missing")}
              </Switch>
              <Switch isSelected={settings.skipInvalid} onChange={(skipInvalid) => { change({ skipInvalid }); }} isDisabled={busy}>
                {t("csv-skip-invalid")}
              </Switch>
            </div>
            <section aria-label={t("csv-summary")} data-testid="csv-summary">
              <ul className={styles.summary}>
                <li>{t("csv-summary-rows", { rows: result.counts.rows })}</li>
                <li>{t("csv-summary-created", { count: result.counts.created })}</li>
                <li>{t("csv-summary-skipped", { count: result.counts.skipped })}</li>
                <li>{t("csv-summary-warnings", { count: result.counts.warnings })}</li>
                <li>{t("csv-summary-links", { parents: result.counts.parents, links: result.counts.links, entries: result.counts.time_entries, comments: result.counts.comments })}</li>
                {(result.new_statuses ?? []).length > 0 && <li>{t("csv-new-statuses", { names: list(result.new_statuses ?? []) })}</li>}
                {(result.new_types ?? []).length > 0 && <li>{t("csv-new-types", { names: list(result.new_types ?? []) })}</li>}
                {(result.new_labels ?? []).length > 0 && <li>{t("csv-new-labels", { names: list(result.new_labels ?? []) })}</li>}
              </ul>
              <p className={styles.note}>{t("csv-no-recurrence")}</p>
              {result.blocked && <p className={styles.bad}>{t("csv-blocked")}</p>}
            </section>
            <section aria-label={t("csv-problems")} className={styles.scroll} data-testid="csv-problems">
              {problemRows.length === 0 ? (
                <p className="muted">{t("csv-problems-none")}</p>
              ) : (
                <table className={styles.table}>
                  <thead>
                    <tr>
                      <th scope="col">{t("csv-problem-line")}</th>
                      <th scope="col">{t("csv-problem-row")}</th>
                      <th scope="col">{t("csv-problem-column")}</th>
                      <th scope="col">{t("csv-problem-what")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {problemRows.slice(0, PROBLEMS_SHOWN).flatMap((row) =>
                      (row.problems ?? []).map((problem, index) => (
                        <tr key={`${String(row.line)}-${String(index)}`} className={problem.severity === "error" ? styles.bad : undefined}>
                          <td className={styles.number}>{new Intl.NumberFormat(locale).format(row.line)}</td>
                          <td className="selectable">{row.source_key ?? row.title}</td>
                          <td className="selectable">{problem.column ?? ""}</td>
                          <td>
                            <span className={styles.severity}>{t(problem.severity === "error" ? "csv-problem-error" : "csv-problem-warning")}</span> {problem.message}
                          </td>
                        </tr>
                      )),
                    )}
                  </tbody>
                </table>
              )}
              {problemRows.length > PROBLEMS_SHOWN && <p className="muted">{t("csv-problems-more", { count: problemRows.length - PROBLEMS_SHOWN })}</p>}
            </section>
            <form
              className={styles.row}
              onSubmit={(event) => {
                event.preventDefault();
                savePreset();
              }}
            >
              <span className="field grow">
                <input aria-label={t("csv-save-preset-name")} placeholder={t("csv-save-preset-name")} value={presetName} onChange={(event) => { setPresetName(event.target.value); }} maxLength={60} disabled={busy} />
              </span>
              <Button type="submit" className="btn" isDisabled={presetName.trim() === "" || busy}>
                {t("csv-save-preset")}
              </Button>
            </form>
          </>
        )}
      </div>
    </Dialog>
  );
}
