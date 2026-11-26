// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { ChevronDown, GripVertical, Plus } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button, Tree, TreeItem, TreeItemContent, useDragAndDrop, type Key } from "react-aria-components";

import { classes } from "../../classes";
import { isMenuKey, useContextMenu, type MenuEntry } from "../../components/ContextMenu";
import { Tooltip } from "../../components/Tooltip";
import { buildTree, keyboardMove, moveFor, type KeyboardMove, type WikiNode } from "../../data/wiki";
import { type WikiMoveInput, type WikiPageSummary } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Wiki.module.css";

/** The data a dragged page carries: its ID. */
const PAGE_TYPE = "application/x-glyphkeel-wiki-page";

/** The keys that move the focused page: Alt with an arrow. */
const MOVE_KEYS: Readonly<Record<string, KeyboardMove>> = { ArrowUp: "up", ArrowDown: "down", ArrowRight: "in", ArrowLeft: "out" };

interface PageTreeProps {
  pages: readonly WikiPageSummary[];
  /** The page shown. */
  current: string | undefined;
  /** Whether pages may be added, moved, renamed and deleted (the project is active and its wiki on). */
  writable: boolean;
  onOpen: (id: string) => void;
  onMove: (move: WikiMoveInput) => void;
  /** A new page, inside `parent` or at the top. */
  onNew: (parent: string | null) => void;
  onRename: (page: WikiPageSummary) => void;
  onDelete: (page: WikiPageSummary) => void;
  onOpenWindow: (page: WikiPageSummary) => void;
}

/**
 * The project's pages as a tree: click or Enter opens a page; a page is
 * dragged before, after or into another, or moved with Alt and the arrow
 * keys (up and down among its siblings, right into the page above it, left
 * out of its parent). Right-click or the Menu key opens the page menu.
 */
export function PageTree({ pages, current, writable, onOpen, onMove, onNew, onRename, onDelete, onOpenWindow }: PageTreeProps) {
  const t = useT();
  const menu = useContextMenu(t("wiki-page-menu"));
  const [collapsed, setCollapsed] = useState<readonly string[]>([]);
  const tree = buildTree(pages);
  const byId = new Map(pages.map((page) => [page.id, page]));

  const { dragAndDropHooks } = useDragAndDrop({
    isDisabled: !writable,
    getItems: (keys) => [...keys].map((key) => ({ [PAGE_TYPE]: String(key), "text/plain": byId.get(String(key))?.title ?? "" })),
    getDropOperation: (target, _types, allowed) => {
      if (target.type !== "item") {
        return "cancel";
      }
      return allowed.includes("move") ? "move" : "cancel";
    },
    onMove: (event) => {
      const [dragged] = [...event.keys].map(String);
      if (dragged === undefined) {
        return;
      }
      const move = moveFor(pages, dragged, String(event.target.key), event.target.dropPosition);
      if (move) {
        onMove(move);
      }
    },
  });

  const entries = (page: WikiPageSummary): MenuEntry[] => [
    { id: "open", label: t("wiki-open"), run: () => { onOpen(page.id); } },
    { id: "window", label: t("wiki-open-window"), run: () => { onOpenWindow(page); } },
    "separator",
    { id: "child", label: t("wiki-new-child"), run: () => { onNew(page.id); }, disabled: !writable },
    { id: "rename", label: t("wiki-rename"), run: () => { onRename(page); }, disabled: !writable },
    ...(["up", "down", "in", "out"] as const).map((direction): MenuEntry => {
      const move = keyboardMove(pages, page.id, direction);
      return { id: `move-${direction}`, label: t(`wiki-move-${direction}`), run: () => { if (move) { onMove(move); } }, disabled: !writable || move === null };
    }),
    "separator",
    { id: "delete", label: t("wiki-delete"), run: () => { onDelete(page); }, disabled: !writable },
  ];

  const item = (node: WikiNode): ReactNode => {
    const { page } = node;
    return (
      <TreeItem key={page.id} id={page.id} textValue={page.title} className={classes(styles.treeItem)} data-page={page.id}>
        <TreeItemContent>
          {({ isExpanded, isDropTarget }) => (
            <div
              className={classes("nav", styles.treeRow, page.id === current && "on", isDropTarget && styles.dropTarget)}
              onContextMenu={(event) => {
                menu.open(event, entries(page));
              }}
            >
              {node.children.length > 0 ? (
                <Button slot="chevron" className={classes(styles.chevron, !isExpanded && styles.closed)} aria-label={t(isExpanded ? "wiki-collapse" : "wiki-expand", { page: page.title })}>
                  <ChevronDown className="ic s muted" />
                </Button>
              ) : (
                <span className={styles.chevron} />
              )}
              <span className="truncate grow">{page.title}</span>
              {page.agent_brief === true && <span className="chip hue-purple">{t("wiki-agent-brief")}</span>}
              {writable && (
                <Button slot="drag" className={classes(styles.dragHandle)} aria-label={t("wiki-drag", { page: page.title })}>
                  <GripVertical className="ic s faint" />
                </Button>
              )}
            </div>
          )}
        </TreeItemContent>
        {node.children.map(item)}
      </TreeItem>
    );
  };

  const expanded = pages.filter((page) => page.children > 0 && !collapsed.includes(page.id)).map((page) => page.id);

  return (
    <aside className={styles.treePane} aria-label={t("wiki-pages")}>
      <div className={styles.treeHead}>
        <span className="grow">{t("wiki-pages")}</span>
        <Tooltip text={t("wiki-new-page")}>
          <Button className="btn flat icon sm" aria-label={t("wiki-new-page")} isDisabled={!writable} onPress={() => { onNew(null); }}>
            <Plus className="ic s" />
          </Button>
        </Tooltip>
      </div>
      {pages.length === 0 ? (
        <p className={classes("faint", styles.treeEmpty)}>{t("wiki-no-pages")}</p>
      ) : (
        <div
          className={styles.treeScroll}
          onKeyDownCapture={(event) => {
            const row = (event.target as Element).closest("[data-key]");
            const page = byId.get(row?.getAttribute("data-key") ?? "");
            if (!row || !page) {
              return;
            }
            if (isMenuKey(event)) {
              event.preventDefault();
              event.stopPropagation();
              menu.openAt(row, entries(page));
              return;
            }
            const direction = event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey ? MOVE_KEYS[event.key] : undefined;
            if (direction !== undefined) {
              event.preventDefault();
              event.stopPropagation();
              const move = writable ? keyboardMove(pages, page.id, direction) : null;
              if (move) {
                onMove(move);
                // The page keeps the focus at its new place, once the tree has it.
                requestAnimationFrame(() => {
                  requestAnimationFrame(() => {
                    document.querySelector<HTMLElement>(`[data-key="${CSS.escape(page.id)}"]`)?.focus();
                  });
                });
              }
            }
          }}
        >
          <Tree
            aria-label={t("wiki-pages")}
            className={classes(styles.tree)}
            selectionMode="single"
            selectionBehavior="replace"
            selectedKeys={current === undefined ? [] : [current]}
            disallowEmptySelection
            onSelectionChange={(keys) => {
              const [key] = keys === "all" ? [] : [...keys];
              if (key !== undefined) {
                onOpen(String(key));
              }
            }}
            expandedKeys={expanded}
            onExpandedChange={(keys: Set<Key>) => {
              setCollapsed(pages.filter((page) => page.children > 0 && !keys.has(page.id)).map((page) => page.id));
            }}
            dragAndDropHooks={dragAndDropHooks}
          >
            {tree.map(item)}
          </Tree>
        </div>
      )}
      {menu.menu}
    </aside>
  );
}

