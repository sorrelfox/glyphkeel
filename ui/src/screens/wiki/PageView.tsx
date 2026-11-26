// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { AppWindow, ChevronRight, GitCompare, History as HistoryIcon, House, Link as LinkIcon, MoreHorizontal, Pencil, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button, Menu, MenuTrigger, Popover } from "react-aria-components";

import { classes } from "../../classes";
import { MenuEntries, type MenuEntry } from "../../components/ContextMenu";
import { Kbd } from "../../components/Kbd";
import { Tooltip } from "../../components/Tooltip";
import { useToasts } from "../../components/Toasts";
import { useDraft, type Draft } from "../../data/drafts";
import { errorToast, useCall, useCommand, useOp } from "../../data/hooks";
import { toggleChecklist } from "../../data/markdown";
import { splitBacklinks } from "../../data/wiki";
import { type Backlink, type Reference, type RevisionInfo, type WikiHeading, type WikiPage } from "../../generated/ops";
import { useT, type Translate } from "../../i18n/I18n";
import { ownerKey } from "../../ipc/files";
import { MarkdownReader } from "../../markdown/Reader";
import { useMomentFormat } from "../task/Activity";
import { DiffView } from "../task/DiffView";
import { FilesSection, useFileDrop } from "../task/Files";
import { MarkdownField } from "../task/MarkdownField";
import { ExportPageDialog } from "./TransferDialogs";
import styles from "./Wiki.module.css";

/** `E`: Edit. */
const EDIT_KEY = { ctrl: false, alt: false, shift: false, meta: false, key: "E" } as const;

/** What the page area shows: the text, its editor, or its history. */
export type PageMode = "read" | "edit" | "history";

export interface PageViewProps {
  pageId: string;
  /** Whether the page may change: its project is active and its wiki switched on. */
  writable: boolean;
  /** The heading to show first, from a link. */
  heading?: string | undefined;
  /** Opens the editor at once, for a page just made. */
  startEditing?: boolean | undefined;
  /** In a window of its own: no "open in its own window". */
  inWindow?: boolean;
  /** Opens another page by its ID, at a heading. */
  onNavigate: (id: string, heading?: string) => void;
  /** A link to a page that doesn't exist yet: offers to make it. */
  onCreateMissing: (title: string) => void;
  onOpenTask: (key: string) => void;
  onRename: (page: WikiPage) => void;
  onDelete: (page: WikiPage) => void;
  onNewChild: (page: WikiPage) => void;
  onOpenWindow?: ((page: WikiPage) => void) | undefined;
}

/** The heading element of `root` whose text is `text`, ignoring case. */
function headingElement(root: HTMLElement | null, text: string): HTMLElement | undefined {
  const wanted = text.trim().toLocaleLowerCase();
  return [...(root?.querySelectorAll<HTMLElement>("h1, h2, h3, h4, h5, h6") ?? [])].find((element) => element.textContent.trim().toLocaleLowerCase() === wanted);
}

/**
 * One wiki page: a bar with its place in the tree, when it changed, History,
 * Edit (`E`) and the page menu; then its text as the reader shows it, with
 * an aside of its outline, what links to it and what it links to. Edit
 * swaps the text for the Markdown editor; unsaved text is kept as a draft.
 * History lists the revisions in the aside's place and shows the
 * differences between two in the page area.
 */
