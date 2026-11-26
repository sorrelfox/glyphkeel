// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { chordParts, type Chord } from "../actions/keymap";
import { useT, type Translate } from "../i18n/I18n";

/** Message IDs of the keys that have a name of their own in hints. */
const KEY_MESSAGES: Readonly<Record<string, string>> = {
  ctrl: "key-ctrl",
  alt: "key-alt",
  shift: "key-shift",
  super: "key-super",
  Escape: "key-escape",
  Delete: "key-delete",
  Enter: "key-enter",
  " ": "key-space",
  ArrowLeft: "key-left",
  ArrowRight: "key-right",
  ArrowUp: "key-up",
  ArrowDown: "key-down",
  PageUp: "key-page-up",
  PageDown: "key-page-down",
  Home: "key-home",
  End: "key-end",
};

/** A chord as the hint shows it: "Ctrl K", "Ctrl Shift Z", "F9", "?". */
export function chordLabel(chord: Chord, t: Translate): string {
  return chordParts(chord)
    .map((part) => {
      const message = KEY_MESSAGES[part];
      return message ? t(message) : part.toUpperCase();
    })
    .join(" ");
}

/** The shortcut hint `.kbd`. */
export function Kbd({ chord }: { chord: Chord }) {
  const t = useT();
  return <span className="kbd">{chordLabel(chord, t)}</span>;
}
