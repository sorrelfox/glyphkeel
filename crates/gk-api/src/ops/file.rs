// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Attached files.
//!
//! A file is attached to a task or a comment, and named by the
//! attachment's ID. The same content attached twice is stored once. Its type
//! is read from its bytes, never taken from its name.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{Exposure, Meta, OpDescriptor, OpKind, Safety};
use crate::ops::common::Actor;

/// What a file holds, which decides how it is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    /// A picture shown inline.
    Image,
    /// Text in UTF-8.
    Text,
    /// A program or script: only ever saved, never opened or shown.
    Executable,
    /// Anything else.
    Other,
}

/// One attached file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    /// The attachment's ID.
    pub id: String,
    /// The file name.
    pub name: String,
    /// The media type, read from the content.
    pub mime: String,
    /// What it holds.
    pub kind: FileKind,
    /// The size in bytes.
    pub size: u64,
    /// The content's SHA-256.
    pub sha256: String,
    /// An image's width in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// An image's height in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// The task's key.
    pub task: String,
    /// The comment's ID, for a file attached to a comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Who attached it.
    pub added_by: Actor,
    /// When it was attached (RFC 3339, UTC).
    pub created_at: String,
}

/// A file to attach: from a local path, or as Base64 data with a name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileAttachInput {
    /// The task to attach it to: its key or ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The comment to attach it to: its ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// The path of a local file to read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The file name; default: the path's last part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The content as Base64, instead of `path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
    /// Your own ID for this request; repeating it within 24 hours returns
    /// the first answer instead of attaching the file again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_request_id: Option<String>,
}

/// Names one attached file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    /// The attachment's ID.
    pub attachment: String,
}

/// An attached file's content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileContent {
    /// The file.
    pub file: Attachment,
    /// The text, for a text file (at most 256 KB).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The content as Base64, for an image (at most 5 MB).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
    /// Whether the text was cut at the limit.
    #[serde(default)]
    pub truncated: bool,
    /// Why no content is given, for other files and large images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A thumbnail of an attached image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Thumbnail {
    /// The attachment's ID.
    pub attachment: String,
    /// The thumbnail's file in the workspace's cache.
    pub path: String,
    /// Its media type: `image/png`.
    pub mime: String,
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
}

/// A file attached or deleted, with the history group of the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileChange {
    /// The file.
    pub file: Attachment,
    /// Whether it is in the trash now.
    #[serde(default)]
    pub deleted: bool,
    /// The history group, for revert and Undo; left out when nothing
    /// changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

op!(
    /// An attached file's details.
    FileGet,
    "file.get",
    FileRef,
    Attachment
);
op!(
    /// An attached file's content.
    FileRead,
    "file.read",
    FileRef,
    FileContent
);
op!(
    /// Attaches a file.
    FileAttach,
    "file.attach",
    FileAttachInput,
    FileChange
);
op!(
    /// An attached file to the trash.
    FileDelete,
    "file.delete",
    FileRef,
    FileChange
);
op!(
    /// A thumbnail of an attached image.
    FileThumbnail,
    "file.thumbnail",
    FileRef,
    Thumbnail
);

pub(crate) const FILE_GET: OpDescriptor = OpDescriptor::of::<FileGet>(Meta {
    summary: "An attached file's name, type, size and owner.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("file show <attachment>"),
    input_name: "FileRef",
    output_name: "Attachment",
});

pub(crate) const FILE_READ: OpDescriptor = OpDescriptor::of::<FileRead>(Meta {
    summary: "An attached file's content: text for text files, Base64 for images, otherwise only its details.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::Data,
    cli: Some("file cat <attachment>"),
    input_name: "FileRef",
    output_name: "FileContent",
});

pub(crate) const FILE_ATTACH: OpDescriptor = OpDescriptor::of::<FileAttach>(Meta {
    summary: "Attach a local file, or Base64 data with a name, to a task or a comment.",
    kind: OpKind::Command,
    safety: Safety::Write,
    exposure: Exposure::Data,
    cli: Some("file attach"),
    input_name: "FileAttachInput",
    output_name: "FileChange",
});

pub(crate) const FILE_DELETE: OpDescriptor = OpDescriptor::of::<FileDelete>(Meta {
    summary: "Move an attached file to the trash.",
    kind: OpKind::Command,
    safety: Safety::Trash,
    exposure: Exposure::Data,
    cli: Some("file delete <attachment>"),
    input_name: "FileRef",
    output_name: "FileChange",
});

pub(crate) const FILE_THUMBNAIL: OpDescriptor = OpDescriptor::of::<FileThumbnail>(Meta {
    summary: "A small PNG of an attached PNG, JPEG, GIF or WebP image, at most 256 pixels wide and high, from the workspace's cache; made when the image is attached, and again when the cache has lost it.",
    kind: OpKind::Query,
    safety: Safety::Read,
    exposure: Exposure::App {
        reason: "It gives a path in the workspace's cache, which the app serves to its pages; agents and the command line read images with file.read.",
    },
    cli: None,
    input_name: "FileRef",
    output_name: "Thumbnail",
});
