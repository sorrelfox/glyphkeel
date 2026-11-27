// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { Popover, type PopoverProps } from "react-aria-components";

import { classes } from "../classes";

interface FitPopoverProps extends Omit<PopoverProps, "className" | "boundaryElement"> {
  className?: string;
}

/**
 * The area a popover stays inside: the window's content, below the header
 * bar, where there is one; else the whole page (a task's own window).
 */
function contentArea(): { boundaryElement?: Element } {
  return typeof document === "undefined" ? {} : { boundaryElement: document.querySelector('[data-region="content"]') ?? document.body };
}

/**
 * A popover that stays inside the content area: it flips or shifts to fit,
 * is never taller than the room it has, and scrolls its body instead of
 * spilling out of its box. A dialog inside lays out as a column; a form in
 * it with the `popBody` and `popFoot` parts keeps its buttons in view while
 * the body scrolls.
 */
export function FitPopover({ className, containerPadding = 8, ...props }: FitPopoverProps) {
  return <Popover {...props} className={classes("pop fitPop", className)} {...contentArea()} containerPadding={containerPadding} />;
}
