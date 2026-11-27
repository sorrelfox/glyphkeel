// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { columnOrderingFeature, columnVisibilityFeature, createColumnHelper, tableFeatures, useTable } from "@tanstack/react-table";
import { type VirtualItem } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp, ChevronDown, ChevronRight, Plus } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type MouseEvent, type PointerEvent, type RefObject } from "react";

import { classes } from "../../classes";
import { isMenuKey } from "../../components/ContextMenu";
import { CLICK_SLOP, useWidthMenu } from "../../components/WidthMenu";
import { type ListRows } from "../../data/list";
import { useDurations } from "../../data/time";
import { NUMBER_KINDS } from "../../data/fields";
import { type CustomField, type ListedTask, type ListGroup } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { Cell } from "./cells";
import { GroupName, useGroupText } from "./GroupName";
import { columnWidth, COLUMNS, gridTemplate, isCustomColumn, MAX_WIDTH, MIN_WIDTH, NUMERIC_COLUMNS, sortKeyOf, sortState, type ColumnId, type Grouping, type ListLines, type Selection } from "./model";
import styles from "./List.module.css";

const features = tableFeatures({ columnVisibilityFeature, columnOrderingFeature });
const helper = createColumnHelper<typeof features, ListedTask>();

/**
 * Every column, as the table knows them: the built-in ones and those of the
 * custom fields shown; the list chooses which show, in which order.
 */
function columnDefs(custom: readonly ColumnId[], fields: readonly CustomField[]) {
  return helper.columns(
    [...COLUMNS, ...custom].map((id) =>
      helper.display({
        id,
        cell: (info) => <Cell column={id} task={info.row.original} field={fields.find((field) => field.query_name === id)} />,
      }),
    ),
  );
}

/** How far the keyboard moves a column edge, in pixels. */
const RESIZE_STEP = 16;
/** One "Wider" or "Narrower" from a column edge's menu. */
const MENU_STEP = 32;
/** How far the pointer moves before a press on a header becomes a drag. */
const DRAG_THRESHOLD = 4;
/** The space between columns and at the row's ends, from the design. */
const COLUMN_GAP = 12;
const ROW_PADDING = 24;
const CHECK_WIDTH = 28;

interface ListTableProps {
  rows: ListRows;
  columns: readonly ColumnId[];
  /** The custom fields of the tasks listed, for their columns' names and kinds. */
  fields?: readonly CustomField[];
  /** The person's column widths, in pixels. */
  widths: Readonly<Record<string, number>>;
  /** A column edge was dragged (or moved with the keyboard) to a new width. */
  onResize: (column: ColumnId, width: number) => void;
  /** Drops a column's kept width, back to its default. */
  onResetWidth?: (column: ColumnId) => void;
  /** A column was dragged (or moved with the keyboard) to another place. */
  onMove: (column: ColumnId, to: number) => void;
  /** The lines shown: rows and, when grouped, the groups' headers. */
  lines: ListLines;
  grouping: Grouping | undefined;
  isCollapsed: (group: ListGroup) => boolean;
  onToggleGroup: (group: ListGroup) => void;
  /** Adds a task with the group's value (the line at the end of each open group); absent where a group's value can't be given. */
  onAddToGroup?: ((group: ListGroup, title: string) => Promise<unknown>) | undefined;
  /** The order in use, as sort keys. */
  sort: readonly string[];
  /** A click on a header; `add` with Shift, for one more sort key. */
  onSort: (column: ColumnId, add: boolean) => void;
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
  /** The lines in view, from the virtualizer, and the height of all lines. */
  items: readonly VirtualItem[];
  totalSize: number;
  scrollRef: RefObject<HTMLDivElement | null>;
}

