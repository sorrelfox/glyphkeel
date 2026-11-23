// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * `F6` moves the focus between the window's regions, in visual order:
 * sidebar, header bar, content, status bar. A region is an element with a
 * `data-region` attribute; the focus goes to its first control, or to the
 * region itself.
 */

const FOCUSABLE = [
  "button:not([disabled])",
  "[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(", ");

function shown(element: HTMLElement): boolean {
  return !element.closest("[hidden]") && getComputedStyle(element).display !== "none";
}

export function focusRegion(direction: 1 | -1, doc: Document = document): void {
  const regions = Array.from(doc.querySelectorAll<HTMLElement>("[data-region]")).filter(shown);
  if (regions.length === 0) {
    return;
  }
  const current = regions.findIndex((region) => region.contains(doc.activeElement));
  const index = current === -1 ? (direction === 1 ? 0 : regions.length - 1) : (current + direction + regions.length) % regions.length;
  const region = regions[index];
  if (!region) {
    return;
  }
  const target = Array.from(region.querySelectorAll<HTMLElement>(FOCUSABLE)).find(shown) ?? region;
  target.focus();
}
