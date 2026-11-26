// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

// The Gantt chart as one picture, for printing and for a PNG file: every
// row the chart shows (with its grouping and folds) and the whole stretch
// of time its tasks cover, as a standalone SVG document in the light theme.
// Right-to-left languages get the table on the right and the timeline
// mirrored, as on screen.

import { type Schedule, type ScheduledTask } from "../../generated/ops";
import { addDays, daysBetween, weekday, type Day } from "../calendar/model";
import { ROW, behindSchedule, elbow, geometry, type Row } from "./model";
import { PX_PER_DAY, UNITS, dayX, headerCells, weekendsBetween, type HeaderCell, type Unit, type Zoom } from "./scale";

/** The picture's colours, resolved from the light theme's tokens. */
export interface Palette {
  surface: string;
  sunken: string;
  border: string;
  fg: string;
  muted: string;
  accent: string;
  bar: string;
  progress: string;
  today: string;
  warning: string;
  danger: string;
  milestone: string;
  overdueBar: string;
  startBar: string;
}

/** The words and numbers in the picture, in the person's language. */
export interface ExportText {
  title: string;
  columns: { key: string; title: string; start: string; due: string; progress: string };
  /** A header cell's label. */
  cell: (cell: HeaderCell, top: boolean) => string;
  /** A day in a table cell. */
  day: (day: Day) => string;
  percent: (share: number) => string;
  /** A group row's text. */
  group: (row: Extract<Row, { kind: "group" }>) => string;
  /** The note after a bar, if any. */
  note: (task: ScheduledTask) => { text: string; tone: "danger" | "warning" } | undefined;
}

export interface ExportInput {
  schedule: Schedule;
  rows: readonly Row[];
  zoom: Zoom;
  rtl: boolean;
  weekStart: number;
  weekend: ReadonlySet<number>;
  palette: Palette;
  text: ExportText;
  font: string;
}

/** Days drawn before the first task and after the last, by zoom level. */
const PAD_DAYS: Readonly<Record<Zoom, number>> = { day: 3, week: 7, month: 15, quarter: 45 };
/** The table's columns and their widths in the picture. */
const TABLE_COLUMNS = [
  ["key", 90],
  ["title", 280],
  ["start", 100],
  ["due", 100],
  ["progress", 56],
] as const;
const TITLE_H = 40;
const HEAD_H = 48;
const PAD = 8;

const escape = (text: string) => text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const n = (value: number) => String(Math.round(value * 100) / 100);

/** The first and last day the picture shows. */
export function exportSpan(schedule: Schedule, zoom: Zoom): { from: Day; to: Day } {
  const start = schedule.range?.start ?? schedule.today;
  const end = schedule.range?.end ?? schedule.today;
  const from = addDays(start < schedule.today ? start : schedule.today, -PAD_DAYS[zoom]);
  const to = addDays(end > schedule.today ? end : schedule.today, PAD_DAYS[zoom]);
  return { from, to };
}

