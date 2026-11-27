// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The editor without React: its state, its extensions, and the operations a
 * screen uses. The React component and the tests build on this.
 */

import { autocompletion, completionKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { drawSelection, EditorView, keymap, tooltips, type Rect } from "@codemirror/view";

import { completionProviders, completionSources, editorText } from "./complete";
import { formatKeys } from "./format";
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
  /** The web view checks spelling itself; see `labelExtension`. */
  webViewSpelling?: boolean;
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

export function modeExtension(mode: EditorMode): Extension {
  return mode === "preview" ? livePreview : [];
}

export function spellingExtension(checker: SpellChecker | null): Extension {
  return checker ? spelling(checker) : [];
}

export function readOnlyExtension(readOnly: boolean): Extension {
  return [EditorState.readOnly.of(readOnly), EditorView.editable.of(!readOnly)];
}

/**
 * The text area's name, and whether the web view's own spell checker runs:
 * off where spelling comes from the app, on where the web view checks it
 * itself (WebView2 on Windows).
 */
export function labelExtension(label: string, webViewSpelling = false): Extension {
  return EditorView.contentAttributes.of({ "aria-label": label, "aria-multiline": "true", spellcheck: webViewSpelling ? "true" : "false", autocorrect: "off", autocapitalize: "off" });
}

export function rendererExtension(renderer: BlockRenderer | null): Extension {
  return renderer ? blockRenderer.of(renderer) : [];
}

/** The gap between the cursor's line and the completion list, as in the app's menus. */
const POPUP_GAP = 4;

/**
 * Where the completion list may go: the window's content area (the closest
 * `main`, which leaves out the header and status bars), or the whole window
 * where the editor sits elsewhere (a dialog, a window of its own). The list
 * opens below the cursor when it fits there and above it otherwise, and moves
 * sideways to stay inside.
 */
export function popupSpace(view: EditorView): Rect {
  const root = view.dom.ownerDocument.documentElement;
  let space = { top: 0, left: 0, right: root.clientWidth, bottom: root.clientHeight };
  const area = view.dom.closest("main");
  if (area) {
    const rect = area.getBoundingClientRect();
    space = { top: Math.max(space.top, rect.top), left: Math.max(space.left, rect.left), right: Math.min(space.right, rect.right), bottom: Math.min(space.bottom, rect.bottom) };
  }
  return { top: space.top + POPUP_GAP, left: space.left + POPUP_GAP, right: space.right - POPUP_GAP, bottom: space.bottom - POPUP_GAP };
}

/**
 * Pop-ups (the completion list) live at the end of the page's body, so no
 * scrolling or clipping box around the editor cuts them off.
 */
function popupPlacement(): Extension {
  return tooltips({ position: "fixed", tooltipSpace: popupSpace, ...(typeof document === "undefined" ? {} : { parent: document.body }) });
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
    popupPlacement(),
    // The completion list's accessible name.
    EditorState.phrases.of({ Completions: environment.text("editor-completions") }),
    completionProviders.of(environment.providers),
    editorText.of(environment.text),
    checklistHandler.of(environment.toggle),
    pasting,
    keymap.of([...completionKeymap, ...formatKeys, ...defaultKeymap, ...historyKeymap, indentWithTab]),
    editorTheme,
    compartments.mode.of(modeExtension(options.mode)),
    compartments.readOnly.of(readOnlyExtension(options.readOnly)),
    compartments.spelling.of(spellingExtension(options.spellChecker)),
    compartments.renderer.of(rendererExtension(options.renderBlock)),
    compartments.label.of(labelExtension(options.label, options.webViewSpelling)),
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
