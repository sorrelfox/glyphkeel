// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The board with swimlanes: the column headers once at the top, with each
// column's total against its WIP limit, then a band for each lane with a
// cell per column. Each cell is a drop target of its own; the lanes scroll
// vertically and the board alone scrolls sideways, never the page.

import { useDroppable } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { ChevronDown, ChevronRight, Ellipsis, Plus } from "lucide-react";
import { useEffect, useState, type CSSProperties } from "react";
import { Button, Menu, MenuTrigger, Popover } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries, type MenuEntry } from "../../components/ContextMenu";
import { type Card } from "../../data/catalog";
import { type TemplateSummary } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { type LaneData } from "./dnd";
import { AddTaskField, CollapsedLane, laneCount, laneLabel, SortableCard, VIRTUAL_OVER, VirtualBody, type CardHandlers } from "./Lane";
import { type Lane as ColumnModel } from "./model";
import { cellId, type SwimlaneView } from "./swimlanes";
import styles from "./Board.module.css";

/** A column's width in the grid: a strip while collapsed. */
const track = (column: ColumnModel) => (column.collapsed ? "36px" : "minmax(252px, 360px)");

export interface SwimlanesProps {
  /** The board's label. */
  label: string;
  columns: readonly ColumnModel[];
  lanes: readonly SwimlaneView[];
  /** The card keys of each cell, by cell ID: during a drag, as they will be after it. */
  keys: Readonly<Record<string, string[]>>;
  cards: ReadonlyMap<string, Card>;
  handlers: CardHandlers;
  /** The key being dragged, if any. */
  active: string | null;
  /** The cell the dragged card is over. */
  over: string | null;
  /** The dragged card can't land in this cell. */
  closed: (cell: string) => boolean;
  /** Why the dragged card can't land in this cell, shown while it is dragged. */
  reason: (cell: string) => string | undefined;
  /** A lane's name as people read it. */
  nameOf: (lane: SwimlaneView["lane"]) => string;
  onToggle: (lane: string) => void;
  columnMenu: (column: string, index: number) => readonly MenuEntry[];
  onExpandColumn: (column: string) => void;
  /** The column whose Add task field is open. */
  adding: string | null;
  templates: readonly TemplateSummary[];
  onAdd: (column: string) => void;
  onCreate: (column: string, title: string, template: string | null) => void;
  onStopAdding: () => void;
  onScroller: (cell: string, run: ((index: number) => void) | null) => void;
}

export function Swimlanes(props: SwimlanesProps) {
  const { columns, lanes, label } = props;
  const grid: CSSProperties = { gridTemplateColumns: columns.map(track).join(" ") };
  return (
    <div className={styles.swimBoard} role="group" aria-label={label}>
      <div className={styles.swimHead} style={grid}>
        {columns.map((column, index) =>
          column.collapsed ? (
            <CollapsedLane
              key={column.column.id}
              lane={column}
              onExpand={() => {
                props.onExpandColumn(column.column.id);
              }}
            />
          ) : (
            <ColumnHead key={column.column.id} column={column} index={index} {...props} />
          ),
        )}
      </div>
      {lanes.map((lane, row) => (
        <SwimlaneRow key={lane.lane.id} lane={lane} row={row} grid={grid} {...props} />
      ))}
    </div>
  );
}

/** A column's header: its name, its total against the WIP limit, Add task and its menu. */
function ColumnHead({ column, index, columnMenu, adding, templates, onAdd, onCreate, onStopAdding }: { column: ColumnModel; index: number } & SwimlanesProps) {
  const t = useT();
  const { column: data } = column;
  const limit = data.wip_limit;
  const min = data.wip_min;
  // Limits are the column's, whatever the lanes: they warn on its total.
  const over = limit !== undefined && column.total > limit;
  const under = min !== undefined && column.total < min;
  return (
    <section className={classes("lane", styles.swimColumn)} aria-label={laneLabel(t, data.name, column.total, limit, min, under)} data-column={data.id}>
      <header className={styles.laneHeader}>
        <span className={`dot hue-${data.hue}`} />
        <h2 className={styles.laneName}>{data.name}</h2>
        <span className={classes("count", over && "over", under && "under")} data-testid="lane-count">
          {laneCount(t, column.total, limit, min)}
        </span>
        <div className="grow" />
        <Button
          className="btn flat icon sm"
          aria-label={t("lane-add", { name: data.name })}
          onPress={() => {
            onAdd(data.id);
          }}
        >
          <Plus className="ic s" />
        </Button>
        <MenuTrigger>
          <Button className="btn flat icon sm" aria-label={t("lane-menu", { name: data.name })}>
            <Ellipsis className="ic s dots" />
          </Button>
          <Popover className="pop menuPopover" placement="bottom end" offset={4}>
            <Menu className="menu">
              <MenuEntries entries={columnMenu(data.id, index)} />
            </Menu>
          </Popover>
        </MenuTrigger>
      </header>
      {adding === data.id && (
        <div className={styles.swimAdd}>
          <AddTaskField
            name={data.name}
            templates={templates}
            onCreate={(title, template) => {
              onCreate(data.id, title, template);
            }}
            onStop={onStopAdding}
          />
        </div>
      )}
    </section>
  );
}

