// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useRef, type KeyboardEvent, type PointerEvent } from "react";

import { isMenuKey } from "../components/ContextMenu";
import { CLICK_SLOP, useWidthMenu } from "../components/WidthMenu";
import { useT } from "../i18n/I18n";
import styles from "./Shell.module.css";

interface SplitterProps {
  /** The accessible name: what the splitter resizes. */
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
  /** Goes back to the default width; left out while the width is the default. */
  onReset?: (() => void) | undefined;
}

const STEP = 8;
/** One "Wider" or "Narrower" from the handle's menu. */
const MENU_STEP = 24;

/**
 * A vertical divider that resizes the pane on its left: drag it, focus it
 * and use the arrow keys, `Home` and `End`, or click it for a menu that
 * makes the pane wider or narrower or resets it; a double-click resets it.
 */
export function Splitter({ label, value, min, max, onChange, onReset }: SplitterProps) {
  const t = useT();
  const drag = useRef<{ startX: number; startValue: number; moved: boolean } | null>(null);
  const clamp = (next: number) => Math.min(max, Math.max(min, Math.round(next)));
  const widthMenu = useWidthMenu();
  const openMenu = (handle: HTMLElement, click = false) => {
    widthMenu.openAt(handle, { label, value, min, max, step: MENU_STEP, onChange, onReset }, click);
  };

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { startX: event.clientX, startValue: value, moved: false };
  };
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (drag.current && (drag.current.moved || Math.abs(event.clientX - drag.current.startX) > CLICK_SLOP)) {
      drag.current.moved = true;
      onChange(clamp(drag.current.startValue + event.clientX - drag.current.startX));
    }
  };
  const onPointerEnd = (event: PointerEvent<HTMLDivElement>) => {
    const clicked = event.type === "pointerup" && drag.current?.moved === false;
    drag.current = null;
    if (clicked) {
      openMenu(event.currentTarget, true);
    }
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (isMenuKey(event)) {
      event.preventDefault();
      openMenu(event.currentTarget);
      return;
    }
    const next = { ArrowLeft: value - STEP, ArrowRight: value + STEP, Home: min, End: max }[event.key];
    if (next !== undefined) {
      event.preventDefault();
      onChange(clamp(next));
    }
  };

  return (
    <>
      <div
        className={styles.splitter}
        role="separator"
        aria-orientation="vertical"
        aria-label={label}
        aria-valuenow={value}
        aria-valuemin={min}
        aria-valuemax={max}
        title={t("resize-hint")}
        tabIndex={0}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerEnd}
        onPointerCancel={onPointerEnd}
        onKeyDown={onKeyDown}
        onContextMenu={(event) => {
          event.preventDefault();
          openMenu(event.currentTarget);
        }}
      />
      {widthMenu.menu}
    </>
  );
}
