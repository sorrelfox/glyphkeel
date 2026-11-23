// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Pasting. HTML (web pages, office documents, spreadsheet cells) becomes
 * Markdown through our turndown rules, loaded on the first HTML paste. Plain
 * text is CodeMirror's own paste. Either way the inserted text gets the
 * document's line break.
 */

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
