// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ArrowDown, ArrowUp, ChevronDown, Pencil, Plus, Trash2 } from "lucide-react";
import { useId, useState } from "react";
import { Button, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import {
  ACTIONS,
  CLEARABLE,
  EVERY,
  LINKS,
  PLACEHOLDERS,
  SETTABLE,
  TRIGGERS,
  WATCHABLE,
  WEEKDAYS,
  actionsSummary,
  cleanAction,
  cleanTrigger,
  fieldName,
  newAction,
  newDraft,
  setField,
  triggerSummary,
  weekdayDate,
  withTriggerKind,
  type Draft,
  type Naming,
  type Settable,
} from "../../data/automation";
import { PRIORITIES, type Project } from "../../data/catalog";
import { useCommand, useOp, failureText } from "../../data/hooks";
import { useTemplates } from "../../data/recurrence";
import type { ActionKind, AutomationAction, AutomationRule, AutomationTrigger, AutomationRun, AutomationTestChange, AutomationTestResult, RelationKind, ScheduleEvery, TriggerKind, Weekday } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import styles from "./Automation.module.css";

/** How the page names lists, weekdays and moments in the person's language. */
function useNaming(): Naming & { moment: (at: string) => string } {
  const { locale } = useLocale();
  const list = new Intl.ListFormat(locale, { style: "long", type: "conjunction" });
  const weekday = new Intl.DateTimeFormat(locale, { weekday: "long", timeZone: "UTC" });
  const moment = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" });
  return {
    list: (items) => list.format(items),
    weekday: (day) => weekday.format(weekdayDate(day)),
    moment: (at) => moment.format(new Date(at)),
  };
}

/** A rule being written: new, or one already saved. */
interface Editing {
  id?: string;
  draft: Draft;
}

/**
 * Project settings › Automation: the rules of the project and of the whole
 * workspace, each with its switch and run log, and the editor with its dry
 * run. Saving, switching and deleting can be undone.
 */
export function AutomationSettings({ project }: { project: Project }) {
  const t = useT();
  const rules = useOp("automation.list", { project: project.key });
  const [editing, setEditing] = useState<Editing | null>(null);
  const readOnly = project.state === "archived";
  if (editing !== null) {
    return (
      <RuleEditor
        project={project}
        editing={editing}
        onDone={() => {
          setEditing(null);
        }}
      />
    );
  }
  const items = rules.data?.items ?? [];
  return (
    <div className={styles.automation}>
      <p className="muted">{t("automation-intro")}</p>
      <div>
        <Button
          className="btn"
          isDisabled={readOnly}
          onPress={() => {
            const draft = newDraft(project.key);
            const label = project.labels[0]?.name;
            setEditing({ draft: { ...draft, actions: label === undefined ? [newAction("add_comment")] : [{ kind: "add_label", label }] } });
          }}
        >
          <Plus className="ic s" />
          {t("automation-new")}
        </Button>
      </div>
      {rules.isError && <p className="faint">{failureText(t, rules.error)}</p>}
      {rules.data && items.length === 0 && <p className="faint">{t("automation-none")}</p>}
      {items.length > 0 && (
        <ul className={styles.list} aria-label={t("automation-list")}>
          {items.map((rule) => (
            <RuleRow
              key={rule.id}
              rule={rule}
              readOnly={readOnly}
              onEdit={() => {
                setEditing({
                  id: rule.id,
                  draft: { name: rule.name, project: rule.project ?? "", enabled: rule.enabled, trigger: rule.trigger, condition: rule.condition ?? "", actions: rule.actions },
                });
              }}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

function RuleRow({ rule, readOnly, onEdit }: { rule: AutomationRule; readOnly: boolean; onEdit: () => void }) {
  const t = useT();
  const naming = useNaming();
  const update = useCommand("automation.update", { undoable: t("toast-automation-changed") });
  const remove = useCommand("automation.delete", { undoable: t("toast-automation-deleted") });
  const [open, setOpen] = useState(false);
  const logId = useId();
  const ref = { rule: rule.id };
  return (
    <li className={styles.rule} data-testid={`automation-${rule.name}`}>
      <div className={styles.head}>
        <Switch
          aria-label={t("automation-enabled-label", { name: rule.name })}
          isSelected={rule.enabled}
          isDisabled={readOnly}
          onChange={(enabled) => {
            update.mutate({ ...ref, enabled });
          }}
        />
        <h3 className={styles.name}>{rule.name}</h3>
        {rule.project === undefined && <span className="chip">{t("automation-workspace")}</span>}
        {rule.held_as_agent === true && <span className="chip hue-purple">{t("automation-held")}</span>}
        <Button className="btn flat" aria-label={t("automation-edit-label", { name: rule.name })} isDisabled={readOnly} onPress={onEdit}>
          <Pencil className="ic s" />
        </Button>
        <Button
          className="btn flat"
          aria-label={t("automation-delete-label", { name: rule.name })}
          isDisabled={readOnly}
          onPress={() => {
            remove.mutate(ref);
          }}
        >
          <Trash2 className="ic s" />
        </Button>
      </div>
      <p className={styles.summary}>
        <span>{triggerSummary(t, rule.trigger, naming)}</span>
        {rule.condition !== undefined && rule.condition !== "" && <code className={styles.condition}>{rule.condition}</code>}
        <span className="muted">{actionsSummary(t, rule.actions, naming)}</span>
      </p>
      <div className={styles.head}>
        {rule.last_run && (
          <span className={styles.last}>
            <RunChip run={rule.last_run} />
            <span className="muted">{t("automation-last-run", { time: naming.moment(rule.last_run.at) })}</span>
          </span>
        )}
        <Button
          className={classes("btn flat", styles.toggle)}
          aria-expanded={open}
          aria-controls={logId}
          aria-label={t("automation-runs-label", { name: rule.name })}
          onPress={() => {
            setOpen(!open);
          }}
        >
          {t("automation-runs-show")}
          <ChevronDown className={classes("ic s muted", !open && styles.turned)} />
        </Button>
      </div>
      {open && <RunLog id={logId} rule={rule} />}
    </li>
  );
}

function RunChip({ run }: { run: AutomationRun }) {
  const t = useT();
  const hue = run.result === "done" ? "hue-green" : run.result === "failed" ? "hue-red" : "hue-orange";
  return <span className={classes("chip", hue)}>{t(`automation-run-${run.result}`)}</span>;
}

function RunLog({ id, rule }: { id: string; rule: AutomationRule }) {
  const t = useT();
  const naming = useNaming();
  const runs = useOp("automation.runs", { rule: rule.id });
  const items = runs.data?.items ?? [];
  return (
    <div id={id}>
      {runs.data && items.length === 0 && <p className="faint">{t("automation-runs-none")}</p>}
      {items.length > 0 && (
        <ol className={styles.runs} aria-label={t("automation-runs-label", { name: rule.name })}>
          {items.map((run) => (
            <li key={run.id} className={styles.run}>
              <RunChip run={run} />
              <time dateTime={run.at}>{naming.moment(run.at)}</time>
              {run.task !== undefined && <span className={styles.key}>{run.task}</span>}
              <span className="muted">{t(`automation-trigger-${run.trigger}`)}</span>
              {run.result === "done" && <span className="muted">{t("automation-run-changes", { count: run.changes })}</span>}
              {run.error && <span className={styles.error}>{failureText(t, run.error)}</span>}
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------
// The editor

function RuleEditor({ project, editing, onDone }: { project: Project; editing: Editing; onDone: () => void }) {
  const t = useT();
  const naming = useNaming();
  const [draft, setDraft] = useState<Draft>(editing.draft);
  const create = useCommand("automation.create", { undoable: t("toast-automation-saved") });
  const update = useCommand("automation.update", { undoable: t("toast-automation-changed") });
  const conditionHint = useId();
  const inProject = draft.project !== "";
  const change = (patch: Partial<Draft>) => {
    setDraft({ ...draft, ...patch });
  };
  const setAction = (index: number, action: AutomationAction) => {
    change({ actions: draft.actions.map((each, i) => (i === index ? action : each)) });
  };
  const move = (index: number, by: number) => {
    const next = [...draft.actions];
    const [taken] = next.splice(index, 1);
    if (taken !== undefined) {
      next.splice(index + by, 0, taken);
      change({ actions: next });
    }
  };
  const body = {
    trigger: cleanTrigger(draft.trigger),
    condition: draft.condition.trim(),
    actions: draft.actions.map(cleanAction),
  };
  const save = (event: { preventDefault: () => void }) => {
    event.preventDefault();
    const done = { onSuccess: onDone };
    if (editing.id === undefined) {
      create.mutate({ name: draft.name.trim(), enabled: draft.enabled, ...(inProject ? { project: draft.project } : {}), ...body }, done);
    } else {
      update.mutate({ rule: editing.id, name: draft.name.trim(), enabled: draft.enabled, ...body }, done);
    }
  };
  const pending = create.isPending || update.isPending;
  return (
    <form className={styles.editor} aria-label={t(editing.id === undefined ? "automation-editor-new" : "automation-editor-edit")} onSubmit={save}>
      <h3 className={styles.name}>{t(editing.id === undefined ? "automation-editor-new" : "automation-editor-edit")}</h3>
      <label className={styles.row}>
        <span>{t("automation-name")}</span>
        <span className="field grow">
          <input
            required
            maxLength={100}
            value={draft.name}
            onChange={(event) => {
              change({ name: event.target.value });
            }}
          />
        </span>
      </label>
      {editing.id === undefined && (
        <Select<string>
          label={t("automation-scope")}
          value={draft.project === "" ? "" : "project"}
          onChange={(value) => {
            change({ project: value === "" ? "" : project.key });
          }}
          options={[
            { id: "project", label: t("automation-scope-project", { project: project.key }) },
            { id: "", label: t("automation-scope-workspace") },
          ]}
        />
      )}
      <Switch
        isSelected={draft.enabled}
        onChange={(enabled) => {
          change({ enabled });
        }}
      >
        {t("automation-enabled")}
      </Switch>

      <fieldset className={styles.group}>
        <legend className={styles.legend}>{t("automation-trigger")}</legend>
        <Select<TriggerKind>
          label={t("automation-trigger")}
          value={draft.trigger.on}
          onChange={(on) => {
            change({ trigger: withTriggerKind(draft.trigger, on) });
          }}
          options={TRIGGERS.map((on) => ({ id: on, label: t(`automation-trigger-${on}`) }))}
        />
        {draft.trigger.on === "field_changed" && (
          <fieldset className={styles.group}>
            <legend className={styles.legend}>{t("automation-trigger-fields")}</legend>
            <p className="faint">{t("automation-trigger-fields-any")}</p>
            <div className={styles.fields}>
              {WATCHABLE.map((field) => {
                const chosen = draft.trigger.fields ?? [];
                return (
                  <label key={field} className={styles.check}>
                    <input
                      type="checkbox"
                      className="check"
                      checked={chosen.includes(field)}
                      onChange={(event) => {
                        const kept = chosen.filter((each) => each !== field);
                        change({ trigger: { ...draft.trigger, fields: event.target.checked ? [...kept, field] : kept } });
                      }}
                    />
                    {fieldName(t, field)}
                  </label>
                );
              })}
            </div>
          </fieldset>
        )}
        {draft.trigger.on === "moved_to_status" && (
          <NameChoice
            label={t("automation-trigger-status")}
            names={inProject ? project.statuses.map((status) => status.name) : undefined}
            value={draft.trigger.status ?? ""}
            emptyLabel={t("automation-status-any")}
            onChange={(status) => {
              const next: AutomationTrigger = { ...draft.trigger, status };
              if (status === "") {
                delete next.status;
              }
              change({ trigger: next });
            }}
          />
        )}
        {draft.trigger.on === "schedule" && (
          <div className={styles.choice}>
            <Select<ScheduleEvery>
              label={t("automation-every")}
              value={draft.trigger.every ?? "daily"}
              onChange={(every) => {
                change({ trigger: cleanTrigger({ ...draft.trigger, every, ...(every === "weekly" ? { weekday: draft.trigger.weekday ?? "mon" } : {}) }) });
              }}
              options={EVERY.map((every) => ({ id: every, label: t(`automation-every-${every}`) }))}
            />
            {draft.trigger.every === "weekly" && (
              <Select<Weekday>
                label={t("automation-weekday")}
                value={draft.trigger.weekday ?? "mon"}
                onChange={(weekday) => {
                  change({ trigger: { ...draft.trigger, weekday } });
                }}
                options={WEEKDAYS.map((day) => ({ id: day, label: naming.weekday(day) }))}
              />
            )}
            <label className={styles.row}>
              <span>{t("automation-at-time")}</span>
              <span className="field">
                <input
                  type="time"
                  required
                  value={draft.trigger.time ?? "09:00"}
                  onChange={(event) => {
                    change({ trigger: { ...draft.trigger, time: event.target.value } });
                  }}
                />
              </span>
            </label>
          </div>
        )}
      </fieldset>

      <label className={styles.stack}>
        <span>{t("automation-condition")}</span>
        <span className="field grow">
          <input
            className={styles.mono}
            aria-describedby={conditionHint}
            value={draft.condition}
            required={draft.trigger.on === "schedule"}
            onChange={(event) => {
              change({ condition: event.target.value });
            }}
          />
        </span>
        <span id={conditionHint} className="faint">
          {t("automation-condition-hint")}
        </span>
      </label>

      <fieldset className={styles.group}>
        <legend className={styles.legend}>{t("automation-actions")}</legend>
        <ol className={styles.actions}>
          {draft.actions.map((action, index) => (
            <li key={index}>
              <ActionEditor
                index={index}
                count={draft.actions.length}
                action={action}
                project={project}
                inProject={inProject}
                onChange={(next) => {
                  setAction(index, next);
                }}
                onMove={(by) => {
                  move(index, by);
                }}
                onRemove={() => {
                  change({ actions: draft.actions.filter((_, i) => i !== index) });
                }}
              />
            </li>
          ))}
        </ol>
        <div>
          <Button
            className="btn flat"
            isDisabled={draft.actions.length >= 20}
            onPress={() => {
              change({ actions: [...draft.actions, newAction("add_comment")] });
            }}
          >
            <Plus className="ic s" />
            {t("automation-add-action")}
          </Button>
        </div>
        <p className="faint">{t("automation-placeholders", { list: naming.list(PLACEHOLDERS.map((name) => `{${name}}`)) })}</p>
      </fieldset>

      <TryIt project={project} name={draft.name} body={body} />

      <div className={styles.buttons}>
        <button type="button" className="btn flat" onClick={onDone}>
          {t("automation-cancel")}
        </button>
        <button type="submit" className="btn primary" disabled={pending || draft.name.trim() === "" || draft.actions.length === 0}>
          {t("automation-save")}
        </button>
      </div>
    </form>
  );
}

/** A status or label by name: a choice among the project's, or typed for a rule of every project. */
function NameChoice({ label, names, value, emptyLabel, onChange }: { label: string; names: readonly string[] | undefined; value: string; emptyLabel?: string; onChange: (value: string) => void }) {
  if (names === undefined) {
    return (
      <label className={styles.row}>
        <span>{label}</span>
        <span className="field grow">
          <input
            value={value}
            placeholder={emptyLabel}
            onChange={(event) => {
              onChange(event.target.value);
            }}
          />
        </span>
      </label>
    );
  }
  const options = [...(emptyLabel === undefined ? [] : [{ id: "", label: emptyLabel }]), ...names.map((name) => ({ id: name, label: name }))];
  const known = options.some((option) => option.id === value);
  return <Select<string> label={label} value={known ? value : (options[0]?.id ?? "")} onChange={onChange} options={options} />;
}

function ActionEditor({
  index,
  count,
  action,
  project,
  inProject,
  onChange,
  onMove,
  onRemove,
}: {
  index: number;
  count: number;
  action: AutomationAction;
  project: Project;
  inProject: boolean;
  onChange: (action: AutomationAction) => void;
  onMove: (by: number) => void;
  onRemove: () => void;
}) {
  const t = useT();
  const n = index + 1;
  const templates = useTemplates("task", project.key, action.kind === "create_subtask" || action.kind === "create_linked_task");
  const text = (key: "text", label: string, multiline: boolean) => (
    <label className={styles.stack}>
      <span>{label}</span>
      <span className="field grow">
        {multiline ? (
          <textarea
            rows={3}
            value={action[key] ?? ""}
            onChange={(event) => {
              onChange({ ...action, [key]: event.target.value });
            }}
          />
        ) : (
          <input
            value={action[key] ?? ""}
            onChange={(event) => {
              onChange({ ...action, [key]: event.target.value });
            }}
          />
        )}
      </span>
    </label>
  );
  // A label to pick, but keep the first one chosen when the action starts empty.
  const firstLabel = project.labels[0]?.name ?? "";
  const firstStatus = project.statuses[0]?.name ?? "";
  return (
    <fieldset className={styles.action} aria-label={t("automation-action-label", { n })}>
      <div className={styles.head}>
        <Select<ActionKind>
          label={t("automation-action-kind")}
          value={action.kind}
          onChange={(kind) => {
            const fresh = newAction(kind);
            onChange(kind === "add_label" || kind === "remove_label" ? { ...fresh, label: inProject ? firstLabel : "" } : kind === "move_to_status" ? { ...fresh, status: inProject ? firstStatus : "" } : fresh);
          }}
          options={ACTIONS.map((kind) => ({ id: kind, label: t(`automation-action-${kind}`) }))}
        />
        <span className={styles.spacer} />
        <Button className="btn flat" aria-label={t("automation-action-up", { n })} isDisabled={index === 0} onPress={() => { onMove(-1); }}>
          <ArrowUp className="ic s" />
        </Button>
        <Button className="btn flat" aria-label={t("automation-action-down", { n })} isDisabled={index === count - 1} onPress={() => { onMove(1); }}>
          <ArrowDown className="ic s" />
        </Button>
        <Button className="btn flat" aria-label={t("automation-action-remove", { n })} isDisabled={count === 1} onPress={onRemove}>
          <Trash2 className="ic s" />
        </Button>
      </div>
      {action.kind === "set_fields" && <SetFieldEditor action={action} project={project} inProject={inProject} onChange={onChange} />}
      {(action.kind === "add_label" || action.kind === "remove_label") && (
        <NameChoice
          label={t("automation-label")}
          names={inProject ? project.labels.map((label) => label.name) : undefined}
          value={action.label ?? (inProject ? firstLabel : "")}
          onChange={(label) => {
            onChange({ ...action, label });
          }}
        />
      )}
      {action.kind === "move_to_status" && (
        <NameChoice
          label={t("automation-status")}
          names={inProject ? project.statuses.map((status) => status.name) : undefined}
          value={action.status ?? (inProject ? firstStatus : "")}
          onChange={(status) => {
            onChange({ ...action, status });
          }}
        />
      )}
      {(action.kind === "add_comment" || action.kind === "notify") && text("text", t("automation-text"), true)}
      {(action.kind === "create_subtask" || action.kind === "create_linked_task") && (
        <div className={styles.choice}>
          {text("text", t("automation-title"), false)}
          <Select<string>
            label={t("automation-template")}
            value={action.template ?? ""}
            onChange={(template) => {
              const next: AutomationAction = { ...action, template };
              if (template === "") {
                delete next.template;
              }
              onChange(next);
            }}
            options={[{ id: "", label: t("automation-template-none") }, ...(templates.data?.items ?? []).map((template) => ({ id: template.name, label: template.name }))]}
          />
          {action.kind === "create_linked_task" && (
            <Select<RelationKind>
              label={t("automation-link")}
              value={action.link ?? "relates"}
              onChange={(link) => {
                onChange({ ...action, link });
              }}
              options={LINKS.map((link) => ({ id: link, label: t(`relation-${link}`) }))}
            />
          )}
        </div>
      )}
    </fieldset>
  );
}

/** The one field a `set_fields` action sets, and its value or that it is cleared. */
function SetFieldEditor({ action, project, inProject, onChange }: { action: AutomationAction; project: Project; inProject: boolean; onChange: (action: AutomationAction) => void }) {
  const t = useT();
  const { field, value } = setField(action);
  const set = (next: Settable, nextValue: unknown) => {
    onChange({ ...action, fields: { [next]: nextValue } });
  };
  const cleared = value === null;
  const valueField = (() => {
    if (cleared) {
      return null;
    }
    switch (field) {
      case "priority":
        return (
          <Select<string>
            label={t("automation-value")}
            value={typeof value === "string" ? value : "high"}
            onChange={(next) => {
              set(field, next);
            }}
            options={[...PRIORITIES].reverse().map((priority) => ({ id: priority, label: t(`priority-${priority}`) }))}
          />
        );
      case "type":
        if (inProject) {
          return (
            <NameChoice
              label={t("automation-value")}
              names={project.types.map((type) => type.name)}
              value={typeof value === "string" ? value : ""}
              onChange={(next) => {
                set(field, next);
              }}
            />
          );
        }
        break;
      case "progress":
        return (
          <label className={styles.row}>
            <span>{t("automation-value")}</span>
            <span className="field">
              <input
                type="number"
                min={0}
                max={100}
                value={typeof value === "number" ? value : 0}
                onChange={(event) => {
                  set(field, Math.max(0, Math.min(100, Number(event.target.value))));
                }}
              />
            </span>
          </label>
        );
      default:
        break;
    }
    const dated = field === "due_date" || field === "start_date";
    return (
      <label className={styles.row}>
        <span>{t("automation-value")}</span>
        <span className="field grow">
          <input
            value={typeof value === "string" ? value : ""}
            placeholder={dated ? t("automation-set-date-hint") : undefined}
            onChange={(event) => {
              set(field, event.target.value);
            }}
          />
        </span>
      </label>
    );
  })();
  const defaults: Record<Settable, unknown> = { priority: "high", due_date: "+3d", start_date: "today", estimate: "1h", progress: 100, type: project.types[0]?.name ?? "", title: "{title}", description: "", fix_version: "" };
  return (
    <div className={styles.choice}>
      <Select<Settable>
        label={t("automation-set-field")}
        value={field}
        onChange={(next) => {
          set(next, defaults[next]);
        }}
        options={SETTABLE.map((each) => ({ id: each, label: fieldName(t, each) }))}
      />
      {valueField}
      {CLEARABLE.includes(field) && (
        <label className={styles.check}>
          <input
            type="checkbox"
            className="check"
            checked={cleared}
            onChange={(event) => {
              set(field, event.target.checked ? null : defaults[field]);
            }}
          />
          {t("automation-set-clear")}
        </label>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------
// The dry run

function TryIt({ project, name, body }: { project: Project; name: string; body: { trigger: AutomationRule["trigger"]; condition: string; actions: AutomationAction[] } }) {
  const t = useT();
  const naming = useNaming();
  const [key, setKey] = useState("");
  const test = useCommand("automation.test");
  const result = test.data;
  return (
    <section className={styles.group} aria-labelledby="automation-test">
      <h4 id="automation-test" className={styles.legend}>
        {t("automation-test")}
      </h4>
      <div className={styles.head}>
        <label className={styles.row}>
          <span>{t("automation-test-task")}</span>
          <span className="field">
            <input
              value={key}
              placeholder={`${project.key}-1`}
              onChange={(event) => {
                setKey(event.target.value);
              }}
            />
          </span>
        </label>
        <Button
          className="btn"
          isDisabled={key.trim() === "" || body.actions.length === 0 || test.isPending}
          onPress={() => {
            test.mutate({ task: key.trim(), draft: { ...(name.trim() === "" ? {} : { name: name.trim() }), trigger: body.trigger, ...(body.condition === "" ? {} : { condition: body.condition }), actions: body.actions } });
          }}
        >
          {t("automation-test-run")}
        </Button>
        <span className="faint">{t("automation-test-note")}</span>
      </div>
      <div role="status" aria-live="polite">
        {result && <TestResult t={t} list={naming.list} result={result} />}
      </div>
    </section>
  );
}

function TestResult({ t, list, result }: { t: Translate; list: (items: readonly string[]) => string; result: AutomationTestResult }) {
  const show = (value: unknown): string => {
    if (value === undefined || value === null || value === "" || (Array.isArray(value) && value.length === 0)) {
      return t("automation-test-empty");
    }
    const one = (item: unknown) => (typeof item === "string" ? item : JSON.stringify(item));
    return Array.isArray(value) ? list(value.map(one)) : one(value);
  };
  const line = (change: AutomationTestChange) => {
    if (change.field === "created") {
      return t("automation-test-created", { task: change.task ?? "", what: t(`activity-entity-${change.entity === "task" || change.entity === "comment" || change.entity === "relation" ? change.entity : "other"}`) });
    }
    return t("automation-test-change", { task: change.task ?? "", field: change.entity === "task" ? fieldLabel(t, change.field) : change.field, before: show(change.before), after: show(change.after) });
  };
  if (!result.matches && result.error === undefined) {
    return <p>{t("automation-test-no-match")}</p>;
  }
  const changes = result.changes ?? [];
  return (
    <div className={styles.result}>
      {result.error !== undefined && (
        <div className={styles.error}>
          <p>{result.failed_action === undefined ? t("automation-test-failed-rule") : t("automation-test-failed", { n: result.failed_action + 1 })}</p>
          <p>{failureText(t, result.error)}</p>
        </div>
      )}
      {result.ok && <p>{changes.length === 0 && (result.notices ?? []).length === 0 ? t("automation-test-nothing") : t("automation-test-ok")}</p>}
      {(changes.length > 0 || (result.notices ?? []).length > 0) && (
        <ul className={styles.changes}>
          {changes.map((change, index) => (
            <li key={index}>{line(change)}</li>
          ))}
          {(result.notices ?? []).map((text, index) => (
            <li key={`n${String(index)}`}>{t("automation-test-notice", { text })}</li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** A task field's name in a dry run's line; fields rules don't name show as history names them. */
function fieldLabel(t: Translate, field: string): string {
  return (WATCHABLE as readonly string[]).includes(field) ? fieldName(t, field) : field;
}
