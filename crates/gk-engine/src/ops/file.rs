// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Attached files and the file store.
//!
//! A file belongs to a task, a comment or a wiki page. It is live while it
//! and what it belongs to are: a page's files go to the trash, come back
//! and are purged with the page.
//!
//! Attaching reads and checks the file outside the write transaction: its
//! size against the workspace's limit, its type from its bytes, an image's
//! size from its header. The bytes go into the file store under their
//! SHA-256 first (flushed, then renamed), and only then does the command
//! write the rows, so a stored row always has its file. The same content
//! attached twice is stored once; a duplicated task's attachments share
//! their files.
//!
//! History records an attachment's `created` (its life) and `trashed`.
//!
//! An image's thumbnail is made right after it is attached, outside the
//! transaction; a failure there only leaves it to be made when first asked
//! for (`file.thumbnail`).

use std::sync::Arc;

use gk_api::ops::file::{
    Attachment, FileAttach, FileAttachInput, FileChange, FileContent, FileDelete, FileGet,
    FileKind, FileRead, FileRef, FileThumbnail, Thumbnail,
};
use gk_api::{ErrorCode, OpError, Operation, descriptor};
use gk_core::Id;
use gk_store::StoreError;
use gk_store::rusqlite::{OptionalExtension, Row, Transaction, params};
use serde_json::Value;

use super::desc;
use super::fields::text;
use super::lookup::{self, b, id_at};
use super::relation::bump;
use crate::failure::Failure;
use crate::messages;
use crate::registry::{CommandFn, Registry};
use crate::time::rfc3339;
use crate::write::WriteCtx;

/// History's name for attachments.
pub(crate) const ENTITY: &str = "attachment";
/// The most text `file.read` returns.
const TEXT_LIMIT: u64 = 256 * 1024;
/// The largest image `file.read` returns.
const IMAGE_LIMIT: u64 = 5 * 1024 * 1024;
const MB: u64 = 1024 * 1024;

pub(super) fn register(r: &mut Registry) {
    r.query::<FileGet, _>(desc::<FileGet>(), |q, input: FileRef| {
        let id = find(q.tx, &input.attachment)?;
        load(q.tx, id)
    });
    r.query::<FileRead, _>(desc::<FileRead>(), |q, input: FileRef| {
        let id = find(q.tx, &input.attachment)?;
        read(&q.env.layout, load(q.tx, id)?)
    });
    r.query::<FileThumbnail, _>(desc::<FileThumbnail>(), |q, input: FileRef| {
        let id = find(q.tx, &input.attachment)?;
        let file = load(q.tx, id)?;
        let thumb = crate::thumbs::get(&q.env.layout, &file.sha256, &file.mime).map_err(|e| {
            Failure::Op(messages::hint(
                messages::invalid_at(
                    "attachment",
                    "file-no-thumbnail",
                    &[("name", file.name.clone()), ("error", e.to_string())],
                ),
                "file-no-thumbnail-hint",
                &[],
            ))
        })?;
        Ok(Thumbnail {
            attachment: file.id,
            path: thumb.path.to_string_lossy().into_owned(),
            mime: "image/png".to_owned(),
            width: thumb.width,
            height: thumb.height,
        })
    });
    r.engine_op::<FileAttach, _>(desc::<FileAttach>(), |engine, ctx, input| {
        let prepared = Arc::new(prepare(engine, ctx, &input)?);
        let value = serde_json::to_value(&input)
            .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))?;
        let for_command = Arc::clone(&prepared);
        let f: CommandFn = Arc::new(move |w, _| {
            let change = attach(w, &input, &for_command)?;
            serde_json::to_value(change).map_err(|e| {
                Failure::Op(messages::storage(
                    "storage-json",
                    &[("error", e.to_string())],
                ))
            })
        });
        let op = descriptor(FileAttach::ID)
            .ok_or_else(|| OpError::storage("file.attach is not in the catalog"))?;
        let output = engine.execute_command(ctx, op, f, value)?;
        make_thumbnail(engine, &prepared);
        serde_json::from_value(output)
            .map_err(|e| messages::storage("storage-json", &[("error", e.to_string())]))
    });
    r.command::<FileDelete, _>(desc::<FileDelete>(), |w, input: FileRef| {
        let id = find(w.tx(), &input.attachment)?;
        let owner = owner_of(w.tx(), id)?;
        ensure_writable(w.tx(), &owner)?;
        set_live(
            w,
            id,
            Some(owner.project),
            "trashed",
            Some(Value::Bool(true)),
        )?;
        changed(w, id)
    });
}

