// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Attached files and the file store.
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
            Failure::Op(
                OpError::invalid_at(
                    "attachment",
                    format!("{} has no thumbnail: {e}.", file.name),
                )
                .with_hint("Thumbnails are made for PNG, JPEG, GIF and WebP images."),
            )
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
        let prepared = Arc::new(prepare(engine, &input)?);
        let value = serde_json::to_value(&input).map_err(|e| OpError::invalid(e.to_string()))?;
        let for_command = Arc::clone(&prepared);
        let f: CommandFn = Arc::new(move |w, _| {
            let change = attach(w, &input, &for_command)?;
            serde_json::to_value(change).map_err(|e| Failure::Op(OpError::storage(e.to_string())))
        });
        let op = descriptor(FileAttach::ID)
            .ok_or_else(|| OpError::storage("file.attach is not in the catalog"))?;
        let output = engine.execute_command(ctx, op, f, value)?;
        make_thumbnail(engine, &prepared);
        serde_json::from_value(output).map_err(|e| OpError::storage(e.to_string()))
    });
    r.command::<FileDelete, _>(desc::<FileDelete>(), |w, input: FileRef| {
        let id = find(w.tx(), &input.attachment)?;
        let owner = owner_task(w.tx(), id)?;
        ensure_active(w.tx(), owner)?;
        let project = task_project(w.tx(), owner)?;
        set_live(w, id, Some(project), "trashed", Some(Value::Bool(true)))?;
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
            added_by: gk_api::ops::common::Actor {
                kind: if actor_kind == "agent" {
                    gk_api::ops::common::ActorKind::Agent
                } else {
                    gk_api::ops::common::ActorKind::User
                },
                name: r.get(10)?,
            },
            created_at: rfc3339(r.get(11)?),
        },
        r.get(7)?,
        id_at(r, 8)?,
    ))
}

