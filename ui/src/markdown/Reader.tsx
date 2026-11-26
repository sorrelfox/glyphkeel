// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { lazy, Suspense, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";

import { classes } from "../classes";
import { useAppearance } from "../appearance/AppearanceProvider";
import { useToasts } from "../components/Toasts";
import { requestComment } from "../data/commentLinks";
import { useImageSource, type CheckboxClick } from "../data/markdown";
import { referenceIndex, splitTarget } from "../data/wiki";
import { type Reference } from "../generated/ops";
import { errorToast, useOp } from "../data/hooks";
import { useT, type Translate } from "../i18n/I18n";
import { openAttachment } from "../ipc/files";
import { openLink } from "../ipc/links";
import { attachmentOf, drawReader } from "./draw";
import { insertSanitized } from "./insert";
import styles from "./Reader.module.css";

// A wiki link opens its page read only; loaded on the first such click.
const WikiPageDialog = lazy(() => import("../screens/search/WikiPageDialog"));

interface MarkdownReaderProps {
  /** The engine's sanitized HTML. */
  html: string;
  /** Tick or clear an item; the promise settles when the engine has answered. Without it, checkboxes are read-only. */
  onToggle?: ((click: CheckboxClick) => Promise<unknown>) | undefined;
  onOpenTask: (key: string) => void;
  /** Where the text's links point now (`wiki.references`, `task.references`): links to pages that don't exist are marked. */
  references?: readonly Reference[] | undefined;
  /** Opens the page a wiki link names; without it, the page opens read only in a dialog. */
  onOpenPage?: ((target: string, heading: string | undefined) => void) | undefined;
  /** The smaller text of comments. */
  compact?: boolean;
  className?: string | undefined;
  label?: string | undefined;
}

/**
 * Markdown as the reader shows it: the engine's HTML, inserted through the
 * one sanitizing function, with formulas, diagrams and code drawn on top.
 * Checkboxes tick through the engine, task keys open the task (and show a
 * card on hover; a link to a comment scrolls to it), wiki links open their page read only, other links open in the
 * browser, and external images wait as buttons.
 */
