// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useRef, useState, type MouseEvent as ReactMouseEvent, type ReactNode } from "react";
import { Header, Menu, MenuItem, MenuSection, Popover, Separator, SubmenuTrigger, Text } from "react-aria-components";

/** One entry of a context menu. */
export type MenuEntry =
  | {
      id: string;
      label: string;
      run: () => void;
      disabled?: boolean;
      /**
       * Whether the entry is chosen, for entries that are choices. Neighbouring
       * choices are one choice of several (radio items), unless `checkbox` says
       * each is turned on and off on its own.
       */
      checked?: boolean;
      /** The entry is an on/off choice of its own (a checkbox item). */
      checkbox?: boolean;
      icon?: ReactNode;
      /** A line under the label, read as the item's description. */
      detail?: string;
    }
  | { id: string; label: string; submenu: MenuEntry[] }
  | { id: string; section: string; items: MenuEntry[] }
  | "separator";

type ActionEntry = Extract<MenuEntry, { run: () => void }>;

/** How an entry is chosen: not at all, one of several, or on its own. */
type ChoiceMode = "none" | "single" | "multiple";

function choiceMode(entry: MenuEntry): ChoiceMode {
  if (entry === "separator" || !("run" in entry) || entry.checked === undefined) {
    return "none";
  }
  return entry.checkbox === true ? "multiple" : "single";
}

/** `entries` cut into runs of neighbours that are chosen the same way. */
function runs(entries: readonly MenuEntry[]): { mode: ChoiceMode; entries: MenuEntry[] }[] {
  const out: { mode: ChoiceMode; entries: MenuEntry[] }[] = [];
  for (const entry of entries) {
    const mode = choiceMode(entry);
    const last = out.at(-1);
    if (last !== undefined && last.mode === mode && entry !== "separator") {
      last.entries.push(entry);
    } else {
      out.push({ mode, entries: [entry] });
    }
  }
  return out;
}

/** The IDs of the chosen entries of a run. */
function chosen(entries: readonly MenuEntry[]): string[] {
  return entries.filter((entry): entry is ActionEntry => entry !== "separator" && "run" in entry && entry.checked === true).map((entry) => entry.id);
}

/**
 * Renders entries into a React Aria menu. Choices are menu items with the
 * radio or checkbox role and their state (`aria-checked`); a check mark shows
 * the chosen ones.
 */
export function MenuEntries({ entries, onClose }: { entries: readonly MenuEntry[]; onClose?: () => void }) {
  const close = onClose ? { onClose } : {};
  return (
    <>
      {runs(entries).map((run, index) => {
        if (run.mode === "none") {
          return run.entries.map((entry, at) => <Entry key={entryKey(entry, `${String(index)}-${String(at)}`)} entry={entry} {...close} />);
        }
        const first = run.entries[0] as ActionEntry;
        return (
          <MenuSection key={`choices-${first.id}`} id={`choices-${first.id}`} selectionMode={run.mode} selectedKeys={chosen(run.entries)}>
            {run.entries.map((entry, at) => (
              <Entry key={entryKey(entry, `${String(index)}-${String(at)}`)} entry={entry} {...close} />
            ))}
          </MenuSection>
        );
      })}
    </>
  );
}

function entryKey(entry: MenuEntry, fallback: string): string {
  return entry === "separator" ? `separator-${fallback}` : entry.id;
}

/** A section: its heading, then its entries, each run of choices a group of its own. */
function Section({ entry, onClose }: { entry: Extract<MenuEntry, { section: string }>; onClose?: () => void }) {
  const close = onClose ? { onClose } : {};
  return (
    <>
      {runs(entry.items).map((run, index) => {
        const id = index === 0 ? entry.id : `${entry.id}-${String(index)}`;
        const choice = run.mode === "none" ? {} : { selectionMode: run.mode, selectedKeys: chosen(run.entries) };
        return (
          <MenuSection key={id} id={id} {...choice}>
            {index === 0 && <Header className="caption menuHeader">{entry.section}</Header>}
            {run.entries.map((item, at) => (
              <Entry key={entryKey(item, `${String(index)}-${String(at)}`)} entry={item} {...close} />
            ))}
          </MenuSection>
        );
      })}
    </>
  );
}

function Entry({ entry, onClose }: { entry: MenuEntry; onClose?: () => void }) {
  if (entry === "separator") {
    return <Separator className="menuSeparator" />;
  }
  if ("section" in entry) {
    return <Section entry={entry} {...(onClose ? { onClose } : {})} />;
  }
  if ("submenu" in entry) {
    return (
      <SubmenuTrigger>
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
      id={entry.id}
      className="item"
      textValue={entry.label}
      isDisabled={entry.disabled ?? false}
      // A choice closes the menu as any other entry does.
      shouldCloseOnSelect
      onAction={() => {
        entry.run();
        onClose?.();
      }}
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
      {/* The pointer's position is physical in every writing direction. */}
      {/* eslint-disable-next-line no-restricted-syntax */}
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
