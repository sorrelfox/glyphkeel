// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Bot, CalendarClock, CalendarDays, Check, Clock, Hourglass, MessageSquare, Paperclip, SquareCheck, Timer, Workflow } from "lucide-react";
import { forwardRef, Fragment, useEffect, useRef, useState, type CSSProperties, type HTMLAttributes } from "react";
import { useLocale } from "react-aria-components";

import { classes } from "../../classes";
import { PriorityGlyph, RecurringMark, TypeGlyph } from "../../components/Glyphs";
import { type Card, type CardField } from "../../data/catalog";
import { useFieldText } from "../../data/fields";
import { useOp } from "../../data/hooks";
import { inApp, thumbnailUrl } from "../../ipc/files";
import { ProjectMark } from "../../components/ProjectMark";
import { type CardCover, type CustomField, type Hue } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import { useDurations } from "../../data/time";
import { dueState } from "./model";
import styles from "./Board.module.css";

interface CardViewProps extends HTMLAttributes<HTMLElement> {
  card: Card;
  fields: readonly string[];
  /** The custom fields of the board's project, for the `cf.` entries of `fields`. */
  custom?: readonly CustomField[];
  compact: boolean;
  today: string;
  selected?: boolean;
  /** The slot a dragged card leaves behind. */
  ghost?: boolean;
  /** The card that follows the pointer. */
  lifted?: boolean;
  /** On a board across projects: the card's project, shown by its colour and name. */
  project?: { name: string; hue: Hue } | undefined;
  style?: CSSProperties;
}

/** A due date as the card shows it: "Nov 24". Days carry no time zone. */
export function useDayFormat(): (day: string) => string {
  const { locale } = useLocale();
  const format = new Intl.DateTimeFormat(locale, { month: "short", day: "numeric", timeZone: "UTC" });
  return (day) => format.format(new Date(`${day}T00:00:00Z`));
}

/** What a card's coloured edge stands for, in words: the edge is never the only sign of it. */
function useEdgeText(): (card: Card) => string | undefined {
  const t = useT();
  return ({ edge }) => {
    if (edge === undefined) {
      return undefined;
    }
    switch (edge.by) {
      case "type":
        return t("card-edge-type", { name: edge.name });
      case "priority":
        return t("card-edge-priority", { priority: t(`priority-${edge.name}`) });
      case "label":
        return t("card-edge-label", { name: edge.name });
      default:
        return t("card-edge-rule", { name: edge.name });
    }
  };
}

/** Whether `element` has come near the screen; it stays true once it has. */
function useSeen<T extends Element>(): [React.RefObject<T | null>, boolean] {
  const ref = useRef<T>(null);
  // Without an observer to ask (a test page, an old browser), it counts as seen.
  const [seen, setSeen] = useState(() => typeof IntersectionObserver === "undefined");
  useEffect(() => {
    const element = ref.current;
    if (seen || element === null) {
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setSeen(true);
        }
      },
      { rootMargin: "200px" },
    );
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, [seen]);
  return [ref, seen];
}

/**
 * A card's cover: the image at a fixed shape, cropped to fill it. In the app
 * it is the engine's thumbnail from `gk-file:`, loaded when the card comes
 * near the screen; in a browser, which can't reach the app's files, the
 * image's content once the card is seen. It is named by its file name.
 */
function Cover({ cover }: { cover: CardCover }) {
  const app = inApp();
  const [ref, seen] = useSeen<HTMLDivElement>();
  const [broken, setBroken] = useState(false);
  const content = useOp("file.read", { attachment: cover.attachment }, { enabled: !app && seen });
  if (broken) {
    return null;
  }
  const data = content.data?.data_base64;
  const source = app ? thumbnailUrl(cover.attachment) : data === undefined ? undefined : `data:${cover.mime};base64,${data}`;
  return (
    <div ref={ref} className={styles.cover} data-cover={cover.name}>
      {source !== undefined && (seen || app) && (
        <img
          src={source}
          alt={cover.name}
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => {
            setBroken(true);
          }}
        />
      )}
    </div>
  );
}

/**
 * A task on the board. Its cover image first, when the layout shows covers;
 * its coloured edge, named in words too. Rows, each left out when empty: type, key, the
 * recurring mark, Blocked, an agent's claim and priority; the title; labels; start and due dates,
 * checklist, subtasks, estimate and days in the column, as the personal
 * card fields choose, and the custom fields they name, each as its name and
 * value; then comments, files and time spent, when there are any.
 */
