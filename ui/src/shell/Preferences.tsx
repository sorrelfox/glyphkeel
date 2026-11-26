// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Monitor } from "lucide-react";
import { useState } from "react";
import { RadioButton, RadioField, RadioGroup, Tab, TabList, TabPanel, Tabs } from "react-aria-components";

import { ACCENTS, DENSITIES, THEMES, type Accent, type Theme } from "../appearance/appearance";
import { useAppearance } from "../appearance/AppearanceProvider";
import { classes } from "../classes";
import { Dialog } from "../components/Dialog";
import { Segmented } from "../components/Segmented";
import { Switch } from "../components/Switch";
import { useCommand, useOp } from "../data/hooks";
import { type SettingsPatch, type WeekStart } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { canRestart, restartApp, SYSTEM_ZONE, useAppSettings, useSetAutostart, useTimeZones, useUpdateAppSettings } from "../ipc/appSettings";
import { useQuickAddShortcut, useSetQuickAddShortcut, type QuickAddShortcut, type ShortcutState as ShortcutStateName } from "../ipc/quickAdd";
import { platform, type Platform } from "../platform";
import { parseChord } from "../actions/keymap";
import { chordLabel } from "../components/Kbd";
import { Select } from "../components/Select";
import { DataPage } from "../screens/safety/DataPage";
import { Group, NumberSetting, Row } from "./PreferenceRows";
import { AboutPage } from "./preferences/AboutPage";
import { AgentsPage } from "./preferences/AgentsPage";
import { LanguagePage } from "./preferences/LanguagePage";
import { ShortcutsPage } from "./preferences/ShortcutsPage";
import { WorkKindsGroup } from "./preferences/WorkKinds";
import styles from "./Preferences.module.css";
import { TimeZonePicker } from "./TimeZonePicker";

const WEEK_STARTS: readonly WeekStart[] = ["monday", "saturday", "sunday"];

/** The pages, in the order of the list on the left. */
export const PREFERENCE_PAGES = ["appearance", "language", "time", "notifications", "quick-add", "shortcuts", "agents", "workspace", "data", "about"] as const;
export type PreferencePage = (typeof PREFERENCE_PAGES)[number];

interface PreferencesProps {
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  /** The page it opens on; Appearance by default. */
  page?: PreferencePage;
}

/**
 * Preferences: a list of pages on the left, the page on the right. Every
 * change applies at once; there is no OK or Apply. The appearance and the
 * language are this person's; time, reminders, trash and what agents may
 * do are the workspace's settings, kept by the engine; background mode,
 * starting with the system, notifications, safe rendering and the time
 * zone are the app's own.
 */
export function Preferences({ isOpen, onOpenChange, page = "appearance" }: PreferencesProps) {
  const t = useT();
  return (
    <Dialog title={t("preferences-title")} isOpen={isOpen} onOpenChange={onOpenChange}>
      <Tabs orientation="vertical" className={classes(styles.layout)} defaultSelectedKey={page}>
        <TabList className={classes(styles.pages)} aria-label={t("preferences-pages")}>
          {PREFERENCE_PAGES.map((id) => (
            <Tab key={id} id={id} className={({ isSelected }) => classes("nav", isSelected && "on")}>
              {t(`preferences-page-${id}`)}
            </Tab>
          ))}
        </TabList>
        <TabPanel id="appearance" className={classes(styles.page)}>
          <AppearancePage />
        </TabPanel>
        <TabPanel id="language" className={classes(styles.page)}>
          <LanguagePage />
        </TabPanel>
        <TabPanel id="time" className={classes(styles.page)}>
          <TimePage />
        </TabPanel>
        <TabPanel id="notifications" className={classes(styles.page)}>
          <NotificationsPage />
        </TabPanel>
        <TabPanel id="quick-add" className={classes(styles.page)}>
          <QuickAddPage />
        </TabPanel>
        <TabPanel id="shortcuts" className={classes(styles.page)}>
          <ShortcutsPage />
        </TabPanel>
        <TabPanel id="agents" className={classes(styles.page)}>
          <AgentsPage />
        </TabPanel>
        <TabPanel id="workspace" className={classes(styles.page)}>
          <WorkspacePage />
        </TabPanel>
        <TabPanel id="data" className={classes(styles.page)}>
          <DataPage />
        </TabPanel>
        <TabPanel id="about" className={classes(styles.page)}>
          <AboutPage />
        </TabPanel>
      </Tabs>
    </Dialog>
  );
}

