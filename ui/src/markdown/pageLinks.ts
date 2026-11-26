// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { createContext, useContext } from "react";

/** A wiki page a link found: its ID and project, and the heading the link names. */
export interface PageTarget {
  id: string;
  project: string;
  heading?: string | undefined;
}

/**
 * How a window opens a wiki page that a link names: the main window shows
 * it in the project's wiki. Without one (task and wiki windows), a link's
 * page opens read only first, and from there in a window of its own.
 */
export const PageOpener = createContext<((page: PageTarget) => void) | null>(null);

export function usePageOpener(): ((page: PageTarget) => void) | null {
  return useContext(PageOpener);
}
