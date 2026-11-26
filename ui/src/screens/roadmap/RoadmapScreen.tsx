// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";
import { Check, ChevronDown, ChevronRight, Milestone, MoreHorizontal, Plus } from "lucide-react";
import { useState } from "react";
import { Button, Menu, MenuTrigger, Popover, useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries } from "../../components/ContextMenu";
import { Switch } from "../../components/Switch";
import { useCommand } from "../../data/hooks";
import { isLate, shownVersions, useRoadmap } from "../../data/versions";
import { useToday } from "../../data/zone";
import { type Version } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { TaskPanel } from "../task/TaskPanel";
import { CloseVersionDialog, DeleteVersionDialog, VersionDialog } from "./VersionDialogs";
import styles from "./Roadmap.module.css";

const route = getRouteApi("/shell/projects/$key/roadmap");

/** A date-only value as a UTC moment, so formatting never shifts the day. */
function utc(day: string): Date {
  return new Date(`${day}T00:00:00Z`);
}

type Editing = { kind: "new" } | { kind: "edit" | "close" | "delete"; version: Version } | null;

/** A project's roadmap, with the open task's panel docked on the right. */
export function RoadmapScreen() {
  const { key } = route.useParams();
  const { task } = route.useSearch();
  const navigate = useNavigate();
  const open = (other: string | undefined) => {
    void navigate({ to: "/projects/$key/roadmap", params: { key }, search: other === undefined ? {} : { task: other } });
  };
  return (
    <div className={styles.layout}>
      <Roadmap key={key} project={key} openTask={task} onOpen={open} />
      {task !== undefined && (
        <TaskPanel
          key={task}
          taskKey={task}
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

interface RoadmapProps {
  project: string;
  openTask: string | undefined;
  onOpen: (key: string) => void;
}

/**
 * The versions of a project and those shared with it, by due date, each with
 * its progress and its tasks. Closed versions are hidden until asked for.
 * The project's own versions are made, changed, closed and deleted here.
 */
export function Roadmap({ project, openTask, onOpen }: RoadmapProps) {
  const t = useT();
  const roadmap = useRoadmap(project);
  const [showClosed, setShowClosed] = useState(false);
  const [editing, setEditing] = useState<Editing>(null);
  const all = roadmap.data?.items ?? [];
  const versions = shownVersions(all, showClosed);
  const closedCount = all.length - shownVersions(all, false).length;
  const unplanned = roadmap.data?.unplanned ?? 0;
  const done = () => {
    setEditing(null);
  };
  return (
    <section className={styles.screen} aria-label={t("roadmap-title", { project })}>
      <div className={styles.controls}>
        <Button
          className="btn primary"
          onPress={() => {
            setEditing({ kind: "new" });
          }}
        >
          <Plus className="ic s" aria-hidden="true" />
          {t("roadmap-new-version")}
        </Button>
        <Switch isSelected={showClosed} onChange={setShowClosed}>
          {t("roadmap-show-closed", { count: closedCount })}
        </Switch>
        {roadmap.data && unplanned > 0 && <span className="muted">{t("roadmap-unplanned", { count: unplanned })}</span>}
      </div>
      {roadmap.error && (
        <p className={styles.note} role="alert">
          {t("roadmap-error", { message: roadmap.error.error.message })}
        </p>
      )}
      {roadmap.isPending && <p className={styles.note} aria-busy="true" />}
      {roadmap.data && versions.length === 0 && (
        <div className={styles.empty}>
          <Milestone className="ic" aria-hidden="true" />
          <p>{all.length === 0 ? t("roadmap-empty") : t("roadmap-all-closed")}</p>
        </div>
      )}
      <ol className={styles.versions}>
        {versions.map((version) => (
          <li key={version.id}>
            <VersionCard
              version={version}
              openTask={openTask}
              onOpen={onOpen}
              onEdit={(kind) => {
                setEditing({ kind, version });
              }}
            />
          </li>
        ))}
      </ol>
      {editing?.kind === "new" && <VersionDialog project={project} onClose={done} />}
      {editing?.kind === "edit" && <VersionDialog project={project} version={editing.version} onClose={done} />}
      {editing?.kind === "close" && <CloseVersionDialog project={project} version={editing.version} others={all} onClose={done} />}
      {editing?.kind === "delete" && <DeleteVersionDialog project={project} version={editing.version} onClose={done} />}
    </section>
  );
}

interface VersionCardProps {
  version: Version;
  openTask: string | undefined;
  onOpen: (key: string) => void;
  onEdit: (kind: "edit" | "close" | "delete") => void;
}

function VersionCard({ version, openTask, onOpen, onEdit }: VersionCardProps) {
  const t = useT();
  const { locale } = useLocale();
  const today = useToday();
  const reopen = useCommand("version.update", { undoable: t("roadmap-reopened", { name: version.name }) });
  // Closed versions start folded; the others show their tasks.
  const [expanded, setExpanded] = useState(version.status !== "closed");
  const date = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" });
  const percent = new Intl.NumberFormat(locale, { style: "percent" });
  const { start_date: start, due_date: due } = version;
  const dates =
    start !== undefined && due !== undefined
      ? date.formatRange(utc(start), utc(due))
      : due !== undefined
        ? t("roadmap-due", { date: date.format(utc(due)) })
        : start !== undefined
          ? t("roadmap-starts", { date: date.format(utc(start)) })
          : t("roadmap-no-dates");
  const progress = version.progress;
  const tasks = version.tasks ?? [];
  const headingId = `version-${version.id}`;
  return (
    <article className={styles.card} aria-labelledby={headingId}>
      <header className={styles.cardHead}>
        <h2 id={headingId} className={styles.name}>
          {version.name}
        </h2>
        <span className={classes("chip", version.status === "open" ? "hue-green" : version.status === "locked" ? "hue-orange" : "hue-grey")}>{t(`version-status-${version.status}`)}</span>
        {isLate(version, today) && <span className="chip hue-red">{t("roadmap-late")}</span>}
        {version.inherited === true && <span className="chip outline">{t("roadmap-shared-from", { project: version.project })}</span>}
        {version.inherited !== true && version.sharing === "descendants" && <span className="chip outline">{t("roadmap-shared")}</span>}
        <span className={classes("muted num", styles.dates)}>{dates}</span>
        {version.inherited !== true && (
          <MenuTrigger>
            <Button className="btn flat icon sm" aria-label={t("roadmap-version-menu", { name: version.name })}>
              <MoreHorizontal className="ic s" />
            </Button>
            <Popover className="pop menuPopover" placement="bottom end" offset={4}>
              <Menu className="menu" aria-label={t("roadmap-version-menu", { name: version.name })}>
                <MenuEntries
                  entries={[
                    { id: "edit", label: t("roadmap-edit"), run: () => { onEdit("edit"); } },
                    version.status === "closed"
                      ? { id: "reopen", label: t("roadmap-reopen"), run: () => { reopen.mutate({ version: version.id, status: "open" }); } }
                      : { id: "close", label: t("roadmap-close"), run: () => { onEdit("close"); } },
                    "separator",
                    { id: "delete", label: t("roadmap-delete"), run: () => { onEdit("delete"); } },
                  ]}
                />
              </Menu>
            </Popover>
          </MenuTrigger>
        )}
      </header>
      {version.description !== undefined && version.description !== "" && <p className={styles.description}>{version.description}</p>}
      <div className={styles.progress}>
        <div className="meter grow" role="progressbar" aria-label={t("roadmap-progress-label", { name: version.name })} aria-valuemin={0} aria-valuemax={100} aria-valuenow={progress.percent}>
          <i style={{ inlineSize: `${String(progress.percent)}%` }} />
        </div>
        <span className="muted num">{t("roadmap-progress", { closed: progress.closed, total: progress.total, percent: percent.format(progress.percent / 100) })}</span>
      </div>
      {tasks.length > 0 && (
        <>
          <Button
            className="btn flat sm"
            aria-expanded={expanded}
            onPress={() => {
              setExpanded(!expanded);
            }}
          >
            {expanded ? <ChevronDown className="ic s" aria-hidden="true" /> : <ChevronRight className="ic s" aria-hidden="true" />}
            {t("roadmap-tasks", { count: tasks.length })}
          </Button>
          {expanded && (
            <ul className={styles.tasks} aria-label={t("roadmap-tasks-of", { name: version.name })}>
              {tasks.map((task) => (
                <li key={task.id} className={classes(styles.task, task.closed && styles.closed, openTask === task.key && styles.open)}>
                  {task.closed ? <Check className="ic s muted" aria-label={t("roadmap-task-closed")} /> : <span className={styles.bullet} aria-hidden="true" />}
                  <Button
                    className={classes("linkbtn", styles.taskLink)}
                    onPress={() => {
                      onOpen(task.key);
                    }}
                  >
                    <span className={classes("key", styles.key)}>{task.key}</span>
                    <span className="truncate">{task.title}</span>
                  </Button>
                  <span className="muted">{task.type}</span>
                  <span className="muted">{task.status.name}</span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </article>
  );
}
