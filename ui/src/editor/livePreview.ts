// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Live Preview: the Markdown source drawn as it will read, without changing
 * a character of it.
 *
 * - Inline: a view plugin over the visible ranges hides markers on every
 *   line except the cursor's, styles headings, emphasis, code and links, and
 *   draws checkboxes and inline math as widgets.
 * - Blocks: a state field (plugins can't provide block decorations) draws
 *   tables, callouts, math blocks and Mermaid diagrams as widgets while the
 *   selection is outside them. It keeps the list of blocks between
 *   transactions: moving the cursor only re-decides which block shows its
 *   source, and a change walks the top-level blocks without reading the
 *   text of those that never become widgets.
 * - Arrow keys enter a block widget (its source opens) instead of jumping
 *   over it.
 */

import { syntaxTree } from "@codemirror/language";
import { EditorSelection, Prec, StateField, type EditorState, type Range, type SelectionRange, type Transaction } from "@codemirror/state";
import { Decoration, EditorView, keymap, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import type { SyntaxNode, SyntaxNodeRef, Tree } from "@lezer/common";

import { CheckboxWidget, MathWidget, MermaidWidget, RenderedBlockWidget } from "./widgets";

/** Time spent building decorations, for the development page and the tests. */
export const previewStats = { inlineBuilds: 0, inlineMs: 0, blockPasses: 0, blockScans: 0, blockRescans: 0, blockMs: 0, skippedMultiline: 0 };

export function resetPreviewStats(): void {
  Object.assign(previewStats, { inlineBuilds: 0, inlineMs: 0, blockPasses: 0, blockScans: 0, blockRescans: 0, blockMs: 0, skippedMultiline: 0 });
}

const hidden = Decoration.replace({});
const markClass = (name: string) => Decoration.mark({ class: name });
const lineClass = (name: string) => Decoration.line({ class: name });

const MARK = markClass("cm-gk-mark");
const INLINE_CODE = markClass("cm-gk-code");
const LINK = markClass("cm-gk-link");
const WIKI_LINK = markClass("cm-gk-wikilink");
const WIKI_SOURCE = markClass("cm-gk-wikilink cm-gk-source");
const TASK_KEY = markClass("cm-gk-taskkey");
const MATH_SOURCE = markClass("cm-gk-math-source");
const DONE = markClass("cm-gk-done");
const CODE_LINE = lineClass("cm-gk-codeline");
const TABLE_LINE = lineClass("cm-gk-tableline");
const RULE_LINE = lineClass("cm-gk-rule");
const HEADING_LINES = [1, 2, 3, 4, 5, 6].map((level) => lineClass(`cm-gk-h${String(level)}`));

/** The line ranges the selection touches: their markers stay visible. */
function activeRanges(state: EditorState): [number, number][] {
  return state.selection.ranges.map((range) => [state.doc.lineAt(range.from).from, state.doc.lineAt(range.to).to]);
}

function buildInline(view: EditorView): DecorationSet {
  const started = performance.now();
  const { state } = view;
  const active = activeRanges(state);
  const isActive = (pos: number) => active.some(([from, to]) => pos >= from && pos <= to);
  const ranges: Range<Decoration>[] = [];
  const replace = (from: number, to: number, decoration: Decoration = hidden) => {
    // A plugin may not replace a line break; links and inline math can wrap
    // across lines, and those stay undecorated.
    if (state.sliceDoc(from, to).search(/[\r\n]/) >= 0) {
      previewStats.skippedMultiline++;
      return;
    }
    ranges.push(decoration.range(from, to));
  };
  const lines = (from: number, to: number, decoration: Decoration) => {
    const first = state.doc.lineAt(from).number;
    const last = state.doc.lineAt(to).number;
    for (let n = first; n <= last; n++) {
      ranges.push(decoration.range(state.doc.line(n).from));
    }
  };

  const tree = syntaxTree(state);
  for (const { from, to } of view.visibleRanges) {
    tree.iterate({
      from,
      to,
      enter(node: SyntaxNodeRef) {
        const name = node.name;
        const heading = /^(?:ATX|Setext)Heading([1-6])$/.exec(name);
        if (heading) {
          const decoration = HEADING_LINES[Number(heading[1]) - 1];
          if (decoration) {
            ranges.push(decoration.range(state.doc.lineAt(node.from).from));
          }
          return undefined;
        }
        switch (name) {
          case "HeaderMark": {
            if (node.node.parent?.name.startsWith("Setext")) {
              return undefined;
            }
            if (isActive(node.from)) {
              ranges.push(MARK.range(node.from, node.to));
              return undefined;
            }
            const space = state.sliceDoc(node.to, node.to + 1) === " " ? 1 : 0;
            if (node.to + space > node.from) {
              replace(node.from, node.to + space);
            }
            return undefined;
          }
          case "EmphasisMark":
          case "StrikethroughMark":
            if (isActive(node.from)) {
              ranges.push(MARK.range(node.from, node.to));
            } else {
              replace(node.from, node.to);
            }
            return undefined;
          case "InlineCode":
            ranges.push(INLINE_CODE.range(node.from, node.to));
            if (!isActive(node.from)) {
              for (const mark of node.node.getChildren("CodeMark")) {
                replace(mark.from, mark.to);
              }
            }
            return false;
          case "Link": {
            const marks = node.node.getChildren("LinkMark");
            const open = marks[0];
            const close = marks[1];
            if (!open || !close) {
              return undefined;
            }
            if (close.from > open.to) {
              ranges.push(LINK.range(open.to, close.from));
            }
            if (!isActive(node.from)) {
              replace(open.from, open.to);
              replace(close.from, node.to);
            }
            return false;
          }
          case "WikiLink": {
            if (isActive(node.from)) {
              ranges.push(WIKI_SOURCE.range(node.from, node.to));
              return false;
            }
            const bar = state.sliceDoc(node.from + 2, node.to - 2).indexOf("|");
            const labelFrom = bar >= 0 ? node.from + 2 + bar + 1 : node.from + 2;
            replace(node.from, labelFrom);
            if (node.to - 2 > labelFrom) {
              ranges.push(WIKI_LINK.range(labelFrom, node.to - 2));
            }
            replace(node.to - 2, node.to);
            return false;
          }
          case "TaskKey":
            ranges.push(TASK_KEY.range(node.from, node.to));
            return false;
          case "InlineMath": {
            if (isActive(node.from)) {
              ranges.push(MATH_SOURCE.range(node.from, node.to));
              return false;
            }
            const tex = state.sliceDoc(node.from + 1, node.to - 1);
            replace(node.from, node.to, Decoration.replace({ widget: new MathWidget(tex, false) }));
            return false;
          }
          case "ListMark": {
            if (node.node.nextSibling?.name === "Task" && !isActive(node.from)) {
              const space = state.sliceDoc(node.to, node.to + 1) === " " ? 1 : 0;
              replace(node.from, node.to + space);
            }
            return undefined;
          }
          case "TaskMarker": {
            const selection = state.selection.main;
            if (selection.from <= node.to && selection.to >= node.from) {
              return undefined;
            }
            const checked = state.sliceDoc(node.from + 1, node.to - 1) !== " ";
            ranges.push(Decoration.replace({ widget: new CheckboxWidget(checked) }).range(node.from, node.to));
            if (checked) {
              const line = state.doc.lineAt(node.from);
              if (node.to + 1 < line.to) {
                ranges.push(DONE.range(node.to + 1, line.to));
              }
            }
            return undefined;
          }
          case "QuoteMark":
            ranges.push(MARK.range(node.from, node.to));
            return undefined;
          case "HorizontalRule":
            ranges.push(RULE_LINE.range(state.doc.lineAt(node.from).from));
            if (!isActive(node.from)) {
              replace(node.from, node.to);
            }
            return false;
          case "FencedCode":
          case "CodeBlock":
            lines(node.from, node.to, CODE_LINE);
            return false;
          case "Table":
            lines(node.from, node.to, TABLE_LINE);
            return false;
          case "BlockMath":
            return false;
          default:
            return undefined;
        }
      },
    });
  }
  const set = Decoration.set(ranges, true);
  previewStats.inlineBuilds++;
  previewStats.inlineMs += performance.now() - started;
  return set;
}

const inlinePreview = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildInline(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet || update.viewportChanged || syntaxTree(update.startState) !== syntaxTree(update.state)) {
        this.decorations = buildInline(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

// ---------------------------------------------------------------- blocks

/** A block that can be drawn as a widget: its whole lines, and the widget. */
export interface PreviewBlock {
  from: number;
  to: number;
  widget: MathWidget | MermaidWidget | RenderedBlockWidget;
}

interface BlockState {
  tree: Tree;
  blocks: readonly PreviewBlock[];
  /** The blocks drawn as widgets: those the selection doesn't touch. */
  shown: readonly PreviewBlock[];
  decorations: DecorationSet;
}

const CALLOUT = /^>\s?\[!\w+\]/;
const FENCE_LINE = /^\s*(?:`{3,}|~{3,})/;

function blockWidget(state: EditorState, name: string, node: SyntaxNodeRef, from: number, to: number): PreviewBlock["widget"] | null {
  switch (name) {
    case "Table":
      return new RenderedBlockWidget(state.sliceDoc(from, to), "table");
    case "Blockquote": {
      // Only callouts; a plain quote stays text.
      const first = state.doc.lineAt(from);
      return CALLOUT.test(first.text) ? new RenderedBlockWidget(state.sliceDoc(from, to), "callout") : null;
    }
    case "BlockMath": {
      const first = state.doc.lineAt(from);
      const last = state.doc.lineAt(to);
      const bodyTo = last.number > first.number && /^\s*\$\$\s*$/.test(last.text) ? last.from : to;
      const bodyFrom = Math.min(first.to + 1, bodyTo);
      const tex = state.sliceDoc(bodyFrom, bodyTo).replace(/[\r\n]+$/, "");
      return new MathWidget(tex, true);
    }
    case "FencedCode": {
      const info = node.node.getChild("CodeInfo");
      if (!info || state.sliceDoc(info.from, info.to).trim() !== "mermaid") {
        return null;
      }
      const first = state.doc.lineAt(from);
      const last = state.doc.lineAt(to);
      const bodyTo = last.number > first.number && FENCE_LINE.test(last.text) ? last.from : to;
      const bodyFrom = Math.min(first.to + 1, bodyTo);
      return new MermaidWidget(state.sliceDoc(bodyFrom, bodyTo).replace(/[\r\n]+$/, "").replace(/\r\n?/g, "\n"));
    }
    default:
      return null;
  }
}

const CANDIDATES = new Set(["Table", "Blockquote", "BlockMath", "FencedCode"]);

/** The block a top-level node draws as, if any. */
function candidate(state: EditorState, node: SyntaxNodeRef): PreviewBlock | null {
  const name = node.name;
  if (!CANDIDATES.has(name)) {
    return null;
  }
  const from = state.doc.lineAt(node.from).from;
  const end = node.to > node.from && /[\r\n]/.test(state.sliceDoc(node.to - 1, node.to)) ? node.to - 1 : node.to;
  const to = state.doc.lineAt(Math.max(node.from, end)).to;
  const widget = blockWidget(state, name, node, from, to);
  return widget ? { from, to, widget } : null;
}

/** Walks all top-level blocks; only the candidates' lines and text are read. */
function scanBlocks(state: EditorState, tree: Tree): PreviewBlock[] {
  const blocks: PreviewBlock[] = [];
  const cursor = tree.cursor();
  if (cursor.firstChild()) {
    do {
      const block = candidate(state, cursor);
      if (block) {
        blocks.push(block);
      }
    } while (cursor.nextSibling());
  }
  previewStats.blockScans++;
  return blocks;
}

/**
 * The blocks after a change, re-reading only the part of the text the change
 * can affect: from the line before it (a table or quote can take in the next
 * line) to the first top-level block after it that starts and ends where a
 * block of the old text did. From there on the text, and so its blocks, are
 * the same as before, only moved.
 */
function rescanBlocks(previous: BlockState, tr: Transaction, tree: Tree): PreviewBlock[] {
  const { state, changes } = tr;
  let fromA = Number.POSITIVE_INFINITY;
  let fromB = Number.POSITIVE_INFINITY;
  let toB = -1;
  changes.iterChangedRanges((changedFromA, _toA, changedFromB, changedToB) => {
    fromA = Math.min(fromA, changedFromA);
    fromB = Math.min(fromB, changedFromB);
    toB = Math.max(toB, changedToB);
  });
  const line = state.doc.lineAt(fromB);
  const start = line.number > 1 ? state.doc.line(line.number - 1).from : 0;
  let node: SyntaxNode | null = tree.topNode.childAfter(start);
  const walkFrom = Math.min(node?.from ?? state.doc.length, fromA);
  // Blocks before the walk are untouched, at the same positions.
  const blocks = previous.blocks.filter((block) => block.to < walkFrom);
  const inverse = changes.invertedDesc;
  for (; node; node = node.nextSibling) {
    if (node.from > toB) {
      const oldFrom = inverse.mapPos(node.from, 1);
      const old = previous.tree.topNode.childAfter(oldFrom);
      if (old && old.from === oldFrom && old.name === node.name && changes.mapPos(old.to, -1) === node.to) {
        for (const block of previous.blocks) {
          if (block.from >= oldFrom) {
            blocks.push({ from: changes.mapPos(block.from, 1), to: changes.mapPos(block.to, -1), widget: block.widget });
          }
        }
        previewStats.blockRescans++;
        return blocks;
      }
    }
    const block = candidate(state, node);
    if (block) {
      blocks.push(block);
    }
  }
  previewStats.blockRescans++;
  return blocks;
}

const touches = (ranges: readonly SelectionRange[], block: PreviewBlock) => ranges.some((range) => range.from <= block.to && range.to >= block.from);

function decorate(state: EditorState, tree: Tree, blocks: readonly PreviewBlock[], previous?: BlockState): BlockState {
  const ranges = state.selection.ranges;
  const shown = blocks.filter((block) => !touches(ranges, block));
  if (previous && previous.blocks === blocks && previous.shown.length === shown.length && previous.shown.every((block, i) => block === shown[i])) {
    return previous.tree === tree ? previous : { ...previous, tree };
  }
  const decorations = Decoration.set(
    shown.map((block) => Decoration.replace({ widget: block.widget, block: true }).range(block.from, block.to)),
    true,
  );
  return { tree, blocks, shown, decorations };
}

export const blockPreview = StateField.define<BlockState>({
  create(state) {
    const tree = syntaxTree(state);
    return decorate(state, tree, scanBlocks(state, tree));
  },
  update(value, tr) {
    const tree = syntaxTree(tr.state);
    if (!tr.docChanged && tree === value.tree) {
      return tr.selection ? decorate(tr.state, tree, value.blocks, value) : value;
    }
    const started = performance.now();
    // A change re-reads the part it touches; a parse that went further re-reads all.
    const blocks = tr.docChanged && syntaxTree(tr.startState) === value.tree ? rescanBlocks(value, tr, tree) : scanBlocks(tr.state, tree);
    const next = decorate(tr.state, tree, blocks);
    previewStats.blockMs += performance.now() - started;
    previewStats.blockPasses++;
    return next;
  },
  provide: (field) => EditorView.decorations.from(field, (value) => value.decorations),
});

// ---------------------------------------------------------------- keyboard

/**
 * Arrow keys that would jump over a block widget put the cursor into the
 * block instead, which opens its source.
 */
function enterBlock(forward: boolean, vertical: boolean) {
  return (view: EditorView): boolean => {
    const selection = view.state.selection.main;
    if (!selection.empty) {
      return false;
    }
    const { shown } = view.state.field(blockPreview);
    if (shown.length === 0) {
      return false;
    }
    const head = selection.head;
    const target = (vertical ? view.moveVertically(selection, forward) : view.moveByChar(selection, forward)).head;
    let block: PreviewBlock | undefined;
    if (forward) {
      block = shown.find((b) => b.from > head && b.from <= target);
    } else {
      block = shown.findLast((b) => b.to < head && b.to >= target);
    }
    if (!block) {
      return false;
    }
    const anchor = forward ? block.from : view.state.doc.lineAt(block.to).from;
    view.dispatch({ selection: EditorSelection.cursor(anchor), scrollIntoView: true, userEvent: "select" });
    return true;
  };
}

const blockKeys = Prec.high(
  keymap.of([
    { key: "ArrowDown", run: enterBlock(true, true) },
    { key: "ArrowUp", run: enterBlock(false, true) },
    { key: "ArrowRight", run: enterBlock(true, false) },
    { key: "ArrowLeft", run: enterBlock(false, false) },
  ]),
);

/** Live Preview, as one extension. */
export const livePreview = [inlinePreview, blockPreview, blockKeys];
