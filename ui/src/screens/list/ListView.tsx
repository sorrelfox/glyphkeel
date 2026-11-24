// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useLocalization } from "@fluent/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Columns3, Download, Filter } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover } from "react-aria-components";

import { useTrashTasks } from "../../data/safety";
import { useAppearance } from "../../appearance/AppearanceProvider";
import { classes } from "../../classes";
import { Kbd } from "../../components/Kbd";
import { QueryField, type QueryFieldHandle, type Reading } from "../../components/QueryField/QueryField";
import { useToasts } from "../../components/Toasts";
import { Switch } from "../../components/Switch";
import { keysOf } from "../../actions/actions";
import { errorToast, useCall, useCommand } from "../../data/hooks";
import { allRows, defaultView, LIST_PAGE, toCsv, toJson, useListNames, useListRows, useParsedQuery, useQueryFields, useSavedViews, useUnfilteredCount, type ListScope } from "../../data/list";
import { useDurationUnits } from "../../data/time";
import { type ListedTask, type QueryProblem, type SavedView } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { saveExport } from "../../ipc/exportFile";
import { useStatus, useTimerTask } from "../../shell/context";
import { isoDay } from "../board/model";
import { BulkBar } from "./BulkBar";
import { CellContext, type CellActions } from "./cells";
import { Chips } from "./Chips";
import { SaveControls, useViewCommands, ViewPicker } from "./ViewMenu";
import { ListTable } from "./ListTable";
import { changedFrom, clickRow, COLUMNS, exportName, FIXED_COLUMNS, nextSort, NO_SELECTION, removeRanges, viewColumns, withSort, type ColumnId, type Selection } from "./model";
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
}

/**
 * The list: saved views and the query field on top, its conditions as
 * chips, the matching tasks in a virtualized table with totals, and the bulk
 * bar over a selection. The list shows nothing until its saved views are
 * read, so the default view is the first thing drawn.
 */
export function ListView({ project, openTask, onOpen }: ListViewProps) {
  const saved = useSavedViews(project);
  if (!saved.views) {
    return <div className={styles.screen} aria-busy="true" />;
  }
  return <LoadedList key={project ?? ""} project={project} views={saved.views} openTask={openTask} onOpen={onOpen} />;
}

/** The engine's message for a query mistake, in the person's language. */
function useProblemText(): (problem: QueryProblem) => string {
  const { l10n } = useLocalization();
  return (problem) => (l10n.getBundle(problem.message_id) ? l10n.getString(problem.message_id, problem.args ?? null) : problem.message);
}