/// Fills in the task key and comment of an attachment row.
fn owned(
    tx: &Transaction<'_>,
    (mut a, owner_kind, owner): (Attachment, String, Id),
) -> Result<Attachment, Failure> {
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

/// The live attachment `text` names (its ID), on a live task.
fn find(tx: &Transaction<'_>, text: &str) -> Result<Id, Failure> {
    let missing = || {
        Failure::Op(
            OpError::not_found(format!("There is no attached file \"{text}\"."), &[])
                .with_detail("field", "attachment")
                .with_hint("Attachment IDs are listed by get_task with include [\"files\"]."),
        )
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
    let task = owner_task(tx, id)?;
    if lookup::task_by_id(tx, task)?.is_none() {
        return Err(missing());
    }
    Ok(id)
}

/// The task an attachment belongs to, directly or through its comment.
fn owner_task(tx: &Transaction<'_>, id: Id) -> Result<Id, Failure> {
    let (kind, owner): (String, Vec<u8>) = tx.query_row(
        "SELECT owner_kind, owner_id FROM attachments WHERE id = ?1",
        [b(id)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let owner =
        Id::from_slice(&owner).map_err(|e| Failure::Store(StoreError::Corrupt(e.to_string())))?;
    if kind == "comment" {
        Ok(tx.query_row(
            "SELECT task_id FROM comments WHERE id = ?1",
            [b(owner)],
            |r| id_at(r, 0),
        )?)
    } else {
        Ok(owner)
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
        .ok_or_else(|| Failure::Op(OpError::not_found("The file's task is gone.", &[])))?;
    if let Some(p) = lookup::project_by_id(tx, t.project)? {
        lookup::ensure_active(&p)?;
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
        Failure::Op(OpError::storage(format!(
            "The stored file can't be read: {e}."
        )))
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
            out.note = Some(format!(
                "The image is larger than {} MB, so its content is not returned.",
                IMAGE_LIMIT / MB
            ));
        }
    } else {
        out.note =
            Some("This file is neither text nor an image; only its details are returned.".into());
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
    OpError::new(
        ErrorCode::Limit,
        format!(
            "The file is {:.1} MB; files can be at most {max_mb} MB.",
            size as f64 / MB as f64
        ),
    )
    .with_detail("limit_mb", max_mb)
    .with_hint("The limit is the workspace setting file_max_mb.")
}

fn prepare(engine: &crate::engine::Engine, input: &FileAttachInput) -> Result<Prepared, OpError> {
    if input.task.is_some() == input.comment.is_some() {
        return Err(OpError::invalid(
            "Name either the `task` or the `comment` to attach the file to.",
        ));
    }
    let max_mb = engine
        .store()
        .read(|tx| super::settings::load(tx).map_err(Failure::from))
        .map_err(OpError::from)?
        .file_max_mb;
    let max = u64::from(max_mb) * MB;
    let (bytes, default_name) = match (&input.path, &input.data_base64) {
        (Some(path), None) => {
            let meta = std::fs::metadata(path)
                .map_err(|e| OpError::invalid_at("path", format!("Can't read \"{path}\": {e}.")))?;
            if !meta.is_file() {
                return Err(OpError::invalid_at(
                    "path",
                    format!("\"{path}\" is not a file."),
                ));
            }
            if meta.len() > max {
                return Err(limit_error(meta.len(), max_mb));
            }
            let bytes = std::fs::read(path)
                .map_err(|e| OpError::invalid_at("path", format!("Can't read \"{path}\": {e}.")))?;
            (bytes, Some(path.clone()))
        }
        (None, Some(data)) => {
            if data.len() as u64 / 4 * 3 > max + 3 {
                return Err(limit_error(data.len() as u64 / 4 * 3, max_mb));
            }
            let bytes = gk_core::base64_decode(data)
                .map_err(|e| OpError::invalid_at("data_base64", format!("{e}.")))?;
            (bytes, None)
        }
        _ => {
            return Err(OpError::invalid(
                "Give either `path` or `data_base64` (with `name`).",
            ));
        }
    };
    if bytes.len() as u64 > max {
        return Err(limit_error(bytes.len() as u64, max_mb));
    }
    let raw_name = input.name.clone().or(default_name).ok_or_else(|| {
        OpError::invalid_at("name", "A file given as `data_base64` needs a `name`.")
    })?;
    let name = gk_core::clean_file_name(&raw_name)
        .map_err(|e| OpError::invalid_at("name", format!("{e}.")))?;
    let mime = gk_core::sniff(&bytes, &name);
    let size = if mime.starts_with("image/") {
        gk_core::image_size(&bytes)
    } else {
        None
    };
    let stored = gk_store::store_file(&engine.env().layout, &bytes)
        .map_err(|e| OpError::storage(format!("The file can't be stored: {e}.")))?;
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
    let layout = &engine.env().layout;
    if !crate::thumbs::supported(prepared.mime)
        || gk_store::thumb_path(layout, &prepared.sha256).is_some()
    {
        return;
    }
    if let Err(e) = crate::thumbs::cache(layout, &prepared.sha256, prepared.mime, &prepared.bytes) {
        tracing::warn!(sha256 = %prepared.sha256, "no thumbnail: {e}");
    }
}

/// Makes sure the stored file `sha256` exists for a row about to point at
/// it, writing it again from `bytes` if it was removed meanwhile.
fn ensure_stored(w: &WriteCtx<'_>, sha256: &str, bytes: &[u8]) -> Result<(), Failure> {
    if !gk_store::has_file(w.layout(), sha256, bytes.len() as u64) {
        gk_store::store_file(w.layout(), bytes).map_err(|e| {
            Failure::Op(OpError::storage(format!("The file can't be stored: {e}.")))
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
    pub(crate) task: Id,
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
    bump(w, a.task)?;
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
                task: to,
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
    let (owner_kind, owner, task) = match (&input.task, &input.comment) {
        (Some(key), _) => {
            let t = lookup::task(w.tx(), "task", key)?;
            ("task", t.id, t.id)
        }
        (None, Some(comment)) => {
            let c = super::comment::find(w.tx(), comment)?;
            ("comment", c.id, c.task)
        }
        (None, None) => return Err(OpError::invalid("Name the task or the comment.").into()),
    };
    ensure_active(w.tx(), task)?;
    let project = task_project(w.tx(), task)?;
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
            let task = owner_task(w.tx(), id)?;
            let project = task_project(w.tx(), task)?;
            w.tx().execute(
                "INSERT INTO search_docs (kind, entity_id, task_id, project_id, title, body,
                                          updated_at)
                 VALUES ('file', ?1, ?2, ?3, ?4, '', ?5)
                 ON CONFLICT (kind, entity_id) DO UPDATE SET title = excluded.title,
                   updated_at = excluded.updated_at",
                params![b(id), b(task), b(project), name, w.now()],
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
            return Err(OpError::conflict(
                "The file's content is no longer stored, so it can't come back.",
            )
            .into());
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
    bump(w, owner_task(w.tx(), id)?)?;
    w.record_field(ENTITY, id, project, field, before, value);
    Ok(())
}

/// Removes the blob rows and stored files of `sha256s` that no attachment
/// uses any more. Runs inside the write transaction, so an attachment
/// added at the same time either commits first (and keeps the file) or
/// finds the file gone and stores it again.
pub(crate) fn drop_unused(w: &WriteCtx<'_>, sha256s: &[String]) -> Result<(), Failure> {
    for sha in sha256s {
        let used: Option<i64> = w
            .tx()
            .query_row(
                "SELECT 1 FROM attachments WHERE sha256 = ?1 LIMIT 1",
                [sha],
                |r| r.get(0),
            )
            .optional()?;
        if used.is_some() {
            continue;
        }
        w.tx()
            .execute("DELETE FROM blobs WHERE sha256 = ?1", [sha])?;
        if let Err(e) = gk_store::remove_file(w.layout(), sha) {
            tracing::warn!(error = %e, "a stored file that is no longer used could not be removed");
        }
    }
    Ok(())
}
