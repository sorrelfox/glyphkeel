// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The language of the text a program shows itself: the command line's
//! help, output and errors, and the desktop app's dialogs and
//! notifications. It is one setting for the whole process, chosen by the
//! program at its start (and by the app again when the person changes it).
//!
//! It never changes data or machine text: operation outputs and the
//! English `message` of an error stay English for agents, scripts and logs;
//! only [`Messages::text`](crate::Messages::text) follows it.
//!
//! The command line takes the first of: its `--lang` option,
//! `GLYPHKEEL_LANG`, the language chosen in the app's Preferences, the
//! system's language, English. A message missing in the chosen language
//! falls back to English, one message at a time.
//!
//! `en-XA` is the pseudo-locale, as in the app: English with every letter
//! accented, vowels doubled and each message in brackets, so that text that
//! doesn't come from the message files stands out.

use std::sync::{PoisonError, RwLock};

use icu_locale_core::Locale;

/// The language every message has.
pub const SOURCE_LANGUAGE: &str = "en";

/// The pseudo-locale.
pub const PSEUDO_LOCALE: &str = "en-XA";

/// The environment variable that chooses the language of the command line.
pub const LANG_ENV: &str = "GLYPHKEEL_LANG";

/// The name of the page's preference that holds the chosen language, in the
/// app's settings file (`app.json`, under `ui`). `"system"` or missing
/// means the system's language.
pub const UI_LANGUAGE_PREF: &str = "language";

static CURRENT: RwLock<Option<String>> = RwLock::new(None);

thread_local! {
    /// A language for this thread only, while [`with_language`] runs.
    static THIS_THREAD: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The language of this process's text, as a BCP 47 tag.
#[must_use]
pub fn language() -> String {
    if let Some(tag) = THIS_THREAD.with(|t| t.borrow().clone()) {
        return tag;
    }
    CURRENT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_else(|| SOURCE_LANGUAGE.to_owned())
}

/// Runs `f` with the text of this thread in language `tag`, whatever the
/// process's language is: for tests, which run side by side.
pub fn with_language<T>(tag: &str, f: impl FnOnce() -> T) -> T {
    let tag = normalize(tag).unwrap_or_else(|| SOURCE_LANGUAGE.to_owned());
    let before = THIS_THREAD.with(|t| t.replace(Some(tag)));
    let out = f();
    THIS_THREAD.with(|t| *t.borrow_mut() = before);
    out
}

/// Chooses the language of this process's text. A tag that isn't one
/// means English.
pub fn set_language(tag: &str) {
    let tag = normalize(tag).unwrap_or_else(|| SOURCE_LANGUAGE.to_owned());
    *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = Some(tag);
}

/// Whether `tag` is the pseudo-locale.
#[must_use]
pub fn is_pseudo(tag: &str) -> bool {
    tag.eq_ignore_ascii_case(PSEUDO_LOCALE)
}

/// A language as a BCP 47 tag, from a tag (`de-CH`) or a POSIX locale
/// (`de_CH.UTF-8@euro`); `None` for `system`, the C locale, an empty value
/// and anything that isn't a language.
#[must_use]
pub fn normalize(value: &str) -> Option<String> {
    let value = value.trim();
    let base = value.split(['.', '@']).next().unwrap_or_default();
    if base.is_empty() || base.eq_ignore_ascii_case("system") || base == "C" || base == "POSIX" {
        return None;
    }
    if is_pseudo(&base.replace('_', "-")) {
        return Some(PSEUDO_LOCALE.to_owned());
    }
    let locale = Locale::try_from_str(&base.replace('_', "-")).ok()?;
    if locale.id.language.is_unknown() {
        return None;
    }
    Some(locale.to_string())
}

/// The language a translated message file is looked up by: the language
/// subtag (`de` for `de-CH`).
#[must_use]
pub fn language_subtag(tag: &str) -> &str {
    tag.split('-').next().unwrap_or(tag)
}

/// The language from, in this order: the command line's option, the
/// environment variable, the chosen language, the system's; English when
/// none of them names one.
#[must_use]
pub fn resolve(
    flag: Option<&str>,
    env: Option<&str>,
    setting: Option<&str>,
    system: Option<&str>,
) -> String {
    [flag, env, setting, system]
        .into_iter()
        .flatten()
        .find_map(normalize)
        .unwrap_or_else(|| SOURCE_LANGUAGE.to_owned())
}

/// The language chosen in the app's Preferences, from the app's settings
/// file next to the personal settings; `None` when it is the system's or
/// there is no file.
#[must_use]
pub fn ui_language_setting() -> Option<String> {
    let path = gk_store::config_dir()?.join("app.json");
    ui_language_from(&std::fs::read(path).ok()?)
}

/// The chosen language in the app's settings file `bytes`.
#[must_use]
pub fn ui_language_from(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    normalize(value.get("ui")?.get(UI_LANGUAGE_PREF)?.as_str()?)
}

/// The system's language for messages.
#[must_use]
pub fn system_language() -> Option<String> {
    #[cfg(windows)]
    if let Some(tag) = windows_ui_language() {
        return Some(tag);
    }
    system_language_from(|name| std::env::var(name).ok())
}

/// The language for messages from the environment, as gettext reads it:
/// `LANGUAGE` (its first entry), then `LC_ALL`, `LC_MESSAGES` and `LANG`.
/// The C locale set explicitly means no language, whatever `LANGUAGE`
/// says.
#[must_use]
pub fn system_language_from(var: impl Fn(&str) -> Option<String>) -> Option<String> {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| var(name).filter(|v| !v.trim().is_empty()));
    if let Some(locale) = &locale {
        let base = locale.split(['.', '@']).next().unwrap_or_default();
        if base == "C" || base == "POSIX" {
            return None;
        }
    }
    var("LANGUAGE")
        .and_then(|list| list.split(':').find_map(normalize))
        .or_else(|| locale.as_deref().and_then(normalize))
}

