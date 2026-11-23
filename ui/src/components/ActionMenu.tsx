// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type ReactNode } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover, Separator } from "react-aria-components";

import { type Action } from "../actions/actions";
import { useT } from "../i18n/I18n";
import { Kbd } from "./Kbd";

/** A divider between groups of menu items. */
export const SEPARATOR = "separator";

interface ActionMenuProps {
  /** The accessible name of the button that opens the menu. */
  label: string;
  /** What the button shows: usually an icon. */
  icon: ReactNode;
  items: readonly (Action | typeof SEPARATOR)[];
}

/**
 * A menu button whose items are actions from the registry, each with its
 * shortcut hint. Unavailable actions stay in place, dimmed.
 */
export function ActionMenu({ label, icon, items }: ActionMenuProps) {
  const t = useT();
  return (
    <MenuTrigger>
      <Button className="btn flat icon" aria-label={label}>
        {icon}
      </Button>
      <Popover className="pop menuPopover" placement="bottom end" offset={4}>
        <Menu className="menu">
          {items.map((item, index) =>
            item === SEPARATOR ? (
              <Separator key={`separator-${String(index)}`} className="menuSeparator" />
            ) : (
              <MenuItem
                key={item.id}
                id={item.id}
                className="item"
                textValue={t(item.label)}
                isDisabled={item.disabled ?? false}
                onAction={item.run}
              >
                <span className="grow truncate">{t(item.label)}</span>
                {item.keys[0] && <Kbd chord={item.keys[0]} />}
              </MenuItem>
            ),
          )}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
