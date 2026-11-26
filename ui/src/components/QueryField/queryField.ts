// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The query field without React: a one-line CodeMirror editor whose colours
 * come from the engine's reading of the query (its tokens), with the first
 * mistake underlined, and completion of fields, operators and names. The
 * engine counts in UTF-16 units, as CodeMirror does, so its ranges apply as
 * they are.
 */

import { autocompletion, closeBrackets, completionKeymap, startCompletion, type Completion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { EditorState, RangeSetBuilder, StateEffect, StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView, keymap, placeholder as placeholderText, type DecorationSet } from "@codemirror/view";

import { type QueryField, type QueryProblem, type QueryToken } from "../../generated/ops";

/** The engine's reading of one text: its tokens and its first mistake. */
export interface Reading {
  text: string;
  tokens: readonly QueryToken[];
  error?: QueryProblem | undefined;
}

/** Names that complete values: the workspace's statuses, types, labels and projects. */
export interface QueryNames {
  statuses: readonly string[];
  types: readonly string[];
  labels: readonly string[];
  projects: readonly string[];
}

/** What completion reads while the field runs. */
export interface CompletionEnvironment {
  fields: () => readonly QueryField[] | undefined;
  names: () => QueryNames;
}

/** Hands the field a new reading; it applies only if the text still is what was read. */
export const setReading = StateEffect.define<Reading>();

const MARKS: Record<string, Decoration> = {};
function mark(kind: string): Decoration {
  MARKS[kind] ??= Decoration.mark({ class: `cm-q-${kind}` });
  return MARKS[kind];
}
const ERROR_MARK = Decoration.mark({ class: "cm-q-problem", attributes: { "aria-invalid": "true" } });

/** The decorations of a reading: a colour per token, a wavy line under the mistake. */
export function decorationsOf(reading: Reading, length: number): DecorationSet {
  const ranges: { from: number; to: number; value: Decoration }[] = [];
  for (const token of reading.tokens) {
    const from = Math.max(0, Math.min(token.range.from, length));
    const to = Math.max(from, Math.min(token.range.to, length));
    if (to > from) {
      ranges.push({ from, to, value: mark(token.kind) });
    }
  }
  if (reading.error) {
    const from = Math.max(0, Math.min(reading.error.range.from, length));
    // An empty range (a mistake at the end) underlines the character before it.
    let to = Math.max(from, Math.min(reading.error.range.to, length));
    let start = from;
    if (to === start) {
      start = Math.max(0, start - 1);
      to = Math.min(length, start + 1);
    }
    if (to > start) {
      ranges.push({ from: start, to, value: ERROR_MARK });
    }
  }
  ranges.sort((a, b) => a.from - b.from || a.to - b.to);
  const builder = new RangeSetBuilder<Decoration>();
  for (const range of ranges) {
    builder.add(range.from, range.to, range.value);
  }
  return builder.finish();
}

/** The colours: kept through edits until the next reading of the same text arrives. */
const readingField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(value, transaction) {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (effect.is(setReading) && effect.value.text === transaction.state.doc.toString()) {
        next = decorationsOf(effect.value, transaction.state.doc.length);
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/** One line: line breaks typed or pasted become spaces. */
const oneLine = EditorState.transactionFilter.of((transaction) => {
  if (!transaction.docChanged || transaction.newDoc.lines === 1) {
    return transaction;
  }
  const changes: { from: number; to: number; insert: string }[] = [];
  transaction.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    changes.push({ from: fromA, to: toA, insert: inserted.toString().replace(/\r?\n|\r/g, " ") });
  });
  return [{ changes, selection: transaction.selection, effects: transaction.effects, scrollIntoView: transaction.scrollIntoView }];
});

/** A value as typed in a query: quoted when it holds a space, a comma or a parenthesis. */
export function quoteValue(value: string): string {
  return /[\s,()"]/.test(value) ? `"${value.replace(/"/g, "")}"` : value;
}

/** Values every date field takes, besides dates. */
const DATE_WORDS = ["today", "yesterday", "tomorrow", "+7d", "-7d", "this-week", "next-week", "this-month", "none"];

function valuesFor(field: QueryField, names: QueryNames, fields: readonly QueryField[]): string[] {
  if (field.choices && field.choices.length > 0) {
    return field.choices;
  }
  switch (field.name) {
    case "status":
      return [...names.statuses];
    case "type":
      return [...names.types];
    case "label":
      return [...names.labels, "none"];
    case "project":
      return [...names.projects];
    case "start":
    case "due":
    case "created":
    case "updated":
    case "closed":
      return DATE_WORDS;
    case "author":
    case "changed-by":
      return ["me", "agent"];
    case "sort":
      return fields.filter((candidate) => candidate.sortable).flatMap((candidate) => [candidate.name, `-${candidate.name}`]);
    default:
      return field.values.includes("none") ? ["none"] : [];
  }
}

function findField(fields: readonly QueryField[], name: string): QueryField | undefined {
  const lower = name.toLowerCase();
  return fields.find((field) => field.name === lower || (field.aliases ?? []).includes(lower));
}

/** The field registry's `sort:`, which isn't a field but completes like one. */
const SORT_FIELD: QueryField = { name: "sort", operators: [":"], ranges: false, sortable: false, values: "" };

/**
 * Completion at the cursor: field names at the start of a word (`sta` →
 * `status:`), then the field's values after its operator, one after each
 * comma (`status:"In progress",Re` → `Review`).
 */
export function queryCompletions(environment: CompletionEnvironment) {
  return (context: CompletionContext): CompletionResult | null => {
    const fields = environment.fields();
    if (!fields) {
      return null;
    }
    const before = context.state.sliceDoc(Math.max(0, context.pos - 200), context.pos);
    // A condition on the history names a field of its own: `was:status:Re`, `changed:due>`.
    const history = /(?:^|[\s(])-?(was|changed):([a-zA-Z][a-zA-Z-]*)((?:!=|<=|>=|:|<|>|=)(?:"[^"]*"|[^\s,"()]*,)*)("?[^\s,"()]*)$/i.exec(before);
    if (history) {
      const [, kind = "", inner = "", , partial = ""] = history;
      const field = findField(fields, inner);
      if (!field) {
        return null;
      }
      const values = (kind.toLowerCase() === "changed" ? DATE_WORDS : valuesFor(field, environment.names(), fields)).filter((value) => value !== "none");
      return values.length === 0
        ? null
        : { from: context.pos - partial.length, options: values.map((value) => ({ label: quoteValue(value), type: "constant" })), validFor: /^"?[^\s,"()]*$/ };
    }
    const word = /(?:^|[\s(])(-?)([a-zA-Z][a-zA-Z-]*)?((?:!=|<=|>=|:|<|>|=)(?:"[^"]*"|[^\s,"()]*,)*)?("?[^\s,"()]*)$/.exec(before);
    if (!word) {
      return null;
    }
    const [, , name = "", operator, partial = ""] = word;
    if (operator === undefined) {
      if (name === "" && !context.explicit) {
        return null;
      }
      const options: Completion[] = [...fields, SORT_FIELD].map((field) => ({
        label: `${field.name}:`,
        detail: field.values,
        type: "property",
        boost: field.name === "sort" ? -1 : 0,
        // After a field, its values come next.
        apply: (view, _completion, from, to) => {
          view.dispatch({ changes: { from, to, insert: `${field.name}:` }, selection: { anchor: from + field.name.length + 1 } });
          startCompletion(view);
        },
      }));
      return { from: context.pos - name.length, options, validFor: /^[a-zA-Z-]*$/ };
    }
    const field = name.toLowerCase() === "sort" ? SORT_FIELD : findField(fields, name);
    if (!field) {
      return null;
    }
    const values = valuesFor(field, environment.names(), fields);
    if (values.length === 0) {
      return null;
    }
    // `was:` takes a field and then its value: `status:` comes first, then the statuses.
    if (field.name === "was") {
      return {
        from: context.pos - partial.length,
        options: values.map((value) => ({
          label: `${value}:`,
          type: "property",
          apply: (view, _completion, from, to) => {
            view.dispatch({ changes: { from, to, insert: `${value}:` }, selection: { anchor: from + value.length + 1 } });
            startCompletion(view);
          },
        })),
        validFor: /^[a-zA-Z-]*$/,
      };
    }
    return {
      from: context.pos - partial.length,
      options: values.map((value) => ({ label: quoteValue(value), type: "constant" })),
      validFor: /^"?[^\s,"()]*$/,
    };
  };
}

/** The field's look, from the design tokens: monospace, the syntax colours. */
const theme = EditorView.theme({
  "&": { color: "var(--fg)", fontSize: "var(--fs-small)", flex: "1 1 auto", minWidth: "0" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "20px", overflow: "hidden" },
  ".cm-content": { padding: "4px 0", caretColor: "var(--fg)" },
  ".cm-line": { padding: "0" },
  ".cm-placeholder": { color: "var(--fg-faint)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in srgb, var(--accent) 28%, transparent)",
  },
  ".cm-q-field": { color: "var(--syn-func)" },
  ".cm-q-sort": { color: "var(--syn-func)" },
  ".cm-q-operator, .cm-q-comma, .cm-q-paren": { color: "var(--fg-faint)" },
  ".cm-q-value": { color: "var(--syn-string)" },
  ".cm-q-keyword, .cm-q-negation": { color: "var(--syn-keyword)", fontWeight: "600" },
  ".cm-q-key": { color: "var(--accent-fg)", fontWeight: "550" },
  ".cm-q-text": { color: "var(--fg)" },
  ".cm-q-error": { color: "var(--danger)" },
  ".cm-q-problem": {
    textDecoration: "underline wavy var(--danger)",
    textDecorationSkipInk: "none",
    textUnderlineOffset: "3px",
  },
  ".cm-tooltip": { backgroundColor: "var(--popover)", border: "1px solid var(--border)", borderRadius: "var(--r-m)", color: "var(--fg)", fontFamily: "var(--font-ui)" },
  ".cm-tooltip-autocomplete > ul > li": { padding: "2px 8px" },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "var(--selected)", color: "var(--fg)" },
  ".cm-completionDetail": { color: "var(--fg-muted)", marginInlineStart: "12px", fontStyle: "normal" },
});

export interface QueryFieldOptions {
  label: string;
  placeholder: string;
  environment: CompletionEnvironment;
  /** Enter, when no completion is open. */
  onSubmit: () => void;
  /** Escape, when no completion is open. */
  onEscape: () => void;
  /** Every change of the text. */
  onText: (text: string) => void;
}

/** The field's extensions. */
export function queryFieldExtensions(options: QueryFieldOptions): Extension[] {
  return [
    history(),
    oneLine,
    closeBrackets(),
    readingField,
    autocompletion({ override: [queryCompletions(options.environment)], icons: false, activateOnTyping: true }),
    // Completion's keys first: Enter takes the chosen completion while the list is open.
    keymap.of([
      ...completionKeymap,
      {
        key: "Enter",
        run: () => {
          options.onSubmit();
          return true;
        },
      },
      {
        key: "Escape",
        run: () => {
          options.onEscape();
          return true;
        },
      },
      ...historyKeymap,
      ...defaultKeymap.filter((binding) => binding.key !== "Enter" && binding.key !== "Mod-Enter"),
    ]),
    placeholderText(options.placeholder),
    EditorView.contentAttributes.of({ "aria-label": options.label, role: "textbox", "aria-multiline": "false", spellcheck: "false", autocapitalize: "off", autocorrect: "off" }),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        options.onText(update.state.doc.toString());
      }
    }),
    theme,
  ];
}
