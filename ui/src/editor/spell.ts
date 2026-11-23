// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Spelling through the app's spell checker: the visible prose is sent with
 * code, links, math, task keys and wiki links blanked out, and misspelt
 * words come back as ranges, drawn as wavy underlines. Loaded and typed text
 * are checked alike. The web view's own checker stays off.
 */

import { syntaxTree } from "@codemirror/language";
import { Facet, RangeSetBuilder } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { invoke, isTauri } from "@tauri-apps/api/core";

import type { SpellChecker } from "./types";

/** The spell checker of the desktop app, behind its two commands. */
export const appSpellChecker: SpellChecker = {
  async check(chunks) {
    const answer = await invoke<{ misspelt: [number, number][] }>("spell_check", { chunks });
    return answer.misspelt;
  },
  async suggest(word) {
    return invoke<string[]>("spell_suggest", { word });
  },
};

/** No spelling: in a plain browser, which has no app behind the page. */
export const noSpellChecker: SpellChecker = {
  check: () => Promise.resolve([]),
  suggest: () => Promise.resolve([]),
};

/** The spell checker to use when a screen passes none: the app's, or none in a plain browser. */
export function defaultSpellChecker(): SpellChecker | null {
  return isTauri() ? appSpellChecker : null;
}

export const spellCheckerFacet = Facet.define<SpellChecker, SpellChecker>({
  combine: (values) => values[0] ?? noSpellChecker,
});

/** Nodes whose text is never checked. */
const SKIPPED = new Set([
  "FencedCode",
  "CodeBlock",
  "InlineCode",
  "URL",
  "Autolink",
  "InlineMath",
  "BlockMath",
  "TaskKey",
  "WikiLink",
  "HTMLTag",
  "HTMLBlock",
  "LinkMark",
  "LinkLabel",
  "LinkTitle",
  "CodeInfo",
  "Comment",
  "CommentBlock",
  "ProcessingInstruction",
  "ProcessingInstructionBlock",
  "Entity",
]);

const MISSPELT = Decoration.mark({ class: "cm-gk-misspelt" });

/** The visible prose, as (start, text) chunks with skipped parts as spaces. */
export function proseChunks(view: EditorView): [number, string][] {
  const { state } = view;
  const tree = syntaxTree(state);
  const chunks: [number, string][] = [];
  for (const { from, to } of view.visibleRanges) {
    // Line breaks as "\n": one position each, as in the editor.
    const chars = state.doc.sliceString(from, to, "\n").split("");
    tree.iterate({
      from,
      to,
      enter(node) {
        if (!SKIPPED.has(node.name)) {
          return undefined;
        }
        for (let i = Math.max(node.from, from); i < Math.min(node.to, to); i++) {
          if (chars[i - from] !== "\n") {
            chars[i - from] = " ";
          }
        }
        return false;
      },
    });
    chunks.push([from, chars.join("")]);
  }
  return chunks;
}

const CHECK_DELAY_MS = 250;

class SpellPlugin {
  decorations: DecorationSet = Decoration.none;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private generation = 0;

  constructor(readonly view: EditorView) {
    this.schedule(0);
  }

  update(update: ViewUpdate) {
    if (update.docChanged) {
      this.decorations = this.decorations.map(update.changes);
    }
    if (update.docChanged || update.viewportChanged) {
      this.schedule(CHECK_DELAY_MS);
    }
  }

  private schedule(delay: number) {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.run(), delay);
  }

  private async run() {
    const generation = ++this.generation;
    const checker = this.view.state.facet(spellCheckerFacet);
    let misspelt: readonly (readonly [number, number])[];
    try {
      misspelt = await checker.check(proseChunks(this.view));
    } catch (error) {
      console.error("spelling could not be checked", error);
      return;
    }
    if (generation !== this.generation) {
      return;
    }
    const length = this.view.state.doc.length;
    const builder = new RangeSetBuilder<Decoration>();
    for (const [from, to] of [...misspelt].sort((a, b) => a[0] - b[0])) {
      if (from < to && to <= length) {
        builder.add(from, to, MISSPELT);
      }
    }
    const decorations = builder.finish();
    if (decorations.size === 0 && this.decorations.size === 0) {
      return;
    }
    this.decorations = decorations;
    this.view.dispatch({});
  }

  destroy() {
    clearTimeout(this.timer);
    this.generation++;
  }
}

const spellPlugin = ViewPlugin.fromClass(SpellPlugin, { decorations: (plugin) => plugin.decorations });

/** The misspelt word at a position, if any. */
export function misspeltAt(view: EditorView, pos: number): { from: number; to: number } | null {
  const plugin = view.plugin(spellPlugin);
  if (!plugin) {
    return null;
  }
  let found: { from: number; to: number } | null = null;
  plugin.decorations.between(pos, pos, (from, to) => {
    found = { from, to };
    return false;
  });
  return found;
}

/** Spelling with the given checker; the web view's own checker stays off. */
export function spelling(checker: SpellChecker) {
  return [spellCheckerFacet.of(checker), spellPlugin];
}
