// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The hooks the project screens use: projects, the board and its layout,
// tasks. Screens call these; these call the transport through the client.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useAskCloseDuplicates } from "../components/DuplicatePrompt";
import { useToasts } from "../components/Toasts";
import { type LayoutChange, type LayoutUpdateInput, type ProjectList, type TaskCreateInput, type TaskUpdateInput } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { type Board, type BoardLayout, type Card, type Place } from "./catalog";
import { errorToast, useCall, useOp } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

export function useProjects() {
  return useOp("project.list", { include_archived: true });
}

/** Open tasks: in all active projects, and by project key. */
export interface Counts {
  all: number;
  /** Open tasks of each project alone. */
  own: Record<string, number>;
  /** Open tasks of each project with its sub-projects. */
  withSubprojects: Record<string, number>;
}

export function countsOf(list: ProjectList): Counts {
  const counts: Counts = { all: 0, own: {}, withSubprojects: {} };
  for (const project of list.items) {
    counts.own[project.key] = project.open_tasks;
    counts.withSubprojects[project.key] = project.open_tasks_with_subprojects;
    if (project.state === "active") {
      counts.all += project.open_tasks;
    }
  }
  return counts;
}

/** The sidebar's counts, read from the project tree. */
export function useCounts(): Counts | undefined {
  const projects = useProjects();
  return projects.data ? countsOf(projects.data) : undefined;
}

export function useProject(key: string | undefined) {
  return useOp("project.get", { project: key ?? "" }, { enabled: key !== undefined });
}

/**
 * The board's input. Closed cards older than the layout's limit stay in the
 * engine until "Show older" asks for them: a long-lived project holds far more
 * of them than open cards, and the board would carry them all in memory.
 */
export function boardInput(project: string, includeSubprojects: boolean, showOlder = false) {
  return { project, include_subprojects: includeSubprojects, show_older: showOlder };
}

/** The board; while the older cards are on their way, the board without them stays. */
export function useBoard(project: string, includeSubprojects: boolean, enabled = true, showOlder = false) {
  const call = useCall();
  const input = boardInput(project, includeSubprojects, showOlder);
  return useQuery<Board, OpFailure>({
    queryKey: opKey("board.get", input),
    queryFn: () => call("board.get", input),
    enabled,
    placeholderData: (previous) => (previous?.project === project ? previous : undefined),
  });
}

export function useLayout(project: string) {
  return useOp("layout.get", { project });
}

/** One task. */
export function useTask(key: string | undefined) {
  return useOp("task.get", { keys: [key ?? ""] }, { enabled: key !== undefined });
}

/** A change to the layout as the screens make it: the fields that change, whole. */
export type LayoutEdit = Partial<BoardLayout>;

const added = (before: readonly string[], after: readonly string[]) => after.filter((id) => !before.includes(id));

/** The engine's input for a layout change. */
export function layoutUpdate(project: string, before: BoardLayout, change: LayoutEdit | "reset"): LayoutUpdateInput {
  if (change === "reset") {
    return {
      project,
      reset: true,
      // Reset brings back the project's columns; the card fields and the rest stay.
      card_fields: before.card_fields,
      hide_done_after_days: before.hide_done_after_days,
      include_subprojects: before.include_subprojects,
    };
  }
  const input: LayoutUpdateInput = { project };
  if (change.order !== undefined) {
    input.order = change.order;
  }
  if (change.hidden !== undefined) {
    input.hide = added(before.hidden, change.hidden);
    input.show = added(change.hidden, before.hidden);
  }
  if (change.collapsed !== undefined) {
    input.collapse = added(before.collapsed, change.collapsed);
    input.expand = added(change.collapsed, before.collapsed);
  }
  if (change.sort_mode !== undefined) {
    input.sort_mode = change.sort_mode;
  }
  if (change.column_sort !== undefined) {
    // A column that goes back to the board's mode loses its own.
    const mode = change.sort_mode ?? before.sort_mode;
    input.column_sort = { ...Object.fromEntries(Object.keys(before.column_sort).map((id) => [id, mode])), ...change.column_sort };
  }
  if (change.hide_done_after_days !== undefined) {
    input.hide_done_after_days = change.hide_done_after_days;
  }
  if (change.card_fields !== undefined) {
    input.card_fields = change.card_fields;
  }
  if (change.include_subprojects !== undefined) {
    input.include_subprojects = change.include_subprojects;
  }
  return input;
}

/**
 * Changes the personal layout. The board follows at once; if the change
 * fails, it goes back and a toast says why.
 */