/** The picture as an SVG document, with its size in pixels. */
export function chartSvg(input: ExportInput): { svg: string; width: number; height: number } {
  const { schedule, rows, zoom, rtl, palette: c, text } = input;
  const ppd = PX_PER_DAY[zoom];
  const { from: origin, to } = exportSpan(schedule, zoom);
  const days = daysBetween(origin, to) + 1;
  const timelineW = Math.ceil(days * ppd);
  const tableW = TABLE_COLUMNS.reduce((sum, [, w]) => sum + w, 0);
  const width = tableW + timelineW;
  const bodyTop = TITLE_H + HEAD_H;
  const height = bodyTop + rows.length * ROW + PAD;
  const index = new Map<string, number>();
  rows.forEach((row, at) => {
    if (row.kind !== "group") {
      index.set(row.task.key, at);
    }
  });
  const byKey = new Map(schedule.tasks.map((task) => [task.key, task]));

  // Placing: the table starts at the start edge, the timeline after it.
  const tableX = (x: number) => (rtl ? width - x : x);
  const timeX = (x: number) => (rtl ? timelineW - x : tableW + x);
  const start = rtl ? "end" : "start";
  const end = rtl ? "start" : "end";
  const timeGroup = rtl ? `matrix(-1 0 0 1 ${n(timelineW)} 0)` : `translate(${n(tableW)} 0)`;
  const tableLeft = rtl ? timelineW : 0;

  const out: string[] = [];
  const textEl = (x: number, y: number, body: string, anchor: string, opts: { fill?: string; weight?: number; size?: number; clip?: string } = {}) =>
    `<text x="${n(x)}" y="${n(y)}" text-anchor="${anchor}" fill="${opts.fill ?? c.fg}" font-size="${String(opts.size ?? 12)}"${opts.weight ? ` font-weight="${String(opts.weight)}"` : ""}${opts.clip ? ` clip-path="url(#${opts.clip})"` : ""}>${escape(body)}</text>`;

  out.push(`<rect x="0" y="0" width="${n(width)}" height="${n(height)}" fill="${c.surface}"/>`);
  out.push(textEl(tableX(PAD), 26, text.title, start, { size: 16, weight: 600 }));

  // Header: column names, then the timeline's two rows of cells.
  out.push(`<rect x="0" y="${n(TITLE_H)}" width="${n(width)}" height="${n(HEAD_H)}" fill="${c.sunken}"/>`);
  let colX = 0;
  const colStart: Record<string, number> = {};
  for (const [column, w] of TABLE_COLUMNS) {
    colStart[column] = colX;
    const numeric = column !== "key" && column !== "title";
    const x = numeric ? tableX(colX + w - PAD) : tableX(colX + PAD);
    out.push(textEl(x, TITLE_H + HEAD_H - 10, text.columns[column], numeric ? end : start, { fill: c.muted, weight: 600 }));
    colX += w;
  }
  const [topUnit, lowUnit] = UNITS[zoom];
  const cells = (unit: Unit, y: number, top: boolean) => {
    for (const cell of headerCells(origin, ppd, unit, 0, timelineW, timelineW, input.weekStart)) {
      const x0 = cell.x;
      out.push(`<line x1="${n(timeX(x0))}" x2="${n(timeX(x0))}" y1="${n(y - 16)}" y2="${n(y + 6)}" stroke="${c.border}"/>`);
      if (cell.width >= 14) {
        const weekendCell = cell.unit === "day" && input.weekend.has(weekday(cell.day));
        out.push(textEl(timeX(x0 + 4), y, text.cell(cell, top), start, { fill: weekendCell ? c.muted : c.fg, size: 11 }));
      }
    }
  };
  cells(topUnit, TITLE_H + 18, true);
  cells(lowUnit, TITLE_H + 40, false);

  // The timeline's shapes, in its own coordinates (mirrored for right to left).
  const g: string[] = [];
  const bodyH = rows.length * ROW;
  if (zoom === "day" || zoom === "week") {
    for (const at of weekendsBetween(origin, 0, days, input.weekend)) {
      g.push(`<rect x="${n(at * ppd)}" y="0" width="${n(ppd)}" height="${n(bodyH)}" fill="${c.sunken}"/>`);
    }
  }
  rows.forEach((row, at) => {
    if (row.kind === "group") {
      g.push(`<rect x="0" y="${n(at * ROW)}" width="${n(timelineW)}" height="${n(ROW)}" fill="${c.sunken}"/>`);
    }
    g.push(`<line x1="0" x2="${n(timelineW)}" y1="${n((at + 1) * ROW)}" y2="${n((at + 1) * ROW)}" stroke="${c.border}" stroke-opacity="0.5"/>`);
  });
  const todayX = dayX(origin, schedule.today, ppd) + ppd / 2;
  g.push(`<line x1="${n(todayX)}" x2="${n(todayX)}" y1="0" y2="${n(bodyH)}" stroke="${c.today}" stroke-width="2"/>`);
  for (const dependency of schedule.dependencies) {
    const a = byKey.get(dependency.from);
    const b = byKey.get(dependency.to);
    const ya = index.get(dependency.from);
    const yb = index.get(dependency.to);
    const ga = a && geometry(a, origin, ppd);
    const gb = b && geometry(b, origin, ppd);
    if (!ga || !gb || ya === undefined || yb === undefined) {
      continue;
    }
    const colour = dependency.violated ? c.warning : c.muted;
    g.push(`<path d="${elbow(ga.x2, ya * ROW + ROW / 2, gb.x1, yb * ROW + ROW / 2)}" fill="none" stroke="${colour}" stroke-width="1.5"/>`);
    const tipX = gb.x1;
    const tipY = yb * ROW + ROW / 2;
    g.push(`<path d="M${n(tipX - 6)},${n(tipY - 3)} L${n(tipX)},${n(tipY)} L${n(tipX - 6)},${n(tipY + 3)} z" fill="${colour}"/>`);
  }
  rows.forEach((row, at) => {
    if (row.kind !== "task") {
      return;
    }
    const task = row.task;
    const geo = geometry(task, origin, ppd);
    if (!geo) {
      return;
    }
    const cy = at * ROW + ROW / 2;
    const w = geo.x2 - geo.x1;
    const faded = task.closed ? ` opacity="0.55"` : "";
    switch (geo.shape) {
      case "summary":
        g.push(`<rect x="${n(geo.x1)}" y="${n(cy - 4)}" width="${n(w)}" height="8" rx="2" fill="${c.muted}"${faded}/>`);
        break;
      case "milestone": {
        const cx = (geo.x1 + geo.x2) / 2;
        g.push(`<rect x="${n(cx - 5)}" y="${n(cy - 5)}" width="10" height="10" rx="2" fill="${c.milestone}" transform="rotate(45 ${n(cx)} ${n(cy)})"${faded}/>`);
        break;
      }
      case "deadline": {
        const x = geo.x2;
        g.push(`<path d="M${n(x - 9)},${n(cy - 9)} h5 a4,4 0 0 1 4,4 v10 a4,4 0 0 1 -4,4 h-5" fill="none" stroke="${c.accent}" stroke-width="2"${faded}/>`);
        break;
      }
      default: {
        const fill = task.overdue ? c.overdueBar : geo.shape === "no_deadline" ? c.startBar : c.bar;
        g.push(`<rect x="${n(geo.x1)}" y="${n(cy - 9)}" width="${n(w)}" height="18" rx="4" fill="${fill}"${faded}/>`);
        const done = Math.max(0, Math.min(100, task.progress_percent));
        if (done > 0 && geo.shape === "bar") {
          g.push(`<rect x="${n(geo.x1)}" y="${n(cy - 9)}" width="${n((w * done) / 100)}" height="18" rx="4" fill="${task.overdue ? c.danger : c.progress}"${faded}/>`);
        }
        if (behindSchedule(task, schedule.today)) {
          g.push(`<rect x="${n(geo.x1)}" y="${n(cy - 9)}" width="${n(w)}" height="18" rx="4" fill="none" stroke="${c.warning}" stroke-width="2" stroke-dasharray="4 3"/>`);
        }
      }
    }
  });

  out.push(`<g transform="translate(0 ${n(bodyTop)})"><g transform="${timeGroup}">${g.join("")}</g></g>`);

  // The table's text, the group rows' text and the notes after the bars.
  out.push(`<line x1="${n(tableX(tableW))}" x2="${n(tableX(tableW))}" y1="${n(TITLE_H)}" y2="${n(height)}" stroke="${c.border}"/>`);
  const titleClip = "gantt-title-clip";
  out.push(`<defs><clipPath id="${titleClip}"><rect x="${n(tableLeft + (rtl ? tableW - (colStart.title ?? 0) - 280 : (colStart.title ?? 0)))}" y="0" width="${n(280 - PAD)}" height="${n(height)}"/></clipPath></defs>`);
  rows.forEach((row, at) => {
    const y = bodyTop + at * ROW + ROW / 2 + 4;
    if (row.kind === "group") {
      out.push(`<rect x="${n(tableLeft)}" y="${n(bodyTop + at * ROW)}" width="${n(tableW)}" height="${n(ROW)}" fill="${c.sunken}"/>`);
      out.push(textEl(tableX(PAD), y, text.group(row), start, { weight: 600 }));
      return;
    }
    const task = row.task;
    const scheduled = row.kind === "task" ? row.task : undefined;
    const fill = task.closed ? c.muted : c.fg;
    const keyX = colStart.key ?? 0;
    out.push(textEl(tableX(keyX + PAD), y, task.key, start, { fill: c.muted }));
    const indent = row.kind === "task" ? (row.level - 1) * 14 : 0;
    out.push(textEl(tableX((colStart.title ?? 0) + PAD + indent), y, task.title, start, { fill, clip: titleClip, weight: scheduled?.shape === "summary" ? 600 : 400 }));
    const cell = (column: "start" | "due" | "progress", value: string) => {
      const [, w] = TABLE_COLUMNS.find(([name]) => name === column) ?? ["", 0];
      out.push(textEl(tableX((colStart[column] ?? 0) + w - PAD), y, value, end, { fill }));
    };
    if (scheduled) {
      cell("start", scheduled.start_date === undefined ? "" : text.day(scheduled.start_date));
      cell("due", scheduled.due_date === undefined ? "" : text.day(scheduled.due_date));
      cell("progress", text.percent(scheduled.progress_percent / 100));
      const geo = geometry(scheduled, origin, ppd);
      const note = text.note(scheduled);
      if (geo && note) {
        out.push(textEl(timeX(geo.x2 + 8), y, note.text, start, { fill: note.tone === "danger" ? c.danger : c.warning, size: 11 }));
      }
    }
  });

  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${String(width)}" height="${String(height)}" viewBox="0 0 ${String(width)} ${String(height)}" font-family="${escape(input.font)}" direction="${rtl ? "rtl" : "ltr"}">${out.join("")}</svg>`;
  return { svg, width, height };
}

