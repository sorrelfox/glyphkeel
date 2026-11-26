// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The workspace switcher's commands: the workspaces of the data folder, by
// name, and making, renaming and opening one. They belong to the app, not
// to a workspace, so they are app commands rather than operations. Opening
// another workspace closes the one shown and opens the other in the same
// window; the app remembers it for its next start, and the command line and
// the agents work in it unless told another. The sample data keeps several
// workspaces in memory; a plain browser without it has no switcher.

import { useMutation, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { invoke } from "@tauri-apps/api/core";

import { useToasts } from "../components/Toasts";
import { errorToast } from "../data/hooks";
import { OP_ROOT } from "../data/queries";
import { OpFailure, toCallError } from "../data/transport";
import { useT } from "../i18n/I18n";
import { inApp } from "./files";

/** One workspace of the data folder. */
export interface WorkspaceItem {
  name: string;
  /** Whether it is the one open now. */
  current: boolean;
}

/** What the switcher shows. */
export interface WorkspacesView {
  /** The name of the workspace open now; absent when none is. */
  current?: string;
  /** The open workspace is a folder outside the data folder, so it isn't among `items`. */
  elsewhere: boolean;
  /** The workspaces of the data folder, by name. */
  items: WorkspaceItem[];
}

type Command = "workspaces" | "create_workspace" | "open_workspace" | "rename_workspace";

/** Runs a switcher command in the app or the sample data; `null` where there is no switcher. */
async function run(command: Command, args: Record<string, string> = {}): Promise<WorkspacesView | null> {
  try {
    if (inApp()) {
      return await invoke<WorkspacesView>(command, args);
    }
    if (__FAKE__) {
      const { fakeWorkspaces } = await import("../data/fake/workspaces");
      const fake = await fakeWorkspaces();
      switch (command) {
        case "workspaces":
          return fake.view();
        case "create_workspace":
          return fake.create(args.name ?? "");
        case "open_workspace":
          return fake.open(args.name ?? "");
        case "rename_workspace":
          return fake.rename(args.old ?? "", args.new ?? "");
      }
    }
    return null;
  } catch (error: unknown) {
    const engineError = typeof error === "object" && error !== null && "error" in error ? error.error : error;
    throw new OpFailure(command, toCallError(engineError));
  }
}

const KEY = ["workspaces"] as const;

/** The workspaces and the open one; `null` where the page can't switch workspaces. */
export function useWorkspaces(): UseQueryResult<WorkspacesView | null, OpFailure> {
  return useQuery({ queryKey: KEY, queryFn: () => run("workspaces") });
}

/** What a switcher change asks for. */
export type WorkspaceChange = { kind: "create"; name: string } | { kind: "open"; name: string } | { kind: "rename"; old: string; name: string };

/**
 * Makes, opens or renames a workspace. When another workspace opens, the
 * page reads everything again and goes to Today, as nothing it showed
 * belongs to the new one. A failure becomes a toast, unless `quiet`: a
 * dialog shows it itself.
 */
export function useChangeWorkspace({ quiet = false }: { quiet?: boolean } = {}) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const toasts = useToasts();
  const t = useT();
  return useMutation({
    mutationFn: (change: WorkspaceChange): Promise<WorkspacesView | null> => {
      switch (change.kind) {
        case "create":
          return run("create_workspace", { name: change.name });
        case "open":
          return run("open_workspace", { name: change.name });
        case "rename":
          return run("rename_workspace", { old: change.old, new: change.name });
      }
    },
    onMutate: () => queryClient.getQueryData<WorkspacesView | null>(KEY)?.current,
    onSuccess: async (view, change, before) => {
      queryClient.setQueryData(KEY, view);
      const switched = change.kind !== "rename" && view?.current !== before;
      if (switched) {
        await navigate({ to: "/" });
        toasts.show({ tone: "info", text: t("workspace-opened", { name: view?.current ?? "" }) });
      } else if (change.kind === "rename") {
        toasts.show({ tone: "info", text: t("workspace-renamed", { name: change.name.trim() }) });
      }
      await queryClient.invalidateQueries({ queryKey: OP_ROOT });
    },
    onError: (error) => {
      if (!quiet) {
        toasts.show(errorToast(t, error));
      }
    },
  });
}
