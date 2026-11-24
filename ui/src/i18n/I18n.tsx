// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type FluentVariable } from "@fluent/bundle";
import { LocalizationProvider, ReactLocalization, useLocalization } from "@fluent/react";
import { createContext, use, useMemo, useState, type ReactNode } from "react";
import { I18nProvider } from "react-aria-components";

import { readPref, writePref } from "../prefs/store";
import { choosableLanguages, createBundle, messageLanguage, SOURCE_LANGUAGE } from "./i18n";

/**
 * A missing or broken message is a bug: tests fail on it, development builds
 * report it, and release builds show the message ID rather than nothing.
 */
function reportError(error: Error): void {
  if (import.meta.env.MODE === "test") {
    throw error;
  }
  console.error(error);
}

function localization(locale: string): ReactLocalization {
  const language = messageLanguage(locale);
  const bundles = language === SOURCE_LANGUAGE ? [createBundle(language)] : [createBundle(language), createBundle(SOURCE_LANGUAGE)];
  return new ReactLocalization(bundles, null, reportError);
}

/** The language of the interface: the system's, or one chosen by its tag. */
export const SYSTEM_LANGUAGE = "system";
const LANGUAGE_PREF = "language";

/** The chosen language, kept with this person's preferences; an unknown one means the system's. */
export function loadLanguage(): string {
  const stored = readPref(LANGUAGE_PREF);
  return typeof stored === "string" && choosableLanguages().includes(stored) ? stored : SYSTEM_LANGUAGE;
}

interface LanguageChoice {
  /** `"system"` or a language tag. */
  language: string;
  setLanguage: (language: string) => void;
}

const LanguageContext = createContext<LanguageChoice>({ language: SYSTEM_LANGUAGE, setLanguage: () => undefined });

/** The chosen language of the interface, and a way to change it; it applies at once. */
export function useLanguage(): LanguageChoice {
  return use(LanguageContext);
}

interface I18nProps {
  /** The user's locale. It picks the messages (unless a language is chosen) and formats dates and numbers. */
  locale: string;
  children: ReactNode;
}

/**
 * The app's text and formats. WebKit takes its own locale from `LANG` only, so
 * React Aria gets the locale explicitly. A language chosen in Preferences
 * changes the text only: dates and numbers keep the system's formats.
 */
export function I18n({ locale, children }: I18nProps) {
  const [language, setLanguage] = useState(loadLanguage);
  const l10n = useMemo(() => localization(language === SYSTEM_LANGUAGE ? locale : language), [language, locale]);
  const choice = useMemo<LanguageChoice>(
    () => ({
      language,
      setLanguage: (next) => {
        setLanguage(next);
        writePref(LANGUAGE_PREF, next);
      },
    }),
    [language],
  );
  return (
    <LanguageContext value={choice}>
      <LocalizationProvider l10n={l10n}>
        <I18nProvider locale={locale}>{children}</I18nProvider>
      </LocalizationProvider>
    </LanguageContext>
  );
}

export type Translate = ((id: string, vars?: Record<string, FluentVariable>) => string) & {
  /** Whether a message exists, for IDs that come from elsewhere (the engine's). */
  has?: (id: string) => boolean;
};

/** The function that turns a message ID into text. */
export function useT(): Translate {
  const { l10n } = useLocalization();
  return Object.assign((id: string, vars?: Record<string, FluentVariable>) => l10n.getString(id, vars ?? null), {
    has: (id: string) => l10n.getBundle(id) !== null,
  });
}