// ---------- Appearance ----------

const ACCENT_CHOICES = ["system", ...ACCENTS] as const;
type AccentChoice = (typeof ACCENT_CHOICES)[number];

/**
 * The accent colours as swatches, after a "from the desktop" button. Each
 * swatch is drawn in its own colour and shows its name, as a tooltip, while
 * the pointer rests on it or it has the keyboard focus; the name is also its
 * accessible name. Arrow keys move.
 */
function AccentSwatches({ label, value, onChange }: { label: string; value: AccentChoice; onChange: (accent: AccentChoice) => void }) {
  const t = useT();
  return (
    <RadioGroup
      aria-label={label}
      orientation="horizontal"
      className={classes(styles.swatches)}
      value={value}
      onChange={(next) => {
        const accent = ACCENT_CHOICES.find((candidate) => candidate === next);
        if (accent) {
          onChange(accent);
        }
      }}
    >
      {ACCENT_CHOICES.map((accent) => (
        <RadioField
          key={accent}
          value={accent}
          className={classes(styles.swatchField)}
          {...(accent === "system" ? {} : { "aria-label": t(`accent-${accent}`) })}
        >
          {accent === "system" ? (
            <RadioButton className={({ isSelected, isFocusVisible }) => classes("btn sm", isSelected && "on", isFocusVisible && styles.tileFocus)}>
              <Monitor className="ic s" aria-hidden="true" />
              <span>{t("accent-system")}</span>
            </RadioButton>
          ) : (
            <RadioButton
              data-accent={accent}
              className={({ isSelected, isFocusVisible }) => classes(styles.swatch, isSelected && styles.swatchOn, isFocusVisible && styles.swatchFocus)}
            >
              <span className={styles.swatchName} aria-hidden="true">
                {t(`accent-${accent}`)}
              </span>
            </RadioButton>
          )}
        </RadioField>
      ))}
    </RadioGroup>
  );
}


const THEME_CHOICES = ["system", ...THEMES] as const;
type ThemeChoice = (typeof THEME_CHOICES)[number];

/** A miniature window drawn with a theme's own tokens: sidebar strip, cards, accent bar. */
function ThemePreview({ theme, accent }: { theme: Theme; accent: Accent }) {
  return (
    <span className={styles.preview} data-theme={theme} data-accent={accent}>
      <span className={styles.previewSide} />
      <span className={styles.previewMain}>
        <span className={styles.previewCard} />
        <span className={styles.previewCard} />
        <span className={styles.previewAccent} />
      </span>
    </span>
  );
}

/**
 * The themes as tiles, each a preview with its name below; System shows the
 * light and the dark theme it follows side by side. The tiles wrap onto more
 * lines when the page is narrow. Arrow keys move between them.
 */
export function ThemeTiles({
  label,
  value,
  accent,
  systemThemes,
  onChange,
}: {
  label: string;
  value: ThemeChoice;
  accent: Accent;
  systemThemes: readonly [Theme, Theme];
  onChange: (theme: ThemeChoice) => void;
}) {
  const t = useT();
  return (
    <RadioGroup
      aria-label={label}
      orientation="horizontal"
      className={classes(styles.tiles)}
      value={value}
      onChange={(next) => {
        const theme = THEME_CHOICES.find((candidate) => candidate === next);
        if (theme) {
          onChange(theme);
        }
      }}
    >
      {THEME_CHOICES.map((theme) => (
        <RadioField key={theme} value={theme} className={classes(styles.tileField)}>
          <RadioButton className={({ isSelected, isFocusVisible }) => classes(styles.tile, isSelected && styles.tileOn, isFocusVisible && styles.tileFocus)}>
            {({ isSelected }) => (
              <>
                {theme === "system" ? (
                  <span className={styles.previewPair}>
                    <ThemePreview theme={systemThemes[0]} accent={accent} />
                    <ThemePreview theme={systemThemes[1]} accent={accent} />
                  </span>
                ) : (
                  <ThemePreview theme={theme} accent={accent} />
                )}
                <span className={styles.tileName}>
                  <span className={classes(styles.mark, isSelected && styles.markOn)} aria-hidden="true" />
                  <span className={styles.tileText}>{t(`theme-${theme}`)}</span>
                </span>
              </>
            )}
          </RadioButton>
        </RadioField>
      ))}
    </RadioGroup>
  );
}

