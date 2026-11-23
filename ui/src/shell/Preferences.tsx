// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type ReactNode } from "react";

import { Dialog } from "../components/Dialog";
import { Segmented } from "../components/Segmented";
import { useCommand, useOp } from "../data/hooks";
import { type WeekStart } from "../generated/ops";
import { useT } from "../i18n/I18n";
import styles from "./Preferences.module.css";

const WEEK_STARTS: readonly WeekStart[] = ["monday", "saturday", "sunday"];

interface PreferencesProps {
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
}

/**
 * Preferences, a first version: the open workspace and its settings, read
 * from the engine; the first day of the week can be changed. The full
 * dialog of the design (appearance, keys, agents…) comes later.
 */
export function Preferences({ isOpen, onOpenChange }: PreferencesProps) {
  const t = useT();
  return (
    <Dialog title={t("preferences-title")} isOpen={isOpen} onOpenChange={onOpenChange}>
      <div className={styles.body}>
        <WorkspaceSection />
        <SettingsSection />
      </div>
    </Dialog>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.row}>
      <dt className={styles.label}>{label}</dt>
      <dd className={styles.value}>{children}</dd>
    </div>
  );
}

function WorkspaceSection() {
  const t = useT();
  const workspace = useOp("workspace.get", {});
  return (
    <section className={styles.section} aria-labelledby="preferences-workspace">
      <h2 id="preferences-workspace" className={styles.heading}>
        {t("preferences-workspace")}
      </h2>
      {workspace.isError && <p className={styles.error}>{workspace.error.error.message}</p>}
      {workspace.data && (
        <dl className={styles.rows}>
          <Row label={t("preferences-workspace-name")}>{workspace.data.name}</Row>
          <Row label={t("preferences-program-version")}>{workspace.data.program_version}</Row>
          <Row label={t("preferences-schema-version")}>{workspace.data.schema_version}</Row>
        </dl>
      )}
    </section>
  );
}

function SettingsSection() {
  const t = useT();
  const settings = useOp("settings.get", {});
  const update = useCommand("settings.update", { undoable: t("toast-settings-changed") });
  const weekday: Record<WeekStart, string> = {
    monday: t("weekday-monday"),
    saturday: t("weekday-saturday"),
    sunday: t("weekday-sunday"),
  };
  return (
    <section className={styles.section} aria-labelledby="preferences-time">
      <h2 id="preferences-time" className={styles.heading}>
        {t("preferences-time")}
      </h2>
      {settings.isError && <p className={styles.error}>{settings.error.error.message}</p>}
      {settings.data && (
        <dl className={styles.rows}>
          <Row label={t("preferences-week-start")}>
            <Segmented
              label={t("preferences-week-start")}
              segments={WEEK_STARTS.map((day) => ({ id: day, label: weekday[day] }))}
              value={settings.data.week_start}
              onChange={(week_start) => {
                update.mutate({ week_start });
              }}
            />
          </Row>
          <Row label={t("preferences-hours-per-day")}>
            <span className="num">{settings.data.hours_per_day}</span>
          </Row>
          <Row label={t("preferences-days-per-week")}>
            <span className="num">{settings.data.days_per_week}</span>
          </Row>
          <Row label={t("preferences-reminder-time")}>
            <span className="num">{settings.data.reminder_time}</span>
          </Row>
          <Row label={t("preferences-trash-days")}>
            <span className="num">{t("preferences-days", { count: settings.data.trash_days })}</span>
          </Row>
        </dl>
      )}
    </section>
  );
}
