// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

/**
 * Turns off the habits that would give the app away as a web page: the
 * browser's context menu (the app draws its own), reloading, printing, caret
 * browsing, history keys, zooming, and dragging images and links out of the
 * window. Text fields and selectable content keep their own context menu.
 *
 * The listeners sit on the window, so the app's own key handlers, which sit
 * on the document and below, see the keys first.
 */
export function suppressBrowserBehaviour(win: Window): () => void {
  const onContextMenu = (event: MouseEvent) => {
    const target = event.target;
    if (target instanceof Element && target.closest("input, textarea, [contenteditable], .selectable")) {
      return;
    }
    event.preventDefault();
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (isBrowserKey(event)) {
      event.preventDefault();
    }
  };
  const onWheel = (event: WheelEvent) => {
    // Ctrl and the wheel zooms the page; the app has its own text size.
    if (event.ctrlKey) {
      event.preventDefault();
    }
  };
  const onDragStart = (event: DragEvent) => {
    const target = event.target;
    if (target instanceof Element && target.closest("img, svg, a")) {
      event.preventDefault();
    }
  };

  win.addEventListener("contextmenu", onContextMenu);
  win.addEventListener("keydown", onKeyDown);
  win.addEventListener("wheel", onWheel, { passive: false });
  win.addEventListener("dragstart", onDragStart);
  return () => {
    win.removeEventListener("contextmenu", onContextMenu);
    win.removeEventListener("keydown", onKeyDown);
    win.removeEventListener("wheel", onWheel);
    win.removeEventListener("dragstart", onDragStart);
  };
}

/** Keys the web view would act on by itself. */
export function isBrowserKey(event: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): boolean {
  const mod = event.ctrlKey || event.metaKey;
  const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
  if (["F5", "F7", "BrowserBack", "BrowserForward", "BrowserRefresh", "BrowserHome"].includes(key)) {
    return true; // reload, caret browsing, history
  }
  if (event.altKey && !mod && (key === "ArrowLeft" || key === "ArrowRight" || key === "Home")) {
    return true; // back, forward, home page
  }
  if (mod && ["r", "p", "s", "o", "u", "+", "=", "-", "_", "0"].includes(key)) {
    return true; // reload, print, save, open, source, zoom
  }
  return false;
}
