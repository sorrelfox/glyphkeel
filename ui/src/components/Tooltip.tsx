// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type ReactElement } from "react";
import { Tooltip as AriaTooltip, TooltipTrigger } from "react-aria-components";

import { type Chord } from "../actions/keymap";
import { Kbd } from "./Kbd";

interface TooltipProps {
  /** The control the tooltip describes: a React Aria button. */
  children: ReactElement;
  text: string;
  chord?: Chord | undefined;
}

/** The control's name and its shortcut, after the pointer rests on it for half a second. */
export function Tooltip({ children, text, chord }: TooltipProps) {
  return (
    <TooltipTrigger delay={500}>
      {children}
      <AriaTooltip className="tooltip" offset={6}>
        {text}
        {chord && <Kbd chord={chord} />}
      </AriaTooltip>
    </TooltipTrigger>
  );
}
