// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { CalendarDays, Clock, MessageSquare, Paperclip, SquareCheck, Workflow } from "lucide-react";
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
  fields: readonly CardField[];
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
 * A task on the board. Rows, each left out when empty: type, key, Blocked
 * and priority; the title; labels; due date, checklist, subtasks, files,
 * comments and time spent, as the personal card fields choose.
 */
export const CardView = forwardRef<HTMLElement, CardViewProps>(function CardView(
  { card, fields, compact, today, selected = false, ghost = false, lifted = false, className, ...rest },
  ref,
) {
  const t = useT();
  const day = useDayFormat();
  const show = (field: CardField) => !compact && fields.includes(field);
  const closed = card.closed_at !== null;
  const due = dueState(card.due_date, closed, today);
  const meta = [
    show("due") && card.due_date !== null && (
      <span key="due" className={classes("meta", due === "overdue" && "overdue", (due === "soon" || due === "today") && "soon")}>
        <CalendarDays className="ic" />
        {due === "today" ? t("due-today") : day(card.due_date)}
      </span>
    ),
    show("checklist") && card.checklist && (
      <span key="checklist" className="meta" aria-label={t("card-checklist", { done: card.checklist.done, total: card.checklist.total })}>
        <SquareCheck className="ic" />
        {card.checklist.done}/{card.checklist.total}
      </span>
    ),
    show("subtasks") && card.subtasks && (
      <span key="subtasks" className="meta" aria-label={t("card-subtasks", { done: card.subtasks.done, total: card.subtasks.total })}>
        <Workflow className="ic" />
        {card.subtasks.done}/{card.subtasks.total}
      </span>
    ),
    show("files") && card.files > 0 && (
      <span key="files" className="meta" aria-label={t("card-files", { count: card.files })}>
        <Paperclip className="ic" />
        {card.files}
      </span>
    ),
    show("comments") && card.comments > 0 && (
      <span key="comments" className="meta" aria-label={t("card-comments", { count: card.comments })}>
        <MessageSquare className="ic" />
        {card.comments}
      </span>
    ),
    show("spent") && card.spent_min > 0 && (
      <span key="spent" className="meta" aria-label={t("card-spent", { time: formatMinutes(card.spent_min) })}>
        <Clock className="ic" />
        {formatMinutes(card.spent_min)}
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
        <TypeGlyph glyph={card.type} label={card.type_name} />
        <span className="key">{card.key}</span>
        <div className="grow" />
        {card.blocked && <span className="chip hue-red">{t("card-blocked")}</span>}
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
