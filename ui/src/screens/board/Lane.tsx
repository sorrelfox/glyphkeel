// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useDroppable } from "@dnd-kit/core";
import { SortableContext, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronRight, Ellipsis, Plus } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Button, Menu, MenuTrigger, Popover } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries, type MenuEntry } from "../../components/ContextMenu";
import { motionReduced } from "../../appearance/appearance";
import { Select } from "../../components/Select";
import { type Card } from "../../data/catalog";
import { type CustomField, type TemplateSummary } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { CardView } from "./CardView";
import { keepMounted, type LaneData } from "./dnd";
import { type Lane as LaneModel } from "./model";
import styles from "./Board.module.css";

/** A lane draws its cards directly up to this many, and virtually above. */
export const VIRTUAL_OVER = 150;

export interface CardHandlers {
  selected: string | null;
  fields: readonly string[];
  /** The custom fields of the board's project. */
  custom: readonly CustomField[];
  compact: boolean;
  today: string;
  onSelect: (key: string) => void;
  onOpen: (key: string) => void;
  /** Keys the card handles itself before drag and drop sees them; true when handled. */
  onKey: (key: string, event: KeyboardEvent<HTMLElement>) => boolean;
  onMenu: (key: string, at: { clientX: number; clientY: number } | Element) => void;
}

interface SortableCardProps {
  card: Card;
  handlers: CardHandlers;
  /** No sorting inside this lane: a sort mode other than manual. */
  sortable: boolean;
  /** Cards of another project's status can't be dropped here. */
  dropDisabled: boolean;
}

function SortableCard({ card, handlers, sortable, dropDisabled }: SortableCardProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: card.key,
    data: { kind: "card" },
    disabled: { draggable: false, droppable: dropDisabled || !sortable },
    // Cards make way for the dragged one without sliding when motion is off.
    ...(motionReduced() ? { transition: null } : {}),
  });
  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (handlers.onKey(card.key, event)) {
      return;
    }
    (listeners?.onKeyDown as ((event: KeyboardEvent<HTMLElement>) => void) | undefined)?.(event);
  };
  return (
    <CardView
      ref={setNodeRef}
      card={card}
      fields={handlers.fields}
      custom={handlers.custom}
      compact={handlers.compact}
      today={handlers.today}
      selected={handlers.selected === card.key}
      ghost={isDragging}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      {...attributes}
      {...listeners}
      onKeyDown={onKeyDown}
      onClick={() => {
        handlers.onSelect(card.key);
      }}
      onDoubleClick={() => {
        handlers.onOpen(card.key);
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        handlers.onSelect(card.key);
        handlers.onMenu(card.key, event);
      }}
      onFocus={() => {
        if (handlers.selected !== card.key) {
          handlers.onSelect(card.key);
        }
      }}
    />
  );
}

interface LaneProps {
  lane: LaneModel;
  /** The card keys to draw, in order: during a drag, the lanes as they will be after it. */
  keys: readonly string[];
  cards: ReadonlyMap<string, Card>;
  handlers: CardHandlers;
  /** The key being dragged, if any. */
  active: string | null;
  /** The dragged card can't land here. */
  closed: boolean;
  /** Why the status's rules keep the dragged card out, shown while it is dragged. */
  reason?: string | undefined;
  /** The dragged card is over this lane. */
  hot: boolean;
  adding: boolean;
  menu: readonly MenuEntry[];
  onAdd: () => void;
  /** Task templates the Add task field offers. */
  templates?: readonly TemplateSummary[];
  /** A new task: its title, and the template it starts from (the title may then be empty). */
  onCreate: (title: string, template: string | null) => void;
  onStopAdding: () => void;
  onShowOlder: () => void;
  /** Hands the board how to bring a place of this lane into view while it is drawn virtually, and null when not. */
  onScroller?: (run: ((index: number) => void) | null) => void;
}

/** The count in a column's header: alone, against the WIP limit, or against the minimum and the limit. */
function laneCount(t: ReturnType<typeof useT>, count: number, limit: number | undefined, min: number | undefined): string | number {
  if (min !== undefined && limit !== undefined) {
    return t("lane-wip-range", { count, min, limit });
  }
  if (min !== undefined) {
    return t("lane-wip-min", { count, min });
  }
  return limit === undefined ? count : t("lane-wip", { count, limit });
}

