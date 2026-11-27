// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useEffect, useRef, useState } from "react";
import { Menu, Popover } from "react-aria-components";

import { useT } from "../i18n/I18n";
import { MenuEntries } from "./ContextMenu";

/** What a width menu changes: the width now, its limits and one step. */
export interface WidthTarget {
  /** The menu's name: the handle's own. */
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
  /** Goes back to the default width, dropping the one kept; left out where there is nothing to drop. */
  onReset?: (() => void) | undefined;
}

/** A press shorter than this, in pixels, is a click, not a drag. */
export const CLICK_SLOP = 3;

/** A second click within this time, in milliseconds, makes a double-click: the width goes back to its default. */
export const DOUBLE_CLICK_MS = 300;

/**
 * "Wider", "Narrower" and "Reset width" for a resize handle: the way to
 * change a width with one click or tap, without dragging. The handle opens
 * it when it is clicked without moving, or with the context-menu key; a
 * double-click resets the width at once.
 */
export function useWidthMenu() {
  const t = useT();
  const anchor = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState<{ x: number; y: number; target: WidthTarget } | null>(null);
  // A click waits briefly for a second one before its menu opens.
  const pending = useRef<{ handle: HTMLElement; timer: ReturnType<typeof setTimeout> } | null>(null);
  useEffect(
    () => () => {
      if (pending.current) {
        clearTimeout(pending.current.timer);
      }
    },
    [],
  );

  const show = (handle: HTMLElement, target: WidthTarget) => {
    // Focus returns to the handle when the menu closes.
    handle.focus();
    const rect = handle.getBoundingClientRect();
    setOpen({ x: rect.left + rect.width / 2, y: rect.top + Math.min(rect.height, 24), target });
  };

  /** Opens the menu for `handle`; after a click (`click`), only once no second click follows, which resets the width instead. */
  const openAt = (handle: HTMLElement, target: WidthTarget, click = false) => {
    const waiting = pending.current;
    if (waiting) {
      clearTimeout(waiting.timer);
      pending.current = null;
      if (click && waiting.handle === handle && target.onReset) {
        handle.focus();
        target.onReset();
        return;
      }
    }
    if (!click || !target.onReset) {
      show(handle, target);
      return;
    }
    pending.current = {
      handle,
      timer: setTimeout(() => {
        pending.current = null;
        show(handle, target);
      }, DOUBLE_CLICK_MS),
    };
  };

  const close = () => {
    setOpen(null);
  };

  const menu = open && (
    <>
      {/* The handle's position is physical in every writing direction. */}
      {/* eslint-disable-next-line no-restricted-syntax */}
      <div ref={anchor} style={{ position: "fixed", left: open.x, top: open.y, width: 0, height: 0 }} />
      <Popover
        className="pop menuPopover"
        triggerRef={anchor}
        placement="bottom"
        offset={2}
        isOpen
        onOpenChange={(isOpen) => {
          if (!isOpen) {
            close();
          }
        }}
      >
        <Menu className="menu" aria-label={open.target.label} autoFocus="first">
          <MenuEntries
            entries={[
              {
                id: "wider",
                label: t("resize-wider"),
                disabled: open.target.value >= open.target.max,
                run: () => {
                  open.target.onChange(Math.min(open.target.max, Math.round(open.target.value + open.target.step)));
                },
              },
              {
                id: "narrower",
                label: t("resize-narrower"),
                disabled: open.target.value <= open.target.min,
                run: () => {
                  open.target.onChange(Math.max(open.target.min, Math.round(open.target.value - open.target.step)));
                },
              },
              {
                id: "reset",
                label: t("resize-reset"),
                disabled: !open.target.onReset,
                run: () => {
                  open.target.onReset?.();
                },
              },
            ]}
            onClose={close}
          />
        </Menu>
      </Popover>
    </>
  );

  return { openAt, menu };
}
