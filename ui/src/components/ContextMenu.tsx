// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useRef, useState, type MouseEvent as ReactMouseEvent, type ReactNode } from "react";
import { Header, Menu, MenuItem, MenuSection, Popover, Separator, SubmenuTrigger, Text } from "react-aria-components";

/** One entry of a context menu. */
export type MenuEntry =
  | { id: string; label: string; run: () => void; disabled?: boolean; checked?: boolean; icon?: ReactNode; /** A line under the label, read as the item's description. */ detail?: string }
  | { id: string; label: string; submenu: MenuEntry[] }
  | { id: string; section: string; items: MenuEntry[] }
  | "separator";

/** Renders entries into a React Aria menu. */
export function MenuEntries({ entries, onClose }: { entries: readonly MenuEntry[]; onClose?: () => void }) {
  return (
    <>
      {entries.map((entry, index) => {
        if (entry === "separator") {
          return <Separator key={`separator-${String(index)}`} className="menuSeparator" />;
        }
        if ("section" in entry) {
          return (
            <MenuSection key={entry.id} id={entry.id}>
              <Header className="caption menuHeader">{entry.section}</Header>
              <MenuEntries entries={entry.items} {...(onClose ? { onClose } : {})} />
            </MenuSection>
          );
        }
        if ("submenu" in entry) {
          return (
            <SubmenuTrigger key={entry.id}>
              <MenuItem id={entry.id} className="item" textValue={entry.label}>
                <span className="grow truncate">{entry.label}</span>
                <span aria-hidden="true" className="muted">
                  ›
                </span>
              </MenuItem>
              <Popover className="pop menuPopover" offset={-4}>
                <Menu className="menu">
                  <MenuEntries entries={entry.submenu} {...(onClose ? { onClose } : {})} />
                </Menu>
              </Popover>
            </SubmenuTrigger>
          );
        }
        return (
          <MenuItem
            key={entry.id}
            id={entry.id}
            className="item"
            textValue={entry.label}
            isDisabled={entry.disabled ?? false}
            onAction={() => {
              entry.run();
              onClose?.();
            }}
            {...(entry.checked === undefined ? {} : { "aria-checked": entry.checked })}
          >
            {entry.checked !== undefined && (
              <span className="menuCheck" aria-hidden="true">
                {entry.checked ? "✓" : ""}
              </span>
            )}
            {entry.icon}
            {entry.detail === undefined || entry.detail === "" ? (
              <span className="grow truncate">{entry.label}</span>
            ) : (
              <span className="grow menuText">
                <Text slot="label" className="truncate">
                  {entry.label}
                </Text>
                <Text slot="description" className="menuDetail">
                  {entry.detail}
                </Text>
              </span>
            )}
          </MenuItem>
        );
      })}
    </>
  );
}

/**
 * A menu at the pointer, for right-clicks; also opened from the keyboard
 * (Shift+F10, the Menu key) at the focused element.
 */
export function useContextMenu(label: string) {
  const anchor = useRef<HTMLDivElement | null>(null);
  const [at, setAt] = useState<{ x: number; y: number; entries: readonly MenuEntry[] } | null>(null);

  const open = (event: ReactMouseEvent | { clientX: number; clientY: number }, entries: readonly MenuEntry[]) => {
    if ("preventDefault" in event) {
      event.preventDefault();
      event.stopPropagation();
    }
    setAt({ x: event.clientX, y: event.clientY, entries });
  };

  const openAt = (element: Element, entries: readonly MenuEntry[]) => {
    const rect = element.getBoundingClientRect();
    setAt({ x: rect.left + 8, y: rect.top + Math.min(rect.height, 24), entries });
  };

  const close = () => {
    setAt(null);
  };

  const menu = at && (
    <>
      <div ref={anchor} style={{ position: "fixed", left: at.x, top: at.y, width: 0, height: 0 }} />
      <Popover
        className="pop menuPopover"
        triggerRef={anchor}
        placement="bottom start"
        offset={2}
        isOpen
        onOpenChange={(isOpen) => {
          if (!isOpen) {
            close();
          }
        }}
      >
        <Menu className="menu" aria-label={label} autoFocus="first">
          <MenuEntries entries={at.entries} onClose={close} />
        </Menu>
      </Popover>
    </>
  );

  return { open, openAt, close, menu };
}

/** Whether a key press asks for the context menu. */
export function isMenuKey(event: { key: string; shiftKey: boolean }): boolean {
  return event.key === "ContextMenu" || (event.shiftKey && event.key === "F10");
}
