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
import { CalendarDays, ChevronDown, Flag, Folder, IterationCw, Rows3 } from "lucide-react";
import { useEffect, useState, type KeyboardEvent } from "react";

import { keysOf, type Action } from "../../actions/actions";
import { ActionsProvider } from "../../actions/ActionsProvider";
import { motionReduced } from "../../appearance/appearance";
import { classes } from "../../classes";
import { isMenuKey, useContextMenu, type MenuEntry } from "../../components/ContextMenu";
import { useToasts } from "../../components/Toasts";
import { cardsOf, columnsOf } from "../../data/board";
import { useFolders } from "../../data/folders";
import { acrossInput, useAcrossBoard, useMoveAcross } from "../../data/myWork";
import { fieldsOfCard } from "../../data/rules";
import { useEntryReason, useProjectsByKey } from "../../data/useRules";
import { useToday } from "../../data/zone";
import { type AcrossProject, type AcrossStatus, type BoardAcross, type Card, type Category, type LaneBy, type SavedView, type Swimlane } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { loadCollapsedLanes, saveCollapsedLanes } from "../../prefs/store";
import { useStatus } from "../../shell/context";
import { CardView } from "../board/CardView";
import { indexCoordinates, pointerThenCenter, step } from "../board/dnd";
import { Lane, type CardHandlers } from "../board/Lane";
import { boardView, NO_FILTERS } from "../board/model";
import { cellId, laneOfCard, parseCell, swimlaneView, ACROSS_LANE_KINDS } from "../board/swimlanes";
import { Swimlanes } from "../board/Swimlanes";
import { MenuButton } from "../board/Toolbar";
import { QueryInput } from "../list/QueryInput";
import { SaveControls, useViewCommands, ViewPicker } from "../list/ViewMenu";
import { joinQuery, landing, lanesOf, splitToggles, toBoard, TOGGLE_IDS, type Toggle } from "./model";
import boardStyles from "../board/Board.module.css";
import styles from "./MyWork.module.css";

/** The person's own collapsed lanes are kept under this name per view. */
const prefScope = (view: SavedView | undefined) => `my-work.${view?.id ?? "all"}`;

/** How long typing rests before the board reads the query. */
const TYPING_MS = 300;

const TOGGLE_ICONS: Record<Toggle, typeof Flag> = { week: CalendarDays, sprint: IterationCw, high: Flag };

interface MyWorkProps {
  /** The saved view shown, if any. */
  view: SavedView | undefined;
  views: readonly SavedView[];
  /** The task open in the panel. */
  openTask: string | undefined;
  onOpenTask: (key: string) => void;
  onChooseView: (view: SavedView | undefined) => void;
  /** A clock for tests. */
  now?: () => Date;
}

/**
 * My work: the toolbar (saved views, the query with completion, the quick
 * toggles, the folder and the swimlanes) over the board across projects.
 */
