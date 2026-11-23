// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Pasting. HTML (web pages, office documents, spreadsheet cells) becomes
 * Markdown through our turndown rules, loaded on the first HTML paste. Plain
 * text is CodeMirror's own paste. Either way the inserted text gets the
 * document's line break.
 */

import { Prec, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { convertLineBreaks } from "./lineEndings";

let converter: Promise<(html: string) => string> | null = null;

/** Loads the HTML converter once. */
export function loadHtmlToMarkdown(): Promise<(html: string) => string> {
  converter ??= import("./turndown").then((module) => module.htmlToMarkdown);
  return converter;
}

/** Inserts Markdown at the selection, with the document's own line breaks. */
export function insertMarkdown(view: EditorView, markdown: string, userEvent = "input.paste"): void {
  const text = convertLineBreaks(markdown, view.state.lineBreak);
  view.dispatch(view.state.replaceSelection(text), { userEvent, scrollIntoView: true });
}

/** Converts pasted HTML and inserts it. Returns when the text is in. */
export async function pasteHtml(view: EditorView, html: string): Promise<void> {
  const toMarkdown = await loadHtmlToMarkdown();
  insertMarkdown(view, toMarkdown(html));
}

const htmlPaste = EditorView.domEventHandlers({
  paste(event, view) {
    const html = event.clipboardData?.getData("text/html");
    if (!html || view.state.readOnly) {
      return false;
    }
    event.preventDefault();
    void pasteHtml(view, html);
    return true;
  },
});

/** Plain pastes and drops get the document's line break too. */
const plainPaste = EditorView.clipboardInputFilter.of((text, state) => convertLineBreaks(text, state.lineBreak));

export const pasting = [htmlPaste, plainPaste];

/** Attaches pasted or dropped files and answers the Markdown that refers to them. */
export type FileHandler = (files: readonly File[]) => Promise<string>;

/**
 * Files pasted or dropped on the editor: `handler` (when there is one)
 * attaches them, and the Markdown it answers goes in at the cursor (for a
 * drop, where they were dropped). Without a handler, or without files, the
 * paste or drop is the editor's usual one.
 */
export function filePasting(handler: () => FileHandler | null): Extension {
  const take = (view: EditorView, files: readonly File[], at: number | null): boolean => {
    const attach = handler();
    if (files.length === 0 || !attach || view.state.readOnly) {
      return false;
    }
    if (at !== null) {
      view.dispatch({ selection: { anchor: at } });
    }
    void attach(files).then((markdown) => {
      if (markdown !== "" && !view.state.readOnly) {
        insertMarkdown(view, markdown, "input.paste");
      }
    });
    return true;
  };
  return Prec.high(
    EditorView.domEventHandlers({
      paste(event, view) {
        const taken = take(view, [...(event.clipboardData?.files ?? [])], null);
        if (taken) {
          event.preventDefault();
        }
        return taken;
      },
      drop(event, view) {
        const at = view.posAtCoords({ x: event.clientX, y: event.clientY });
        const taken = take(view, [...(event.dataTransfer?.files ?? [])], at);
        if (taken) {
          event.preventDefault();
          // The task panel attaches dropped files too; this drop is the editor's.
          event.stopPropagation();
        }
        return taken;
      },
    }),
  );
}
