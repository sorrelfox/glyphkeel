// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Turns off the parts of the web view that would give the app away as a web
 * page: the browser's context menu (the app draws its own) and reloading.
 * Text fields and selectable content keep their own context menu.
 */
export function suppressBrowserBehaviour(doc: Document): void {
  doc.addEventListener("contextmenu", (event) => {
    const target = event.target;
    if (target instanceof Element && target.closest("input, textarea, [contenteditable], .selectable")) {
      return;
    }
    event.preventDefault();
  });
  doc.addEventListener("keydown", (event) => {
    const reload = event.key === "F5" || ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "r");
    if (reload) {
      event.preventDefault();
    }
  });
}
