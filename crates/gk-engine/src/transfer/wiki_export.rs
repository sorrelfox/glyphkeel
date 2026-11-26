// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Exporting one wiki page, optionally with the pages inside it, or a
//! project's whole wiki, as plain Markdown files or as web pages that read
//! well outside the app.
//!
//! ```text
//! <slug>.md                   the page's current text (`.html`: the page rendered)
//! <slug>/<child-slug>.md      the pages inside it (with `subpages`), the tree as folders
//! files/<file name>           the files the pages hold or link to
//! index.html                  web pages only, when there are several: the tree of pages
//! ```
//!
//! Web pages are rendered by the same Markdown renderer as the wiki's
//! reader, with a small style sheet of their own, and need nothing else:
//! no script, no fonts, nothing from the network except images the text
//! itself takes from the web.
//!
//! Links between the exported pages (`[[Page]]`, `[[Page#Heading|label]]`)
//! become relative Markdown links, a heading as a GitHub-style anchor;
//! links to pages that aren't exported stay as written. Links to attachments
//! (`gk-file://<id>`) point at the copied file. Only attachments of the
//! exported pages, or of the same project, are copied.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use gk_api::ops::transfer::ArchiveFormat;
use gk_api::ops::wiki::{WikiExportInput, WikiExportResult, WikiTextFormat};
use gk_api::{ErrorCode, OpError};
use gk_core::Id;
use gk_store::rusqlite::{OptionalExtension, Transaction, params};

use super::resolve;
use super::sink::Sink;
use crate::context::Context;
use crate::engine::Engine;
use crate::failure::Failure;
use crate::messages;
use crate::ops::description::{self, Owner};
use crate::ops::lookup::{self, b};
use crate::ops::wiki::{self, PageRow};

/// How deep the export follows the tree; pages nest less deep than this.
const MAX_LEVELS: usize = 16;
/// The folder attached files go to.
const FILES_DIR: &str = "files";

/// One page to write.
pub(super) struct Page {
    /// Its path in the export, `a/b.md`.
    pub(super) path: String,
    pub(super) body: String,
}

/// One attached file to copy.
struct File {
    path: String,
    sha256: String,
}

impl Engine {
    /// Exports a wiki page, with `subpages` the pages inside it, or without
    /// a page the project's whole wiki, as Markdown files with their
    /// attachments.
    pub fn export_wiki_page(
        &self,
        ctx: &Context,
        input: &WikiExportInput,
    ) -> Result<WikiExportResult, OpError> {
        let format = input.format.unwrap_or_else(|| match &input.path {
            Some(p) if !p.ends_with(".zip") && !ctx.is_agent() => ArchiveFormat::Folder,
            _ => ArchiveFormat::Zip,
        });
        let (pages, files, slug) = self
            .store()
            .read(|tx| {
                let _scope = crate::scope::enter(tx, ctx)?;
                read(tx, input)
            })
            .map_err(OpError::from)?;
        let target = match &input.path {
            Some(p) => resolve(self, ctx, p, "path")?,
            None => {
                let stamp = crate::time::rfc3339(self.now_ms())
                    .replace([':', '.'], "")
                    .replace('-', "");
                let name = format!("{slug}-{}", &stamp[..15]);
                let name = match format {
                    ArchiveFormat::Zip => format!("{name}.zip"),
                    ArchiveFormat::Folder => name,
                };
                self.store().layout().exports_dir().join(name)
            }
        };
        write(self, &target, format, &pages, &files)
    }
}

fn write(
    engine: &Engine,
    target: &Path,
    format: ArchiveFormat,
    pages: &[Page],
    files: &[File],
) -> Result<WikiExportResult, OpError> {
    let cant_write = |e: std::io::Error| {
        messages::error(
            ErrorCode::Storage,
            "transfer-write-failed",
            &[
                ("path", target.display().to_string()),
                ("error", e.to_string()),
            ],
        )
    };
    let mut sink = Sink::create(target, format).map_err(cant_write)?;
    let put = |sink: &mut Sink| -> Result<(), OpError> {
        for page in pages {
            sink.put(&page.path, page.body.as_bytes())
                .map_err(cant_write)?;
        }
        let layout = engine.store().layout();
        for file in files {
            let source = gk_store::file_path(layout, &file.sha256).ok_or_else(|| {
                messages::error(
                    ErrorCode::Storage,
                    "transfer-stored-file-missing",
                    &[("sha256", file.sha256.clone())],
                )
            })?;
            sink.put_file(&file.path, &file.sha256, &source)
                .map_err(cant_write)?;
        }
        Ok(())
    };
    if let Err(e) = put(&mut sink) {
        sink.abandon();
        return Err(e);
    }
    let size = sink.finish().map_err(cant_write)?;
    let index = pages
        .iter()
        .filter(|p| p.path == super::wiki_html::INDEX)
        .count();
    Ok(WikiExportResult {
        path: target.to_string_lossy().into_owned(),
        format,
        pages: (pages.len() - index) as u64,
        attachments: files.len() as u64,
        size,
    })
}

