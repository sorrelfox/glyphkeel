// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useState, type ReactNode } from "react";
import { Tab, TabList, TabPanel, Tabs } from "react-aria-components";

import { ACCENTS, DENSITIES, THEMES } from "../appearance/appearance";
import { useAppearance } from "../appearance/AppearanceProvider";
import { classes } from "../classes";
import { Dialog } from "../components/Dialog";
import { Segmented } from "../components/Segmented";
import { Switch } from "../components/Switch";
import { useCommand, useOp } from "../data/hooks";
import { type SettingsPatch, type WeekStart } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { useAppSettings, useUpdateAppSettings } from "../ipc/appSettings";
import styles from "./Preferences.module.css";

const WEEK_STARTS: readonly WeekStart[] = ["monday", "saturday", "sunday"];

/** The pages, in the order of the list on the left. */
export const PREFERENCE_PAGES = ["appearance", "time", "reminders", "workspace"] as const;
export type PreferencePage = (typeof PREFERENCE_PAGES)[number];

interface PreferencesProps {
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  /** The page it opens on; Appearance by default. */
  page?: PreferencePage;
}

/**
 * Preferences: a list of pages on the left, the page on the right. Every
 * change applies at once; there is no OK or Apply. The appearance belongs to
 * this computer; time, reminders and trash are the workspace's settings, kept
 * by the engine; background mode and notifications are the app's own.
 */
export function Preferences({ isOpen, onOpenChange, page = "appearance" }: PreferencesProps) {
  const t = useT();
  return (
    <Dialog title={t("preferences-title")} isOpen={isOpen} onOpenChange={onOpenChange}>
      <Tabs orientation="vertical" className={classes(styles.layout)} defaultSelectedKey={page}>
        <TabList className={classes("sidebar", styles.pages)} aria-label={t("preferences-pages")}>
          {PREFERENCE_PAGES.map((id) => (
            <Tab key={id} id={id} className={({ isSelected }) => classes("nav", isSelected && "on")}>
              {t(`preferences-page-${id}`)}
            </Tab>
          ))}
        </TabList>
        <TabPanel id="appearance" className={classes(styles.page)}>
          <AppearancePage />
        </TabPanel>
        <TabPanel id="time" className={classes(styles.page)}>
          <TimePage />
        </TabPanel>
        <TabPanel id="reminders" className={classes(styles.page)}>
          <RemindersPage />
        </TabPanel>
        <TabPanel id="workspace" className={classes(styles.page)}>
          <WorkspacePage />
        </TabPanel>
      </Tabs>
    </Dialog>
  );
}

/** A titled group of rows in a box. */
function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className={styles.group} aria-label={title}>
      <h2 className={styles.groupTitle}>{title}</h2>
      <div className={styles.boxed}>{children}</div>
    </section>
  );
}

/** One setting: its name, an optional line about it, and its control on the right. */
function Row({ label, about, children }: { label: string; about?: string | undefined; children: ReactNode }) {
  return (
    <div className={styles.row}>
      <span className={styles.text}>
        <span>{label}</span>
        {about !== undefined && <small>{about}</small>}
      </span>
      <span className={styles.control}>{children}</span>
    </div>
  );
}

// ---------- Appearance ----------

/** Theme, accent, density and motion: they apply at once and are kept for the next start. */
function AppearancePage() {
  const t = useT();
  const { appearance, update } = useAppearance();
  return (
    <>
      <Group title={t("preferences-theme")}>
        <Row label={t("preferences-theme")}>
          <Segmented
            label={t("preferences-theme")}
            segments={(["system", ...THEMES] as const).map((theme) => ({ id: theme, label: t(`theme-${theme}`) }))}
            value={appearance.theme}
            onChange={(theme) => {
              update({ theme });
            }}
          />
        </Row>
        <Row label={t("preferences-accent")} about={t("preferences-accent-about")}>
          <span className={classes("field", styles.select)}>
            <select
              aria-label={t("preferences-accent")}
              value={appearance.accent}
              onChange={(event) => {
                const accent = (["system", ...ACCENTS] as const).find((candidate) => candidate === event.target.value);
                if (accent) {
                  update({ accent });
                }
              }}
            >
              {(["system", ...ACCENTS] as const).map((accent) => (
                <option key={accent} value={accent}>
                  {t(`accent-${accent}`)}
                </option>
              ))}
            </select>
          </span>
        </Row>
      </Group>
      <Group title={t("preferences-interface")}>
        <Row label={t("preferences-density")} about={t("preferences-density-about")}>
          <Segmented
            label={t("preferences-density")}
            segments={DENSITIES.map((density) => ({ id: density, label: t(`density-${density}`) }))}
            value={appearance.density}
            onChange={(density) => {
              update({ density });
            }}
          />
        </Row>
        <Row label={t("preferences-reduce-motion")} about={t("preferences-reduce-motion-about")}>
          <Switch
            aria-label={t("preferences-reduce-motion")}
            isSelected={appearance.reduceMotion}
            onChange={(reduceMotion) => {
              update({ reduceMotion });
            }}
          />
        </Row>
      </Group>
    </>
  );
}

