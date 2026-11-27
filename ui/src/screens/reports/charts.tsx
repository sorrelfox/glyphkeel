// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { type Category, type CreatedClosedRow, type CumulativeFlowRow, type CycleTimeRow, type CycleTimeTask, type FlowBand } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { bandPath, daysBetween, INSETS, labelStep, linePath, niceAxis, slots, stack, stepIndex, yScale } from "./chartMath";
import styles from "./FlowReports.module.css";

const HEIGHT = 260;
/** About how wide a period label is; labels are thinned out so that they don't overlap. */
const LABEL_WIDTH = 64;

/** The colour token of a status category's band. */
export function categoryColour(category: Category): string {
  switch (category) {
    case "todo":
      return "var(--hue-grey)";
    case "in_progress":
      return "var(--hue-blue)";
    case "done":
      return "var(--hue-green)";
    case "cancelled":
      return "var(--hue-red)";
  }
}

export function bandColour(band: FlowBand): string {
  return band.hue === undefined ? categoryColour(band.category) : `var(--hue-${band.hue})`;
}

/** The chart's width, following its box. */
function useWidth(): [React.RefObject<HTMLDivElement | null>, number] {
  const ref = useRef<HTMLDivElement | null>(null);
  const [width, setWidth] = useState(720);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof ResizeObserver === "undefined") {
      return;
    }
    const measure = () => {
      if (el.clientWidth > 0) {
        setWidth(el.clientWidth);
      }
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => {
      observer.disconnect();
    };
  }, []);
  return [ref, width];
}

interface FrameProps {
  /** The chart's name for screen readers. */
  label: string;
  /** A sentence that sums the chart up, read with it and shown under it. */
  summary: string;
  /** How many periods the arrow keys move between. */
  count: number;
  /** What the period of `index` holds, read out and shown while it is chosen. */
  describe: (index: number) => ReactNode;
  /** The legend, as list items. */
  legend?: ReactNode;
  children: (props: { width: number; rtl: boolean; chosen: number | null }) => ReactNode;
}

/**
 * A chart that can be read without seeing it: the drawing is one image with
 * a name and a summary; the frame takes the focus, and the arrow keys (in
 * the reading direction) choose a period, whose figures show under the
 * chart and are announced.
 */
export function ChartFrame({ label, summary, count, describe, legend, children }: FrameProps) {
  const t = useT();
  const { direction } = useLocale();
  const rtl = direction === "rtl";
  const [ref, width] = useWidth();
  const [chosen, setChosen] = useState<number | null>(null);
  const summaryId = useId();
  const index = chosen !== null && chosen < count ? chosen : null;
  const onKey = (event: KeyboardEvent<HTMLDivElement>) => {
    // Nothing chosen yet: forward starts at the first period, back at the last.
    const back = event.key === (rtl ? "ArrowRight" : "ArrowLeft");
    const next = stepIndex(event.key, index ?? (back ? count : -1), count, rtl);
    if (next !== null && count > 0) {
      event.preventDefault();
      setChosen(next);
    } else if (event.key === "Escape" && index !== null) {
      event.preventDefault();
      setChosen(null);
    }
  };
  return (
    <figure className={styles.figure}>
      <div
        ref={ref}
        className={styles.chart}
        tabIndex={0}
        role="group"
        aria-label={label}
        aria-description={t("flow-chart-keys")}
        data-chart
        onKeyDown={onKey}
        onBlur={() => {
          setChosen(null);
        }}
      >
        <svg role="img" aria-label={label} aria-describedby={summaryId} width={width} height={HEIGHT} viewBox={`0 0 ${String(width)} ${String(HEIGHT)}`} className={styles.svg}>
          {children({ width, rtl, chosen: index })}
        </svg>
      </div>
      <span id={summaryId} className={styles.srOnly}>
        {summary}
      </span>
      {legend !== undefined && <ul className={styles.legend}>{legend}</ul>}
      <figcaption className={classes("muted", styles.caption)} aria-live="polite" data-chart-readout>
        {index === null ? summary : describe(index)}
      </figcaption>
    </figure>
  );
}

export function Swatch({ colour, line = false }: { colour: string; line?: boolean }) {
  return <i className={classes(styles.swatch, line && styles.swatchLine)} style={{ backgroundColor: colour }} aria-hidden="true" />;
}

interface AxisProps {
  width: number;
  rtl: boolean;
  max: number;
  ticks: readonly number[];
  format: (n: number) => string;
  labels: readonly string[];
  chosen: number | null;
}