fn kind(mime: &str, name: &str) -> FileKind {
    match gk_core::kind_of(mime, name) {
        gk_core::FileKind::Image => FileKind::Image,
        gk_core::FileKind::Text => FileKind::Text,
        gk_core::FileKind::Executable => FileKind::Executable,
        gk_core::FileKind::Other => FileKind::Other,
    }
}

const COLUMNS: &str = "a.id, a.file_name, a.mime, a.size, a.sha256, a.width, a.height,
    a.owner_kind, a.owner_id, a.actor_kind, a.actor_name, a.created_at";

fn row(r: &Row<'_>) -> gk_store::rusqlite::Result<(Attachment, String, Id)> {
    let name: String = r.get(1)?;
    let mime: String = r.get(2)?;
    let actor_kind: String = r.get(9)?;
    Ok((
        Attachment {
            id: id_at(r, 0)?.to_string(),
            kind: kind(&mime, &name),
            name,
            mime,
            size: u64::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
            sha256: r.get(4)?,
            width: r
                .get::<_, Option<i64>>(5)?
                .and_then(|v| u32::try_from(v).ok()),
            height: r
                .get::<_, Option<i64>>(6)?
                .and_then(|v| u32::try_from(v).ok()),
            task: String::new(),
            comment: None,
            page: None,
            project: None,
            added_by: gk_api::ops::common::Actor {
                kind: gk_api::ops::common::ActorKind::from_stored(&actor_kind),
                name: r.get(10)?,
            },
            created_at: rfc3339(r.get(11)?),
        },
        r.get(7)?,
        id_at(r, 8)?,
    ))
}

/// Fills in the task key and comment, or the page, of an attachment row.
fn owned(
    tx: &Transaction<'_>,
    (mut a, owner_kind, owner): (Attachment, String, Id),
) -> Result<Attachment, Failure> {
    if owner_kind == "wiki_page" {
        a.page = Some(owner.to_string());
        a.project = tx
            .query_row(
                "SELECT p.key FROM wiki_pages w JOIN projects p ON p.id = w.project_id
                  WHERE w.id = ?1",
                [b(owner)],
                |r| r.get(0),
            )
            .optional()?;
        return Ok(a);
    }
    let task = if owner_kind == "comment" {
        a.comment = Some(owner.to_string());
        tx.query_row(
            "SELECT task_id FROM comments WHERE id = ?1",
            [b(owner)],
            |r| id_at(r, 0),
        )?
    } else {
        owner
    };
    if let Some(t) = lookup::task_by_id(tx, task)? {
        a.task = lookup::task_key(tx, t.project, t.number)?;
    }
    Ok(a)
}

/// One attachment as outputs carry it, trashed or not.
fn load(tx: &Transaction<'_>, id: Id) -> Result<Attachment, Failure> {
    let r = tx.query_row(
        &format!("SELECT {COLUMNS} FROM attachments a WHERE a.id = ?1"),
        [b(id)],
        row,
    )?;
    owned(tx, r)
}

fn owned_by(tx: &Transaction<'_>, kind: &str, owner: Id) -> Result<Vec<Attachment>, Failure> {
    let mut stmt = tx.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM attachments a
          WHERE a.owner_kind = ?1 AND a.owner_id = ?2 AND a.deleted_at IS NULL
          ORDER BY a.created_at, a.id"
    ))?;
    let rows = stmt
        .query_map(params![kind, b(owner)], row)?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(|r| owned(tx, r)).collect()
}

/// The live files attached to task `task` itself, oldest first.
pub(crate) fn of_task(tx: &Transaction<'_>, task: Id) -> Result<Vec<Attachment>, Failure> {
    owned_by(tx, "task", task)
}

