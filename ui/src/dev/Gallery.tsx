// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ArrowLeft, ChevronDown, Filter, Plus, Search } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover } from "react-aria-components";

import { keysOf, type Action } from "../actions/actions";
import { ActionsProvider } from "../actions/ActionsProvider";
import { parseChord } from "../actions/keymap";
import { ACCENTS, DENSITIES, THEME_ACCENT, THEMES, type Accent, type Theme } from "../appearance/appearance";
import { useAppearance } from "../appearance/AppearanceProvider";
import { Dialog } from "../components/Dialog";
import { Kbd } from "../components/Kbd";
import { Segmented } from "../components/Segmented";
import { Switch } from "../components/Switch";
import { Tooltip } from "../components/Tooltip";
import { classes } from "../classes";
import { useT } from "../i18n/I18n";
import { appWindow } from "../ipc/window";
import styles from "./Gallery.module.css";
import { BANNER, CARDS, HUES, LANE, MARKDOWN, MENU, PROJECTS, PROPERTIES, STATUSES, SWATCHES, TABLE, TABS, VIEWS } from "./samples";

interface GalleryProps {
  onLeave: () => void;
}

/**
 * The development gallery: every token and component in all five themes side
 * by side, plus overlays in the window's own theme. Its switches set the
 * window's theme, accent and density, as Preferences will. It is the visual
 * test for every theme.
 */
export default function Gallery({ onLeave }: GalleryProps) {
  const t = useT();
  const { appearance, resolved, update } = useAppearance();
  const [dialogOpen, setDialogOpen] = useState(false);

  const actions: Action[] = [
    { id: "dev.leaveGallery", label: "action-leave-gallery", group: "navigation", keys: [parseChord("Escape")], run: onLeave },
    { id: "window.close", label: "action-close-window", group: "window", keys: keysOf("window.close"), run: () => void appWindow.close() },
    { id: "app.quit", label: "action-quit", group: "app", keys: keysOf("app.quit"), run: () => void appWindow.close() },
  ];

  const theme: Theme = resolved.theme;
  const accent = appearance.accent === "system" ? "default" : appearance.accent;
  const columnAccent = (column: Theme): Accent => (appearance.accent === "system" ? THEME_ACCENT[column] : appearance.accent);

  return (
    <ActionsProvider actions={actions}>
      <div className={styles.gallery}>
        <header className={classes("headerbar", styles.toolbar)} data-tauri-drag-region>
          <Button className="btn flat" onPress={onLeave}>
            <ArrowLeft className="ic" />
            {t("action-leave-gallery")}
          </Button>
          <h1 className={styles.title} data-tauri-drag-region>
            {t("gallery-title")}
          </h1>
          <div className="grow" data-tauri-drag-region />
          <Segmented
            label={t("gallery-theme")}
            segments={THEMES.map((id) => ({ id, label: t(`theme-${id}`) }))}
            value={theme}
            onChange={(next) => { update({ theme: next }); }}
          />
          <Segmented
            label={t("gallery-density")}
            segments={DENSITIES.map((id) => ({ id, label: t(`density-${id}`) }))}
            value={appearance.density}
            onChange={(next) => { update({ density: next }); }}
          />
        </header>
        <div className={styles.accentBar}>
          <span className="caption">{t("gallery-accent")}</span>
          <Segmented
            label={t("gallery-accent")}
            segments={[
              { id: "default", label: t("gallery-accent-default") },
              ...ACCENTS.map((id) => ({
                id,
                label: <span className={styles.accentDot} data-accent={id} />,
                ariaLabel: t(`accent-${id}`),
              })),
            ]}
            value={accent}
            onChange={(next) => { update({ accent: next === "default" ? "system" : next }); }}
          />
        </div>

        <section className={styles.overlays} aria-label={t("gallery-overlays")}>
          <span className="caption">{t("gallery-overlays")}</span>
          <div className={styles.row}>
            <SampleMenu label={t("gallery-open-menu")} />
            <Button className="btn" onPress={() => { setDialogOpen(true); }}>
              {t("gallery-open-dialog")}
            </Button>
            <Tooltip text={t("gallery-tooltip-text")} chord={keysOf("palette.open")[0]}>
              <Button className="btn flat">{t("gallery-tooltip")}</Button>
            </Tooltip>
          </div>
        </section>

        <div className={styles.columns}>
          {THEMES.map((column) => (
            <ThemeColumn key={column} theme={column} accent={columnAccent(column)} density={appearance.density} />
          ))}
        </div>
      </div>
      <Dialog
        title={t("gallery-dialog-title")}
        isOpen={dialogOpen}
        onOpenChange={setDialogOpen}
        footer={
          <>
            <Button className="btn" onPress={() => { setDialogOpen(false); }}>
              {t("gallery-dialog-cancel")}
            </Button>
            <Button className="btn primary" onPress={() => { setDialogOpen(false); }}>
              {t("gallery-dialog-confirm")}
            </Button>
          </>
        }
      >
        <p className={styles.dialogText}>{t("gallery-dialog-body")}</p>
      </Dialog>
    </ActionsProvider>
  );
}