/** The largest canvas side and area browsers draw reliably. */
const MAX_SIDE = 16_000;
const MAX_AREA = 120_000_000;

/** How much to scale a picture of `width` × `height` so a canvas can hold it; at most twice for sharpness. */
export function pngScale(width: number, height: number): number {
  return Math.min(2, MAX_SIDE / width, MAX_SIDE / height, Math.sqrt(MAX_AREA / (width * height)));
}

/** The picture as a PNG file. */
export async function svgToPng(svg: string, width: number, height: number): Promise<Uint8Array> {
  const scale = pngScale(width, height);
  const image = new Image();
  image.decoding = "async";
  image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  await image.decode();
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.floor(width * scale));
  canvas.height = Math.max(1, Math.floor(height * scale));
  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("canvas");
  }
  context.scale(scale, scale);
  context.drawImage(image, 0, 0, width, height);
  const blob = await new Promise<Blob | null>((resolve) => {
    canvas.toBlob(resolve, "image/png");
  });
  if (!blob) {
    throw new Error("png");
  }
  return new Uint8Array(await blob.arrayBuffer());
}

/** The light theme's colours, read from the app's tokens with the person's accent colour. */
export function lightPalette(): Palette {
  const probe = document.createElement("div");
  probe.setAttribute("data-theme", "light");
  const accent = document.documentElement.closest("[data-accent]")?.getAttribute("data-accent") ?? document.querySelector("[data-accent]")?.getAttribute("data-accent");
  if (accent) {
    probe.setAttribute("data-accent", accent);
  }
  probe.style.position = "absolute";
  probe.style.visibility = "hidden";
  document.body.append(probe);
  const resolve = (expression: string) => {
    probe.style.color = expression;
    return getComputedStyle(probe).color;
  };
  try {
    return {
      surface: resolve("var(--surface)"),
      sunken: resolve("var(--sunken)"),
      border: resolve("var(--border)"),
      fg: resolve("var(--fg)"),
      muted: resolve("var(--fg-muted)"),
      accent: resolve("var(--accent)"),
      bar: resolve("var(--gantt-bar)"),
      progress: resolve("var(--gantt-progress)"),
      today: resolve("var(--gantt-today)"),
      warning: resolve("var(--warning)"),
      danger: resolve("var(--danger)"),
      milestone: resolve("var(--hue-purple)"),
      overdueBar: resolve("color-mix(in srgb, var(--danger) 30%, var(--surface))"),
      startBar: resolve("color-mix(in srgb, var(--warning) 30%, var(--surface))"),
    };
  } finally {
    probe.remove();
  }
}

