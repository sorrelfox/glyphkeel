// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Spelling for the Markdown editor, through `spellbook` (a Rust port of
//! Hunspell) and the system's Hunspell dictionaries.
//!
//! The page sends the visible prose, with code, links, math, task keys and
//! wiki links already blanked out, and gets back the misspelt words as
//! UTF-16 ranges, the positions the editor uses. The web view's own spell
//! checker stays off: it marks loaded text only where the caret has been.
//!
//! The dictionary is not shipped with the app. It is the system's `en_US`
//! Hunspell dictionary (on Fedora the `hunspell-en-US` package, on Debian and
//! Ubuntu `hunspell-en-us`), looked up in the usual folders. Without one,
//! nothing is marked.
//!
//! On Windows there are no system Hunspell dictionaries, and WebView2's own
//! checker uses the system's spelling for every language the person has
//! installed. There the answer says so (`webView`), nothing is looked up, and
//! the page turns the web view's checker on instead.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use spellbook::Dictionary;

/// The dictionary the editor checks with.
const LANGUAGE: &str = "en_US";

/// The most text one call may check, in bytes: the visible part of a page is
/// far smaller.
const MAX_TEXT: usize = 1 << 20;

/// The most suggestions one word gets.
const MAX_SUGGESTIONS: usize = 8;

static DICTIONARY: OnceLock<Option<Dictionary>> = OnceLock::new();

/// The folders that hold Hunspell dictionaries, most specific first.
fn dictionary_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    for dir in data_dirs.split(':').filter(|dir| !dir.is_empty()) {
        folders.push(Path::new(dir).join("hunspell"));
        folders.push(Path::new(dir).join("myspell"));
        folders.push(Path::new(dir).join("myspell/dicts"));
    }
    for fixed in [
        "/usr/share/hunspell",
        "/usr/share/myspell",
        "/usr/share/myspell/dicts",
        // Flatpak: dictionaries from the runtime's extension.
        "/app/share/hunspell",
        "/usr/share/runtime/share/hunspell",
    ] {
        let fixed = PathBuf::from(fixed);
        if !folders.contains(&fixed) {
            folders.push(fixed);
        }
    }
    folders
}

/// Reads a language's dictionary from the first folder that has both of its
/// files.
fn find_dictionary(folders: &[PathBuf], language: &str) -> Option<Dictionary> {
    for folder in folders {
        let aff = folder.join(format!("{language}.aff"));
        let dic = folder.join(format!("{language}.dic"));
        let (Ok(aff), Ok(dic)) = (std::fs::read_to_string(&aff), std::fs::read_to_string(&dic))
        else {
            continue;
        };
        match Dictionary::new(&aff, &dic) {
            Ok(dictionary) => return Some(dictionary),
            Err(err) => eprintln!(
                "glyphkeel-app: the spelling dictionary in {} can't be read: {err}",
                folder.display()
            ),
        }
    }
    None
}

/// The dictionary, read on first use.
fn dictionary() -> Option<&'static Dictionary> {
    DICTIONARY
        .get_or_init(|| {
            let found = find_dictionary(&dictionary_folders(), LANGUAGE);
            if found.is_none() {
                eprintln!(
                    "glyphkeel-app: no {LANGUAGE} Hunspell dictionary found; spelling is off"
                );
            }
            found
        })
        .as_ref()
}

/// The answer to `spell_check`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct SpellCheck {
    /// Misspelt words as `[from, to]` in UTF-16 positions.
    misspelt: Vec<(u32, u32)>,
    /// Whether a dictionary was found; without one nothing is marked.
    available: bool,
    /// The web view checks spelling itself (on Windows): the page turns its
    /// checker on and sends nothing more.
    #[serde(rename = "webView", skip_serializing_if = "std::ops::Not::not")]
    web_view: bool,
}

/// Whether the web view's own checker does the work on this platform.
const WEB_VIEW_CHECKS: bool = cfg!(windows);

/// The answer where the web view checks spelling itself.
fn left_to_the_web_view() -> SpellCheck {
    SpellCheck {
        misspelt: Vec::new(),
        available: false,
        web_view: true,
    }
}

/// Whether a character belongs to a word.
fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || c == '\'' || c == '\u{2019}'
}

/// The words of a text, as `(word, from, to)` with UTF-16 positions counted
/// from `start`. Typographic apostrophes become plain ones, and apostrophes at
/// the ends of a word are left out. Words with digits are skipped.
fn words(start: u32, text: &str) -> Vec<(String, u32, u32)> {
    let mut found = Vec::new();
    let mut pos = start;
    let mut word = String::new();
    let mut word_start = start;
    let mut push = |word: &mut String, from: u32, to: u32| {
        let lead = word.chars().take_while(|&c| c == '\'').count() as u32;
        let trail = word.chars().rev().take_while(|&c| c == '\'').count() as u32;
        let trimmed = word.trim_matches('\'');
        if !trimmed.is_empty() {
            found.push((trimmed.to_owned(), from + lead, to - trail));
        }
        word.clear();
    };
    let mut skip_word = false;
    for c in text.chars() {
        if is_word_char(c) {
            if word.is_empty() && !skip_word {
                word_start = pos;
            }
            if !skip_word {
                word.push(if c == '\u{2019}' { '\'' } else { c });
            }
        } else if c.is_ascii_digit() {
            // "abc123" is not a word to check.
            word.clear();
            skip_word = true;
        } else {
            if !word.is_empty() {
                push(&mut word, word_start, pos);
            }
            skip_word = false;
        }
        pos += c.len_utf16() as u32;
    }
    if !word.is_empty() {
        push(&mut word, word_start, pos);
    }
    found
}

