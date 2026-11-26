// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// What a template holds, as project settings show it under the template:
// for a task template its title, type, priority, labels, dates, custom
// field values, version and subtasks; for a project template its statuses
// (with their WIP limits and rules), types, labels, custom fields, rules
// for agents, pages, tasks and task templates. Read only: a template
// changes by saving it again from a task or a project.

import { type ReactNode } from "react";
import { useLocale } from "react-aria-components";

import { TypeGlyph } from "../../components/Glyphs";
import { useFieldText } from "../../data/fields";
import { useOp } from "../../data/hooks";
import { isEmpty } from "../../data/rules";
import { useDurations } from "../../data/time";
import {
  type CustomField,
  type CustomFieldKind,
  type ProjectBlueprint,
  type StatusRules,
  type TaskBlueprint,
  type TemplateSummary,
} from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Project.module.css";

/** How many rules a status has: each field named and each switch on counts as one. */
export function ruleCount(rules: StatusRules | undefined): number {
  if (!rules || isEmpty(rules)) {
    return 0;
  }
  const lists = [
    rules.set,
    rules.clear,
    rules.required,
    rules.read_only,
  ].reduce((sum, list) => sum + (list ?? []).length, 0);
  const switches = [
    rules.stop_timer,
    rules.release_claim,
    rules.agents_may_not_enter,
  ].filter((on) => on === true).length;
  return lists + switches;
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="muted">{label}</dt>
      <dd>{children}</dd>
    </>
  );
}

/** The contents of template `template`, read when shown. */
export function TemplateContents({
  template,
  fields,
}: {
  template: TemplateSummary;
  fields: readonly CustomField[];
}) {
  const t = useT();
  const full = useOp("template.get", { template: template.id });
  const name = template.name;
  if (!full.data) {
    return (
      <p className="muted" role="status">
        {t("template-contents-loading")}
      </p>
    );
  }
  return (
    <dl
      className={styles.templateContents}
      aria-label={t("template-contents", { name })}
      data-template-contents={template.name}
    >
      {full.data.task !== undefined && (
        <TaskContents task={full.data.task as TaskBlueprint} fields={fields} />
      )}
      {full.data.setup !== undefined && (
        <SetupContents setup={full.data.setup as ProjectBlueprint} />
      )}
    </dl>
  );
}

function TaskContents({
  task,
  fields,
}: {
  task: TaskBlueprint;
  fields: readonly CustomField[];
}) {
  const t = useT();
  const { locale } = useLocale();
  const fieldText = useFieldText();
  const { format: duration } = useDurations();
  const list = new Intl.ListFormat(locale, {
    style: "long",
    type: "conjunction",
  });
  const kindOf = (fieldName: string): CustomFieldKind =>
    fields.find(
      (field) =>
        field.name.toLocaleLowerCase(locale) ===
        fieldName.toLocaleLowerCase(locale),
    )?.kind ?? "text";
  const values = Object.entries(task.fields ?? {});
  return (
    <>
      <Row label={t("template-task-title")}>{task.title}</Row>
      {task.type !== undefined && (
        <Row label={t("template-task-type")}>{task.type}</Row>
      )}
      {task.priority !== undefined && task.priority !== "none" && (
        <Row label={t("template-task-priority")}>
          {t(`priority-${task.priority}`)}
        </Row>
      )}
      {(task.labels ?? []).length > 0 && (
        <Row label={t("template-task-labels")}>
          {list.format(task.labels ?? [])}
        </Row>
      )}
      {task.estimate_minutes !== undefined && (
        <Row label={t("template-task-estimate")}>
          {duration(task.estimate_minutes)}
        </Row>
      )}
      {task.start_in_days !== undefined && (
        <Row label={t("template-task-start")}>
          {t("template-task-in-days", { days: task.start_in_days })}
        </Row>
      )}
      {task.due_in_days !== undefined && (
        <Row label={t("template-task-due")}>
          {t("template-task-in-days", { days: task.due_in_days })}
        </Row>
      )}
      {task.fix_version !== undefined && (
        <Row label={t("template-task-version")}>
          <span data-template-version>{task.fix_version}</span>
        </Row>
      )}
      {values.length > 0 && (
        <Row label={t("template-task-fields")}>
          <ul className={styles.templateValues}>
            {values.map(([fieldName, value]) => (
              <li key={fieldName} data-template-field={fieldName}>
                <span className="muted">{fieldName}</span>{" "}
                <span>{fieldText(kindOf(fieldName), value)}</span>
              </li>
            ))}
          </ul>
        </Row>
      )}
      {(task.subtasks ?? []).length > 0 && (
        <Row label={t("template-task-subtasks")}>
          {list.format((task.subtasks ?? []).map((sub) => sub.title))}
        </Row>
      )}
    </>
  );
}

