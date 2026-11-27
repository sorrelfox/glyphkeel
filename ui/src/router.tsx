// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import {
  createMemoryHistory,
  createRootRouteWithContext,
  createRoute,
  createRouter,
  Outlet,
  useNavigate,
  useRouteContext,
} from "@tanstack/react-router";
import { lazy, Suspense, useCallback, useEffect, useState } from "react";

import { onOpenLink } from "./ipc/openLinks";
import { PageOpener, type PageTarget } from "./markdown/pageLinks";
import { windowReady, type Transport } from "./ipc/ready";
import { BoardScreen } from "./screens/board/BoardScreen";
import { OpenTaskScreen, TaskWindowScreen } from "./screens/task/TaskWindow";
import { AllTasksListScreen, ProjectListScreen } from "./screens/list/ListScreen";
import { FlowReportsScreen } from "./screens/reports/FlowReports";
import { TimeReportScreen } from "./screens/time/TimeReport";
import { ActivityScreen } from "./screens/safety/ActivityScreen";
import { TrashScreen } from "./screens/safety/TrashScreen";
import { QuickAdd } from "./screens/quick-add/QuickAdd";
import { SearchScreen, searchParams } from "./screens/search/SearchScreen";
import { TodayScreen } from "./screens/today/TodayScreen";
import { wikiSearch, WikiScreen } from "./screens/wiki/WikiScreen";
import { WikiWindowScreen } from "./screens/wiki/WikiWindow";
import { offersFirstRun } from "./shell/FirstRun";
import { Shell } from "./shell/Shell";
import { StartupFailure } from "./shell/StartupFailure";
import { type StartData } from "./start";

// Loaded on first use, so that it costs the app's start nothing.
const Gallery = lazy(() => import("./dev/Gallery"));
const EditorPlayground = lazy(() => import("./editor/dev/EditorPlayground"));
const CalendarScreen = lazy(() => import("./screens/calendar/CalendarScreen").then((module) => ({ default: module.CalendarScreen })));
const GanttScreen = lazy(() => import("./screens/gantt/GanttScreen").then((module) => ({ default: module.GanttScreen })));
const RoadmapScreen = lazy(() => import("./screens/roadmap/RoadmapScreen").then((module) => ({ default: module.RoadmapScreen })));

/** The path that opens the gallery directly, for the browser tests. */
export const GALLERY_PATH = "/dev/themes";

/** The path of the Markdown editor's development page, for the browser tests. */
export const EDITOR_PLAYGROUND_PATH = "/dev/editor";

/** The quick-add window's page; a plain browser opens it by this path. */
export const QUICK_ADD_PATH = "/quick-add";

export interface RouterContext {
  start: StartData;
}

/**
 * The window's screens. Navigation is app state in memory, not the address
 * bar: back and forward work inside the window, and a link such as
 * `glyphkeel://open/TM-41` becomes a navigation.
 */
const rootRoute = createRootRouteWithContext<RouterContext>()({ component: Root });

const galleryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: GALLERY_PATH,
  component: GalleryScreen,
});

const editorPlaygroundRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: EDITOR_PLAYGROUND_PATH,
  component: EditorPlaygroundScreen,
});

const shellRoute = createRoute({ getParentRoute: () => rootRoute, id: "shell", component: ShellScreen });

/** The task open in the panel beside a board or a list. */
const taskSearch = (search: Record<string, unknown>): { task?: string } =>
  typeof search.task === "string" && search.task !== "" ? { task: search.task } : {};

/** Today; `task` is the task open in the panel beside it. */
const todayRoute = createRoute({ getParentRoute: () => shellRoute, path: "/", validateSearch: taskSearch, component: TodayScreen });

/** The search page: the text, filters and the task open beside the results. */
const searchRoute = createRoute({ getParentRoute: () => shellRoute, path: "/search", validateSearch: searchParams, component: SearchScreen });

/** Every task of the workspace, as a list. */
const allTasksRoute = createRoute({ getParentRoute: () => shellRoute, path: "/all", validateSearch: taskSearch, component: AllTasksListScreen });

/** The time report: logged time over a range of days. */
const timeReportRoute = createRoute({ getParentRoute: () => shellRoute, path: "/time", component: TimeReportScreen });
/** The flow reports: created against closed, cumulative flow, cycle time. */
const reportsRoute = createRoute({ getParentRoute: () => shellRoute, path: "/reports", component: FlowReportsScreen });

/** The trash: deleted items, to restore or delete for good. */
const trashRoute = createRoute({ getParentRoute: () => shellRoute, path: "/trash", component: TrashScreen });

/** The workspace's activity: every change, newest first. */
const activityRoute = createRoute({ getParentRoute: () => shellRoute, path: "/activity", component: ActivityScreen });

/** A project's board; `task` is the task open in the panel beside it. */
export const boardRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: "/projects/$key",
  validateSearch: (search: Record<string, unknown>): { task?: string } =>
    typeof search.task === "string" && search.task !== "" ? { task: search.task } : {},
  component: BoardScreen,
});

/** A project's list; `query`, when given, is the query it opens with instead of the default view's. */
const listSearch = (search: Record<string, unknown>): { task?: string; query?: string } => ({
  ...taskSearch(search),
  ...(typeof search.query === "string" && search.query.trim() !== "" ? { query: search.query } : {}),
});

/** A project's list. */
export const projectListRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: "/projects/$key/list",
  validateSearch: listSearch,
  component: ProjectListScreen,
});

