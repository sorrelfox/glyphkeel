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
import { type Card, type CardField } from "../../data/catalog";
import { useT } from "../../i18n/I18n";
import { CardView } from "./CardView";
import { keepMounted, type LaneData } from "./dnd";
import { type Lane as LaneModel } from "./model";
import styles from "./Board.module.css";

/** A lane draws its cards directly up to this many, and virtually above. */
export const VIRTUAL_OVER = 150;

export interface CardHandlers {
  selected: string | null;
  fields: readonly CardField[];
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
  /** The dragged card is over this lane. */
  hot: boolean;
  adding: boolean;
  menu: readonly MenuEntry[];
  onAdd: () => void;
  onCreate: (title: string) => void;
  onStopAdding: () => void;
  onShowOlder: () => void;
}

/** One board column: its header with the count and WIP limit, its cards, and the inline Add task field. */
export function Lane({ lane, keys, cards, handlers, active, closed, hot, adding, menu, onAdd, onCreate, onStopAdding, onShowOlder }: LaneProps) {
  const t = useT();
  const { column } = lane;
  const limit = column.wip_limit;
  const over = limit !== null && lane.total > limit;
  // A virtual lane hands over how to bring an index into view, for keyboard moves.
  const [scrollTo, setScrollTo] = useState<{ run: (index: number) => void } | null>(null);
  const data: LaneData = { kind: "lane", cards: keys, closed, scrollTo: scrollTo?.run };
  const { setNodeRef } = useDroppable({ id: column.id, data, disabled: closed });
  const body = keys.length > VIRTUAL_OVER;
  const label = limit === null ? t("lane-label", { name: column.name, count: lane.total }) : t("lane-label-wip", { name: column.name, count: lane.total, limit });

  return (
    <section className={classes("lane", styles.lane, hot && !closed && styles.hot, closed && active !== null && styles.closedLane)} aria-label={label} data-lane={column.id}>
      <header className={styles.laneHeader}>
        <span className={`dot hue-${column.hue}`} />
        <h2 className={styles.laneName}>{column.name}</h2>
        <span className={classes("count", over && "over")} data-testid="lane-count">
          {limit === null ? lane.total : t("lane-wip", { count: lane.total, limit })}
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
      <SortableContext id={column.id} items={[...keys]} strategy={verticalListSortingStrategy}>
        {body ? (
          <VirtualBody keys={keys} cards={cards} handlers={handlers} active={active} sortable={lane.sort === "manual"} closed={closed} setDroppable={setNodeRef} onScroller={setScrollTo}>
            <LaneFooter lane={lane} adding={adding} onAdd={onAdd} onCreate={onCreate} onStopAdding={onStopAdding} onShowOlder={onShowOlder} />
          </VirtualBody>
        ) : (
          <div ref={setNodeRef} className={styles.laneBody}>
            {keys.map((key) => {
              const card = cards.get(key);
              return card ? <SortableCard key={key} card={card} handlers={handlers} sortable={lane.sort === "manual"} dropDisabled={closed} /> : null;
            })}
            <LaneFooter lane={lane} adding={adding} onAdd={onAdd} onCreate={onCreate} onStopAdding={onStopAdding} onShowOlder={onShowOlder} />
          </div>
        )}
      </SortableContext>
    </section>
  );
}

function LaneFooter({ lane, adding, onAdd, onCreate, onStopAdding, onShowOlder }: Pick<LaneProps, "lane" | "adding" | "onAdd" | "onCreate" | "onStopAdding" | "onShowOlder">) {
  const t = useT();
  return (
    <>
      {lane.older > 0 && (
        <button type="button" className="add linkbtn" onClick={onShowOlder}>
          {t("lane-show-older", { count: lane.older })}
        </button>
      )}
      {adding ? (
        <AddTaskField name={lane.column.name} onCreate={onCreate} onStop={onStopAdding} />
      ) : (
        <button type="button" className="add" onClick={onAdd}>
          <Plus className="ic s" />
          <span>{t("lane-add-task")}</span>
        </button>
      )}
    </>
  );
}

/** The inline title field: Enter creates and stays open for the next task, Escape closes. */
function AddTaskField({ name, onCreate, onStop }: { name: string; onCreate: (title: string) => void; onStop: () => void }) {
  const t = useT();
  const [title, setTitle] = useState("");
  return (
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
          if (event.key === "Enter" && title.trim() !== "") {
            event.preventDefault();
            onCreate(title);
            setTitle("");
          } else if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            onStop();
          }
        }}
        onBlur={() => {
          if (title.trim() === "") {
            onStop();
          }
        }}
      />
    </span>
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
