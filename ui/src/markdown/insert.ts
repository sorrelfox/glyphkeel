// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * The one place where HTML strings become part of the page. The engine's
 * renderer has already sanitized the reader's HTML; DOMPurify checks it once
 * more here, against the same allowlist, as a second layer. Diagrams and
 * highlighted code, which the page draws itself, pass through their own,
 * narrower profiles. Every insertion builds a DOM fragment and puts it in
 * place, so no element ever parses markup through `innerHTML`.
 */

import DOMPurify, { type Config } from "dompurify";

/** The tags the engine's reader HTML may contain. */
const READER_TAGS = [
  "p", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "ul", "ol", "li", "pre", "code", "em", "strong", "del", "hr", "br",
  "a", "img", "table", "thead", "tbody", "tr", "th", "td", "section", "sup", "span", "div", "input", "details", "summary",
  "kbd", "sub", "mark",
];

/** Their attributes, all tags together (the engine allows each on fewer tags). */
const READER_ATTRIBUTES = [
  "title", "href", "id", "class", "rel", "src", "alt", "width", "height", "type", "checked", "disabled", "start", "align", "open",
  "data-item", "data-macro", "data-macro-note", "data-macro-arg", "data-ref", "data-comment", "data-wiki", "data-math", "data-lang", "data-external-src", "data-footnotes", "data-footnote-ref",
  "data-footnote-backref",
];

/**
 * Addresses: the web, mail and attached images, or no scheme at all (a
 * footnote's `#…`). DOMPurify checks every attribute value that isn't an
 * address against this too, so plain values must pass.
 */
const READER_URLS = /^(?:(?:https?|mailto|gk-file):|[^a-z]|[a-z+.-]+(?:[^a-z+.\-:]|$))/i;

export type HtmlProfile = "reader" | "diagram" | "code";

const PROFILES: Record<HtmlProfile, Config> = {
  reader: {
    ALLOWED_TAGS: READER_TAGS,
    ALLOWED_ATTR: READER_ATTRIBUTES,
    ALLOW_DATA_ATTR: false,
    ALLOW_ARIA_ATTR: false,
    ALLOWED_URI_REGEXP: READER_URLS,
  },
  // Mermaid's SVG: shapes and text, with the style element it themes them by.
  diagram: { USE_PROFILES: { svg: true, svgFilters: true }, ADD_TAGS: ["style"] },
  // Shiki's output: lines of coloured spans.
  code: { ALLOWED_TAGS: ["pre", "code", "span"], ALLOWED_ATTR: ["class", "style", "tabindex"], ALLOW_DATA_ATTR: false },
};

/** Replaces the children of `target` with `html`, sanitized by `profile`. */
export function insertSanitized(target: Element, html: string, profile: HtmlProfile = "reader"): void {
  const fragment = DOMPurify.sanitize(html, { ...PROFILES[profile], RETURN_DOM_FRAGMENT: true });
  target.replaceChildren(fragment);
}

/** The text without the ticks of its checkboxes: two texts that differ only in what is ticked read the same. */
export function withoutTicks(html: string): string {
  return html.replace(/<input\b[^>]*>/g, (tag) => tag.replace(/\s+checked(?:=(?:""|"checked"|checked))?(?=[\s/>])/, ""));
}

/** Whether each checkbox of the reader's `html` is ticked, by its item number, in reading order. */
export function tickedItems(html: string): { item: string; checked: boolean }[] {
  const fragment = DOMPurify.sanitize(html, { ...PROFILES.reader, RETURN_DOM_FRAGMENT: true });
  return Array.from(fragment.querySelectorAll<HTMLInputElement>("input[data-item]"), (box) => ({ item: box.dataset.item ?? "", checked: box.hasAttribute("checked") }));
}
