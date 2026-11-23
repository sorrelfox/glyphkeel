// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useRef, type KeyboardEvent, type PointerEvent } from "react";

import styles from "./Shell.module.css";

interface SplitterProps {
  /** The accessible name: what the splitter resizes. */
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}

const STEP = 8;

/**
 * A vertical divider that resizes the pane on its left: drag it, or focus it
 * and use the arrow keys, `Home` and `End`.
 */
export function Splitter({ label, value, min, max, onChange }: SplitterProps) {
  const drag = useRef<{ startX: number; startValue: number } | null>(null);
  const clamp = (next: number) => Math.min(max, Math.max(min, Math.round(next)));

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { startX: event.clientX, startValue: value };
  };
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (drag.current) {
      onChange(clamp(drag.current.startValue + event.clientX - drag.current.startX));
    }
  };
  const onPointerEnd = () => {
    drag.current = null;
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const next = { ArrowLeft: value - STEP, ArrowRight: value + STEP, Home: min, End: max }[event.key];
    if (next !== undefined) {
      event.preventDefault();
      onChange(clamp(next));
    }
  };

  return (
    <div
      className={styles.splitter}
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
      onKeyDown={onKeyDown}
    />
  );
}