/** Theme, accent, density and motion: they apply at once and are kept for the next start. */
function AppearancePage() {
  const t = useT();
  const { appearance, resolved, update } = useAppearance();
  return (
    <>
      <Group
        title={t("preferences-theme")}
        lead={
          <ThemeTiles
            label={t("preferences-theme")}
            value={appearance.theme}
            accent={resolved.accent}
            systemThemes={[appearance.systemLight, appearance.systemDark]}
            onChange={(theme) => {
              update({ theme });
            }}
          />
        }
      >
        <Row label={t("preferences-accent")} about={t("preferences-accent-about")}>
          <AccentSwatches
            label={t("preferences-accent")}
            value={appearance.accent}
            onChange={(accent) => {
              update({ accent });
            }}
          />
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
      <RenderingGroup />
    </>
  );
}

/** Safe rendering: drawing without the GPU, for drivers that leave the window blank. */
function RenderingGroup() {
  const t = useT();
  const app = useAppSettings();
  const updateApp = useUpdateAppSettings();
  const data = app.data;
  const pending = data !== undefined && data.safe_rendering !== data.safe_rendering_in_use;
  return (
    <Group title={t("preferences-rendering")}>
      <Row label={t("preferences-safe-rendering")} about={t("preferences-safe-rendering-about")}>
        <Switch
          aria-label={t("preferences-safe-rendering")}
          isDisabled={!data}
          isSelected={data?.safe_rendering ?? false}
          onChange={(safe_rendering) => {
            updateApp({ safe_rendering });
          }}
        />
      </Row>
      {pending && (
        <div className={styles.restartNote} role="status" data-testid="safe-rendering-pending">
          <p>{t(data.safe_rendering ? "preferences-safe-rendering-on-next-start" : "preferences-safe-rendering-off-next-start")}</p>
          {canRestart() && (
            <button type="button" className="btn sm" onClick={() => void restartApp()}>
              {t("preferences-restart-now")}
            </button>
          )}
        </div>
      )}
    </Group>
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
      <ZoneGroup />
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
      <WorkKindsGroup />
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

/** The person's time zone: theirs, not the workspace's, so it is kept with the app's settings. */
function ZoneGroup() {
  const t = useT();
  const app = useAppSettings();
  const zones = useTimeZones(true);
  const updateApp = useUpdateAppSettings();
  // The field takes its list once; it waits for it (an error leaves only "System").
  if (!app.data || zones.isPending) {
    return null;
  }
  const chosen = app.data.time_zone;
  return (
    <Group title={t("preferences-zone-group")}>
      <Row label={t("preferences-time-zone")} about={t("preferences-time-zone-about")}>
        <TimeZonePicker
          label={t("preferences-time-zone")}
          value={chosen}
          systemZone={chosen === SYSTEM_ZONE ? app.data.zone_in_use : systemZoneName()}
          zones={zones.data ?? []}
          onChange={(time_zone) => {
            updateApp({ time_zone });
          }}
        />
      </Row>
    </Group>
  );
}

/** The computer's own zone, by name, as the page sees it. */
function systemZoneName(): string | undefined {
  const zone = new Intl.DateTimeFormat().resolvedOptions().timeZone;
  return zone === "" ? undefined : zone;
}

/** Desktop notifications, when reminders come, and whether the app stays for them. */
function NotificationsPage() {
  const t = useT();
  const { settings, change } = useSettings();
  const app = useAppSettings();
  const updateApp = useUpdateAppSettings();
  const setAutostart = useSetAutostart();
  const autostart = autostartMessages(platform());
  return (
    <>
      <Group title={t("preferences-notifications-group")}>
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
        <Row label={t(autostart.label)} about={t(autostart.about)}>
          <Switch
            aria-label={t(autostart.label)}
            isDisabled={!app.data}
            isSelected={app.data?.autostart ?? false}
            onChange={setAutostart}
          />
        </Row>
      </Group>
    </>
  );
}

// ---------- Quick add ----------

/**
 * Starting with the system: on Windows it is signing in, and Task Manager
 * can switch it off too.
 */
export function autostartMessages(on: Platform): { label: string; about: string } {
  return on === "windows"
    ? { label: "preferences-autostart-windows", about: "preferences-autostart-windows-about" }
    : { label: "preferences-autostart", about: "preferences-autostart-about" };
}

/**
 * The message for the shortcut's state, and whether the person needs the
 * command to bind it in the desktop's settings. Windows has no portal and no
 * such settings: the app holds the keys itself, or they are taken.
 */
export function shortcutStateMessage(state: ShortcutStateName, on: Platform): { id: string; needsCommand: boolean } {
  if (on === "windows") {
    switch (state) {
      case "grab":
        return { id: "preferences-shortcut-state-grab", needsCommand: false };
      case "manual":
      case "refused":
        return { id: "preferences-shortcut-state-taken", needsCommand: false };
      default:
        return { id: "preferences-shortcut-first-time-windows", needsCommand: false };
    }
  }
  switch (state) {
    case "off":
      return { id: "preferences-shortcut-first-time", needsCommand: false };
    case "waiting":
      return { id: "preferences-shortcut-state-waiting", needsCommand: false };
    case "portal":
      return { id: "preferences-shortcut-state-portal", needsCommand: false };
    case "grab":
      return { id: "preferences-shortcut-state-grab", needsCommand: false };
    case "manual":
      return { id: "preferences-shortcut-state-manual", needsCommand: true };
    case "refused":
      return { id: "preferences-shortcut-state-refused", needsCommand: true };
  }
}

/** What the shortcut's state means, in a sentence; the command where the person binds it. */
function ShortcutState({ shortcut, onAskAgain }: { shortcut: QuickAddShortcut; onAskAgain: () => void }) {
  const t = useT();
  const keys = chordLabel(parseChord(shortcut.keys), t);
  const message = shortcutStateMessage(shortcut.state, platform());
  const text = t(message.id, { keys, trigger: shortcut.trigger ?? keys });
  const needsCommand = message.needsCommand;
  return (
    <div className={styles.shortcutState} role="status" data-testid="shortcut-state" data-state={shortcut.state}>
      <p>{text}</p>
      {needsCommand && <code className={classes("selectable", styles.command)}>{shortcut.command}</code>}
      {shortcut.detail !== undefined && needsCommand && <small className="faint">{shortcut.detail}</small>}
      {shortcut.state === "refused" && platform() !== "windows" && (
        <button type="button" className="btn sm" onClick={onAskAgain}>
          {t("preferences-shortcut-try-again")}
        </button>
      )}
    </div>
  );
}

function QuickAddPage() {
  const t = useT();
  const shortcut = useQuickAddShortcut();
  const change = useSetQuickAddShortcut();
  const data = shortcut.data ?? null;
  return (
    <Group title={t("preferences-quick-add")}>
      {shortcut.isError && <Failure message={shortcut.error.error.message} />}
      <Row label={t("preferences-shortcut")} about={t(data === null ? "preferences-shortcut-unavailable" : "preferences-shortcut-about")}>
        <Switch
          aria-label={t("preferences-shortcut")}
          isDisabled={data === null}
          isSelected={data?.enabled ?? false}
          onChange={(enabled) => {
            if (data) {
              change({ enabled, keys: data.keys });
            }
          }}
        />
      </Row>
      {data && (
        <>
          <Row label={t("preferences-shortcut-keys")}>
            <Select
              label={t("preferences-shortcut-keys")}
              value={data.keys}
              options={data.choices.map((choice) => ({ id: choice, label: chordLabel(parseChord(choice), t) }))}
              onChange={(keys) => {
                change({ enabled: data.enabled, keys });
              }}
            />
          </Row>
          <ShortcutState
            shortcut={data}
            onAskAgain={() => {
              change({ enabled: true, keys: data.keys });
            }}
          />
        </>
      )}
    </Group>
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
