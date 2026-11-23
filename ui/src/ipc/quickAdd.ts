// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The quick-add window and its desktop-wide shortcut. The app keeps the
// window hidden and ready; the shortcut, the tray and the Quick add action
// show it. A plain browser has no second window and no desktop shortcut:
// there the quick-add page is opened by its path, and Preferences says the
// shortcut belongs to the desktop app.

import { useMutation, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { Channel, invoke } from "@tauri-apps/api/core";

import { useToasts } from "../components/Toasts";
import { errorToast } from "../data/hooks";
import { OpFailure, toCallError } from "../data/transport";
import { useT } from "../i18n/I18n";
import { inApp } from "./files";

async function command<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (error: unknown) {
    throw new OpFailure(name, toCallError(error));
  }
}

/** Whether the quick-add window exists: only in the desktop app. */
export function hasQuickAddWindow(): boolean {
  return inApp();
}

/** Shows the quick-add window. */
export function showQuickAdd(): Promise<void> {
  return inApp() ? command("show_quick_add") : Promise.resolve();
}

/** Hides the quick-add window; the desktop gives the focus back to the window that had it. */
export function hideQuickAdd(): Promise<void> {
  return inApp() ? command("hide_quick_add") : Promise.resolve();
}

/**
 * Listens for the quick-add window being shown. Each time, the app sends the
 * current appearance, which the main window may have changed meanwhile.
 */
export function onQuickAddShown(listener: (appearance: unknown) => void): () => void {
  if (!inApp()) {
    return () => undefined;
  }
  let live = true;
  const channel = new Channel<{ appearance?: unknown }>((message) => {
    if (live) {
      listener(message.appearance ?? null);
    }
  });
  command("watch_quick_add", { channel }).catch((error: unknown) => {
    console.error("the app did not take the quick-add channel", error);
  });
  return () => {
    live = false;
  };
}

/** Where the shortcut stands. */
export type ShortcutState = "off" | "waiting" | "portal" | "grab" | "manual" | "refused";

export interface QuickAddShortcut {
  enabled: boolean;
  /** One of `choices`, written like the keymap (`Ctrl+Alt+N`). */
  keys: string;
  choices: string[];
  state: ShortcutState;
  /** The key as the desktop describes it, when it bound it. */
  trigger?: string;
  /** Why the last way to register it failed. */
  detail?: string;
  /** The command to bind where the app can't have the key. */
  command: string;
}

const KEY = ["quick-add-shortcut"] as const;

/** The shortcut's setting and state; null outside the desktop app. Asks again while the desktop decides. */
export function useQuickAddShortcut(): UseQueryResult<QuickAddShortcut | null, OpFailure> {
  return useQuery({
    queryKey: KEY,
    queryFn: () => (inApp() ? command<QuickAddShortcut>("quick_add_shortcut") : Promise.resolve(null)),
    staleTime: Infinity,
    refetchInterval: (query) => (query.state.data?.state === "waiting" ? 1000 : false),
  });
}

/** Changes the shortcut; the answer comes back as `waiting` while the desktop registers it. */
export function useSetQuickAddShortcut(): (change: { enabled: boolean; keys: string }) => void {
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const t = useT();
  const mutation = useMutation({
    mutationFn: (change: { enabled: boolean; keys: string }) => command<QuickAddShortcut>("set_quick_add_shortcut", change),
    onSuccess: (view) => {
      queryClient.setQueryData(KEY, view);
    },
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
  });
  return (change) => {
    mutation.mutate(change);
  };
}
