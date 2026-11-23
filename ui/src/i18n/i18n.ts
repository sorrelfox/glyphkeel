// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { FluentBundle, FluentResource } from "@fluent/bundle";

import enApp from "./en/app.ftl?raw";
import enDev from "./en/dev.ftl?raw";
import enEditor from "./en/editor.ftl?raw";
import enList from "./en/list.ftl?raw";
import enPalette from "./en/palette.ftl?raw";
import enReader from "./en/reader.ftl?raw";
import enSearch from "./en/search.ftl?raw";
import enToday from "./en/today.ftl?raw";
// The engine's messages about queries, kept with the query language in Rust.
import enQuery from "../../../crates/gk-query/i18n/en/query.ftl?raw";
// The engine's messages about search, Today, the wiki and dates.
import enEngine from "../../../crates/gk-engine/i18n/en/engine.ftl?raw";

/** The languages the app has text for, with their message files. */
export const MESSAGES: Readonly<Record<string, readonly string[]>> = {
  en: [enApp, enDev, enEditor, enEngine, enList, enPalette, enQuery, enReader, enSearch, enToday],
};

/** The language the app falls back to. */
export const SOURCE_LANGUAGE = "en";

/** Picks the best language with messages for a BCP 47 locale. */
export function messageLanguage(locale: string): string {
  const language = locale.split("-")[0]?.toLowerCase() ?? SOURCE_LANGUAGE;
  return language in MESSAGES ? language : SOURCE_LANGUAGE;
}

/** Builds the message bundle for a language. A syntax error in a file is a bug, so it throws. */
export function createBundle(language: string): FluentBundle {
  const bundle = new FluentBundle(language);
  for (const source of MESSAGES[language] ?? []) {
    const errors = bundle.addResource(new FluentResource(source));
    if (errors.length > 0) {
      throw new Error(`messages for ${language}: ${errors.map((e) => e.message).join("; ")}`);
    }
  }
  return bundle;
}
