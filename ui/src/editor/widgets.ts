// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Live Preview's widgets. Each draws one span or block on its own; none
 * changes the text, except the checkbox, which changes exactly one character
 * (or asks the engine to, when the engine owns the text).
 */

import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import type { SyntaxNode } from "@lezer/common";
import { Facet, type EditorState } from "@codemirror/state";
import { EditorView, WidgetType } from "@codemirror/view";
import { insertSanitized } from "../markdown/insert";
import { drawMath, renderDiagram } from "../markdown/libraries";
import type { BlockRenderer, ChecklistToggle, RenderedBlockKind } from "./types";

// ---------------------------------------------------------------- settings

/** Where checkbox clicks go: `null` changes the text itself. */
export const checklistHandler = Facet.define<() => ((toggle: ChecklistToggle) => void) | null, () => ((toggle: ChecklistToggle) => void) | null>({
  combine: (values) => values[0] ?? (() => null),
});

const escapeHtml = (text: string) =>
  text.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c);

/** The stand-in renderer: the block's source, as preformatted text. */
export const placeholderRenderer: BlockRenderer = (source) => Promise.resolve(`<pre>${escapeHtml(source)}</pre>`);

/** Renders tables and callouts. */
export const blockRenderer = Facet.define<BlockRenderer, BlockRenderer>({
  combine: (values) => values[0] ?? placeholderRenderer,
});

/** Puts sanitized HTML into an element, through the app's one insertion point. */
function setSanitized(element: HTMLElement, html: string, svg = false): void {
  insertSanitized(element, html, svg ? "diagram" : "reader");
}

/** A click on a block widget puts the cursor into its source. */
function openOnMouseDown(element: HTMLElement, view: EditorView): void {
  element.addEventListener("mousedown", (event) => {
    event.preventDefault();
    const pos = view.posAtDOM(element);
    view.dispatch({ selection: { anchor: pos }, scrollIntoView: true });
    view.focus();
  });
}

// ---------------------------------------------------------------- checkboxes

/**
 * An item's text as the engine reports it: its inline Markdown as plain
 * text, with spaces collapsed. The engine uses it to check that a number
 * still names the item that was clicked.
 */
