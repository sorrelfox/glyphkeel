// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The hooks the project screens use: projects, the board and its layout,
// tasks. Screens call these; these call the transport through the client.

import { useMutation, useQueryClient } from "@tanstack/react-query";

import { useToasts } from "../components/Toasts";
import { useT } from "../i18n/I18n";
import { type Board, type BoardLayout, type Position, type TaskPatch } from "./catalog";
import { errorToast, useCall, useOp } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

export function useProjects() {
  return useOp("project.list", { include_archived: true });
}

export function useCounts() {
  return useOp("workspace.counts", {});
}

export function useProject(key: string | undefined) {
  return useOp("project.get", { project: key ?? "" }, { enabled: key !== undefined });
}

export function boardInput(project: string, includeSubprojects: boolean) {
  return { project, include_subprojects: includeSubprojects };
}

export function useBoard(project: string, includeSubprojects: boolean, enabled = true) {
  return useOp("board.get", boardInput(project, includeSubprojects), { enabled });
}

export function layoutInput(project: string) {
  return { project, view: "board" as const };
}

export function useLayout(project: string) {
  return useOp("layout.get", layoutInput(project));
}

export function useTask(key: string | undefined) {
  return useOp("task.get", { key: key ?? "" }, { enabled: key !== undefined });
}

/**
 * Changes the personal layout. The board follows at once; if the change
 * fails, it goes back and a toast says why.
 */
export function useUpdateLayout(project: string): (change: Partial<BoardLayout> | "reset") => void {
  const call = useCall();
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const t = useT();
  const key = opKey("layout.get", layoutInput(project));
  const mutation = useMutation({
    mutationFn: (change: Partial<BoardLayout> | "reset") =>
      call("layout.update", change === "reset" ? { ...layoutInput(project), reset: true } : { ...layoutInput(project), patch: change }),
    onMutate: async (change) => {
      await queryClient.cancelQueries({ queryKey: key });
      const before = queryClient.getQueryData<BoardLayout>(key);
      if (before && change !== "reset") {
        queryClient.setQueryData<BoardLayout>(key, { ...before, ...change });
      }
      return { before };
    },
    onError: (error, _change, context) => {
      if (context?.before) {
        queryClient.setQueryData(key, context.before);
      }
      toasts.show(errorToast(t, error));
    },
  });
  return (change) => {
    mutation.mutate(change);
  };
}

export interface Move {
  key: string;
  /** The status in the card's own project. */
  status: string;
  /** The column the card lands in. */
  column: string;
  position: Position;
}

/** The board with one card moved, as it will look once the engine agrees. */
export function moveOnBoard(board: Board, move: Move): Board {
  const card = board.cards.find((candidate) => candidate.key === move.key);
  if (!card) {
    return board;
  }
  const moved = { ...card, status: move.status };
  const rest = board.cards.filter((candidate) => candidate.key !== move.key);
  const columnOf = { ...board.column_of, [move.key]: move.column };
  let at: number;
  if (typeof move.position === "object") {
    const anchor = "before" in move.position ? move.position.before : move.position.after;
    at = rest.findIndex((candidate) => candidate.key === anchor) + ("before" in move.position ? 0 : 1);
  } else {
    const inColumn = rest.map((candidate, index) => [candidate, index] as const).filter(([candidate]) => columnOf[candidate.key] === move.column);
    const first = inColumn[0];
    const last = inColumn[inColumn.length - 1];
    at = move.position === "top" ? (first?.[1] ?? rest.length) : last ? last[1] + 1 : rest.length;
  }
  rest.splice(at, 0, moved);
  return { ...board, cards: rest, column_of: columnOf };
}

/**
 * Moves a card: the board shows the move at once (in the same frame as the
 * drop, so the card never jumps back), the engine records it, and a failure
 * puts the card back with a toast.
 */
export function useMoveTask(board: { project: string; include_subprojects: boolean }): { mutate: (move: Move) => void } {
  const call = useCall();
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const t = useT();
  const key = opKey("board.get", boardInput(board.project, board.include_subprojects));
  const mutation = useMutation({
    mutationFn: ({ move }: { move: Move; before: Board | undefined }) =>
      call("task.move", { key: move.key, status: move.status, position: move.position }),
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
    mutationFn: (input: { project: string; title: string; status?: string }) => call("task.create", input),
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
    mutationFn: (input: { key: string; patch: TaskPatch }) => call("task.update", input),
    onError: (error) => {
      toasts.show(errorToast(t, error));
    },
  });
}
