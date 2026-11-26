// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Importing a folder of Markdown files (or a `.zip` file of one) as wiki
//! pages.
//!
//! - Each `.md` file becomes a page titled after the file. A sub-folder
//!   becomes a page holding the pages of its files: the file next to it with
//!   the same name (`guide.md` for `guide/`) is its text, or else its
//!   `index.md` or `README.md`; without one it is an empty page.
//! - Relative links between the files (`[setup](guide/setup.md#install)`)
//!   become wiki links (`[[Setup#Install|setup]]`), and `[[…]]` links that
//!   name an imported file follow its page's title. Images and other files
//!   the texts link to are attached to the page that links to them. Other
//!   links stay as written.
//! - The folder is read like an archive, without trusting it: no links, no
//!   paths out of it, limits on the number and size of files, and Markdown
//!   must be UTF-8. Nothing is written before all of it has been read.
//! - Existing pages are never changed; a title that is taken gets a number.
//!   Everything is made by one command, so Undo takes the whole import back.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use gk_api::ops::wiki::{
    WikiCreateInput, WikiImport, WikiImportInput, WikiImportRename, WikiImportResult, WikiPageRef,
};
use gk_api::{OpError, Operation, descriptor};
use gk_core::{Id, fold};
use gk_store::rusqlite::params;
use serde::{Deserialize, Serialize};

use super::source::Source;
use super::wiki_export::anchor;
use super::{Limits, resolve};
use crate::context::Context;
use crate::engine::Engine;
use crate::failure::Failure;
use crate::messages;
use crate::ops::file::{self, NewAttachment};
use crate::ops::{lookup, settings, wiki};
use crate::registry::CommandFn;

/// The most pages one import makes.
const MAX_PAGES: usize = 5_000;
/// The most entries the folder may hold.
const MAX_ENTRIES: usize = 20_000;
/// The most bytes the folder may hold.
const MAX_TOTAL: u64 = 8 << 30;
/// The largest Markdown file: the largest page text.
const MAX_MARKDOWN: u64 = 1 << 20;
const MB: u64 = 1024 * 1024;

/// One page to make.
#[derive(Debug, Clone)]
struct Node {
    /// Where it is in the folder: the file without `.md`, or the folder.
    key: String,
    parent: Option<usize>,
    /// The Markdown file that holds its text.
    source: Option<String>,
    /// The folder its file's relative links start from.
    base: String,
    title: String,
    text: String,
    /// The text with its links rewritten; attachments by their place in
    /// `files`.
    pieces: Vec<Piece>,
    /// The files it links to, by their place in the import's files.
    files: Vec<usize>,
}

#[derive(Debug, Clone)]
enum Piece {
    Text(String),
    File(usize),
}

/// One file to attach, read and stored.
#[derive(Debug, Clone)]
struct Attached {
    path: String,
    name: String,
    mime: String,
    size: u64,
    sha256: String,
    width: Option<u32>,
    height: Option<u32>,
}

/// What the command reports back.
#[derive(Debug, Serialize, Deserialize)]
struct Made {
    top: Vec<WikiPageRef>,
    group: Option<String>,
}

/// What the import needs to know about the wiki it goes into.
struct Target {
    project: String,
    project_id: Id,
    parent: Option<Id>,
    /// How many pages the parent is nested in, plus one; 0 at the top.
    depth: u32,
    /// The folded titles of the project's live pages.
    taken: BTreeSet<String>,
    max_file: u64,
}

