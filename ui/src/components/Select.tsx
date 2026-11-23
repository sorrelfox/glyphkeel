// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Check, ChevronDown } from "lucide-react";
import { type ReactNode } from "react";
import { Button, ListBox, ListBoxItem, Popover, Select as AriaSelect } from "react-aria-components";

import { classes } from "../classes";
import styles from "./Select.module.css";

export interface SelectOption<T extends string> {
  id: T;
  /** The text shown, also used for typeahead. */
  label: string;
  /** Drawn before the label, in the list and in the field: a colour dot or a glyph. */
  icon?: ReactNode;
}

interface SelectProps<T extends string> {
  /** The accessible name of the field. */
  label: string;
  options: readonly SelectOption<T>[];
  value: T;
  onChange: (value: T) => void;
  className?: string | undefined;
  isDisabled?: boolean | undefined;
}

/**
 * A choice of one value from a short list: a field that opens the design's
 * menu popover (`.pop`, `.item`) with a check mark at the chosen item. The
 * popover takes the window's theme like every overlay. Arrow keys, typeahead,
 * Home and End work as in any list box.
 */
export function Select<T extends string>({ label, options, value, onChange, className, isDisabled }: SelectProps<T>) {
  const current = options.find((option) => option.id === value);
  return (
    <AriaSelect
      aria-label={label}
      className={classes(styles.select, className)}
      value={value}
      isDisabled={isDisabled ?? false}
      onChange={(key) => {
        const option = options.find((candidate) => candidate.id === key);
        if (option) {
          onChange(option.id);
        }
      }}
    >
      <Button className={({ isFocusVisible }) => classes("field", styles.trigger, isFocusVisible && styles.focus)}>
        {current?.icon}
        <span className={classes("truncate", styles.value)}>{current?.label}</span>
        <ChevronDown className={classes("ic s", styles.chevron)} aria-hidden="true" />
      </Button>
      <Popover className={classes("pop menuPopover", styles.popover)} placement="bottom start" offset={4}>
        <ListBox className="menu" items={options}>
          {(option) => (
            <ListBoxItem id={option.id} textValue={option.label} className="item">
              {({ isSelected }) => (
                <>
                  {option.icon}
                  <span className="grow truncate">{option.label}</span>
                  <Check className={classes("ic s", styles.check, !isSelected && styles.hidden)} aria-hidden="true" />
                </>
              )}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </AriaSelect>
  );
}
