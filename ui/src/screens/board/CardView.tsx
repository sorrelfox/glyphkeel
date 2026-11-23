// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { CalendarClock, CalendarDays, Clock, Hourglass, SquareCheck, Workflow } from "lucide-react";
import { forwardRef, type CSSProperties, type HTMLAttributes } from "react";
import { useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { PriorityGlyph, TypeGlyph } from "../../components/Glyphs";
import { type Card, type CardField } from "../../data/catalog";
import { useT } from "../../i18n/I18n";
import { dueState, formatMinutes } from "./model";
import styles from "./Board.module.css";

interface CardViewProps extends HTMLAttributes<HTMLElement> {
  card: Card;
  fields: readonly string[];
  compact: boolean;
  today: string;
  selected?: boolean;
  /** The slot a dragged card leaves behind. */
  ghost?: boolean;
  /** The card that follows the pointer. */
  lifted?: boolean;
  style?: CSSProperties;
}

/** A due date as the card shows it: "Nov 24". Days carry no time zone. */
export function useDayFormat(): (day: string) => string {
  const { locale } = useLocale();
  const format = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", timeZone: "UTC" });
  return (day) => format.format(new Date(`${day}T00:00:00Z`));
}

/**
 * A task on the board. Rows, each left out when empty: type, key and
 * priority; the title; labels; start and due dates, checklist, subtasks,
 * estimate and days in the column, as the personal card fields choose.
 */
export const CardView = forwardRef<HTMLElement, CardViewProps>(function CardView(
  { card, fields, compact, today, selected = false, ghost = false, lifted = false, className, ...rest },
  ref,
) {
  const t = useT();
  const day = useDayFormat();
  const show = (field: CardField) => !compact && fields.includes(field);
  const closed = card.closed_at !== undefined;
  const due = dueState(card.due_date, closed, today);
  const days = Math.max(0, Math.floor((Date.parse(`${today}T23:59:59`) - Date.parse(card.status_entered_at)) / 86_400_000));
  const meta = [
    show("start") && card.start_date !== undefined && (
      <span key="start" className="meta" aria-label={t("card-start", { day: day(card.start_date) })}>
        <CalendarClock className="ic" />
        {day(card.start_date)}
      </span>
    ),
    show("due") && card.due_date !== undefined && (
      <span key="due" className={classes("meta", due === "overdue" && "overdue", (due === "soon" || due === "today") && "soon")}>
        <CalendarDays className="ic" />
        {due === "today" ? t("due-today") : day(card.due_date)}
      </span>
    ),
    show("checklist") && card.checklist.total > 0 && (
      <span key="checklist" className="meta" aria-label={t("card-checklist", { done: card.checklist.done, total: card.checklist.total })}>
        <SquareCheck className="ic" />
        {card.checklist.done}/{card.checklist.total}
      </span>
    ),
    show("subtasks") && card.subtasks.total > 0 && (
      <span key="subtasks" className="meta" aria-label={t("card-subtasks", { done: card.subtasks.done, total: card.subtasks.total })}>
        <Workflow className="ic" />
        {card.subtasks.done}/{card.subtasks.total}
      </span>
    ),
    show("estimate") && card.estimate_minutes !== undefined && (
      <span key="estimate" className="meta" aria-label={t("card-estimate", { time: formatMinutes(card.estimate_minutes) })}>
        <Clock className="ic" />
        {formatMinutes(card.estimate_minutes)}
      </span>
    ),
    show("days_in_column") && !closed && days > 0 && (
      <span key="days" className="meta" aria-label={t("card-days-in-column", { count: days })}>
        <Hourglass className="ic" />
        {days}
      </span>
    ),
  ].filter(Boolean);
  const labels = show("labels") ? card.labels : [];

  return (
    <article
      ref={ref}
      className={classes("card", styles.card, selected && "sel", closed && "done", ghost && styles.ghost, lifted && styles.lifted, className)}
      data-card={card.key}
      aria-label={t("card-label", { key: card.key, title: card.title })}
      {...rest}
    >
      <div className={styles.cardTop}>
        <TypeGlyph glyph={card.type.glyph} label={card.type.name} />
        <span className="key">{card.key}</span>
        <div className="grow" />
        <PriorityGlyph priority={card.priority} label={t(`priority-${card.priority}`)} />
      </div>
      <div className={classes("card-title", styles.cardTitle, compact && styles.compactTitle)}>{card.title}</div>
      {labels.length > 0 && (
        <div className={styles.cardLabels}>
          {labels.map((label) => (
            <span key={label.id} className={`chip hue-${label.hue}`}>
              {label.name}
            </span>
          ))}
        </div>
      )}
      {meta.length > 0 && <div className={styles.cardMeta}>{meta}</div>}
    </article>
  );
});