/** A project's calendar; loaded on first use. */
export const projectCalendarRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: "/projects/$key/calendar",
  validateSearch: taskSearch,
  component: LazyCalendarScreen,
});

/** A project's Gantt chart; loaded on first use. */
export const projectGanttRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: "/projects/$key/gantt",
  validateSearch: taskSearch,
  component: LazyGanttScreen,
});

/** A project's roadmap: its versions with their progress and tasks; loaded on first use. */
export const projectRoadmapRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: "/projects/$key/roadmap",
  validateSearch: taskSearch,
  component: LazyRoadmapScreen,
});

/** The quick-add window: one field, without the board and the sidebar. */
const quickAddRoute = createRoute({ getParentRoute: () => rootRoute, path: QUICK_ADD_PATH, component: QuickAdd });

/** A project's wiki: the page tree and one page; `task` is a task open beside it. */
export const wikiRoute = createRoute({
  getParentRoute: () => shellRoute,
  path: "/projects/$key/wiki",
  validateSearch: wikiSearch,
  component: WikiScreen,
});

/** A wiki window: one page, without the tree and the sidebar. */
const wikiWindowRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/wiki/$id",
  validateSearch: (search: Record<string, unknown>): { heading?: string } => (typeof search.heading === "string" && search.heading !== "" ? { heading: search.heading } : {}),
  component: WikiWindowScreen,
});

/** A task window: one task, without the board and the sidebar. */
const taskWindowRoute = createRoute({ getParentRoute: () => rootRoute, path: "/task/$key", component: TaskWindowScreen });

/** A task named by a link: its board opens with the task beside it. */
const openTaskRoute = createRoute({ getParentRoute: () => shellRoute, path: "/open/$key", component: OpenTaskScreen });

const routeTree = rootRoute.addChildren([
  galleryRoute,
  editorPlaygroundRoute,
  quickAddRoute,
  taskWindowRoute,
  wikiWindowRoute,
  shellRoute.addChildren([todayRoute, searchRoute, allTasksRoute, timeReportRoute, reportsRoute, trashRoute, activityRoute, boardRoute, projectListRoute, projectCalendarRoute, projectGanttRoute, projectRoadmapRoute, wikiRoute, openTaskRoute]),
]);

/**
 * The first screen: a development page or the quick-add page from its path
 * (browser tests); the quick-add window's field; a task or wiki window's item; the
 * task of the link the app was started with; else Today.
 */
export function initialPath(pathname: string, start: Pick<StartData, "task" | "wiki" | "open" | "quickAdd"> = {}): string {
  for (const path of [GALLERY_PATH, EDITOR_PLAYGROUND_PATH, QUICK_ADD_PATH]) {
    if (pathname.startsWith(path)) {
      return path;
    }
  }
  if (start.quickAdd === true) {
    return QUICK_ADD_PATH;
  }
  if (start.task !== undefined) {
    return `/task/${start.task}`;
  }
  if (start.wiki !== undefined) {
    return `/wiki/${start.wiki}`;
  }
  if (start.open !== undefined) {
    return `/open/${start.open}`;
  }
  return "/";
}

export function createAppRouter(path: string, context: RouterContext) {
  return createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [path] }), context });
}

declare module "@tanstack/react-router" {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}

function Root() {
  const [transport, setTransport] = useState<Transport | null>(null);

  // React's first commit: the window may be shown now. The answer also says
  // whether the page's calls travel the way they should.
  useEffect(() => {
    windowReady().then(setTransport, (error: unknown) => {
      console.error("the app did not answer the first-screen signal", error);
    });
  }, []);

  if (transport === "postMessage") {
    return <StartupFailure reason="startup-ipc-fallback" />;
  }
  return <Outlet />;
}

function ShellScreen() {
  const { start } = useRouteContext({ from: "__root__" });
  const navigate = useNavigate();
  // Links handed to the running app open their task here, in the main window.
  useEffect(() => onOpenLink((key) => void navigate({ to: "/open/$key", params: { key } })), [navigate]);
  const [firstRun] = useState(() => offersFirstRun(start));
  // A wiki link in any text of the main window opens its page in the project's wiki.
  const openPage = useCallback(
    (page: PageTarget) => void navigate({ to: "/projects/$key/wiki", params: { key: page.project }, search: { page: page.id, ...(page.heading === undefined ? {} : { heading: page.heading }) } }),
    [navigate],
  );
  return (
    <PageOpener value={openPage}>
      <Shell titleBar={start.titleBar} firstRun={firstRun} openGallery={import.meta.env.DEV ? () => void navigate({ to: GALLERY_PATH }) : undefined} />
    </PageOpener>
  );
}

function GalleryScreen() {
  const navigate = useNavigate();
  return (
    <Suspense>
      <Gallery onLeave={() => void navigate({ to: "/" })} />
    </Suspense>
  );
}

function LazyCalendarScreen() {
  return (
    <Suspense>
      <CalendarScreen />
    </Suspense>
  );
}

function LazyGanttScreen() {
  return (
    <Suspense>
      <GanttScreen />
    </Suspense>
  );
}

function LazyRoadmapScreen() {
  return (
    <Suspense>
      <RoadmapScreen />
    </Suspense>
  );
}

function EditorPlaygroundScreen() {
  const navigate = useNavigate();
  return (
    <Suspense>
      <EditorPlayground onLeave={() => void navigate({ to: "/" })} />
    </Suspense>
  );
}
