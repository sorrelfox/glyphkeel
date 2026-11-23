// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { RadioButton, RadioField, RadioGroup } from "react-aria-components";

import { classes } from "../../classes";
import { HUES, type Hue } from "../../data/catalog";
import { useT } from "../../i18n/I18n";
import styles from "./Project.module.css";

interface HuePickerProps {
  label: string;
  value: Hue;
  onChange: (hue: Hue) => void;
}

/** The nine hues as round swatches; arrow keys move between them. */
export function HuePicker({ label, value, onChange }: HuePickerProps) {
  const t = useT();
  return (
    <RadioGroup
      aria-label={label}
      orientation="horizontal"
      className={classes(styles.hues)}
      value={value}
      onChange={(next) => {
        const hue = HUES.find((candidate) => candidate === next);
        if (hue) {
          onChange(hue);
        }
      }}
    >
      {HUES.map((hue) => (
        <RadioField key={hue} value={hue} aria-label={t(`hue-${hue}`)}>
          <RadioButton data-hue={hue} className={({ isSelected, isFocusVisible }) => classes(styles.swatch, `hue-${hue}`, isSelected && styles.swatchOn, isFocusVisible && styles.swatchFocus)} />
        </RadioField>
      ))}
    </RadioGroup>
  );
}
