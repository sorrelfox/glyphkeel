// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type Changed } from "../../generated/ops";
import { type Transport } from "../transport";
import { FakeError, FakeWorkspace } from "./engine";
import { addManyLinks, addManyTasks } from "./many";
import { seed } from "./seed";
import { addSampleWiki } from "./wiki";
import { registerFakeWorkspaces, WORKSPACE_NAME } from "./workspaces";

export interface FakeOptions {
  /** The day the sample data is built around, and the clock of changes. */
  today?: () => Date;
  /** Milliseconds each call waits, to show loading states; none by default. */
  latency?: number;
  /** Plain tasks added to the sample data, for long lists. */
  extraTasks?: number;
  /** Sample wiki pages in Task Manager, for the wiki's screens. */
  sampleWiki?: boolean;
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
  if (options.sampleWiki === true) {
    addSampleWiki(state, today());
  }
  // Several workspaces, by name, as the app's data folder holds them; the
  // sample data is the first, and a new one starts empty.
  const workspaces = new Map<string, FakeWorkspace>([[state.workspaceName, new FakeWorkspace(state, today)]]);
  let current = state.workspaceName;
  const open = () => workspaces.get(current) ?? fail("not_found", "There is no workspace.");
  const listeners = new Set<(event: Changed) => void>();
  // Everything changed: the page reads it all again.
  const reopened = () => {
    const event: Changed = {
      seq: 0,
      group: "",
      origin: { via: "app", actor: { kind: "user", name: "You" } },
      changes: [{ entity: "workspace", id: "restored", fields: ["*"] }],
    };
    setTimeout(() => {
      for (const listener of listeners) {
        listener(event);
      }
    }, 0);
  };
  const view = () => ({
    current,
    elsewhere: false,
    items: [...workspaces.keys()].sort((a, b) => a.localeCompare(b)).map((name) => ({ name, current: name === current })),
  });
  const checked = (name: string) => {
    const trimmed = name.trim();
    if (!WORKSPACE_NAME.test(trimmed)) {
      fail("invalid", `"${trimmed}" can't be the name of a workspace.`, { message_id: "workspace-name-invalid", args: { name: trimmed } });
    }
    if ([...workspaces.keys()].some((other) => other.toLowerCase() === trimmed.toLowerCase())) {
      fail("conflict", `There is already a workspace called "${trimmed}".`, { message_id: "workspace-exists", args: { name: trimmed } });
    }
    return trimmed;
  };
  registerFakeWorkspaces({
    view,
    create(name) {
      const wanted = checked(name);
      const empty = seed(today());
      empty.projects = [];
      empty.tasks = [];
      empty.relations = [];
      empty.workspaceName = wanted;
      workspaces.set(wanted, new FakeWorkspace(empty, today));
      current = wanted;
      reopened();
      return view();
    },
    open(name) {
      if (!workspaces.has(name)) {
        fail("not_found", `There is no workspace "${name}".`, { message_id: "workspace-not-found", args: { name } });
      }
      if (name !== current) {
        current = name;
        reopened();
      }
      return view();
    },
    rename(old, name) {
      const found = workspaces.get(old) ?? fail("not_found", `There is no workspace "${old}".`, { message_id: "workspace-not-found", args: { name: old } });
      const wanted = name.trim() === old ? old : checked(name);
      workspaces.delete(old);
      workspaces.set(wanted, found);
      found.rename(wanted);
      if (current === old) {
        current = wanted;
        reopened();
      }
      return view();
    },
  });
  const wait = () => new Promise<void>((resolve) => setTimeout(resolve, options.latency ?? 0));

  return {
    get workspace() {
      return open();
    },
    async call(op, input) {
      await wait();
      try {
        const answer = open().call(op, structuredClone(input));
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

/** Throws an error as the app's workspace commands do, with its message's ID for the page's language. */
function fail(code: "invalid" | "conflict" | "not_found", message: string, details?: { message_id: string; args: Record<string, string> }): never {
  throw new FakeError(code, message, details ? { details } : {});
}
