// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The app's own settings, which belong to this computer rather than to the
// workspace: whether the app keeps running in the background when its last
// window closes, and whether reminders show as desktop notifications. The
// app keeps them in its settings file; a plain browser (the development
// bridge, the sample data, tests) keeps them in the page's storage.

import { useMutation, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";

import { useToasts } from "../components/Toasts";
import { errorToast } from "../data/hooks";
import { OpFailure, toCallError } from "../data/transport";
import { useT } from "../i18n/I18n";
import { readPref, writePref } from "../prefs/store";
import { inApp } from "./files";

export interface AppSettings {
  /** Closing the last window keeps the app running for reminders. */
  background: boolean;
  /** Reminders show as desktop notifications. */
  notifications: boolean;
  /** The open workspace's folder; absent where the page can't know it. */
  workspace_path?: string;
}

export type AppSettingsPatch = Partial<Pick<AppSettings, "background" | "notifications">>;

const DEFAULTS: AppSettings = { background: true, notifications: true };
const PREF = "app";

/** The settings kept in the page's storage, outside the app. */
function stored(): AppSettings {
  const value = readPref(PREF);
  const record = typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {};
  return {
    background: typeof record.background === "boolean" ? record.background : DEFAULTS.background,
    notifications: typeof record.notifications === "boolean" ? record.notifications : DEFAULTS.notifications,
  };
}

async function command(name: string, args: Record<string, unknown> = {}): Promise<AppSettings> {
  try {
    return await invoke<AppSettings>(name, args);
  } catch (error: unknown) {
    throw new OpFailure(name, toCallError(error));
  }
}

export function loadAppSettings(): Promise<AppSettings> {
  return inApp() ? command("app_settings") : Promise.resolve(stored());
}

export function updateAppSettings(patch: AppSettingsPatch): Promise<AppSettings> {
  if (inApp()) {
    return command("update_app_settings", { patch });
  }
  const next = { ...stored(), ...patch };
  writePref(PREF, next);
  return Promise.resolve(next);
}

const KEY = ["app-settings"] as const;

/** The app's settings, read once and kept current by changes made here. */
export function useAppSettings(): UseQueryResult<AppSettings, OpFailure> {
  return useQuery({ queryKey: KEY, queryFn: loadAppSettings, staleTime: Infinity });
}

/** Changes the app's settings; they apply at once. Errors become a toast. */
export function useUpdateAppSettings(): (patch: AppSettingsPatch) => void {
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const t = useT();
  const mutation = useMutation({
    mutationFn: updateAppSettings,
    onSuccess: (settings) => {
      queryClient.setQueryData(KEY, settings);
    },
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
  });
  return (patch) => {
    mutation.mutate(patch);
  };
}
