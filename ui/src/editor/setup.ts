// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The editor without React: its state, its extensions, and the operations a
 * screen uses. The React component and the tests build on this.
 */

import { autocompletion, completionKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { Compartment, EditorSelection, EditorState, type Extension } from "@codemirror/state";
import { drawSelection, EditorView, keymap, type KeyBinding } from "@codemirror/view";

import { completionProviders, completionSources, editorText } from "./complete";
import { checkEdits, convertLineBreaks, type LineBreak, type TextEdit } from "./lineEndings";
import { livePreview } from "./livePreview";
import { pasting } from "./paste";
import { spelling } from "./spell";
import { glyphkeelMarkdown } from "./syntax";
import { editorTheme } from "./theme";
import type { BlockRenderer, ChecklistToggle, CompletionProviders, EditorMode, SpellChecker } from "./types";
import { blockRenderer, checklistHandler } from "./widgets";

/** What the editor's extensions read while it runs. */
export interface EditorEnvironment {
  /** The current completion providers. */
  providers: () => CompletionProviders;
  /** Translates a message ID. */
  text: (id: string) => string;
  /** Receives checkbox clicks when the engine owns the text; `null` lets the editor change the text. */
  toggle: () => ((toggle: ChecklistToggle) => void) | null;
}

export interface EditorOptions {
  lineBreak: LineBreak;
  mode: EditorMode;
  readOnly: boolean;
  spellChecker: SpellChecker | null;
  renderBlock: BlockRenderer | null;
  label: string;
  environment: EditorEnvironment;
  /** Extra extensions, for example the change listener. */
  extensions?: Extension[];
}

/** Parts of the set-up that can change while the editor runs. */
export const compartments = {
  mode: new Compartment(),
  readOnly: new Compartment(),
  spelling: new Compartment(),
  renderer: new Compartment(),
  label: new Compartment(),
};

/** Wraps each selection in markers, or puts the cursor between them. */
function wrapSelection(before: string, after: string) {
  return (view: EditorView): boolean => {
    if (view.state.readOnly) {
      return false;
    }
    const transaction = view.state.changeByRange((range) => ({
      changes: [
        { from: range.from, insert: before },
        { from: range.to, insert: after },
      ],
      range: range.empty
        ? EditorSelection.cursor(range.from + before.length)
        : EditorSelection.range(range.from + before.length, range.to + before.length),
    }));
    view.dispatch(transaction, { userEvent: "input.format" });
    return true;
  };
}

/** Markdown keys on top of CodeMirror's own: Ctrl+B bold, Ctrl+I italic, Ctrl+E code, Ctrl+K link. */
const formattingKeys: KeyBinding[] = [
  { key: "Mod-b", run: wrapSelection("**", "**") },
  { key: "Mod-i", run: wrapSelection("*", "*") },
  { key: "Mod-e", run: wrapSelection("`", "`") },
  { key: "Mod-k", run: wrapSelection("[", "]()") },
];

export function modeExtension(mode: EditorMode): Extension {
  return mode === "preview" ? livePreview : [];
}

export function spellingExtension(checker: SpellChecker | null): Extension {
  return checker ? spelling(checker) : [];
}

export function readOnlyExtension(readOnly: boolean): Extension {
  return [EditorState.readOnly.of(readOnly), EditorView.editable.of(!readOnly)];
}

export function labelExtension(label: string): Extension {
  // The web view's own spell checker stays off; spelling comes from the app.
  return EditorView.contentAttributes.of({ "aria-label": label, "aria-multiline": "true", spellcheck: "false", autocorrect: "off", autocapitalize: "off" });
}

export function rendererExtension(renderer: BlockRenderer | null): Extension {
  return renderer ? blockRenderer.of(renderer) : [];
}

/** Every extension of the editor. */
export function editorExtensions(options: EditorOptions): Extension[] {
  const { environment } = options;
  return [
    // Lines are joined with the text's own break; see lineEndings.ts.
    EditorState.lineSeparator.of(options.lineBreak),
    history(),
    drawSelection(),
    EditorView.lineWrapping,
    glyphkeelMarkdown(),
    autocompletion({ override: completionSources, icons: false }),
    completionProviders.of(environment.providers),
    editorText.of(environment.text),
    checklistHandler.of(environment.toggle),
    pasting,
    keymap.of([...completionKeymap, ...formattingKeys, ...defaultKeymap, ...historyKeymap, indentWithTab]),
    editorTheme,
    compartments.mode.of(modeExtension(options.mode)),
    compartments.readOnly.of(readOnlyExtension(options.readOnly)),
    compartments.spelling.of(spellingExtension(options.spellChecker)),
    compartments.renderer.of(rendererExtension(options.renderBlock)),
    compartments.label.of(labelExtension(options.label)),
    ...(options.extensions ?? []),
  ];
}

/** Positions in the editor for UTF-16 offsets in the text: a CRLF is one position there. */
function offsetMapper(text: string, lineBreak: string): (offset: number) => number | null {
  if (lineBreak !== "\r\n") {
    return (offset) => offset;
  }
  const breaks: number[] = [];
  for (let i = text.indexOf("\r\n"); i >= 0; i = text.indexOf("\r\n", i + 2)) {
    breaks.push(i);
  }
  return (offset) => {
    // How many breaks end at or before the offset; an offset between "\r" and "\n" is refused.
    let low = 0;
    let high = breaks.length;
    while (low < high) {
      const mid = (low + high) >> 1;
      if ((breaks[mid] ?? 0) < offset) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }
    if (low > 0 && (breaks[low - 1] ?? -2) + 1 === offset) {
      return null;
    }
    return offset - low;
  };
}

/**
 * Applies edits given in UTF-16 offsets of the text, with their line breaks
 * converted to the text's own. Changes nothing and returns false when the
 * edits are refused.
 */
export function applyTextEdits(view: EditorView, edits: readonly TextEdit[]): boolean {
  const { state } = view;
  const text = state.sliceDoc();
  if (checkEdits(text, edits) !== null) {
    return false;
  }
  const toPos = offsetMapper(text, state.lineBreak);
  const changes = [];
  for (const edit of edits) {
    const from = toPos(edit.from);
    const to = toPos(edit.to);
    if (from === null || to === null) {
      return false;
    }
    changes.push({ from, to, insert: convertLineBreaks(edit.insert, state.lineBreak) });
  }
  view.dispatch({ changes, userEvent: "input.external" });
  return true;
}
