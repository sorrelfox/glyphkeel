// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The app's own settings, which belong to this computer rather than to the
// workspace: whether the app keeps running in the background when its last
// window closes, and whether reminders show as desktop notifications. The
// app keeps them in its settings file; a plain browser (the development
// bridge, the sample data, tests) keeps them in the page's storage.
//
// The person's time zone comes with them: the app keeps it in the personal
// settings that the command line and the MCP server read too. In a plain
// browser it lives in the page's storage and changes only how the page
// shows times.

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
  /** The time zone as chosen: `"system"` or an IANA name such as `"Europe/Berlin"`. */
  time_zone: string;
  /** The zone times are shown in now, by its IANA name; absent when the system's zone has no name. */
  zone_in_use?: string;
  /** The open workspace's folder; absent where the page can't know it. */
  workspace_path?: string;
}

export type AppSettingsPatch = Partial<Pick<AppSettings, "background" | "notifications" | "time_zone">>;

/** A time zone to choose from, with its offset from UTC now. */
export interface ZoneChoice {
  name: string;
  offset_seconds: number;
}

export const SYSTEM_ZONE = "system";

const DEFAULTS = { background: true, notifications: true, time_zone: SYSTEM_ZONE };
const PREF = "app";

/** The settings kept in the page's storage, outside the app. */
function stored(): AppSettings {
  const value = readPref(PREF);
  const record = typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {};
  const zone = typeof record.time_zone === "string" && knownZone(record.time_zone) ? record.time_zone : DEFAULTS.time_zone;
  const inUse = zone === SYSTEM_ZONE ? systemZone() : zone;
  return {
    background: typeof record.background === "boolean" ? record.background : DEFAULTS.background,
    notifications: typeof record.notifications === "boolean" ? record.notifications : DEFAULTS.notifications,
    time_zone: zone,
    ...(inUse === undefined ? {} : { zone_in_use: inUse }),
  };
}

/** The system's zone by its IANA name, if it has one. */
function systemZone(): string | undefined {
  const zone = new Intl.DateTimeFormat().resolvedOptions().timeZone;
  return zone === "" ? undefined : zone;
}

/** Whether the page's date library knows the zone. */
function knownZone(zone: string): boolean {
  if (zone === SYSTEM_ZONE) {
    return true;
  }
  try {
    new Intl.DateTimeFormat("en", { timeZone: zone }).format(0);
    return true;
  } catch {
    return false;
  }
}

/** The zone's offset from UTC at `at`, in seconds. */
export function zoneOffsetSeconds(zone: string, at: number): number {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: zone,
    hourCycle: "h23",
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "numeric",
    second: "numeric",
  }).formatToParts(at);
  const part = (type: Intl.DateTimeFormatPartTypes) => Number(parts.find((each) => each.type === type)?.value ?? 0);
  const local = Date.UTC(part("year"), part("month") - 1, part("day"), part("hour"), part("minute"), part("second"));
  return Math.round((local - Math.floor(at / 1000) * 1000) / 1000);
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
  if (patch.time_zone !== undefined && !knownZone(patch.time_zone)) {
    return Promise.reject(new OpFailure("update_app_settings", { code: "invalid", message: `Unknown time zone "${patch.time_zone}".` }));
  }
  const { background, notifications, time_zone } = { ...stored(), ...patch };
  writePref(PREF, { background, notifications, time_zone });
  return Promise.resolve(stored());
}

/** Every time zone a person can choose, by name, with its offset from UTC now. */
export async function loadTimeZones(): Promise<ZoneChoice[]> {
  if (inApp()) {
    try {
      return await invoke<ZoneChoice[]>("time_zones");
    } catch (error: unknown) {
      throw new OpFailure("time_zones", toCallError(error));
    }
  }
  const now = Date.now();
  const names = typeof Intl.supportedValuesOf === "function" ? Intl.supportedValuesOf("timeZone") : [];
  return [...new Set(["UTC", ...names])].map((name) => ({ name, offset_seconds: zoneOffsetSeconds(name, now) })).sort((a, b) => a.name.localeCompare(b.name));
}

/** The time zones for the picker, read when first asked for. */
export function useTimeZones(enabled: boolean): UseQueryResult<ZoneChoice[], OpFailure> {
  return useQuery({ queryKey: ["time-zones"], queryFn: loadTimeZones, staleTime: 60 * 60 * 1000, enabled });
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
    onSuccess: (settings, patch) => {
      queryClient.setQueryData(KEY, settings);
      if (patch.time_zone !== undefined) {
        // "Today" and local times moved: everything the engine answered may differ now.
        void queryClient.invalidateQueries({ predicate: (query) => query.queryKey[0] !== KEY[0] });
      }
    },
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
  });
  return (patch) => {
    mutation.mutate(patch);
  };
}
