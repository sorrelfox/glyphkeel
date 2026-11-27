// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `cargo xtask check-bundle [DIR]`: the release UI (`ui/dist` by default)
//! names no remote address it could load.
//!
//! - Stylesheets and the HTML page may not point at any address with a
//!   scheme: no `url(https://…)`, no `src` or `href` to another origin, no
//!   preconnect or DNS prefetch hints.
//! - Scripts may contain addresses only from the hosts listed in
//!   [`KNOWN_HOSTS`]. The libraries carry some as plain text: XML namespace
//!   names, which are identifiers and never fetched, and links in error
//!   messages. Any other host fails the check, so that a new one is looked
//!   at before it ships.
//!
//! The development transport is checked by the UI's own `check:bundle`
//! script; the content security policy blocks every remote origin at run
//! time as well.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Hosts that may appear in the scripts, and why.
const KNOWN_HOSTS: &[(&str, &str)] = &[
    ("www.w3.org", "XML namespace names (SVG, XLink, MathML)"),
    (
        "www.eclipse.org",
        "namespace names of the diagram layout library's model",
    ),
    (
        "chevrotain.io",
        "links in the diagram parser's error messages",
    ),
    (
        "langium.org",
        "links in the diagram parser's error messages",
    ),
    (
        "github.com",
        "links in libraries' error messages, and the source code's address in About",
    ),
    ("react.dev", "links in React's error messages"),
    ("rolldown.rs", "a link in the bundler's error message"),
    (
        "en.wikipedia.org",
        "a link in the diagram parser's error message",
    ),
    ("example.com", "an example address in placeholder text"),
    (
        "localhost",
        "the router's fallback origin, never a request target",
    ),
    (
        "127.0.0.1",
        "the agents' local HTTP endpoint as Preferences shows it outside the app; the page never connects to it",
    ),
];

/// Text that marks a remote address: a scheme and `//`.
const SCHEMES: &[&str] = &["http://", "https://", "ws://", "wss://", "ftp://"];

pub(crate) fn run(root: &Path, args: &[String]) -> Result<bool> {
    let dir = match args {
        [] => root.join("ui/dist"),
        [dir] => PathBuf::from(dir),
        _ => anyhow::bail!("`check-bundle` takes at most one folder"),
    };
    let mut files = Vec::new();
    collect(&dir, &mut files)
        .with_context(|| format!("cannot read {}; build the UI first", dir.display()))?;
    let mut problems = Vec::new();
    let mut checked = 0;
    for file in &files {
        let Some(kind) = Kind::of(file) else {
            continue;
        };
        let text =
            fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))?;
        checked += 1;
        let shown = file
            .strip_prefix(&dir)
            .unwrap_or(file)
            .display()
            .to_string();
        problems.extend(
            check_text(kind, &text)
                .into_iter()
                .map(|problem| format!("{shown}: {problem}")),
        );
    }
    if checked == 0 {
        eprintln!(
            "check-bundle: no scripts, stylesheets or pages in {}",
            dir.display()
        );
        return Ok(false);
    }
    if problems.is_empty() {
        eprintln!("check-bundle: {checked} files, no remote address they could load");
        return Ok(true);
    }
    for problem in &problems {
        eprintln!("check-bundle: {problem}");
    }
    eprintln!(
        "check-bundle: {} problems; a new host in a library's text can be added to \
         KNOWN_HOSTS in xtask/src/bundle.rs with the reason",
        problems.len()
    );
    Ok(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Script,
    Style,
    Page,
}

impl Kind {
    fn of(path: &Path) -> Option<Self> {
        match path.extension().and_then(|e| e.to_str())? {
            "js" | "mjs" | "json" => Some(Self::Script),
            "css" => Some(Self::Style),
            "html" | "svg" => Some(Self::Page),
            _ => None,
        }
    }
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, files)?;
        } else {
            files.push(path);
        }
    }
    files.sort();
    Ok(())
}