/// The pages to write with their texts rewritten, the files to copy, and
/// a name for the export: the page's slug, or the project's key.
fn read(
    tx: &Transaction<'_>,
    input: &WikiExportInput,
) -> Result<(Vec<Page>, Vec<File>, String), Failure> {
    let html = input.text_format == Some(WikiTextFormat::Html);
    let ext = if html { ".html" } else { ".md" };
    let (roots, project, name) = match &input.page {
        Some(page) => {
            let root = wiki::find_page(tx, input.project.as_deref(), "page", page)?;
            let project = lookup::project_by_id(tx, root.project)?
                .ok_or_else(|| Failure::Op(messages::not_found("wiki-page-gone", &[])))?;
            let name = root.slug.clone();
            (vec![root], project, name)
        }
        None => {
            let Some(key) = input.project.as_deref() else {
                return Err(messages::invalid("project", "wiki-export-what", &[]));
            };
            let project = lookup::project(tx, "project", key)?;
            let roots = wiki::children(tx, project.id, None)?;
            let name = format!("{}-wiki", project.key.to_lowercase());
            (roots, project, name)
        }
    };
    let subpages = input.subpages || input.page.is_none();

    // The pages, the tree as folders: a page's children go in a folder
    // named like its file.
    let mut used = BTreeSet::new();
    if html {
        used.insert(super::wiki_html::INDEX.to_owned());
    }
    let mut rows: Vec<(PageRow, String)> = Vec::new();
    let mut levels: Vec<usize> = Vec::new();
    let mut queue = Vec::new();
    for root in roots.into_iter().rev() {
        let path = free_path(&mut used, "", &root.slug, ext);
        queue.push((root, path, 0usize));
    }
    while let Some((page, path, level)) = queue.pop() {
        if subpages && level < MAX_LEVELS {
            let folder = path.trim_end_matches(ext).to_owned();
            let kids = wiki::children(tx, page.project, Some(page.id))?;
            for kid in kids.into_iter().rev() {
                let kid_path = free_path(&mut used, &format!("{folder}/"), &kid.slug, ext);
                queue.push((kid, kid_path, level + 1));
            }
        }
        rows.push((page, path));
        levels.push(level);
    }
    let by_id: HashMap<Id, String> = rows.iter().map(|(p, path)| (p.id, path.clone())).collect();

    // The files: those the pages hold, then those they link to in the
    // same project.
    let mut files: Vec<File> = Vec::new();
    let mut file_paths: HashMap<Id, String> = HashMap::new();
    for (page, _) in &rows {
        let mut stmt = tx.prepare_cached(
            "SELECT id, file_name, sha256 FROM attachments
              WHERE owner_kind = 'wiki_page' AND owner_id = ?1 AND deleted_at IS NULL
              ORDER BY created_at, id",
        )?;
        let held = stmt
            .query_map([b(page.id)], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?, r.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, name, sha) in held {
            if let Ok(id) = Id::from_slice(&id) {
                add_file(&mut files, &mut file_paths, &mut used, id, &name, sha);
            }
        }
    }
    let mut texts = Vec::new();
    for (page, path) in &rows {
        let (body, _) = description::current_text(tx, Owner::Wiki(page.id))?;
        for id in attachment_ids(&body) {
            if file_paths.contains_key(&id) {
                continue;
            }
            if let Some((name, sha)) = project_attachment(tx, id, project.id)? {
                add_file(&mut files, &mut file_paths, &mut used, id, &name, sha);
            }
        }
        texts.push((page.id, path.clone(), body));
    }

    let mut pages = Vec::new();
    if html {
        let titles: HashMap<Id, String> =
            rows.iter().map(|(p, _)| (p.id, p.title.clone())).collect();
        let several = rows.len() > 1;
        for (id, path, body) in texts {
            let resolve_page = |link: &str| -> Option<String> {
                let (page, heading) = match link.split_once('#') {
                    Some((p, h)) => (p.trim(), Some(h.trim())),
                    None => (link.trim(), None),
                };
                let target = if page.is_empty() {
                    Some(id)
                } else {
                    wiki::find_page(tx, Some(&project.key), "page", page)
                        .ok()
                        .map(|p| p.id)
                };
                let target_path = target.and_then(|t| by_id.get(&t))?;
                let mut url = if target == Some(id) {
                    String::new()
                } else {
                    encode(&relative(&path, target_path))
                };
                if let Some(h) = heading {
                    url.push('#');
                    url.push_str(&anchor(h));
                }
                Some(url)
            };
            let resolve_file =
                |file: Id| file_paths.get(&file).map(|to| encode(&relative(&path, to)));
            let page_row = rows.iter().find(|(p, _)| p.id == id).map(|(p, _)| p);
            let trail: Vec<(String, String)> = page_row
                .map(|p| wiki::ancestor_ids(tx, p.parent))
                .transpose()?
                .unwrap_or_default()
                .into_iter()
                .filter_map(|a| {
                    let to = by_id.get(&a)?;
                    Some((titles.get(&a)?.clone(), encode(&relative(&path, to))))
                })
                .collect();
            let index = several.then(|| encode(&relative(&path, super::wiki_html::INDEX)));
            let title = titles.get(&id).cloned().unwrap_or_default();
            let text = super::wiki_html::page(
                &title,
                &id.to_string(),
                &body,
                &trail,
                index.as_deref(),
                &resolve_page,
                &resolve_file,
            );
            pages.push(Page { path, body: text });
        }
        if several {
            let entries: Vec<(usize, String, String)> = rows
                .iter()
                .zip(&levels)
                .map(|((p, path), level)| (*level, p.title.clone(), encode(path)))
                .collect();
            let heading = match &input.page {
                Some(_) => rows
                    .first()
                    .map(|(p, _)| p.title.clone())
                    .unwrap_or_default(),
                None => tx.query_row(
                    "SELECT name FROM projects WHERE id = ?1",
                    [b(project.id)],
                    |r| r.get(0),
                )?,
            };
            pages.push(Page {
                path: super::wiki_html::INDEX.to_owned(),
                body: super::wiki_html::index(&heading, &entries),
            });
        }
    } else {
        for (id, path, body) in texts {
            let text = rewrite(tx, &project.key, id, &path, &body, &by_id, &file_paths)?;
            pages.push(Page { path, body: text });
        }
    }
    Ok((pages, files, name))
}

