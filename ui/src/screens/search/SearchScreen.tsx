// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { BookOpen, FolderOpen, MessageSquare, Paperclip, Search, SquareCheck, type LucideIcon } from "lucide-react";
import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { Button } from "react-aria-components";

import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { Segmented } from "../../components/Segmented";
import { Select } from "../../components/Select";
import { useProjects } from "../../data/board";
import { highlightPieces, SEARCH_KINDS, useSearch } from "../../data/search";
import { type SearchHit, type SearchKind, type TextRange } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useMomentFormat } from "../task/Activity";
import { TaskPanel } from "../task/TaskPanel";
import styles from "./Search.module.css";

const WikiPageDialog = lazy(() => import("./WikiPageDialog"));

const route = getRouteApi("/shell/search");

/** The search page's address: the text, one kind or all, one project or all, and the task open beside. */
export interface SearchParams {
  q?: string;
  kind?: SearchKind;
  project?: string;
  task?: string;
}

/** Reads the search page's address. */
export function searchParams(search: Record<string, unknown>): SearchParams {
  const params: SearchParams = {};
  for (const key of ["q", "kind", "project", "task"] as const) {
    const value = search[key];
    if (typeof value !== "string" || value === "") {
      continue;
    }
    if (key === "kind") {
      if ((SEARCH_KINDS as readonly string[]).includes(value)) {
        params.kind = value as SearchKind;
      }
    } else {
      params[key] = value;
    }
  }
  return params;
}

/** The address without one of its parts. */
function without(params: SearchParams, key: keyof SearchParams): SearchParams {
  return Object.fromEntries(Object.entries(params).filter(([name]) => name !== key));
}

const ICONS: Readonly<Record<SearchKind, LucideIcon>> = {
  task: SquareCheck,
  comment: MessageSquare,
  wiki: BookOpen,
  file: Paperclip,
  project: FolderOpen,
};

/** The project filter's "all" entry. */
const ALL = "*";

/**
 * Search across the workspace: tasks, comments, wiki pages, files and
 * projects, ranked, with the matches marked. A task, comment or file opens
 * its task beside the results; a wiki page opens read only; a project opens
 * its board.
 */
export function SearchScreen() {
  const params = route.useSearch();
  const navigate = useNavigate();
  const go = (next: SearchParams) => {
    void navigate({ to: "/search", search: next, replace: true });
  };
  const open = (task: string | undefined) => {
    const rest = without(params, "task");
    void navigate({ to: "/search", search: task === undefined ? rest : { ...rest, task } });
  };
  return (
    <div className={styles.layout}>
      <div className={styles.scroll}>
        <SearchResults params={params} onChange={go} onOpenTask={open} />
      </div>
      {params.task !== undefined && (
        <TaskPanel
          key={params.task}
          taskKey={params.task}
          onClose={() => {
            open(undefined);
          }}
          onOpenTask={open}
          onOpenWindow={(other) => void navigate({ to: "/task/$key", params: { key: other } })}
        />
      )}
    </div>
  );
}

