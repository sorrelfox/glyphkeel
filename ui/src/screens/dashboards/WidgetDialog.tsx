// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useState } from "react";
import { Button } from "react-aria-components";

import { Dialog } from "../../components/Dialog";
import { Segmented } from "../../components/Segmented";
import { Select } from "../../components/Select";
import { BUCKETS, COUNT_FIELDS, defaultWidth, KIND_SETTINGS, NEEDS_PROJECT, TIME_GROUPS, WIDGET_KINDS } from "../../data/dashboards";
import { useOp } from "../../data/hooks";
import { type ActorKind, type Dashboard, type FlowBucket, type TimeGroupBy, type Widget, type WidgetKind, type WidgetSetting, type WidgetSettings } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { widgetTitle } from "./WidgetBody";
import styles from "./Dashboards.module.css";

/** The list's key for "no project" and "anyone"; no key or kind has this form. */
const NONE = "*";

/** What the dialog saves: a new widget's whole spec, or a widget's changes. */
export interface WidgetDraft {
  kind: WidgetKind;
  title: string;
  width: number;
  settings: WidgetSettings;
  /** The settings emptied, for a widget that had them. */
  clear: WidgetSetting[];
}

interface WidgetDialogProps {
  dashboard: Dashboard;
  /** The widget to change; none: a new one. */
  widget?: Widget;
  error: string | null;
  onSave: (draft: WidgetDraft) => void;
  onEdit: () => void;
  onClose: () => void;
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className={styles.formRow}>
      <span className={styles.formLabel} aria-hidden="true">
        {label}
      </span>
      {children}
    </div>
  );
}

