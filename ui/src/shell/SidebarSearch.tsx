// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Search } from "lucide-react";
import { useId, useState, type KeyboardEvent } from "react";

import { type Action } from "../actions/actions";
import { useAction, useActions } from "../actions/ActionsProvider";
import { classes } from "../classes";
import { Kbd } from "../components/Kbd";
import { ProjectMark } from "../components/ProjectMark";
import { useProjects } from "../data/board";
import { useCall } from "../data/hooks";
import { opKey } from "../data/queries";
import { type Hue, type ProjectIcon } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { matchActions, matchProjects, typedKey } from "../screens/palette/model";
import { Highlight, useSettled } from "../screens/palette/shared";
import styles from "./Shell.module.css";

/** How many tasks, projects and actions the field suggests. */
const TASKS_SHOWN = 5;
const PROJECTS_SHOWN = 3;
const ACTIONS_SHOWN = 3;
/** How long typing pauses before the tasks are searched. */
const SEARCH_DELAY_MS = 120;

/** One suggestion under the field. */
type Row =
  | { kind: "search"; id: string }
  | { kind: "task"; id: string; key: string; title: string; status: string }
  | { kind: "project"; id: string; key: string; path: string; hue: Hue; icon: ProjectIcon | undefined }
  | { kind: "action"; id: string; action: Action; name: string };

/**
 * The sidebar's search field. Typing lists, as the command palette does,
 * the matching tasks (a typed key first), projects and actions; the first
 * row, chosen unless the arrows pick another, opens the search page with
 * the text, so `Enter` searches everything. `Esc` empties the field. The
 * palette itself keeps its own key, shown in the field.
 */
export function SidebarSearch() {
  const t = useT();
  const navigate = useNavigate();
  const call = useCall();
  const actions = useActions();
  const palette = useAction("palette.open");
  const projects = useProjects();
  const id = useId();
  const [query, setQuery] = useState("");
  const [focused, setFocused] = useState(false);
  const [active, setActive] = useState(0);
  const text = query.trim();
  const settled = useSettled(text, SEARCH_DELAY_MS);
  const key = typedKey(text);
  const open = focused && text !== "";

  const searchInput = { query: settled, limit: TASKS_SHOWN };
  const search = useQuery({
    queryKey: opKey("task.search", searchInput),
    queryFn: () => call("task.search", searchInput),
    enabled: open && settled !== "",
  });
  const keyInput = { keys: [key ?? ""] };
  const exact = useQuery({
    queryKey: opKey("task.get", keyInput),
    queryFn: () => call("task.get", keyInput),
    enabled: open && key !== null,
  });

  const rows: Row[] = [{ kind: "search", id: "search" }];
  if (open) {
    const keyTask = key !== null ? exact.data?.items.find((task) => task.key === key) : undefined;
    const tasks = [...(keyTask ? [keyTask] : []), ...(search.data?.items ?? []).filter((task) => task.key !== keyTask?.key)].slice(0, TASKS_SHOWN);
    for (const task of tasks) {
      rows.push({ kind: "task", id: `task-${task.key}`, key: task.key, title: task.title, status: task.status.name });
    }
    const activeProjects = (projects.data?.items ?? []).filter((project) => project.state === "active");
    for (const project of matchProjects(activeProjects, text, PROJECTS_SHOWN)) {
      rows.push({ kind: "project", id: `project-${project.key}`, key: project.key, path: project.path, hue: project.hue, icon: project.icon });
    }
    const named = actions.filter((action) => !action.disabled).map((action) => ({ ...action, name: t(action.label) }));
    for (const action of matchActions(named, text).slice(0, ACTIONS_SHOWN)) {
      rows.push({ kind: "action", id: `action-${action.id}`, action, name: action.name });
    }
  }
  const current = Math.min(active, rows.length - 1);

  const reset = () => {
    setQuery("");
    setActive(0);
  };

  const choose = (row: Row) => {
    reset();
    switch (row.kind) {
      case "search":
        void navigate({ to: "/search", search: { q: text } });
        break;
      case "task":
        void navigate({ to: "/open/$key", params: { key: row.key } });
        break;
      case "project":
        void navigate({ to: "/projects/$key", params: { key: row.key } });
        break;
      case "action":
        // After the field has let go of the key.
        window.setTimeout(row.action.run, 0);
        break;
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      if (query !== "") {
        event.preventDefault();
        reset();
      }
      return;
    }
    if ((event.key === "ArrowDown" || event.key === "ArrowUp") && open) {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : rows.length - 1;
      setActive((current + step) % rows.length);
    } else if (event.key === "Enter" && text !== "") {
      event.preventDefault();
      const row = rows[current];
      if (row) {
        choose(row);
      }
    }
  };

  const listId = `${id}-list`;
  const optionId = (row: Row) => `${id}-${row.id}`;

  return (
    <div className={styles.searchBox}>
      <label className={classes("field quiet", styles.search)} data-search-field>
        <Search className="ic" aria-hidden="true" />
        <input
          type="search"
          role="combobox"
          aria-label={t("sidebar-search-label")}
          aria-autocomplete="list"
          aria-expanded={open}
          aria-controls={open ? listId : undefined}
          aria-activedescendant={open && rows[current] ? optionId(rows[current]) : undefined}
          placeholder={t("sidebar-search")}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
          onFocus={() => {
            setFocused(true);
          }}
          onBlur={() => {
            setFocused(false);
          }}
          onKeyDown={onKeyDown}
        />
        {palette.keys[0] && <Kbd chord={palette.keys[0]} />}
      </label>
      {open && (
        <ul id={listId} role="listbox" aria-label={t("sidebar-search-results")} className={classes("pop", styles.suggestions)}>
          {rows.map((row, index) => (
            <li
              key={row.id}
              id={optionId(row)}
              role="option"
              aria-selected={index === current}
              data-suggestion={row.kind}
              className={classes("item", index === current && "on", styles.suggestion)}
              // Keep the focus in the field.
              onMouseDown={(event) => {
                event.preventDefault();
              }}
              onMouseEnter={() => {
                setActive(index);
              }}
              onClick={() => {
                choose(row);
              }}
            >
              {row.kind === "search" && (
                <>
                  <Search className="ic s muted" aria-hidden="true" />
                  <span className="truncate grow">{t("sidebar-search-everything", { query: text })}</span>
                  <span className="kbd">{t("key-enter")}</span>
                </>
              )}
              {row.kind === "task" && (
                <>
                  <span className={classes("key", styles.suggestionKey)}>{row.key}</span>
                  <span className="truncate grow">
                    <Highlight text={row.title} query={text} />
                  </span>
                  <span className="meta">{row.status}</span>
                </>
              )}
              {row.kind === "project" && (
                <>
                  <span className={classes("key", styles.suggestionKey)}>{row.key}</span>
                  <ProjectMark hue={row.hue} icon={row.icon} />
                  <span className="truncate grow">{t("palette-go-to-project", { project: row.path })}</span>
                </>
              )}
              {row.kind === "action" && (
                <>
                  <span className="truncate grow">
                    <Highlight text={row.name} query={text} />
                  </span>
                  {row.action.keys[0] && <Kbd chord={row.action.keys[0]} />}
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
