// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useLocalization } from "@fluent/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronDown, Columns3, Download, Filter, IterationCw } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover } from "react-aria-components";

import { useTrashTasks } from "../../data/safety";
import { useCreateTask } from "../../data/board";
import { groupingField, useGroupText } from "./GroupName";
import { useAppearance } from "../../appearance/AppearanceProvider";
import { classes } from "../../classes";
import { useAskCloseDuplicates } from "../../components/DuplicatePrompt";
import { Kbd } from "../../components/Kbd";
import { QueryField, type QueryFieldHandle, type Reading } from "../../components/QueryField/QueryField";
import { useToasts } from "../../components/Toasts";
import { Switch } from "../../components/Switch";
import { keysOf } from "../../actions/actions";
import { errorToast, failureText, useCall, useCommand } from "../../data/hooks";
import { allRows, defaultView, LIST_PAGE, toCsv, toJson, useListLayout, useListNames, useListRows, useParsedQuery, useQueryFields, useSavedViews, useUnfilteredCount, type ListScope } from "../../data/list";
import { joinable, useSprints } from "../../data/sprints";
import { useDurationUnits } from "../../data/time";
import { type CustomField, type ListedTask, type ListGroup, type ListLayout, type QueryChip, type QueryProblem, type SavedView } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { saveExport } from "../../ipc/exportFile";
import { useStatus, useTimerTask } from "../../shell/context";
import { isoDay } from "../board/model";
import { BulkBar } from "./BulkBar";
import { CellContext, type CellActions } from "./cells";
import { Chips } from "./Chips";
import { SaveControls, useViewCommands, ViewPicker } from "./ViewMenu";
import { ListTable } from "./ListTable";
import { changedFrom, clickRow, COLUMNS, exportName, FIXED_COLUMNS, GROUPINGS, GROUPING_FIELD_KINDS, groupTaskInput, isCustomColumn, isGrouping, listLines, moveColumn, nextSort, NO_SELECTION, removeRanges, viewColumns, withSort, withSprint, type ColumnId, type Grouping, type Selection } from "./model";
import { useCustomFields } from "../../data/fields";
import styles from "./List.module.css";

/** How long typing pauses before the query runs. */
const RUN_DELAY_MS = 250;

/** Row heights of the two densities, from the design. */
const ROW_HEIGHT = { regular: 32, compact: 28 } as const;

interface ListViewProps {
  /** The project; none for every task of the workspace. */
  project: string | undefined;
  /** The task open in the panel beside the list. */
  openTask: string | undefined;
  onOpen: (key: string) => void;
  /** A query to open with instead of the default view's (from a link elsewhere, such as the Gantt's tasks without dates). */
  initialQuery?: string | undefined;
}

/**
 * The list: saved views and the query field on top, its conditions as
 * chips, the matching tasks in a virtualized table with totals, and the bulk
 * bar over a selection. The list shows nothing until its saved views and
 * the person's layout of it are read, so the default view, in the person's
 * columns, is the first thing drawn.
 */
export function ListView({ project, openTask, onOpen, initialQuery }: ListViewProps) {
  const saved = useSavedViews(project);
  const own = useListLayout(project);
  if (!saved.views || (!own.layout && !own.error)) {
    return <div className={styles.screen} aria-busy="true" />;
  }
  return <LoadedList key={project ?? ""} project={project} views={saved.views} layout={own.layout?.layout ?? { columns: [], widths: {} }} openTask={openTask} onOpen={onOpen} initialQuery={initialQuery} />;
}

/** A group's key among the folded ones, as the engine keeps it: its value, or an empty text for the tasks without one. */
const groupKey = (group: ListGroup) => group.value ?? "";

/** The engine's message for a query mistake, in the person's language. */
function useProblemText(): (problem: QueryProblem) => string {
  const { l10n } = useLocalization();
  return (problem) => (l10n.getBundle(problem.message_id) ? l10n.getString(problem.message_id, problem.args ?? null) : problem.message);
}

interface LoadedListProps {
  project: string | undefined;
  views: SavedView[];
  /** The person's own layout of this list, as read when it opened. */
  layout: ListLayout;
  openTask: string | undefined;
  onOpen: (key: string) => void;
  initialQuery?: string | undefined;
}

