// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The board across projects ("My work"): reading it, and moving a card to
// a status of its own project, shown at once and put back if the engine
// refuses.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useAskCloseDuplicates } from "../components/DuplicatePrompt";
import { useToasts } from "../components/Toasts";
import { type AcrossStatus, type BoardAcross, type BoardAcrossInput, type LaneBy } from "../generated/ops";
import { useT } from "../i18n/I18n";
import { moveAcross } from "../screens/mywork/model";
import { errorToast, useCall } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

/** The engine's input for the board: the view's, with what the person changed on top. */
export function acrossInput(view: string | undefined, query: string, lanes: LaneBy, folder: string | undefined, showOlder: boolean): BoardAcrossInput {
  return {
    ...(view === undefined ? {} : { view }),
    query,
    lanes,
    // An empty folder is "every project", over the view's own.
    folder: folder ?? "",
    show_older: showOlder,
  };
}

/** The board; while a changed query is on its way, the board before it stays. */
export function useAcrossBoard(input: BoardAcrossInput) {
  const call = useCall();
  return useQuery<BoardAcross, OpFailure>({
    queryKey: opKey("board.get_across", input),
    queryFn: () => call("board.get_across", input),
    placeholderData: (previous) => previous,
  });
}

export interface AcrossMove {
  key: string;
  /** The column (status category) the card lands in. */
  column: string;
  status: AcrossStatus;
}

/**
 * Moves a card to a status of its own project: one change, undone in one
 * step. The board shows it at once; a refusal (a status rule, a claim)
 * puts the card back with the engine's message.
 */
export function useMoveAcross(input: BoardAcrossInput): { mutate: (move: AcrossMove) => void } {
  const call = useCall();
  const queryClient = useQueryClient();
  const toasts = useToasts();
  const t = useT();
  const ask = useAskCloseDuplicates();
  const key = opKey("board.get_across", input);
  const mutation = useMutation({
    mutationFn: async ({ move }: { move: AcrossMove; before: BoardAcross | undefined }) => {
      const close = await ask(move.key, move.column === "done");
      return call("task.move", {
        key: move.key,
        status: move.status.id,
        at: "bottom",
        ...(close === undefined ? {} : { close_duplicates: close }),
      });
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
      const before = queryClient.getQueryData<BoardAcross>(key);
      if (before) {
        queryClient.setQueryData<BoardAcross>(key, moveAcross(before, move.key, move.column, move.status));
      }
      void queryClient.cancelQueries({ queryKey: key });
      mutation.mutate({ move, before });
    },
  };
}
