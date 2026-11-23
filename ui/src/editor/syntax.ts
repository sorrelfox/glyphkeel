// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The Markdown language of the editor: Lezer's GFM parser plus what
 * Glyphkeel adds (task keys, [[wiki links]], $math$ and $$math$$ blocks).
 *
 * The language only decides how the text looks and where widgets go; the
 * stored text never comes from the parse. It is built without the HTML
 * language that `@codemirror/lang-markdown`'s `markdown()` pulls in, which
 * keeps the editor's bundle smaller; HTML blocks stay unhighlighted.
 */

import { Language, LanguageSupport } from "@codemirror/language";
import { markdownKeymap, markdownLanguage } from "@codemirror/lang-markdown";
import { Prec } from "@codemirror/state";
import { keymap } from "@codemirror/view";
import type { BlockContext, InlineContext, LeafBlock, LeafBlockParser, Line, MarkdownConfig, MarkdownParser } from "@lezer/markdown";

const isWordChar = (c: number) => (c >= 48 && c <= 57) || (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || c === 95 || c > 127;

const TASK_KEY = /^[A-Z][A-Z0-9]{0,9}-[0-9]{1,7}/;
const MATH_FENCE = /^\$\$\s*$/;

/** A task item's checkbox, as the engine reads one: `[ ]`, `[x]` or `[X]`, then a space, a tab or the end of the line. */
const TASK_MARKER = /^\[[ xX]\](?:[ \t\r\n]|$)/;

/** A task item: the checkbox, then the item's text. */
class TaskParser implements LeafBlockParser {
  nextLine(): boolean {
    return false;
  }
  finish(cx: BlockContext, leaf: LeafBlock): boolean {
    cx.addLeafElement(
      leaf,
      cx.elt("Task", leaf.start, leaf.start + leaf.content.length, [cx.elt("TaskMarker", leaf.start, leaf.start + 3), ...cx.parser.parseInline(leaf.content.slice(3), leaf.start + 3)]),
    );
    return true;
  }
}

/** Lezer extensions for task keys, wiki links and math. */
export const glyphkeelSyntax: MarkdownConfig = {
  defineNodes: [
    { name: "WikiLink" },
    { name: "WikiMark" },
    { name: "TaskKey" },
    { name: "InlineMath" },
    { name: "MathMark" },
    { name: "BlockMath", block: true },
  ],
  parseInline: [
    {
      name: "WikiLink",
      before: "Link",
      parse(cx: InlineContext, next: number, pos: number) {
        if (next !== 91 || cx.char(pos + 1) !== 91) {
          return -1;
        }
        const rest = cx.slice(pos + 2, cx.end);
        const close = rest.indexOf("]]");
        const inner = rest.slice(0, Math.max(close, 0));
        if (close <= 0 || inner.includes("\n") || inner.includes("[")) {
          return -1;
        }
        const end = pos + 2 + close + 2;
        return cx.addElement(cx.elt("WikiLink", pos, end, [cx.elt("WikiMark", pos, pos + 2), cx.elt("WikiMark", end - 2, end)]));
      },
    },
    {
      name: "TaskKey",
      parse(cx: InlineContext, next: number, pos: number) {
        if (next < 65 || next > 90) {
          return -1;
        }
        if (pos > cx.offset && isWordChar(cx.char(pos - 1))) {
          return -1;
        }
        const match = TASK_KEY.exec(cx.slice(pos, Math.min(cx.end, pos + 20)));
        if (!match) {
          return -1;
        }
        const end = pos + match[0].length;
        if (end < cx.end && isWordChar(cx.char(end))) {
          return -1;
        }
        return cx.addElement(cx.elt("TaskKey", pos, end));
      },
    },
    {
      name: "InlineMath",
      before: "Emphasis",
      parse(cx: InlineContext, next: number, pos: number) {
        if (next !== 36 || cx.char(pos + 1) === 36 || cx.char(pos + 1) === 32) {
          return -1;
        }
        for (let i = pos + 1; i < cx.end; i++) {
          const c = cx.char(i);
          if (c === 92) {
            i++;
            continue;
          }
          if (c === 36) {
            if (cx.char(i - 1) === 32 || i === pos + 1) {
              return -1;
            }
            return cx.addElement(cx.elt("InlineMath", pos, i + 1, [cx.elt("MathMark", pos, pos + 1), cx.elt("MathMark", i, i + 1)]));
          }
        }
        return -1;
      },
    },
  ],
  parseBlock: [
    {
      // Replaces GFM's task items, which need text after the checkbox: the
      // engine also counts an empty item, so the numbers stay the same.
      name: "TaskList",
      leaf(cx: BlockContext, leaf: LeafBlock): LeafBlockParser | null {
        return TASK_MARKER.test(leaf.content) && cx.parentType().name === "ListItem" ? new TaskParser() : null;
      },
      after: "SetextHeading",
    },
    {
      name: "BlockMath",
      before: "FencedCode",
      parse(cx: BlockContext, line: Line) {
        if (!MATH_FENCE.test(line.text.slice(line.pos))) {
          return false;
        }
        const start = cx.lineStart + line.pos;
        let end = cx.lineStart + line.text.length;
        while (cx.nextLine()) {
          end = cx.lineStart + line.text.length;
          if (/^\s*\$\$\s*$/.test(line.text)) {
            cx.nextLine();
            break;
          }
        }
        cx.addElement(cx.elt("BlockMath", start, end));
        return true;
      },
    },
  ],
};

/** The parser: GFM (as `markdownLanguage` has it) with Glyphkeel's extensions. */
export const glyphkeelParser: MarkdownParser = (markdownLanguage.parser as MarkdownParser).configure([glyphkeelSyntax]);

/**
 * The language, sharing `markdownLanguage`'s language data so that list
 * continuation and the other Markdown commands recognise it.
 */
export const glyphkeelLanguage = new Language(markdownLanguage.data, glyphkeelParser, [], "markdown");

/** The language with Enter continuing lists and Backspace removing list markup. */
export function glyphkeelMarkdown(): LanguageSupport {
  return new LanguageSupport(glyphkeelLanguage, [Prec.high(keymap.of(markdownKeymap))]);
}
