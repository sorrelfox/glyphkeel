// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useNavigate } from "@tanstack/react-router";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Undo2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { ProjectMark } from "../../components/ProjectMark";
import { Segmented } from "../../components/Segmented";
import { Select } from "../../components/Select";
import { useToasts } from "../../components/Toasts";
import { useCall, useOp } from "../../data/hooks";
import { useActivityFeed, useFailureToast, type FeedRequest, skippedReason } from "../../data/safety";
import { revertible } from "../../data/task";
import { dayIn, momentFrom, useTimeZone } from "../../data/zone";
import { type ActivityEntry, type ActorKind, type ProjectSummary } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import { useMomentFormat } from "../task/Activity";
import styles from "./Safety.module.css";

/** The feed's filters: one project, people or agents, and a range of days. */
export interface FeedFilter {
  /** A project's ID; every project when absent. */
  project?: string | undefined;
  actor?: ActorKind | undefined;
  /** First and last day, `YYYY-MM-DD`, in the workspace's zone. */
  from?: string | undefined;
  to?: string | undefined;
}

/**
 * Whether the engine's activity read takes the screen's filters. The
 * generated client of this build doesn't have them yet, so the page
 * filters; switching over is this one constant.
 */
const ENGINE_FILTERS = false;

/** The filters as the engine's activity read takes them: an actor kind, a project (with its sub-projects) and a range of moments. */
export interface EngineFeedFilters {
  actor_kind?: ActorKind;
  project?: string;
  /** The first moment, RFC 3339. */
  from?: string;
  /** The moment the range ends (the start of the day after the last), RFC 3339. */
  to?: string;
}

/** The screen's filter as the engine's read takes it; days become moments in the person's zone. */
export function engineFilters(filter: FeedFilter, zone: string | undefined): EngineFeedFilters {
  const startOf = (day: string) => new Date(momentFrom(`${day}T00:00`, zone)).toISOString();
  const dayAfter = (day: string) => {
    const next = new Date(`${day}T12:00:00Z`);
    next.setUTCDate(next.getUTCDate() + 1);
    return next.toISOString().slice(0, 10);
  };
  return {
    ...(filter.actor === undefined ? {} : { actor_kind: filter.actor }),
    ...(filter.project === undefined ? {} : { project: filter.project }),
    ...(filter.from === undefined ? {} : { from: startOf(filter.from) }),
    ...(filter.to === undefined ? {} : { to: startOf(dayAfter(filter.to)) }),
  };
}

/**
 * Splits the filter into what the engine's activity read applies and what
 * the page still applies to the entries it reads.
 */
export function feedRequest(filter: FeedFilter, zone: string | undefined, engine: boolean = ENGINE_FILTERS): { request: FeedRequest; inPage: FeedFilter } {
  if (engine) {
    // The page's project filter is exact; the engine's includes sub-projects, as the screen means.
    return { request: engineFilters(filter, zone) as FeedRequest, inPage: {} };
  }
  return { request: {}, inPage: filter };
}

/** Whether an entry passes the filter; `day` is its day in the workspace's zone. */
export function passes(entry: ActivityEntry, day: string, filter: FeedFilter): boolean {
  return (
    (filter.project === undefined || entry.project === filter.project) &&
    (filter.actor === undefined || entry.actor.kind === filter.actor) &&
    (filter.from === undefined || day >= filter.from) &&
    (filter.to === undefined || day <= filter.to)
  );
}

/** Fields that tell nothing on their own in the feed. */
const QUIET_FIELDS = new Set(["created", "trashed", "purged", "rank", "subtask_rank", "version"]);
/** Fields with a name of their own in the messages. */
const NAMED_FIELDS = new Set(["title", "description", "status", "type", "priority", "labels", "parent", "start_date", "due_date", "estimate", "progress", "rank", "project"]);

function text(value: unknown): string | undefined {
  return typeof value === "string" && value !== "" ? value : undefined;
}

/** What an entry changed, in words: the operation, the item and the fields. */
export function describeChange(t: Translate, entry: ActivityEntry, locale: string): { what: string; item: string | undefined; fields: string | undefined } {
  const f = entry.fields;
  const item = text(f.key?.after) ?? text(f.key?.before) ?? text(f.title?.after) ?? text(f.title?.before) ?? text(f.name?.after) ?? text(f.name?.before);
  const names = Object.keys(f)
    .filter((field) => !QUIET_FIELDS.has(field))
    .map((field) => (NAMED_FIELDS.has(field) ? t(`history-field-${field}`) : field));
  const fields = names.length === 0 || f.created !== undefined ? undefined : new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(names);
  let what: string;
  if (f.purged !== undefined) {
    what = t("activity-purged", { entity: t(`activity-entity-${entityName(entry.entity)}`) });
  } else if (f.trashed !== undefined) {
    what = t(f.trashed.after === true ? "activity-trashed" : "activity-restored", { entity: t(`activity-entity-${entityName(entry.entity)}`) });
  } else if (f.created !== undefined) {
    what = t("activity-created", { entity: t(`activity-entity-${entityName(entry.entity)}`) });
  } else {
    what = t("activity-changed", { entity: t(`activity-entity-${entityName(entry.entity)}`) });
  }
  return { what, item, fields };
}

