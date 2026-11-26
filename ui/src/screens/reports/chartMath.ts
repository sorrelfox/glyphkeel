// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The geometry of the report charts, apart from React: scales that run from
// the start edge (left, or right in right-to-left text), round axis steps,
// stacked bands and which period labels fit.

/** The space around the plot, in pixels, by logical side. */
export interface Insets {
  start: number;
  end: number;
  top: number;
  bottom: number;
}

export const INSETS: Insets = { start: 44, end: 16, top: 12, bottom: 28 };

/** An axis from 0 to a round maximum, with its steps. */
export interface Axis {
  max: number;
  ticks: number[];
}

/** A round axis that holds `max`, with about `steps` steps. */
export function niceAxis(max: number, steps = 4): Axis {
  if (!(max > 0)) {
    return { max: 1, ticks: [0, 1] };
  }
  const raw = max / steps;
  const power = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * power).find((s) => s >= raw) ?? 10 * power;
  const top = Math.ceil(max / step) * step;
  const ticks: number[] = [];
  for (let v = 0; v <= top + step / 2; v += step) {
    ticks.push(Math.round(v * 1000) / 1000);
  }
  return { max: top, ticks };
}

/**
 * The horizontal scale of `count` equal slots over a chart `width` wide:
 * where each slot starts and its centre, measured from the left edge, with
 * the first slot at the start edge.
 */
export function slots(count: number, width: number, rtl: boolean, insets: Insets = INSETS) {
  const plot = Math.max(1, width - insets.start - insets.end);
  const size = plot / Math.max(1, count);
  /** A position along the time axis (0 at the start edge) as an x from the left. */
  const x = (along: number) => (rtl ? width - insets.start - along : insets.start + along);
  return {
    size,
    plot,
    x,
    /** The left edge of slot `i`. */
    left: (i: number) => (rtl ? x((i + 1) * size) : x(i * size)),
    centre: (i: number) => x((i + 0.5) * size),
    /** The x of the start and the end edge of the plot. */
    startEdge: x(0),
    endEdge: x(plot),
  };
}

/** The vertical scale: `value` (0 to `max`) as a y in a chart `height` high. */
export function yScale(max: number, height: number, insets: Insets = INSETS) {
  const plot = Math.max(1, height - insets.top - insets.bottom);
  return (value: number) => insets.top + plot - (Math.max(0, value) / (max || 1)) * plot;
}

/** Every `n`th label is shown so that labels about `labelWidth` wide don't overlap. */
export function labelStep(count: number, slotSize: number, labelWidth: number): number {
  if (count === 0 || slotSize <= 0) {
    return 1;
  }
  return Math.max(1, Math.ceil(labelWidth / slotSize));
}

/**
 * Stacked bands: for each period, each band's lower and upper value, the
 * bands stacked in `order` from the bottom up.
 */
export function stack(rows: readonly (readonly number[])[], order: readonly number[]): { low: number; high: number }[][] {
  return rows.map((row) => {
    const out: { low: number; high: number }[] = row.map(() => ({ low: 0, high: 0 }));
    let base = 0;
    for (const band of order) {
      const value = row[band] ?? 0;
      out[band] = { low: base, high: base + value };
      base += value;
    }
    return out;
  });
}

/** The SVG path of one stacked band through the periods' centres. */
export function bandPath(xs: readonly number[], low: readonly number[], high: readonly number[]): string {
  if (xs.length === 0) {
    return "";
  }
  const up = xs.map((x, i) => `${i === 0 ? "M" : "L"}${String(round(x))},${String(round(high[i] ?? 0))}`);
  const down = xs.map((x, i) => `L${String(round(x))},${String(round(low[i] ?? 0))}`).reverse();
  return `${up.join("")}${down.join("")}Z`;
}

/** The SVG path of a line through points. */
export function linePath(points: readonly (readonly [number, number])[]): string {
  return points.map(([x, y], i) => `${i === 0 ? "M" : "L"}${String(round(x))},${String(round(y))}`).join("");
}

function round(n: number): number {
  return Math.round(n * 10) / 10;
}

/** The index after a key press among `count` items, or null for other keys; arrows follow the reading direction. */
export function stepIndex(key: string, index: number, count: number, rtl: boolean): number | null {
  const forward = rtl ? -1 : 1;
  switch (key) {
    case "ArrowRight":
      return clamp(index + forward, count);
    case "ArrowLeft":
      return clamp(index - forward, count);
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

function clamp(i: number, count: number): number {
  return Math.min(Math.max(0, i), count - 1);
}

/** Days between two `YYYY-MM-DD` days. */
export function daysBetween(from: string, to: string): number {
  return Math.round((Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / 86_400_000);
}
