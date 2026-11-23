// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Command } from "cmdk";
import { Search } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Dialog, Modal, ModalOverlay } from "react-aria-components";

import { type Action } from "../../actions/actions";
import { useActions } from "../../actions/ActionsProvider";
import { classes } from "../../classes";
import { Kbd } from "../../components/Kbd";
import { ProjectMark } from "../../components/ProjectMark";
import { useProjects } from "../../data/board";
import { useCall } from "../../data/hooks";
import { opKey } from "../../data/queries";
import { OpFailure } from "../../data/transport";
import { useT } from "../../i18n/I18n";
import { openTaskWindow } from "../../ipc/taskWindow";
import { readPref, writePref } from "../../prefs/store";
import { findMatch, matchActions, matchProjects, nextScope, recentFrom, remember, SCOPES, typedKey, type Recent, type Scope } from "./model";
import styles from "./Palette.module.css";

/** Where the palette keeps what was opened from it. */
const RECENT_PREF = "palette.recent";
/** How many tasks a search shows. */
const TASKS_SHOWN = 8;
/** How long typing pauses before the tasks are searched. */
const SEARCH_DELAY_MS = 120;

/** A task in the results: from the search, or the one a typed key names. */
interface FoundTask {
  key: string;
  title: string;
  status: { name: string };
}

interface CommandPaletteProps {
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
}

/** `text` with the part that matches `query` marked. */
function Highlight({ text, query }: { text: string; query: string }) {
  const found = findMatch(text, query);
  if (!found) {
    return <>{text}</>;
  }
  return (
    <>
      {text.slice(0, found.start)}
      <mark>{text.slice(found.start, found.end)}</mark>
      {text.slice(found.end)}
    </>
  );
}

/** The value of a typing pause: `value`, once it hasn't changed for `delay`. */
function useSettled<T>(value: T, delay: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setSettled(value);
    }, delay);
    return () => {
      window.clearTimeout(timer);
    };
  }, [value, delay]);
  return settled;
}

/**
 * The command palette (`Ctrl+K`): one field for actions, tasks and
 * projects. Actions come from the same registry as the menus, with their
 * shortcuts; tasks from the engine's search (words, the query language, or
 * a key such as TM-41, which comes first); projects by key or name. With
 * an empty field it lists what was opened from it lately and the common
 * actions. `↑` `↓` move, `Enter` opens, `Ctrl+Enter` opens a task in its own
 * window, `Tab` changes what is listed, `Esc` closes.
 */