/** One lane: its header with the collapse button, then a cell for each column. */
function SwimlaneRow({ lane, row, grid, columns, nameOf, onToggle, ...rest }: { lane: SwimlaneView; row: number; grid: CSSProperties } & SwimlanesProps) {
  const t = useT();
  const name = nameOf(lane.lane);
  const collapsed = lane.collapsed;
  const Chevron = collapsed ? ChevronRight : ChevronDown;
  return (
    <section className={styles.swimlane} aria-label={t("swimlane-label", { name, count: lane.total })} data-swimlane={lane.lane.id}>
      <header className={styles.swimHeader}>
        <button
          type="button"
          className="btn flat icon sm"
          aria-expanded={!collapsed}
          aria-label={t(collapsed ? "swimlane-expand" : "swimlane-collapse", { name, count: lane.total })}
          onClick={() => {
            onToggle(lane.lane.id);
          }}
        >
          <Chevron className="ic s muted" />
        </button>
        {lane.lane.hue !== undefined && <span className={`dot hue-${lane.lane.hue}`} />}
        <h3 className={styles.swimName}>{name}</h3>
        <span className="count" data-testid="swimlane-count">
          {lane.total}
        </span>
      </header>
      {!collapsed && (
        <div className={styles.swimCells} style={grid}>
          {columns.map((column) =>
            column.collapsed ? (
              <div key={column.column.id} className={styles.swimGap} />
            ) : (
              <Cell key={column.column.id} id={cellId(lane.lane.id, column.column.id)} row={row} column={column} lane={lane} laneName={name} {...rest} />
            ),
          )}
        </div>
      )}
    </section>
  );
}

interface CellProps extends Omit<SwimlanesProps, "columns" | "lanes" | "label" | "nameOf" | "onToggle"> {
  id: string;
  row: number;
  column: ColumnModel;
  lane: SwimlaneView;
  laneName: string;
}

/** One lane's cards in one column: a drop target of its own. */
function Cell({ id, row, column, lane, laneName, keys: allKeys, cards, handlers, active, over, closed: isClosed, reason: reasonOf, onScroller }: CellProps) {
  const t = useT();
  const keys = allKeys[id] ?? [];
  const closed = active !== null && isClosed(id);
  const reason = active === null ? undefined : reasonOf(id);
  const [scrollTo, setScrollTo] = useState<{ run: (index: number) => void } | null>(null);
  useEffect(() => {
    onScroller(id, scrollTo?.run ?? null);
    return () => {
      onScroller(id, null);
    };
  }, [id, onScroller, scrollTo]);
  const data: LaneData = { kind: "lane", cards: keys, closed, scrollTo: scrollTo?.run, row, column: column.column.id };
  const { setNodeRef } = useDroppable({ id, data, disabled: closed });
  const count = lane.counts[column.column.id] ?? 0;
  const sortable = column.sort === "manual";
  return (
    <div
      className={classes(styles.cell, over === id && !closed && styles.hot, closed && styles.closedLane)}
      role="group"
      aria-label={t("swimlane-cell", { lane: laneName, column: column.column.name, count })}
      data-lane={id}
    >
      <span className={classes("count", styles.cellCount)} aria-hidden="true">
        {count}
      </span>
      {reason !== undefined && (
        <p className={styles.laneRule} role="note">
          {reason}
        </p>
      )}
      <SortableContext id={id} items={[...keys]} strategy={verticalListSortingStrategy}>
        {keys.length > VIRTUAL_OVER ? (
          <VirtualBody keys={keys} cards={cards} handlers={handlers} active={active} sortable={sortable} closed={closed} setDroppable={setNodeRef} onScroller={setScrollTo} />
        ) : (
          <div ref={setNodeRef} className={styles.cellBody}>
            {keys.map((key) => {
              const card = cards.get(key);
              return card ? <SortableCard key={key} card={card} handlers={handlers} sortable={sortable} dropDisabled={closed} /> : null;
            })}
          </div>
        )}
      </SortableContext>
    </div>
  );
}
