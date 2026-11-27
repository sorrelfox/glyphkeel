// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useMutation, useQuery, type UseMutationResult, type UseQueryResult } from "@tanstack/react-query";
import { useLocale } from "react-aria-components";

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

/**
 * A message from the engine or the app in the person's language: message
 * `id` with its arguments where the message files have it, else the English
 * text that came with it.
 */
export function engineText(t: Translate, id: unknown, args: unknown, english: string): string;
export function engineText(t: Translate, id: unknown, args: unknown, english: string | undefined): string | undefined;
export function engineText(t: Translate, id: unknown, args: unknown, english: string | undefined): string | undefined {
  if (typeof id !== "string" || t.has?.(id) !== true) return english;
  const vars = typeof args === "object" && args !== null ? Object.fromEntries(Object.entries(args).map(([key, value]) => [key, String(value)])) : {};
  return t(id, vars);
}

/** The error of a failed call: an `OpFailure`'s, one already unwrapped, or anything else as the transport's. */
function callError(error: unknown): CallError {
  if (error instanceof OpFailure) {
    return error.error;
  }
  if (typeof error === "object" && error !== null && "code" in error && "message" in error && typeof error.code === "string" && typeof error.message === "string") {
    return error as CallError;
  }
  return { code: "ipc", message: String(error) };
}

/**
 * What a failed call says, for people: the engine's message in the person's
 * language; for the transport, ours with its own words after it.
 */
export function failureText(t: Translate, error: unknown): string {
  // An OpFailure, a CallError or anything thrown.
  const failure = callError(error);
  if (failure.code === "ipc") {
    return t("error-ipc-detail", { detail: failure.message });
  }
  return engineText(t, failure.details?.message_id, failure.details?.args, failure.message);
}

/** What a failed call says to do next, if anything, in the person's language. */
export function failureHint(t: Translate, error: unknown): string | undefined {
  const failure = callError(error);
  if (failure.code === "ipc") {
    return undefined;
  }
  return engineText(t, failure.details?.hint_id, failure.details?.hint_args, failure.hint);
}

/** The text of an error toast: the engine's own message and hint; for the transport, ours. */
export function errorToast(t: Translate, error: unknown): Toast {
  const failure = callError(error);
  if (failure.code === "ipc") {
    return { tone: "error", text: t("error-ipc"), detail: failure.message };
  }
  // The engine's errors name their message, so that it shows in the person's language.
  return { tone: "error", text: failureText(t, error), detail: failureHint(t, error) };
}

/** Turns a failed call into its text, in the person's language. */
export function useFailureText(): (error: unknown) => string {
  const t = useT();
  return (error) => failureText(t, error);
}

/** The names of the saved views a change rewrote, from its output. */
export function viewsRewritten(output: unknown): string[] {
  if (typeof output !== "object" || output === null || !("views_rewritten" in output) || !Array.isArray(output.views_rewritten)) {
    return [];
  }
  return output.views_rewritten.filter((name): name is string => typeof name === "string");
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
  const { locale } = useLocale();
  return useMutation({
    mutationFn: (input: OpMap[K]["input"]) => call(op, input),
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
    onSuccess: (output) => {
      // A rename (or a label's deletion) that changed saved views says which.
      const views = viewsRewritten(output);
      const rewritten = views.length === 0 ? undefined : t("views-rewritten", { count: views.length, names: new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(views) });
      if (options.undoable !== undefined) {
        toasts.show({ tone: "info", text: options.undoable, detail: rewritten, action: { label: t("toast-undo"), run: undo } });
      } else if (rewritten !== undefined) {
        toasts.show({ tone: "info", text: rewritten });
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