interface ThemeColumnProps {
  theme: Theme;
  accent: Accent;
  density: (typeof DENSITIES)[number];
}

function ThemeColumn({ theme, accent, density }: ThemeColumnProps) {
  const t = useT();
  const [switchOn, setSwitchOn] = useState(true);
  const [view, setView] = useState<string>(VIEWS[0]);
  return (
    <div className={styles.column} data-theme={theme} data-accent={accent} data-density={density} data-testid={`theme-${theme}`}>
      <header className={styles.columnHeader}>
        <div className={styles.row}>
          <h2 className={styles.columnTitle}>{t(`theme-${theme}`)}</h2>
          <span className="key">{`data-theme="${theme}"`}</span>
        </div>
        <span className="muted">{t(`theme-note-${theme}`)}</span>
      </header>

      <Section caption={t("gallery-surfaces")}>
        <div className={styles.swatches}>
          {SWATCHES.map((name) => (
            <div key={name} className={styles.swatchCell}>
              <div className={styles.swatch} style={{ background: `var(--${name})` }} />
              <span className={styles.swatchName}>{`--${name}`}</span>
            </div>
          ))}
        </div>
      </Section>

      <Section caption={t("gallery-hues")}>
        <div className={styles.row}>
          {HUES.map((hue) => (
            <span key={hue} className={`chip hue-${hue}`}>
              {hue}
            </span>
          ))}
        </div>
      </Section>

      <Section caption={t("gallery-buttons")}>
        <div className={styles.row}>
          <Button className="btn primary">
            <Plus className="ic" />
            {t("header-new-task")}
          </Button>
          <Button className="btn">{t("view-today")}</Button>
          <Button className="btn flat">
            {t("gallery-theme")}
            <ChevronDown className="ic s muted" />
          </Button>
          <Button className="btn on">{t("gallery-selected")}</Button>
          <Button className="btn danger">{t("gallery-dialog-confirm")}</Button>
          <Button className="btn" isDisabled>
            {t("action-undo")}
          </Button>
        </div>
      </Section>

      <Section caption={t("gallery-inputs")}>
        <div className={styles.row}>
          <label className={classes("field", styles.field)}>
            <Search className="ic" />
            <input type="text" aria-label={t("gallery-sample-field")} placeholder={t("gallery-sample-search")} spellCheck={false} autoCapitalize="off" autoCorrect="off" />
            {keysOf("palette.open").map((chord) => (
              <Kbd key="palette" chord={chord} />
            ))}
          </label>
          <input type="checkbox" className="check" defaultChecked aria-label={t("gallery-checked")} />
          <input type="checkbox" className="check" aria-label={t("gallery-unchecked")} />
          <input type="radio" className="radio" defaultChecked name={`radio-${theme}`} aria-label={t("gallery-selected")} />
          <input type="radio" className="radio" name={`radio-${theme}`} aria-label={t("gallery-unchecked")} />
          <Switch isSelected={switchOn} onChange={setSwitchOn} aria-label={t(switchOn ? "gallery-switch-on" : "gallery-switch-off")} />
        </div>
      </Section>

      <Section caption={t("gallery-switcher")}>
        <Segmented label={t("gallery-switcher")} segments={VIEWS.map((id) => ({ id, label: id }))} value={view} onChange={setView} />
      </Section>

      <Section caption={t("gallery-sidebar")}>
        <div className={styles.sidebarSample}>
          {PROJECTS.map((project) => (
            <button key={project.name} type="button" className={classes("nav", project.selected && "on")}>
              <span className={`dot hue-${project.hue}`} />
              <span>{project.name}</span>
              <span className="count">{project.count}</span>
            </button>
          ))}
        </div>
      </Section>

      <Section caption={t("gallery-board")}>
        <div className={classes("lane", styles.lane)}>
          <div className={styles.laneHeader}>
            <span className={`dot hue-${LANE.hue}`} />
            <span className={styles.strong}>{LANE.name}</span>
            <span className="count over">{t("gallery-wip", { count: LANE.count, limit: LANE.limit })}</span>
          </div>
          {CARDS.map((card) => (
            <article key={card.key} className={classes("card", card.selected && "sel")}>
              <div className={styles.row}>
                <TypeGlyph type={card.type} />
                <span className="key">{card.key}</span>
                <span className="grow" />
                <PriorityGlyph priority={card.priority} />
              </div>
              <div className="card-title">{card.title}</div>
              <div className={styles.row}>
                <span className={`chip hue-${card.label.hue}`}>{card.label.name}</span>
                <span className={`meta ${card.due.state}`}>{card.due.text}</span>
                {card.checklist && <span className="meta">{card.checklist}</span>}
              </div>
            </article>
          ))}
        </div>
      </Section>

      <Section caption={t("gallery-pills")}>
        <div className={styles.row}>
          {STATUSES.map((status) => (
            <span key={status.name} className={`pill hue-${status.hue}`}>
              <span className="dot" />
              {status.name}
            </span>
          ))}
        </div>
      </Section>

      <Section caption={t("gallery-tabs")}>
        <div className="tabs">
          {TABS.map((tab, index) => (
            <span key={tab} className={classes("tab", index === 0 && "on")}>
              {tab}
            </span>
          ))}
        </div>
      </Section>

      <Section caption={t("gallery-table")}>
        <div className={styles.table}>
          <div className={classes("tr head", styles.tableRow)}>
            {TABLE.columns.map((column) => (
              <span key={column} className="th">
                {column}
              </span>
            ))}
          </div>
          <div className={classes("tr group", styles.tableRow)}>
            <span>{TABLE.group}</span>
          </div>
          {TABLE.rows.map((row) => (
            <div key={row.key} className={classes("tr", styles.tableRow, row.selected && "on")}>
              <span className="key">{row.key}</span>
              <span className="truncate">{row.title}</span>
              <span className="muted">{row.status}</span>
              <span className="meta">{row.due}</span>
            </div>
          ))}
          <div className="banner">
            <Filter className="ic s" />
            {BANNER}
          </div>
        </div>
      </Section>

      <Section caption={t("gallery-properties")}>
        <dl className={styles.properties}>
          {PROPERTIES.map((property) => (
            <div key={property.name} className="prop">
              <dt>{property.name}</dt>
              <dd>
                <span className="val">{property.value}</span>
              </dd>
            </div>
          ))}
        </dl>
        <div className="meter">
          <i style={{ width: "45%" }} />
        </div>
      </Section>

      <Section caption={t("gallery-menu")}>
        <div className={classes("pop", styles.menuSample)}>
          {MENU.map((item, index) => (
            <div key={item} className={classes("item", index === 0 && "on")}>
              <span className={classes("grow", index === MENU.length - 1 && "accent")}>{item}</span>
            </div>
          ))}
        </div>
      </Section>

      <Section caption={t("gallery-markdown")}>
        <div className="md">
          <h3>{MARKDOWN.heading}</h3>
          <p>
            {MARKDOWN.paragraph} <a href="#markdown">{MARKDOWN.link}</a>
          </p>
          <pre>
            <span className="com">{MARKDOWN.code[0]}</span>
            {"\n"}
            <span className="kw">{MARKDOWN.code[1]}</span>
            {"\n"}
            <span className="fn">{MARKDOWN.code[2]}</span>
          </pre>
          <div className="callout hue-blue">
            <span>{MARKDOWN.paragraph}</span>
          </div>
        </div>
      </Section>

      <Section caption={t("gallery-gantt")}>
        <div className={styles.row}>
          <div className={styles.ganttBar}>
            <i className={styles.ganttOnTrack} />
          </div>
          <div className={classes(styles.ganttBar, styles.ganttOverdue)}>
            <i className={styles.ganttLate} />
          </div>
        </div>
      </Section>
    </div>
  );
}