/** The column's accessible name, with its WIP limit and minimum. */
function laneLabel(t: ReturnType<typeof useT>, name: string, count: number, limit: number | undefined, min: number | undefined, under: boolean): string {
  if (under && min !== undefined) {
    return t("lane-label-under", { name, count, min });
  }
  if (min !== undefined && limit !== undefined) {
    return t("lane-label-wip-range", { name, count, min, limit });
  }
  if (min !== undefined) {
    return t("lane-label-wip-min", { name, count, min });
  }
  return limit === undefined ? t("lane-label", { name, count }) : t("lane-label-wip", { name, count, limit });
}

/** One board column: its header with the count and WIP limit, its cards, and the inline Add task field. */
export function Lane({ lane, keys, cards, handlers, active, closed, reason, hot, adding, menu, templates = [], onAdd, onCreate, onStopAdding, onShowOlder, onScroller }: LaneProps) {
  const t = useT();
  const { column } = lane;
  const limit = column.wip_limit;
  const min = column.wip_min;
  const over = limit !== undefined && lane.total > limit;
  const under = min !== undefined && lane.total < min;
  // A virtual lane hands over how to bring an index into view, for keyboard moves.
  const [scrollTo, setScrollTo] = useState<{ run: (index: number) => void } | null>(null);
  const data: LaneData = { kind: "lane", cards: keys, closed, scrollTo: scrollTo?.run };
  useEffect(() => {
    onScroller?.(scrollTo?.run ?? null);
    return () => {
      onScroller?.(null);
    };
  }, [onScroller, scrollTo]);
  const { setNodeRef } = useDroppable({ id: column.id, data, disabled: closed });
  const body = keys.length > VIRTUAL_OVER;
  const label = laneLabel(t, column.name, lane.total, limit, min, under);

  return (
    <section className={classes("lane", styles.lane, hot && !closed && styles.hot, closed && active !== null && styles.closedLane)} aria-label={label} data-lane={column.id}>
      <header className={styles.laneHeader}>
        <span className={`dot hue-${column.hue}`} />
        <h2 className={styles.laneName}>{column.name}</h2>
        <span className={classes("count", over && "over", under && "under")} data-testid="lane-count" title={under ? t("lane-under-minimum", { count: lane.total, min }) : undefined}>
          {laneCount(t, lane.total, limit, min)}
        </span>
        <div className="grow" />
        <Button className="btn flat icon sm" aria-label={t("lane-add", { name: column.name })} onPress={onAdd}>
          <Plus className="ic s" />
        </Button>
        <MenuTrigger>
          <Button className="btn flat icon sm" aria-label={t("lane-menu", { name: column.name })}>
            <Ellipsis className="ic s dots" />
          </Button>
          <Popover className="pop menuPopover" placement="bottom end" offset={4}>
            <Menu className="menu">
              <MenuEntries entries={menu} />
            </Menu>
          </Popover>
        </MenuTrigger>
      </header>
      {reason !== undefined && active !== null && (
        <p className={styles.laneRule} role="note" aria-label={t("lane-rule-closed", { name: column.name, reason })}>
          {reason}
        </p>
      )}
      <SortableContext id={column.id} items={[...keys]} strategy={verticalListSortingStrategy}>
        {body ? (
          <VirtualBody keys={keys} cards={cards} handlers={handlers} active={active} sortable={lane.sort === "manual"} closed={closed} setDroppable={setNodeRef} onScroller={setScrollTo}>
            <LaneFooter lane={lane} adding={adding} templates={templates} onAdd={onAdd} onCreate={onCreate} onStopAdding={onStopAdding} onShowOlder={onShowOlder} />
          </VirtualBody>
        ) : (
          <div ref={setNodeRef} className={styles.laneBody}>
            {keys.map((key) => {
              const card = cards.get(key);
              return card ? <SortableCard key={key} card={card} handlers={handlers} sortable={lane.sort === "manual"} dropDisabled={closed} /> : null;
            })}
            <LaneFooter lane={lane} adding={adding} templates={templates} onAdd={onAdd} onCreate={onCreate} onStopAdding={onStopAdding} onShowOlder={onShowOlder} />
          </div>
        )}
      </SortableContext>
    </section>
  );
}

function LaneFooter({ lane, adding, templates, onAdd, onCreate, onStopAdding, onShowOlder }: Pick<LaneProps, "lane" | "adding" | "templates" | "onAdd" | "onCreate" | "onStopAdding" | "onShowOlder">) {
  const t = useT();
  return (
    <>
      {lane.older > 0 && (
        <button type="button" className="add linkbtn" onClick={onShowOlder}>
          {t("lane-show-older", { count: lane.older })}
        </button>
      )}
      {adding ? (
        <AddTaskField name={lane.column.name} templates={templates ?? []} onCreate={onCreate} onStop={onStopAdding} />
      ) : (
        <button type="button" className="add" onClick={onAdd}>
          <Plus className="ic s" />
          <span>{t("lane-add-task")}</span>
        </button>
      )}
    </>
  );
}