/// The live files attached to comment `comment`, oldest first.
pub(crate) fn of_comment(tx: &Transaction<'_>, comment: Id) -> Result<Vec<Attachment>, Failure> {
    owned_by(tx, "comment", comment)
}

/// The live files attached to wiki page `page`, oldest first.
pub(crate) fn of_page(tx: &Transaction<'_>, page: Id) -> Result<Vec<Attachment>, Failure> {
    owned_by(tx, "wiki_page", page)
}

/// The live attachment `text` names (its ID), on a live task or page.
fn find(tx: &Transaction<'_>, text: &str) -> Result<Id, Failure> {
    let missing = || {
        Failure::Op(messages::hint(
            messages::not_found("file-not-found", &[("file", text.to_owned())])
                .with_detail("field", "attachment"),
            "file-not-found-hint",
            &[],
        ))
    };
    let id: Id = text.trim().parse().map_err(|_| missing())?;
    let live: Option<bool> = tx
        .query_row(
            "SELECT deleted_at IS NULL FROM attachments WHERE id = ?1",
            [b(id)],
            |r| r.get(0),
        )
        .optional()?;
    if live != Some(true) {
        return Err(missing());
    }
    let owner = owner_of(tx, id)?;
    let seen = match (owner.task, owner.page) {
        (Some(task), _) => lookup::task_by_id(tx, task)?.is_some(),
        (None, Some(page)) => super::wiki::by_id(tx, page)?.is_some(),
        (None, None) => false,
    };
    if !seen {
        return Err(missing());
    }
    Ok(id)
}

/// What an attachment belongs to: a task (directly or through one of its
/// comments) or a wiki page, and the project of either.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Owned {
    /// The task, for a file on a task or a comment.
    pub(crate) task: Option<Id>,
    /// The page, for a file on a wiki page.
    pub(crate) page: Option<Id>,
    pub(crate) project: Id,
}

