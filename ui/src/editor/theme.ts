// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The editor's look, from the design tokens, so it follows the window's
 * theme, accent and density.
 */

import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

const highlight = HighlightStyle.define([
  { tag: tags.heading, fontWeight: "650" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strong, fontWeight: "700" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: tags.monospace, fontFamily: "var(--font-mono)" },
  { tag: tags.link, color: "var(--accent-fg)" },
  { tag: tags.url, color: "var(--fg-faint)" },
  { tag: tags.processingInstruction, color: "var(--fg-faint)" },
  { tag: tags.quote, color: "var(--fg-muted)" },
  { tag: tags.keyword, color: "var(--syn-keyword)" },
  { tag: tags.string, color: "var(--syn-string)" },
  { tag: tags.number, color: "var(--syn-number)" },
  { tag: tags.comment, color: "var(--syn-comment)" },
]);

const theme = EditorView.theme({
  "&": {
    color: "var(--fg)",
    backgroundColor: "var(--surface)",
    fontSize: "var(--fs-read)",
    height: "100%",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--font-ui)", lineHeight: "var(--lh-read)" },
  ".cm-content": { padding: "12px 16px", caretColor: "var(--fg)" },
  // CodeMirror draws the caret as a left border in every writing direction.
  // eslint-disable-next-line no-restricted-syntax
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--fg)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in srgb, var(--accent) 28%, transparent)",
  },
  ".cm-gk-h1": { fontSize: "var(--fs-h1)", lineHeight: "30px", fontWeight: "700" },
  ".cm-gk-h2": { fontSize: "17px", lineHeight: "24px", fontWeight: "650" },
  ".cm-gk-h3": { fontSize: "14px", fontWeight: "650" },
  ".cm-gk-mark": { color: "var(--fg-faint)" },
  ".cm-gk-code": {
    fontFamily: "var(--font-mono)",
    fontSize: ".9em",
    padding: "1px 4px",
    borderRadius: "var(--r-s)",
    backgroundColor: "var(--sunken)",
  },
  ".cm-gk-codeline": { fontFamily: "var(--font-mono)", fontSize: "12.5px", backgroundColor: "var(--sunken)" },
  ".cm-gk-tableline": { fontFamily: "var(--font-mono)", fontSize: "12.5px" },
  ".cm-gk-rule": { backgroundImage: "linear-gradient(var(--border), var(--border))", backgroundSize: "100% 1px", backgroundPosition: "center", backgroundRepeat: "no-repeat" },
  ".cm-gk-link, .cm-gk-wikilink": { color: "var(--accent-fg)", textDecoration: "underline", textUnderlineOffset: "2px" },
  ".cm-gk-source": { textDecoration: "none" },
  ".cm-gk-taskkey": { color: "var(--accent-fg)", fontWeight: "550" },
  ".cm-gk-math-source": { fontFamily: "var(--font-mono)", fontSize: ".9em" },
  ".cm-gk-done": { color: "var(--fg-muted)", textDecoration: "line-through" },
  ".cm-gk-task": { margin: "0 6px 0 0", verticalAlign: "-2px", cursor: "pointer" },
  ".cm-gk-math-block": { display: "block", textAlign: "center", padding: "6px 0", cursor: "text" },
  ".cm-gk-diagram": { display: "flex", justifyContent: "center", padding: "6px 0", cursor: "text" },
  ".cm-gk-diagram svg": { maxWidth: "100%", height: "auto" },
  ".cm-gk-rendered": { padding: "2px 0", cursor: "text" },
  ".cm-gk-pending": { whiteSpace: "pre-wrap", fontFamily: "var(--font-mono)", fontSize: "12.5px", color: "var(--fg-faint)" },
  ".cm-gk-widget-error": { color: "var(--danger)", fontFamily: "var(--font-mono)", fontSize: "12.5px", whiteSpace: "pre-wrap" },
  ".cm-gk-misspelt": {
    textDecorationLine: "underline",
    textDecorationStyle: "wavy",
    textDecorationColor: "var(--danger)",
    textDecorationSkipInk: "none",
    textUnderlineOffset: "3px",
  },
  ".cm-tooltip": { backgroundColor: "var(--popover)", border: "1px solid var(--border)", borderRadius: "var(--r-m)", color: "var(--fg)" },
  // The completion list looks like the app's menus: the popover surface,
  // rows of the menu height, hover and keyboard highlight as in a menu.
  ".cm-tooltip.cm-tooltip-autocomplete": {
    border: "none",
    borderRadius: "var(--r-l)",
    boxShadow: "var(--shadow-pop)",
    padding: "6px",
    fontFamily: "var(--font-ui)",
    fontSize: "var(--fs)",
    lineHeight: "var(--lh)",
  },
  ".cm-tooltip.cm-tooltip-below": { marginBlockStart: "4px" },
  ".cm-tooltip.cm-tooltip-above": { marginBlockStart: "-4px" },
  ".cm-tooltip.cm-tooltip-autocomplete > ul": {
    fontFamily: "var(--font-ui)",
    minWidth: "200px",
    maxWidth: "min(420px, 90vw)",
    maxHeight: "calc(var(--h-row) * 8)",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li": {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    minHeight: "var(--h-row)",
    padding: "0 8px",
    borderRadius: "var(--r-m)",
    lineHeight: "var(--lh)",
    color: "var(--fg)",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li:hover": { backgroundColor: "var(--hover)" },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected], .cm-tooltip.cm-tooltip-autocomplete-disabled > ul > li[aria-selected]": {
    backgroundColor: "var(--selected)",
    color: "var(--fg)",
  },
  ".cm-completionLabel": { flex: "0 1 auto", minWidth: "0", overflow: "hidden", textOverflow: "ellipsis" },
  ".cm-completionMatchedText": { textDecoration: "none", fontWeight: "600" },
  ".cm-completionDetail": {
    flex: "0 1 auto",
    minWidth: "0",
    marginInlineStart: "auto",
    paddingInlineStart: "12px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    color: "var(--fg-muted)",
    fontSize: "var(--fs-small)",
    fontStyle: "normal",
  },
  ".cm-completionListIncompleteTop:before, .cm-completionListIncompleteBottom:after": { color: "var(--fg-muted)", opacity: "1" },
  "@media (forced-colors: active)": {
    ".cm-tooltip.cm-tooltip-autocomplete": { border: "1px solid CanvasText" },
    ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": { forcedColorAdjust: "none", backgroundColor: "Highlight", color: "HighlightText" },
    ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected] .cm-completionDetail": { color: "HighlightText" },
  },
});

export const editorTheme = [theme, syntaxHighlighting(highlight)];
