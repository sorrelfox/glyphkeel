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
import { insertSanitized, tickedItems, withoutTicks } from "./insert";
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

  // Whether the checkboxes take clicks changes without drawing the text
  // again, so links found in it stay the same elements.
  const takesClicks = useRef(toggleable);
  useLayoutEffect(() => {
    takesClicks.current = toggleable;
    for (const box of root.current?.querySelectorAll<HTMLInputElement>("input[data-item]") ?? []) {
      box.disabled = !toggleable;
    }
  }, [toggleable]);
  // The items whose click the engine hasn't answered yet, by number.
  const saving = useRef(new Set<string>());
  // What the reader shows now: the text without its ticks, in which theme, with which links missing.
  const shown = useRef<{ text: string; theme: string; missing: string | null } | null>(null);

  // Insert and draw again when the text or the theme changes. A tick
  // changes the text only in what is ticked: the boxes shown take the new
  // ticks, so the focus, the selection, the scroll position and what was
  // drawn stay as they are.
  useLayoutEffect(() => {
    const element = root.current;
    if (!element) {
      return;
    }
    const text = withoutTicks(html);
    const was = shown.current;
    if (was !== null && was.text === text && was.theme === theme && was.missing === missing) {
      const boxes = element.querySelectorAll<HTMLInputElement>("input[data-item]");
      tickedItems(html).forEach(({ item, checked }, index) => {
        const box = boxes[index];
        if (box?.dataset.item === item && !saving.current.has(item)) {
          box.checked = checked;
        }
      });
      return;
    }
    shown.current = { text, theme, missing };
    drawing.current += 1;
    const generation = drawing.current;
    const place = keepPlace(element);
    insertSanitized(element, html, "reader");
    for (const box of element.querySelectorAll<HTMLInputElement>("input[data-item]")) {
      box.disabled = !takesClicks.current;
    }
    place.restore();
    markMissing(element, missing === null ? null : new Set(missing.split("\n")), latest.current.t);
    void drawReader(element, {
      text: (id, args) => latest.current.t(id, args),
      imageSource: (attachment) => latest.current.imageSource(attachment),
      current: () => drawing.current === generation && element.isConnected,
    });
  }, [html, theme, missing]);

  const toggle = (box: HTMLInputElement, event: MouseEvent) => {
    const item = box.dataset.item ?? "";
    const number = Number(item);
    if (!onToggle || !Number.isInteger(number)) {
      return;
    }
    // One click at a time per item: another before the engine answers is undone.
    if (saving.current.has(item)) {
      event.preventDefault();
      return;
    }
    // The box shows the click at once and goes back if the engine refuses
    // it. It stays enabled meanwhile, as a disabled box would lose the focus.
    const checked = box.checked;
    saving.current.add(item);
    box.setAttribute("aria-busy", "true");
    const current = () => root.current?.querySelector<HTMLInputElement>(`input[data-item="${CSS.escape(item)}"]`) ?? box;
    onToggle({ number, checked }).then(
      () => {
        saving.current.delete(item);
        current().removeAttribute("aria-busy");
      },
      (error: unknown) => {
        saving.current.delete(item);
        const shownBox = current();
        shownBox.removeAttribute("aria-busy");
        shownBox.checked = !checked;
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
        // A name needs a role to carry it: a group, not a landmark, as there may be many.
        role={label === undefined ? undefined : "group"}
        aria-label={label}
        onClick={(event) => {
          const target = event.target as HTMLElement;
          if (target instanceof HTMLInputElement && target.dataset.item !== undefined) {
            toggle(target, event);
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
 * Keeps the reader's place while its text is put in again: the scroll
 * position of every box it scrolls in, and the focus on a checkbox, which
 * goes to the same item's new box.
 */
function keepPlace(element: HTMLElement): { restore: () => void } {
  const scrolled: { box: Element; top: number; left: number }[] = [];
  for (let box = element.parentElement; box; box = box.parentElement) {
    if (box.scrollTop !== 0 || box.scrollLeft !== 0) {
      scrolled.push({ box, top: box.scrollTop, left: box.scrollLeft });
    }
  }
  const focused = document.activeElement;
  const item = focused instanceof HTMLInputElement && element.contains(focused) ? focused.dataset.item : undefined;
  return {
    restore: () => {
      for (const { box, top, left } of scrolled) {
        box.scrollTop = top;
        box.scrollLeft = left;
      }
      if (item !== undefined && !element.contains(document.activeElement)) {
        element.querySelector<HTMLInputElement>(`input[data-item="${CSS.escape(item)}"]`)?.focus({ preventScroll: true });
      }
    },
  };
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
