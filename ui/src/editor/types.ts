// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The editor's public types: what a screen passes in and what it gets back.
 * Kept free of CodeMirror imports, so that a screen can use them without
 * loading the editor's chunk.
 */

import type { Ref } from "react";

import type { LineBreak, LineBreakCounts, TextEdit } from "./lineEndings";

export type { LineBreak, LineBreakCounts, TextEdit };

/** The block kinds whose HTML comes from the Markdown renderer. */
export type RenderedBlockKind = "table" | "callout";

/**
 * Turns the Markdown source of one block into sanitized HTML. The app passes
 * the Rust renderer, over IPC; the editor sanitizes the answer once more and
 * caches it by source text.
 */
export type BlockRenderer = (source: string, kind: RenderedBlockKind) => Promise<string>;

/** A task the person may link to by its key. */
export interface TaskSuggestion {
  key: string;
  title: string;
}

/**
 * Completion sources. Each is asked as the person types, with the text typed
 * so far, and may answer from a cache or over IPC.
 */
export interface CompletionProviders {
  /** Task keys: `query` is what follows the project key's dash, e.g. "4" for "TM-4", or a title word. */
  tasks?: (projectKey: string, query: string) => Promise<readonly TaskSuggestion[]> | readonly TaskSuggestion[];
  /** Wiki pages, for `[[`: `query` is the text typed after the brackets. */
  pages?: (query: string) => Promise<readonly string[]> | readonly string[];
}

/** A click on a checkbox, when the engine owns the text. */
export interface ChecklistToggle {
  /** The item's number in the text, counting every task item in order from 0. */
  index: number;
  /** Where the item's "[" is in the text, in UTF-16 code units (a CRLF counts two). */
  offset: number;
  /** Whether the item is ticked now, before the click. */
  checked: boolean;
  /** The item's text after the checkbox, trimmed. */
  text: string;
}

/** Spelling, as the app provides it. */
export interface SpellChecker {
  /**
   * Checks prose. Each chunk is a start position and its text, with code,
   * links, math, task keys and wiki links already blanked out by spaces.
   * Answers misspelt words as [from, to] in the same (UTF-16) positions.
   */
  check(chunks: readonly (readonly [number, string])[]): Promise<readonly (readonly [number, number])[]>;
  /** Suggestions for one word, best first. */
  suggest(word: string): Promise<readonly string[]>;
}

/** Said once when a text with mixed line breaks was converted on opening. */
export interface LineBreakNote {
  from: LineBreakCounts;
  to: LineBreak;
}

export type EditorMode = "preview" | "source";

export interface MarkdownEditorProps {
  /** The text. The editor reports every change through `onChange`, byte for byte. */
  value: string;
  onChange?: (value: string) => void;
  /** Live Preview (the default) or the plain Markdown source. */
  mode?: EditorMode;
  /**
   * When given, a click on a checkbox calls this instead of changing the
   * text: the engine toggles the item, and the new text comes back through
   * `value`. Without it, the click changes exactly one character.
   */
  onToggleChecklist?: (toggle: ChecklistToggle) => void;
  /** Renders tables and callouts; a stand-in that shows the source is used when absent. */
  renderBlock?: BlockRenderer;
  completion?: CompletionProviders;
  /** Spelling; the app's spell checker in the desktop app, none in a plain browser. */
  spellChecker?: SpellChecker | null;
  /** Called when mixed line breaks were converted on opening, in addition to the note the editor shows. */
  onLineBreaksConverted?: (note: LineBreakNote) => void;
  readOnly?: boolean;
  autoFocus?: boolean;
  /** The accessible name of the text area. */
  label: string;
  className?: string | undefined;
  ref?: Ref<MarkdownEditorHandle>;
}

/** What a screen can do with a mounted editor. */
export interface MarkdownEditorHandle {
  focus(): void;
  /** The current text, exactly. */
  text(): string;
  /**
   * Applies edits made elsewhere (an agent's targeted edits shown live),
   * with their line breaks converted to the text's own. Returns false, and
   * changes nothing, when an edit is out of range, overlaps another, or would
   * split a surrogate pair.
   */
  applyEdits(edits: readonly TextEdit[]): boolean;
  /** Inserts text at the selection, with its line breaks converted. */
  insert(text: string): void;
}
