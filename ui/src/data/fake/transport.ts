// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type Changed } from "../../generated/ops";
import { type Transport } from "../transport";
import { FakeError, FakeWorkspace } from "./engine";
import { addManyLinks, addManyTasks } from "./many";
import { seed } from "./seed";

export interface FakeOptions {
  /** The day the sample data is built around, and the clock of changes. */
  today?: () => Date;
  /** Milliseconds each call waits, to show loading states; none by default. */
  latency?: number;
  /** Plain tasks added to the sample data, for long lists. */
  extraTasks?: number;
  /** Dates for those tasks and `blocks` links between them, for a large Gantt chart. */
  extraLinks?: number;
}

/**
 * A transport to an in-memory workspace with sample data, for working on the
 * screens without the engine. It answers like the real transports: outputs
 * as plain data, errors as the engine's error objects, and a change event
 * after every command, sent to every subscriber.
 */
export function fakeTransport(options: FakeOptions = {}): Transport & { workspace: FakeWorkspace } {
  const today = options.today ?? (() => new Date());
  const state = seed(today());
  if (options.extraTasks !== undefined && options.extraTasks > 0) {
    addManyTasks(state, options.extraTasks);
    if (options.extraLinks !== undefined && options.extraLinks > 0) {
      addManyLinks(state, options.extraLinks, today());
    }
  }
  const workspace = new FakeWorkspace(state, today);
  const listeners = new Set<(event: Changed) => void>();
  const wait = () => new Promise<void>((resolve) => setTimeout(resolve, options.latency ?? 0));

  return {
    workspace,
    async call(op, input) {
      await wait();
      try {
        const answer = workspace.call(op, structuredClone(input));
        if (answer.changes) {
          const event: Changed = {
            seq: answer.changes.seq,
            group: answer.changes.group,
            origin: { via: "app", actor: { kind: "user", name: "You" } },
            changes: answer.changes.changes,
          };
          // After the answer, as the engine's events arrive after the reply.
          setTimeout(() => {
            for (const listener of listeners) {
              listener(event);
            }
          }, 0);
        }
        return answer.output;
      } catch (error: unknown) {
        if (error instanceof FakeError) {
          // The engine's errors arrive as plain objects, as over the real transports.
          // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- the engine's errors are plain objects
          return Promise.reject(error.error);
        }
        throw error;
      }
    },
    subscribe(onEvent) {
      listeners.add(onEvent);
      return Promise.resolve(() => {
        listeners.delete(onEvent);
      });
    },
  };
}
