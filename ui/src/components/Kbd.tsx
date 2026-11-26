// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { chordParts, type Chord } from "../actions/keymap";
import { useT, type Translate } from "../i18n/I18n";
import { platform } from "../platform";

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
  ContextMenu: "key-menu",
};

/** On Windows the Super key is the Windows key, and is named so. */
const WINDOWS_MESSAGES: Readonly<Record<string, string>> = { super: "key-windows" };

/** A chord as the hint shows it: "Ctrl K", "Ctrl Shift Z", "F9", "?". */
export function chordLabel(chord: Chord, t: Translate): string {
  const own = platform() === "windows" ? WINDOWS_MESSAGES : {};
  return chordParts(chord)
    .map((part) => {
      const message = own[part] ?? KEY_MESSAGES[part];
      return message ? t(message) : part.toUpperCase();
    })
    .join(" ");
}

/** The shortcut hint `.kbd`. */
export function Kbd({ chord }: { chord: Chord }) {
  const t = useT();
  return <span className="kbd">{chordLabel(chord, t)}</span>;
}
