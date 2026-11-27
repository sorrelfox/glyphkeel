// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The app's own text shown by Rust (native dialogs, file choosers), by
//! message ID. The English text lives in `i18n/en/app.ftl`, in Fluent's
//! syntax, so that it is translated with the page's messages.

/// The English messages.
const MESSAGES_FTL: &str = include_str!("../i18n/en/app.ftl");

/// The app's messages, in English and the languages they are translated to.
const MESSAGES: gk_engine::Messages = gk_engine::Messages::new(MESSAGES_FTL);

/// Message `id` in the app's language (the one chosen in Preferences, else
/// the system's; see [`follow_preferences`]) with its placeables filled in:
/// for what Rust shows itself, such as native dialogs, notifications and
/// the tray menu. The ID itself when there is no such message.
pub(crate) fn text(id: &str, args: &[(&str, String)]) -> String {
    MESSAGES.text(id, args)
}

/// Message `id` in English: the `message` of an error, which the page
/// shows in its own language by the error's message ID.
fn english(id: &str, args: &[(&str, String)]) -> String {
    MESSAGES.english(id, args)
}

/// The language of the app's own text for the page's preferences `ui`:
/// the language chosen in Preferences, else the system's, else English;
/// the same as the page's, so that a dialog never speaks another language
/// than the window behind it.
pub(crate) fn language_for(
    ui: &serde_json::Map<String, serde_json::Value>,
    system: Option<&str>,
) -> String {
    let chosen = ui
        .get(gk_engine::language::UI_LANGUAGE_PREF)
        .and_then(serde_json::Value::as_str);
    gk_engine::language::resolve(None, None, chosen, system)
}

/// Makes the app's own text follow the page's preferences `ui`: at the
/// start, and again whenever the page changes its language.
pub(crate) fn follow_preferences(ui: &serde_json::Map<String, serde_json::Value>) {
    let system = gk_engine::language::system_language();
    gk_engine::language::set_language(&language_for(ui, system.as_deref()));
}

fn args_value(args: &[(&str, String)]) -> serde_json::Value {
    serde_json::Value::Object(
        args.iter()
            .map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(v.clone())))
            .collect(),
    )
}

/// The error `make` builds from message `id`'s English text, with the ID
/// and its arguments (`message_id`, `args`) added, so that the page shows
/// it in the person's language: `error("link-too-long", &[], |m|
/// OpError::limit(m, max))`.
pub(crate) fn error(
    id: &str,
    args: &[(&str, String)],
    make: impl FnOnce(String) -> gk_api::OpError,
) -> gk_api::OpError {
    let mut error = make(english(id, args)).with_detail("message_id", id);
    if !args.is_empty() {
        error = error.with_detail("args", args_value(args));
    }
    error
}

/// Sets `error`'s hint to message `id`, with its ID and arguments
/// (`hint_id`, `hint_args`).
pub(crate) fn hint(error: gk_api::OpError, id: &str, args: &[(&str, String)]) -> gk_api::OpError {
    let mut error = error
        .with_hint(english(id, args))
        .with_detail("hint_id", id);
    if !args.is_empty() {
        error = error.with_detail("hint_args", args_value(args));
    }
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_renders_without_syntax_left_over() {
        let ids: Vec<&str> = MESSAGES_FTL
            .lines()
            .filter(|l| !l.starts_with(['#', ' ']) && l.contains(" = "))
            .filter_map(|l| l.split_once(" = ").map(|(id, _)| id))
            .collect();
        assert!(ids.len() > 10);
        for id in ids {
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
                "{id}"
            );
            let args = [
                ("count", "2".to_owned()),
                ("name", "daily-2026-11-24".to_owned()),
                ("summary", "Something.".to_owned()),
            ];
            let shown = english(id, &args);
            assert!(!shown.trim().is_empty(), "{id}");
            assert!(!shown.contains(['{', '}', '[']), "{id}: {shown}");
        }
    }

    /// The Rust sources of the app, without their tests.
    fn sources(dir: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("source folder") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && path.file_name().is_some_and(|n| n != "tests.rs")
            {
                out.push(std::fs::read_to_string(&path).expect("source file"));
            }
        }
    }

    #[test]
    fn every_message_the_app_names_exists_and_every_message_is_named() {
        let mut texts = Vec::new();
        sources(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut texts,
        );
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        // Window labels and their prefixes, not messages.
        let labels = ["quick-add", "task-", "wiki-"];
        let (missing, unused) = MESSAGES.check_usage(&refs, &labels);
        assert!(
            missing.is_empty(),
            "named in the code but not in app.ftl: {missing:?}"
        );
        assert!(
            unused.is_empty(),
            "in app.ftl but named nowhere: {unused:?}"
        );
    }

    #[test]
    fn plurals_and_placeables_fill_in() {
        assert_eq!(
            text("confirm-purge-items", &[("count", "1".into())]),
            "Delete one item in the trash for good? Its text is removed from the history too."
        );
        assert!(
            text("confirm-purge-items", &[("count", "3".into())]).starts_with("Delete 3 items")
        );
        assert!(
            text("confirm-backup-delete", &[("name", "manual-1".into())]).contains("“manual-1”")
        );
        assert_eq!(text("no-such-message", &[]), "no-such-message");
    }

    #[test]
    fn the_language_follows_preferences_then_the_system() {
        let mut ui = serde_json::Map::new();
        assert_eq!(language_for(&ui, None), "en");
        assert_eq!(language_for(&ui, Some("de_DE.UTF-8")), "de-DE");
        ui.insert("language".into(), "system".into());
        assert_eq!(language_for(&ui, Some("fr")), "fr");
        ui.insert("language".into(), "en-XA".into());
        assert_eq!(language_for(&ui, Some("fr")), "en-XA");
    }

    #[test]
    fn shown_text_follows_the_language_and_errors_stay_english() {
        gk_engine::language::with_language("en-XA", || {
            let shown = text("lock-notice-title", &[]);
            assert!(shown.starts_with('[') && shown.ends_with(']'), "{shown}");
            assert_ne!(shown, english("lock-notice-title", &[]));
            let e = error("preferences-too-many", &[], gk_api::OpError::invalid);
            assert_eq!(e.message, english("preferences-too-many", &[]));
            assert_eq!(e.details["message_id"], "preferences-too-many");
        });
    }
}
