// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { columnOrderingFeature, columnVisibilityFeature, createColumnHelper, tableFeatures, useTable } from "@tanstack/react-table";
import { type VirtualItem } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp } from "lucide-react";
import { type CSSProperties, type KeyboardEvent, type MouseEvent, type RefObject } from "react";

import { classes } from "../../classes";
import { type ListRows } from "../../data/list";
import { useDurations } from "../../data/time";
import { type ListedTask } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { Cell } from "./cells";
import { COLUMNS, gridTemplate, NUMERIC_COLUMNS, SORT_KEY, sortState, type ColumnId, type Selection } from "./model";
import styles from "./List.module.css";

const features = tableFeatures({ columnVisibilityFeature, columnOrderingFeature });
const helper = createColumnHelper<typeof features, ListedTask>();

/** Every column, as the table knows them; the list chooses which show, in which order. */
const COLUMN_DEFS = helper.columns(
  COLUMNS.map((id) =>
    helper.display({
      id,
      cell: (info) => <Cell column={id} task={info.row.original} />,
    }),
  ),
);

interface ListTableProps {
  rows: ListRows;
  columns: readonly ColumnId[];
  /** The order in use, as sort keys. */
  sort: readonly string[];
  onSort: (column: ColumnId) => void;
  selection: Selection;
  /** A click on a row, with the keys held. */
  onRowClick: (index: number, task: ListedTask, mods: { toggle: boolean; range: boolean }) => void;
  onToggleAll: () => void;
  allSelected: boolean;
  onOpen: (key: string) => void;
  /** The row with the keyboard focus. */
  focused: number;
  onFocusRow: (index: number) => void;
  onKey: (event: KeyboardEvent, index: number) => void;
  /** The rows in view, from the virtualizer, and the height of all rows. */
  items: readonly VirtualItem[];
  totalSize: number;
  scrollRef: RefObject<HTMLDivElement | null>;
}

/**
 * The list's table: a header that sorts, the rows in view only (virtualized,
 * fixed heights from the density), and a totals row over every matching
 * task. Rows not loaded yet show as empty placeholders.
 */
export function ListTable({ rows, columns, sort, onSort, selection, onRowClick, onToggleAll, allSelected, onOpen, focused, onFocusRow, onKey, items, totalSize, scrollRef }: ListTableProps) {
  const t = useT();
  const { format } = useDurations();
  // The table models the rows in view, with the columns chosen.
  const visible = items.map((item) => rows.rowAt(item.index)).filter((task): task is ListedTask => task !== undefined);
  const table = useTable({
    features,
    columns: COLUMN_DEFS,
    data: visible,
    getRowId: (task) => task.key,
    state: {
      columnOrder: [...columns],
      columnVisibility: Object.fromEntries(COLUMNS.map((id) => [id, columns.includes(id)])),
    },
  });
  const rowById = new Map(table.getRowModel().rows.map((row) => [row.id, row]));
  const template: CSSProperties = { gridTemplateColumns: gridTemplate(columns) };
  const total = rows.total ?? 0;
  const totals = rows.totals;

  return (
    <div
      ref={scrollRef}
      className={styles.scroll}
      role="grid"
      aria-label={t("list-table-label")}
      aria-rowcount={total + 2}
      aria-multiselectable="true"
      aria-busy={rows.pending}
      data-list-scroll
    >
      <div className={styles.sizer}>
        <div className={classes("tr head", styles.head)} role="row" aria-rowindex={1} style={template}>
          <span role="columnheader" className={styles.checkCell}>
            <input type="checkbox" className={styles.check} aria-label={t("list-select-all")} checked={allSelected} onChange={onToggleAll} disabled={total === 0} />
          </span>
          {columns.map((column) => {
            const state = sortState(sort, column);
            const label = t(`list-column-${column}`);
            return (
              <span
                key={column}
                role="columnheader"
                aria-sort={state ?? "none"}
                className={classes("th", NUMERIC_COLUMNS.has(column) && styles.end, state !== null && "accent")}
              >
                {SORT_KEY[column] === undefined ? (
                  label
                ) : (
                  <button type="button" className={styles.sortButton} onClick={() => { onSort(column); }} aria-label={t("list-sort-by", { column: label })}>
                    <span className="truncate">{label}</span>
                    {state === "ascending" && <ArrowUp className="ic s" aria-hidden="true" />}
                    {state === "descending" && <ArrowDown className="ic s" aria-hidden="true" />}
                  </button>
                )}
              </span>
            );
          })}
        </div>
        <div className={styles.body} style={{ height: totalSize }}>
          {items.map((item) => {
            const task = rows.rowAt(item.index);
            const style: CSSProperties = { ...template, height: item.size, transform: `translateY(${String(item.start)}px)` };
            if (!task) {
              return (
                <div key={item.key} className={classes("tr", styles.row, styles.placeholder)} role="row" aria-rowindex={item.index + 2} style={style}>
                  <span />
                  <span className={styles.skeleton} />
                </div>
              );
            }
            const row = rowById.get(task.key);
            const selected = selection.keys.has(task.key);
            const click = (event: MouseEvent) => {
              onRowClick(item.index, task, { toggle: event.ctrlKey || event.metaKey, range: event.shiftKey });
            };
            return (
              <div
                key={task.key}
                className={classes("tr", styles.row, selected && "on")}
                role="row"
                aria-rowindex={item.index + 2}
                aria-selected={selected}
                tabIndex={focused === item.index ? 0 : -1}
                data-row={task.key}
                data-index={item.index}
                style={style}
                onClick={click}
                onDoubleClick={() => {
                  onOpen(task.key);
                }}
                onFocus={(event) => {
                  if (event.target === event.currentTarget) {
                    onFocusRow(item.index);
                  }
                }}
                onKeyDown={(event) => {
                  onKey(event, item.index);
                }}
              >
                <span role="gridcell" className={styles.checkCell}>
                  <input
                    type="checkbox"
                    className={styles.check}
                    tabIndex={-1}
                    aria-label={t("list-select-task", { key: task.key })}
                    checked={selected}
                    onClick={(event) => {
                      event.stopPropagation();
                      onRowClick(item.index, task, { toggle: true, range: event.shiftKey });
                    }}
                    onChange={() => undefined}
                  />
                </span>
                {row?.getVisibleCells().map((cell) => (
                  <span key={cell.id} role="gridcell" className={classes(styles.cell, NUMERIC_COLUMNS.has(cell.column.id as ColumnId) && styles.end)}>
                    <table.FlexRender cell={cell} />
                  </span>
                ))}
              </div>
            );
          })}
        </div>
        {totals && (
          <div className={classes("tr", styles.totals)} role="row" aria-rowindex={total + 2} style={template} data-testid="list-totals">
            <span />
            {columns.map((column) => (
              <span key={column} role="gridcell" className={classes(styles.cell, styles.totalCell, NUMERIC_COLUMNS.has(column) && styles.end)}>
                {column === "title" && <span className="muted">{t("list-totals", { count: totals.count, open: totals.open })}</span>}
                {column === "estimate" && <span className="num">{format(totals.estimate_minutes)}</span>}
                {column === "spent" && <span className="num">{format(totals.spent_minutes)}</span>}
                {column === "remaining" && <span className="num">{format(totals.remaining_minutes)}</span>}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
