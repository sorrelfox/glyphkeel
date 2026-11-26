// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { describe, expect, it } from "vitest";

import { drawReader } from "./draw";
import { insertSanitized } from "./insert";

const MACROS =
  '<div data-macro="toc"><ul><li><a data-wiki="#Plan">Plan</a></li></ul></div>' +
  '<div data-macro="include"><ul><li class="task-item"><input type="checkbox" disabled="" /> theirs</li></ul></div>' +
  '<div data-macro="tasks"><ul><li><a data-ref="TM-1">TM-1</a> Write <span data-macro="status">Open</span></li></ul><p data-macro-note="tasks-more" data-macro-arg="3"></p></div>' +
  '<div data-macro="include"><p data-macro-note="include-missing" data-macro-arg="Nowhere"></p></div>' +
  '<div data-macro="include"><p data-macro-note="include-too-deep" data-macro-arg="Deep"></p></div>' +
  '<div data-macro="tasks"><p data-macro-note="tasks-error" data-macro-arg="Unknown field"></p></div>' +
  '<p data-macro-note="weather"></p>';

async function drawn(html: string): Promise<HTMLElement> {
  const element = document.createElement("div");
  insertSanitized(element, html, "reader");
  const text = (id: string, args?: Record<string, string | number>) => `${id}${args ? JSON.stringify(args) : ""}`;
  await drawReader(element, { text, imageSource: () => Promise.resolve(null), current: () => true });
  return element;
}

describe("wiki macros in the reader", () => {
  it("keep the renderer's macro markup through the insertion point", async () => {
    const element = await drawn(MACROS);
    expect(element.querySelectorAll("[data-macro]").length).toBe(7);
    expect(element.querySelector("span[data-macro='status']")?.textContent).toBe("Open");
    const box = element.querySelector<HTMLInputElement>("[data-macro='include'] input");
    expect(box?.disabled).toBe(true);
    expect(box?.hasAttribute("data-item")).toBe(false);
  });

  it("word the notes in the person's language, with numbers for plurals", async () => {
    const element = await drawn(MACROS);
    const notes = [...element.querySelectorAll<HTMLElement>("p[data-macro-note]")];
    const word = (kind: string) => notes.find((note) => note.dataset.macroNote === kind)?.textContent ?? "";
    expect(word("tasks-more")).toContain('reader-macro-tasks-more{"page":"3","count":3');
    expect(word("include-missing")).toContain('"page":"Nowhere"');
    expect(word("include-too-deep")).toContain('"max":3');
    expect(word("tasks-error")).toContain('"error":"Unknown field"');
    // A note the reader doesn't know stays empty.
    expect(word("weather")).toBe("");
  });

  it("drop macro markup the sanitizer doesn't allow there", () => {
    const element = document.createElement("div");
    insertSanitized(element, '<a data-macro-note="x" data-macro="y" onclick="z">a</a>', "reader");
    expect(element.querySelector("[onclick]")).toBeNull();
  });
});