/// What attachment `id` belongs to.
pub(crate) fn owner_of(tx: &Transaction<'_>, id: Id) -> Result<Owned, Failure> {
    let (kind, owner): (String, Vec<u8>) = tx.query_row(
        "SELECT owner_kind, owner_id FROM attachments WHERE id = ?1",
        [b(id)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let owner =
        Id::from_slice(&owner).map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    match kind.as_str() {
        "wiki_page" => Ok(Owned {
            task: None,
            page: Some(owner),
            project: tx.query_row(
                "SELECT project_id FROM wiki_pages WHERE id = ?1",
                [b(owner)],
                |r| id_at(r, 0),
            )?,
        }),
        "project" => Ok(Owned {
            task: None,
            page: None,
            project: owner,
        }),
        _ => {
            let task = if kind == "comment" {
                tx.query_row(
                    "SELECT task_id FROM comments WHERE id = ?1",
                    [b(owner)],
                    |r| id_at(r, 0),
                )?
            } else {
                owner
            };
            Ok(Owned {
                task: Some(task),
                page: None,
                project: task_project(tx, task)?,
            })
        }
    }
}

fn task_project(tx: &Transaction<'_>, task: Id) -> Result<Id, Failure> {
    Ok(tx.query_row(
        "SELECT project_id FROM tasks WHERE id = ?1",
        [b(task)],
        |r| id_at(r, 0),
    )?)
}

fn ensure_active(tx: &Transaction<'_>, task: Id) -> Result<(), Failure> {
    let t = lookup::task_by_id(tx, task)?
        .ok_or_else(|| Failure::Op(messages::not_found("file-task-gone", &[])))?;
    if let Some(p) = lookup::project_by_id(tx, t.project)? {
        lookup::ensure_active(&p)?;
    }
    Ok(())
}

/// Refuses changes to the files of what `owner` names while its project
/// is archived, or for a page, while the project's wiki is off.
fn ensure_writable(tx: &Transaction<'_>, owner: &Owned) -> Result<(), Failure> {
    if let Some(task) = owner.task {
        return ensure_active(tx, task);
    }
    if let Some(p) = lookup::project_by_id(tx, owner.project)? {
        lookup::ensure_active(&p)?;
        if owner.page.is_some() {
            lookup::ensure_wiki_on(tx, &p)?;
        }
    }
    Ok(())
}

fn read(layout: &gk_store::WorkspaceLayout, file: Attachment) -> Result<FileContent, Failure> {
    let texty = file.mime.starts_with("text/")
        || matches!(
            file.mime.as_str(),
            "application/json" | "application/xml" | "application/yaml" | "application/toml"
        );
    let io = |e: std::io::Error| {
        Failure::Op(messages::storage(
            "file-unreadable",
            &[("error", e.to_string())],
        ))
    };
    let mut out = FileContent {
        text: None,
        data_base64: None,
        truncated: false,
        note: None,
        file,
    };
    if texty {
        let bytes = gk_store::read_file(layout, &out.file.sha256, TEXT_LIMIT).map_err(io)?;
        out.truncated = out.file.size > TEXT_LIMIT;
        let valid = match std::str::from_utf8(&bytes) {
            Ok(t) => t.to_owned(),
            Err(e) => String::from_utf8_lossy(&bytes[..e.valid_up_to()]).into_owned(),
        };
        out.text = Some(valid);
    } else if out.file.kind == FileKind::Image {
        if out.file.size <= IMAGE_LIMIT {
            let bytes = gk_store::read_file(layout, &out.file.sha256, IMAGE_LIMIT).map_err(io)?;
            out.data_base64 = Some(gk_core::base64_encode(&bytes));
        } else {
            out.note = Some(messages::text(
                "file-image-too-large",
                &[("max", (IMAGE_LIMIT / MB).to_string())],
            ));
        }
    } else {
        out.note = Some(messages::text("file-not-text-or-image", &[]));
    }
    Ok(out)
}

/// A file read, checked and stored, ready for its rows.
struct Prepared {
    bytes: Vec<u8>,
    name: String,
    mime: &'static str,
    size: Option<(u32, u32)>,
    sha256: String,
}

fn limit_error(size: u64, max_mb: u32) -> OpError {
    messages::hint(
        messages::error(
            ErrorCode::Limit,
            "file-too-large",
            &[
                ("size", format!("{:.1}", size as f64 / MB as f64)),
                ("max", max_mb.to_string()),
            ],
        ),
        "file-too-large-hint",
        &[],
    )
    .with_detail("limit_mb", max_mb)
}

fn prepare(
    engine: &crate::engine::Engine,
    ctx: &crate::Context,
    input: &FileAttachInput,
) -> Result<Prepared, OpError> {
    let owners = [
        input.task.is_some(),
        input.comment.is_some(),
        input.page.is_some(),
    ];
    if owners.iter().filter(|o| **o).count() != 1 {
        return Err(messages::with("file-attach-to-what", &[], OpError::invalid));
    }
    if input.project.is_some() && input.page.is_none() {
        return Err(messages::invalid_at(
            "project",
            "file-project-for-page",
            &[],
        ));
    }
    let max_mb = engine
        .store()
        .read(|tx| super::settings::load(tx).map_err(Failure::from))
        .map_err(OpError::from)?
        .file_max_mb;
    let max = u64::from(max_mb) * MB;
    let (bytes, default_name) = match (&input.path, &input.data_base64) {
        (Some(given), None) => {
            // An agent's path was checked by the engine already; checked
            // again for callers that come here directly.
            let path = &if ctx.is_agent() {
                crate::agent_files::check(
                    &engine.env().layout,
                    &engine.env().agent_folders,
                    "path",
                    given,
                    crate::agent_files::Access::Read,
                )?
                .to_string_lossy()
                .into_owned()
            } else {
                given.clone()
            };
            let unreadable = |e: std::io::Error| {
                messages::invalid_at(
                    "path",
                    "file-path-unreadable",
                    &[("path", path.clone()), ("error", e.to_string())],
                )
            };
            let meta = std::fs::metadata(path).map_err(unreadable)?;
            if !meta.is_file() {
                return Err(messages::invalid_at(
                    "path",
                    "file-path-not-a-file",
                    &[("path", path.clone())],
                ));
            }
            if meta.len() > max {
                return Err(limit_error(meta.len(), max_mb));
            }
            let bytes = std::fs::read(path).map_err(unreadable)?;
            (bytes, Some(path.clone()))
        }
        (None, Some(data)) => {
            if data.len() as u64 / 4 * 3 > max + 3 {
                return Err(limit_error(data.len() as u64 / 4 * 3, max_mb));
            }
            let bytes = gk_core::base64_decode(data)
                .map_err(|e| OpError::from(super::task::invalid("data_base64", e)))?;
            (bytes, None)
        }
        _ => {
            return Err(messages::with("file-path-or-data", &[], OpError::invalid));
        }
    };
    if bytes.len() as u64 > max {
        return Err(limit_error(bytes.len() as u64, max_mb));
    }
    let raw_name = input
        .name
        .clone()
        .or(default_name)
        .ok_or_else(|| messages::invalid_at("name", "file-data-needs-name", &[]))?;
    let name = gk_core::clean_file_name(&raw_name)
        .map_err(|e| OpError::from(super::task::invalid("name", e)))?;
    let mime = gk_core::sniff(&bytes, &name);
    let size = if mime.starts_with("image/") {
        gk_core::image_size(&bytes)
    } else {
        None
    };
    let stored = gk_store::store_file(&engine.env().layout, &bytes)
        .map_err(|e| messages::storage("file-not-stored", &[("error", e.to_string())]))?;
    Ok(Prepared {
        bytes,
        name,
        mime,
        size,
        sha256: stored.sha256,
    })
}

/// Caches the thumbnail of a newly attached image, unless the cache has it
/// already. A failure is only logged: the thumbnail is made again when
/// asked for.
fn make_thumbnail(engine: &crate::engine::Engine, prepared: &Prepared) {
    cache_thumbnail(engine, &prepared.sha256, prepared.mime, &prepared.bytes);
}

/// Caches the thumbnail of the image `sha256` (of type `mime`, content
/// `bytes`), unless the cache has it already or it is not an image the
/// thumbnails are made for. A failure is only logged.
pub(crate) fn cache_thumbnail(
    engine: &crate::engine::Engine,
    sha256: &str,
    mime: &str,
    bytes: &[u8],
) {
    let layout = &engine.env().layout;
    if !crate::thumbs::supported(mime) || gk_store::thumb_path(layout, sha256).is_some() {
        return;
    }
    if let Err(e) = crate::thumbs::cache(layout, sha256, mime, bytes) {
        tracing::warn!(sha256 = %sha256, "no thumbnail: {e}");
    }
}

/// Makes sure the stored file `sha256` exists for a row about to point at
/// it, writing it again from `bytes` if it was removed meanwhile.
pub(crate) fn ensure_stored(w: &WriteCtx<'_>, sha256: &str, bytes: &[u8]) -> Result<(), Failure> {
    if !gk_store::has_file(w.layout(), sha256, bytes.len() as u64) {
        gk_store::store_file(w.layout(), bytes).map_err(|e| {
            Failure::Op(messages::storage(
                "file-not-stored",
                &[("error", e.to_string())],
            ))
        })?;
    }
    w.tx().execute(
        "INSERT OR IGNORE INTO blobs (sha256, size, created_at) VALUES (?1, ?2, ?3)",
        params![
            sha256,
            i64::try_from(bytes.len()).unwrap_or(i64::MAX),
            w.now()
        ],
    )?;
    Ok(())
}

/// The fields of a new attachment row.
pub(crate) struct NewAttachment<'a> {
    pub(crate) owner_kind: &'a str,
    pub(crate) owner: Id,
    /// The task whose time of change it bumps; none for a page's file.
    pub(crate) task: Option<Id>,
    pub(crate) project: Id,
    pub(crate) name: &'a str,
    pub(crate) mime: &'a str,
    pub(crate) size: i64,
    pub(crate) sha256: &'a str,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
}

/// Writes an attachment row for a file already in the store, indexes its
/// name and records its creation.
pub(crate) fn insert(w: &mut WriteCtx<'_>, a: &NewAttachment<'_>) -> Result<Id, Failure> {
    let id = Id::generate_at(u64::try_from(w.now()).unwrap_or(0));
    let caller = w.caller().clone();
    w.tx().execute(
        "INSERT INTO attachments (id, owner_kind, owner_id, file_name, mime, size, sha256, width,
                                  height, actor_kind, actor_name, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            b(id),
            a.owner_kind,
            b(a.owner),
            a.name,
            a.mime,
            a.size,
            a.sha256,
            a.width,
            a.height,
            caller.actor.kind.as_str(),
            caller.actor.name,
            w.now()
        ],
    )?;
    reindex(w, id)?;
    if let Some(task) = a.task {
        bump(w, task)?;
    }
    let pid = Some(a.project);
    w.record_field(ENTITY, id, pid, "created", None, Some(Value::Bool(true)));
    w.record_field(ENTITY, id, pid, "name", None, text(a.name));
    Ok(id)
}

