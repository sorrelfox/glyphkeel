// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type OpId, type OpMap } from "../generated/ops";

/** The prefix of every query key that holds an operation's answer. */
export const OP_ROOT = ["op"] as const;

/** Operations that only read: these are cached. */
export type QueryOp = { [K in OpId]: OpMap[K]["kind"] extends "query" ? K : never }[OpId];

/** Operations that change something: these run as mutations. */
export type CommandOp = Exclude<OpId, QueryOp>;

/** The key of all cached answers of one operation, whatever the input. */
export function opPrefix(op: OpId): readonly ["op", OpId] {
  return ["op", op];
}

/** The key of one cached answer: the operation and its input. */
export function opKey<K extends QueryOp>(op: K, input: OpMap[K]["input"]): readonly ["op", K, OpMap[K]["input"]] {
  return ["op", op, input];
}
