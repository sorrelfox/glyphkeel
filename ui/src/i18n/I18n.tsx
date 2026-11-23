// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { type FluentVariable } from "@fluent/bundle";
import { LocalizationProvider, ReactLocalization, useLocalization } from "@fluent/react";
import { useState, type ReactNode } from "react";
import { I18nProvider } from "react-aria-components";

import { createBundle, messageLanguage, SOURCE_LANGUAGE } from "./i18n";

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

interface I18nProps {
  /** The user's locale. It picks the messages and formats dates in React Aria's components. */
  locale: string;
  children: ReactNode;
}

/**
 * The app's text and formats. WebKit takes its own locale from `LANG` only, so
 * React Aria gets the locale explicitly.
 */
export function I18n({ locale, children }: I18nProps) {
  const [l10n] = useState(() => localization(locale));
  return (
    <LocalizationProvider l10n={l10n}>
      <I18nProvider locale={locale}>{children}</I18nProvider>
    </LocalizationProvider>
  );
}

export type Translate = (id: string, vars?: Record<string, FluentVariable>) => string;

/** The function that turns a message ID into text. */
export function useT(): Translate {
  const { l10n } = useLocalization();
  return (id, vars) => l10n.getString(id, vars ?? null);
}