/**
 * The list's table: a header that sorts (Shift adds a sort key), whose
 * columns are dragged to another place and resized at their edge; the
 * lines in view only (virtualized, fixed heights from the density), task
 * rows and, when grouped, a header per group that collapses it and a
 * line at its end that adds a task with the group's value; and a
 * totals row over every matching task. Rows not loaded yet show as empty
 * placeholders.
 */
export function ListTable({ rows, columns, fields = [], widths, onResize, onResetWidth, onMove, lines, grouping, isCollapsed, onToggleGroup, onAddToGroup, sort, onSort, selection, onRowClick, onToggleAll, allSelected, onOpen, focused, onFocusRow, onKey, items, totalSize, scrollRef }: ListTableProps) {
  const t = useT();
  const { format } = useDurations();
  // A column edge being dragged shows its width before it is kept.
  const [draft, setDraft] = useState<{ column: ColumnId; width: number } | null>(null);
  const shownWidths = draft ? { ...widths, [draft.column]: draft.width } : widths;
  const headRef = useRef<HTMLDivElement>(null);
  const reorder = useColumnDrag(columns, onMove, headRef);
  // The table models the rows in view, with the columns chosen.
  const lineAt = (index: number) => lines.at(index);
  const visible = items
    .map((item) => {
      const line = lineAt(item.index);
      return line?.kind === "row" ? rows.rowAt(line.row) : undefined;
    })
    .filter((task): task is ListedTask => task !== undefined);
  const custom = columns.filter(isCustomColumn);
  const customKey = custom.join(" ");
  // The definitions change only when other custom columns show.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const defs = useMemo(() => columnDefs(custom, fields), [customKey, fields]);
  const numeric = (column: ColumnId) => NUMERIC_COLUMNS.has(column) || fields.some((field) => field.query_name === column && NUMBER_KINDS.has(field.kind));
  const table = useTable({
    features,
    columns: defs,
    data: visible,
    getRowId: (task) => task.key,
    state: {
      columnOrder: [...columns],
      columnVisibility: Object.fromEntries([...COLUMNS, ...custom].map((id) => [id, columns.includes(id)])),
    },
  });
  const rowById = new Map(table.getRowModel().rows.map((row) => [row.id, row]));
  const template: CSSProperties = { gridTemplateColumns: gridTemplate(columns, shownWidths) };
  // Wider columns than the screen scroll sideways.
  const needed = CHECK_WIDTH + ROW_PADDING + columns.reduce((sum, column) => sum + columnWidth(column, shownWidths) + COLUMN_GAP, 0);
  const minWidth = Math.max(1080, needed);
  const total = rows.total ?? 0;
  const totals = rows.totals;
  // The totals row sits under the scrolling rows, not in them, so no scroll
  // bar ever lies over it: it is as wide as the rows and follows them sideways.
  const sizerRef = useRef<HTMLDivElement>(null);
  const footRef = useRef<HTMLDivElement>(null);
  const [contentWidth, setContentWidth] = useState<number | undefined>(undefined);
  const followScroll = () => {
    const scroller = scrollRef.current;
    if (footRef.current && scroller) {
      footRef.current.scrollLeft = scroller.scrollLeft;
    }
  };
  const hasTotals = totals !== undefined;
  useEffect(() => {
    const sizer = sizerRef.current;
    if (!sizer || typeof ResizeObserver === "undefined") {
      return undefined;
    }
    const observer = new ResizeObserver(() => {
      setContentWidth(sizer.getBoundingClientRect().width);
      const scroller = scrollRef.current;
      if (footRef.current && scroller) {
        footRef.current.scrollLeft = scroller.scrollLeft;
      }
    });
    observer.observe(sizer);
    return () => {
      observer.disconnect();
    };
  }, [scrollRef, hasTotals]);
  const groups = rows.groups ?? [];
  const multiSort = sort.length > 1;

  // A custom field's column is named by the field, as the person wrote it.
  const columnLabel = (column: ColumnId) => (isCustomColumn(column) ? (fields.find((field) => field.query_name === column)?.name ?? column) : t(`list-column-${column}`));
  const widthMenu = useWidthMenu();

  /** Starts dragging a column's edge; the width is kept when the pointer lets go, and a click opens the width menu. */
  const startResize = (event: PointerEvent<HTMLElement>, column: ColumnId) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const header = event.currentTarget.parentElement;
    const startX = event.clientX;
    const measured = header?.getBoundingClientRect().width ?? 0;
    const startWidth = measured > 0 ? measured : columnWidth(column, widths);
    // In a right-to-left layout the edge is on the left: moving left widens.
    const direction = header && getComputedStyle(header).direction === "rtl" ? -1 : 1;
    const target = event.currentTarget;
    target.setPointerCapture(event.pointerId);
    const widthAt = (x: number) => Math.round(Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, startWidth + (x - startX) * direction)));
    const move = (next: globalThis.PointerEvent) => {
      setDraft({ column, width: widthAt(next.clientX) });
    };
    const up = (next: globalThis.PointerEvent) => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
      target.removeEventListener("pointercancel", up);
      setDraft(null);
      if (next.type !== "pointerup") {
        return;
      }
      if (Math.abs(next.clientX - startX) > CLICK_SLOP) {
        onResize(column, widthAt(next.clientX));
      } else {
        // A click without dragging: the menu with "Wider" and "Narrower".
        openWidthMenu(target, column, startWidth, true);
      }
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
    target.addEventListener("pointercancel", up);
  };

  const openWidthMenu = (handle: HTMLElement, column: ColumnId, width: number, click = false) => {
    widthMenu.openAt(
      handle,
      {
        label: t("list-resize-column", { column: columnLabel(column) }),
        value: Math.round(width),
        min: MIN_WIDTH,
        max: MAX_WIDTH,
        step: MENU_STEP,
        onChange: (next) => {
          onResize(column, next);
        },
        onReset:
          onResetWidth && widths[column] !== undefined
            ? () => {
                onResetWidth(column);
              }
            : undefined,
      },
      click,
    );
  };

  const resizeKey = (event: KeyboardEvent, column: ColumnId, width: number) => {
    if (isMenuKey(event)) {
      event.preventDefault();
      openWidthMenu(event.currentTarget as HTMLElement, column, width);
      return;
    }
    const rtl = getComputedStyle(event.currentTarget).direction === "rtl";
    const steps: Record<string, number> = { ArrowLeft: rtl ? RESIZE_STEP : -RESIZE_STEP, ArrowRight: rtl ? -RESIZE_STEP : RESIZE_STEP };
    const step = steps[event.key];
    if (step !== undefined) {
      event.preventDefault();
      onResize(column, Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, Math.round(width) + step)));
    }
  };

  /** Ctrl+Shift+Arrow on a header moves its column. */
  const moveKey = (event: KeyboardEvent, column: ColumnId) => {
    if (!(event.ctrlKey && event.shiftKey) || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) {
      return;
    }
    event.preventDefault();
    const rtl = getComputedStyle(event.currentTarget).direction === "rtl";
    const forward = (event.key === "ArrowRight") !== rtl;
    onMove(column, columns.indexOf(column) + (forward ? 1 : -1));
    // The header keeps the focus in its new place.
    const focused = event.currentTarget as HTMLElement;
    requestAnimationFrame(() => {
      focused.focus();
    });
  };

  return (
    <>
      <div className={styles.grid} role="grid" aria-label={t("list-table-label")} aria-rowcount={lines.count + 2} aria-multiselectable="true" aria-busy={rows.pending}>
        <div
          ref={scrollRef}
          className={styles.scroll}
          data-list-scroll
          onScroll={followScroll}
        >
          <div ref={sizerRef} className={styles.sizer} style={{ minInlineSize: minWidth }}>
            <div ref={headRef} className={classes("tr head", styles.head)} role="row" aria-rowindex={1} style={template}>
              <span role="columnheader" className={styles.checkCell}>
                <input type="checkbox" className={styles.check} aria-label={t("list-select-all")} checked={allSelected} onChange={onToggleAll} disabled={total === 0} />
              </span>
              {columns.map((column) => {
                const state = sortState(sort, column);
                const label = columnLabel(column);
                const width = columnWidth(column, shownWidths);
                return (
                  <span
                    key={column}
                    role="columnheader"
                    aria-sort={state?.position === 1 ? state.direction : "none"}
                    className={classes("th", styles.th, numeric(column) && styles.end, state !== null && "accent", reorder.dragging === column && styles.dragging, reorder.target === column && styles.dropBefore, reorder.target === "end" && column === columns.at(-1) && styles.dropAfter)}
                    data-column={column}
                    onPointerDown={(event) => {
                      reorder.start(event, column);
                    }}
                    onClickCapture={reorder.swallowClick}
                  >
                    {sortKeyOf(column) === undefined ? (
                      <span
                        className={classes("truncate", styles.headLabel)}
                        tabIndex={0}
                        aria-keyshortcuts="Control+Shift+ArrowLeft Control+Shift+ArrowRight"
                        onKeyDown={(event) => {
                          moveKey(event, column);
                        }}
                      >
                        {label}
                      </span>
                    ) : (
                      <button
                        type="button"
                        className={styles.sortButton}
                        onClick={(event) => {
                          onSort(column, event.shiftKey);
                        }}
                        onKeyDown={(event) => {
                          moveKey(event, column);
                        }}
                        aria-keyshortcuts="Control+Shift+ArrowLeft Control+Shift+ArrowRight"
                        aria-label={t("list-sort-by", { column: label })}
                        aria-description={state === null ? undefined : t(`list-sorted-${state.direction}`, { position: state.position })}
                      >
                        <span className="truncate">{label}</span>
                        {state?.direction === "ascending" && <ArrowUp className="ic s" aria-hidden="true" />}
                        {state?.direction === "descending" && <ArrowDown className="ic s" aria-hidden="true" />}
                        {state !== null && multiSort && (
                          <span className={styles.sortPosition} aria-hidden="true">
                            {t("list-sort-position", { position: state.position })}
                          </span>
                        )}
                      </button>
                    )}
                    <span
                      role="separator"
                      aria-orientation="vertical"
                      aria-label={t("list-resize-column", { column: label })}
                      aria-valuenow={Math.round(width)}
                      aria-valuemin={MIN_WIDTH}
                      aria-valuemax={MAX_WIDTH}
                      title={t("resize-hint")}
                      tabIndex={0}
                      className={styles.resizer}
                      data-resize={column}
                      onPointerDown={(event) => {
                        startResize(event, column);
                      }}
                      onClick={(event) => {
                        event.stopPropagation();
                      }}
                      onContextMenu={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        const measured = event.currentTarget.parentElement?.getBoundingClientRect().width ?? 0;
                        openWidthMenu(event.currentTarget, column, measured > 0 ? measured : width);
                      }}
                      onKeyDown={(event) => {
                        const measured = event.currentTarget.parentElement?.getBoundingClientRect().width ?? 0;
                        resizeKey(event, column, measured > 0 ? measured : width);
                      }}
                    />
                  </span>
                );
              })}
            </div>
            <div className={styles.body} style={{ height: totalSize }}>
              {items.map((item) => {
                const line = lineAt(item.index);
                const position: CSSProperties = { height: item.size, transform: `translateY(${String(item.start)}px)` };
                const style: CSSProperties = { ...template, ...position };
                if (line?.kind === "group" && grouping !== undefined) {
                  const group = groups[line.group];
                  if (!group) {
                    return null;
                  }
                  const collapsed = isCollapsed(group);
                  return (
                    <div key={`group-${String(line.group)}`} className={classes("tr group", styles.row, styles.groupRow)} role="row" aria-rowindex={item.index + 2} style={position} data-group={group.value ?? ""}>
                      <span role="gridcell" className={styles.groupCell}>
                        <button
                          type="button"
                          className={styles.groupButton}
                          aria-expanded={!collapsed}
                          onClick={() => {
                            onToggleGroup(group);
                          }}
                        >
                          {collapsed ? <ChevronRight className="ic s muted" aria-hidden="true" /> : <ChevronDown className="ic s muted" aria-hidden="true" />}
                          {group.hue !== undefined && <span className={classes("dot", `hue-${group.hue}`)} aria-hidden="true" />}
                          <GroupName grouping={grouping} group={group} fields={fields} />
                          <span className="count">{t("list-group-count", { count: group.count })}</span>
                        </button>
                      </span>
                    </div>
                  );
                }
                if (line?.kind === "add") {
                  const group = groups[line.group];
                  if (!group || !onAddToGroup) {
                    return null;
                  }
                  return (
                    <div key={`add-${String(line.group)}`} className={classes("tr", styles.row, styles.groupRow, styles.addRow)} role="row" aria-rowindex={item.index + 2} style={position}>
                      <span role="gridcell" className={styles.groupCell}>
                        <AddToGroup group={group} grouping={grouping} fields={fields} onAdd={onAddToGroup} />
                      </span>
                    </div>
                  );
                }
                const index = line?.kind === "row" ? line.row : -1;
                const task = rows.rowAt(index);
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
                  onRowClick(index, task, { toggle: event.ctrlKey || event.metaKey, range: event.shiftKey });
                };
                return (
                  <div
                    key={task.key}
                    className={classes("tr", styles.row, selected && "on")}
                    role="row"
                    aria-rowindex={item.index + 2}
                    aria-selected={selected}
                    tabIndex={focused === index ? 0 : -1}
                    data-row={task.key}
                    data-index={index}
                    style={style}
                    onClick={click}
                    onDoubleClick={() => {
                      onOpen(task.key);
                    }}
                    onFocus={(event) => {
                      if (event.target === event.currentTarget) {
                        onFocusRow(index);
                      }
                    }}
                    onKeyDown={(event) => {
                      onKey(event, index);
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
                          onRowClick(index, task, { toggle: true, range: event.shiftKey });
                        }}
                        onChange={() => undefined}
                      />
                    </span>
                    {row?.getVisibleCells().map((cell) => (
                      <span key={cell.id} role="gridcell" className={classes(styles.cell, numeric(cell.column.id as ColumnId) && styles.end)}>
                        <table.FlexRender cell={cell} />
                      </span>
                    ))}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
        {totals && (
          <div ref={footRef} className={styles.foot}>
            <div className={styles.sizer} style={{ minInlineSize: minWidth, inlineSize: contentWidth }}>
              <div className={classes("tr", styles.totals)} role="row" aria-rowindex={lines.count + 2} style={template} data-testid="list-totals">
                <span />
                {columns.map((column) => (
                  <span key={column} role="gridcell" className={classes(styles.cell, styles.totalCell, numeric(column) && styles.end)}>
                    {column === "title" && <span className="muted">{t("list-totals", { count: totals.count, open: totals.open })}</span>}
                    {column === "estimate" && <span className="num">{format(totals.estimate_minutes)}</span>}
                    {column === "spent" && <span className="num">{format(totals.spent_minutes)}</span>}
                    {column === "remaining" && <span className="num">{format(totals.remaining_minutes)}</span>}
                  </span>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>
      {widthMenu.menu}
    </>
  );
}

/**
 * Dragging a header to another place: a press that moves a few pixels
 * becomes a drag, the header under the pointer shows where the column
 * lands, and the click that ends the drag doesn't sort.
 */
function useColumnDrag(columns: readonly ColumnId[], onMove: (column: ColumnId, to: number) => void, headRef: RefObject<HTMLDivElement | null>) {
  const [dragging, setDragging] = useState<ColumnId | null>(null);
  const [target, setTarget] = useState<ColumnId | "end" | null>(null);
  const dragged = useRef(false);

  /** The place among the columns the pointer is over: before the header whose middle is past it. */
  const placeAt = (x: number): number => {
    const headers = [...(headRef.current?.querySelectorAll<HTMLElement>("[data-column]") ?? [])];
    const rtl = headRef.current ? getComputedStyle(headRef.current).direction === "rtl" : false;
    const index = headers.findIndex((header) => {
      const box = header.getBoundingClientRect();
      const middle = box.left + box.width / 2;
      return rtl ? x > middle : x < middle;
    });
    return index < 0 ? headers.length : index;
  };

  const start = (event: PointerEvent<HTMLElement>, column: ColumnId) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest("[data-resize]")) {
      return;
    }
    const startX = event.clientX;
    const pointer = event.pointerId;
    let active = false;
    dragged.current = false;
    // The pointer leaves the header as it drags: the window follows it.
    const move = (next: globalThis.PointerEvent) => {
      if (next.pointerId !== pointer || (!active && Math.abs(next.clientX - startX) < DRAG_THRESHOLD)) {
        return;
      }
      if (!active) {
        active = true;
        dragged.current = true;
        setDragging(column);
      }
      const place = placeAt(next.clientX);
      setTarget(columns[place] ?? "end");
    };
    const up = (next: globalThis.PointerEvent) => {
      if (next.pointerId !== pointer) {
        return;
      }
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      setDragging(null);
      setTarget(null);
      // The click that ends a drag comes at once; later clicks sort again.
      setTimeout(() => {
        dragged.current = false;
      }, 0);
      if (active && next.type === "pointerup") {
        const place = placeAt(next.clientX);
        const from = columns.indexOf(column);
        // Taking the column out moves the places after it one back.
        onMove(column, place > from ? place - 1 : place);
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  };

  const swallowClick = (event: MouseEvent) => {
    if (dragged.current) {
      dragged.current = false;
      event.stopPropagation();
      event.preventDefault();
    }
  };

  return { dragging, target, start, swallowClick };
}

interface AddToGroupProps {
  group: ListGroup;
  grouping: Grouping | undefined;
  fields: readonly CustomField[];
  onAdd: (group: ListGroup, title: string) => Promise<unknown>;
}

/**
 * The "+" line at the end of a group: a button, then a title field. Enter
 * adds a task with the group's value and keeps the field for the next one;
 * Escape, or leaving the field empty, closes it.
 */
function AddToGroup({ group, grouping, fields, onAdd }: AddToGroupProps) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const groupText = useGroupText(fields);
  const name = grouping === undefined ? "" : groupText(grouping, group);
  if (!open) {
    return (
      <button
        type="button"
        className={classes(styles.groupButton, styles.addButton)}
        aria-label={t("list-group-add-to", { group: name })}
        onClick={() => {
          setOpen(true);
        }}
      >
        <Plus className="ic s" aria-hidden="true" />
        <span>{t("list-group-add")}</span>
      </button>
    );
  }
  return (
    <span className={classes("field", styles.addField)}>
      <input
        aria-label={t("list-group-add-title-in", { group: name })}
        placeholder={t("list-group-add-title")}
        value={title}
        // The field appears where the person asked for it.
        autoFocus
        onChange={(event) => {
          setTitle(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" && title.trim() !== "") {
            event.preventDefault();
            // A refusal shows as a toast; the title stays to be corrected.
            void onAdd(group, title.trim()).then(
              () => {
                setTitle("");
              },
              () => undefined,
            );
          } else if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            setTitle("");
            setOpen(false);
          }
        }}
        onBlur={() => {
          if (title.trim() === "") {
            setOpen(false);
          }
        }}
      />
    </span>
  );
}