/// The problems in one file's text.
fn check_text(kind: Kind, text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for (at, scheme) in addresses(text) {
        let rest = &text[at + scheme.len()..];
        let host: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            .collect();
        // The full stop of a sentence that ends with the address.
        let host = host.trim_end_matches('.').to_owned();
        let shown = format!("{scheme}{host}");
        match kind {
            // Namespace names are fine in a page too (an SVG's xmlns);
            // anything it could load is not.
            Kind::Page | Kind::Style => {
                let before = &text[at.saturating_sub(12)..at];
                let loads = before.contains("url(")
                    || before.contains("src=")
                    || before.contains("href=")
                    || before.contains("@import");
                if loads || host != "www.w3.org" {
                    problems.push(format!("points at {shown}"));
                }
            }
            Kind::Script => {
                if host.is_empty() {
                    // A scheme built up at run time, such as `http://` + a
                    // name: only the autolinker of the Markdown editor does
                    // that, for text the user typed.
                    continue;
                }
                if !KNOWN_HOSTS.iter().any(|(known, _)| *known == host) {
                    problems.push(format!("names the unknown host {shown}"));
                }
            }
        }
    }
    if kind == Kind::Page {
        for hint in ["preconnect", "dns-prefetch", "prefetch", "preload"] {
            let attribute = format!("rel=\"{hint}\"");
            if text.contains(&attribute) && !preload_is_local(text, &attribute) {
                problems.push(format!("has a `{hint}` hint to another origin"));
            }
        }
    }
    problems.sort();
    problems.dedup();
    problems
}

/// Whether every `<link>` with this `rel` points at the page's own files.
fn preload_is_local(text: &str, attribute: &str) -> bool {
    text.match_indices(attribute).all(|(at, _)| {
        let start = text[..at].rfind('<').unwrap_or(0);
        let end = text[at..].find('>').map_or(text.len(), |end| at + end);
        let tag = &text[start..end];
        !SCHEMES.iter().any(|scheme| tag.contains(scheme)) && !tag.contains("href=\"//")
    })
}

/// Every place a scheme starts, with the scheme.
fn addresses(text: &str) -> Vec<(usize, &'static str)> {
    let mut found: Vec<(usize, &'static str)> = SCHEMES
        .iter()
        .flat_map(|scheme| text.match_indices(scheme).map(|(at, _)| (at, *scheme)))
        .collect();
    found.sort_unstable();
    found
}

#[cfg(test)]
mod tests {
    use super::{Kind, check_text};

    #[test]
    fn scripts_may_name_only_known_hosts() {
        let text =
            "const ns=`http://www.w3.org/2000/svg`;throw Error(`see https://react.dev/errors/1`)";
        assert!(check_text(Kind::Script, text).is_empty());
        let text = "fetch(`https://cdn.example.net/lib.js`)";
        assert_eq!(
            check_text(Kind::Script, text),
            ["names the unknown host https://cdn.example.net"]
        );
        // A scheme joined to a name at run time is not an address.
        assert!(check_text(Kind::Script, "n=`http://`+t[0]").is_empty());
        assert!(!check_text(Kind::Script, "new WebSocket(`wss://evil.test`)").is_empty());
        // An address at the end of a sentence is the host without the full stop.
        assert!(check_text(Kind::Script, "`for example https://react.dev.`").is_empty());
        assert_eq!(
            check_text(Kind::Script, "`see https://cdn.example.net.`"),
            ["names the unknown host https://cdn.example.net"]
        );
    }

    #[test]
    fn stylesheets_and_pages_load_nothing_remote() {
        assert!(check_text(Kind::Style, "a{background:url(/assets/a.png)}").is_empty());
        assert!(!check_text(Kind::Style, "a{background:url(https://www.w3.org/x.png)}").is_empty());
        assert!(!check_text(Kind::Style, "@import 'https://fonts.example/x.css';").is_empty());
        let page = r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#;
        assert!(check_text(Kind::Page, page).is_empty());
        let page = r#"<script src="https://cdn.example/x.js"></script>"#;
        assert!(!check_text(Kind::Page, page).is_empty());
        let page = r#"<link rel="modulepreload" href="/assets/a.js"><link rel="preload" href="/assets/f.woff2">"#;
        assert!(check_text(Kind::Page, page).is_empty());
        let page = r#"<link rel="preconnect" href="https://fonts.example">"#;
        let problems = check_text(Kind::Page, page);
        assert!(
            problems.iter().any(|p| p.contains("preconnect")),
            "{problems:?}"
        );
    }
}