function LoadedList({ project, views, openTask, onOpen }: { project: string | undefined; views: SavedView[]; openTask: string | undefined; onOpen: (key: string) => void }) {
  const t = useT();
  const call = useCall();
  const toasts = useToasts();
  const units = useDurationUnits();
  const problemText = useProblemText();
  const workspaceWide = project === undefined;
  const initial = defaultView(views, project);

  // What the list shows.
  const [activeId, setActiveId] = useState<string | undefined>(initial?.id);
  const active = views.find((view) => view.id === activeId);
  const [text, setText] = useState(initial?.query ?? "");
  const [sort, setSort] = useState(initial?.sort ?? "");
  const [columns, setColumns] = useState<ColumnId[]>(() => viewColumns(initial, workspaceWide));
  const [includeSubprojects, setIncludeSubprojects] = useState(false);
  const [selection, setSelection] = useState<Selection>(NO_SELECTION);
  const [focused, setFocused] = useState(0);
  const [editing, setEditing] = useState<CellActions["editing"]>(null);

  // The query that runs: the text once the engine reads it without a mistake.
  const parsed = useParsedQuery(text);
  const reading = parsed.data && !parsed.isPlaceholderData ? parsed.data : undefined;
  const [running, setRunning] = useState(initial?.query ?? "");
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

  const scope: ListScope = { project, includeSubprojects, query: running, sort };
  const scrollRef = useRef<HTMLDivElement>(null);
  const { density } = useAppearance().resolved;
  const rowHeight = ROW_HEIGHT[density];
  const [total, setTotal] = useState(0);
  const virtualizer = useVirtualizer({
    count: total,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 6,
  });
  const items = virtualizer.getVirtualItems();
  const rows = useListRows(scope, { first: items[0]?.index ?? 0, last: items.at(-1)?.index ?? LIST_PAGE - 1 });
  if (rows.total !== undefined && rows.total !== total) {
    setTotal(rows.total);
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
  const move = useCommand("task.move");
  const cellActions: CellActions = {
    today: isoDay(new Date()),
    hueOf: (task) => names.hues.get(task.status.id) ?? "grey",
    editing,
    setEditing,
    rename: (task, title) => {
      update.mutate({ key: task.key, title, expected_version: task.version });
    },
    setStatus: (task, status) => {
      move.mutate({ key: task.key, status });
    },
    setPriority: (task, priority) => {
      update.mutate({ key: task.key, priority });
    },
    setDue: (task, day) => {
      update.mutate(day === null ? { key: task.key, clear: ["due_date"] } : { key: task.key, due_date: day });
    },
  };

  // The header shows an order someone chose, not the default one.
  const shownSort = sort !== "" || /(^|[\s(])sort:/.test(running) ? rows.sort : [];
  const setup = { query: text, sort, columns };
  const changed = changedFrom(active, setup, workspaceWide);
  const selectedKeys = [...selection.keys];
  const allSelected = (rows.total ?? 0) > 0 && selection.keys.size === rows.total;

  const choose = (view: SavedView | undefined) => {
    setActiveId(view?.id);
    setText(view?.query ?? "");
    setRunning(view?.query ?? "");
    setSort(view?.sort ?? "");
    setColumns(viewColumns(view, workspaceWide));
    setSelection(NO_SELECTION);
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

  const sortBy = (column: ColumnId) => {
    const next = nextSort(shownSort, column);
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
        const contents = format === "csv" ? toCsv(tasks, units) : toJson(tasks, scope);
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

  const focusRow = (index: number) => {
    const count = rows.total ?? 0;
    if (count === 0) {
      return;
    }
    const next = Math.max(0, Math.min(count - 1, index));
    setFocused(next);
    virtualizer.scrollToIndex(next, { align: "auto" });
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
      const next = Math.max(0, Math.min((rows.total ?? 1) - 1, index + step));
      const key = rows.rowAt(next)?.key;
      if (key !== undefined) {
        setSelection(clickRow(selection, next, key, { toggle: false, range: event.shiftKey }, (at) => rows.rowAt(at)?.key));
      }
      focusRow(next);
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      focusRow(event.key === "Home" ? 0 : (rows.total ?? 1) - 1);
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
          <ColumnMenu columns={columns} onChange={setColumns} />
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
            {rows.error.error.message}
          </div>
        )}
        {selection.keys.size > 0 && (
          <BulkBar
            keys={selectedKeys}
            names={names}
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

/** Which columns show, and in which order: the chooser's checkboxes. */
function ColumnMenu({ columns, onChange }: { columns: readonly ColumnId[]; onChange: (columns: ColumnId[]) => void }) {
  const t = useT();
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
              onChange([...COLUMNS]);
              return;
            }
            const chosen = COLUMNS.filter((column) => keys.has(column) || FIXED_COLUMNS.includes(column));
            // Columns keep their places; new ones go where the full list has them.
            onChange([...columns.filter((column) => chosen.includes(column)), ...chosen.filter((column) => !columns.includes(column))]);
          }}
        >
          {COLUMNS.map((column) => (
            <MenuItem key={column} id={column} className="item" textValue={t(`list-column-${column}`)}>
              {({ isSelected }) => (
                <>
                  <input type="checkbox" className={styles.check} checked={isSelected} readOnly tabIndex={-1} aria-hidden="true" />
                  <span className="grow">{t(`list-column-${column}`)}</span>
                </>
              )}
            </MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