export function MarkdownReader({ html, onToggle, onOpenTask, references, onOpenPage, compact = false, className, label }: MarkdownReaderProps) {
  const t = useT();
  const toasts = useToasts();
  const root = useRef<HTMLDivElement>(null);
  const drawing = useRef(0);
  const imageSource = useImageSource();
  const { resolved } = useAppearance();
  const theme = `${resolved.theme} ${resolved.accent}`;
  const [card, setCard] = useState<{ key: string; top: number; left: number } | null>(null);
  const [wiki, setWiki] = useState<string | null>(null);
  const latest = useRef({ imageSource, t });
  const toggleable = onToggle !== undefined;
  // Which page links find nothing now, as one string, so that a new answer with the same result draws nothing again.
  const index = referenceIndex(references);
  const missing = references === undefined ? null : [...index.values()].filter((reference) => !reference.found).map((reference) => reference.target.trim().toLocaleLowerCase()).sort().join("\n");

  useEffect(() => {
    latest.current = { imageSource, t };
  });

  // Insert and draw again when the text or the theme changes.
  useLayoutEffect(() => {
    const element = root.current;
    if (!element) {
      return;
    }
    drawing.current += 1;
    const generation = drawing.current;
    insertSanitized(element, html, "reader");
    for (const box of element.querySelectorAll<HTMLInputElement>("input[data-item]")) {
      box.disabled = !toggleable;
    }
    markMissing(element, missing === null ? null : new Set(missing.split("\n")), latest.current.t);
    void drawReader(element, {
      text: (id, args) => latest.current.t(id, args),
      imageSource: (attachment) => latest.current.imageSource(attachment),
      current: () => drawing.current === generation && element.isConnected,
    });
  }, [html, theme, toggleable, missing]);

  const toggle = (box: HTMLInputElement) => {
    const number = Number(box.dataset.item);
    if (!onToggle || !Number.isInteger(number)) {
      return;
    }
    // The box shows the click at once and goes back if the engine refuses it.
    const checked = box.checked;
    box.disabled = true;
    onToggle({ number, checked }).then(
      () => {
        box.disabled = false;
      },
      (error: unknown) => {
        box.checked = !checked;
        box.disabled = false;
        toasts.show(errorToast(t, error));
      },
    );
  };

  const activate = (target: HTMLElement, event: MouseEvent | KeyboardEvent): void => {
    const link = target.closest("a");
    if (link) {
      event.preventDefault();
      const ref = link.dataset.ref;
      const href = link.getAttribute("href") ?? "";
      if (ref !== undefined) {
        // A link to a comment opens its task at the comment.
        if (link.dataset.comment !== undefined) {
          requestComment(ref, link.dataset.comment);
        }
        onOpenTask(ref);
      } else if (link.dataset.wiki !== undefined) {
        const { title, heading } = splitTarget(link.dataset.wiki);
        if (onOpenPage) {
          onOpenPage(title, heading);
        } else {
          setWiki(link.dataset.wiki);
        }
      } else if (attachmentOf(href) !== null) {
        // A link to an attached file opens it in the desktop's app for it.
        openAttachment(attachmentOf(href) ?? "").catch((error: unknown) => {
          toasts.show(errorToast(t, error));
        });
      } else if (href.startsWith("#")) {
        root.current?.querySelector(`[id="${CSS.escape(href.slice(1))}"]`)?.scrollIntoView({ block: "nearest" });
      } else {
        openLink(href).catch((error: unknown) => {
          toasts.show(errorToast(t, error));
        });
      }
      return;
    }
    const external = target.closest<HTMLElement>("[data-external]");
    if (external) {
      openLink(external.dataset.external ?? "").catch((error: unknown) => {
        toasts.show(errorToast(t, error));
      });
      return;
    }
    const copy = target.closest<HTMLElement>("[data-copy]");
    if (copy) {
      const code = copy.closest<HTMLElement>(".gk-code")?.dataset.code ?? "";
      navigator.clipboard.writeText(code).then(
        () => {
          toasts.show({ tone: "info", text: t("reader-copied") });
        },
        () => undefined,
      );
    }
  };

  const showCard = (target: HTMLElement) => {
    const link = target.closest<HTMLAnchorElement>("a[data-ref]");
    const box = root.current?.parentElement;
    if (!link?.dataset.ref || !box) {
      return;
    }
    const at = link.getBoundingClientRect();
    const frame = box.getBoundingClientRect();
    setCard({ key: link.dataset.ref, top: at.bottom - frame.top + 4, left: Math.max(0, at.left - frame.left) });
  };

  return (
    <div className={styles.frame}>
      <div
        ref={root}
        className={classes("md selectable", styles.reader, compact && styles.compact, className)}
        aria-label={label}
        onClick={(event) => {
          const target = event.target as HTMLElement;
          if (target instanceof HTMLInputElement && target.dataset.item !== undefined) {
            toggle(target);
            return;
          }
          activate(target, event);
        }}
        onKeyDown={(event) => {
          const target = event.target as HTMLElement;
          if (event.key === "Enter" && target.matches("a[data-ref], a[data-wiki]")) {
            activate(target, event);
          }
        }}
        onMouseOver={(event) => {
          showCard(event.target as HTMLElement);
        }}
        onFocus={(event) => {
          showCard(event.target);
        }}
        onMouseOut={(event) => {
          if ((event.target as HTMLElement).closest("a[data-ref]")) {
            setCard(null);
          }
        }}
        onBlur={() => {
          setCard(null);
        }}
      />
      {card && <TaskCard taskKey={card.key} top={card.top} left={card.left} />}
      {wiki !== null && (
        <Suspense>
          <WikiPageDialog
            page={{ title: splitTarget(wiki).title }}
            heading={splitTarget(wiki).heading}
            onClose={() => {
              setWiki(null);
            }}
            onOpenTask={onOpenTask}
          />
        </Suspense>
      )}
    </div>
  );
}

/**
 * Marks the wiki links whose page doesn't exist (now): `missing` holds their
 * targets, folded; null when that isn't known, which marks nothing.
 */
function markMissing(element: HTMLElement, missing: ReadonlySet<string> | null, t: Translate): void {
  for (const link of element.querySelectorAll<HTMLAnchorElement>("a[data-wiki]")) {
    const { title } = splitTarget(link.dataset.wiki ?? "");
    if (missing?.has(title.toLocaleLowerCase()) === true) {
      link.dataset.missing = "";
      link.setAttribute("aria-description", t("reader-wiki-missing", { page: title }));
    } else {
      delete link.dataset.missing;
      link.removeAttribute("aria-description");
    }
  }
}

/** The card a task key shows on hover or focus: its title and status. */
function TaskCard({ taskKey, top, left }: { taskKey: string; top: number; left: number }) {
  const task = useOp("task.get", { keys: [taskKey] });
  const found = task.data?.items[0];
  if (!found) {
    return null;
  }
  return (
    // The position comes from getBoundingClientRect, which is physical in
    // every writing direction.
    // eslint-disable-next-line no-restricted-syntax
    <div className={classes("pop", styles.card)} role="tooltip" style={{ top, left }}>
      <span className="key">{found.key}</span>
      <span className={styles.cardTitle}>{found.title}</span>
      <span className="faint">{found.status.name}</span>
    </div>
  );
}