/** No template: the Add task field's first choice. */
const NO_TEMPLATE = "none";

/**
 * The inline title field: Enter creates and stays open for the next task,
 * Escape closes. With task templates, a choice of one to start from; the
 * title may then stay empty for the template's own.
 */
function AddTaskField({ name, templates, onCreate, onStop }: { name: string; templates: readonly TemplateSummary[]; onCreate: (title: string, template: string | null) => void; onStop: () => void }) {
  const t = useT();
  const [title, setTitle] = useState("");
  const [template, setTemplate] = useState(NO_TEMPLATE);
  const wrapper = useRef<HTMLDivElement>(null);
  const chosen = template === NO_TEMPLATE ? null : template;
  return (
    <div
      ref={wrapper}
      className={styles.addTask}
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.defaultPrevented) {
          event.preventDefault();
          event.stopPropagation();
          onStop();
        }
      }}
    >
      <span className={classes("field", styles.addField)}>
        <input
          autoFocus
          aria-label={t("lane-new-task-title", { name })}
          placeholder={t("lane-new-task-placeholder")}
          value={title}
          onChange={(event) => {
            setTitle(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (title.trim() !== "" || chosen !== null)) {
              event.preventDefault();
              onCreate(title, chosen);
              setTitle("");
            }
          }}
          onBlur={(event) => {
            if (title.trim() === "" && !(event.relatedTarget instanceof Node && wrapper.current?.contains(event.relatedTarget) === true)) {
              onStop();
            }
          }}
        />
      </span>
      {templates.length > 0 && (
        <Select
          label={t("template-picker")}
          value={template}
          options={[{ id: NO_TEMPLATE, label: t("template-none") }, ...templates.map((each) => ({ id: each.id, label: each.name }))]}
          onChange={setTemplate}
        />
      )}
    </div>
  );
}

interface VirtualBodyProps {
  keys: readonly string[];
  cards: ReadonlyMap<string, Card>;
  handlers: CardHandlers;
  active: string | null;
  sortable: boolean;
  closed: boolean;
  setDroppable: (element: HTMLElement | null) => void;
  onScroller: (scroller: { run: (index: number) => void } | null) => void;
  children: ReactNode;
}

/** A long lane draws only the cards in view, plus the dragged one, which stays mounted while it scrolls away. */
function VirtualBody({ keys, cards, handlers, active, sortable, closed, setDroppable, onScroller, children }: VirtualBodyProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const dragged = active === null ? -1 : keys.indexOf(active);
  const virtualizer = useVirtualizer({
    count: keys.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 92,
    overscan: 6,
    getItemKey: (index) => keys[index] ?? index,
    rangeExtractor: (range) => keepMounted(range, dragged),
  });
  useEffect(() => {
    onScroller({
      run: (index) => {
        virtualizer.scrollToIndex(index, { align: "auto" });
      },
    });
    return () => {
      onScroller(null);
    };
  }, [onScroller, virtualizer]);
  return (
    <div
      ref={(element) => {
        scrollRef.current = element;
        setDroppable(element);
      }}
      className={styles.laneBody}
      data-virtual="true"
    >
      <div className={styles.virtualSpace} style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((item) => {
          const key = keys[item.index];
          const card = key === undefined ? undefined : cards.get(key);
          return (
            <div key={item.key} data-index={item.index} ref={virtualizer.measureElement} className={styles.virtualRow} style={{ transform: `translateY(${String(item.start)}px)` }}>
              {card && <SortableCard card={card} handlers={handlers} sortable={sortable} dropDisabled={closed} />}
            </div>
          );
        })}
      </div>
      {children}
    </div>
  );
}

interface CollapsedLaneProps {
  lane: LaneModel;
  onExpand: () => void;
}

/** A collapsed column: a narrow strip with its count and its name written upwards. */
export function CollapsedLane({ lane, onExpand }: CollapsedLaneProps) {
  const t = useT();
  return (
    <button type="button" className={classes("lane", styles.collapsed)} aria-label={t("lane-collapsed", { name: lane.column.name, count: lane.total })} onClick={onExpand} data-lane={lane.column.id}>
      <ChevronRight className="ic s muted" />
      <span className="count">{lane.total}</span>
      <span className={styles.collapsedName}>{lane.column.name}</span>
    </button>
  );
}
