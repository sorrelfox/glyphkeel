// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The command-line tool `glyphkeel`, for Preferences › Agents & CLI. The app
// says where it is, whether the shell finds it, and the command an MCP
// client should run; it can link the copy that came with it into
// `~/.local/bin`. A plain browser can't know any of this: there the page
// shows the set-up with the bare command name.

import { useMutation, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

import { useToasts } from "../components/Toasts";
import { errorToast } from "../data/hooks";
import { OpFailure, toCallError } from "../data/transport";
import { useT } from "../i18n/I18n";
import { inApp } from "./files";

export interface CliStatus {
  /** The command an MCP client runs, before `mcp`: an absolute path, or the Flatpak's `flatpak run` command. */
  command: string[];
  /** Where the shell finds `glyphkeel`, if it does. */
  on_path?: string;
  /** The copy that came with the app. */
  bundled?: string;
  /** Where installing puts a link; absent when there is nothing to install. */
  link?: string;
  /** Whether the link's folder is on the `PATH`. */
  link_dir_on_path: boolean;
  /** The app runs as a Flatpak. */
  flatpak: boolean;
}

async function command<T>(name: string): Promise<T> {
  try {
    return await invoke<T>(name);
  } catch (error: unknown) {
    throw new OpFailure(name, toCallError(error));
  }
}

const KEY = ["cli-status"] as const;

/** Where the tool is; `null` outside the app. */
export function useCliStatus(): UseQueryResult<CliStatus | null, OpFailure> {
  return useQuery({ queryKey: KEY, queryFn: () => (inApp() ? command<CliStatus>("cli_status") : Promise.resolve(null)) });
}

/** Links the bundled tool into `~/.local/bin`; a failure becomes a toast. */
export function useInstallCli() {
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const t = useT();
  return useMutation({
    mutationFn: () => command<CliStatus>("install_cli"),
    onSuccess: (status) => {
      queryClient.setQueryData(KEY, status);
      toasts.show({ tone: "info", text: t("agents-cli-installed") });
    },
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
  });
}
