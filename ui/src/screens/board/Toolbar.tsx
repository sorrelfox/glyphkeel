// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ArrowDownWideNarrow, CalendarDays, ChevronDown, EyeOff, Filter, Flag, History, Network, SlidersHorizontal, Tag } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover, type Selection } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries, type MenuEntry } from "../../components/ContextMenu";
import { cardsOf, type LayoutEdit } from "../../data/board";
import { CARD_FIELDS, PRIORITIES, SORT_MODES, type Board, type BoardLayout, type CardSize, type Project } from "../../data/catalog";
import { type CardColourBy, type CoverMode } from "../../generated/ops";
import { useCustomFields } from "../../data/fields";
import { useT } from "../../i18n/I18n";
import { ColourRulesDialog } from "./ColourRules";
import { DUE_FILTERS, type BoardView, type Filters } from "./model";
import styles from "./Board.module.css";

interface ToolbarProps {
  project: Project;
  board: Board;
  layout: BoardLayout;
  view: BoardView;
  filters: Filters;
  onFilters: (filters: Filters) => void;
  onLayout: (change: LayoutEdit | "reset") => void;
  /** Card size is kept in this window, not in the layout. */
  cardSize: CardSize;
  onCardSize: (size: CardSize) => void;
}

/** Choices for "hide closed cards after": days, 0 = never. */
const HIDE_DONE_DAYS = [7, 14, 30, 0] as const;

/** What can colour a card's edge, in the menu's order. */
const COLOUR_BY: readonly CardColourBy[] = ["none", "type", "priority", "label", "query"];

/** Which image cards show as their cover. */
const COVER_MODES: readonly CoverMode[] = ["off", "chosen", "first_image"];

interface PickerProps<T extends string> {
  label: string;
  icon: ReactNode;
  options: readonly { id: T; label: string; icon?: ReactNode }[];
  value: readonly T[];
  onChange: (value: T[]) => void;
}