export function PageView(props: PageViewProps) {
  const { pageId, writable, heading, startEditing = false, inWindow = false, onNavigate, onCreateMissing, onOpenTask, onRename, onDelete, onNewChild, onOpenWindow } = props;
  const t = useT();
  const moment = useMomentFormat();
  const found = useOp("wiki.get", { page: pageId, include: ["files"] });
  const references = useOp("wiki.references", { page: pageId });
  const page = found.data;
  const [mode, setMode] = useState<PageMode>(startEditing && writable ? "edit" : "read");
  const kept = useOp("draft.get", { kind: "wiki_page", target: pageId }).data?.draft;
  const [draft, setDraft] = useDraft({ kind: "wiki_page", id: pageId }, kept ? [kept] : undefined);
  const editing = mode === "edit" && writable;
  // While editing, the formatting toolbar sits in the page bar.
  const [toolbarSlot, setToolbarSlot] = useState<HTMLElement | null>(null);
  const [exporting, setExporting] = useState(false);
  const update = useCommand("wiki.update");
  const toasts = useToasts();
  const lookup = useCall();
  // Files dropped on the page are attached to it.
  const owner = { page: pageId };
  const drop = useFileDrop(owner, writable && found.data !== undefined);

  // `E` edits the page from anywhere in the window but a text field, an open
  // dialog or menu, and a task panel beside the page (which has its own).
  useEffect(() => {
    if (mode === "edit" || !writable) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (
        event.defaultPrevented ||
        event.key.toLowerCase() !== "e" ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        target?.closest('input, textarea, select, [contenteditable=""], [contenteditable="true"], [role="textbox"], [role="combobox"], [role="menu"], [role="dialog"], [data-region="panel"]') ||
        document.querySelector('[role="dialog"], [role="alertdialog"]')
      ) {
        return;
      }
      event.preventDefault();
      setMode("edit");
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [mode, writable]);

  if (found.isError) {
    return (
      <section className={styles.page}>
        <p className={classes("faint", styles.notice)} role="alert">
          {found.error.error.message}
        </p>
      </section>
    );
  }
  if (!page) {
    return <section className={styles.page} aria-busy="true" />;
  }

  const openLink = (target: string, at: string | undefined) => {
    // A link to a heading of this page, such as one of {{toc}}'s.
    if (target.trim() === "") {
      onNavigate(page.id, at);
      return;
    }
    const reference = references.data?.items.find((item) => item.kind === "page" && item.target.trim().toLocaleLowerCase() === target.trim().toLocaleLowerCase());
    if (reference?.page) {
      onNavigate(reference.page.id, at);
    } else if (reference && !reference.found) {
      onCreateMissing(target);
    } else if (!reference) {
      // A link a macro wrote ({{children}}, an included page's) isn't among the text's own links: look the page up.
      lookup("wiki.get", { project: page.project, page: target }).then(
        (found) => {
          onNavigate(found.id, at);
        },
        (error: unknown) => {
          toasts.show(errorToast(t, error));
        },
      );
    }
  };

  const menuEntries: MenuEntry[] = [
    ...(inWindow || !onOpenWindow ? [] : [{ id: "window", label: t("wiki-open-window"), run: () => { onOpenWindow(page); } }]),
    { id: "child", label: t("wiki-new-child"), run: () => { onNewChild(page); }, disabled: !writable },
    { id: "rename", label: t("wiki-rename"), run: () => { onRename(page); }, disabled: !writable },
    {
      id: "brief",
      label: t(page.agent_brief ? "wiki-brief-unset" : "wiki-brief-set"),
      disabled: !writable,
      run: () => {
        const brief = !page.agent_brief;
        update.mutate({ page: page.id, agent_brief: brief }, { onSuccess: () => { toasts.show({ tone: "info", text: t(brief ? "wiki-brief-set-done" : "wiki-brief-unset-done") }); } });
      },
    },
    {
      id: "home",
      label: t(page.home ? "wiki-home-unset" : "wiki-home-set"),
      disabled: !writable,
      run: () => {
        const home = !page.home;
        update.mutate({ page: page.id, home }, { onSuccess: () => { toasts.show({ tone: "info", text: t(home ? "wiki-home-set-done" : "wiki-home-unset-done") }); } });
      },
    },
    {
      id: "template",
      label: t(page.template ? "wiki-template-unset" : "wiki-template-set"),
      disabled: !writable,
      run: () => {
        const template = !page.template;
        update.mutate({ page: page.id, template }, { onSuccess: () => { toasts.show({ tone: "info", text: t(template ? "wiki-template-set-done" : "wiki-template-unset-done") }); } });
      },
    },
    "separator",
    { id: "export", label: t("wiki-export"), run: () => { setExporting(true); } },
    "separator",
    { id: "delete", label: t("wiki-delete"), run: () => { onDelete(page); }, disabled: !writable },
  ];

  return (
    <section
      className={classes(styles.page, drop.over && styles.dropping)}
      aria-label={t("wiki-page-label", { page: page.title })}
      data-wiki-page={page.id}
      data-drop-task={ownerKey(owner)}
      {...drop.handlers}
    >
      <div className={styles.pageBar}>
        <nav className={styles.crumbs} aria-label={t("wiki-breadcrumbs")}>
          {(page.path ?? []).map((parent) => (
            <span key={parent.id} className={styles.crumb}>
              <Button className={classes("val muted", styles.crumbButton)} onPress={() => { onNavigate(parent.id); }}>
                {parent.title}
              </Button>
              <ChevronRight className="ic s faint" aria-hidden="true" />
            </span>
          ))}
          <h2 className={styles.pageTitle}>{page.title}</h2>
          {page.home && (
            <span className="chip hue-blue">
              <House className="ic s" aria-hidden="true" />
              {t("wiki-home")}
            </span>
          )}
          {page.agent_brief && <span className="chip hue-purple">{t("wiki-agent-brief")}</span>}
          {page.template && <span className="chip hue-teal">{t("wiki-template")}</span>}
        </nav>
        {editing && <div ref={setToolbarSlot} className={styles.toolbarSlot} data-toolbar-slot="" />}
        <span className="grow" />
        {!editing && (
          <span className={classes("meta", styles.hideNarrow)} data-testid="wiki-changed">
            {t("wiki-changed", { time: moment(page.updated_at) })}
          </span>
        )}
        {!editing && (
          <>
            <Button
              className={classes("btn flat", mode === "history" && "on")}
              aria-pressed={mode === "history"}
              onPress={() => {
                setMode(mode === "history" ? "read" : "history");
              }}
            >
              <HistoryIcon className="ic" />
              <span className="hide-narrow">{t("wiki-history")}</span>
            </Button>
            <Button className="btn" data-edit="" isDisabled={!writable} onPress={() => { setMode("edit"); }}>
              <Pencil className="ic" />
              <span>{t("wiki-edit")}</span>
              <Kbd chord={EDIT_KEY} />
            </Button>
          </>
        )}
        {inWindow || !onOpenWindow ? null : (
          <Button className="btn flat icon" aria-label={t("wiki-open-window")} onPress={() => { onOpenWindow(page); }}>
            <AppWindow className="ic" />
          </Button>
        )}
        <MenuTrigger>
          <Button className="btn flat icon" aria-label={t("wiki-page-menu")}>
            <MoreHorizontal className="ic" />
          </Button>
          <Popover className="pop menuPopover" placement="bottom end" offset={4}>
            <Menu className="menu" aria-label={t("wiki-page-menu")}>
              <MenuEntries entries={menuEntries} />
            </Menu>
          </Popover>
        </MenuTrigger>
      </div>
      {!writable && <div className={classes("banner", styles.banner)} role="status">{t("wiki-read-only-note")}</div>}
      {!editing && draft !== undefined && draft.text !== page.body && (
        <div className={classes("banner", styles.banner)} role="status">
          <span className="grow">{t("wiki-draft-kept")}</span>
          <Button className="btn sm" isDisabled={!writable} onPress={() => { setMode("edit"); }}>
            {t("description-draft-continue")}
          </Button>
          <Button className="btn flat sm" onPress={() => { setDraft(null); }}>
            {t("description-draft-discard")}
          </Button>
        </div>
      )}
      {page.conflict_copies > 0 && !editing && <ConflictBanner page={page} writable={writable} />}
      {editing ? (
        <PageEditor
          page={page}
          kept={draft}
          toolbarSlot={toolbarSlot}
          onDraft={setDraft}
          onDone={() => {
            setMode("read");
          }}
        />
      ) : mode === "history" ? (
        <PageHistory page={page} writable={writable} onClose={() => { setMode("read"); }} />
      ) : (
        <PageReader page={page} writable={writable} heading={heading} references={references.data?.items} onOpenLink={openLink} onNavigate={onNavigate} onOpenTask={onOpenTask} />
      )}
      {drop.over && (
        <div className={styles.dropNote} role="status">
          {t("wiki-files-drop-here", { page: page.title })}
        </div>
      )}
      {exporting && (
        <ExportPageDialog
          page={{ id: page.id, slug: page.slug, title: page.title, project: page.project, hasChildren: (page.children ?? []).length > 0 }}
          onClose={() => {
            setExporting(false);
          }}
        />
      )}
    </section>
  );
}

interface PageReaderProps {
  page: WikiPage;
  /** Whether its checkboxes can be ticked. */
  writable: boolean;
  heading: string | undefined;
  references: readonly Reference[] | undefined;
  onOpenLink: (target: string, heading: string | undefined) => void;
  onNavigate: (id: string, heading?: string) => void;
  onOpenTask: (key: string) => void;
}

/** The page's text as the reader shows it, beside its outline and links. */
function PageReader({ page, writable, heading, references, onOpenLink, onNavigate, onOpenTask }: PageReaderProps) {
  const t = useT();
  const call = useCall();
  // Rendered as this page's text, so that its macros ({{children}}, {{tasks …}}) are expanded.
  const rendered = useOp("markdown.render", { documents: [{ source: page.body, page: page.id, document_id: `wiki-${page.id}` }] });
  const html = rendered.data?.items[0]?.html;
  const checklist = rendered.data?.items[0]?.checklist ?? [];
  const scroller = useRef<HTMLDivElement>(null);
  const [currentHeading, setCurrentHeading] = useState<number | null>(null);
  const headings = page.headings ?? [];

  // A link's heading comes into view once the text is in.
  useEffect(() => {
    if (html !== undefined && heading !== undefined) {
      headingElement(scroller.current, heading)?.scrollIntoView({ block: "start" });
    }
  }, [html, heading]);

  const markCurrent = () => {
    const box = scroller.current;
    if (!box) {
      return;
    }
    const top = box.getBoundingClientRect().top;
    let current: number | null = null;
    headings.forEach((entry, index) => {
      const element = headingElement(box, entry.text);
      if (element && element.getBoundingClientRect().top - top <= 24) {
        current = index;
      }
    });
    setCurrentHeading(current);
  };

  return (
    <div ref={scroller} className={styles.pageScroll} onScroll={markCurrent}>
      <article className={styles.article}>
        {page.body.trim() === "" ? (
          <p className="faint">{t("wiki-empty")}</p>
        ) : html === undefined ? null : (
          <MarkdownReader
            html={html}
            label={t("wiki-text", { page: page.title })}
            references={references}
            onOpenPage={onOpenLink}
            onOpenTask={onOpenTask}
            onToggle={writable ? (click) => toggleChecklist(call, { page: page.id }, checklist, click) : undefined}
          />
        )}
      </article>
      <PageAside
        page={page}
        writable={writable}
        headings={headings}
        currentHeading={currentHeading}
        references={references}
        onHeading={(entry, index) => {
          headingElement(scroller.current, entry.text)?.scrollIntoView({ block: "start" });
          setCurrentHeading(index);
        }}
        onNavigate={onNavigate}
        onOpenTask={onOpenTask}
      />
    </div>
  );
}

interface PageAsideProps {
  page: WikiPage;
  /** Whether files may be attached and removed. */
  writable: boolean;
  headings: readonly WikiHeading[];
  currentHeading: number | null;
  references: readonly Reference[] | undefined;
  onHeading: (heading: WikiHeading, index: number) => void;
  onNavigate: (id: string, heading?: string) => void;
  onOpenTask: (key: string) => void;
}

/** "On this page", "Files", "Linked from", "Tasks that link here" and "Links on this page". */
function PageAside({ page, writable, headings, currentHeading, references, onHeading, onNavigate, onOpenTask }: PageAsideProps) {
  const t = useT();
  const toasts = useToasts();
  const copyText = (text: string) => {
    navigator.clipboard.writeText(text).then(
      () => {
        toasts.show({ tone: "info", text: t("wiki-heading-link-copied"), detail: text });
      },
      () => undefined,
    );
  };
  const backlinks = useOp("wiki.backlinks", { page: page.id });
  const { pages, tasks } = splitBacklinks(backlinks.data?.items ?? []);
  const outgoing = references ?? [];
  return (
    <aside className={styles.aside} aria-label={t("wiki-aside")}>
      {headings.length > 0 && (
        <nav className={styles.asideGroup} aria-label={t("wiki-outline")}>
          <span className={classes("caption", styles.asideCaption)}>{t("wiki-outline")}</span>
          {headings.map((entry, index) => (
            <span key={`${String(entry.line)}-${entry.text}`} className={styles.tocRow}>
              <button
                type="button"
                className={classes(styles.toc, index === currentHeading && styles.tocOn)}
                data-level={entry.level}
                aria-current={index === currentHeading ? "location" : undefined}
                onClick={() => {
                  onHeading(entry, index);
                }}
              >
                {entry.text}
              </button>
              <Tooltip text={t("wiki-copy-heading-link", { heading: entry.text })}>
                <Button
                  className={classes("btn flat icon sm", styles.tocCopy)}
                  aria-label={t("wiki-copy-heading-link", { heading: entry.text })}
                  onPress={() => {
                    copyText(`[[${page.title}#${entry.text}]]`);
                  }}
                >
                  <LinkIcon className="ic s" aria-hidden="true" />
                </Button>
              </Tooltip>
            </span>
          ))}
        </nav>
      )}
      <div className={styles.asideFiles} data-page-files="">
        <FilesSection task={{ page: page.id }} files={page.files ?? []} readOnly={!writable} />
      </div>
      <section className={styles.asideGroup} aria-label={t("wiki-linked-from")}>
        <span className={classes("caption", styles.asideCaption)}>{t("wiki-linked-from")}</span>
        {backlinks.data && pages.length === 0 && <span className={classes("faint", styles.small)}>{t("wiki-no-backlinks")}</span>}
        {pages.map((link) => (
          <BacklinkRow key={link.page?.id} link={link} onNavigate={onNavigate} onOpenTask={onOpenTask} />
        ))}
      </section>
      <section className={styles.asideGroup} aria-label={t("wiki-tasks-linking")}>
        <span className={classes("caption", styles.asideCaption)}>{t("wiki-tasks-linking")}</span>
        {backlinks.data && tasks.length === 0 && <span className={classes("faint", styles.small)}>{t("wiki-no-task-backlinks")}</span>}
        {tasks.map((link) => (
          <BacklinkRow key={link.task} link={link} onNavigate={onNavigate} onOpenTask={onOpenTask} />
        ))}
      </section>
      {outgoing.length > 0 && (
        <section className={styles.asideGroup} aria-label={t("wiki-references")}>
          <span className={classes("caption", styles.asideCaption)}>{t("wiki-references")}</span>
          {outgoing.map((reference, index) => (
            <ReferenceRow key={`${String(index)}-${reference.target}`} reference={reference} onNavigate={onNavigate} onOpenTask={onOpenTask} />
          ))}
        </section>
      )}
    </aside>
  );
}

function BacklinkRow({ link, onNavigate, onOpenTask }: { link: Backlink; onNavigate: (id: string, heading?: string) => void; onOpenTask: (key: string) => void }) {
  const t = useT();
  if (link.page) {
    const target = link.page;
    return (
      <button type="button" className={classes("linkbtn", styles.asideLink)} data-backlink={target.id} onClick={() => { onNavigate(target.id); }}>
        {link.title}
      </button>
    );
  }
  const key = link.task ?? "";
  return (
    <button type="button" className={styles.asideTask} data-backlink={key} aria-label={t("wiki-open-task", { key, title: link.title })} onClick={() => { onOpenTask(key); }}>
      <span className="key">{key}</span>
      <span className="muted truncate">{link.title}</span>
    </button>
  );
}

function ReferenceRow({ reference, onNavigate, onOpenTask }: { reference: Reference; onNavigate: (id: string, heading?: string) => void; onOpenTask: (key: string) => void }) {
  const t = useT();
  const shown = reference.heading === undefined ? reference.target : t("wiki-link-heading", { page: reference.target, heading: reference.heading });
  if (!reference.found) {
    return (
      <span className={classes(styles.asideMissing, styles.small)} data-reference={reference.target} data-missing="">
        <span className="truncate">{shown}</span>
        <span className="chip hue-red">{t("wiki-missing")}</span>
      </span>
    );
  }
  if (reference.kind === "task" && reference.task !== undefined) {
    const key = reference.task;
    return (
      <button type="button" className={styles.asideTask} data-reference={key} aria-label={t("wiki-open-task", { key, title: reference.title ?? "" })} onClick={() => { onOpenTask(key); }}>
        <span className="key">{key}</span>
        <span className="muted truncate">{reference.title}</span>
      </button>
    );
  }
  const target = reference.page;
  return (
    <button
      type="button"
      className={classes("linkbtn", styles.asideLink)}
      data-reference={reference.target}
      onClick={() => {
        if (target) {
          onNavigate(target.id, reference.heading);
        }
      }}
    >
      {reference.heading === undefined ? (reference.title ?? reference.target) : t("wiki-link-heading", { page: reference.title ?? reference.target, heading: reference.heading })}
    </button>
  );
}

/** Saves a page's text and tells what became of it. */
function useSavePage(page: WikiPage) {
  const t = useT();
  const toasts = useToasts();
  const update = useCommand("wiki.update");
  return {
    pending: update.isPending,
    save: (draft: Draft, message: string, done: () => void) => {
      const resolving = draft.resolving;
      const note = message.trim() === "" ? {} : { message: message.trim() };
      update.mutate(
        resolving === undefined
          ? { page: page.id, body: draft.text, base_revision: draft.base ?? null, ...note }
          : { page: page.id, resolve_conflict: { action: "use_text", copy: resolving, text: draft.text }, base_revision: draft.base ?? null, ...note },
        {
          onSuccess: (change) => {
            const outcome = change.save?.outcome;
            if (outcome === "merged") {
              toasts.show({ tone: "info", text: t("wiki-merged") });
            } else if (outcome === "conflict") {
              toasts.show({ tone: "error", text: t("wiki-conflict-saved"), detail: t("wiki-conflict-detail") });
            }
            done();
          },
        },
      );
    },
  };
}

/**
 * The page's editor, with the formatting toolbar in the page bar (in
 * `toolbarSlot`). Without a kept draft it
 * starts from the text and revision as they are when it opens, so a save
 * names the revision its text came from: an edit saved meanwhile elsewhere
 * is merged, or kept as a conflict copy.
 */
function PageEditor({ page, kept, toolbarSlot, onDraft, onDone }: { page: WikiPage; kept: Draft | undefined; toolbarSlot: HTMLElement | null; onDraft: (draft: Draft | null) => void; onDone: () => void }) {
  const t = useT();
  const [source, setSource] = useState(false);
  const [confirming, setConfirming] = useState(false);
  // A note saved with the revision, such as why the page changed.
  const [message, setMessage] = useState("");
  const [opened] = useState<Draft>(() => ({ text: page.body, base: page.revision }));
  const draft = kept ?? opened;
  const { save, pending } = useSavePage(page);
  const unchanged = draft.text === page.body || (draft.base === opened.base && draft.text === opened.text);
  const changed = draft.resolving !== undefined || !unchanged;
  const submit = () => {
    if (pending) {
      return;
    }
    if (!changed) {
      onDraft(null);
      onDone();
      return;
    }
    save(draft, message, () => {
      onDraft(null);
      onDone();
    });
  };
  const cancel = () => {
    if (changed && !confirming) {
      setConfirming(true);
      return;
    }
    onDraft(null);
    onDone();
  };
  return (
    <div className={styles.editorPane}>
      <div className={styles.editorColumn}>
        <MarkdownField
          label={t("wiki-text", { page: page.title })}
          value={draft.text}
          mode={source ? "source" : "preview"}
          autoFocus
          project={page.project}
          attachTo={{ page: page.id }}
          toolbarSlot={toolbarSlot}
          className={styles.editor}
          onChange={(text) => {
            setConfirming(false);
            onDraft({ ...draft, text });
          }}
          onSubmit={submit}
          onCancel={cancel}
        />
        {confirming ? (
          <div className={classes("banner", styles.editorBar)} role="alertdialog" aria-label={t("description-discard-question")}>
            <span className="grow">{t("description-discard-question")}</span>
            <Button className="btn sm" autoFocus onPress={() => { setConfirming(false); }}>
              {t("description-keep-editing")}
            </Button>
            <Button className="btn sm danger" onPress={cancel}>
              {t("description-discard")}
            </Button>
          </div>
        ) : (
          <div className={styles.editorBar}>
            <Button className={classes("btn flat sm", source && "on")} aria-pressed={source} onPress={() => { setSource((on) => !on); }}>
              {t("description-source")}
            </Button>
            <span className={classes("faint grow", styles.small)}>{t("wiki-draft-note")}</span>
            <span className={classes("field", styles.messageField)}>
              <input
                value={message}
                maxLength={500}
                aria-label={t("wiki-save-message")}
                placeholder={t("wiki-save-message-hint")}
                onChange={(event) => {
                  setMessage(event.target.value);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    submit();
                  }
                }}
              />
            </span>
            <Button className="btn sm" onPress={cancel}>
              {t("wiki-discard")}
            </Button>
            <Button className="btn primary sm" isDisabled={pending} onPress={submit}>
              {t("panel-save")}
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}

/** Who saved a revision, how it came about, and when. */
function revisionLine(t: Translate, moment: (at: string) => string, revision: RevisionInfo): string {
  const who = revision.author.kind === "user" ? t("comment-me") : revision.author.name;
  return t("description-revision-line", { kind: t(`description-kind-${revision.kind}`), actor: who, time: moment(revision.created_at) });
}

/**
 * The page's revisions, newest first, in the aside's place; the differences
 * between the two chosen (or one and the one before it) in the page area.
 * Restoring an older revision saves its text as a new revision.
 */
function PageHistory({ page, writable, onClose }: { page: WikiPage; writable: boolean; onClose: () => void }) {
  const t = useT();
  const moment = useMomentFormat();
  const toasts = useToasts();
  const update = useCommand("wiki.update");
  const history = useOp("wiki.history", { page: page.id });
  const items = history.data?.items ?? [];
  const [chosen, setChosen] = useState<readonly string[]>([]);
  const picked = items.filter((revision) => chosen.includes(revision.id));
  // Without a choice, the latest change: the current revision against the one before.
  const newer = picked[0] ?? items.find((revision) => revision.current);
  const older = picked[1] ?? (newer ? items.slice(items.indexOf(newer) + 1).find((revision) => revision.kind !== "conflict") : undefined);
  const detail = useOp("description.revision", { revision: newer?.id ?? "", ...(older ? { diff_from: older.id } : {}) }, { enabled: newer !== undefined });
  const current = items.find((revision) => revision.current);
  const choose = (id: string) => {
    setChosen((was) => (was.includes(id) ? was.filter((other) => other !== id) : [...was, id].slice(-2)));
  };
  return (
    <div className={styles.historyLayout}>
      <div className={styles.historyDiff}>
        {newer && (
          <>
            <div className={styles.historyHead}>
              <GitCompare className="ic s muted" />
              <span className={classes("grow", styles.small)}>
                {older ? t("description-comparing", { older: moment(older.created_at), newer: moment(newer.created_at) }) : t("description-first", { time: moment(newer.created_at) })}
              </span>
              {current && newer.id !== current.id && newer.kind !== "conflict" && writable && detail.data && (
                <Button
                  className="btn sm"
                  isDisabled={update.isPending}
                  onPress={() => {
                    update.mutate(
                      { page: page.id, body: detail.data.body, base_revision: current.id },
                      {
                        onSuccess: () => {
                          toasts.show({ tone: "info", text: t("wiki-restored") });
                          onClose();
                        },
                      },
                    );
                  }}
                >
                  {t("wiki-restore")}
                </Button>
              )}
            </div>
            {detail.data && (older ? <DiffView diff={detail.data.diff ?? ""} label={t("description-diff")} /> : <pre className={styles.revisionBody}>{detail.data.body}</pre>)}
          </>
        )}
        {history.data && items.length === 0 && <p className={classes("faint", styles.small)}>{t("description-history-none")}</p>}
      </div>
      <aside className={styles.historyList} aria-label={t("wiki-history")}>
        <div className={styles.historyHead}>
          <span className={classes("caption grow", styles.asideCaption)}>{t("wiki-history")}</span>
          <Button className="btn flat icon sm" aria-label={t("description-close")} onPress={onClose}>
            <X className="ic s" />
          </Button>
        </div>
        <span className={classes("faint", styles.small)}>{t("description-history-help")}</span>
        {history.isError && <p className="faint">{history.error.error.message}</p>}
        <ol className={styles.revisionList} aria-label={t("wiki-revisions")}>
          {items.map((revision) => (
            <li key={revision.id} className={classes(styles.revisionRow, chosen.includes(revision.id) && styles.chosen)}>
              <label className={styles.revisionPick}>
                <input type="checkbox" className="check" checked={chosen.includes(revision.id)} onChange={() => { choose(revision.id); }} />
                <span className="grow">
                  {revisionLine(t, moment, revision)}
                  {revision.message !== undefined && <span className={classes("muted", styles.revisionMessage)}>{revision.message}</span>}
                </span>
                {revision.current && <span className="chip hue-green">{t("description-current")}</span>}
              </label>
            </li>
          ))}
        </ol>
      </aside>
    </div>
  );
}

/** Edits that clashed with another and wait: keep the page, take the copy. */
function ConflictBanner({ page, writable }: { page: WikiPage; writable: boolean }) {
  const t = useT();
  const toasts = useToasts();
  const history = useOp("wiki.history", { page: page.id });
  const copy = (history.data?.items ?? []).filter((revision) => revision.kind === "conflict" && revision.resolved_at === undefined).at(-1);
  const update = useCommand("wiki.update");
  const resolve = (action: "keep_current" | "use_copy") => {
    if (copy) {
      update.mutate({ page: page.id, resolve_conflict: { action, copy: copy.id } }, { onSuccess: () => { toasts.show({ tone: "info", text: t("description-resolved") }); } });
    }
  };
  return (
    <div className={classes("banner", styles.banner, styles.conflictBanner)} role="alert">
      <span className="grow">{t("wiki-conflict", { count: page.conflict_copies })}</span>
      <Button className="btn sm" isDisabled={!writable || !copy || update.isPending} onPress={() => { resolve("keep_current"); }}>
        {t("description-keep-current")}
      </Button>
      <Button className="btn primary sm" isDisabled={!writable || !copy || update.isPending} onPress={() => { resolve("use_copy"); }}>
        {t("description-use-copy")}
      </Button>
    </div>
  );
}
