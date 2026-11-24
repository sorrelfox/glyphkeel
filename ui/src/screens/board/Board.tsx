// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import {
  DndContext,
  DragOverlay,
  KeyboardCode,
  KeyboardSensor,
  MeasuringStrategy,
  PointerSensor,
  useSensor,
  useSensors,
  type Announcements,
  type DragEndEvent,
  type DragOverEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { arrayMove } from "@dnd-kit/sortable";
import { useNavigate } from "@tanstack/react-router";
import { useState, type KeyboardEvent } from "react";

import { keysOf, type Action } from "../../actions/actions";
import { ActionsProvider } from "../../actions/ActionsProvider";
import { isMenuKey, useContextMenu, type MenuEntry } from "../../components/ContextMenu";
import { useTrashTasks } from "../../data/safety";
import { useToasts } from "../../components/Toasts";
import { cardsOf, columnsOf, useBoard, useCreateTask, useLayout, useMoveTask, useProject, useUpdateLayout, useUpdateTask, type Move } from "../../data/board";
import { PRIORITIES, SORT_MODES, type Board as BoardData, type BoardLayout, type Card, type CardSize, type Place, type Project } from "../../data/catalog";
import { loadCardSize, saveCardSize } from "../../prefs/store";
import { useT } from "../../i18n/I18n";
import { useNewTask, useStatus, useTimerTask } from "../../shell/context";
import { CardView } from "./CardView";
import { indexCoordinates, pointerThenCenter, step } from "./dnd";
import { CollapsedLane, Lane, type CardHandlers } from "./Lane";
import { useToday } from "../../data/zone";
import { boardView, isFiltered, moveColumn, NO_FILTERS, positionIn, type BoardView, type Filters } from "./model";
import { Toolbar } from "./Toolbar";
import styles from "./Board.module.css";

interface BoardProps {
  projectKey: string;
  /** The task open in the panel. */
  openTask: string | undefined;
  /** A clock for tests. */
  now?: () => Date;
}

/** The board of a project: toolbar, filter banner and lanes. */
export function Board({ projectKey, openTask, now = () => new Date() }: BoardProps) {
  const project = useProject(projectKey);
  const layout = useLayout(projectKey);
  const include = layout.data?.layout.include_subprojects ?? false;
  // The columns whose older closed cards show; the first one asks the engine for them.
  const [showOlder, setShowOlder] = useState<ReadonlySet<string>>(new Set());
  const board = useBoard(projectKey, include, layout.isSuccess, showOlder.size > 0);
  if (!project.data || !layout.data || !board.data) {
    return <div className={styles.board} aria-busy="true" />;
  }
  return (
    <LoadedBoard
      project={project.data}
      layout={layout.data.layout}
      board={board.data}
      openTask={openTask}
      now={now}
      showOlder={showOlder}
      onShowOlder={(column) => {
        setShowOlder(new Set([...showOlder, column]));
      }}
    />
  );
}

interface LoadedBoardProps {
  project: Project;
  layout: BoardLayout;
  board: BoardData;
  openTask: string | undefined;
  now: () => Date;
  showOlder: ReadonlySet<string>;
  onShowOlder: (column: string) => void;
}

/** During a drag: the lanes as they will be after the drop. */
interface Drag {
  key: string;
  lanes: Record<string, string[]>;
  over: string | null;
  /** Started from the keyboard: its steps scroll their target into view themselves. */
  keyboard: boolean;
}