/// The person's display language on Windows: the first of the Desktop
/// key's preferred UI languages, else the locale of the regional settings.
#[cfg(windows)]
fn windows_ui_language() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let user = RegKey::predef(HKEY_CURRENT_USER);
    let preferred = user
        .open_subkey("Control Panel\\Desktop")
        .and_then(|key| key.get_value::<Vec<String>, _>("PreferredUILanguages"))
        .ok()
        .and_then(|list| list.iter().find_map(|tag| normalize(tag)));
    preferred.or_else(|| {
        user.open_subkey("Control Panel\\International")
            .and_then(|key| key.get_value::<String, _>("LocaleName"))
            .ok()
            .and_then(|tag| normalize(&tag))
    })
}

/// The locale formats follow: the current language, or English for the
/// pseudo-locale and a tag ICU can't read.
fn format_locale() -> Locale {
    let tag = language();
    if is_pseudo(&tag) {
        return icu_locale_core::locale!("en");
    }
    Locale::try_from_str(&tag).unwrap_or(icu_locale_core::locale!("en"))
}

/// A number written in the current language's format (`1,234.5` in
/// English, `1.234,5` in German); `text` itself when it isn't a number.
#[must_use]
pub fn format_number(text: &str) -> String {
    let Ok(decimal) = icu_decimal::input::Decimal::try_from_str(text.trim()) else {
        return text.to_owned();
    };
    match icu_decimal::DecimalFormatter::try_new((&format_locale()).into(), Default::default()) {
        Ok(formatter) => formatter.format(&decimal).to_string(),
        Err(_) => text.to_owned(),
    }
}

/// A date (`2026-11-27`, or the date part of a timestamp) written in the
/// current language's medium format (`Nov 27, 2026` in English); `text`
/// itself when it isn't a date.
#[must_use]
pub fn format_date(text: &str) -> String {
    let parse = || -> Option<icu_calendar::Date<icu_calendar::Gregorian>> {
        let day = text.get(..10)?;
        let mut parts = day.split('-');
        let year: i32 = parts.next()?.parse().ok()?;
        let month: u8 = parts.next()?.parse().ok()?;
        let date: u8 = parts.next()?.parse().ok()?;
        if text.len() > 10 && !text[10..].starts_with(['T', ' ']) {
            return None;
        }
        icu_calendar::Date::try_new_gregorian(year, month, date).ok()
    };
    let Some(date) = parse() else {
        return text.to_owned();
    };
    let formatter =
        icu_datetime::FixedCalendarDateTimeFormatter::<icu_calendar::Gregorian, _>::try_new(
            (&format_locale()).into(),
            icu_datetime::fieldsets::YMD::medium(),
        );
    match formatter {
        Ok(f) => f.format(&date).to_string(),
        Err(_) => text.to_owned(),
    }
}

/// The plural category of the number `text` in the current language
/// (`one`, `few`, `other`…), for choosing a message's variant; `None` when
/// it isn't a number.
#[must_use]
pub fn plural_category(text: &str) -> Option<&'static str> {
    use icu_plurals::{PluralCategory, PluralRules};
    let decimal = icu_decimal::input::Decimal::try_from_str(text.trim()).ok()?;
    let rules = PluralRules::try_new_cardinal((&format_locale()).into()).ok()?;
    Some(match rules.category_for(&decimal) {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    })
}

/// Text in the pseudo-locale: every letter accented and every vowel
/// doubled, as the app's pseudo-locale does it.
#[must_use]
pub fn pseudo_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        let accented = accent(c);
        out.push(accented);
        if "aeiouyAEIOUY".contains(c) {
            out.extend(accented.to_lowercase());
        }
    }
    out
}