/// The misspelt words of the chunks.
fn check_with(dictionary: &Dictionary, chunks: &[(u32, String)]) -> Vec<(u32, u32)> {
    let mut misspelt = Vec::new();
    for (start, text) in chunks {
        for (word, from, to) in words(*start, text) {
            if !dictionary.check(&word) {
                misspelt.push((from, to));
            }
        }
    }
    misspelt
}

fn suggest_with(dictionary: &Dictionary, word: &str) -> Vec<String> {
    let mut suggestions = Vec::new();
    dictionary.suggest(word, &mut suggestions);
    suggestions.truncate(MAX_SUGGESTIONS);
    suggestions
}

/// Checks prose: each chunk is a start position and its text.
#[tauri::command]
pub(crate) async fn spell_check(chunks: Vec<(u32, String)>) -> Result<SpellCheck, String> {
    if chunks.iter().map(|(_, text)| text.len()).sum::<usize>() > MAX_TEXT {
        return Err("too much text to check at once".to_owned());
    }
    if WEB_VIEW_CHECKS {
        return Ok(left_to_the_web_view());
    }
    // Reading the dictionary takes a moment the first time; never on the
    // main thread.
    tauri::async_runtime::spawn_blocking(move || match dictionary() {
        Some(dictionary) => SpellCheck {
            misspelt: check_with(dictionary, &chunks),
            available: true,
            web_view: false,
        },
        None => SpellCheck {
            misspelt: Vec::new(),
            available: false,
            web_view: false,
        },
    })
    .await
    .map_err(|err| err.to_string())
}

/// Suggestions for one word, best first.
#[tauri::command]
pub(crate) async fn spell_suggest(word: String) -> Result<Vec<String>, String> {
    if word.len() > 200 || WEB_VIEW_CHECKS {
        return Ok(Vec::new());
    }
    tauri::async_runtime::spawn_blocking(move || {
        dictionary()
            .map(|dictionary| suggest_with(dictionary, &word))
            .unwrap_or_default()
    })
    .await
    .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny dictionary in Hunspell's format, so the tests need no system
    /// dictionary.
    fn small() -> Dictionary {
        let aff = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzESIANRTOLCDUGMPHBYFVKWZ'\nREP 1\nREP ie ei\nSFX S Y 1\nSFX S 0 s .\n";
        let dic = "6\nreceive\nworld\nword/S\ndon't\nGrüße\nthe\n";
        Dictionary::new(aff, dic).expect("the test dictionary parses")
    }

    #[test]
    fn words_have_utf16_positions() {
        // "😀" is two UTF-16 code units.
        let found = words(10, "😀 Grüße, don’t 'quoted' abc123 x");
        let got: Vec<_> = found.iter().map(|(w, a, b)| (w.as_str(), *a, *b)).collect();
        assert_eq!(
            got,
            vec![
                ("Grüße", 13, 18),
                ("don't", 20, 25),
                ("quoted", 27, 33),
                ("x", 42, 43)
            ]
        );
    }

    #[test]
    fn marks_only_misspelt_words() {
        let dictionary = small();
        let chunks = vec![
            (0, "the world recieve words".to_owned()),
            (100, "teh don’t Grüße".to_owned()),
        ];
        assert_eq!(check_with(&dictionary, &chunks), vec![(10, 17), (100, 103)]);
    }

    #[test]
    fn the_answer_says_when_the_web_view_checks_itself() {
        let json = serde_json::to_value(left_to_the_web_view()).expect("serializes");
        assert_eq!(
            json,
            serde_json::json!({"misspelt": [], "available": false, "webView": true})
        );
        let own = SpellCheck {
            misspelt: vec![(0, 3)],
            available: true,
            web_view: false,
        };
        assert_eq!(
            serde_json::to_value(own).expect("serializes"),
            serde_json::json!({"misspelt": [[0, 3]], "available": true})
        );
    }

    #[test]
    fn suggests_corrections() {
        let dictionary = small();
        let suggestions = suggest_with(&dictionary, "recieve");
        assert_eq!(suggestions.first().map(String::as_str), Some("receive"));
    }

    #[test]
    fn finds_a_dictionary_in_the_first_folder_that_has_it() {
        let empty = tempfile::tempdir().expect("a temporary folder");
        let full = tempfile::tempdir().expect("a temporary folder");
        std::fs::write(full.path().join("xx_XX.aff"), "SET UTF-8\n").expect("written");
        std::fs::write(full.path().join("xx_XX.dic"), "1\nhello\n").expect("written");
        let folders = vec![empty.path().to_owned(), full.path().to_owned()];
        let dictionary = find_dictionary(&folders, "xx_XX").expect("found");
        assert!(dictionary.check("hello"));
        assert!(find_dictionary(&folders, "yy_YY").is_none());
    }

    #[test]
    fn looks_in_the_system_folders() {
        let folders = dictionary_folders();
        assert!(folders.contains(&PathBuf::from("/usr/share/hunspell")));
        assert!(folders.contains(&PathBuf::from("/app/share/hunspell")));
    }

    /// Needs the system's `en_US` dictionary: `cargo test -p gk-app spell -- --ignored`.
    #[test]
    #[ignore = "needs the system's en_US Hunspell dictionary"]
    fn checks_with_the_system_dictionary() {
        let dictionary = dictionary().expect("an en_US dictionary is installed");
        let chunks = vec![(0, "Teh quick brown fox doesn’t recieve emails".to_owned())];
        assert_eq!(check_with(dictionary, &chunks), vec![(0, 3), (28, 35)]);
        assert!(suggest_with(dictionary, "recieve").contains(&"receive".to_owned()));
    }
}