/** The value grid, its labels on the start side, and the period labels under the plot. */
function Axes({ width, rtl, max, ticks, format, labels, chosen }: AxisProps) {
  const s = slots(labels.length, width, rtl);
  const y = yScale(max, HEIGHT);
  const step = labelStep(labels.length, s.size, LABEL_WIDTH);
  const anchorStart = rtl ? "start" : "end";
  const labelX = rtl ? s.startEdge + 6 : s.startEdge - 6;
  return (
    <g aria-hidden="true">
      {ticks.map((tick) => (
        <g key={tick}>
          <line className={styles.grid} x1={Math.min(s.startEdge, s.endEdge)} x2={Math.max(s.startEdge, s.endEdge)} y1={y(tick)} y2={y(tick)} />
          <text className={styles.tick} x={labelX} y={y(tick)} dy="0.32em" textAnchor={anchorStart}>
            {format(tick)}
          </text>
        </g>
      ))}
      {chosen !== null && <rect className={styles.chosen} x={s.left(chosen)} y={INSETS.top} width={s.size} height={HEIGHT - INSETS.top - INSETS.bottom} />}
      {labels.map((text, i) =>
        i % step === 0 ? (
          <text key={i} className={styles.tick} x={s.centre(i)} y={HEIGHT - INSETS.bottom + 16} textAnchor="middle">
            {text}
          </text>
        ) : null,
      )}
    </g>
  );
}

// ---------------------------------------------------------------------

export const CREATED = "var(--hue-blue)";
export const CLOSED = "var(--hue-green)";
export const OPEN = "var(--hue-orange)";

/** Bars of tasks created and closed per period, and a line of the open tasks at each period's end. */
export function CreatedClosedChart({ rows, labels, label, summary, describe, legend }: { rows: readonly CreatedClosedRow[]; labels: readonly string[]; label: string; summary: string; describe: (i: number) => ReactNode; legend: ReactNode }) {
  const { locale } = useLocale();
  const number = new Intl.NumberFormat(locale);
  const axis = niceAxis(Math.max(1, ...rows.flatMap((r) => [r.created, r.closed, r.open])));
  return (
    <ChartFrame label={label} summary={summary} count={rows.length} describe={describe} legend={legend}>
      {({ width, rtl, chosen }) => {
        const s = slots(rows.length, width, rtl);
        const y = yScale(axis.max, HEIGHT);
        const bar = Math.max(1, Math.min(24, s.size * 0.36));
        const gap = Math.min(2, s.size * 0.04);
        // The created bar comes first in the reading direction.
        const offset = (first: boolean) => (first !== rtl ? -bar - gap / 2 : gap / 2);
        return (
          <>
            <Axes width={width} rtl={rtl} max={axis.max} ticks={axis.ticks} format={(n) => number.format(n)} labels={labels} chosen={chosen} />
            <g aria-hidden="true">
              {rows.map((r, i) => (
                <g key={r.start}>
                  <rect x={s.centre(i) + offset(true)} y={y(r.created)} width={bar} height={y(0) - y(r.created)} fill={CREATED} data-bar="created" />
                  <rect x={s.centre(i) + offset(false)} y={y(r.closed)} width={bar} height={y(0) - y(r.closed)} fill={CLOSED} data-bar="closed" />
                </g>
              ))}
              <path className={styles.line} d={linePath(rows.map((r, i) => [s.centre(i), y(r.open)]))} stroke={OPEN} />
              {rows.map((r, i) => (
                <circle key={r.start} cx={s.centre(i)} cy={y(r.open)} r={2.5} fill={OPEN} />
              ))}
            </g>
          </>
        );
      }}
    </ChartFrame>
  );
}

/** Stacked bands of the tasks in each status, the last status at the bottom. */
export function CumulativeFlowChart({ rows, bands, labels, label, summary, describe, legend }: { rows: readonly CumulativeFlowRow[]; bands: readonly FlowBand[]; labels: readonly string[]; label: string; summary: string; describe: (i: number) => ReactNode; legend: ReactNode }) {
  const { locale } = useLocale();
  const number = new Intl.NumberFormat(locale);
  const totals = rows.map((r) => r.counts.reduce((sum, n) => sum + n, 0));
  const axis = niceAxis(Math.max(1, ...totals));
  const order = bands.map((_band, i) => bands.length - 1 - i);
  const stacked = stack(
    rows.map((r) => r.counts),
    order,
  );
  return (
    <ChartFrame label={label} summary={summary} count={rows.length} describe={describe} legend={legend}>
      {({ width, rtl, chosen }) => {
        const s = slots(rows.length, width, rtl);
        const y = yScale(axis.max, HEIGHT);
        // A single period still shows as a band across its slot.
        const xs = rows.length === 1 ? [s.left(0), s.left(0) + s.size] : rows.map((_row, i) => s.centre(i));
        const at = (i: number) => (rows.length === 1 ? 0 : i);
        return (
          <>
            <Axes width={width} rtl={rtl} max={axis.max} ticks={axis.ticks} format={(n) => number.format(n)} labels={labels} chosen={chosen} />
            <g aria-hidden="true">
              {bands.map((band, b) => (
                <path
                  key={band.key}
                  className={styles.band}
                  data-band={band.key}
                  fill={bandColour(band)}
                  d={bandPath(
                    xs,
                    xs.map((_x, i) => y(stacked[at(i)]?.[b]?.low ?? 0)),
                    xs.map((_x, i) => y(stacked[at(i)]?.[b]?.high ?? 0)),
                  )}
                />
              ))}
            </g>
          </>
        );
      }}
    </ChartFrame>
  );
}