// ---------- the workspace's settings ----------

/** The workspace's settings and a way to change them, with Undo. */
function useSettings() {
  const t = useT();
  const settings = useOp("settings.get", {});
  const update = useCommand("settings.update", { undoable: t("toast-settings-changed") });
  return {
    settings,
    change: (patch: SettingsPatch) => {
      update.mutate(patch);
    },
  };
}

/**
 * A whole number from `min` to `max`, saved when the field loses the focus
 * or on Enter. Anything else goes back to the stored value.
 */
function NumberSetting({ label, value, min, max, unit, onSave }: { label: string; value: number; min: number; max: number; unit: string; onSave: (value: number) => void }) {
  const [text, setText] = useState(String(value));
  const [seen, setSeen] = useState(value);
  if (value !== seen) {
    // The stored value changed, here or elsewhere: show it.
    setSeen(value);
    setText(String(value));
  }
  const save = () => {
    const number = Number(text);
    if (text.trim() !== "" && Number.isInteger(number) && number >= min && number <= max) {
      if (number !== value) {
        onSave(number);
      }
    } else {
      setText(String(value));
    }
  };
  return (
    <>
      <span className={classes("field", styles.number)}>
        <input
          type="number"
          className="num"
          aria-label={label}
          min={min}
          max={max}
          step={1}
          value={text}
          onChange={(event) => {
            setText(event.target.value);
          }}
          onBlur={save}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              save();
            }
          }}
        />
      </span>
      <span className={classes("muted", styles.unit)}>{unit}</span>
    </>
  );
}

/** A time of day, `HH:MM`, saved when the field loses the focus or on Enter. */
function TimeSetting({ label, value, onSave }: { label: string; value: string; onSave: (value: string) => void }) {
  const [text, setText] = useState(value);
  const [seen, setSeen] = useState(value);
  if (value !== seen) {
    setSeen(value);
    setText(value);
  }
  const save = () => {
    if (/^([01]\d|2[0-3]):[0-5]\d$/u.test(text)) {
      if (text !== value) {
        onSave(text);
      }
    } else {
      setText(value);
    }
  };
  return (
    <span className={classes("field", styles.time)}>
      <input
        type="time"
        className="num"
        aria-label={label}
        value={text}
        onChange={(event) => {
          setText(event.target.value);
        }}
        onBlur={save}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            save();
          }
        }}
      />
    </span>
  );
}

function Failure({ message }: { message: string }) {
  return <p className={styles.error}>{message}</p>;
}

