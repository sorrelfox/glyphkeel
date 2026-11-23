// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { attachmentOf, drawReader } from "./draw";
import { insertSanitized } from "./insert";

function inserted(html: string, profile: "reader" | "diagram" | "code" = "reader"): HTMLElement {
  const element = document.createElement("div");
  insertSanitized(element, html, profile);
  return element;
}

describe("the one insertion point", () => {
  it("keeps the renderer's markup and attributes", () => {
    const element = inserted(
      '<ul><li class="task-item"><input type="checkbox" data-item="2" checked="" /> Two</li></ul><p><a data-ref="TM-4">TM-4</a> <a data-wiki="Plan#Dates" rel="noopener noreferrer">Plan</a> <span data-math="inline">x^2</span></p><pre data-lang="rust"><code>fn x() {}</code></pre><img data-external-src="https://example.com/a.png" alt="a"><img src="gk-file://localhost/0190f3a2-7c41-7d2b-9f00-123456789abc" alt="b"><a href="#fn-task_1-a">1</a><a href="mailto:x@example.com">mail</a>',
    );
    expect(element.querySelector("input")?.getAttribute("data-item")).toBe("2");
    expect(element.querySelector("input")?.getAttribute("type")).toBe("checkbox");
    expect(element.querySelector("a[data-ref]")?.getAttribute("data-ref")).toBe("TM-4");
    expect(element.querySelector("a[data-wiki]")?.getAttribute("data-wiki")).toBe("Plan#Dates");
    expect(element.querySelector("span")?.dataset.math).toBe("inline");
    expect(element.querySelector("pre")?.dataset.lang).toBe("rust");
    expect(element.querySelector("img[alt=a]")?.getAttribute("data-external-src")).toBe("https://example.com/a.png");
    expect(element.querySelector("img[alt=b]")?.getAttribute("src")).toMatch(/^gk-file:/);
    expect(element.querySelector("a[href^='#']")).not.toBeNull();
    expect(element.querySelector("a[href^='mailto:']")).not.toBeNull();
  });

  it("drops what the engine would never write, as a second layer", () => {
    const element = inserted(
      '<script>window.bad = 1</script><img src="x" onerror="window.bad = 1"><a href="javascript:alert(1)">x</a><a href="data:text/html,x">y</a><svg><circle/></svg><iframe src="https://example.com"></iframe><p style="color: red" onclick="x()">p</p><img data-external-src="javascript:alert(1)" alt="z"><form><input type="text" name="q"></form>',
    );
    expect(element.querySelector("script, svg, iframe, form")).toBeNull();
    expect(element.querySelector("[onerror], [onclick], [style], [name]")).toBeNull();
    for (const link of element.querySelectorAll("a")) {
      expect(link.getAttribute("href")).toBeNull();
    }
    expect(element.querySelector("img[alt=z]")?.hasAttribute("data-external-src")).toBe(false);
  });

  it("lets code through as coloured spans only", () => {
    const element = inserted('<pre class="shiki gk" style="background-color:var(--syn-background)" tabindex="0" onclick="x()"><code><span style="color:var(--syn-token-keyword)">fn</span><img src=x></code></pre>', "code");
    expect(element.querySelector("pre")?.getAttribute("style")).toContain("--syn-background");
    expect(element.querySelector("span")?.getAttribute("style")).toContain("--syn-token-keyword");
    expect(element.querySelector("img, [onclick]")).toBeNull();
  });
});

describe("drawing on the reader's HTML", () => {
  it("turns external images into buttons, frames code with a Copy button and makes keys reachable", async () => {
    const element = inserted('<p><img data-external-src="https://example.com/a.png" alt="Chart"> <a data-ref="TM-4">TM-4</a></p><pre><code>plain\n</code></pre><ul><li class="task-item"><input type="checkbox" data-item="1" /> Buy milk</li></ul>');
    await drawReader(element, { text: (id, args) => `${id}${args ? JSON.stringify(args) : ""}`, imageSource: () => Promise.resolve(null), current: () => true });
    const placeholder = element.querySelector<HTMLButtonElement>("button.gk-external-image");
    expect(placeholder?.dataset.external).toBe("https://example.com/a.png");
    expect(element.querySelector("img")).toBeNull();
    expect(element.querySelector<HTMLElement>(".gk-code")?.dataset.code).toBe("plain\n");
    expect(element.querySelector("[data-copy]")).not.toBeNull();
    expect(element.querySelector<HTMLAnchorElement>("a[data-ref]")?.tabIndex).toBe(0);
    expect(element.querySelector("input")?.getAttribute("aria-label")).toBe("Buy milk");
  });

  it("reads attachment IDs from both address forms", () => {
    expect(attachmentOf("gk-file://abc-123")).toBe("abc-123");
    expect(attachmentOf("gk-file://localhost/abc-123")).toBe("abc-123");
    expect(attachmentOf("https://example.com/x.png")).toBeNull();
    expect(attachmentOf("gk-file://localhost/../etc")).toBeNull();
  });
});