export function plainItemText(source: string): string {
  return source
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[\[([^\]|#]+)(?:#[^\]|]*)?(?:\|([^\]]*))?\]\]/g, (_all, page: string, label?: string) => label ?? page)
    .replace(/<[^>]*>/g, "")
    .replace(/(\*\*|__|~~|\*|_|`)/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * The item's number at `pos` (its "["), as the engine and the reader's
 * `data-item` count: from 1, in reading order. Lezer and the engine agree on
 * which list items are task items in ordinary text; where they could differ
 * (footnotes, which the engine numbers after the body), the item's text
 * guards the click. 0 when there is no item at `pos`.
 */
export function checklistNumber(state: EditorState, pos: number): number {
  const tree = ensureSyntaxTree(state, state.doc.length, 200);
  if (!tree) {
    return 0;
  }
  let count = 0;
  let found = 0;
  tree.iterate({
    enter(node) {
      if (found > 0) {
        return false;
      }
      if (node.name === "TaskMarker") {
        count++;
        if (node.from === pos) {
          found = count;
        }
      }
      return undefined;
    },
  });
  return found;
}

/** The toggle a click at `pos` (the "[") asks for. */
export function checklistToggleAt(state: EditorState, pos: number): ChecklistToggle {
  const checked = state.sliceDoc(pos + 1, pos + 2) !== " ";
  const line = state.doc.lineAt(pos);
  const offset = pos + (state.lineBreak.length - 1) * (line.number - 1);
  // The item's text is its whole paragraph, which may go on over several lines.
  let end = line.to;
  for (let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, 1); node; node = node.parent) {
    if (node.name === "Task") {
      end = node.to;
      break;
    }
  }
  return { number: checklistNumber(state, pos), offset, checked, text: plainItemText(state.sliceDoc(pos + 3, end)) };
}

/**
 * Ticks or unticks the item whose "[" is at `pos`: exactly one character
 * changes, as the engine's own toggle does ("[ ]" ↔ "[x]", and "[X]" → "[ ]").
 */
export function toggleChecklistItem(view: EditorView, pos: number): void {
  const handler = view.state.facet(checklistHandler)();
  if (handler) {
    handler(checklistToggleAt(view.state, pos));
    return;
  }
  const insert = view.state.sliceDoc(pos + 1, pos + 2) === " " ? "x" : " ";
  view.dispatch({ changes: { from: pos + 1, to: pos + 2, insert }, userEvent: "input.toggle" });
}

export class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }
  override eq(other: CheckboxWidget): boolean {
    return other.checked === this.checked;
  }
  toDOM(view: EditorView): HTMLElement {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.className = "cm-gk-task";
    box.checked = this.checked;
    box.tabIndex = -1;
    box.addEventListener("mousedown", (event) => {
      event.preventDefault();
      toggleChecklistItem(view, view.posAtDOM(box));
    });
    // The text decides the state; a click must not flip the box by itself.
    box.addEventListener("click", (event) => {
      event.preventDefault();
    });
    return box;
  }
  override ignoreEvent(): boolean {
    return true;
  }
}

// ---------------------------------------------------------------- math

export class MathWidget extends WidgetType {
  constructor(
    readonly tex: string,
    readonly display: boolean,
  ) {
    super();
  }
  override eq(other: MathWidget): boolean {
    return other.tex === this.tex && other.display === this.display;
  }
  toDOM(view: EditorView): HTMLElement {
    const element = document.createElement(this.display ? "div" : "span");
    element.className = this.display ? "cm-gk-math-block" : "cm-gk-math";
    element.textContent = this.tex;
    void drawMath(element, this.tex, this.display).then(() => {
      element.classList.toggle("cm-gk-widget-error", element.classList.contains("gk-render-error"));
      view.requestMeasure();
    });
    openOnMouseDown(element, view);
    return element;
  }
  override get estimatedHeight(): number {
    return this.display ? 48 : -1;
  }
  override ignoreEvent(): boolean {
    return true;
  }
}

// ---------------------------------------------------------------- Mermaid

const diagramCache = new Map<string, string>();
const CACHE_LIMIT = 200;

function remember(cache: Map<string, string>, key: string, value: string): void {
  if (cache.size >= CACHE_LIMIT) {
    const oldest = cache.keys().next();
    if (!oldest.done) {
      cache.delete(oldest.value);
    }
  }
  cache.set(key, value);
}

export class MermaidWidget extends WidgetType {
  constructor(readonly source: string) {
    super();
  }
  override eq(other: MermaidWidget): boolean {
    return other.source === this.source;
  }
  toDOM(view: EditorView): HTMLElement {
    const element = document.createElement("div");
    element.className = "cm-gk-diagram";
    const cached = diagramCache.get(this.source);
    if (cached !== undefined) {
      setSanitized(element, cached, true);
    } else {
      element.textContent = this.source;
      element.classList.add("cm-gk-pending");
      renderDiagram(this.source)
        .then((svg) => {
          remember(diagramCache, this.source, svg);
          element.classList.remove("cm-gk-pending");
          setSanitized(element, svg, true);
          view.requestMeasure();
        })
        .catch((error: unknown) => {
          element.classList.remove("cm-gk-pending");
          element.textContent = error instanceof Error ? error.message : String(error);
          element.classList.add("cm-gk-widget-error");
        });
    }
    openOnMouseDown(element, view);
    return element;
  }
  override get estimatedHeight(): number {
    return 200;
  }
  override ignoreEvent(): boolean {
    return true;
  }
}

// ---------------------------------------------------------------- rendered blocks

const renderCaches = new WeakMap<BlockRenderer, Map<string, string>>();

function renderCache(renderer: BlockRenderer): Map<string, string> {
  let cache = renderCaches.get(renderer);
  if (!cache) {
    cache = new Map();
    renderCaches.set(renderer, cache);
  }
  return cache;
}

/** A table or callout drawn from the renderer's HTML, with its source shown until the HTML arrives. */
export class RenderedBlockWidget extends WidgetType {
  constructor(
    readonly source: string,
    readonly kind: RenderedBlockKind,
  ) {
    super();
  }
  override eq(other: RenderedBlockWidget): boolean {
    return other.source === this.source && other.kind === this.kind;
  }
  toDOM(view: EditorView): HTMLElement {
    const element = document.createElement("div");
    element.className = `cm-gk-rendered md cm-gk-${this.kind}`;
    const renderer = view.state.facet(blockRenderer);
    const cache = renderCache(renderer);
    const key = `${this.kind}\u0000${this.source}`;
    const cached = cache.get(key);
    if (cached !== undefined) {
      setSanitized(element, cached);
    } else {
      element.textContent = this.source;
      element.classList.add("cm-gk-pending");
      renderer(this.source, this.kind)
        .then((html) => {
          remember(cache, key, html);
          element.classList.remove("cm-gk-pending");
          setSanitized(element, html);
          view.requestMeasure();
        })
        .catch((error: unknown) => {
          element.classList.remove("cm-gk-pending");
          element.classList.add("cm-gk-widget-error");
          console.error("a block could not be rendered", error);
        });
    }
    openOnMouseDown(element, view);
    return element;
  }
  override get estimatedHeight(): number {
    return (this.source.split(/\r\n?|\n/).length + 1) * 22;
  }
  override ignoreEvent(): boolean {
    return true;
  }
}