/** A quick filter: a menu of choices, several at once; drawn toggled while any is chosen. */
function Picker<T extends string>({ label, icon, options, value, onChange }: PickerProps<T>) {
  const t = useT();
  const on = value.length > 0;
  return (
    <MenuTrigger>
      <Button className={classes("btn", on ? "on" : "flat")} aria-label={on ? t("filter-active", { name: label, count: value.length }) : label}>
        {icon}
        <span>{label}</span>
        {on && <span className="count">{value.length}</span>}
        <ChevronDown className="ic s muted" />
      </Button>
      <Popover className="pop menuPopover" placement="bottom start" offset={4}>
        <Menu
          className="menu"
          aria-label={label}
          selectionMode="multiple"
          selectedKeys={new Set(value)}
          onSelectionChange={(keys: Selection) => {
            onChange(options.filter((option) => keys === "all" || keys.has(option.id)).map((option) => option.id));
          }}
        >
          {options.map((option) => (
            <MenuItem key={option.id} id={option.id} className="item" textValue={option.label}>
              {({ isSelected }) => (
                <>
                  <input type="checkbox" className="check" checked={isSelected} readOnly tabIndex={-1} aria-hidden="true" />
                  {option.icon}
                  <span className="grow truncate">{option.label}</span>
                </>
              )}
            </MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

function MenuButton({ label, children, entries, on = false, icon }: { label: string; children: ReactNode; entries: readonly MenuEntry[]; on?: boolean; icon?: boolean }) {
  return (
    <MenuTrigger>
      <Button className={classes("btn", on ? "on" : "flat", icon && "icon")} aria-label={label}>
        {children}
      </Button>
      <Popover className="pop menuPopover" placement="bottom end" offset={4}>
        <Menu className="menu" aria-label={label}>
          <MenuEntries entries={entries} />
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

/**
 * The board's toolbar. Left: the filter field and the quick filters. Right:
 * Include sub-projects, the hidden-columns chip, the sort mode and the board
 * options. Filters are for this window; the layout is kept per project.
 */
export function Toolbar({ project, board, layout, view, filters, onFilters, onLayout, cardSize, onCardSize }: ToolbarProps) {
  const t = useT();
  const customFields = useCustomFields(project.key);
  const cards = [...cardsOf(board).values()];
  const labelNames = [...new Set(cards.flatMap((card) => card.labels.map((label) => label.name)))].sort();
  const allLabels = new Map(cards.flatMap((card) => card.labels.map((label) => [label.name, label] as const)));
  const hiddenTasks = view.hidden.reduce((sum, hidden) => sum + hidden.count, 0);
  const sorted = layout.sort_mode !== "manual";
  const [editingRules, setEditingRules] = useState(false);

  return (
    <div className={classes("toolbar", styles.toolbar)} role="toolbar" aria-label={t("board-toolbar")}>
      <label className={classes("field", styles.filterField)}>
        <Filter className="ic" />
        <input
          type="text"
          aria-label={t("filter-label")}
          placeholder={t("filter-placeholder")}
          value={filters.text}
          onChange={(event) => {
            onFilters({ ...filters, text: event.target.value });
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape" && filters.text !== "") {
              event.stopPropagation();
              onFilters({ ...filters, text: "" });
            }
          }}
        />
      </label>
      <Picker
        label={t("filter-labels")}
        icon={<Tag className="ic" />}
        options={labelNames.map((name) => ({ id: name, label: name, icon: <span className={`dot hue-${allLabels.get(name)?.hue ?? "grey"}`} /> }))}
        value={filters.labels}
        onChange={(labels) => {
          onFilters({ ...filters, labels });
        }}
      />
      <Picker
        label={t("filter-priority")}
        icon={<Flag className="ic" />}
        options={[...PRIORITIES].reverse().map((priority) => ({ id: priority, label: t(`priority-${priority}`) }))}
        value={filters.priorities}
        onChange={(priorities) => {
          onFilters({ ...filters, priorities });
        }}
      />
      <Picker
        label={t("filter-due")}
        icon={<CalendarDays className="ic" />}
        options={DUE_FILTERS.map((due) => ({ id: due, label: t(`filter-due-${due}`) }))}
        value={filters.due}
        onChange={(due) => {
          onFilters({ ...filters, due });
        }}
      />
      <button
        type="button"
        className={classes("btn", filters.recentlyUpdated ? "on" : "flat")}
        aria-pressed={filters.recentlyUpdated}
        // Named by its text; the full name is the tooltip, and the name when the text is hidden.
        title={t("filter-recent")}
        onClick={() => {
          onFilters({ ...filters, recentlyUpdated: !filters.recentlyUpdated });
        }}
      >
        <History className="ic" />
        <span className="hide-narrow">{t("filter-recent-short")}</span>
      </button>
      <div className="grow" />
      {project.children.length > 0 && (
        <button
          type="button"
          className={classes("btn", layout.include_subprojects ? "on" : "flat")}
          role="switch"
          aria-checked={layout.include_subprojects}
          onClick={() => {
            onLayout({ include_subprojects: !layout.include_subprojects });
          }}
        >
          <Network className="ic" />
          <span>{t("board-include-subprojects")}</span>
        </button>
      )}
      {view.hidden.length > 0 && (
        <MenuButton
          label={t("board-hidden-label", { columns: view.hidden.length, tasks: hiddenTasks })}
          on
          entries={[
            ...view.hidden.map(
              (hidden): MenuEntry => ({
                id: `show-${hidden.column.id}`,
                label: t("board-show-column", { name: hidden.column.name, count: hidden.count }),
                run: () => {
                  onLayout({ hidden: layout.hidden.filter((id) => id !== hidden.column.id) });
                },
              }),
            ),
            "separator",
            { id: "reset", label: t("board-reset-layout"), run: () => { onLayout("reset"); } },
          ]}
        >
          <EyeOff className="ic" />
          <span>{t("board-hidden", { columns: view.hidden.length, tasks: hiddenTasks })}</span>
          <ChevronDown className="ic s" />
        </MenuButton>
      )}
      <MenuButton
        label={t("board-sort-label", { mode: t(`sort-${layout.sort_mode}`) })}
        on={sorted}
        entries={SORT_MODES.map((mode) => ({
          id: mode,
          label: t(`sort-${mode}`),
          checked: layout.sort_mode === mode,
          run: () => {
            onLayout({ sort_mode: mode, column_sort: {} });
          },
        }))}
      >
        <ArrowDownWideNarrow className="ic" />
        <span className="hide-narrow">{t(`sort-${layout.sort_mode}`)}</span>
        <ChevronDown className="ic s muted" />
      </MenuButton>
      <MenuButton
        label={t("board-options")}
        icon
        entries={[
          {
            id: "fields",
            section: t("board-card-fields"),
            items: [
              ...CARD_FIELDS.map((field) => ({ id: field, label: t(`card-field-${field}`) })),
              // Custom fields: their names are the person's own words.
              ...customFields.map((field) => ({ id: field.query_name, label: field.name })),
            ].map(({ id, label }) => ({
              id: `field-${id}`,
              label,
              checked: layout.card_fields.includes(id),
              run: () => {
                onLayout({
                  card_fields: layout.card_fields.includes(id) ? layout.card_fields.filter((other) => other !== id) : [...layout.card_fields, id],
                });
              },
            })),
          },
          {
            id: "size",
            section: t("board-card-size"),
            items: (["regular", "compact"] as const).map((size) => ({
              id: `size-${size}`,
              label: t(`card-size-${size}`),
              checked: cardSize === size,
              run: () => {
                onCardSize(size);
              },
            })),
          },
          {
            id: "colour",
            section: t("board-card-colour"),
            items: [
              ...COLOUR_BY.map((by) => ({
                id: `colour-${by}`,
                label: t(`card-colour-${by}`),
                checked: layout.colour_by === by,
                run: () => {
                  // Rules need writing first.
                  if (by === "query" && layout.colour_rules.length === 0) {
                    setEditingRules(true);
                  } else {
                    onLayout({ colour_by: by });
                  }
                },
              })),
              {
                id: "colour-rules",
                label: t("card-colour-edit-rules"),
                run: () => {
                  setEditingRules(true);
                },
              },
            ],
          },
          {
            id: "covers",
            section: t("board-covers"),
            items: COVER_MODES.map((mode) => ({
              id: `covers-${mode}`,
              label: t(`covers-${mode}`),
              checked: layout.covers === mode,
              run: () => {
                onLayout({ covers: mode });
              },
            })),
          },
          {
            id: "done",
            section: t("board-hide-done"),
            items: HIDE_DONE_DAYS.map((days) => ({
              id: `done-${String(days)}`,
              label: days === 0 ? t("board-hide-done-never") : t("board-hide-done-days", { days }),
              checked: layout.hide_done_after_days === days,
              run: () => {
                onLayout({ hide_done_after_days: days });
              },
            })),
          },
          "separator",
          { id: "reset", label: t("board-reset-layout"), run: () => { onLayout("reset"); } },
        ]}
      >
        <SlidersHorizontal className="ic" />
      </MenuButton>
      {editingRules && (
        <ColourRulesDialog
          project={project.key}
          rules={layout.colour_rules}
          onClose={() => {
            setEditingRules(false);
          }}
        />
      )}
    </div>
  );
}
