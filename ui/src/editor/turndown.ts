// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * HTML to Markdown for pasting, through `turndown` and its GFM plugin, with
 * our own rules for what office programs and browsers put on the clipboard.
 * Loaded on the first HTML paste.
 */

import TurndownService from "turndown";
import { gfm } from "turndown-plugin-gfm";

const styleOf = (node: Node) => (node instanceof Element ? (node.getAttribute("style") ?? "") : "");
const BOLD_STYLE = /font-weight:\s*(?:[6-9]00|bold)/i;
const ITALIC_STYLE = /font-style:\s*italic/i;

/** Builds the converter with every rule. */
export function createTurndown(): TurndownService {
  const service = new TurndownService({ headingStyle: "atx", codeBlockStyle: "fenced", bulletListMarker: "-", emDelimiter: "*", strongDelimiter: "**" });
  service.use(gfm);

  // Scripts and styles never become text.
  service.remove(["script", "style", "noscript", "template"]);

  // One space after a list marker ("- item"), not turndown's three.
  service.addRule("tight-list-item", {
    filter: "li",
    replacement: (content, node, options) => {
      const text = content.replace(/^\n+/, "").replace(/\n+$/, "\n").replace(/\n/gm, "\n  ");
      const parent = node.parentNode;
      let prefix = `${options.bulletListMarker ?? "-"} `;
      if (parent instanceof Element && parent.nodeName === "OL") {
        const start = Number(parent.getAttribute("start") ?? "1");
        prefix = `${String((Number.isFinite(start) ? start : 1) + Array.prototype.indexOf.call(parent.children, node))}. `;
      }
      const item = text.trim().replace(/^\[([ xX])\]\s+/, "[$1] ");
      return prefix + item + (node.nextSibling && !text.endsWith("\n") ? "\n" : "");
    },
  });

  // Google Docs wraps the whole copy in <b style="font-weight:normal">.
  service.addRule("docs-wrapper", {
    filter: (node) => node.nodeName === "B" && /font-weight:\s*normal/i.test(styleOf(node)),
    replacement: (content) => content,
  });

  // Google Docs and others mark bold and italic with styles on spans.
  service.addRule("styled-span", {
    filter: (node) => node.nodeName === "SPAN" && (BOLD_STYLE.test(styleOf(node)) || ITALIC_STYLE.test(styleOf(node))),
    replacement: (content, node) => {
      if (!content.trim()) {
        return content;
      }
      const style = styleOf(node);
      // The markers go around the words, never around the spaces next to them.
      const lead = /^\s*/.exec(content)?.[0] ?? "";
      const trail = /\s*$/.exec(content)?.[0] ?? "";
      let inner = content.trim();
      if (ITALIC_STYLE.test(style)) {
        inner = `*${inner}*`;
      }
      if (BOLD_STYLE.test(style)) {
        inner = `**${inner}**`;
      }
      return lead + inner + trail;
    },
  });

  // Spreadsheet cells arrive as a table without <th>. A Markdown table needs
  // a header row, so the first row becomes it (the GFM plugin would keep
  // such a table as HTML).
  service.addRule("headerless-table", {
    filter: (node) => node.nodeName === "TABLE" && !(node as HTMLTableElement).querySelector("th"),
    replacement: (_content, node) => {
      const rows = Array.from((node as HTMLTableElement).rows).map((row) =>
        Array.from(row.cells).map((cell) => cell.textContent.trim().replace(/\|/g, "\\|").replace(/\s*\n\s*/g, " ")),
      );
      const first = rows[0];
      if (!first) {
        return "";
      }
      const width = Math.max(...rows.map((row) => row.length));
      const line = (row: string[]) => `| ${Array.from({ length: width }, (_, i) => row[i] ?? "").join(" | ")} |`;
      const lines = [line(first), `| ${Array<string>(width).fill("---").join(" | ")} |`, ...rows.slice(1).map(line)];
      return `\n\n${lines.join("\n")}\n\n`;
    },
  });

  // Strikethrough as "~~", the usual form (the GFM plugin writes one tilde).
  service.addRule("strikethrough", {
    filter: (node) => node.nodeName === "DEL" || node.nodeName === "S" || node.nodeName === "STRIKE",
    replacement: (content) => (content.trim() ? `~~${content}~~` : content),
  });

  return service;
}

let shared: TurndownService | null = null;

/** Converts pasted HTML to Markdown, with "\n" line breaks. */
export function htmlToMarkdown(html: string): string {
  shared ??= createTurndown();
  return shared.turndown(html);
}
