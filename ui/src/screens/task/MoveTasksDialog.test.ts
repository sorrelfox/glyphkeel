// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { type MoveCheck } from "../../generated/ops";
import { mappingOf } from "./MoveTasksDialog";

const CHECK: MoveCheck = {
  project: "OT",
  keys: [{ from: "TM-1", to: "OT-2" }],
  statuses: [{ name: "QA", tasks: 1 }, { name: "Done", tasks: 2, target: "Done" }],
  types: [{ name: "Task", tasks: 3, target: "Task" }],
  labels: [{ name: "ui", tasks: 1 }, { name: "old", tasks: 1 }],
  fields: [{ name: "Area", tasks: 1 }],
  versions: [{ name: "1.0", tasks: 1 }],
  fallback_status: "Backlog",
  fallback_type: "Task",
};

describe("the move's mapping", () => {
  it("names only what has no match, with the defaults until another is chosen, and leaves labels off", () => {
    expect(mappingOf(CHECK, { statuses: {}, types: {}, labels: {} })).toEqual({ statuses: { QA: "Backlog" } });
    expect(mappingOf(CHECK, { statuses: { QA: "Review" }, types: {}, labels: { ui: "frontend" } })).toEqual({ statuses: { QA: "Review" }, labels: { ui: "frontend" } });
  });
});