/** The attribute that marks the print sheet. */
const PRINT_MARK = "data-gantt-print";

/**
 * Prints the picture through the system's print dialog, which also saves
 * PDF files: while printing, the page shows only the picture, scaled to the
 * paper's width in landscape.
 */
export function printSvg(svg: string): void {
  document.querySelectorAll(`[${PRINT_MARK}]`).forEach((old) => {
    old.remove();
  });
  const sheet = document.createElement("img");
  sheet.setAttribute(PRINT_MARK, "");
  sheet.alt = "";
  sheet.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  const style = document.createElement("style");
  style.setAttribute(PRINT_MARK, "");
  style.textContent = `@media screen { img[${PRINT_MARK}] { display: none; } }
@media print {
  @page { size: landscape; margin: 10mm; }
  body > :not([${PRINT_MARK}]) { display: none !important; }
  img[${PRINT_MARK}] { display: block; inline-size: 100%; block-size: auto; }
}`;
  document.head.append(style);
  document.body.append(sheet);
  const done = () => {
    sheet.remove();
    style.remove();
    window.removeEventListener("afterprint", done);
  };
  window.addEventListener("afterprint", done);
  // Print once the picture is ready, or it prints blank.
  void sheet
    .decode()
    .catch(() => undefined)
    .then(() => {
      window.print();
    });
}