export function useUpdateLayout(project: string): (change: LayoutEdit | "reset") => void {
  const call = useCall();
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const t = useT();
  const key = opKey("layout.get", { project });
  const mutation = useMutation({
    mutationFn: ({ input }: { input: LayoutUpdateInput; before: LayoutChange }) => call("layout.update", input),
    onError: (error, { before }) => {
      queryClient.setQueryData(key, before);
      toasts.show(errorToast(t, error));
    },
  });
  return (change) => {
    const before = queryClient.getQueryData<LayoutChange>(key);
    if (!before) {
      return;
    }
    if (change !== "reset") {
      queryClient.setQueryData<LayoutChange>(key, { ...before, layout: { ...before.layout, ...change } });
    }
    void queryClient.cancelQueries({ queryKey: key });
    mutation.mutate({ input: layoutUpdate(project, before.layout, change), before });
  };
}

/** Every card on the board, by key. */
export function cardsOf(board: Board): Map<string, Card> {
  return new Map(board.columns.flatMap((column) => column.cards.map((card) => [card.key, card] as const)));
}

/** The column of every card on the board, by card key. */
export function columnsOf(board: Board): Record<string, string> {
  return Object.fromEntries(board.columns.flatMap((column) => column.cards.map((card) => [card.key, column.id] as const)));
}

export interface Move {
  key: string;
  /** The status in the card's own project. */
  status: string;
  /** The column the card lands in. */
  column: string;
  place: Place;
}

const isClosed = (category: string | undefined) => category === "done" || category === "cancelled";

/** Whether a move takes a card from an open column into a closed one. */
export function closesCard(board: Board, move: Move): boolean {
  const from = board.columns.find((column) => column.cards.some((card) => card.key === move.key));
  const to = board.columns.find((column) => column.id === move.column);
  return to !== undefined && isClosed(to.category) && !isClosed(from?.category);
}

/** The board with one card moved, as it will look once the engine agrees. */
export function moveOnBoard(board: Board, move: Move): Board {
  const card = cardsOf(board).get(move.key);
  if (!card) {
    return board;
  }
  const columns = board.columns.map((column) => {
    const had = column.cards.some((candidate) => candidate.key === move.key);
    const cards = column.cards.filter((candidate) => candidate.key !== move.key);
    if (column.id !== move.column) {
      return had ? { ...column, cards, count: column.count - 1 } : column;
    }
    let at = cards.length;
    if ("before" in move.place || "after" in move.place) {
      const anchor = "before" in move.place ? move.place.before : move.place.after;
      const found = cards.findIndex((candidate) => candidate.key === anchor);
      at = found < 0 ? cards.length : found + ("before" in move.place ? 0 : 1);
    } else if (move.place.at === "top") {
      at = 0;
    }
    cards.splice(at, 0, card);
    return { ...column, cards, count: column.count + (had ? 0 : 1) };
  });
  return { ...board, columns };
}

/**
 * Moves a card: the board shows the move at once (in the same frame as the
 * drop, so the card never jumps back), the engine records it, and a failure
 * puts the card back with a toast.
 */
export function useMoveTask(board: { project: string; layout: { include_subprojects: boolean } }, showOlder = false): { mutate: (move: Move) => void } {
  const call = useCall();
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const t = useT();
  const key = opKey("board.get", boardInput(board.project, board.layout.include_subprojects, showOlder));
  const ask = useAskCloseDuplicates();
  const mutation = useMutation({
    mutationFn: async ({ move, before }: { move: Move; before: Board | undefined }) => {
      const close = await ask(move.key, before !== undefined && closesCard(before, move));
      return call("task.move", { key: move.key, status: move.status, ...move.place, ...(close === undefined ? {} : { close_duplicates: close }) });
    },
    onError: (error: OpFailure, { before }) => {
      if (before) {
        queryClient.setQueryData(key, before);
      }
      toasts.show(errorToast(t, error));
    },
  });
  return {
    mutate: (move) => {
      const before = queryClient.getQueryData<Board>(key);
      if (before) {
        queryClient.setQueryData<Board>(key, moveOnBoard(before, move));
      }
      void queryClient.cancelQueries({ queryKey: key });
      mutation.mutate({ move, before });
    },
  };
}

export function useCreateTask() {
  const call = useCall();
  const toasts = useToasts();
  const t = useT();
  return useMutation({
    mutationFn: (input: TaskCreateInput) => call("task.create", input),
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
  });
}

export function useUpdateTask() {
  const call = useCall();
  const toasts = useToasts();
  const t = useT();
  return useMutation({
    mutationFn: (input: TaskUpdateInput) => call("task.update", input),
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
  });
}
