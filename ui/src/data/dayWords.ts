// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// A day typed in words (`next fri`, `+3d`, `dec 1`), read by the engine
// (`date.parse`) as every date field reads it. The page never reads dates
// itself, so what a field shows is what the engine saves.

import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";

import { type ParsedDate } from "../generated/ops";
import { useCall } from "./hooks";
import { opKey } from "./queries";
import { type OpFailure } from "./transport";

/** How long typing pauses before the words are read: short, as the engine is local. */
export const READ_DELAY_MS = 80;

export interface DayInWords {
  /** The day the words mean; null when they mean none, or nothing is typed. */
  day: string | null;
  /** The engine's answer is for the words as typed now. */
  current: boolean;
  /** Why the words mean no day, when they don't. */
  error: OpFailure | null;
  /** Reads `words` now, without waiting: the day, or null. */
  read: (words: string) => Promise<string | null>;
}

/**
 * The day that `text` means, counted from `today` (the person's), read by
 * the engine shortly after typing pauses. The last answer stays while the
 * next is on its way, so the preview doesn't flicker.
 */
export function useDayInWords(text: string, today: string): DayInWords {
  const call = useCall();
  const client = useQueryClient();
  const [settled, setSettled] = useState(text);
  useEffect(() => {
    if (text === settled) {
      return undefined;
    }
    const timer = setTimeout(() => {
      setSettled(text);
    }, READ_DELAY_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [text, settled]);
  const input = { text: settled, today };
  const answer = useQuery<ParsedDate, OpFailure>({
    queryKey: opKey("date.parse", input),
    queryFn: () => call("date.parse", input),
    enabled: settled !== "",
    placeholderData: keepPreviousData,
  });
  const read = useCallback(
    async (words: string) => {
      if (words === "") {
        return null;
      }
      const asked = { text: words, today };
      try {
        const parsed = await client.query({ queryKey: opKey("date.parse", asked), queryFn: () => call("date.parse", asked) });
        return parsed.date;
      } catch {
        return null;
      }
    },
    [call, client, today],
  );
  if (text === "") {
    return { day: null, current: true, error: null, read };
  }
  const current = settled === text && !answer.isPlaceholderData && !answer.isFetching;
  return { day: answer.isError ? null : (answer.data?.date ?? null), current, error: answer.error, read };
}