export function CommandPalette({ isOpen, onOpenChange }: CommandPaletteProps) {
  const t = useT();
  return (
    <ModalOverlay className={classes(styles.overlay)} isOpen={isOpen} onOpenChange={onOpenChange} isDismissable>
      <Modal className={classes(styles.modal)}>
        <Dialog className={classes("pop", styles.palette)} aria-label={t("palette-title")}>
          {({ close }) => <PaletteBody close={close} />}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}

function PaletteBody({ close }: { close: () => void }) {
  const t = useT();
  const navigate = useNavigate();
  const call = useCall();
  const actions = useActions();
  const projects = useProjects();
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<Scope>("all");
  const [recent, setRecent] = useState<Recent[]>(() => recentFrom(readPref(RECENT_PREF)));
  const selected = useRef("");
  const settled = useSettled(query.trim(), SEARCH_DELAY_MS);
  const key = typedKey(query);

  const showsTasks = scope === "all" || scope === "tasks";
  const showsProjects = scope === "all" || scope === "projects";
  const showsActions = scope === "all" || scope === "actions";

  const searchInput = { query: settled, limit: TASKS_SHOWN };
  const search = useQuery({
    queryKey: opKey("task.search", searchInput),
    queryFn: () => call("task.search", searchInput),
    enabled: showsTasks && settled !== "",
  });
  const keyInput = { keys: [key ?? ""] };
  const exact = useQuery({
    queryKey: opKey("task.get", keyInput),
    queryFn: () => call("task.get", keyInput),
    enabled: showsTasks && key !== null,
  });

  const named = actions.map((action) => ({ ...action, name: t(action.label) }));
  const shownActions = showsActions ? matchActions(named, query) : [];
  const active = (projects.data?.items ?? []).filter((project) => project.state === "active");
  const shownProjects = showsProjects ? matchProjects(active, query) : [];
  const keyTask = key !== null ? exact.data?.items.find((task) => task.key === key) : undefined;
  const tasks: FoundTask[] = [];
  if (showsTasks && query.trim() !== "") {
    if (keyTask) {
      tasks.push(keyTask);
    }
    for (const task of search.data?.items ?? []) {
      if (task.key !== keyTask?.key) {
        tasks.push(task);
      }
    }
  }
  const searchError = showsTasks && settled !== "" && key === null && search.error ? (search.error instanceof OpFailure ? search.error.error.message : String(search.error)) : null;

  const keep = (item: Recent) => {
    const next = remember(recent, item);
    setRecent(next);
    writePref(RECENT_PREF, next);
  };

  const runAction = (action: Action) => {
    if (action.disabled) {
      return;
    }
    close();
    // After the palette has handed the focus back.
    window.setTimeout(action.run, 0);
  };

  const openTask = (task: { key: string; title: string }, inWindow: boolean) => {
    keep({ kind: "task", key: task.key, title: task.title });
    close();
    if (inWindow) {
      openTaskWindow(task.key, () => void navigate({ to: "/task/$key", params: { key: task.key } })).catch((error: unknown) => {
        console.error("the task window did not open", error);
      });
    } else {
      void navigate({ to: "/open/$key", params: { key: task.key } });
    }
  };

  const openProject = (project: { key: string; name: string }) => {
    keep({ kind: "project", key: project.key, title: project.name });
    close();
    void navigate({ to: "/projects/$key", params: { key: project.key } });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Tab" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      setScope((current) => nextScope(current, event.shiftKey));
      return;
    }
    if (event.key === "Enter" && event.ctrlKey) {
      event.preventDefault();
      const value = selected.current;
      const task = value.startsWith("task:") || value.startsWith("recent-task:") ? value.slice(value.indexOf(":") + 1) : null;
      const found = tasks.find((each) => each.key === task) ?? recent.find((each) => each.kind === "task" && each.key === task);
      if (found) {
        openTask(found, true);
      }
    }
  };

  const groups: { id: string; node: ReactNode }[] = [];
  const empty = query.trim() === "";
  if (empty && recent.length > 0 && scope !== "actions") {
    groups.push({
      id: "recent",
      node: (
        <Command.Group key="recent" heading={t("palette-group-recent")}>
          {recent
            .filter((item) => scope === "all" || (scope === "tasks" ? item.kind === "task" : item.kind === "project"))
            .map((item) => (
              <Command.Item
                key={`${item.kind}:${item.key}`}
                value={`recent-${item.kind}:${item.key}`}
                className={classes("item", styles.item)}
                onSelect={() => {
                  if (item.kind === "task") {
                    openTask(item, false);
                  } else {
                    openProject({ key: item.key, name: item.title });
                  }
                }}
              >
                <span className={classes("key", styles.key)}>{item.key}</span>
                <span className="truncate grow">{item.title}</span>
                <span className="meta">{t(item.kind === "task" ? "palette-kind-task" : "palette-kind-project")}</span>
              </Command.Item>
            ))}
        </Command.Group>
      ),
    });
  }
  const actionGroup = shownActions.length > 0 && {
    id: "actions",
    node: (
      <Command.Group key="actions" heading={t("palette-group-actions")}>
        {shownActions.map((action) => (
          <Command.Item
            key={action.id}
            value={`action:${action.id}`}
            disabled={action.disabled === true}
            className={classes("item", styles.item)}
            onSelect={() => {
              runAction(action);
            }}
          >
            <span className="truncate grow">
              <Highlight text={action.name} query={query} />
            </span>
            {action.keys[0] && <Kbd chord={action.keys[0]} />}
          </Command.Item>
        ))}
      </Command.Group>
    ),
  };
  const taskGroup = tasks.length > 0 && {
    id: "tasks",
    node: (
      <Command.Group key="tasks" heading={t("palette-group-tasks")}>
        {tasks.map((task) => (
          <Command.Item
            key={task.key}
            value={`task:${task.key}`}
            className={classes("item", styles.item)}
            onSelect={() => {
              openTask(task, false);
            }}
          >
            <span className={classes("key", styles.key)}>{task.key}</span>
            <span className="truncate grow">
              <Highlight text={task.title} query={query} />
            </span>
            <span className="meta">{task.status.name}</span>
          </Command.Item>
        ))}
      </Command.Group>
    ),
  };
  const projectGroup = shownProjects.length > 0 && {
    id: "projects",
    node: (
      <Command.Group key="projects" heading={t("palette-group-projects")}>
        {shownProjects.map((project) => (
          <Command.Item
            key={project.key}
            value={`project:${project.key}`}
            className={classes("item", styles.item)}
            onSelect={() => {
              openProject(project);
            }}
          >
            <span className={classes("key", styles.key)}>{project.key}</span>
            <ProjectMark hue={project.hue} icon={project.icon} />
            <span className="truncate grow">{t("palette-go-to-project", { project: project.path })}</span>
          </Command.Item>
        ))}
      </Command.Group>
    ),
  };
  // A typed key puts its task first; a project's key, the project.
  const keyIsProject = shownProjects[0]?.key.toLowerCase() === query.trim().toLowerCase();
  const order = keyTask ? [taskGroup, actionGroup, projectGroup] : keyIsProject ? [projectGroup, actionGroup, taskGroup] : [actionGroup, taskGroup, projectGroup];
  for (const group of order) {
    if (group) {
      groups.push(group);
    }
  }

  return (
    <Command
      // cmdk names its field with this label.
      label={t("palette-field")}
      shouldFilter={false}
      loop
      className={styles.command}
      onKeyDown={onKeyDown}
      onValueChange={(value) => {
        selected.current = value;
      }}
    >
      <div className={styles.inputRow}>
        <Search className="ic muted" aria-hidden="true" />
        <Command.Input
          // The palette opens for typing.
          autoFocus
          className={styles.input}
          value={query}
          onValueChange={setQuery}
          placeholder={t("palette-placeholder")}
        />
        <span className="kbd">{t("key-escape")}</span>
      </div>
      <div className={styles.scopeRow}>
        <div className="seg" role="group" aria-label={t("palette-scope")}>
          {SCOPES.map((each) => (
            <button
              key={each}
              type="button"
              tabIndex={-1}
              className={classes(scope === each && "on")}
              aria-pressed={scope === each}
              onClick={() => {
                setScope(each);
              }}
            >
              {t(`palette-scope-${each}`)}
            </button>
          ))}
        </div>
        <span className="grow" />
        {searchError !== null && (
          <span className={classes("meta", styles.searchError)} role="status">
            {searchError}
          </span>
        )}
      </div>
      <Command.List className={styles.list} label={t("palette-results")}>
        {groups.map((group) => group.node)}
        <Command.Empty className={styles.empty}>{search.isFetching ? t("palette-searching") : t("palette-empty")}</Command.Empty>
      </Command.List>
      <div className={styles.footer}>
        <span className={styles.hint}>
          <span className="kbd">↑</span>
          <span className="kbd">↓</span>
          {t("palette-hint-move")}
        </span>
        <span className={styles.hint}>
          <span className="kbd">{t("key-enter")}</span>
          {t("palette-hint-open")}
        </span>
        <span className={styles.hint}>
          <span className="kbd">{t("palette-key-ctrl-enter")}</span>
          {t("palette-hint-window")}
        </span>
        <span className={styles.hint}>
          <span className="kbd">{t("palette-key-tab")}</span>
          {t("palette-hint-scope")}
        </span>
        <span className="grow" />
        <span>{t("palette-hint-key")}</span>
      </div>
    </Command>
  );
}
