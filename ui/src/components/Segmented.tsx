// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type ReactNode } from "react";
import { ToggleButton, ToggleButtonGroup } from "react-aria-components";

export interface Segment<T extends string> {
  id: T;
  label: ReactNode;
  /** The accessible name, when the label is not text. */
  ariaLabel?: string;
}

interface SegmentedProps<T extends string> {
  /** The accessible name of the group. */
  label: string;
  segments: readonly Segment<T>[];
  value: T;
  onChange: (value: T) => void;
}

/** The segmented control `.seg`: one tab stop, arrow keys move between segments. */
export function Segmented<T extends string>({ label, segments, value, onChange }: SegmentedProps<T>) {
  return (
    <ToggleButtonGroup
      className="seg"
      aria-label={label}
      selectionMode="single"
      disallowEmptySelection
      selectedKeys={[value]}
      onSelectionChange={(keys) => {
        const [key] = keys;
        const segment = segments.find((candidate) => candidate.id === key);
        if (segment) {
          onChange(segment.id);
        }
      }}
    >
      {segments.map((segment) => (
        <ToggleButton
          key={segment.id}
          id={segment.id}
          className={({ isSelected }) => (isSelected ? "on" : "")}
          {...(segment.ariaLabel === undefined ? {} : { "aria-label": segment.ariaLabel })}
        >
          {segment.label}
        </ToggleButton>
      ))}
    </ToggleButtonGroup>
  );
}