/// Copies task `from`'s live attachments to task `to`, sharing their stored
/// files.
pub(crate) fn copy_to(w: &mut WriteCtx<'_>, from: Id, to: Id, project: Id) -> Result<(), Failure> {
    type Copied = (String, String, i64, String, Option<u32>, Option<u32>);
    let rows: Vec<Copied> = {
        let mut stmt = w.tx().prepare_cached(
            "SELECT file_name, mime, size, sha256, width, height FROM attachments
              WHERE owner_kind = 'task' AND owner_id = ?1 AND deleted_at IS NULL
              ORDER BY created_at, id",
        )?;
        stmt.query_map([b(from)], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    for (name, mime, size, sha256, width, height) in rows {
        insert(
            w,
            &NewAttachment {
                owner_kind: "task",
                owner: to,
                task: Some(to),
                project,
                name: &name,
                mime: &mime,
                size,
                sha256: &sha256,
                width,
                height,
            },
        )?;
    }
    Ok(())
}

fn attach(
    w: &mut WriteCtx<'_>,
    input: &FileAttachInput,
    p: &Prepared,
) -> Result<FileChange, Failure> {
    let (owner_kind, owner, task, project) = match (&input.task, &input.comment, &input.page) {
        (Some(key), _, _) => {
            let t = lookup::task(w.tx(), "task", key)?;
            ensure_active(w.tx(), t.id)?;
            ("task", t.id, Some(t.id), t.project)
        }
        (None, Some(comment), _) => {
            let c = super::comment::find(w.tx(), comment)?;
            ensure_active(w.tx(), c.task)?;
            let project = task_project(w.tx(), c.task)?;
            ("comment", c.id, Some(c.task), project)
        }
        (None, None, Some(page)) => {
            let page = super::wiki::find_page(w.tx(), input.project.as_deref(), "page", page)?;
            ensure_writable(
                w.tx(),
                &Owned {
                    task: None,
                    page: Some(page.id),
                    project: page.project,
                },
            )?;
            ("wiki_page", page.id, None, page.project)
        }
        (None, None, None) => {
            return Err(messages::with("file-task-or-comment", &[], OpError::invalid).into());
        }
    };
    ensure_stored(w, &p.sha256, &p.bytes)?;
    let id = insert(
        w,
        &NewAttachment {
            owner_kind,
            owner,
            task,
            project,
            name: &p.name,
            mime: p.mime,
            size: i64::try_from(p.bytes.len()).unwrap_or(i64::MAX),
            sha256: &p.sha256,
            width: p.size.map(|s| s.0),
            height: p.size.map(|s| s.1),
        },
    )?;
    changed(w, id)
}

fn changed(w: &WriteCtx<'_>, id: Id) -> Result<FileChange, Failure> {
    let deleted: bool = w.tx().query_row(
        "SELECT deleted_at IS NOT NULL FROM attachments WHERE id = ?1",
        [b(id)],
        |r| r.get(0),
    )?;
    Ok(FileChange {
        file: load(w.tx(), id)?,
        deleted,
        group: w.group_if_changed(),
    })
}

/// Keeps the file name's search entry in step: indexed while live,
/// removed while trashed.
fn reindex(w: &WriteCtx<'_>, id: Id) -> Result<(), Failure> {
    let row: Option<(String, bool)> = w
        .tx()
        .query_row(
            "SELECT file_name, deleted_at IS NULL FROM attachments WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        Some((name, true)) => {
            let owner = owner_of(w.tx(), id)?;
            let (task, project) = (owner.task.map(b), owner.project);
            w.tx().execute(
                "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body,
                                          updated_at)
                 VALUES ('file', ?1, ?2, ?3, ?4, '', ?5)
                 ON CONFLICT (kind, entity_id) DO UPDATE SET title = excluded.title,
                   updated_at = excluded.updated_at",
                params![b(id), task, b(project), name, w.now()],
            )?;
        }
        _ => {
            w.tx().execute(
                "DELETE FROM search_docs WHERE kind = 'file' AND entity_id = ?1",
                [b(id)],
            )?;
        }
    }
    Ok(())
}

