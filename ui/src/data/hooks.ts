// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useMutation, useQuery, type UseMutationResult, type UseQueryResult } from "@tanstack/react-query";

import { useToasts, type Toast } from "../components/Toasts";
import { type CallError } from "../generated/ops";
import { useT, type Translate } from "../i18n/I18n";
import { type AppOpId, type AppOpMap } from "./catalog";
import { useClient } from "./DataProvider";
import { opKey, type CommandOp, type QueryOp } from "./queries";
import { OpFailure, type Client } from "./transport";

/** Calls any operation the UI knows, typed by `AppOpMap`. */
export type AppCall = <K extends AppOpId>(op: K, input: AppOpMap[K]["input"]) => Promise<AppOpMap[K]["output"]>;

/**
 * The client's `call` for every operation the UI uses, including those the
 * generated client doesn't list yet. The transport passes any name through.
 */
export function appCall(client: Client): AppCall {
  return client.call as unknown as AppCall;
}

/** The typed call of the page's client. */
export function useCall(): AppCall {
  return appCall(useClient());
}

/** An operation's answer, cached under its key and refreshed by change events. */
export function useOp<K extends QueryOp>(
  op: K,
  input: AppOpMap[K]["input"],
  options: { enabled?: boolean } = {},
): UseQueryResult<AppOpMap[K]["output"], OpFailure> {
  const call = useCall();
  return useQuery({ queryKey: opKey(op, input), queryFn: () => call(op, input), enabled: options.enabled ?? true });
}

/** The text of an error toast: the engine's own message and hint; for the transport, ours. */
export function errorToast(t: Translate, error: unknown): Toast {
  const failure: CallError = error instanceof OpFailure ? error.error : { code: "ipc", message: String(error) };
  if (failure.code === "ipc") {
    return { tone: "error", text: t("error-ipc"), detail: failure.message };
  }
  return { tone: "error", text: failure.message, detail: failure.hint };
}

interface CommandOptions {
  /**
   * A toast after success with this text and an Undo button. Undo runs the
   * app's Undo, which takes back the last change made in this app.
   */
  undoable?: string;
}

/**
 * Runs a command. Errors become a toast. The cache isn't touched here: the
 * command's change event invalidates what it changed, in this window and
 * every other.
 */
export function useCommand<K extends CommandOp>(
  op: K,
  options: CommandOptions = {},
): UseMutationResult<AppOpMap[K]["output"], OpFailure, AppOpMap[K]["input"]> {
  const call = useCall();
  const toasts = useToasts();
  const t = useT();
  const { undo } = useUndo();
  return useMutation({
    mutationFn: (input: AppOpMap[K]["input"]) => call(op, input),
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
    onSuccess: () => {
      if (options.undoable !== undefined) {
        toasts.show({ tone: "info", text: options.undoable, action: { label: t("toast-undo"), run: undo } });
      }
    },
  });
}

/**
 * The app's Undo and Redo: the engine keeps one stack for the app, shared by
 * every window. Nothing to undo, or a change made since by someone else,
 * shows as a toast.
 */
export function useUndo(): { undo: () => void; redo: () => void } {
  const { ops } = useClient();
  const toasts = useToasts();
  const t = useT();
  const fail = (error: unknown) => {
    toasts.show(errorToast(t, error));
  };
  return {
    undo: () => {
      ops.history.undo().catch(fail);
    },
    redo: () => {
      ops.history.redo().catch(fail);
    },
  };
}