export function MyWork({ view, views, openTask, onOpenTask, onChooseView, now = () => new Date() }: MyWorkProps) {
  const t = useT();
  const start = splitToggles(view?.query ?? "");
  const [typed, setTyped] = useState(start.typed);
  const [toggles, setToggles] = useState<Toggle[]>(start.toggles);
  const [lanes, setLanes] = useState<LaneBy>(lanesOf(view?.group_by));
  const [folder, setFolder] = useState<string | undefined>(view?.folder);
  const [showOlder, setShowOlder] = useState(false);
  const query = joinQuery(typed, toggles);
  // The board follows the typing once it rests; a toggle at once.
  const [applied, setApplied] = useState(query);
  useEffect(() => {
    const timer = setTimeout(() => {
      setApplied(query);
    }, TYPING_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [query]);
  const input = acrossInput(view?.id, applied, lanes, folder, showOlder);
  const board = useAcrossBoard(input);
  const folders = useFolders();
  const commands = useViewCommands({
    project: undefined,
    kind: "board_across",
    views,
    active: view,
    setup: { query, sort: "", columns: [], groupBy: lanes },
    folder,
    onChoose: onChooseView,
    onSaved: onChooseView,
  });
  const changed = view === undefined ? query !== "" || lanes !== "project" || folder !== undefined : query !== view.query.trim() || lanes !== lanesOf(view.group_by) || folder !== view.folder;
  const folderName = folder ?? t("my-work-folder-all");

  return (
    <div className={boardStyles.board}>
      <div className={classes("toolbar", boardStyles.toolbar, styles.toolbar)} role="toolbar" aria-label={t("my-work-toolbar")}>
        <ViewPicker
          active={view}
          none={t("my-work-view-none")}
          entries={commands.entries.map((entry) => (typeof entry === "object" && entry.id === "all" && "run" in entry ? { ...entry, label: t("my-work-view-none") } : entry))}
        />
        <div className={styles.query}>
          <QueryInput
            project={undefined}
            value={typed}
            onChange={setTyped}
            label={t("my-work-query-label")}
            placeholder={t("my-work-query-placeholder")}
            onSubmit={() => {
              setApplied(query);
            }}
          />
        </div>
        {TOGGLE_IDS.map((id) => {
          const Icon = TOGGLE_ICONS[id];
          const on = toggles.includes(id);
          return (
            <button
              key={id}
              type="button"
              className={classes("btn", on ? "on" : "flat")}
              aria-pressed={on}
              data-toggle={id}
              title={t(`my-work-toggle-${id}`)}
              onClick={() => {
                const next = on ? toggles.filter((each) => each !== id) : [...toggles, id];
                setToggles(next);
                setApplied(joinQuery(typed, next));
              }}
            >
              <Icon className="ic" />
              <span className="hide-narrow">{t(`my-work-toggle-${id}`)}</span>
            </button>
          );
        })}
        <div className="grow" />
        <MenuButton
          label={t("my-work-folder-label", { folder: folderName })}
          on={folder !== undefined}
          entries={[
            { id: "folder-all", label: t("my-work-folder-all"), checked: folder === undefined, run: () => { setFolder(undefined); } },
            ...(folders.data?.items ?? []).map(
              (each): MenuEntry => ({
                id: `folder-${each.id}`,
                // The folders' names are the person's own words.
                label: each.name,
                checked: folder === each.name,
                run: () => {
                  setFolder(each.name);
                },
              }),
            ),
          ]}
        >
          <Folder className="ic" />
          <span className="hide-narrow">{folderName}</span>
          <ChevronDown className="ic s muted" />
        </MenuButton>
        <MenuButton
          label={t("board-lanes-label", { by: t(`lanes-by-${lanes}`) })}
          on={lanes !== "none"}
          entries={ACROSS_LANE_KINDS.map(
            (by): MenuEntry => ({
              id: `lanes-${by}`,
              label: t(`lanes-by-${by}`),
              checked: lanes === by,
              run: () => {
                setLanes(by);
              },
            }),
          )}
        >
          <Rows3 className="ic" />
          <span className="hide-narrow">{lanes === "none" ? t("board-lanes") : t(`lanes-by-${lanes}`)}</span>
          <ChevronDown className="ic s muted" />
        </MenuButton>
        <SaveControls changed={changed} active={view} onSave={commands.save} onSaveAs={commands.saveAs} />
      </div>
      {board.data ? (
        <AcrossBoard
          across={board.data}
          input={input}
          openTask={openTask}
          onOpenTask={onOpenTask}
          now={now}
          prefs={prefScope(view)}
          onShowOlder={() => {
            setShowOlder(true);
          }}
        />
      ) : (
        <div className={boardStyles.lanes} aria-busy="true" />
      )}
      {commands.dialog}
    </div>
  );
}

interface AcrossBoardProps {
  across: BoardAcross;
  input: ReturnType<typeof acrossInput>;
  openTask: string | undefined;
  onOpenTask: (key: string) => void;
  now: () => Date;
  /** Where the person's collapsed lanes are kept. */
  prefs: string;
  onShowOlder: () => void;
}

/** During a drag: the cells as they will be after the drop. */
interface Drag {
  key: string;
  lanes: Record<string, string[]>;
  over: string | null;
  keyboard: boolean;
}

/**
 * The columns (one per status category) and, with swimlanes, the lanes.
 * Dropping a card into a column moves it to its project's status of that
 * category; when the project has several, a small menu asks which. The
 * statuses' rules apply as on the project's own board.
 */
function AcrossBoard({ across, input, openTask, onOpenTask, now, prefs, onShowOlder }: AcrossBoardProps) {
  const t = useT();
  const toasts = useToasts();
  const today = useToday(now);
  const menu = useContextMenu(t("card-menu"));
  const move = useMoveAcross(input);
  const [selected, setSelected] = useState<string | null>(openTask ?? null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [note, setNote] = useState<string | undefined>(undefined);
  const board = toBoard(across);
  const columnName = (category: string) => t("my-work-column", { category });
  // The columns' names are the categories', in the person's language.
  const named = { ...board, columns: board.columns.map((column) => ({ ...column, name: columnName(column.category) })) };
  const view = boardView(named, named.layout, NO_FILTERS, now(), new Set());
  const cards = cardsOf(named);
  const columnOf = columnsOf(named);
  const projects = new Map<string, AcrossProject>(across.projects.map((project) => [project.key, project]));
  const withRules = useProjectsByKey(across.projects.map((project) => project.key));
  const entryReason = useEntryReason();
  const swimOn = across.lanes_by !== "none" && (across.lanes?.length ?? 0) > 0;
  const grouping = across.lanes_by;
  const [collapsed, setCollapsed] = useState<{ grouping: string; ids: string[] }>(() => ({ grouping, ids: loadCollapsedLanes(prefs, grouping) }));
  const closedLanes = collapsed.grouping === grouping ? collapsed.ids : loadCollapsedLanes(prefs, grouping);
  const swim = swimOn ? swimlaneView(across.lanes ?? [], view, closedLanes) : [];
  const lanesNow: Record<string, string[]> = swimOn
    ? Object.fromEntries(swim.flatMap((lane) => view.lanes.map((column) => [cellId(lane.lane.id, column.column.id), (lane.cells[column.column.id] ?? []).map((card) => card.key)] as const)))
    : Object.fromEntries(view.lanes.map((lane) => [lane.column.id, lane.cards.map((card) => card.key)]));
  const laneKeys = drag ? drag.lanes : lanesNow;
  const activeCard = drag ? cards.get(drag.key) : undefined;
  const columnAt = (target: string) => (swimOn ? (parseCell(target)?.column ?? target) : target);
  const laneAt = (target: string) => (swimOn ? parseCell(target)?.lane : undefined);
  const homeOf = (key: string) => {
    const card = cards.get(key);
    const column = columnOf[key] ?? "";
    return swimOn && card ? cellId(laneOfCard(card), column) : column;
  };
  const categoryOf = (column: string): Category | undefined => across.columns.find((each) => each.id === column)?.category;
  const laneName = (lane: Swimlane): string => {
    if (lane.empty) {
      return t("swimlane-empty", { by: across.lanes_by });
    }
    if (across.lanes_by === "priority") {
      return t(`priority-${lane.id}`);
    }
    if (lane.key !== undefined) {
      return t("swimlane-parent", { key: lane.key, title: lane.name });
    }
    return lane.name;
  };

  /** Why `card` can't go into `status` under the status's rules, if it can't. */
  const statusReason = (card: Card, status: string) => entryReason(withRules.get(card.project), status, fieldsOfCard(card));
  /** Why the card can't be dropped on `target`, if it can't. */
  const reasonAt = (target: string, card: Card | undefined): string | undefined => {
    if (card === undefined || homeOf(card.key) === target) {
      return undefined;
    }
    // Lanes group the cards; the board across projects changes only their
    // status, so a card stays in its lane (an empty reason: closed, unsaid).
    const lane = laneAt(target);
    if (lane !== undefined && lane !== laneOfCard(card)) {
      return "";
    }
    const column = columnAt(target);
    const category = categoryOf(column);
    if (category === undefined || category === categoryOf(columnOf[card.key] ?? "")) {
      return undefined;
    }
    const statuses = (projects.get(card.project)?.statuses ?? []).filter((status) => status.category === category);
    if (statuses.length === 0) {
      return t("my-work-move-refused", { key: card.key, column: columnName(category) });
    }
    // Refused only when every status there refuses it.
    const reasons = statuses.map((status) => statusReason(card, status.id));
    return reasons.every((reason) => reason !== undefined) ? reasons[0] : undefined;
  };
  const takes = (target: string, card: Card | undefined) => {
    if (card === undefined) {
      return false;
    }
    const lane = laneAt(target);
    if (swimOn && (lane === undefined || closedLanes.includes(lane))) {
      return false;
    }
    return reasonAt(target, card) === undefined;
  };

  /** Moves the card to `status`, and says so. */
  const moveTo = (card: Card, column: string, status: AcrossStatus) => {
    move.mutate({ key: card.key, column, status });
    setNote(t("my-work-move-done", { key: card.key, status: status.name }));
  };

  /** A drop into `target`: one status moves at once; several ask which, beside the card. */
  const commit = (key: string, target: string) => {
    const card = cards.get(key);
    if (!card) {
      return;
    }
    const column = columnAt(target);
    const to = categoryOf(column);
    if (to === undefined) {
      return;
    }
    const project = projects.get(card.project);
    const where = landing(card, project, categoryOf(columnOf[key] ?? ""), to);
    if (where.kind === "none") {
      if (statusesInProject(project, to).length === 0) {
        toasts.show({ tone: "error", text: t("my-work-move-refused", { key, column: columnName(to) }) });
      }
      return;
    }
    if (where.kind === "one") {
      const reason = statusReason(card, where.status.id);
      if (reason !== undefined) {
        toasts.show({ tone: "error", text: reason });
        return;
      }
      moveTo(card, column, where.status);
      return;
    }
    const entries: MenuEntry[] = [
      {
        id: "choose",
        section: t("my-work-choose", { key, project: project?.name ?? card.project }),
        items: where.statuses.map((status) => {
          const reason = statusReason(card, status.id);
          return {
            id: `status-${status.id}`,
            label: status.name,
            icon: <span className={`dot hue-${status.hue}`} />,
            disabled: reason !== undefined,
            ...(reason === undefined ? {} : { detail: reason }),
            run: () => {
              moveTo(card, column, status);
            },
          };
        }),
      },
    ];
    // Beside the card where it was dropped, once the board has drawn it.
    requestAnimationFrame(() => {
      const element = document.querySelector(`[data-card="${CSS.escape(key)}"]`);
      if (element) {
        menu.openAt(element, entries);
      }
    });
  };

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, {
      keyboardCodes: { start: [KeyboardCode.Space], cancel: [KeyboardCode.Esc], end: [KeyboardCode.Space, KeyboardCode.Enter] },
      coordinateGetter: indexCoordinates,
      scrollBehavior: "auto",
    }),
  );
  const laneOf = (key: string, lanes: Record<string, string[]>) => Object.keys(lanes).find((id) => id === key || lanes[id]?.includes(key));

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
    const lanes = { ...drag.lanes, [from]: (drag.lanes[from] ?? []).filter((id) => id !== key), [to]: [...(drag.lanes[to] ?? []), key] };
    setDrag({ ...drag, lanes, over: to });
  };
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    const current = drag;
    setDrag(null);
    if (!over || !current) {
      return;
    }
    const key = String(active.id);
    const target = laneOf(key, current.lanes);
    if (!target || homeOf(key) === target || !takes(target, cards.get(key))) {
      return;
    }
    commit(key, target);
  };
  const placeOf = (over: string) => {
    const target = laneOf(over, laneKeys) ?? "";
    const lane = laneAt(target);
    const swimlane = lane === undefined ? undefined : swim.find((each) => each.lane.id === lane)?.lane;
    const category = categoryOf(columnAt(target));
    return { column: category === undefined ? "" : columnName(category), ...(swimlane ? { lane: laneName(swimlane) } : {}) };
  };
  const announcements: Announcements = {
    onDragStart: ({ active }) => t("drag-start", { key: String(active.id) }),
    onDragOver: ({ active, over }) => {
      if (!over) {
        return undefined;
      }
      const place = placeOf(String(over.id));
      return t(place.lane === undefined ? "drag-over" : "swimlane-drag-over", { key: String(active.id), ...place });
    },
    onDragEnd: ({ active, over }) => {
      if (!over) {
        return t("drag-cancel", { key: String(active.id) });
      }
      const place = placeOf(String(over.id));
      return t(place.lane === undefined ? "drag-end" : "swimlane-drag-end", { key: String(active.id), ...place });
    },
    onDragCancel: ({ active }) => t("drag-cancel", { key: String(active.id) }),
  };

  // ---------- keyboard ----------

  const [scrollers] = useState(() => new Map<string, (index: number) => void>());
  const focusCard = (key: string) => {
    setSelected(key);
    let tries = 0;
    const attempt = () => {
      const element = document.querySelector<HTMLElement>(`[data-card="${CSS.escape(key)}"]`);
      if (element) {
        element.focus();
        return;
      }
      tries += 1;
      if (tries < 10) {
        requestAnimationFrame(attempt);
      }
    };
    requestAnimationFrame(attempt);
  };
  const targets = (closed: (id: string) => boolean) =>
    swimOn
      ? swim.flatMap((lane, row) =>
          view.lanes.map((column) => {
            const id = cellId(lane.lane.id, column.column.id);
            return { id, cards: lanesNow[id] ?? [], closed: lane.collapsed || closed(id), row, column: column.column.id };
          }),
        )
      : view.lanes.map((column) => ({ id: column.column.id, cards: lanesNow[column.column.id] ?? [], closed: closed(column.column.id) }));
  const moveSelection = (key: string, code: string) => {
    const next = step(
      targets((id) => (lanesNow[id] ?? []).length === 0),
      key,
      "",
      code,
    );
    if (next && cards.has(String(next.target))) {
      focusCard(String(next.target));
    }
  };
  /** Alt+Left and Alt+Right: the card into the column beside, in its own lane. */
  const moveByKey = (key: string, by: -1 | 1) => {
    const card = cards.get(key);
    const column = columnOf[key];
    if (!card || column === undefined) {
      return;
    }
    const at = view.lanes.findIndex((lane) => lane.column.id === column);
    const next = view.lanes[at + by];
    if (!next) {
      return;
    }
    const target = swimOn ? cellId(laneOfCard(card), next.column.id) : next.column.id;
    const reason = reasonAt(target, card);
    if (reason !== undefined) {
      toasts.show({ tone: "error", text: reason });
      return;
    }
    commit(key, target);
    focusCard(key);
  };

  const cardEntries = (key: string): MenuEntry[] => {
    const card = cards.get(key);
    if (!card) {
      return [];
    }
    const project = projects.get(card.project);
    return [
      { id: "open", label: t("card-open"), run: () => { onOpenTask(key); } },
      {
        id: "move",
        label: t("my-work-move-to"),
        // Every status of the card's own project, by column.
        submenu: view.lanes.map((lane) => ({
          id: `move-${lane.column.id}`,
          section: lane.column.name,
          items: statusesInProject(project, lane.column.category).map((status) => {
            const reason = statusReason(card, status.id);
            return {
              id: `move-${status.id}`,
              label: status.name,
              checked: card.status?.id === status.id,
              disabled: card.status?.id === status.id || reason !== undefined,
              ...(reason === undefined ? {} : { detail: reason }),
              run: () => {
                moveTo(card, lane.column.id, status);
              },
            };
          }),
        })),
      },
      "separator",
      { id: "copy-key", label: t("card-copy-key"), run: () => void navigator.clipboard.writeText(key).catch(() => undefined) },
    ];
  };

  const onCardKey = (key: string, event: KeyboardEvent<HTMLElement>): boolean => {
    if (drag) {
      return false;
    }
    const plain = !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
    if (plain && event.key === "Enter") {
      event.preventDefault();
      onOpenTask(key);
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
    return false;
  };

  const handlers: CardHandlers = {
    selected,
    fields: named.layout.card_fields,
    custom: [],
    compact: false,
    today,
    onSelect: setSelected,
    onOpen: (key) => {
      setSelected(key);
      onOpenTask(key);
    },
    onKey: onCardKey,
    onMenu: (key, at) => {
      if (at instanceof Element) {
        menu.openAt(at, cardEntries(key));
      } else {
        menu.open(at, cardEntries(key));
      }
    },
    projectOf: (card) => {
      const project = projects.get(card.project);
      return project && { name: project.name, hue: project.hue };
    },
  };

  const selectedCard = selected ? cards.get(selected) : undefined;
  useStatus({
    summary: t("my-work-summary", { total: view.total, open: view.open }),
    selection: selectedCard ? t("board-selected", { key: selectedCard.key }) : undefined,
    note,
  });
  const action = (id: string, label: string, run: () => void): Action => ({ id, label, group: "tasks", keys: keysOf(id), inText: false, run });
  const boardActions: Action[] = drag
    ? []
    : [
        action("card.moveLeft", "action-card-left", () => { if (selected) moveByKey(selected, -1); }),
        action("card.moveRight", "action-card-right", () => { if (selected) moveByKey(selected, 1); }),
      ];
  const empty = view.total === 0;
  // With swimlanes the Done column has no footer of its own: older closed cards are offered above the board.
  const older = across.columns.reduce((sum, column) => sum + column.older, 0);

  return (
    <ActionsProvider actions={boardActions}>
      {empty && (
        <div className="banner" role="status">
          <span>{t("my-work-empty")}</span>
        </div>
      )}
      {swimOn && older > 0 && (
        <div className={styles.older}>
          <button type="button" className="linkbtn" onClick={onShowOlder}>
            {t("my-work-show-older", { count: older })}
          </button>
        </div>
      )}
      <DndContext
        sensors={sensors}
        collisionDetection={pointerThenCenter}
        autoScroll={!drag?.keyboard}
        measuring={{ droppable: { strategy: MeasuringStrategy.WhileDragging } }}
        accessibility={{ announcements, screenReaderInstructions: { draggable: t(swimOn ? "swimlane-drag-instructions" : "drag-instructions") } }}
        onDragStart={onDragStart}
        onDragOver={onDragOver}
        onDragEnd={onDragEnd}
        onDragCancel={() => {
          setDrag(null);
        }}
      >
        {swimOn ? (
          <Swimlanes
            label={t("my-work-board-label")}
            columns={view.lanes}
            lanes={swim}
            keys={laneKeys}
            cards={cards}
            handlers={handlers}
            active={drag?.key ?? null}
            over={drag?.over ?? null}
            closed={(cell) => !takes(cell, activeCard) && homeOf(drag?.key ?? "") !== cell}
            reason={(cell) => (activeCard === undefined ? undefined : reasonAt(cell, activeCard) || undefined)}
            nameOf={laneName}
            onToggle={(lane) => {
              const ids = closedLanes.includes(lane) ? closedLanes.filter((id) => id !== lane) : [...closedLanes, lane];
              setCollapsed({ grouping, ids });
              saveCollapsedLanes(prefs, grouping, ids);
            }}
            columnMenu={() => []}
            onExpandColumn={() => undefined}
            adding={null}
            templates={[]}
            onCreate={() => undefined}
            onStopAdding={() => undefined}
            onScroller={(cell, run) => {
              if (run) {
                scrollers.set(cell, run);
              } else {
                scrollers.delete(cell);
              }
            }}
          />
        ) : (
          <div className={boardStyles.lanes} role="group" aria-label={t("my-work-board-label")}>
            {view.lanes.map((lane) => (
              <Lane
                key={lane.column.id}
                lane={lane}
                keys={laneKeys[lane.column.id] ?? []}
                cards={cards}
                handlers={handlers}
                active={drag?.key ?? null}
                closed={drag !== null && !takes(lane.column.id, activeCard) && homeOf(drag.key) !== lane.column.id}
                reason={drag === null || activeCard === undefined ? undefined : reasonAt(lane.column.id, activeCard) || undefined}
                hot={drag?.over === lane.column.id}
                adding={false}
                menu={[]}
                onCreate={() => undefined}
                onStopAdding={() => undefined}
                onShowOlder={onShowOlder}
              />
            ))}
          </div>
        )}
        <DragOverlay dropAnimation={motionReduced() ? null : { duration: 160 }}>
          {activeCard ? <CardView card={activeCard} fields={named.layout.card_fields} compact={false} today={today} lifted project={handlers.projectOf?.(activeCard)} /> : null}
        </DragOverlay>
      </DndContext>
      {menu.menu}
    </ActionsProvider>
  );
}

function statusesInProject(project: AcrossProject | undefined, category: Category): AcrossStatus[] {
  return (project?.statuses ?? []).filter((status) => status.category === category);
}
