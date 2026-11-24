// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { useLocale } from "react-aria-components";

import { Select } from "../../components/Select";
import { choosableLanguages, messageLanguage } from "../../i18n/i18n";
import { SYSTEM_LANGUAGE, useLanguage, useT } from "../../i18n/I18n";
import { PSEUDO_LOCALE } from "../../i18n/pseudo";
import { Group, Row } from "../PreferenceRows";

/** A language's name in the language of the interface, from the browser's own data. */
function languageName(tag: string, locale: string): string {
  try {
    return new Intl.DisplayNames([locale], { type: "language" }).of(tag) ?? tag;
  } catch {
    return tag;
  }
}

/**
 * The language of the interface: the system's, or one chosen here. It
 * applies at once. Dates and numbers keep the formats of the system's
 * locale. Development builds also offer the pseudo-locale, which shows
 * text the messages miss and text that gets clipped.
 */
export function LanguagePage() {
  const t = useT();
  const { locale } = useLocale();
  const { language, setLanguage } = useLanguage();
  const name = (tag: string) => (tag === PSEUDO_LOCALE ? t("preferences-language-pseudo") : languageName(tag, locale));
  const options = [
    { id: SYSTEM_LANGUAGE, label: t("preferences-language-system", { language: name(messageLanguage(locale)) }) },
    ...choosableLanguages().map((tag) => ({ id: tag, label: name(tag) })),
  ];
  return (
    <Group title={t("preferences-language-group")}>
      <Row label={t("preferences-language")} about={t("preferences-language-about")}>
        <Select
          label={t("preferences-language")}
          value={language}
          options={options}
          onChange={setLanguage}
        />
      </Row>
      <Row label={t("preferences-formats")} about={t("preferences-formats-about", { locale })} />
    </Group>
  );
}