/** Adds a widget, or changes one: what it shows, its title, width and settings. */
export function WidgetDialog({ dashboard, widget, error, onSave, onEdit, onClose }: WidgetDialogProps) {
  const t = useT();
  const projects = useOp("project.list", {});
  const fields = useOp("field.list", {});
  const [kind, setKind] = useState<WidgetKind>(widget?.kind ?? "query_count");
  const [title, setTitle] = useState(widget?.title ?? "");
  const [width, setWidth] = useState(widget?.width ?? defaultWidth(kind));
  const [settings, setSettings] = useState<WidgetSettings>(widget?.settings ?? {});
  const set = <K extends keyof WidgetSettings>(key: K, value: WidgetSettings[K] | undefined) => {
    onEdit();
    setSettings((old) => {
      const rest = Object.fromEntries(Object.entries(old).filter(([name]) => name !== key));
      return value === undefined || value === "" ? rest : { ...rest, [key]: value };
    });
  };
  // A burndown or versions on a dashboard of the whole workspace needs a project: the first unless chosen.
  const needsOne = NEEDS_PROJECT.includes(kind) && dashboard.project === undefined;
  const firstProject = projects.data?.items[0]?.key;
  const projectChoice = settings.project ?? (needsOne ? (firstProject ?? NONE) : NONE);
  const takes = (setting: WidgetSetting) => KIND_SETTINGS[kind].includes(setting);
  const save = () => {
    const kept: WidgetSettings = {};
    for (const setting of KIND_SETTINGS[kind]) {
      const value = settings[setting];
      if (value !== undefined && value !== "") {
        (kept as Record<string, unknown>)[setting] = value;
      }
    }
    if (kind === "tasks_by_field" && kept.field === undefined) {
      kept.field = "status";
    }
    if (needsOne && kept.project === undefined && firstProject !== undefined) {
      kept.project = firstProject;
    }
    const clear = widget === undefined ? [] : KIND_SETTINGS[kind].filter((setting) => widget.settings[setting] !== undefined && kept[setting] === undefined);
    onSave({ kind, title: title.trim(), width, settings: kept, clear });
  };
  const countable = (fields.data?.fields ?? []).filter((field) => field.kind === "select" || field.kind === "multi_select" || field.kind === "boolean");
  const shownTitle = widget === undefined ? "" : widgetTitle(t, widget);
  return (
    <Dialog
      title={widget === undefined ? t("widget-new-title") : t("widget-edit-title", { title: shownTitle })}
      isOpen
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      footer={
        <>
          <Button className="btn" onPress={onClose}>
            {t("dialog-cancel")}
          </Button>
          <Button className="btn primary" onPress={save} data-widget-save>
            {widget === undefined ? t("widget-add") : t("widget-save")}
          </Button>
        </>
      }
    >
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          save();
        }}
      >
        {widget === undefined && (
          <Row label={t("widget-field-kind")}>
            <Select
              label={t("widget-field-kind")}
              value={kind}
              onChange={(next) => {
                onEdit();
                setKind(next);
                setWidth(defaultWidth(next));
              }}
              options={WIDGET_KINDS.map((each) => ({ id: each, label: t(`widget-kind-${each}`) }))}
            />
          </Row>
        )}
        <p className={styles.help}>{t(`widget-help-${kind}`)}</p>
        <label className={styles.formRow}>
          <span className={styles.formLabel}>{t("widget-field-title")}</span>
          <span className="field">
            <input
              value={title}
              maxLength={100}
              placeholder={widgetTitle(t, { id: "", kind, width, settings })}
              onChange={(event) => {
                onEdit();
                setTitle(event.target.value);
              }}
            />
          </span>
        </label>
        <Row label={t("widget-field-width")}>
          <Segmented
            label={t("widget-field-width")}
            value={String(width) as "1" | "2" | "3"}
            onChange={(next) => {
              setWidth(Number(next));
            }}
            segments={(["1", "2", "3"] as const).map((each) => ({ id: each, label: t("widget-width-value", { width: Number(each) }) }))}
          />
        </Row>
        {takes("field") && (
          <Row label={t("widget-field-field")}>
            <Select
              label={t("widget-field-field")}
              value={settings.field ?? "status"}
              onChange={(next) => {
                set("field", next);
              }}
              options={[...COUNT_FIELDS.map((each) => ({ id: each, label: t(`widget-count-by-${each}`) })), ...countable.map((field) => ({ id: field.query_name, label: field.name }))]}
            />
          </Row>
        )}
        {takes("query") && (
          <label className={styles.formRow}>
            <span className={styles.formLabel}>{t("widget-field-query")}</span>
            <span className="field">
              <input
                value={settings.query ?? ""}
                spellCheck={false}
                placeholder={t("widget-query-placeholder")}
                onChange={(event) => {
                  set("query", event.target.value);
                }}
                data-widget-query
              />
            </span>
          </label>
        )}
        {takes("project") && (
          <Row label={t("widget-field-project")}>
            <Select
              label={t("widget-field-project")}
              value={projectChoice}
              onChange={(next) => {
                set("project", next === NONE ? undefined : next);
              }}
              options={[
                ...(needsOne ? [] : [{ id: NONE, label: dashboard.project === undefined ? t("widget-any-project") : t("widget-dashboard-project") }]),
                ...(projects.data?.items ?? []).map((project) => ({ id: project.key, label: project.path })),
              ]}
            />
          </Row>
        )}
        {takes("bucket") && (
          <Row label={t("flow-bucket")}>
            <Segmented<FlowBucket>
              label={t("flow-bucket")}
              value={settings.bucket ?? "week"}
              onChange={(next) => {
                set("bucket", next);
              }}
              segments={BUCKETS.map((each) => ({ id: each, label: t(`flow-bucket-${each}`) }))}
            />
          </Row>
        )}
        {takes("group_by") && (
          <Row label={t("report-group-by")}>
            <Select<TimeGroupBy>
              label={t("report-group-by")}
              value={settings.group_by ?? "project"}
              onChange={(next) => {
                set("group_by", next);
              }}
              options={TIME_GROUPS.map((each) => ({ id: each, label: t(`report-by-${each}`) }))}
            />
          </Row>
        )}
        {takes("actor_kind") && (
          <Row label={t("activity-filter-actor")}>
            <Segmented<ActorKind | typeof NONE>
              label={t("activity-filter-actor")}
              value={settings.actor_kind ?? NONE}
              onChange={(next) => {
                set("actor_kind", next === NONE ? undefined : next);
              }}
              segments={[
                { id: NONE, label: t("activity-actor-all") },
                { id: "user", label: t("activity-actor-user") },
                { id: "agent", label: t("activity-actor-agent") },
                { id: "automation", label: t("activity-actor-automation") },
              ]}
            />
          </Row>
        )}
        {takes("days") && (
          <label className={styles.formRow}>
            <span className={styles.formLabel}>{t("widget-field-days")}</span>
            <span className="field">
              <input
                type="number"
                min={1}
                max={366}
                className="num"
                value={settings.days ?? ""}
                placeholder={t("widget-days-placeholder")}
                onChange={(event) => {
                  set("days", event.target.value === "" ? undefined : Number(event.target.value));
                }}
              />
            </span>
          </label>
        )}
        {takes("limit") && (
          <label className={styles.formRow}>
            <span className={styles.formLabel}>{t("widget-field-limit")}</span>
            <span className="field">
              <input
                type="number"
                min={1}
                max={50}
                className="num"
                value={settings.limit ?? ""}
                placeholder={t("widget-limit-placeholder")}
                onChange={(event) => {
                  set("limit", event.target.value === "" ? undefined : Number(event.target.value));
                }}
              />
            </span>
          </label>
        )}
        {takes("text") && (
          <label className={styles.formRow}>
            <span className={styles.formLabel}>{t("widget-field-text")}</span>
            <span className="field">
              <textarea
                rows={6}
                maxLength={10000}
                value={settings.text ?? ""}
                onChange={(event) => {
                  set("text", event.target.value);
                }}
                data-widget-text
              />
            </span>
          </label>
        )}
        {error !== null && (
          <p className={styles.problem} role="alert">
            {error}
          </p>
        )}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
