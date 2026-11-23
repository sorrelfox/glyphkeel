// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Completion: task keys ("TM-4" → "TM-41", or a word of the title), wiki
 * pages after "[[", and slash commands that insert blocks. Every inserted
 * text gets the document's own line break.
 */

import { insertCompletionText, type Completion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import { Facet, type EditorState } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import { convertLineBreaks } from "./lineEndings";
import type { CompletionProviders } from "./types";

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
  if (!match || inCode(context.state, context.pos)) {
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
    options: found.map((page) => ({ label: page, type: "text", apply: closed ? page : `${page}]]` })),
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

export const completionSources = [taskKeys, wikiPages, slashCommands];