/** One dot per closed task at the day it closed, by how long it took, with the median and 85th percentile. */
export function CycleTimeChart({
  rows,
  tasks,
  metric,
  from,
  to,
  labels,
  label,
  summary,
  describe,
  legend,
  median,
  p85,
}: {
  rows: readonly CycleTimeRow[];
  tasks: readonly CycleTimeTask[];
  metric: "cycle" | "lead";
  from: string;
  to: string;
  labels: readonly string[];
  label: string;
  summary: string;
  describe: (i: number) => ReactNode;
  legend: ReactNode;
  median: number | undefined;
  p85: number | undefined;
}) {
  const { locale } = useLocale();
  const days = new Intl.NumberFormat(locale, { style: "unit", unit: "day", unitDisplay: "narrow", maximumFractionDigits: 1 });
  const value = (task: CycleTimeTask) => (metric === "cycle" ? task.cycle_days : task.lead_days);
  const points = tasks.flatMap((task) => {
    const v = value(task);
    return v === undefined ? [] : [{ task, v }];
  });
  const axis = niceAxis(Math.max(1, ...points.map((p) => p.v)));
  const span = Math.max(1, daysBetween(from, to) + 1);
  return (
    <ChartFrame label={label} summary={summary} count={rows.length} describe={describe} legend={legend}>
      {({ width, rtl, chosen }) => {
        const s = slots(rows.length, width, rtl);
        const y = yScale(axis.max, HEIGHT);
        const x = (day: string) => s.x(((daysBetween(from, day) + 0.5) / span) * s.plot);
        const level = (v: number | undefined, colour: string, key: string) =>
          v === undefined ? null : <line key={key} className={styles.level} x1={s.startEdge} x2={s.endEdge} y1={y(v)} y2={y(v)} stroke={colour} data-level={key} />;
        return (
          <>
            <Axes width={width} rtl={rtl} max={axis.max} ticks={axis.ticks} format={(n) => days.format(n)} labels={labels} chosen={chosen} />
            <g aria-hidden="true">
              {points.map(({ task, v }) => (
                <circle key={task.key} className={styles.dot} cx={x(task.closed_day)} cy={y(v)} r={3} data-dot={task.key} />
              ))}
              {level(median, "var(--accent)", "median")}
              {level(p85, "var(--hue-orange)", "p85")}
            </g>
          </>
        );
      }}
    </ChartFrame>
  );
}

// ---------------------------------------------------------------------

export const REMAINING = "var(--accent)";
export const IDEAL = "var(--fg-faint)";
export const SCOPE = "var(--hue-grey)";

/**
 * A sprint's burndown: the work still open at the end of each day, the
 * ideal line from the work at the start down to none on the last day, and
 * the sprint's scope (all its work), which rises when work is added.
 * `remaining` and `scope` are left out for the days still to come.
 */
export function BurndownChart({
  remaining,
  scope,
  ideal,
  labels,
  label,
  summary,
  describe,
  legend,
  format,
}: {
  remaining: readonly (number | undefined)[];
  scope: readonly (number | undefined)[];
  ideal: readonly number[];
  labels: readonly string[];
  label: string;
  summary: string;
  describe: (i: number) => ReactNode;
  legend: ReactNode;
  format: (n: number) => string;
}) {
  const known = (values: readonly (number | undefined)[]) => values.flatMap((v) => (v === undefined ? [] : [v]));
  const axis = niceAxis(Math.max(1, ...ideal, ...known(remaining), ...known(scope)));
  return (
    <ChartFrame label={label} summary={summary} count={labels.length} describe={describe} legend={legend}>
      {({ width, rtl, chosen }) => {
        const s = slots(labels.length, width, rtl);
        const y = yScale(axis.max, HEIGHT);
        const points = (values: readonly (number | undefined)[]) => values.flatMap((v, i) => (v === undefined ? [] : [[s.centre(i), y(v)] as const]));
        return (
          <>
            <Axes width={width} rtl={rtl} max={axis.max} ticks={axis.ticks} format={format} labels={labels} chosen={chosen} />
            <g aria-hidden="true">
              <path className={styles.level} d={linePath(ideal.map((v, i) => [s.centre(i), y(v)]))} stroke={IDEAL} data-line="ideal" />
              <path className={styles.line} d={linePath(points(scope))} stroke={SCOPE} strokeOpacity={0.6} data-line="scope" />
              <path className={styles.line} d={linePath(points(remaining))} stroke={REMAINING} data-line="remaining" />
              {points(remaining).map(([cx, cy]) => (
                <circle key={`${String(cx)}-${String(cy)}`} cx={cx} cy={cy} r={2.5} fill={REMAINING} />
              ))}
            </g>
          </>
        );
      }}
    </ChartFrame>
  );
}