/// Adds attachment `id` to the files to copy, under a free name in the
/// files folder, unless it is there already.
fn add_file(
    files: &mut Vec<File>,
    paths: &mut HashMap<Id, String>,
    used: &mut BTreeSet<String>,
    id: Id,
    name: &str,
    sha256: String,
) {
    if paths.contains_key(&id) {
        return;
    }
    let (stem, ext) = split_name(&safe_name(name));
    let path = free_path(used, &format!("{FILES_DIR}/"), &stem, &ext);
    paths.insert(id, path.clone());
    files.push(File { path, sha256 });
}

/// The page's text with links to exported pages and to the copied files
/// made relative to `path`.
fn rewrite(
    tx: &Transaction<'_>,
    project: &str,
    id: Id,
    path: &str,
    body: &str,
    pages: &HashMap<Id, String>,
    files: &HashMap<Id, String>,
) -> Result<String, Failure> {
    let mut out = String::with_capacity(body.len());
    let mut at = 0;
    for span in gk_markdown::wiki_link_spans(body) {
        let target = if span.link.page.is_empty() {
            Some(id)
        } else {
            wiki::find_page(tx, Some(project), "page", &span.link.page)
                .ok()
                .map(|p| p.id)
        };
        let Some(target_path) = target.and_then(|t| pages.get(&t)) else {
            continue;
        };
        let mut url = if target == Some(id) {
            String::new()
        } else {
            encode(&relative(path, target_path))
        };
        if let Some(heading) = &span.link.heading {
            url.push('#');
            url.push_str(&anchor(heading));
        }
        let text = span.label.clone().unwrap_or_else(|| {
            let shown = match (&span.link.heading, span.link.page.is_empty()) {
                (Some(h), true) => h.clone(),
                (Some(h), false) => format!("{} § {h}", span.link.page),
                (None, _) => span.link.page.clone(),
            };
            escape_label(&shown)
        });
        out.push_str(&body[at..span.range.start]);
        out.push('[');
        out.push_str(&text);
        out.push_str("](");
        out.push_str(&url);
        out.push(')');
        at = span.range.end;
    }
    out.push_str(&body[at..]);
    for (file, file_path) in files {
        let from = format!("gk-file://{file}");
        if out.contains(&from) {
            out = out.replace(&from, &encode(&relative(path, file_path)));
        }
    }
    Ok(out)
}