fn accent(c: char) -> char {
    const FROM: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const TO: [char; 52] = [
        'á', 'ƀ', 'ç', 'đ', 'é', 'ƒ', 'ĝ', 'ĥ', 'í', 'ĵ', 'ķ', 'ĺ', 'ɱ', 'ñ', 'ó', 'þ', 'ǫ', 'ŕ',
        'š', 'ţ', 'ú', 'ṽ', 'ŵ', 'ẋ', 'ý', 'ž', 'Á', 'Ɓ', 'Ç', 'Đ', 'É', 'Ƒ', 'Ĝ', 'Ĥ', 'Í', 'Ĵ',
        'Ķ', 'Ĺ', 'Ṁ', 'Ñ', 'Ó', 'Þ', 'Ǫ', 'Ŕ', 'Š', 'Ţ', 'Ú', 'Ṽ', 'Ŵ', 'Ẋ', 'Ý', 'Ž',
    ];
    FROM.find(c).map_or(c, |i| TO[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_and_posix_locales_normalize() {
        assert_eq!(normalize("de_DE.UTF-8@euro").as_deref(), Some("de-DE"));
        assert_eq!(normalize("pt-br").as_deref(), Some("pt-BR"));
        assert_eq!(normalize("en-xa").as_deref(), Some(PSEUDO_LOCALE));
        assert_eq!(normalize("ja").as_deref(), Some("ja"));
        for none in [
            "",
            " ",
            "system",
            "C",
            "POSIX",
            "C.UTF-8",
            "not a tag",
            "und",
        ] {
            assert_eq!(normalize(none), None, "{none:?}");
        }
    }

    #[test]
    fn the_first_source_that_names_a_language_wins() {
        assert_eq!(
            resolve(Some("fr"), Some("de"), Some("it"), Some("es")),
            "fr"
        );
        assert_eq!(resolve(None, Some("de"), Some("it"), Some("es")), "de");
        assert_eq!(
            resolve(Some(""), Some("system"), Some("it"), Some("es")),
            "it"
        );
        assert_eq!(resolve(None, None, None, Some("es_ES.UTF-8")), "es-ES");
        assert_eq!(resolve(None, Some("nonsense!"), None, None), "en");
    }

    #[test]
    fn the_environment_is_read_as_gettext_does() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        assert_eq!(
            system_language_from(env(&[("LANGUAGE", "sv:de"), ("LANG", "fr_FR.UTF-8")])).as_deref(),
            Some("sv")
        );
        assert_eq!(
            system_language_from(env(&[("LC_MESSAGES", "nl_NL"), ("LANG", "fr_FR")])).as_deref(),
            Some("nl-NL")
        );
        assert_eq!(
            system_language_from(env(&[("LC_ALL", "de_AT.UTF-8"), ("LC_MESSAGES", "nl_NL")]))
                .as_deref(),
            Some("de-AT")
        );
        assert_eq!(
            system_language_from(env(&[("LANGUAGE", "sv"), ("LC_ALL", "C")])),
            None
        );
        assert_eq!(system_language_from(env(&[])), None);
    }

    #[test]
    fn the_apps_choice_is_read_from_its_settings() {
        assert_eq!(
            ui_language_from(br#"{"ui": {"language": "de"}}"#).as_deref(),
            Some("de")
        );
        assert_eq!(ui_language_from(br#"{"ui": {"language": "system"}}"#), None);
        assert_eq!(ui_language_from(br#"{"notifications": true}"#), None);
        assert_eq!(ui_language_from(b"not json"), None);
    }

    #[test]
    fn numbers_dates_and_plurals_follow_the_language() {
        with_language("de-DE", || {
            assert_eq!(format_number("1234.5"), "1.234,5");
            assert_eq!(format_date("2026-11-27"), "27.11.2026");
        });
        with_language("pl", || assert_eq!(plural_category("3"), Some("few")));
        with_language("en", || {
            assert_eq!(format_number("1234"), "1,234");
            assert_eq!(format_number("TM-41"), "TM-41");
            assert_eq!(format_date("2026-11-27T10:00:00Z"), "Nov 27, 2026");
            assert_eq!(format_date("soon"), "soon");
            assert_eq!(plural_category("1"), Some("one"));
        });
        with_language(PSEUDO_LOCALE, || {
            assert_eq!(language(), PSEUDO_LOCALE);
            assert_eq!(format_number("1234"), "1,234");
        });
        with_language("nonsense!", || assert_eq!(language(), "en"));
    }

    #[test]
    fn pseudo_text_matches_the_apps() {
        assert_eq!(pseudo_text("Today"), "Ţóóđááýý");
        assert_eq!(pseudo_text("Ctrl + 1"), "Çţŕĺ + 1");
    }
}
