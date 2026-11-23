// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useMutation, useQuery, type UseMutationResult, type UseQueryResult } from "@tanstack/react-query";

import { useToasts, type Toast } from "../components/Toasts";
import { type Call, type CallError, type OpMap } from "../generated/ops";
import { useT, type Translate } from "../i18n/I18n";
import { useClient } from "./DataProvider";
import { opKey, type CommandOp, type QueryOp } from "./queries";
import { OpFailure } from "./transport";

/** The typed call of the page's client. */
export function useCall(): Call {
  return useClient().call;
}

/** An operation's answer, cached under its key and refreshed by change events. */
export function useOp<K extends QueryOp>(
  op: K,
  input: OpMap[K]["input"],
  options: { enabled?: boolean } = {},
): UseQueryResult<OpMap[K]["output"], OpFailure> {
  const call = useCall();
  return useQuery({ queryKey: opKey(op, input), queryFn: () => call(op, input), enabled: options.enabled ?? true });
}

/** The text of an error toast: the engine's own message and hint; for the transport, ours. */
export function errorToast(t: Translate, error: unknown): Toast {
  const failure: CallError = error instanceof OpFailure ? error.error : { code: "ipc", message: String(error) };
  if (failure.code === "ipc") {
    return { tone: "error", text: t("error-ipc"), detail: failure.message };
  }
  // The engine's newer errors name their message, so that it shows in the person's language.
  const id = failure.details?.message_id;
  const args = failure.details?.args;
  if (typeof id === "string" && t.has?.(id) === true) {
    const vars = typeof args === "object" && args !== null ? Object.fromEntries(Object.entries(args).map(([key, value]) => [key, String(value)])) : {};
    return { tone: "error", text: t(id, vars), detail: failure.hint };
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
): UseMutationResult<OpMap[K]["output"], OpFailure, OpMap[K]["input"]> {
  const call = useCall();
  const toasts = useToasts();
  const t = useT();
  const { undo } = useUndo();
  return useMutation({
    mutationFn: (input: OpMap[K]["input"]) => call(op, input),
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