impl Engine {
    /// Imports the folder `input.path` names as wiki pages of
    /// `input.project`, or with `dry_run` only checks it.
    pub fn import_wiki_folder(
        &self,
        ctx: &Context,
        input: &WikiImportInput,
    ) -> Result<WikiImportResult, OpError> {
        let path = resolve(self, ctx, &input.path, "path")?;
        let target = self
            .store()
            .read(|tx| {
                let _scope = crate::scope::enter(tx, ctx)?;
                let project = lookup::project(tx, "project", &input.project)?;
                lookup::ensure_active(&project)?;
                lookup::ensure_wiki_on(tx, &project)?;
                let parent = input
                    .parent
                    .as_deref()
                    .map(|p| wiki::find_page(tx, Some(&project.key), "parent", p))
                    .transpose()?;
                let depth = match &parent {
                    Some(p) => wiki::depth(tx, p.parent)? + 1,
                    None => 0,
                };
                let mut stmt = tx.prepare_cached(
                    "SELECT title_fold FROM wiki_pages WHERE project_id = ?1 AND deleted_at IS NULL",
                )?;
                let taken = stmt
                    .query_map([lookup::b(project.id)], |r| r.get::<_, String>(0))?
                    .collect::<Result<BTreeSet<_>, _>>()?;
                let max_mb = settings::load(tx)?.file_max_mb;
                Ok::<_, Failure>(Target {
                    project: project.key.clone(),
                    project_id: project.id,
                    parent: parent.map(|p| p.id),
                    depth,
                    taken,
                    max_file: u64::from(max_mb) * MB,
                })
            })
            .map_err(OpError::from)?;

        let limits = Limits {
            max_entries: MAX_ENTRIES,
            max_total: MAX_TOTAL,
            max_markdown: MAX_MARKDOWN,
            max_file: target.max_file,
            ..Limits::default()
        };
        let mut source = Source::open(&path, &limits).map_err(OpError::from)?;
        let entries: BTreeMap<String, u64> = source
            .entries()
            .iter()
            .map(|(p, info)| (p.clone(), info.size))
            .collect();
        let mut plan = plan(&entries, &target)?;
        for node in &mut plan.nodes {
            if let Some(file) = &node.source {
                let bytes = source.read(file, MAX_MARKDOWN).map_err(OpError::from)?;
                let text = String::from_utf8(bytes).map_err(|_| {
                    messages::invalid_at("path", "wiki-import-not-text", &[("path", file.clone())])
                })?;
                node.text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned();
            }
        }
        let wanted = link(&mut plan, &entries);
        let mut files = Vec::with_capacity(wanted.len());
        for file_path in &wanted {
            let size = entries.get(file_path).copied().unwrap_or(0);
            if size > target.max_file {
                return Err(
                    super::Rejected::EntryTooLarge(file_path.clone(), target.max_file).into(),
                );
            }
            let name = file_path.rsplit('/').next().unwrap_or(file_path);
            let name = gk_core::clean_file_name(name).unwrap_or_else(|_| "file".to_owned());
            if input.dry_run {
                files.push(Attached {
                    path: file_path.clone(),
                    name,
                    mime: String::new(),
                    size,
                    sha256: String::new(),
                    width: None,
                    height: None,
                });
                continue;
            }
            let bytes = source
                .read(file_path, target.max_file)
                .map_err(OpError::from)?;
            let mime = gk_core::sniff(&bytes, &name);
            let dims = if mime.starts_with("image/") {
                gk_core::image_size(&bytes)
            } else {
                None
            };
            let stored = gk_store::store_file(&self.env().layout, &bytes)
                .map_err(|e| messages::storage("file-not-stored", &[("error", e.to_string())]))?;
            file::cache_thumbnail(self, &stored.sha256, mime, &bytes);
            files.push(Attached {
                path: file_path.clone(),
                name,
                mime: mime.to_owned(),
                size: bytes.len() as u64,
                sha256: stored.sha256,
                width: dims.map(|d| d.0),
                height: dims.map(|d| d.1),
            });
        }

        let linked: BTreeSet<&String> = wanted.iter().collect();
        let used: BTreeSet<&String> = plan
            .nodes
            .iter()
            .filter_map(|n| n.source.as_ref())
            .collect();
        let skipped: Vec<String> = entries
            .keys()
            .filter(|p| !used.contains(p) && !linked.contains(p))
            .cloned()
            .collect();
        let attachments: u64 = plan.nodes.iter().map(|n| n.files.len() as u64).sum();
        let mut result = WikiImportResult {
            dry_run: input.dry_run,
            pages: plan.nodes.len() as u64,
            attachments,
            top: Vec::new(),
            renamed: plan.renamed.clone(),
            skipped,
            group: None,
        };
        if input.dry_run {
            result.top = plan
                .nodes
                .iter()
                .filter(|n| n.parent.is_none())
                .map(|n| WikiPageRef {
                    title: n.title.clone(),
                    slug: gk_core::slug(&n.title),
                    id: String::new(),
                })
                .collect();
            return Ok(result);
        }

        let nodes = Arc::new(plan.nodes);
        let files = Arc::new(files);
        let project = target.project.clone();
        let project_id = target.project_id;
        let parent = target.parent;
        let f: CommandFn = Arc::new(move |w, _| {
            let mut ids: Vec<Id> = Vec::with_capacity(nodes.len());
            let mut top = Vec::new();
            for node in nodes.iter() {
                let parent = node.parent.map(|i| ids[i]).or(parent);
                let change = wiki::create_page(
                    w,
                    WikiCreateInput {
                        project: project.clone(),
                        title: node.title.clone(),
                        parent: parent.map(|p| p.to_string()),
                        ..WikiCreateInput::default()
                    },
                )?;
                let id: Id = change
                    .page
                    .id
                    .parse()
                    .map_err(|_| Failure::Op(messages::storage("wiki-no-place", &[])))?;
                if node.parent.is_none() {
                    top.push(WikiPageRef {
                        title: change.page.title.clone(),
                        slug: change.page.slug.clone(),
                        id: change.page.id.clone(),
                    });
                }
                ids.push(id);
            }
            for (node, id) in nodes.iter().zip(&ids) {
                let mut attached = Vec::with_capacity(node.files.len());
                for &n in &node.files {
                    let file = &files[n];
                    if !gk_store::has_file(w.layout(), &file.sha256, file.size) {
                        return Err(Failure::Op(messages::storage(
                            "file-not-stored",
                            &[("error", file.path.clone())],
                        )));
                    }
                    let size = i64::try_from(file.size).unwrap_or(i64::MAX);
                    w.tx().execute(
                        "INSERT OR IGNORE INTO blobs (sha256, size, created_at) VALUES (?1, ?2, ?3)",
                        params![file.sha256, size, w.now()],
                    )?;
                    attached.push(file::insert(
                        w,
                        &NewAttachment {
                            owner_kind: "wiki_page",
                            owner: *id,
                            task: None,
                            project: project_id,
                            name: &file.name,
                            mime: &file.mime,
                            size,
                            sha256: &file.sha256,
                            width: file.width,
                            height: file.height,
                        },
                    )?);
                }
                let mut body = String::with_capacity(node.text.len());
                for piece in &node.pieces {
                    match piece {
                        Piece::Text(t) => body.push_str(t),
                        Piece::File(i) => {
                            let place = node.files.iter().position(|f| f == i).unwrap_or(0);
                            if let Some(a) = attached.get(place) {
                                body.push_str(&format!("gk-file://{a}"));
                            }
                        }
                    }
                }
                if !body.trim().is_empty() {
                    wiki::first_text(w, *id, &body)?;
                }
            }
            serde_json::to_value(Made {
                top,
                group: w.group_if_changed(),
            })
            .map_err(|e| {
                Failure::Op(messages::storage(
                    "storage-json",
                    &[("error", e.to_string())],
                ))
            })
        });
        let op = descriptor(WikiImport::ID)
            .ok_or_else(|| OpError::storage("wiki.import is not in the catalog"))?;
        let value = serde_json::to_value(input)
            .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))?;
        let output = self.execute_command(ctx, op, f, value)?;
        let made: Made = serde_json::from_value(output)
            .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))?;
        result.top = made.top;
        result.group = made.group;
        Ok(result)
    }
}

