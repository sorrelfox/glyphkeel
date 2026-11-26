// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ChevronDown } from "lucide-react";
import { useId, useState } from "react";
import { Button, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { PRIORITIES, type Project, type Status } from "../../data/catalog";
import { useCommand } from "../../data/hooks";
import { CLEARABLE_FIELDS, LOCKABLE_FIELDS, REQUIRABLE_FIELDS, isCustom, isEmpty } from "../../data/rules";
import type { Priority, RuleSet, StatusRules } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Rules.module.css";

type Progress = "keep" | "0" | "100";
type PriorityChoice = "keep" | Priority;

/**
 * Project settings › Rules: what happens to a task entering each status,
 * what it needs first and what can't change in it, and the rules for
 * agents. Every change applies at once and can be undone.
 */
export function RulesSettings({ project }: { project: Project }) {
  const t = useT();
  const update = useCommand("rules.update", { undoable: t("toast-rules-changed") });
  const readOnly = project.state === "archived";
  const agentFields = project.agent_rules?.read_only ?? [];
  const setStatusRules = (status: Status, rules: StatusRules) => {
    update.mutate({ project: project.key, statuses: [{ status: status.id, rules }] });
  };
  return (
    <div className={styles.rules}>
      <p className="muted">{t("rules-intro")}</p>
      {project.statuses.map((status) => (
        <StatusRulesCard key={status.id} status={status} readOnly={readOnly} onChange={(rules) => { setStatusRules(status, rules); }} />
      ))}
      <section className={styles.card} aria-labelledby="rules-agents">
        <h3 id="rules-agents" className={styles.heading}>
          {t("rules-agents-heading")}
        </h3>
        <p className="muted">{t("rules-agents-intro")}</p>
        <FieldChecks
          legend={t("rules-agents-read-only")}
          fields={LOCKABLE_FIELDS}
          chosen={agentFields}
          readOnly={readOnly}
          onChange={(read_only) => {
            update.mutate({ project: project.key, agents: { read_only } });
          }}
        />
      </section>
    </div>
  );
}

function StatusRulesCard({ status, readOnly, onChange }: { status: Status; readOnly: boolean; onChange: (rules: StatusRules) => void }) {
  const t = useT();
  const { locale } = useLocale();
  const rules = status.rules ?? {};
  const [open, setOpen] = useState(!isEmpty(rules));
  const bodyId = useId();
  const percent = new Intl.NumberFormat(locale, { style: "percent" });
  const set = rules.set ?? [];
  const setOf = (field: string) => set.find((entry) => entry.field === field);
  const withSet = (field: string, entry: RuleSet | null): StatusRules => {
    const others = set.filter((candidate) => candidate.field !== field);
    return { ...rules, set: entry === null ? others : [...others, entry] };
  };
  const count = countRules(rules);
  const progress: Progress = (setOf("progress")?.value as Progress | undefined) ?? "keep";
  const priority: PriorityChoice = (setOf("priority")?.value as Priority | undefined) ?? "keep";
  return (
    <section className={styles.card} data-testid={`rules-${status.name}`} aria-label={t("rules-status-label", { name: status.name })}>
      <Button
        className={classes("btn flat", styles.toggle)}
        aria-expanded={open}
        aria-controls={bodyId}
        onPress={() => {
          setOpen(!open);
        }}
      >
        <span className={`dot hue-${status.hue}`} />
        <span className={styles.heading}>{status.name}</span>
        <span className="muted">{t("rules-count", { count })}</span>
        <ChevronDown className={classes("ic s muted", !open && styles.turned)} />
      </Button>
      {open && (
        <div id={bodyId} className={styles.body}>
          <fieldset className={styles.group} disabled={readOnly}>
            <legend className={styles.legend}>{t("rules-on-entry")}</legend>
            {(["start_date", "due_date"] as const).map((field) => (
              <label key={field} className={styles.check}>
                <input
                  type="checkbox"
                  className="check"
                  checked={setOf(field) !== undefined}
                  onChange={(event) => {
                    onChange(withSet(field, event.target.checked ? { field, value: "today", only_if_empty: true } : null));
                  }}
                />
                {t(`rules-${field}-today`)}
              </label>
            ))}
            <div className={styles.choice}>
              <Select<Progress>
                label={t("rules-progress")}
                value={progress}
                isDisabled={readOnly}
                onChange={(value) => {
                  onChange(withSet("progress", value === "keep" ? null : { field: "progress", value }));
                }}
                options={[
                  { id: "keep", label: t("rules-progress-keep") },
                  { id: "0", label: t("rules-progress-to", { value: percent.format(0) }) },
                  { id: "100", label: t("rules-progress-to", { value: percent.format(1) }) },
                ]}
              />
              <Select<PriorityChoice>
                label={t("rules-priority")}
                value={priority}
                isDisabled={readOnly}
                onChange={(value) => {
                  onChange(withSet("priority", value === "keep" ? null : { field: "priority", value }));
                }}
                options={[{ id: "keep", label: t("rules-priority-keep") }, ...[...PRIORITIES].reverse().map((p) => ({ id: p, label: t("rules-priority-to", { value: t(`priority-${p}`) }) }))]}
              />
            </div>
            <label className={styles.check}>
              <input
                type="checkbox"
                className="check"
                checked={rules.stop_timer === true}
                onChange={(event) => {
                  onChange({ ...rules, stop_timer: event.target.checked });
                }}
              />
              {t("rules-stop-timer")}
            </label>
            <label className={styles.check}>
              <input
                type="checkbox"
                className="check"
                checked={rules.release_claim === true}
                onChange={(event) => {
                  onChange({ ...rules, release_claim: event.target.checked });
                }}
              />
              {t("rules-release-claim")}
            </label>
          </fieldset>
          <FieldChecks
            legend={t("rules-clear")}
            fields={CLEARABLE_FIELDS}
            chosen={rules.clear ?? []}
            disabledFields={rules.required ?? []}
            readOnly={readOnly}
            onChange={(clear) => {
              onChange({ ...rules, clear });
            }}
          />
          <FieldChecks
            legend={t("rules-required")}
            fields={REQUIRABLE_FIELDS}
            chosen={rules.required ?? []}
            disabledFields={rules.clear ?? []}
            readOnly={readOnly}
            onChange={(required) => {
              onChange({ ...rules, required });
            }}
          />
          <FieldChecks
            legend={t("rules-read-only")}
            fields={LOCKABLE_FIELDS}
            chosen={rules.read_only ?? []}
            readOnly={readOnly}
            onChange={(read_only) => {
              onChange({ ...rules, read_only });
            }}
          />
          <Switch
            isSelected={rules.agents_may_not_enter === true}
            isDisabled={readOnly}
            onChange={(on) => {
              onChange({ ...rules, agents_may_not_enter: on });
            }}
          >
            {t("rules-agents-may-not-enter")}
          </Switch>
        </div>
      )}
    </section>
  );
}

/**
 * A group of checkboxes, one per field. Custom fields already chosen (set
 * from the command line, say) stay chosen: the list keeps them.
 */
function FieldChecks({
  legend,
  fields,
  chosen,
  disabledFields = [],
  readOnly,
  onChange,
}: {
  legend: string;
  fields: readonly string[];
  chosen: readonly string[];
  disabledFields?: readonly string[];
  readOnly: boolean;
  onChange: (fields: string[]) => void;
}) {
  const t = useT();
  return (
    <fieldset className={styles.group} disabled={readOnly}>
      <legend className={styles.legend}>{legend}</legend>
      <div className={styles.fields}>
        {fields.map((field) => (
          <label key={field} className={styles.check}>
            <input
              type="checkbox"
              className="check"
              checked={chosen.includes(field)}
              disabled={disabledFields.includes(field)}
              onChange={(event) => {
                const kept = chosen.filter((candidate) => candidate !== field);
                onChange(event.target.checked ? [...kept, field] : kept);
              }}
            />
            {t(`rule-field-${field}`)}
          </label>
        ))}
        {chosen.filter(isCustom).map((field) => (
          <label key={field} className={styles.check}>
            <input
              type="checkbox"
              className="check"
              checked
              onChange={() => {
                onChange(chosen.filter((candidate) => candidate !== field));
              }}
            />
            {t("rule-field-custom")}
          </label>
        ))}
      </div>
    </fieldset>
  );
}

/** How many rules a status has, for its collapsed card. */
function countRules(rules: StatusRules): number {
  return (
    (rules.set ?? []).length +
    (rules.clear ?? []).length +
    (rules.required ?? []).length +
    (rules.read_only ?? []).length +
    (rules.stop_timer === true ? 1 : 0) +
    (rules.release_claim === true ? 1 : 0) +
    (rules.agents_may_not_enter === true ? 1 : 0)
  );
}
