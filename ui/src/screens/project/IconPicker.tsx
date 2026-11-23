// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { RadioButton, RadioField, RadioGroup } from "react-aria-components";

import { classes } from "../../classes";
import { PROJECT_ICONS, ProjectIconGlyph } from "../../components/ProjectMark";
import { type Hue, type ProjectIcon } from "../../generated/ops";
import { useT } from "../../i18n/I18n";
import styles from "./Project.module.css";

const CIRCLE = "circle";

interface IconPickerProps {
  label: string;
  /** The project's colour, which the chosen icon is drawn in. */
  hue: Hue;
  /** No icon: the circle. */
  value: ProjectIcon | undefined;
  onChange: (icon: ProjectIcon | undefined) => void;
}

/**
 * The project's mark: the circle, or one of the icons, in a grid. Each
 * icon's name shows above it while the pointer rests on it or it has the
 * keyboard focus, and is its accessible name; arrow keys move.
 */
export function IconPicker({ label, hue, value, onChange }: IconPickerProps) {
  const t = useT();
  const choices = [CIRCLE, ...PROJECT_ICONS] as const;
  return (
    <RadioGroup
      aria-label={label}
      className={classes(styles.icons, `hue-${hue}`)}
      value={value ?? CIRCLE}
      onChange={(next) => {
        onChange(PROJECT_ICONS.find((icon) => icon === next));
      }}
    >
      {choices.map((choice) => {
        const name = t(`project-icon-${choice}`);
        return (
          <RadioField key={choice} value={choice} aria-label={name} className={classes(styles.iconField)}>
            <RadioButton data-icon={choice} className={({ isSelected, isFocusVisible }) => classes(styles.iconChoice, isSelected && styles.iconOn, isFocusVisible && styles.iconFocus)}>
              {choice === CIRCLE ? <span className={classes("dot", styles.iconCircle)} aria-hidden="true" /> : <ProjectIconGlyph icon={choice} />}
              <span className={styles.iconName} aria-hidden="true">
                {name}
              </span>
            </RadioButton>
          </RadioField>
        );
      })}
    </RadioGroup>
  );
}
