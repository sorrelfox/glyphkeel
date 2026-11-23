// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Completion: tasks after "#" (by a word of the title or by key; the key
 * replaces the search), task keys ("TM-4" → "TM-41", or a word of the
 * title), wiki pages and their headings after "[[", and slash commands that
 * insert blocks. Every inserted text gets the document's own line break.
 */

import { insertCompletionText, startCompletion, type Completion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import { Facet, type EditorState } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import { convertLineBreaks } from "./lineEndings";
import type { CompletionProviders, LinkKind } from "./types";

/** The providers, read at the time of each request so that a screen can change them freely. */
export const completionProviders = Facet.define<() => CompletionProviders, () => CompletionProviders>({
  combine: (values) => values[0] ?? (() => ({})),
});

/** Translates the slash commands' names. */
export const editorText = Facet.define<(id: string) => string, (id: string) => string>({
  combine: (values) => values[0] ?? ((id) => id),
});

const CODE_NODES = new Set(["FencedCode", "CodeBlock", "InlineCode", "CodeText", "BlockMath", "InlineMath", "URL", "Autolink"]);

/** Whether the position sits in code or math, where nothing is completed. */
function inCode(state: EditorState, pos: number): boolean {
  for (let node: ReturnType<ReturnType<typeof syntaxTree>["resolveInner"]> | null = syntaxTree(state).resolveInner(pos, -1); node; node = node.parent) {
    if (CODE_NODES.has(node.name)) {
      return true;
    }
  }
  return false;
}

/** Inserts a text written with "\n" using the document's own line break. */
export function insertWithLineBreaks(text: string) {
  return (view: EditorView, _completion: Completion, from: number, to: number): void => {
    view.dispatch(insertCompletionText(view.state, convertLineBreaks(text, view.state.lineBreak), from, to));
  };
}

async function taskKeys(context: CompletionContext): Promise<CompletionResult | null> {
  const match = context.matchBefore(/\b[A-Z][A-Z0-9]{0,9}-[\p{L}\p{N}_]*/u);
  // After "#", the task search answers instead.
  if (!match || inCode(context.state, context.pos) || context.state.sliceDoc(match.from - 1, match.from) === "#") {
    return null;
  }
  const tasks = context.state.facet(completionProviders)().tasks;
  if (!tasks) {
    return null;
  }
  const dash = match.text.indexOf("-");
  const project = match.text.slice(0, dash);
  const query = match.text.slice(dash + 1);
  const found = await tasks(project, query);
  if (context.aborted) {
    return null;
  }
  const options: Completion[] = found.map((task) => ({ label: task.key, detail: task.title, type: "keyword", apply: task.key }));
  return { from: match.from, options, filter: false };
}

/**
 * "#" after a space, a bracket or at the start of a line, then any text
 * without spaces: "#sync", "#TM-4". "##" and "# " (headings) don't count.
 */
export const TASK_TRIGGER = /(?<![^\s([])#[^\s#()[\]]*/u;

async function taskSearch(context: CompletionContext): Promise<CompletionResult | null> {
  const match = context.matchBefore(TASK_TRIGGER);
  if (!match || inCode(context.state, context.pos)) {
    return null;
  }
  const findTasks = context.state.facet(completionProviders)().findTasks;
  if (!findTasks) {
    return null;
  }
  const found = await findTasks(match.text.slice(1));
  if (context.aborted) {
    return null;
  }
  return {
    from: match.from,
    options: found.map((task) => ({ label: task.key, detail: task.title, type: "keyword", apply: task.key })),
    filter: false,
  };
}

async function wikiPages(context: CompletionContext): Promise<CompletionResult | null> {
  const match = context.matchBefore(/\[\[[^\]\r\n|]*/);
  if (!match || inCode(context.state, context.pos)) {
    return null;
  }
  const pages = context.state.facet(completionProviders)().pages;
  if (!pages) {
    return null;
  }
  const found = await pages(match.text.slice(2));
  if (context.aborted) {
    return null;
  }
  const closed = context.state.sliceDoc(context.pos, context.pos + 2) === "]]";
  return {
    from: match.from + 2,
    options: found.map((page) => {
      const { target, detail } = typeof page === "string" ? { target: page, detail: undefined } : page;
      return { label: target, ...(detail === undefined ? {} : { detail }), type: "text", apply: closed ? target : `${target}]]` };
    }),
    filter: false,
  };
}

/** The slash commands: name, message ID of their description, and the Markdown they insert. */
export const SLASH_COMMANDS: readonly { name: string; message: string; text: string }[] = [
  { name: "table", message: "editor-slash-table", text: "| Column | Column |\n| --- | --- |\n|  |  |" },
  { name: "task", message: "editor-slash-task", text: "- [ ] " },
  { name: "code", message: "editor-slash-code", text: "```\n\n```" },
  { name: "math", message: "editor-slash-math", text: "$$\n\n$$" },
  { name: "mermaid", message: "editor-slash-mermaid", text: "```mermaid\nflowchart TD\n  A --> B\n```" },
  { name: "callout", message: "editor-slash-callout", text: "> [!NOTE]\n> " },
  { name: "heading", message: "editor-slash-heading", text: "## " },
];

function slashCommands(context: CompletionContext): CompletionResult | null {
  const match = context.matchBefore(/(?:^|\s)\/\w*/);
  if (!match || inCode(context.state, context.pos)) {
    return null;
  }
  const text = context.state.facet(editorText);
  const from = match.from + match.text.indexOf("/");
  return {
    from,
    options: SLASH_COMMANDS.map((command) => ({
      label: `/${command.name}`,
      detail: text(command.message),
      type: "function",
      apply: insertWithLineBreaks(command.text),
    })),
    validFor: /^\/\w*$/,
  };
}

export const completionSources = [taskSearch, taskKeys, wikiPages, slashCommands];

/**
 * What the toolbar's "Link task" and "Link wiki page" do: type the trigger
 * at the selection and open its search. A selected word becomes the search
 * text; a page link gets its closing brackets at once.
 */
export function startLink(view: EditorView, kind: LinkKind): void {
  const { from, to } = view.state.selection.main;
  const selected = view.state.sliceDoc(from, to);
  // Searching for a selected word replaces it; otherwise the trigger goes after the selection.
  const word = kind === "task" ? /^[^\s#()[\]]+$/u.test(selected) : /^[^\]\r\n|]+$/u.test(selected);
  const start = word ? from : to;
  const before = start === 0 ? "" : view.state.sliceDoc(start - 1, start);
  const space = before === "" || /[\s([]/u.test(before) ? "" : " ";
  const typed = `${space}${kind === "task" ? "#" : "[["}${word ? selected : ""}`;
  view.dispatch({
    changes: { from: start, to, insert: kind === "task" ? typed : `${typed}]]` },
    selection: { anchor: start + typed.length },
    userEvent: "input",
    scrollIntoView: true,
  });
  view.focus();
  startCompletion(view);
}