function LoadedBoard({ project, layout, board, openTask, now, showOlder, onShowOlder }: LoadedBoardProps) {
  const t = useT();
  const navigate = useNavigate();
  const toasts = useToasts();
  const [filters, setFilters] = useState<Filters>(NO_FILTERS);
  const [selected, setSelected] = useState<string | null>(openTask ?? null);
  const [adding, setAdding] = useState<string | null>(board.columns.every((column) => column.count === 0) ? (board.columns[0]?.id ?? null) : null);
  const [cardSize, setCardSize] = useState<CardSize>(() => loadCardSize(project.id));
  const [drag, setDrag] = useState<Drag | null>(null);
  const [note, setNote] = useState<string | undefined>(undefined);
  const updateLayout = useUpdateLayout(project.key);
  const move = useMoveTask(board, showOlder.size > 0);
  const create = useCreateTask();
  const update = useUpdateTask();
  const menu = useContextMenu(t("card-menu"));
  const trash = useTrashTasks();
  const today = useToday(now);
  const readOnly = project.state === "archived";

  const view: BoardView = boardView(board, layout, filters, now(), showOlder);
  const cards = cardsOf(board);
  const columnOf = columnsOf(board);
  const columnName = new Map(board.columns.map((column) => [column.id, column.name]));
  const lanesNow: Record<string, string[]> = Object.fromEntries(view.lanes.map((lane) => [lane.column.id, lane.cards.map((card) => card.key)]));
  const laneKeys = drag ? drag.lanes : lanesNow;
  const activeCard = drag ? cards.get(drag.key) : undefined;

  /** A lane can take the card: open, and its column has a status in the card's project. */
  const takes = (laneId: string, card: Card | undefined) => {
    const lane = view.lanes.find((candidate) => candidate.column.id === laneId);
    return lane !== undefined && !lane.collapsed && card !== undefined && lane.column.statuses[card.project] !== undefined;
  };
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, {
      keyboardCodes: { start: [KeyboardCode.Space], cancel: [KeyboardCode.Esc], end: [KeyboardCode.Space, KeyboardCode.Enter] },
      coordinateGetter: indexCoordinates,
      // When a step would take the card past the middle of its lane, the
      // sensor scrolls the lane under it instead. At once, so a key pressed
      // during a smooth scroll isn't measured against a moving lane.
      scrollBehavior: "auto",
    }),
  );

  const laneOf = (key: string, lanes: Record<string, string[]>) => Object.keys(lanes).find((id) => id === key || lanes[id]?.includes(key));

  const announce = (text: string) => {
    setNote(text);
  };

  /** Sends a move to the engine and says what happened. */
  const commit = (key: string, laneId: string, place: Place, ids: readonly string[]) => {
    const card = cards.get(key);
    const lane = view.lanes.find((candidate) => candidate.column.id === laneId);
    const status = card && lane?.column.statuses[card.project];
    if (!card || !lane || status === undefined) {
      toasts.show({ tone: "error", text: t("move-refused", { key, column: lane?.column.name ?? "" }) });
      return;
    }
    const sameColumn = columnOf[key] === laneId;
    const change: Move = { key, status, column: laneId, place };
    move.mutate(change);
    announce(
      sameColumn
        ? t("move-done-position", { key, column: lane.column.name, position: ids.indexOf(key) + 1 })
        : t("move-done", { key, column: lane.column.name }),
    );
  };

  // ---------- pointer and keyboard drags ----------

  const onDragStart = ({ active, activatorEvent }: DragStartEvent) => {
    const key = String(active.id);
    setSelected(key);
    setDrag({ key, lanes: structuredClone(lanesNow), over: laneOf(key, lanesNow) ?? null, keyboard: activatorEvent instanceof globalThis.KeyboardEvent });
  };

  const onDragOver = ({ active, over }: DragOverEvent) => {
    if (!over || !drag) {
      return;
    }
    const key = String(active.id);
    const from = laneOf(key, drag.lanes);
    const to = laneOf(String(over.id), drag.lanes);
    if (!from || !to || !takes(to, cards.get(key))) {
      return;
    }
    if (from === to) {
      if (drag.over !== to) {
        setDrag({ ...drag, over: to });
      }
      return;
    }
    const lanes = { ...drag.lanes, [from]: (drag.lanes[from] ?? []).filter((id) => id !== key) };
    const target = [...(lanes[to] ?? [])];
    const at = target.indexOf(String(over.id));
    target.splice(at < 0 ? target.length : at, 0, key);
    lanes[to] = target;
    setDrag({ ...drag, lanes, over: to });
  };

  const onDragEnd = ({ active, over }: DragEndEvent) => {
    const current = drag;
    setDrag(null);
    if (!over || !current) {
      return;
    }
    const key = String(active.id);
    const laneId = laneOf(key, current.lanes);
    if (!laneId || !takes(laneId, cards.get(key))) {
      return;
    }
    let ids = current.lanes[laneId] ?? [];
    const lane = view.lanes.find((candidate) => candidate.column.id === laneId);
    const manual = lane?.sort === "manual";
    if (manual) {
      const from = ids.indexOf(key);
      const to = ids.indexOf(String(over.id));
      if (to >= 0 && from !== to) {
        ids = arrayMove(ids, from, to);
      }
    }
    const moved = columnOf[key] !== laneId;
    const before = lanesNow[laneId] ?? [];
    if (!moved && ids.join() === before.join()) {
      return;
    }
    commit(key, laneId, manual ? positionIn(ids, key) : { at: "bottom" }, ids);
  };

  const announcements: Announcements = {
    onDragStart: ({ active }) => t("drag-start", { key: String(active.id) }),
    onDragOver: ({ active, over }) =>
      over ? t("drag-over", { key: String(active.id), column: columnName.get(laneOf(String(over.id), laneKeys) ?? "") ?? "" }) : undefined,
    onDragEnd: ({ active, over }) =>
      over ? t("drag-end", { key: String(active.id), column: columnName.get(laneOf(String(over.id), laneKeys) ?? "") ?? "" }) : t("drag-cancel", { key: String(active.id) }),
    onDragCancel: ({ active }) => t("drag-cancel", { key: String(active.id) }),
  };

  // ---------- keyboard: selection and Alt+arrow moves ----------

  // How each virtual lane brings one of its places into view (a long lane
  // draws only the cards near the screen, so a card moved far down isn't in
  // the page until its lane scrolls there).
  const [scrollers] = useState(() => new Map<string, (index: number) => void>());

  /** Selects a card and focuses it, once it is drawn: at `index` of `lane`, which scrolls there first. */
  const focusCard = (key: string, lane?: string, index?: number) => {
    setSelected(key);
    let tries = 0;
    const attempt = () => {
      // In its new lane: until the board draws the move, the card is still in the old one.
      const within = lane === undefined ? "" : `section[data-lane="${CSS.escape(lane)}"] `;
      const element = document.querySelector<HTMLElement>(`${within}[data-card="${CSS.escape(key)}"]`);
      if (element) {
        element.focus();
        return;
      }
      if (lane !== undefined && index !== undefined) {
        scrollers.get(lane)?.(index);
      }
      tries += 1;
      if (tries < 10) {
        requestAnimationFrame(attempt);
      }
    };
    requestAnimationFrame(attempt);
  };

  const moveSelection = (key: string, code: string) => {
    const lanes = view.lanes.map((lane) => ({ id: lane.column.id, cards: lanesNow[lane.column.id] ?? [], closed: lane.collapsed || (lanesNow[lane.column.id] ?? []).length === 0 }));
    const next = step(lanes, key, "", code);
    if (next && cards.has(String(next.target))) {
      focusCard(String(next.target), next.lane, next.index);
    }
  };

  const moveByKey = (key: string, code: string) => {
    const card = cards.get(key);
    const lanes = view.lanes.map((lane) => ({ id: lane.column.id, cards: lanesNow[lane.column.id] ?? [], closed: !takes(lane.column.id, card) }));
    const next = step(lanes, key, key, code);
    if (!next || readOnly) {
      return;
    }
    const lane = view.lanes.find((candidate) => candidate.column.id === next.lane);
    if (!lane) {
      return;
    }
    const sameLane = columnOf[key] === next.lane;
    if (sameLane && lane.sort !== "manual") {
      announce(t("move-sorted", { column: lane.column.name }));
      return;
    }
    const ids = (lanesNow[next.lane] ?? []).filter((id) => id !== key);
    ids.splice(next.index, 0, key);
    commit(key, next.lane, lane.sort === "manual" ? positionIn(ids, key) : { at: "bottom" }, ids);
    focusCard(key, next.lane, lane.sort === "manual" ? next.index : ids.length - 1);
  };

  const cardEntries = (key: string): MenuEntry[] => {
    const card = cards.get(key);
    if (!card) {
      return [];
    }
    return [
      { id: "open", label: t("card-open"), run: () => { openPanel(key); } },
      {
        id: "move",
        label: t("card-move-to"),
        submenu: view.lanes.map((lane) => ({
          id: `move-${lane.column.id}`,
          label: lane.column.name,
          disabled: readOnly || columnOf[key] === lane.column.id || lane.column.statuses[card.project] === undefined,
          run: () => { commit(key, lane.column.id, { at: "bottom" }, [...(lanesNow[lane.column.id] ?? []), key]); },
        })),
      },
      {
        id: "priority",
        label: t("card-priority"),
        submenu: [...PRIORITIES].reverse().map((priority) => ({
          id: `priority-${priority}`,
          label: t(`priority-${priority}`),
          checked: card.priority === priority,
          disabled: readOnly,
          run: () => { update.mutate({ key, priority }); },
        })),
      },
      "separator",
      {
        id: "copy-key",
        label: t("card-copy-key"),
        run: () => void navigator.clipboard.writeText(key).catch(() => undefined),
      },
      "separator",
      { id: "trash", label: t("trash-move", { count: 1 }), disabled: readOnly, run: () => { trashCard(key); } },
    ];
  };

  /** Moves a card's task to the trash; the panel closes when it showed it. */
  const trashCard = (key: string) => {
    trash([key], () => {
      if (openTask === key) {
        void navigate({ to: "/projects/$key", params: { key: project.key }, search: {} });
      }
    });
  };

  const openPanel = (key: string) => {
    setSelected(key);
    void navigate({ to: "/projects/$key", params: { key: project.key }, search: { task: key } });
  };

  const onCardKey = (key: string, event: KeyboardEvent<HTMLElement>): boolean => {
    if (drag) {
      return false;
    }
    const plain = !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
    if (plain && event.key === "Enter") {
      event.preventDefault();
      openPanel(key);
      return true;
    }
    if (plain && ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
      event.preventDefault();
      moveSelection(key, event.key);
      return true;
    }
    if (isMenuKey(event)) {
      event.preventDefault();
      menu.openAt(event.currentTarget, cardEntries(key));
      return true;
    }
    if (plain && event.key === "Delete" && !readOnly) {
      event.preventDefault();
      trashCard(key);
      return true;
    }
    return false;
  };

  const handlers: CardHandlers = {
    selected,
    fields: layout.card_fields,
    compact: cardSize === "compact",
    today,
    onSelect: setSelected,
    onOpen: openPanel,
    onKey: onCardKey,
    onMenu: (key, at) => {
      if (at instanceof Element) {
        menu.openAt(at, cardEntries(key));
      } else {
        menu.open(at, cardEntries(key));
      }
    },
  };

  // ---------- layout ----------

  const orderOf = () => view.lanes.map((lane) => lane.column.id).concat(view.hidden.map((hidden) => hidden.column.id));
  const laneMenu = (laneId: string, index: number): MenuEntry[] => {
    const lane = view.lanes.find((candidate) => candidate.column.id === laneId);
    if (!lane) {
      return [];
    }
    return [
      { id: "add", label: t("lane-add-task"), disabled: readOnly, run: () => { setAdding(laneId); } },
      { id: "hide", label: t("lane-hide"), run: () => { updateLayout({ hidden: [...layout.hidden, laneId] }); } },
      { id: "collapse", label: t("lane-collapse"), run: () => { updateLayout({ collapsed: [...layout.collapsed, laneId] }); } },
      "separator",
      { id: "left", label: t("lane-move-left"), disabled: index === 0, run: () => { updateLayout({ order: moveColumn(board.columns, orderOf(), laneId, -1) }); } },
      {
        id: "right",
        label: t("lane-move-right"),
        disabled: index === view.lanes.length - 1,
        run: () => { updateLayout({ order: moveColumn(board.columns, orderOf(), laneId, 1) }); },
      },
      "separator",
      {
        id: "sort",
        label: t("lane-sort"),
        submenu: SORT_MODES.map((mode) => ({
          id: `sort-${mode}`,
          label: t(`sort-${mode}`),
          checked: lane.sort === mode,
          run: () => { updateLayout({ column_sort: { ...layout.column_sort, [laneId]: mode } }); },
        })),
      },
    ];
  };

  const newTaskColumn = () => {
    const selectedColumn = selected ? columnOf[selected] : undefined;
    const open = view.lanes.filter((lane) => !lane.collapsed);
    return open.find((lane) => lane.column.id === selectedColumn)?.column.id ?? open.find((lane) => lane.column.category === "todo")?.column.id ?? open[0]?.column.id ?? null;
  };
  // `T` times the task open in the panel, or else the selected card.
  useTimerTask(readOnly ? null : (openTask ?? selected));
  useNewTask(() => {
    if (!readOnly) {
      setAdding(newTaskColumn());
    }
  });

  const createIn = (laneId: string, title: string) => {
    const column = view.lanes.find((lane) => lane.column.id === laneId)?.column;
    if (!column) {
      return;
    }
    const target = column.statuses[project.key] !== undefined ? project.key : Object.keys(column.statuses)[0];
    const status = target === undefined ? undefined : column.statuses[target];
    if (target === undefined || status === undefined) {
      return;
    }
    create.mutate(
      { project: target, title, status },
      {
        onSuccess: (change) => {
          setSelected(change.task.key);
          announce(t("task-created", { key: change.task.key, column: column.name }));
        },
      },
    );
  };

  const selectedCard = selected ? cards.get(selected) : undefined;
  useStatus({
    summary: isFiltered(filters)
      ? t("board-summary-filtered", { shown: view.shown, total: view.total })
      : t("board-summary", { total: view.total, open: view.open }),
    selection: selectedCard ? t("board-selected", { key: selectedCard.key }) : undefined,
    note,
  });

  const action = (id: string, label: string, run: () => void): Action => ({ id, label, group: "tasks", keys: keysOf(id), inText: false, run });
  const boardActions: Action[] = drag
    ? []
    : [
        action("card.moveLeft", "action-card-left", () => { if (selected) moveByKey(selected, KeyboardCode.Left); }),
        action("card.moveRight", "action-card-right", () => { if (selected) moveByKey(selected, KeyboardCode.Right); }),
        action("card.moveUp", "action-card-up", () => { if (selected) moveByKey(selected, KeyboardCode.Up); }),
        action("card.moveDown", "action-card-down", () => { if (selected) moveByKey(selected, KeyboardCode.Down); }),
        action("column.moveLeft", "action-column-left", () => {
          const column = selected ? columnOf[selected] : undefined;
          if (column) updateLayout({ order: moveColumn(board.columns, orderOf(), column, -1) });
        }),
        action("column.moveRight", "action-column-right", () => {
          const column = selected ? columnOf[selected] : undefined;
          if (column) updateLayout({ order: moveColumn(board.columns, orderOf(), column, 1) });
        }),
      ];

  return (
    <ActionsProvider actions={boardActions}>
      <div className={styles.board}>
        <Toolbar
          project={project}
          board={board}
          layout={layout}
          view={view}
          filters={filters}
          onFilters={setFilters}
          onLayout={updateLayout}
          cardSize={cardSize}
          onCardSize={(size) => {
            setCardSize(size);
            saveCardSize(project.id, size);
          }}
        />
        {isFiltered(filters) && (
          <div className="banner" role="status">
            <span>{t("board-filtered", { shown: view.shown, total: view.total })}</span>
            <button type="button" className="linkbtn" onClick={() => { setFilters(NO_FILTERS); }}>
              {t("board-clear-filter")}
            </button>
          </div>
        )}
        {readOnly && (
          <div className="banner" role="status">
            <span>{t("board-archived")}</span>
          </div>
        )}
        <DndContext
          sensors={readOnly ? [] : sensors}
          collisionDetection={pointerThenCenter}
          // Scrolling on its own near a lane's edge would slide the cards
          // under a card the keyboard holds still, changing its target.
          autoScroll={!drag?.keyboard}
          measuring={{ droppable: { strategy: MeasuringStrategy.Always } }}
          accessibility={{ announcements, screenReaderInstructions: { draggable: t("drag-instructions") } }}
          onDragStart={onDragStart}
          onDragOver={onDragOver}
          onDragEnd={onDragEnd}
          onDragCancel={() => {
            setDrag(null);
          }}
        >
          <div className={styles.lanes} role="group" aria-label={t("board-label", { name: project.name })}>
            {view.lanes.map((lane, index) =>
              lane.collapsed ? (
                <CollapsedLane
                  key={lane.column.id}
                  lane={lane}
                  onExpand={() => {
                    updateLayout({ collapsed: layout.collapsed.filter((id) => id !== lane.column.id) });
                  }}
                />
              ) : (
                <Lane
                  key={lane.column.id}
                  lane={lane}
                  keys={laneKeys[lane.column.id] ?? []}
                  cards={cards}
                  handlers={handlers}
                  active={drag?.key ?? null}
                  closed={drag !== null && !takes(lane.column.id, activeCard)}
                  hot={drag?.over === lane.column.id}
                  adding={adding === lane.column.id}
                  menu={laneMenu(lane.column.id, index)}
                  onAdd={() => {
                    if (!readOnly) {
                      setAdding(lane.column.id);
                    }
                  }}
                  onCreate={(title) => {
                    createIn(lane.column.id, title);
                  }}
                  onStopAdding={() => {
                    setAdding(null);
                  }}
                  onShowOlder={() => {
                    onShowOlder(lane.column.id);
                  }}
                  onScroller={(run) => {
                    if (run) {
                      scrollers.set(lane.column.id, run);
                    } else {
                      scrollers.delete(lane.column.id);
                    }
                  }}
                />
              ),
            )}
          </div>
          <DragOverlay dropAnimation={{ duration: 160 }}>
            {activeCard ? <CardView card={activeCard} fields={layout.card_fields} compact={cardSize === "compact"} today={today} lifted /> : null}
          </DragOverlay>
        </DndContext>
        {menu.menu}
      </div>
    </ActionsProvider>
  );
}