/// The pages to make, in an order that has each page after its parent and
/// siblings by name, and the titles that had to change.
struct Plan {
    nodes: Vec<Node>,
    renamed: Vec<WikiImportRename>,
}

fn is_markdown(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

fn hidden(path: &str) -> bool {
    path.split('/').any(|p| p.starts_with('.'))
}

/// `path` without its Markdown extension.
fn stem(path: &str) -> &str {
    let cut = path.rfind('.').unwrap_or(path.len());
    &path[..cut]
}

fn parent_dir(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

fn last(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Whether `file` (in folder `dir`) is the folder's own text.
fn is_folder_text(file: &str) -> bool {
    let name = last(file).to_ascii_lowercase();
    matches!(
        name.as_str(),
        "index.md" | "index.markdown" | "readme.md" | "readme.markdown"
    )
}

/// A page title made from a file name: the characters links can't hold
/// become `-`, at most the length titles may have.
fn title_of(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '[' | ']' | '#' | '|') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = cleaned
        .chars()
        .take(gk_core::PAGE_TITLE_MAX_CHARS - 8)
        .collect();
    let cut = cut.trim().to_owned();
    if cut.is_empty() {
        "Page".to_owned()
    } else {
        cut
    }
}

fn plan(entries: &BTreeMap<String, u64>, target: &Target) -> Result<Plan, OpError> {
    // The pages by their key: each Markdown file, and each folder that
    // holds one somewhere below.
    let mut sources: BTreeMap<String, Option<String>> = BTreeMap::new();
    let markdown: Vec<&String> = entries
        .keys()
        .filter(|p| is_markdown(p) && !hidden(p))
        .collect();
    if markdown.is_empty() {
        return Err(messages::invalid_at("path", "wiki-import-nothing", &[]));
    }
    for file in &markdown {
        let dir = parent_dir(file);
        let key = if !dir.is_empty() && is_folder_text(file) {
            dir.to_owned()
        } else {
            stem(file).to_owned()
        };
        let mut d = parent_dir(&key).to_owned();
        while !d.is_empty() {
            sources.entry(d.clone()).or_insert(None);
            d = parent_dir(&d).to_owned();
        }
        // `guide.md` beats `guide/index.md` as the folder's text.
        let slot = sources.entry(key).or_insert(None);
        let first_choice = !is_folder_text(file) || slot.is_none();
        if first_choice && slot.as_ref().is_none_or(|s| is_folder_text(s)) {
            *slot = Some((*file).clone());
        }
    }
    if sources.len() > MAX_PAGES {
        return Err(messages::invalid_at(
            "path",
            "wiki-import-too-many",
            &[
                ("count", sources.len().to_string()),
                ("max", MAX_PAGES.to_string()),
            ],
        ));
    }

    let mut taken = target.taken.clone();
    let mut nodes: Vec<Node> = Vec::with_capacity(sources.len());
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut renamed = Vec::new();
    for (key, source) in sources {
        let level = key.matches('/').count();
        if target.depth as usize + level > wiki::MAX_DEPTH as usize {
            return Err(messages::invalid_at(
                "path",
                "wiki-import-too-deep",
                &[("path", key.clone()), ("max", wiki::MAX_DEPTH.to_string())],
            ));
        }
        let wanted = title_of(last(&key));
        let mut title = wanted.clone();
        let mut n = 2;
        while taken.contains(&fold(&title)) {
            title = format!("{wanted} ({n})");
            n += 1;
        }
        taken.insert(fold(&title));
        if title != last(&key) {
            renamed.push(WikiImportRename {
                path: source.clone().unwrap_or_else(|| key.clone()),
                title: title.clone(),
            });
        }
        let parent = index.get(parent_dir(&key)).copied();
        let base = source
            .as_deref()
            .map_or(key.as_str(), parent_dir)
            .to_owned();
        index.insert(key.clone(), nodes.len());
        nodes.push(Node {
            key,
            parent,
            source,
            base,
            title,
            text: String::new(),
            pieces: Vec::new(),
            files: Vec::new(),
        });
    }
    Ok(Plan { nodes, renamed })
}

/// `rel` (a link's path, percent-decoded) from folder `base`, inside the
/// import; `None` when it leaves it.
fn join(base: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = if base.is_empty() {
        Vec::new()
    } else {
        base.split('/').collect()
    };
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// `%xx` escapes read, as in link destinations.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(h) = text.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(h, 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether a link's destination points somewhere else than a file of the
/// import: a web address, `mailto:`, an absolute path, or this page.
fn external(url: &str) -> bool {
    let scheme = url.find(':').is_some_and(|c| {
        url[..c]
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "+-.".contains(ch))
            && c > 0
    });
    scheme || url.starts_with('/') || url.starts_with('#') || url.is_empty()
}

/// The heading of `text` whose anchor is `fragment`, as wiki links name it.
fn heading_for(text: &str, fragment: &str) -> Option<String> {
    let wanted = decode(fragment).to_lowercase();
    gk_markdown::headings(text).into_iter().find_map(|h| {
        let a = decode(&anchor(&h.text));
        (a == wanted || fold(&h.text) == fold(&wanted)).then_some(h.text)
    })
}

/// Text for a wiki link's label, or `None` when it can't be one.
fn label_of(text: &str, title: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty()
        || t == title
        || t.contains(['|', '\n', '\r'])
        || t.contains("]]")
        || t.contains("[[")
    {
        return None;
    }
    Some(t.to_owned())
}

fn wiki_link(title: &str, heading: Option<&str>, label: Option<&str>) -> String {
    let mut out = format!("[[{title}");
    if let Some(h) = heading.filter(|h| !h.contains(['|', ']', '['])) {
        out.push('#');
        out.push_str(h);
    }
    if let Some(l) = label {
        out.push('|');
        out.push_str(l);
    }
    out.push_str("]]");
    out
}

/// Rewrites every page's links (into its `pieces`) and returns the files
/// to attach, by path; each page's `files` names them by their place.
fn link(plan: &mut Plan, entries: &BTreeMap<String, u64>) -> Vec<String> {
    let by_key: HashMap<String, usize> = plan
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.key.clone(), i))
        .collect();
    let mut by_source: HashMap<String, usize> = HashMap::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for (i, n) in plan.nodes.iter().enumerate() {
        if let Some(s) = &n.source {
            by_source.insert(s.clone(), i);
        }
        by_name.entry(fold(last(&n.key))).or_insert(i);
        by_name.entry(fold(&n.key)).or_insert(i);
    }
    let titles: Vec<String> = plan.nodes.iter().map(|n| n.title.clone()).collect();
    let texts: Vec<String> = plan.nodes.iter().map(|n| n.text.clone()).collect();
    let mut wanted: Vec<String> = Vec::new();
    for node in &mut plan.nodes {
        let text = node.text.clone();
        // (range, replacement); a replacement of None is a file.
        let mut edits: Vec<(std::ops::Range<usize>, Result<String, usize>)> = Vec::new();
        for span in gk_markdown::wiki_link_spans(&text) {
            if span.link.page.is_empty() {
                continue;
            }
            let Some(&i) = by_name.get(&fold(&span.link.page)) else {
                continue;
            };
            if titles[i] == span.link.page {
                continue;
            }
            edits.push((
                span.range.clone(),
                Ok(wiki_link(
                    &titles[i],
                    span.link.heading.as_deref(),
                    span.label.as_deref(),
                )),
            ));
        }
        for span in gk_markdown::link_spans(&text) {
            let url = span.url.trim();
            if external(url) {
                continue;
            }
            let (path, fragment) = match url.split_once('#') {
                Some((p, f)) => (p, Some(f)),
                None => (url, None),
            };
            let Some(path) = join(&node.base, &decode(path)) else {
                continue;
            };
            let page = by_source
                .get(&path)
                .or_else(|| by_key.get(path.trim_end_matches('/')))
                .copied();
            if let (Some(i), false) = (page, span.image) {
                let heading = fragment.and_then(|f| heading_for(&texts[i], f));
                let label = label_of(&text[span.text.clone()], &titles[i]);
                edits.push((
                    span.range.clone(),
                    Ok(wiki_link(&titles[i], heading.as_deref(), label.as_deref())),
                ));
            } else if entries.contains_key(&path) && !is_markdown(&path) && !hidden(&path) {
                let n = match wanted.iter().position(|w| *w == path) {
                    Some(n) => n,
                    None => {
                        wanted.push(path.clone());
                        wanted.len() - 1
                    }
                };
                if !node.files.contains(&n) {
                    node.files.push(n);
                }
                edits.push((span.destination.clone(), Err(n)));
            }
        }
        edits.sort_by_key(|(r, _)| r.start);
        let mut pieces = Vec::new();
        let mut at = 0;
        for (range, replacement) in edits {
            if range.start < at {
                continue;
            }
            pieces.push(Piece::Text(text[at..range.start].to_owned()));
            pieces.push(match replacement {
                Ok(t) => Piece::Text(t),
                Err(n) => Piece::File(n),
            });
            at = range.end;
        }
        pieces.push(Piece::Text(text[at..].to_owned()));
        node.pieces = pieces;
    }
    wanted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_stay_inside_the_import() {
        assert_eq!(join("a/b", "../c.md").as_deref(), Some("a/c.md"));
        assert_eq!(join("", "./img/x.png").as_deref(), Some("img/x.png"));
        assert_eq!(join("a", "../../etc/passwd"), None);
        assert_eq!(join("", ".."), None);
        assert!(external("https://example.com/a.md"));
        assert!(external("mailto:me@example.com"));
        assert!(external("/etc/passwd"));
        assert!(external("#top"));
        assert!(!external("guide/setup.md"));
        assert_eq!(decode("my%20plan.md"), "my plan.md");
    }

    #[test]
    fn titles_are_made_safe_for_links() {
        assert_eq!(title_of("Plan [v2] #1"), "Plan -v2- -1");
        assert_eq!(title_of("   "), "Page");
        assert_eq!(
            wiki_link("Setup", Some("Install"), Some("setup")),
            "[[Setup#Install|setup]]"
        );
        assert_eq!(label_of("Setup", "Setup"), None);
        assert_eq!(label_of("a|b", "Setup"), None);
    }

    #[test]
    fn folders_become_pages_holding_their_files() {
        let entries: BTreeMap<String, u64> = [
            "guide.md",
            "guide/setup.md",
            "notes/index.md",
            "notes/deep/a.md",
            "img/x.png",
            ".git/config",
        ]
        .iter()
        .map(|p| ((*p).to_owned(), 1))
        .collect();
        let target = Target {
            project: "TM".into(),
            project_id: Id::generate_at(0),
            parent: None,
            depth: 0,
            taken: ["guide".to_owned()].into_iter().collect(),
            max_file: MB,
        };
        let plan = plan(&entries, &target).unwrap();
        let keys: Vec<(&str, Option<&str>, &str)> = plan
            .nodes
            .iter()
            .map(|n| (n.key.as_str(), n.source.as_deref(), n.title.as_str()))
            .collect();
        assert_eq!(
            keys,
            [
                ("guide", Some("guide.md"), "guide (2)"),
                ("guide/setup", Some("guide/setup.md"), "setup"),
                ("notes", Some("notes/index.md"), "notes"),
                ("notes/deep", None, "deep"),
                ("notes/deep/a", Some("notes/deep/a.md"), "a"),
            ]
        );
        assert_eq!(plan.nodes[1].parent, Some(0));
        assert_eq!(plan.nodes[4].parent, Some(3));
        assert_eq!(plan.nodes[2].base, "notes");
        assert_eq!(plan.renamed.len(), 1);
    }
}
