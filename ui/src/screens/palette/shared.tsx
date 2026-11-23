// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// Pieces the command palette shares with the sidebar's search field.

import { useEffect, useState } from "react";

import { findMatch } from "./model";

/** `text` with the part that matches `query` marked. */
export function Highlight({ text, query }: { text: string; query: string }) {
  const found = findMatch(text, query);
  if (!found) {
    return <>{text}</>;
  }
  return (
    <>
      {text.slice(0, found.start)}
      <mark>{text.slice(found.start, found.end)}</mark>
      {text.slice(found.end)}
    </>
  );
}

/** The value of a typing pause: `value`, once it hasn't changed for `delay`. */
export function useSettled<T>(value: T, delay: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setSettled(value);
    }, delay);
    return () => {
      window.clearTimeout(timer);
    };
  }, [value, delay]);
  return settled;
}