function SetupContents({ setup }: { setup: ProjectBlueprint }) {
  const t = useT();
  const { locale } = useLocale();
  const list = new Intl.ListFormat(locale, {
    style: "long",
    type: "conjunction",
  });
  const none = t("template-nothing");
  const agents = setup.agent_rules?.read_only ?? [];
  return (
    <>
      {(setup.statuses ?? []).length > 0 && (
        <Row label={t("template-setup-statuses")}>
          <ul className={styles.templateValues}>
            {(setup.statuses ?? []).map((status) => {
              const rules = ruleCount(status.rules);
              return (
                <li key={status.name} data-template-status={status.name}>
                  <span
                    className={`dot hue-${status.hue}`}
                    aria-hidden="true"
                  />{" "}
                  {status.name}
                  {status.wip_limit !== undefined && (
                    <span className="meta">
                      {" "}
                      {t("template-setup-wip", { limit: status.wip_limit })}
                    </span>
                  )}
                  {rules > 0 && (
                    <span className="meta" data-template-rules>
                      {" "}
                      {t("rules-count", { count: rules })}
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        </Row>
      )}
      {(setup.types ?? []).length > 0 && (
        <Row label={t("template-setup-types")}>
          <ul className={styles.templateValues}>
            {(setup.types ?? []).map((type) => (
              <li key={type.name}>
                <TypeGlyph glyph={type.glyph} /> {type.name}
              </li>
            ))}
          </ul>
        </Row>
      )}
      <Row label={t("template-setup-labels")}>
        {(setup.labels ?? []).length > 0
          ? list.format((setup.labels ?? []).map((label) => label.name))
          : none}
      </Row>
      <Row label={t("template-setup-fields")}>
        {(setup.fields ?? []).length > 0 ? (
          <ul className={styles.templateValues}>
            {(setup.fields ?? []).map((field) => (
              <li key={field.id} data-template-field={field.name}>
                {field.required === true
                  ? t("template-setup-field-required", { name: field.name })
                  : field.name}{" "}
                <span className="meta">{t(`field-kind-${field.kind}`)}</span>
              </li>
            ))}
          </ul>
        ) : (
          none
        )}
      </Row>
      {agents.length > 0 && (
        <Row label={t("template-setup-agent-rules")}>
          {t("template-setup-agent-read-only", { count: agents.length })}
        </Row>
      )}
      {(setup.wiki ?? []).length > 0 && (
        <Row label={t("template-setup-pages")}>
          {list.format((setup.wiki ?? []).map((page) => page.title))}
        </Row>
      )}
      {(setup.tasks ?? []).length > 0 && (
        <Row label={t("template-setup-tasks")}>
          {list.format((setup.tasks ?? []).map((task) => task.title))}
        </Row>
      )}
      {(setup.task_templates ?? []).length > 0 && (
        <Row label={t("template-setup-task-templates")}>
          {list.format((setup.task_templates ?? []).map((named) => named.name))}
        </Row>
      )}
    </>
  );
}