function SearchResults({ params, onChange, onOpenTask }: { params: SearchParams; onChange: (next: SearchParams) => void; onOpenTask: (key: string) => void }) {
  const t = useT();
  const navigate = useNavigate();
  const projects = useProjects();
  const [text, setText] = useState(params.q ?? "");
  const [wiki, setWiki] = useState<string | null>(null);
  const results = useSearch({ text: params.q ?? "", kinds: params.kind === undefined ? [] : [params.kind], project: params.project });
  const hits = results.data?.pages.flatMap((page) => page.items) ?? [];
  const total = results.data?.pages[0]?.total ?? 0;

  // The text follows the address when another search (the sidebar's) changes it.
  const [shownQuery, setShownQuery] = useState(params.q);
  if (shownQuery !== params.q) {
    setShownQuery(params.q);
    if (text.trim() !== (params.q ?? "").trim()) {
      setText(params.q ?? "");
    }
  }

  // Searching as one types, after a short pause.
  const latest = useRef({ params, onChange });
  useEffect(() => {
    latest.current = { params, onChange };
  });
  useEffect(() => {
    const timer = setTimeout(() => {
      const current = latest.current;
      if (text.trim() !== (current.params.q ?? "").trim()) {
        current.onChange({ ...current.params, q: text });
      }
    }, 250);
    return () => {
      clearTimeout(timer);
    };
  }, [text]);

  const activate = (hit: SearchHit) => {
    if (hit.kind === "wiki") {
      setWiki(hit.id);
    } else if (hit.page !== undefined) {
      // A file on a wiki page opens the page.
      setWiki(hit.page);
    } else if (hit.kind === "project") {
      void navigate({ to: "/projects/$key", params: { key: hit.project } });
    } else if (hit.task !== undefined) {
      onOpenTask(hit.task);
    }
  };

  const kinds = [{ id: "all" as const, label: t("search-kind-all") }, ...SEARCH_KINDS.map((kind) => ({ id: kind, label: t(`search-kind-${kind}`) }))];
  const projectOptions = [
    { id: ALL, label: t("search-all-projects") },
    ...(projects.data?.items ?? []).map((project) => ({ id: project.key, label: project.path, icon: <ProjectMark hue={project.hue} icon={project.icon} /> })),
  ];

  return (
    <div className={styles.column}>
      <label className={classes("field", styles.field)}>
        <Search className="ic" aria-hidden="true" />
        <input
          type="search"
          aria-label={t("search-field")}
          placeholder={t("search-placeholder")}
          autoFocus
          value={text}
          onChange={(event) => {
            setText(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              onChange({ ...params, q: text });
            }
          }}
        />
      </label>
      <div className={styles.filters}>
        <Segmented
          label={t("search-kinds")}
          segments={kinds}
          value={params.kind ?? "all"}
          onChange={(kind) => {
            const rest = without(params, "kind");
            onChange(kind === "all" ? rest : { ...rest, kind });
          }}
        />
        <Select
          label={t("search-project")}
          className={styles.project}
          options={projectOptions}
          value={params.project ?? ALL}
          onChange={(project) => {
            const rest = without(params, "project");
            onChange(project === ALL ? rest : { ...rest, project });
          }}
        />
      </div>

      {(params.q ?? "").trim() === "" ? (
        <p className={classes("muted", styles.nothing)}>{t("search-hint")}</p>
      ) : results.error ? (
        <p className={styles.nothing} role="alert">
          {results.error.error.message}
        </p>
      ) : results.data === undefined ? (
        <div aria-busy="true" />
      ) : hits.length === 0 ? (
        <p className={classes("muted", styles.nothing)}>{t("search-nothing", { text: params.q ?? "" })}</p>
      ) : (
        <>
          <p className={classes("muted", styles.summary)} role="status">
            {t("search-count", { count: total })}
          </p>
          <ul className={styles.results} aria-label={t("search-results")}>
            {hits.map((hit) => (
              <li key={`${hit.kind}-${hit.id}`}>
                <HitRow hit={hit} open={hit.task !== undefined && hit.task === params.task} onActivate={activate} />
              </li>
            ))}
          </ul>
          {results.hasNextPage && (
            <Button
              className={classes("btn", styles.more)}
              isDisabled={results.isFetchingNextPage}
              onPress={() => {
                void results.fetchNextPage();
              }}
            >
              {t("search-more")}
            </Button>
          )}
        </>
      )}
      {wiki !== null && (
        <Suspense>
          <WikiPageDialog
            page={{ id: wiki }}
            onClose={() => {
              setWiki(null);
            }}
            onOpenTask={onOpenTask}
          />
        </Suspense>
      )}
    </div>
  );
}

/** A text with its matches marked. */
function Marked({ text, ranges }: { text: string; ranges: readonly TextRange[] | undefined }) {
  return (
    <>
      {highlightPieces(text, ranges).map((piece, index) => (piece.hit ? <mark key={index}>{piece.text}</mark> : <span key={index}>{piece.text}</span>))}
    </>
  );
}

function HitRow({ hit, open, onActivate }: { hit: SearchHit; open: boolean; onActivate: (hit: SearchHit) => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const Icon = ICONS[hit.kind];
  const kind = t(`search-kind-one-${hit.kind}`);
  return (
    <button
      type="button"
      className={styles.hit}
      data-hit={hit.kind}
      data-open={open ? "" : undefined}
      onClick={() => {
        onActivate(hit);
      }}
    >
      <Icon className="ic" role="img" aria-label={kind} />
      <span className={styles.hitTitle}>
        {hit.task !== undefined && <span className="key">{hit.task}</span>}
        <span className={styles.text}>
          <Marked text={hit.title} ranges={hit.title_matches} />
        </span>
      </span>
      <span className={classes("meta", styles.where)} title={moment(hit.updated_at)}>
        {hit.status === undefined ? hit.project : t("search-where", { project: hit.project, status: hit.status })}
      </span>
      {hit.snippet !== undefined && (
        <span className={styles.snippet}>
          {hit.kind === "comment" && <span className="faint">{t("search-in-comment")} </span>}
          <Marked text={hit.snippet} ranges={hit.snippet_matches} />
        </span>
      )}
    </button>
  );
}
