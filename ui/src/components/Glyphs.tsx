// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type Glyph, type Priority } from "../data/catalog";
import { useT } from "../i18n/I18n";

interface GlyphProps {
  /** Spoken name; without one the glyph is decoration. */
  label?: string | undefined;
}

function role(label: string | undefined) {
  return label === undefined ? { "aria-hidden": true as const } : { role: "img", "aria-label": label };
}

/** The task type glyph: one drawing holds the four shapes, the class shows one. */
export function TypeGlyph({ glyph, label }: GlyphProps & { glyph: Glyph }) {
  return (
    <svg className={`tt t-${glyph}`} viewBox="0 0 16 16" {...role(label)}>
      <path className="s-task" d="M3 3h10v10H3zM5.5 8.2l1.8 1.8 3.2-3.8" />
      <circle className="s-bug" cx="8" cy="8" r="4" />
      <path className="s-feature" d="M8 3v10M3 8h10" />
      <path className="s-milestone" d="M8 2.5 13.5 8 8 13.5 2.5 8z" />
    </svg>
  );
}

/** The priority glyph: bars count up; urgent is a filled alert box. */
export function PriorityGlyph({ priority, label }: GlyphProps & { priority: Priority }) {
  return (
    <svg className={`prio p-${priority}`} viewBox="0 0 16 16" {...role(label)}>
      <rect className="b b1" x="2" y="10" width="3" height="4" rx="1" />
      <rect className="b b2" x="6.5" y="6.5" width="3" height="7.5" rx="1" />
      <rect className="b b3" x="11" y="3" width="3" height="11" rx="1" />
      <path
        className="u"
        d="M3.5 2h9A1.5 1.5 0 0 1 14 3.5v9a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 12.5v-9A1.5 1.5 0 0 1 3.5 2zm3.7 2v5h1.6V4zm0 6.4V12h1.6v-1.6z"
      />
    </svg>
  );
}

/** The name of a priority, for menus and labels. */
export function usePriorityName(): (priority: Priority) => string {
  const t = useT();
  return (priority) => t(`priority-${priority}`);
}
