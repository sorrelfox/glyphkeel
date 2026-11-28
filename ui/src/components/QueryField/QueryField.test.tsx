// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { act, cleanup, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { type QueryField as FieldInfo } from "../../generated/ops";
import { ReactLocalization } from "@fluent/react";

import { I18n } from "../../i18n/I18n";
import { createBundle } from "../../i18n/i18n";
import { QueryField, type QueryFieldHandle, valuesHint } from "./QueryField";
import { decorationsOf, queryCompletions, queryFieldExtensions, quoteValue, type QueryNames } from "./queryField";

afterEach(cleanup);

function English({ children }: { children: React.ReactNode }) {
  return <I18n locale="en-US">{children}</I18n>;
}

const FIELDS: FieldInfo[] = [
  { name: "status", operators: [":", "!="], ranges: false, sortable: true, values: "status names", values_id: "query-values-status" },
  { name: "priority", aliases: ["prio"], choices: ["none", "low", "normal", "high", "urgent"], operators: [":", "<", ">"], ranges: false, sortable: true, values: "none < low", values_id: "query-values-priority", values_args: { order: "none < low < normal < high < urgent" } },
  { name: "due", operators: [":", "<", ">"], ranges: true, sortable: true, values: "a date", values_id: "query-values-due", values_args: { today: "today", ahead: "+7d", week: "this-week", none: "none" } },
  { name: "label", aliases: ["labels"], operators: [":", "!="], ranges: false, sortable: false, values: "label names, or none", values_id: "query-values-label", values_args: { none: "none" } },
  { name: "was", choices: ["status", "priority", "label"], operators: [":"], ranges: false, sortable: false, values: "a field and a value", values_id: "query-values-was", values_args: { example: "was:status:Review" } },
  { name: "changed", choices: ["status", "due"], operators: [":"], ranges: false, sortable: false, values: "a field", values_id: "query-values-changed", values_args: { example: "changed:due>-7d" } },
  { name: "changed-by", operators: [":", "!="], ranges: false, sortable: false, values: "me, agent", values_id: "query-values-changed-by", values_args: { me: "me", agent: "agent" } },
  { name: "version", aliases: ["fix-version"], operators: [":", "!="], ranges: false, sortable: false, values: "version names, or none", values_id: "query-values-version", values_args: { none: "none" } },
  { name: "cf.stage", choices: ["Alpha", "Beta stage"], operators: [":", "!="], ranges: false, sortable: false, values: "one of the options", values_id: "query-values-custom-select", values_args: { none: "none" } },
];
const NAMES: QueryNames = { statuses: ["To do", "In progress", "Done"], types: ["Task"], labels: ["bug"], projects: ["TM"], versions: ["1.0", "Spring release"] };

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

  it("completes the history: a field after was:, then its values; dates after changed:", () => {
    expect(labels(complete("was:"))).toEqual(["status:", "priority:", "label:"]);
    expect(labels(complete("was:status:"))).toEqual(['"To do"', '"In progress"', "Done"]);
    expect(complete("is:open was:status:Do")?.from).toBe(19);
    expect(labels(complete("-was:prio:"))).toEqual(["low", "normal", "high", "urgent"]);
    expect(labels(complete("changed:"))).toEqual(["status", "due"]);
    expect(labels(complete("changed:due>"))).toContain("-7d");
    expect(labels(complete("changed:due>"))).not.toContain("none");
    expect(labels(complete("changed-by:"))).toEqual(["me", "agent"]);
    expect(complete("was:nothing:")).toBeNull();
  });

  it("offers the next value after a comma, and nothing for unknown fields", () => {
    expect(complete('status:"To do",D')?.from).toBe(15);
    expect(complete("nonsense:")).toBeNull();
    // A space with nothing typed only completes when asked.
    expect(complete("is:open ")).toBeNull();
    expect(labels(complete("is:open ", true))).toContain("status:");
  });

  it("offers the versions after version:, and a select field's options", () => {
    expect(labels(complete("version:"))).toEqual(["1.0", '"Spring release"', "none"]);
    expect(labels(complete("fix-version!="))).toContain("1.0");
    expect(labels(complete("cf.stage:"))).toEqual(["Alpha", '"Beta stage"']);
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
        extensions: queryFieldExtensions({ label: "Filter", placeholder: "", environment: { fields: () => FIELDS, names: () => NAMES }, onSubmit, onEscape: vi.fn(() => true), onText }),
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
    const { rerender } = render(<QueryField {...props} value="status:Doing" problem='No status "Doing".' />, { wrapper: English });
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

describe("help", () => {
  it("gives each field its help as the completion's tooltip", () => {
    const state = EditorState.create({ doc: "sta" });
    const result = queryCompletions({ fields: () => FIELDS, names: () => NAMES, help: (name) => (name === "status" ? "The status by name." : undefined) })(
      new CompletionContext(state, 3, false),
    );
    const status = result?.options.find((option) => option.label === "status:");
    expect(status?.info).toBe("The status by name.");
    expect(result?.options.find((option) => option.label === "due:")?.info).toBeUndefined();
  });
});

describe("value hints", () => {
  const hints = (language: string) => {
    const l10n = new ReactLocalization([createBundle(language)]);
    const state = EditorState.create({ doc: "" });
    const result = queryCompletions({ fields: () => FIELDS, names: () => NAMES, hint: (field) => valuesHint(l10n, field) })(new CompletionContext(state, 0, true));
    // Placeables come isolated for right-to-left text; the words are what counts here.
    return new Map(result?.options.map((option) => [option.label, option.detail?.replace(/[\u2068\u2069]/g, "")]));
  };

  it("shows the values a field takes in the person's language, the query's words as they are", () => {
    const english = hints("en");
    expect(english.get("due:")).toBe("a date, today, +7d, this-week, or none");
    expect(english.get("cf.stage:")).toBe("option names, or none");
    const pseudo = hints("en-XA");
    for (const field of FIELDS) {
      const detail = pseudo.get(`${field.name}:`) ?? "";
      expect(detail, field.name).toMatch(/^\[.*\]$/);
      // What is left of plain English is the query's own words.
      const words = Object.values(field.values_args ?? {}).join(" ");
      const plain = detail.split(/[^A-Za-z]+/).filter((word) => word !== "" && !words.includes(word));
      expect(plain, `${field.name}: ${detail}`).toEqual([]);
    }
    expect(pseudo.get("due:")).toContain("+7d");
    expect(pseudo.get("due:")).toContain("this-week");
  });

  it("falls back to the engine's English without a message or its arguments", () => {
    const l10n = new ReactLocalization([createBundle("en")]);
    const unknown: FieldInfo = { name: "x", operators: [":"], ranges: false, sortable: false, values: "x values", values_id: "query-values-nothing" };
    expect(valuesHint(l10n, unknown)).toBeUndefined();
    const missing: FieldInfo = { ...unknown, values_id: "query-values-due" };
    expect(valuesHint(l10n, missing)).toBeUndefined();
  });
});