/// Whether attachment `id` is live, as `created` (`true` while live) or
/// `trashed` (`true` while in the trash) records it.
pub(crate) fn raw_life(
    tx: &Transaction<'_>,
    id: Id,
    field: &str,
) -> Result<Option<Value>, StoreError> {
    let live: Option<bool> = tx
        .query_row(
            "SELECT deleted_at IS NULL FROM attachments WHERE id = ?1",
            [b(id)],
            |r| r.get(0),
        )
        .optional()?;
    Ok(live
        .filter(|l| *l == (field == "created"))
        .map(|_| Value::Bool(true)))
}

/// Moves attachment `id` to the trash or back, as `field` (`created` or
/// `trashed`) set to `value` asks, and records the change.
pub(crate) fn set_live(
    w: &mut WriteCtx<'_>,
    id: Id,
    project: Option<Id>,
    field: &str,
    value: Option<Value>,
) -> Result<(), Failure> {
    let live = (field == "created") == value.is_some();
    let before = raw_life(w.tx(), id, field)?;
    if before == value {
        return Ok(());
    }
    if live {
        let (sha256, size): (String, i64) = w.tx().query_row(
            "SELECT sha256, size FROM attachments WHERE id = ?1",
            [b(id)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if !gk_store::has_file(w.layout(), &sha256, u64::try_from(size).unwrap_or(0)) {
            return Err(messages::conflict("file-content-gone", &[]).into());
        }
        w.tx().execute(
            "UPDATE attachments SET deleted_at = NULL, deleted_group = NULL WHERE id = ?1",
            [b(id)],
        )?;
    } else {
        w.tx().execute(
            "UPDATE attachments SET deleted_at = ?2, deleted_group = ?3 WHERE id = ?1",
            params![b(id), w.now(), b(w.group())],
        )?;
    }
    reindex(w, id)?;
    if let Some(task) = owner_of(w.tx(), id)?.task {
        bump(w, task)?;
    }
    w.record_field(ENTITY, id, project, field, before, value);
    Ok(())
}

/// Removes the blob rows of `sha256s` that no attachment uses any more and
/// returns their hashes; the stored files go after the commit
/// ([`remove_unused_files`]), so a rolled-back transaction keeps them.
pub(crate) fn drop_unused_rows(
    w: &WriteCtx<'_>,
    sha256s: &[String],
) -> Result<Vec<String>, Failure> {
    let mut out = Vec::new();
    for sha in sha256s {
        let used: Option<i64> = w
            .tx()
            .query_row(
                "SELECT 1 FROM attachments WHERE sha256 = ?1 LIMIT 1",
                [sha],
                |r| r.get(0),
            )
            .optional()?;
        if used.is_none() {
            w.tx()
                .execute("DELETE FROM blobs WHERE sha256 = ?1", [sha])?;
            out.push(sha.clone());
        }
    }
    Ok(out)
}

/// Removes the stored files of `sha256s` that have no blob row (checked
/// again now, in case another program attached the same content since).
pub(crate) fn remove_unused_files(
    store: &gk_store::Store,
    sha256s: &[String],
) -> Result<u64, Failure> {
    let mut removed = 0;
    for sha in sha256s {
        let known: Option<i64> = store.read(|tx| {
            Ok::<_, Failure>(
                tx.query_row("SELECT 1 FROM blobs WHERE sha256 = ?1", [sha], |r| r.get(0))
                    .optional()?,
            )
        })?;
        if known.is_some() {
            continue;
        }
        match gk_store::remove_file(store.layout(), sha) {
            Ok(()) => removed += 1,
            Err(e) => {
                tracing::warn!(error = %e, "a stored file that is no longer used could not be removed");
            }
        }
    }
    Ok(removed)
}