const ENTITIES = new Set(["task", "project", "comment", "attachment", "time_entry", "relation", "status", "task_type", "label", "layout", "view", "wiki_page", "settings", "timer", "description"]);

/** The entity's name in the messages; anything newer reads as an item. */
function entityName(entity: string): string {
  return ENTITIES.has(entity) ? entity : "other";
}

const ALL = "*";
type ActorChoice = ActorKind | typeof ALL;

/**
 * The workspace's activity: every change, newest first, with who made it,
 * through which program and when, filtered by project, by people or agents
 * and by days. The list is virtual and reads further pages of the log as
 * it scrolls; a change can be reverted where the engine allows it.
 */
export function ActivityScreen() {
  const t = useT();
  const zone = useTimeZone();
  const projects = useOp("project.list", { include_archived: true });
  const [project, setProject] = useState<string>(ALL);
  const [actor, setActor] = useState<ActorChoice>(ALL);
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const filter: FeedFilter = {
    project: project === ALL ? undefined : project,
    actor: actor === ALL ? undefined : actor,
    from: from === "" ? undefined : from,
    to: to === "" ? undefined : to,
  };
  const { request, inPage } = feedRequest(filter, zone);
  const feed = useActivityFeed(request);
  const loaded = feed.data?.pages.flatMap((page) => page.items) ?? [];
  const rows = loaded.filter((entry) => passes(entry, dayIn(new Date(entry.at), zone), inPage));
  const oldest = loaded.at(-1);
  // Newest first: once the log reaches back before the first day, older pages can't match.
  const pastRange = inPage.from !== undefined && oldest !== undefined && dayIn(new Date(oldest.at), zone) < inPage.from;
  const canLoad = feed.hasNextPage && !feed.isFetchingNextPage && !pastRange;
  const byId = new Map((projects.data?.items ?? []).map((each) => [each.id, each]));

  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 44,
    overscan: 8,
    initialRect: { width: 800, height: 600 },
  });
  const items = virtualizer.getVirtualItems();
  const lastShown = items.at(-1)?.index ?? -1;
  // Near the end of what is loaded (or nothing matches yet): read the next page.
  const wantMore = canLoad && lastShown >= rows.length - 10;
  const { fetchNextPage } = feed;
  useEffect(() => {
    if (wantMore) {
      void fetchNextPage();
    }
  }, [wantMore, fetchNextPage, rows.length]);

  const seen = new Set<string>();
  const firstOfGroup = new Set<number>();
  for (const entry of loaded) {
    if (!seen.has(entry.group)) {
      seen.add(entry.group);
      firstOfGroup.add(entry.seq);
    }
  }

  return (
    <section className={styles.screen} aria-label={t("view-activity")}>
      <div className={classes("toolbar", styles.toolbar)} role="toolbar" aria-label={t("activity-filters")}>
        <Select
          className={styles.select}
          label={t("activity-filter-project")}
          value={project}
          onChange={setProject}
          options={[{ id: ALL, label: t("report-all-projects") }, ...(projects.data?.items ?? []).map((each) => ({ id: each.id, label: each.path, icon: <ProjectMark hue={each.hue} icon={each.icon} /> }))]}
        />
        <Segmented<ActorChoice>
          label={t("activity-filter-actor")}
          value={actor}
          onChange={setActor}
          segments={[
            { id: ALL, label: t("activity-actor-all") },
            { id: "user", label: t("activity-actor-user") },
            { id: "agent", label: t("activity-actor-agent") },
          ]}
        />
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("activity-from")} value={from} max={to === "" ? undefined : to} onChange={(event) => { setFrom(event.target.value); }} />
        </span>
        <span className="muted">–</span>
        <span className={classes("field", styles.dayField)}>
          <input type="date" aria-label={t("activity-to")} value={to} min={from === "" ? undefined : from} onChange={(event) => { setTo(event.target.value); }} />
        </span>
        {(from !== "" || to !== "" || project !== ALL || actor !== ALL) && (
          <Button
            className="btn flat sm"
            onPress={() => {
              setProject(ALL);
              setActor(ALL);
              setFrom("");
              setTo("");
            }}
          >
            {t("activity-clear-filters")}
          </Button>
        )}
        <div className="grow" />
        {feed.data && <span className={classes("muted num", styles.small)}>{t("activity-count", { shown: rows.length, total: feed.data.pages[0]?.total ?? 0 })}</span>}
      </div>
      {feed.isError && <p className={classes("faint", styles.note)}>{feed.error.error.message}</p>}
      {feed.data && rows.length === 0 && !canLoad && <p className={classes("muted", styles.note)}>{t(loaded.length === 0 ? "activity-none" : "activity-none-matching")}</p>}
      <div ref={scrollRef} className={styles.scroll}>
        {rows.length > 0 && (
          <div role="table" aria-label={t("view-activity")} aria-rowcount={rows.length + 1}>
            <div role="row" className={classes("tr head", styles.feedRow)}>
              <span role="columnheader" className="th">{t("activity-column-when")}</span>
              <span role="columnheader" className="th">{t("activity-column-who")}</span>
              <span role="columnheader" className="th">{t("activity-column-what")}</span>
              <span role="columnheader" className="th">{t("activity-column-project")}</span>
              <span role="columnheader" className="th" aria-label={t("activity-column-revert")} />
            </div>
            <div role="rowgroup" className={styles.virtual} style={{ blockSize: virtualizer.getTotalSize() }}>
              {items.map((item) => {
                const entry = rows[item.index];
                if (!entry) {
                  return null;
                }
                return (
                  <FeedRow
                    key={`${String(entry.seq)}-${entry.id}`}
                    entry={entry}
                    index={item.index}
                    start={item.start}
                    measure={virtualizer.measureElement}
                    project={entry.project === undefined ? undefined : byId.get(entry.project)}
                    offerRevert={firstOfGroup.has(entry.seq)}
                  />
                );
              })}
            </div>
          </div>
        )}
        {feed.isFetchingNextPage && <p className={classes("faint", styles.note, styles.small)}>{t("activity-loading")}</p>}
      </div>
    </section>
  );
}

