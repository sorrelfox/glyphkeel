// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type AppOpId, type AppOpMap } from "./catalog";

/** The prefix of every query key that holds an operation's answer. */
export const OP_ROOT = ["op"] as const;

/** Operations that only read: these are cached. */
export type QueryOp = { [K in AppOpId]: AppOpMap[K]["kind"] extends "query" ? K : never }[AppOpId];

/** Operations that change something: these run as mutations. */
export type CommandOp = Exclude<AppOpId, QueryOp>;

/** The key of all cached answers of one operation, whatever the input. */
export function opPrefix(op: AppOpId): readonly ["op", AppOpId] {
  return ["op", op];
}

/** The key of one cached answer: the operation and its input. */
export function opKey<K extends QueryOp>(op: K, input: AppOpMap[K]["input"]): readonly ["op", K, AppOpMap[K]["input"]] {
  return ["op", op, input];
}
