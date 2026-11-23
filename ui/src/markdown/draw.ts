// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * What the page draws on top of the reader's HTML: formulas (KaTeX),
 * diagrams (Mermaid), highlighted code with its language and a Copy button
 * (Shiki), attached images, and placeholders for external images, which
 * never load in the app. Links to tasks and wiki pages become keyboard
 * reachable. Everything works on the element the HTML was inserted into.
 */

import { insertSanitized } from "./insert";
import { drawMath, highlight, renderDiagram } from "./libraries";

export interface DrawOptions {
  /** Translated texts of the drawn parts. */
  text: (id: string, args?: Record<string, string>) => string;
  /** The address of an attached image, or null when it can't be shown. */
  imageSource: (attachment: string) => Promise<string | null>;
  /** Whether this drawing still counts; a newer one replaces it. */
  current: () => boolean;
}

/** The attachment ID of a `gk-file:` address, as the renderer writes it. */
export function attachmentOf(src: string): string | null {
  const found = /^gk-file:\/\/(?:localhost\/)?([0-9A-Za-z-]+)\/?$/.exec(src);
  return found?.[1] ?? null;
}

function button(className: string, label: string): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.className = className;
  element.textContent = label;
  return element;
}

/** Puts a header with the language and a Copy button over each code block. */
function frameCode(root: HTMLElement, options: DrawOptions): { pre: HTMLPreElement; code: string; language: string }[] {
  const blocks: { pre: HTMLPreElement; code: string; language: string }[] = [];
  for (const pre of root.querySelectorAll<HTMLPreElement>("pre")) {
    const language = pre.dataset.lang ?? "";
    if (language === "mermaid" || pre.closest(".gk-code") !== null) {
      continue;
    }
    const code = pre.textContent;
    const frame = document.createElement("div");
    frame.className = "gk-code";
    const head = document.createElement("div");
    head.className = "codehead";
    const name = document.createElement("span");
    name.className = "gk-code-lang";
    name.textContent = language === "" ? options.text("reader-code-plain") : language;
    const copy = button("btn flat sm gk-copy", options.text("reader-copy"));
    copy.dataset.copy = "";
    copy.setAttribute("aria-label", options.text("reader-copy-code", { language: name.textContent }));
    head.append(name, copy);
    pre.replaceWith(frame);
    frame.append(head, pre);
    // The code to copy is the text as written, whatever the highlighting does.
    frame.dataset.code = code;
    blocks.push({ pre, code, language });
  }
  return blocks;
}

async function drawCode(blocks: { pre: HTMLPreElement; code: string; language: string }[], options: DrawOptions): Promise<void> {
  for (const { pre, code, language } of blocks) {
    if (language === "") {
      continue;
    }
    let html: string | null;
    try {
      html = await highlight(code.replace(/\n$/, ""), language);
    } catch (error) {
      console.error("code could not be highlighted", error);
      html = null;
    }
    if (html === null || !options.current()) {
      continue;
    }
    const holder = document.createElement("div");
    insertSanitized(holder, html, "code");
    const highlighted = holder.firstElementChild;
    if (highlighted instanceof HTMLPreElement) {
      highlighted.dataset.lang = language;
      highlighted.removeAttribute("tabindex");
      pre.replaceWith(highlighted);
    }
  }
}

async function drawDiagrams(root: HTMLElement, options: DrawOptions): Promise<void> {
  for (const pre of root.querySelectorAll<HTMLPreElement>('pre[data-lang="mermaid"]')) {
    const source = pre.textContent;
    const holder = document.createElement("div");
    holder.className = "gk-diagram";
    holder.setAttribute("role", "img");
    holder.setAttribute("aria-label", options.text("reader-diagram"));
    try {
      const svg = await renderDiagram(source);
      if (!options.current()) {
        return;
      }
      insertSanitized(holder, svg, "diagram");
    } catch (error) {
      if (!options.current()) {
        return;
      }
      holder.classList.add("gk-render-error");
      holder.removeAttribute("role");
      holder.textContent = options.text("reader-diagram-error", { error: error instanceof Error ? error.message : String(error) });
    }
    pre.replaceWith(holder);
  }
}

async function drawFormulas(root: HTMLElement): Promise<void> {
  const formulas = [...root.querySelectorAll<HTMLElement>("span[data-math]")];
  await Promise.all(formulas.map((element) => drawMath(element, element.textContent, element.dataset.math === "display")));
}

/** Attached images come from the app; external ones become a button that opens them in the browser. */
async function drawImages(root: HTMLElement, options: DrawOptions): Promise<void> {
  for (const image of root.querySelectorAll<HTMLImageElement>("img[data-external-src]")) {
    const url = image.dataset.externalSrc ?? "";
    const holder = button("gk-external-image", options.text("reader-external-image", { name: image.alt === "" ? url : image.alt }));
    holder.dataset.external = url;
    holder.title = url;
    image.replaceWith(holder);
  }
  await Promise.all(
    [...root.querySelectorAll<HTMLImageElement>("img[src]")].map(async (image) => {
      const attachment = attachmentOf(image.getAttribute("src") ?? "");
      image.removeAttribute("src");
      if (attachment === null) {
        return;
      }
      image.dataset.attachment = attachment;
      image.loading = "lazy";
      const source = await options.imageSource(attachment).catch(() => null);
      if (source !== null && options.current()) {
        image.src = source;
      }
    }),
  );
}

/** Task keys and wiki links have no address: they become links reachable by keyboard. */
function linkReferences(root: HTMLElement, options: DrawOptions): void {
  for (const link of root.querySelectorAll<HTMLAnchorElement>("a[data-ref], a[data-wiki]")) {
    link.tabIndex = 0;
    link.setAttribute("role", "link");
    if (link.dataset.wiki !== undefined) {
      link.title = options.text("reader-wiki-link", { page: link.dataset.wiki });
    }
  }
  for (const box of root.querySelectorAll<HTMLInputElement>("input[data-item]")) {
    const item = box.closest("li");
    box.setAttribute("aria-label", item?.textContent.trim() ?? options.text("reader-checkbox"));
  }
}

/** Draws everything on the reader's HTML in `root`. Later steps check that the drawing is still current. */
export async function drawReader(root: HTMLElement, options: DrawOptions): Promise<void> {
  linkReferences(root, options);
  const blocks = frameCode(root, options);
  await Promise.all([drawFormulas(root), drawDiagrams(root, options), drawCode(blocks, options), drawImages(root, options)]);
}
