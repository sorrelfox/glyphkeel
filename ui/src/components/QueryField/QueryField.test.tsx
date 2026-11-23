// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { act, cleanup, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type QueryField as FieldInfo } from "../../generated/ops";
import { QueryField, type QueryFieldHandle } from "./QueryField";
import { decorationsOf, queryCompletions, queryFieldExtensions, quoteValue, type QueryNames } from "./queryField";

afterEach(cleanup);

const FIELDS: FieldInfo[] = [
  { name: "status", operators: [":", "!="], ranges: false, sortable: true, values: "status names" },
  { name: "priority", aliases: ["prio"], choices: ["none", "low", "normal", "high", "urgent"], operators: [":", "<", ">"], ranges: false, sortable: true, values: "none < low" },
  { name: "due", operators: [":", "<", ">"], ranges: true, sortable: true, values: "a date" },
  { name: "label", aliases: ["labels"], operators: [":", "!="], ranges: false, sortable: false, values: "label names" },
];
const NAMES: QueryNames = { statuses: ["To do", "In progress", "Done"], types: ["Task"], labels: ["bug"], projects: ["TM"] };

function complete(text: string, explicit = false): CompletionResult | null {
  const state = EditorState.create({ doc: text });
  const source = queryCompletions({ fields: () => FIELDS, names: () => NAMES });
  return source(new CompletionContext(state, text.length, explicit));
}

const labels = (result: CompletionResult | null) => result?.options.map((option) => option.label) ?? [];

describe("completion", () => {
  it("offers fields at the start of a word, values after the operator", () => {
    expect(labels(complete("sta"))).toContain("status:");
    expect(complete("sta")?.from).toBe(0);
    expect(labels(complete("is:open status:"))).toEqual(['"To do"', '"In progress"', "Done"]);
    expect(complete("is:open status:In")?.from).toBe(15);
    expect(labels(complete("prio>"))).toEqual(["none", "low", "normal", "high", "urgent"]);
    expect(labels(complete("due<"))).toContain("today");
    expect(labels(complete("sort:"))).toEqual(["status", "-status", "priority", "-priority", "due", "-due"]);
  });

  it("offers the next value after a comma, and nothing for unknown fields", () => {
    expect(complete('status:"To do",D')?.from).toBe(15);
    expect(complete("nonsense:")).toBeNull();
    // A space with nothing typed only completes when asked.
    expect(complete("is:open ")).toBeNull();
    expect(labels(complete("is:open ", true))).toContain("status:");
  });

  it("quotes values with spaces", () => {
    expect(quoteValue("In progress")).toBe('"In progress"');
    expect(quoteValue("Done")).toBe("Done");
  });
});

describe("colours and the mistake", () => {
  it("marks every token, and underlines the mistake", () => {
    const set = decorationsOf(
      {
        text: "status:Doing",
        tokens: [
          { kind: "field", range: { from: 0, to: 6 } },
          { kind: "operator", range: { from: 6, to: 7 } },
          { kind: "value", range: { from: 7, to: 12 } },
        ],
        error: { message: "No status", message_id: "query-unknown-status", range: { from: 7, to: 12 } },
      },
      12,
    );
    const marks: string[] = [];
    set.between(0, 12, (from, to, value) => {
      marks.push(`${String(from)}-${String(to)} ${String((value.spec as { class?: string }).class)}`);
    });
    expect(marks).toEqual(["0-6 cm-q-field", "6-7 cm-q-operator", "7-12 cm-q-value", "7-12 cm-q-problem"]);
  });

  it("underlines the last character for a mistake at the end, and ignores ranges past the text", () => {
    const set = decorationsOf({ text: "due<", tokens: [{ kind: "field", range: { from: 0, to: 40 } }], error: { message: "", message_id: "query-missing-value", range: { from: 4, to: 4 } } }, 4);
    const marks: string[] = [];
    set.between(0, 4, (from, to, value) => {
      marks.push(`${String(from)}-${String(to)} ${String((value.spec as { class?: string }).class)}`);
    });
    expect(marks).toEqual(["0-4 cm-q-field", "3-4 cm-q-problem"]);
  });
});

describe("one line", () => {
  it("turns pasted line breaks into spaces and runs Enter and Escape", () => {
    const onSubmit = vi.fn();
    const onText = vi.fn();
    const view = new EditorView({
      state: EditorState.create({
        doc: "",
        extensions: queryFieldExtensions({ label: "Filter", placeholder: "", environment: { fields: () => FIELDS, names: () => NAMES }, onSubmit, onEscape: vi.fn(), onText }),
      }),
    });
    view.dispatch({ changes: { from: 0, insert: "is:open\nlabel:bug\r\nsort:due" } });
    expect(view.state.doc.toString()).toBe("is:open label:bug sort:due");
    expect(view.state.doc.lines).toBe(1);
    expect(onText).toHaveBeenLastCalledWith("is:open label:bug sort:due");
    view.destroy();
  });
});

describe("the field", () => {
  it("shows the problem under the field and marks it invalid", () => {
    const ref = createRef<QueryFieldHandle>();
    const props = {
      onChange: vi.fn(),
      reading: undefined,
      fields: FIELDS,
      names: NAMES,
      label: "Filter query",
      placeholder: "Filter",
      ref,
    };
    const { rerender } = render(<QueryField {...props} value="status:Doing" problem='No status "Doing".' />);
    const field = screen.getByRole("textbox", { name: "Filter query" });
    expect(field.textContent).toBe("status:Doing");
    expect(screen.getByRole("alert").textContent).toBe('No status "Doing".');
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect(field.getAttribute("aria-describedby")).toBe(screen.getByRole("alert").id);

    // A text from outside replaces the typed one; the problem goes.
    rerender(<QueryField {...props} value="status:Done" problem={undefined} />);
    expect(field.textContent).toBe("status:Done");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(field.getAttribute("aria-invalid")).toBeNull();

    // Add condition: a space at the end, the cursor after it.
    act(() => {
      ref.current?.addCondition();
    });
    expect(props.onChange).toHaveBeenLastCalledWith("status:Done ");
  });
});