function FeedRow({
  entry,
  index,
  start,
  measure,
  project,
  offerRevert,
}: {
  entry: ActivityEntry;
  index: number;
  start: number;
  measure: (element: Element | null) => void;
  project: ProjectSummary | undefined;
  offerRevert: boolean;
}) {
  const t = useT();
  const { locale } = useLocale();
  const call = useCall();
  const toasts = useToasts();
  const fail = useFailureToast();
  const navigate = useNavigate();
  const moment = useMomentFormat();
  const { what, item, fields } = describeChange(t, entry, locale);
  const openable = entry.entity === "task" && entry.fields.trashed?.after !== true && entry.fields.purged === undefined;
  const revert = () => {
    call("history.revert", { groups: [entry.group] }).then((result) => {
      const skipped = result.skipped[0];
      toasts.show(skipped ? { tone: "error", text: t("history-not-reverted"), detail: skippedReason(t, skipped) } : { tone: "info", text: t("history-reverted-toast") });
    }, fail);
  };
  return (
    <div
      role="row"
      ref={measure}
      data-index={index}
      data-activity={entry.op}
      className={classes("tr", styles.feedRow, styles.virtualRow, entry.reverted_by !== undefined && styles.reverted)}
      style={{ transform: `translateY(${String(start)}px)` }}
    >
      <span role="cell" className={classes(styles.cell, styles.small, "num")}>{moment(entry.at)}</span>
      <span role="cell" className={styles.cell}>
        <span>{entry.actor.kind === "user" ? t("comment-me") : entry.actor.name}</span>
        <span className={classes("chip", entry.actor.kind === "agent" && "hue-purple")}>{t(`via-${entry.via}`)}</span>
      </span>
      <span role="cell" className={styles.cell}>
        <span>{what}</span>
        {item !== undefined &&
          (openable ? (
            <Button className="linkbtn" onPress={() => void navigate({ to: "/open/$key", params: { key: entry.id } })}>
              {item}
            </Button>
          ) : (
            <span className={styles.title}>{item}</span>
          ))}
        {item === undefined && openable && (
          <Button className="linkbtn" onPress={() => void navigate({ to: "/open/$key", params: { key: entry.id } })}>
            {t("activity-open-task")}
          </Button>
        )}
        {fields !== undefined && <span className={classes("faint", styles.small)}>{fields}</span>}
        {entry.reverted_by !== undefined && <span className={classes("faint", styles.small)}>{t("history-was-reverted")}</span>}
      </span>
      <span role="cell" className={classes(styles.cell, "muted")}>{project?.path ?? ""}</span>
      <span role="cell" className={styles.cell}>
        {offerRevert && revertible(entry) && (
          <Button className="btn flat icon sm" aria-label={t("history-revert", { time: moment(entry.at) })} onPress={revert}>
            <Undo2 className="ic s" />
          </Button>
        )}
      </span>
    </div>
  );
}