export const CardView = forwardRef<HTMLElement, CardViewProps>(function CardView(
  { card, fields, custom = [], compact, today, selected = false, ghost = false, lifted = false, project, className, ...rest },
  ref,
) {
  const t = useT();
  const day = useDayFormat();
  const { format: duration } = useDurations();
  const fieldText = useFieldText();
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
      <span key="estimate" className="meta" aria-label={t("card-estimate", { time: duration(card.estimate_minutes) })}>
        <Clock className="ic" />
        {duration(card.estimate_minutes)}
      </span>
    ),
    show("days_in_column") && !closed && days > 0 && (
      <span key="days" className="meta" aria-label={t("card-days-in-column", { count: days })}>
        <Hourglass className="ic" />
        {days}
      </span>
    ),
    ...(compact
      ? []
      : fields
          .filter((name) => name.startsWith("cf."))
          .map((name) => {
            const field = custom.find((f) => f.query_name === name);
            const value = card.fields?.[name.slice(3)];
            if (field === undefined || value === undefined || value === null) {
              return false;
            }
            const text = fieldText(field.kind, value);
            return (
              <span key={name} className={classes("meta", styles.cardField)} aria-label={t("card-custom-field", { field: field.name, value: text })}>
                <span className={styles.cardFieldName} aria-hidden="true">
                  {field.name}
                </span>
                {field.kind === "boolean" && value === true ? <Check className="ic" aria-hidden="true" /> : <span aria-hidden="true">{text}</span>}
              </span>
            );
          })),
    // The engine's card fields don't list these: they show whenever there are any.
    !compact && card.comment_count > 0 && (
      <span key="comments" className="meta" aria-label={t("card-comments", { count: card.comment_count })}>
        <MessageSquare className="ic" />
        {card.comment_count}
      </span>
    ),
    !compact && card.file_count > 0 && (
      <span key="files" className="meta" aria-label={t("card-files", { count: card.file_count })}>
        <Paperclip className="ic" />
        {card.file_count}
      </span>
    ),
    !compact && card.spent_minutes > 0 && (
      <span key="spent" className="meta" aria-label={t("card-spent", { time: duration(card.spent_minutes) })}>
        <Timer className="ic" />
        {duration(card.spent_minutes)}
      </span>
    ),
  ].filter(Boolean);
  const labels = show("labels") ? card.labels : [];
  const edgeText = useEdgeText()(card);

  return (
    <article
      ref={ref}
      className={classes("card", styles.card, selected && "sel", closed && "done", ghost && styles.ghost, lifted && styles.lifted, card.edge && styles.edged, className)}
      data-card={card.key}
      {...(card.edge ? { "data-edge": card.edge.hue } : {})}
      {...rest}
    >
      {card.edge && (
        <span className={classes(styles.edge, `hue-${card.edge.hue}`)} title={edgeText} aria-hidden="true" />
      )}
      {!compact && card.cover && <Cover key={card.cover.attachment} cover={card.cover} />}
      {/* Named by its text, all of it, so that the name holds what is on the
          card; the spaces keep the parts apart in the name. */}
      <div className={styles.cardTop}>
        <TypeGlyph glyph={card.type.glyph} label={card.type.name} />{" "}
        {project && <ProjectMark hue={project.hue} label={t("card-project", { name: project.name })} />}{" "}
        <span className="key">{card.key}</span>{" "}
        {card.recurring === true && <RecurringMark />}{" "}
        {card.blocked && !closed && <span className="chip hue-red">{t("card-blocked")}</span>}{" "}
        <div className="grow" />
        {card.claim && (
          <span className="meta" role="img" aria-label={t("card-claimed-label", { agent: card.claim.agent })}>
            <Bot className="ic" />
          </span>
        )}
        <PriorityGlyph priority={card.priority} label={t(`priority-${card.priority}`)} />
      </div>{" "}
      {edgeText !== undefined && <span className={styles.hiddenText}>{edgeText} </span>}
      <div className={classes("card-title", styles.cardTitle, compact && styles.compactTitle)}>{card.title}</div>{" "}
      {card.status && (
        <div className={styles.cardLabels}>
          <span className={`chip hue-${card.status.hue}`} data-status={card.status.name}>
            <span className={styles.hiddenText}>{t("card-status")} </span>
            {card.status.name}
          </span>
        </div>
      )}{" "}
      {labels.length > 0 && (
        <div className={styles.cardLabels}>
          {labels.map((label) => (
            <Fragment key={label.id}>
              <span className={`chip hue-${label.hue}`}>{label.name}</span>{" "}
            </Fragment>
          ))}
        </div>
      )}{" "}
      {meta.length > 0 && (
        <div className={styles.cardMeta}>
          {meta.map((item, at) => (
            <Fragment key={at}>
              {item}{" "}
            </Fragment>
          ))}
        </div>
      )}
    </article>
  );
});