function LoadedList({ project, views, layout, openTask, onOpen, initialQuery }: LoadedListProps) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const units = useDurationUnits();
  const problemText = useProblemText();
  const workspaceWide = project === undefined;
  const initial = defaultView(views, project);
  const customFields = useCustomFields(project);

  // What the list shows.
  const [activeId, setActiveId] = useState<string | undefined>(initialQuery === undefined ? initial?.id : undefined);
  const active = views.find((view) => view.id === activeId);
  const [text, setText] = useState(initialQuery ?? initial?.query ?? "");
  const [sort, setSort] = useState(initial?.sort ?? "");
  // The person's own columns and widths for this list, kept by the engine.
  const [ownColumns, setOwnColumns] = useState<readonly string[]>(layout.columns);
  const [widths, setWidths] = useState<Readonly<Record<string, number>>>(layout.widths);
  const [columns, setColumns] = useState<ColumnId[]>(() => viewColumns(initial, workspaceWide, layout.columns));
  const [groupBy, setGroupBy] = useState<Grouping | undefined>(isGrouping(initial?.group_by) ? initial.group_by : undefined);
  // The groups the person folded away, per grouping, kept by the engine with the layout.
  const [folded, setFolded] = useState<Readonly<Record<string, readonly string[]>>>(layout.collapsed ?? {});
  const collapsed: ReadonlySet<string> = new Set(groupBy === undefined ? [] : (folded[groupBy] ?? []));
  const [includeSubprojects, setIncludeSubprojects] = useState(false);
  const [selection, setSelection] = useState<Selection>(NO_SELECTION);
  const [focused, setFocused] = useState(0);
  const [editing, setEditing] = useState<CellActions["editing"]>(null);

  // The query that runs: the text once the engine reads it without a mistake.
  const parsed = useParsedQuery(text);
  const reading = parsed.data && !parsed.isPlaceholderData ? parsed.data : undefined;
  const [running, setRunning] = useState(initialQuery ?? initial?.query ?? "");
  // Typing pauses briefly before the query runs, so half-typed words don't empty the list.
  const valid = reading?.valid === true;
  useEffect(() => {
    if (!valid) {
      return;
    }
    const timer = setTimeout(() => {
      setRunning(text);
    }, RUN_DELAY_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [text, valid]);
  const problem = reading?.error;

  const scope: ListScope = { project, includeSubprojects, query: running, sort, groupBy };
  const scrollRef = useRef<HTMLDivElement>(null);
  const { density } = useAppearance().resolved;
  const rowHeight = ROW_HEIGHT[density];
  // Lines: the rows and, when grouped, a header per group.
  const [lineCount, setLineCount] = useState(0);
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer returns new functions each render, so React Compiler rightly leaves this component unmemoized
  const virtualizer = useVirtualizer({
    count: lineCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 6,
  });
  const items = virtualizer.getVirtualItems();
  const isCollapsed = (group: ListGroup) => collapsed.has(groupKey(group));
  // Each open group ends with a "+" line that adds a task with its value,
  // where there is a project to put it in.
  const adds = groupBy !== undefined && (project !== undefined || groupBy === "project" || groupBy === "parent");
  const groupField = groupingField(groupBy, customFields);
  const linesOf = (groups: readonly ListGroup[] | undefined, total: number) =>
    listLines(
      total,
      groups,
      (index) => {
        const group = groups?.[index];
        return group !== undefined && isCollapsed(group);
      },
      adds,
    );
  const rows = useListRows(scope, (groups, total) => linesOf(groups, total).rowsIn(items[0]?.index ?? 0, items.at(-1)?.index ?? LIST_PAGE - 1));
  const lines = linesOf(rows.groups, rows.total ?? 0);
  if (rows.total !== undefined && lines.count !== lineCount) {
    setLineCount(lines.count);
  }
  useEffect(() => {
    virtualizer.measure();
  }, [virtualizer, rowHeight]);

  const unfiltered = useUnfilteredCount({ project, includeSubprojects });
  const names = useListNames(project);
  const trash = useTrashTasks();
  const fields = useQueryFields();
  const field = useRef<QueryFieldHandle>(null);

  // Commands of the cells.
  const update = useCommand("task.update");
  const keepLayout = useCommand("list.layout.update");
  const move = useCommand("task.move");
  const askDuplicates = useAskCloseDuplicates();
  const cellActions: CellActions = {
    today: isoDay(new Date()),
    hueOf: (task) => names.hues.get(task.status.id) ?? "grey",
    editing,
    setEditing,
    rename: (task, title) => {
      update.mutate({ key: task.key, title, expected_version: task.version });
    },
    setStatus: (task, status, closing) => {
      void askDuplicates(task.key, closing && task.closed_at === undefined).then((close) => {
        move.mutate({ key: task.key, status, ...(close === undefined ? {} : { close_duplicates: close }) });
      });
    },
    setPriority: (task, priority) => {
      update.mutate({ key: task.key, priority });
    },
    setDue: (task, day) => {
      update.mutate(day === null ? { key: task.key, clear: ["due_date"] } : { key: task.key, due_date: day });
    },
  };

  // The header shows an order someone chose, not the default one.
  // The query's own `sort:` wins over the one chosen in the headers; that one shows at once.
  const shownSort = /(^|[\s(])sort:/.test(running) ? rows.sort : sort === "" ? [] : sort.split(",");
  const setup = { query: text, sort, columns, groupBy: groupBy ?? "" };
  const changed = changedFrom(active, setup, viewColumns(active, workspaceWide, ownColumns));
  const selectedKeys = [...selection.keys];
  const allSelected = (rows.total ?? 0) > 0 && selection.keys.size === rows.total;

  const choose = (view: SavedView | undefined) => {
    setActiveId(view?.id);
    setText(view?.query ?? "");
    setRunning(view?.query ?? "");
    setSort(view?.sort ?? "");
    setColumns(viewColumns(view, workspaceWide, ownColumns));
    setGroupBy(isGrouping(view?.group_by) ? view.group_by : undefined);
    setSelection(NO_SELECTION);
  };

  /**
   * Shows other columns, or the same in another order. The person's own
   * layout keeps them unless the view names its columns: then Save keeps
   * them in the view.
   */
  const changeColumns = (next: ColumnId[]) => {
    setColumns(next);
    if ((active?.columns ?? []).length === 0) {
      setOwnColumns(next);
      keepLayout.mutate({ ...(project === undefined ? {} : { project }), columns: next });
    }
  };

  const resize = (column: ColumnId, width: number) => {
    setWidths({ ...widths, [column]: width });
    keepLayout.mutate({ ...(project === undefined ? {} : { project }), widths: { [column]: width } });
  };
  const resetWidth = (column: ColumnId) => {
    setWidths(Object.fromEntries(Object.entries(widths).filter(([name]) => name !== column)));
    keepLayout.mutate({ ...(project === undefined ? {} : { project }), clear_widths: [column] });
  };

  const group = (next: Grouping | undefined) => {
    setGroupBy(next);
  };

  const createTask = useCreateTask();
  const groupText = useGroupText(customFields);
  /** Adds a task with a group's value, from the "+" line at the group's end. */
  const addToGroup = async (target: ListGroup, title: string) => {
    const input = groupBy === undefined ? undefined : groupTaskInput(groupBy, target, project, title, groupField);
    if (input === undefined) {
      return;
    }
    const change = await createTask.mutateAsync(input);
    toasts.show({ tone: "info", text: t("list-group-added", { key: change.task.key, group: groupBy === undefined ? "" : groupText(groupBy, target) }) });
  };

  /** Folds a group away or opens it again, and keeps that for the person. */
  const toggleGroup = (toggled: ListGroup) => {
    if (groupBy === undefined) {
      return;
    }
    const next = new Set(collapsed);
    if (!next.delete(groupKey(toggled))) {
      next.add(groupKey(toggled));
    }
    setFolded({ ...folded, [groupBy]: [...next] });
    keepLayout.mutate({ ...(project === undefined ? {} : { project }), collapsed: { [groupBy]: [...next] } });
  };

  const viewCommands = useViewCommands({
    project,
    views,
    active,
    setup,
    onChoose: choose,
    onSaved: (view) => {
      setActiveId(view?.id);
      if (!view) {
        choose(undefined);
      }
    },
  });

  const sortBy = (column: ColumnId, add: boolean) => {
    const next = nextSort(shownSort, column, add);
    if (reading && reading.tokens.some((token) => token.kind === "sort")) {
      setText(withSort(text, reading.tokens, next));
    } else {
      setSort(next.join(","));
    }
  };

  const toggleAll = () => {
    if (allSelected || selection.keys.size > 0) {
      setSelection(NO_SELECTION);
      return;
    }
    // Every matching task, not only the rows read so far.
    allRows(call, scope).then(
      (tasks) => {
        setSelection({ keys: new Set(tasks.map((task) => task.key)), anchor: null });
      },
      (error: unknown) => {
        toasts.show(errorToast(t, error));
      },
    );
  };

  const exportAs = (format: "csv" | "json") => {
    allRows(call, scope)
      .then((tasks: ListedTask[]) => {
        const contents = format === "csv" ? toCsv(tasks, units, customFields.map((field) => field.query_name)) : toJson(tasks, scope);
        return saveExport(exportName(active?.name, isoDay(new Date()), format), contents, format === "csv" ? "text/csv" : "application/json").then((done) => {
          if (done) {
            toasts.show({ tone: "info", text: t("list-exported", { count: tasks.length }) });
          }
        });
      })
      .catch((error: unknown) => {
        toasts.show(errorToast(t, error));
      });
  };

  /** The row `step` shown rows away from row `index` (or from line `start`), past group headers and collapsed groups. */
  const rowAway = (index: number, step: number, start = lines.lineOf(index) ?? -1): number => {
    let line = start;
    let found = index;
    for (let left = Math.abs(step); left > 0; ) {
      line += Math.sign(step);
      const at = lines.at(line);
      if (!at) {
        break;
      }
      if (at.kind === "row") {
        found = at.row;
        left -= 1;
      }
    }
    return found;
  };

  const focusRow = (index: number) => {
    const count = rows.total ?? 0;
    if (count === 0) {
      return;
    }
    const next = Math.max(0, Math.min(count - 1, index));
    const line = lines.lineOf(next);
    setFocused(next);
    if (line === undefined) {
      return;
    }
    virtualizer.scrollToIndex(line, { align: "auto" });
    requestAnimationFrame(() => {
      scrollRef.current?.querySelector<HTMLElement>(`[data-index="${String(next)}"]`)?.focus();
    });
  };

  const onKey = (event: KeyboardEvent, index: number) => {
    const task = rows.rowAt(index);
    const page = Math.max(1, Math.floor((scrollRef.current?.clientHeight ?? rowHeight * 10) / rowHeight) - 1);
    const moves: Record<string, number> = { ArrowDown: 1, ArrowUp: -1, PageDown: page, PageUp: -page };
    const step = moves[event.key];
    if (step !== undefined) {
      event.preventDefault();
      const next = rowAway(index, step);
      const key = rows.rowAt(next)?.key;
      if (key !== undefined) {
        setSelection(clickRow(selection, next, key, { toggle: false, range: event.shiftKey }, (at) => rows.rowAt(at)?.key));
      }
      focusRow(next);
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      // The first or last row shown: from outside the list, one step in.
      focusRow(event.key === "Home" ? rowAway(index, 1, -1) : rowAway(index, -1, lines.count));
    } else if (event.key === "Enter" && task) {
      event.preventDefault();
      event.stopPropagation();
      onOpen(task.key);
    } else if (event.key === " " && task) {
      event.preventDefault();
      event.stopPropagation();
      setSelection(clickRow(selection, index, task.key, { toggle: true, range: false }, (at) => rows.rowAt(at)?.key));
    } else if (event.key === "Delete" && !event.altKey && !event.ctrlKey && !event.metaKey && (selection.keys.size > 0 || task)) {
      event.preventDefault();
      event.stopPropagation();
      trash(selection.keys.size > 0 ? [...selection.keys] : task ? [task.key] : [], () => {
        setSelection(NO_SELECTION);
      });
    } else if (event.key === "F2" && task) {
      event.preventDefault();
      setEditing({ key: task.key, column: "title" });
    } else if (event.key === "Escape" && selection.keys.size > 0) {
      event.preventDefault();
      event.stopPropagation();
      setSelection(NO_SELECTION);
    } else if (event.key === "a" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      toggleAll();
    }
  };

  // New results replace the rows (each row is its task), so the row that had
  // the focus can leave the page and take the focus with it. The focus then
  // stays in the list, on the row at the same place.
  const lastRow = useRef<Element | null>(null);
  useEffect(() => {
    const onFocusIn = (event: FocusEvent) => {
      lastRow.current = event.target instanceof Element ? event.target.closest("[data-list-scroll] [data-index]") : null;
    };
    document.addEventListener("focusin", onFocusIn);
    return () => {
      document.removeEventListener("focusin", onFocusIn);
    };
  }, []);
  useEffect(() => {
    const row = lastRow.current;
    if (row && !row.isConnected && (document.activeElement === null || document.activeElement === document.body)) {
      lastRow.current = null;
      focusRow(focused);
    }
  });

  // Ctrl+F puts the cursor in the query field.
  useEffect(() => {
    const [chord] = keysOf("view.filter");
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (chord && event.ctrlKey && !event.shiftKey && !event.altKey && event.key.toLowerCase() === "f") {
        event.preventDefault();
        field.current?.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  // The status bar: counts and the selection.
  const count = rows.total ?? 0;
  // A query that leaves tasks out is a filter: tasks never vanish without a trace.
  const filtered = unfiltered !== undefined && rows.total !== undefined && count < unfiltered;
  useStatus({
    summary: filtered ? t("list-status-filtered", { count, total: unfiltered }) : t("status-tasks", { count }),
    selection: selection.keys.size > 0 ? t("list-selected", { count: selection.keys.size }) : undefined,
  });
  useTimerTask(openTask ?? (selection.keys.size === 1 ? (selectedKeys[0] ?? null) : null));

  const queryReading: Reading | undefined = reading ? { text, tokens: reading.tokens, error: reading.error } : undefined;
  const filterChord = keysOf("view.filter")[0];

  return (
    <CellContext value={cellActions}>
      <div className={styles.screen} data-list={project ?? "all"}>
        <div className={classes("toolbar", styles.toolbar)}>
          <ViewPicker active={active} entries={viewCommands.entries} />
          <QueryField
            ref={field}
            value={text}
            onChange={(next) => {
              setText(next);
            }}
            reading={queryReading}
            problem={problem ? problemText(problem) : undefined}
            fields={fields}
            names={names}
            label={t("list-query-label")}
            placeholder={t("list-query-placeholder")}
            onSubmit={() => {
              focusRow(focused);
            }}
            onEscape={() => {
              focusRow(focused);
            }}
            shortcut={filterChord && <Kbd chord={filterChord} />}
          />
          {project !== undefined && (
            <div className={styles.toolbarItem}>
              <Switch isSelected={includeSubprojects} onChange={setIncludeSubprojects}>
                {t("list-include-subprojects")}
              </Switch>
            </div>
          )}
          {project !== undefined && (
            <SprintMenu
              project={project}
              chips={parsed.data?.valid === true && !parsed.data.advanced ? parsed.data.chips : undefined}
              onChange={(value) => {
                setText(withSprint(text, parsed.data?.chips ?? [], value));
              }}
            />
          )}
          <GroupMenu value={groupBy} withProject={workspaceWide || includeSubprojects} fields={customFields} onChange={group} />
          <ColumnMenu columns={columns} fields={customFields} onChange={changeColumns} />
          <MenuTrigger>
            <Button className="btn flat">
              <Download className="ic" aria-hidden="true" />
              <span className="hide-narrow">{t("list-export")}</span>
            </Button>
            <Popover className="pop menuPopover" placement="bottom end" offset={4}>
              <Menu
                className="menu"
                aria-label={t("list-export")}
                onAction={(id) => {
                  exportAs(id === "json" ? "json" : "csv");
                }}
              >
                <MenuItem id="csv" className="item">
                  {t("list-export-csv")}
                </MenuItem>
                <MenuItem id="json" className="item">
                  {t("list-export-json")}
                </MenuItem>
              </Menu>
            </Popover>
          </MenuTrigger>
        </div>
        <div className={styles.conditions}>
          <Chips
            chips={parsed.data?.valid === true ? parsed.data.chips : undefined}
            advanced={parsed.data?.advanced ?? false}
            onRemove={(chip) => {
              setText(removeRanges(text, chip.ranges));
            }}
            onAdd={() => field.current?.addCondition()}
          />
          <div className="grow" />
          <SaveControls changed={changed} active={active} onSave={viewCommands.save} onSaveAs={viewCommands.saveAs} />
        </div>
        {filtered && (
          <div className="banner" role="status" data-testid="list-banner">
            <Filter className="ic s accent" aria-hidden="true" />
            <span>{t("list-filtered", { count, total: unfiltered })}</span>
            <button
              type="button"
              className="linkbtn"
              onClick={() => {
                setText("");
                setRunning("");
              }}
            >
              {t("list-clear-filter")}
            </button>
          </div>
        )}
        {rows.error && (
          <div className="banner" role="alert">
            {failureText(t, rows.error)}
          </div>
        )}
        {selection.keys.size > 0 && (
          <BulkBar
            keys={selectedKeys}
            names={names}
            fields={customFields}
            onClear={() => {
              setSelection(NO_SELECTION);
            }}
          />
        )}
        {rows.total === 0 ? (
          <div className={styles.empty}>
            <p className="muted">{t(running.trim() === "" ? "list-empty" : "list-empty-filtered")}</p>
          </div>
        ) : (
          <ListTable
            rows={rows}
            columns={columns}
            fields={customFields}
            widths={widths}
            onResize={resize}
            onResetWidth={resetWidth}
            onMove={(column, to) => {
              changeColumns(moveColumn(columns, column, to));
            }}
            lines={lines}
            grouping={groupBy}
            isCollapsed={isCollapsed}
            onToggleGroup={toggleGroup}
            onAddToGroup={adds ? addToGroup : undefined}
            sort={shownSort}
            onSort={sortBy}
            selection={selection}
            onRowClick={(index, task, mods) => {
              setFocused(index);
              setSelection(clickRow(selection, index, task.key, mods, (at) => rows.rowAt(at)?.key));
            }}
            onToggleAll={toggleAll}
            allSelected={allSelected}
            onOpen={onOpen}
            focused={focused}
            onFocusRow={setFocused}
            onKey={onKey}
            items={items}
            totalSize={virtualizer.getTotalSize()}
            scrollRef={scrollRef}
          />
        )}
        {viewCommands.dialog}
      </div>
    </CellContext>
  );
}

/** "Group: Status": how the rows are grouped, or not at all. */
function GroupMenu({ value, withProject, fields, onChange }: { value: Grouping | undefined; withProject: boolean; fields: readonly CustomField[]; onChange: (grouping: Grouping | undefined) => void }) {
  const t = useT();
  // Custom fields of a kind a list groups by: their names are the person's own words.
  const custom = [...new Map(fields.filter((field) => GROUPING_FIELD_KINDS.has(field.kind)).map((field) => [field.query_name, field] as const)).values()];
  const choices: Grouping[] = [...GROUPINGS.filter((grouping) => grouping !== "project" || withProject), ...custom.map((field) => field.query_name as Grouping)];
  const name = (grouping: Grouping | undefined) =>
    grouping === undefined ? t("list-group-by-none") : grouping.startsWith("cf.") ? (custom.find((field) => field.query_name === grouping)?.name ?? grouping) : t(`list-group-by-${grouping}`);
  return (
    <MenuTrigger>
      <Button className="btn flat" aria-label={t("list-group-button", { grouping: name(value) })}>
        <span className="muted hide-narrow">{t("list-group")}</span>{" "}
        <span>{name(value)}</span>
        <ChevronDown className="ic s muted" aria-hidden="true" />
      </Button>
      <Popover className="pop menuPopover" placement="bottom end" offset={4}>
        <Menu
          className="menu"
          aria-label={t("list-group")}
          selectionMode="single"
          selectedKeys={[value ?? "none"]}
          onAction={(id) => {
            const chosen = String(id);
            onChange(isGrouping(chosen) ? chosen : undefined);
          }}
        >
          {[...choices, "none" as const].map((grouping) => (
            <MenuItem key={grouping} id={grouping} className="item">
              {name(grouping === "none" ? undefined : grouping)}
            </MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

/** The sprint the list shows: the active one, a planned one, the backlog, or any; it edits the query's `sprint:` condition. */
function SprintMenu({ project, chips, onChange }: { project: string; chips: readonly QueryChip[] | undefined; onChange: (value: string | undefined) => void }) {
  const t = useT();
  const sprints = useSprints(project);
  const current = chips?.find((chip) => !chip.negated && (chip.field === "sprint" || chip.field === "sprints"))?.values;
  const chosen = current?.length === 1 ? current[0]?.toLowerCase() : undefined;
  const options = [
    { id: "active", label: t("list-sprint-active") },
    ...joinable(sprints.data?.items ?? []).filter((sprint) => sprint.state === "planned").map((sprint) => ({ id: sprint.name, label: sprint.name })),
    { id: "none", label: t("list-sprint-backlog") },
  ];
  const selected = options.find((option) => option.id.toLowerCase() === chosen);
  const label = selected?.label ?? (current === undefined ? t("list-sprint-any") : current.join(", "));
  return (
    <MenuTrigger>
      <Button className={classes("btn", current === undefined ? "flat" : "on")} aria-label={t("list-sprint-button", { sprint: label })}>
        <IterationCw className="ic" aria-hidden="true" />
        <span className="hide-narrow">{label}</span>
        <ChevronDown className="ic s muted" aria-hidden="true" />
      </Button>
      <Popover className="pop menuPopover" placement="bottom end" offset={4}>
        <Menu
          className="menu"
          aria-label={t("list-sprint")}
          selectionMode="single"
          selectedKeys={[selected?.id ?? (current === undefined ? "*any" : "")]}
          onAction={(id) => {
            const value = String(id);
            onChange(value === "*any" ? undefined : value);
          }}
        >
          {[...options, { id: "*any", label: t("list-sprint-any") }].map((option) => (
            <MenuItem key={option.id} id={option.id} className="item">
              {option.label}
            </MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

/** Which columns show, and in which order: the chooser's checkboxes, the custom fields' after the built-in ones. */
function ColumnMenu({ columns, fields, onChange }: { columns: readonly ColumnId[]; fields: readonly CustomField[]; onChange: (columns: ColumnId[]) => void }) {
  const t = useT();
  // Fields of several projects may share a query name: one column each.
  const custom = [...new Map(fields.map((field) => [field.query_name, field] as const)).values()];
  const all: ColumnId[] = [...COLUMNS, ...custom.map((field) => field.query_name as ColumnId)];
  const label = (column: ColumnId) => (isCustomColumn(column) ? (custom.find((field) => field.query_name === column)?.name ?? column) : t(`list-column-${column}`));
  return (
    <MenuTrigger>
      <Button className="btn flat">
        <Columns3 className="ic" aria-hidden="true" />
        <span className="hide-narrow">{t("list-columns")}</span>
      </Button>
      <Popover className="pop menuPopover" placement="bottom end" offset={4}>
        <Menu
          className="menu"
          aria-label={t("list-columns")}
          selectionMode="multiple"
          selectedKeys={columns}
          disabledKeys={FIXED_COLUMNS}
          onSelectionChange={(keys) => {
            if (keys === "all") {
              onChange([...all]);
              return;
            }
            const chosen = all.filter((column) => keys.has(column) || FIXED_COLUMNS.includes(column));
            // Columns keep their places; new ones go where the full list has them.
            onChange([...columns.filter((column) => chosen.includes(column)), ...chosen.filter((column) => !columns.includes(column))]);
          }}
        >
          {all.map((column) => (
            <MenuItem key={column} id={column} className="item" textValue={label(column)}>
              {({ isSelected }) => (
                <>
                  <input type="checkbox" className={styles.check} checked={isSelected} readOnly tabIndex={-1} aria-hidden="true" />
                  <span className="grow">{label(column)}</span>
                </>
              )}
            </MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
