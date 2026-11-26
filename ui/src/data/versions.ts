// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Versions (releases) of a project: the roadmap's data, the task panel's
// choice of version and the Gantt chart's markers.

import { type Version, type VersionStatus } from "../generated/ops";
import { useOp } from "./hooks";

/** The statuses, in the order the roadmap and the forms offer them. */
export const VERSION_STATUSES = ["open", "locked", "closed"] as const satisfies readonly VersionStatus[];

/** A project's versions with their tasks, for the roadmap; refreshed by change events. */
export function useRoadmap(project: string) {
  return useOp("version.list", { project, include_tasks: true });
}

/** The versions a project's tasks can be planned for, with those shared by its ancestors. */
export function useVersions(project: string, options: { enabled?: boolean } = {}) {
  return useOp("version.list", { project }, options);
}

/** The versions the roadmap shows: closed ones only when asked for. */
export function shownVersions(versions: readonly Version[], showClosed: boolean): Version[] {
  return versions.filter((version) => showClosed || version.status !== "closed");
}

/** The versions a task can be planned for: the open ones. */
export function plannable(versions: readonly Version[]): Version[] {
  return versions.filter((version) => version.status === "open");
}

/** A version's due date, as a chart marker: due versions that aren't closed. */
export interface VersionMarker {
  id: string;
  name: string;
  day: string;
  status: VersionStatus;
}

export function markersOf(versions: readonly Version[] | undefined): VersionMarker[] {
  return (versions ?? []).flatMap((version) => (version.due_date === undefined ? [] : [{ id: version.id, name: version.name, day: version.due_date, status: version.status }]));
}

/** Whether a version is late: due before today and not closed. */
export function isLate(version: Pick<Version, "due_date" | "status">, today: string): boolean {
  return version.status !== "closed" && version.due_date !== undefined && version.due_date < today;
}
