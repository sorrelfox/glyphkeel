// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The editor's public types: what a screen passes in and what it gets back.
 * Kept free of CodeMirror imports, so that a screen can use them without
 * loading the editor's chunk.
 */

import type { Ref } from "react";

import type { FormatKind } from "./formats";
import type { LineBreak, LineBreakCounts, TextEdit } from "./lineEndings";

export type { FormatKind, LineBreak, LineBreakCounts, TextEdit };

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

/** A wiki page, or one heading of it, that a `[[` link may point at. */
export interface PageSuggestion {
  /** What goes between the brackets: `Page` or `Page#Heading`. */
  target: string;
  /** More about it: its project, or the heading's page. */
  detail?: string | undefined;
}

/**
 * Completion sources. Each is asked as the person types, with the text typed
 * so far, and may answer from a cache or over IPC.
 */
export interface CompletionProviders {
  /** Task keys: `query` is what follows the project key's dash, e.g. "4" for "TM-4", or a title word. */
  tasks?: (projectKey: string, query: string) => Promise<readonly TaskSuggestion[]> | readonly TaskSuggestion[];
  /**
   * Tasks found by a word of the title or by key, for `#`: `query` is the
   * text typed after the `#`, which may be empty.
   */
  findTasks?: (query: string) => Promise<readonly TaskSuggestion[]> | readonly TaskSuggestion[];
  /**
   * Wiki pages, for `[[`: `query` is the text typed after the brackets;
   * `Page#Heading` asks for the headings of a page.
   */
  pages?: (query: string) => Promise<readonly (string | PageSuggestion)[]> | readonly (string | PageSuggestion)[];
}

/** The searches a toolbar button can open in the text. */
export type LinkKind = "task" | "page";

/** A click on a checkbox, when the engine owns the text. */
export interface ChecklistToggle {
  /**
   * The item's number, as the engine's checklist and the reader's
   * `data-item` count it: from 1, in reading order. Pass it with `text` to
   * `checklist.update`.
   */
  number: number;
  /** Where the item's "[" is in the text, in UTF-16 code units (a CRLF counts two). */
  offset: number;
  /** Whether the item is ticked now, before the click. */
  checked: boolean;
  /** The item's text after the checkbox as plain text, as the engine reports it. */
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
  /**
   * Files pasted or dropped on the editor: attaches them and answers the
   * Markdown to insert for them. Without it, files are left to the page.
   */
  onFiles?: (files: readonly File[]) => Promise<string>;
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
  /**
   * Inserts text at the selection, or at position `at` of the text, with its
   * line breaks converted.
   */
  insert(text: string, at?: number): void;
  /**
   * Applies a formatting command (bold, a list, a code block…) to the
   * selection, as its shortcut does, and keeps the focus in the text.
   */
  format(kind: FormatKind): void;
  /**
   * Opens the search for a link at the selection, as typing its trigger
   * does: `#` for a task, `[[` for a wiki page.
   */
  startLink(kind: LinkKind): void;
  /** The position in the text under a point of the window, if the point is on the text. */
  positionAt(x: number, y: number): number | null;
}