/// The attachment IDs a text links to, as `gk-file://<id>`.
fn attachment_ids(text: &str) -> Vec<Id> {
    let mut out = Vec::new();
    for (start, _) in text.match_indices("gk-file://") {
        let rest = &text[start + "gk-file://".len()..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .unwrap_or(rest.len());
        if let Ok(id) = rest[..end].parse::<Id>()
            && !out.contains(&id)
        {
            out.push(id);
        }
    }
    out
}

/// The name and content of live attachment `id` when it belongs to
/// something of `project`.
fn project_attachment(
    tx: &Transaction<'_>,
    id: Id,
    project: Id,
) -> Result<Option<(String, String)>, Failure> {
    Ok(tx
        .query_row(
            "SELECT a.file_name, a.sha256 FROM attachments a
              WHERE a.id = ?1 AND a.deleted_at IS NULL
                AND ((a.owner_kind = 'wiki_page'
                      AND a.owner_id IN (SELECT id FROM wiki_pages WHERE project_id = ?2))
                  OR (a.owner_kind = 'task'
                      AND a.owner_id IN (SELECT id FROM tasks WHERE project_id = ?2))
                  OR (a.owner_kind = 'comment'
                      AND a.owner_id IN (SELECT c.id FROM comments c JOIN tasks t ON t.id = c.task_id
                                          WHERE t.project_id = ?2))
                  OR (a.owner_kind = 'project' AND a.owner_id = ?2))",
            params![b(id), b(project)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

/// `dir` + `stem` + `ext`, with `-2`, `-3`… after the stem when taken.
fn free_path(used: &mut BTreeSet<String>, dir: &str, stem: &str, ext: &str) -> String {
    let stem = if stem.is_empty() { "page" } else { stem };
    let mut n = 1;
    loop {
        let path = if n == 1 {
            format!("{dir}{stem}{ext}")
        } else {
            format!("{dir}{stem}-{n}{ext}")
        };
        // Ignoring case, for file systems that do.
        if used.insert(path.to_lowercase()) {
            return path;
        }
        n += 1;
    }
}

/// A file name that is safe in any archive and file system.
fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    if trimmed.is_empty() {
        "file".to_owned()
    } else {
        trimmed.chars().take(120).collect()
    }
}

/// `name` split before its extension (`.png`), which keeps its dot.
fn split_name(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_owned(), name[i..].to_owned()),
        _ => (name.to_owned(), String::new()),
    }
}

/// `to` (a path in the export) as seen from the file `from`.
pub(super) fn relative(from: &str, to: &str) -> String {
    let from_dirs: Vec<&str> = from.split('/').collect();
    let from_dirs = &from_dirs[..from_dirs.len() - 1];
    let to_parts: Vec<&str> = to.split('/').collect();
    let common = from_dirs
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count()
        .min(to_parts.len() - 1);
    let mut parts: Vec<&str> = vec![".."; from_dirs.len() - common];
    parts.extend(&to_parts[common..]);
    parts.join("/")
}

/// A path as a link destination: bytes other than letters, digits, `/`
/// and `-._~` percent-encoded.
pub(super) fn encode(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// A heading's anchor as GitHub and most Markdown viewers make it: lower
/// case, spaces as `-`, punctuation dropped.
pub(super) fn anchor(heading: &str) -> String {
    let slug: String = heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect();
    encode(&slug)
}

/// Text for a link's label, with the characters that would end it escaped.
fn escape_label(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{anchor, encode, relative, safe_name, split_name};

    #[test]
    fn paths_between_exported_files_are_relative() {
        assert_eq!(relative("a.md", "a/b.md"), "a/b.md");
        assert_eq!(relative("a/b.md", "a.md"), "../a.md");
        assert_eq!(relative("a/b/c.md", "a/d.md"), "../d.md");
        assert_eq!(relative("a/b.md", "files/x.png"), "../files/x.png");
        assert_eq!(relative("a.md", "a.md"), "a.md");
    }

    #[test]
    fn names_and_anchors_are_safe_in_links() {
        assert_eq!(
            encode("files/my plan (v2).pdf"),
            "files/my%20plan%20%28v2%29.pdf"
        );
        assert_eq!(anchor("Dates & times"), "dates--times");
        assert_eq!(anchor("Größe"), "gr%C3%B6%C3%9Fe");
        assert_eq!(safe_name("../a:b.png"), "_a_b.png");
        assert_eq!(split_name("a.tar.gz"), ("a.tar".into(), ".gz".into()));
        assert_eq!(split_name(".hidden"), (".hidden".into(), String::new()));
    }
}
