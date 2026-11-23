// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type ReactNode } from "react";
import { SwitchButton, SwitchField, type SwitchFieldProps } from "react-aria-components";

import { classes } from "../classes";

interface SwitchProps extends Omit<SwitchFieldProps, "children" | "className"> {
  children?: ReactNode;
}

/** The design's switch, for settings that apply at once. Never for an action. */
export function Switch({ children, ...props }: SwitchProps) {
  return (
    <SwitchField {...props}>
      <SwitchButton className="switchField">
        {({ isSelected }) => (
          <>
            <span className={classes("switch", isSelected && "on")} aria-hidden="true" />
            {children}
          </>
        )}
      </SwitchButton>
    </SwitchField>
  );
}
