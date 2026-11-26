// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The sample data's workspace switcher: the fake transport keeps several
// in-memory workspaces and registers itself here, so the switcher works in
// a plain browser as the app's does, without any of this reaching the
// app's own build.

import { type WorkspacesView } from "../../ipc/workspaces";

/** What the fake transport offers the switcher. */
export interface FakeWorkspaces {
  view(): WorkspacesView;
  create(name: string): WorkspacesView;
  open(name: string): WorkspacesView;
  rename(old: string, name: string): WorkspacesView;
}

let register: (workspaces: FakeWorkspaces) => void = () => undefined;
const registered = new Promise<FakeWorkspaces>((resolve) => {
  register = resolve;
});

/** Called by the fake transport when it starts. */
export function registerFakeWorkspaces(workspaces: FakeWorkspaces): void {
  register(workspaces);
}

/** The fake transport's switcher, once the transport has started. */
export function fakeWorkspaces(): Promise<FakeWorkspaces> {
  return registered;
}

/** The letters a workspace's name may have, as in the app: a folder name on every system. */
export const WORKSPACE_NAME = /^[\p{L}\p{N}_-](?:[\p{L}\p{N} _-]{0,62}[\p{L}\p{N}_-])?$/u;
