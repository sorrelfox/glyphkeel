// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { FluentBundle, FluentResource } from "@fluent/bundle";

import { PSEUDO_LOCALE, pseudoResource } from "./pseudo";

import enAgents from "./en/agents.ftl?raw";
import enApp from "./en/app.ftl?raw";
import enCalendar from "./en/calendar.ftl?raw";
import enDev from "./en/dev.ftl?raw";
import enEditor from "./en/editor.ftl?raw";
import enFirstRun from "./en/first-run.ftl?raw";
import enList from "./en/list.ftl?raw";
import enPalette from "./en/palette.ftl?raw";
import enPrefs from "./en/prefs.ftl?raw";
import enReader from "./en/reader.ftl?raw";
import enSafety from "./en/safety.ftl?raw";
import enSearch from "./en/search.ftl?raw";
import enToday from "./en/today.ftl?raw";
// The engine's messages about queries, kept with the query language in Rust.
import enQuery from "../../../crates/gk-query/i18n/en/query.ftl?raw";
// The engine's messages about search, Today, the wiki and dates.
import enEngine from "../../../crates/gk-engine/i18n/en/engine.ftl?raw";

/** The languages the app has text for, with their message files. */
export const MESSAGES: Readonly<Record<string, readonly string[]>> = {
  en: [enAgents, enApp, enCalendar, enDev, enEditor, enEngine, enFirstRun, enList, enPalette, enPrefs, enQuery, enReader, enSafety, enSearch, enToday],
};

/** The language the app falls back to. */
export const SOURCE_LANGUAGE = "en";

/**
 * Whether this build offers the pseudo-locale: development builds, and the
 * builds for the development bridge and the sample data, which the
 * screenshot runs use. Release builds don't.
 */
export const PSEUDO_AVAILABLE: boolean = import.meta.env.DEV || __BRIDGE__ || __FAKE__;

/** Picks the best language with messages for a BCP 47 locale. */
export function messageLanguage(locale: string): string {
  if (PSEUDO_AVAILABLE && locale === PSEUDO_LOCALE) {
    return PSEUDO_LOCALE;
  }
  const language = locale.split("-")[0]?.toLowerCase() ?? SOURCE_LANGUAGE;
  return language in MESSAGES ? language : SOURCE_LANGUAGE;
}

/** The languages a person can choose, by tag: those with messages, and the pseudo-locale where offered. */
export function choosableLanguages(): string[] {
  return [...Object.keys(MESSAGES), ...(PSEUDO_AVAILABLE ? [PSEUDO_LOCALE] : [])];
}

/**
 * Builds the message bundle for a language; the pseudo-locale is English,
 * transformed. A syntax error in a file is a bug, so it throws.
 */
export function createBundle(language: string): FluentBundle {
  const pseudo = language === PSEUDO_LOCALE;
  const bundle = new FluentBundle(pseudo ? SOURCE_LANGUAGE : language);
  for (const source of MESSAGES[pseudo ? SOURCE_LANGUAGE : language] ?? []) {
    const resource = new FluentResource(source);
    const errors = bundle.addResource(pseudo ? pseudoResource(resource) : resource);
    if (errors.length > 0) {
      throw new Error(`messages for ${language}: ${errors.map((e) => e.message).join("; ")}`);
    }
  }
  return bundle;
}