function TimePage() {
  const t = useT();
  const { settings, change } = useSettings();
  const weekday: Record<WeekStart, string> = {
    monday: t("weekday-monday"),
    saturday: t("weekday-saturday"),
    sunday: t("weekday-sunday"),
  };
  if (settings.isError) {
    return <Failure message={settings.error.error.message} />;
  }
  if (!settings.data) {
    return null;
  }
  const data = settings.data;
  return (
    <>
      <Group title={t("preferences-calendar")}>
        <Row label={t("preferences-week-start")} about={t("preferences-week-start-about")}>
          <Segmented
            label={t("preferences-week-start")}
            segments={WEEK_STARTS.map((day) => ({ id: day, label: weekday[day] }))}
            value={data.week_start}
            onChange={(week_start) => {
              change({ week_start });
            }}
          />
        </Row>
      </Group>
      <Group title={t("preferences-working-time")}>
        <Row label={t("preferences-hours-per-day")} about={t("preferences-hours-per-day-about")}>
          <NumberSetting
            label={t("preferences-hours-per-day")}
            value={data.hours_per_day}
            min={1}
            max={24}
            unit={t("preferences-unit-hours", { count: data.hours_per_day })}
            onSave={(hours_per_day) => {
              change({ hours_per_day });
            }}
          />
        </Row>
        <Row label={t("preferences-days-per-week")} about={t("preferences-days-per-week-about")}>
          <NumberSetting
            label={t("preferences-days-per-week")}
            value={data.days_per_week}
            min={1}
            max={7}
            unit={t("preferences-unit-days", { count: data.days_per_week })}
            onSave={(days_per_week) => {
              change({ days_per_week });
            }}
          />
        </Row>
      </Group>
      <Group title={t("preferences-timer")}>
        <Row label={t("preferences-timer-reminder")} about={t("preferences-timer-reminder-about")}>
          <NumberSetting
            label={t("preferences-timer-reminder")}
            value={data.timer_reminder_hours}
            min={0}
            max={24}
            unit={t("preferences-unit-hours", { count: data.timer_reminder_hours })}
            onSave={(timer_reminder_hours) => {
              change({ timer_reminder_hours });
            }}
          />
        </Row>
      </Group>
    </>
  );
}

function RemindersPage() {
  const t = useT();
  const { settings, change } = useSettings();
  const app = useAppSettings();
  const updateApp = useUpdateAppSettings();
  return (
    <>
      <Group title={t("preferences-reminders")}>
        {settings.isError && <Failure message={settings.error.error.message} />}
        {settings.data && (
          <Row label={t("preferences-reminder-time")} about={t("preferences-reminder-time-about")}>
            <TimeSetting
              label={t("preferences-reminder-time")}
              value={settings.data.reminder_time}
              onSave={(reminder_time) => {
                change({ reminder_time });
              }}
            />
          </Row>
        )}
        <Row label={t("preferences-notifications")} about={t("preferences-notifications-about")}>
          <Switch
            aria-label={t("preferences-notifications")}
            isDisabled={!app.data}
            isSelected={app.data?.notifications ?? true}
            onChange={(notifications) => {
              updateApp({ notifications });
            }}
          />
        </Row>
      </Group>
      <Group title={t("preferences-background")}>
        <Row label={t("preferences-run-in-background")} about={t("preferences-run-in-background-about")}>
          <Switch
            aria-label={t("preferences-run-in-background")}
            isDisabled={!app.data}
            isSelected={app.data?.background ?? true}
            onChange={(background) => {
              updateApp({ background });
            }}
          />
        </Row>
      </Group>
    </>
  );
}

function WorkspacePage() {
  const t = useT();
  const workspace = useOp("workspace.get", {});
  const app = useAppSettings();
  const { settings, change } = useSettings();
  return (
    <>
      <Group title={t("preferences-workspace")}>
        {workspace.isError && <Failure message={workspace.error.error.message} />}
        {workspace.data && (
          <>
            <Row label={t("preferences-workspace-name")} about={t("preferences-workspace-name-about")}>
              <span className="selectable">{workspace.data.name}</span>
            </Row>
            <Row label={t("preferences-workspace-location")}>
              {app.data?.workspace_path === undefined ? (
                <span className="faint">{t("preferences-workspace-location-unknown")}</span>
              ) : (
                <span className={classes("selectable", styles.path)} data-testid="workspace-path">
                  {app.data.workspace_path}
                </span>
              )}
            </Row>
            <Row label={t("preferences-program-version")}>
              <span className="num selectable">{workspace.data.program_version}</span>
            </Row>
            <Row label={t("preferences-schema-version")}>
              <span className="num selectable">{workspace.data.schema_version}</span>
            </Row>
          </>
        )}
      </Group>
      {settings.data && (
        <Group title={t("preferences-trash")}>
          <Row label={t("preferences-trash-days")}>
            <NumberSetting
              label={t("preferences-trash-days")}
              value={settings.data.trash_days}
              min={1}
              max={3650}
              unit={t("preferences-unit-days", { count: settings.data.trash_days })}
              onSave={(trash_days) => {
                change({ trash_days });
              }}
            />
          </Row>
        </Group>
      )}
    </>
  );
}