/** A menu of sample items in a popover, to show that overlays take the window's theme. */
function SampleMenu({ label }: { label: string }) {
  const open = parseChord("Enter");
  return (
    <MenuTrigger>
      <Button className="btn">
        {label}
        <ChevronDown className="ic s muted" />
      </Button>
      <Popover className="pop menuPopover" placement="bottom start" offset={4} data-testid="sample-menu">
        <Menu className="menu" disabledKeys={[MENU[3]]}>
          {MENU.map((item, index) => (
            <MenuItem key={item} id={item} className="item" textValue={item}>
              <span className={classes("grow", item === MENU[MENU.length - 1] && "accent")}>{item}</span>
              {index === 0 && <Kbd chord={open} />}
            </MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

function Section({ caption, children }: { caption: string; children: ReactNode }) {
  return (
    <section className={styles.section}>
      <span className="caption">{caption}</span>
      {children}
    </section>
  );
}

/** The task type glyph: one SVG holds the four shapes, the class shows one. */
function TypeGlyph({ type }: { type: "task" | "bug" | "feature" | "milestone" }) {
  return (
    <svg className={`tt t-${type}`} viewBox="0 0 16 16" aria-hidden="true">
      <rect className="s-task" x="3" y="3" width="10" height="10" rx="2" />
      <circle className="s-bug" cx="8" cy="8" r="4" />
      <path className="s-feature" d="M8 3v10M3 8h10" />
      <path className="s-milestone" d="M8 2l6 6-6 6-6-6z" />
    </svg>
  );
}

/** The priority glyph: bars count up; urgent swaps to a filled alert box. */
function PriorityGlyph({ priority }: { priority: "low" | "normal" | "high" | "urgent" }) {
  return (
    <svg className={`prio p-${priority}`} viewBox="0 0 16 16" aria-hidden="true">
      <rect className="b b1" x="2" y="10" width="3" height="4" rx="1" />
      <rect className="b b2" x="6.5" y="6.5" width="3" height="7.5" rx="1" />
      <rect className="b b3" x="11" y="3" width="3" height="11" rx="1" />
      <path
        className="u"
        d="M3.5 2h9A1.5 1.5 0 0 1 14 3.5v9a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 12.5v-9A1.5 1.5 0 0 1 3.5 2zm3.7 2v5h1.6V4zm0 6.4V12h1.6v-1.6z"
      />
    </svg>
  );
}
