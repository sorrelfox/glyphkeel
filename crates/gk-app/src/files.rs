// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Attached files between the app and the page.
//!
//! - **`gk-file:`** shows attached images to the page: `gk-file://localhost/<id>`
//!   (`http://gk-file.localhost/<id>` with WebView2), and their thumbnails at
//!   `<id>/thumb`. It answers only for known attachment IDs, only for images
//!   within the size limits, with the type the engine sniffed from the
//!   content and `nosniff`. Other files are never served to the page.
//! - **The file chooser** and **files dropped on a window** are handled here
//!   in Rust: the page names the task or the wiki page, never a path, so it
//!   can't make the app read a file the person didn't choose.
//! - **Opening and saving** go through the system: a file opens in the
//!   desktop's default app from a copy, and an executable or a script is
//!   never opened, only saved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use gk_api::OpError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::http::{Response, StatusCode, header};
use tauri::ipc::Channel;
use tauri::{AppHandle, DragDropEvent, Runtime, State, WebviewWindow, Window};

use crate::ipc::Workspace;

/// The protocol's scheme.
pub(crate) const SCHEME: &str = "gk-file";

/// The largest image the protocol serves.
const MAX_IMAGE_BYTES: u64 = 50 * 1024 * 1024;
/// The most pixels of an image the protocol serves, when its size is known.
const MAX_IMAGE_PIXELS: u64 = 100_000_000;
/// The largest thumbnail the protocol serves (they are at most 256 pixels a side).
const MAX_THUMB_BYTES: u64 = 4 * 1024 * 1024;
/// Drops kept per window until the page attaches them.
const KEPT_DROPS: usize = 8;

/// An attached file as `file.get` describes it: the fields the app needs.
#[derive(Debug, Deserialize)]
struct FileInfo {
    id: String,
    name: String,
    kind: String,
    mime: String,
    sha256: String,
    size: u64,
    width: Option<u64>,
    height: Option<u64>,
}

fn file_info(workspace: &Workspace, attachment: &str) -> Result<FileInfo, OpError> {
    let value = workspace.call("file.get", json!({ "attachment": attachment }), None)?;
    serde_json::from_value(value)
        .map_err(|e| OpError::storage(format!("The file's details can't be read: {e}.")))
}

/// The stored content of `info`.
fn stored_path(workspace: &Workspace, info: &FileInfo) -> Result<PathBuf, OpError> {
    workspace
        .layout()
        .and_then(|layout| gk_store::file_path(&layout, &info.sha256))
        .ok_or_else(|| {
            OpError::storage(format!("The stored content of {} is missing.", info.name))
                .with_hint("Restore the workspace from a backup.")
        })
}

// ---------- the gk-file: protocol ----------

/// Whether `text` is an attachment ID: a UUID in its usual lower-case form.
fn is_id(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_digit() || ('a'..='f').contains(&c),
        })
}

/// What a request asks for: an attached image, or its thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wanted {
    Image,
    Thumbnail,
}

/// The attachment a request's path names, and what of it: `/<id>` or
/// `/<id>/thumb`, nothing else.
pub(crate) fn attachment_in(path: &str) -> Option<(&str, Wanted)> {
    let rest = path.strip_prefix('/')?;
    let (id, wanted) = match rest.strip_suffix("/thumb") {
        Some(id) => (id, Wanted::Thumbnail),
        None => (rest, Wanted::Image),
    };
    is_id(id).then_some((id, wanted))
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    response
}

/// Answers a `gk-file:` request for the page.
pub(crate) fn respond(workspace: &Workspace, method: &str, path: &str) -> Response<Vec<u8>> {
    if method != "GET" {
        return status(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some((id, wanted)) = attachment_in(path) else {
        return status(StatusCode::NOT_FOUND);
    };
    let info = match file_info(workspace, id) {
        Ok(info) => info,
        Err(err) if err.code == gk_api::ErrorCode::NotFound => {
            return status(StatusCode::NOT_FOUND);
        }
        Err(err) => {
            eprintln!("glyphkeel-app: {SCHEME}: {}", err.message);
            return status(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    // Only images reach the page, and only through <img>: an SVG shown that
    // way never runs script. Every other file opens in its own app.
    if info.kind != "image" || !info.mime.starts_with("image/") {
        return status(StatusCode::FORBIDDEN);
    }
    if wanted == Wanted::Thumbnail {
        return thumbnail(workspace, &info);
    }
    let pixels = info
        .width
        .unwrap_or(0)
        .saturating_mul(info.height.unwrap_or(0));
    if info.size > MAX_IMAGE_BYTES || pixels > MAX_IMAGE_PIXELS {
        return status(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let Some(layout) = workspace.layout() else {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    };
    let bytes = match gk_store::read_file(&layout, &info.sha256, MAX_IMAGE_BYTES) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("glyphkeel-app: {SCHEME}: {}: {err}", info.name);
            return status(StatusCode::NOT_FOUND);
        }
    };
    image_response(&info.mime, bytes)
}

/// The thumbnail of the image `info`, made by the engine (again, when the
/// cache lost it). An image the engine can't decode has none: 404, and the
/// page shows the file's icon.
fn thumbnail(workspace: &Workspace, info: &FileInfo) -> Response<Vec<u8>> {
    let made = workspace
        .call("file.thumbnail", json!({ "attachment": info.id }), None)
        .and_then(|value| {
            serde_json::from_value::<ThumbInfo>(value)
                .map_err(|e| OpError::storage(format!("The thumbnail can't be read: {e}.")))
        });
    let thumb = match made {
        Ok(thumb) => thumb,
        Err(err)
            if matches!(
                err.code,
                gk_api::ErrorCode::NotFound | gk_api::ErrorCode::Invalid
            ) =>
        {
            return status(StatusCode::NOT_FOUND);
        }
        Err(err) => {
            eprintln!("glyphkeel-app: {SCHEME}: {}: {}", info.name, err.message);
            return status(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    // Only the workspace's own thumbnail of this file is read, whatever path
    // the answer names.
    let expected = workspace
        .layout()
        .and_then(|layout| layout.thumb_path(&info.sha256));
    if thumb.mime != "image/png" || expected.as_deref() != Some(Path::new(&thumb.path)) {
        eprintln!(
            "glyphkeel-app: {SCHEME}: {}: unexpected thumbnail",
            info.name
        );
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let bytes = match read_limited(Path::new(&thumb.path), MAX_THUMB_BYTES) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("glyphkeel-app: {SCHEME}: {}: {err}", info.name);
            return status(StatusCode::NOT_FOUND);
        }
    };
    image_response("image/png", bytes)
}

/// What `file.thumbnail` answers: the fields the app needs.
#[derive(Debug, Deserialize)]
struct ThumbInfo {
    path: String,
    mime: String,
}

fn read_limited(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;
    let mut out = Vec::new();
    std::fs::File::open(path)?
        .take(limit)
        .read_to_end(&mut out)?;
    Ok(out)
}

/// An image for `<img>`, of the type the engine named.
fn image_response(mime: &str, bytes: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header("X-Content-Type-Options", "nosniff")
        // An image opened on its own still can't run anything.
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; style-src 'unsafe-inline'; sandbox",
        )
        // The content of an attachment never changes.
        .header(header::CACHE_CONTROL, "private, max-age=86400, immutable")
        .body(bytes)
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

// ---------- attaching ----------

/// What an attach did: the files attached, and those that failed with why.
#[derive(Debug, Default, Serialize)]
pub(crate) struct Attached {
    /// The engine's answers (`FileChange`), in order.
    files: Vec<Value>,
    failed: Vec<AttachFailure>,
}

#[derive(Debug, Serialize)]
struct AttachFailure {
    name: String,
    error: OpError,
}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// What files are attached to: a task (its key) or a wiki page (its ID).
fn owner(task: Option<String>, page: Option<String>) -> Result<Value, OpError> {
    match (task, page) {
        (Some(task), None) => Ok(json!({ "task": task })),
        (None, Some(page)) => Ok(json!({ "page": page })),
        _ => Err(OpError::invalid(
            "Name either the task or the wiki page to attach the files to.",
        )),
    }
}

/// Attaches each of `paths` to `owner`, one command each.
fn attach_paths(workspace: &Workspace, owner: &Value, paths: &[PathBuf]) -> Attached {
    let mut attached = Attached::default();
    for path in paths {
        let result = match path.to_str() {
            Some(text) => {
                let mut input = owner.clone();
                input["path"] = json!(text);
                workspace.call("file.attach", input, None)
            }
            None => Err(OpError::invalid(format!(
                "The path of {} can't be read as text.",
                display_name(path)
            ))),
        };
        match result {
            Ok(change) => attached.files.push(change),
            Err(error) => attached.failed.push(AttachFailure {
                name: display_name(path),
                error,
            }),
        }
    }
    attached
}

/// Lets the person choose files and attaches them to `task` or to wiki
/// page `page`. Choosing nothing attaches nothing.
#[tauri::command]
pub(crate) async fn attach_files(
    window: WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    task: Option<String>,
    page: Option<String>,
) -> Result<Attached, OpError> {
    let owner = owner(task, page)?;
    let paths = dialog::choose(&window, dialog::Mode::Open).await?;
    let workspace = Arc::clone(&workspace);
    blocking(move || Ok(attach_paths(&workspace, &owner, &paths))).await
}

pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, OpError> + Send + 'static,
) -> Result<T, OpError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| OpError::storage(format!("The command stopped unexpectedly: {e}.")))?
}

// ---------- drops from the desktop ----------

/// What the page hears about files dragged over its window. Positions are
/// in CSS pixels; paths never reach the page, only names.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum DropEvent {
    Enter {
        count: usize,
        x: f64,
        y: f64,
    },
    Over {
        x: f64,
        y: f64,
    },
    Leave,
    Drop {
        id: u64,
        names: Vec<String>,
        x: f64,
        y: f64,
    },
}

/// The windows' drop channels and the dropped files waiting to be attached.
#[derive(Default)]
pub(crate) struct Drops(Mutex<DropsInner>);

#[derive(Default)]
struct DropsInner {
    next: u64,
    channels: HashMap<String, Channel<DropEvent>>,
    /// Dropped paths by drop ID, with the window they were dropped on.
    pending: Vec<(u64, String, Vec<PathBuf>)>,
}

impl std::fmt::Debug for Drops {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Drops").finish_non_exhaustive()
    }
}

impl Drops {
    fn lock(&self) -> MutexGuard<'_, DropsInner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps the paths of a drop on `window` and returns its ID.
    fn keep(&self, window: &str, paths: Vec<PathBuf>) -> u64 {
        let mut inner = self.lock();
        inner.next += 1;
        let id = inner.next;
        inner.pending.push((id, window.to_owned(), paths));
        let mine = inner.pending.iter().filter(|(_, w, _)| w == window).count();
        if mine > KEPT_DROPS
            && let Some(oldest) = inner.pending.iter().position(|(_, w, _)| w == window)
        {
            inner.pending.remove(oldest);
        }
        id
    }

    /// Takes the paths of drop `id`, if it was made on `window`.
    fn take(&self, window: &str, id: u64) -> Option<Vec<PathBuf>> {
        let mut inner = self.lock();
        let at = inner
            .pending
            .iter()
            .position(|(i, w, _)| *i == id && w == window)?;
        Some(inner.pending.remove(at).2)
    }

    fn send(&self, window: &str, event: DropEvent) {
        let mut inner = self.lock();
        if let Some(channel) = inner.channels.get(window)
            && channel.send(event).is_err()
        {
            inner.channels.remove(window);
        }
    }

    /// Forgets a closed window.
    pub(crate) fn forget(&self, window: &str) {
        let mut inner = self.lock();
        inner.channels.remove(window);
        inner.pending.retain(|(_, w, _)| w != window);
    }
}

/// Hands over the channel for this window's drop events; a reload replaces
/// it.
#[tauri::command]
pub(crate) fn watch_drops<R: Runtime>(
    window: WebviewWindow<R>,
    drops: State<'_, Drops>,
    channel: Channel<DropEvent>,
) {
    drops
        .lock()
        .channels
        .insert(window.label().to_owned(), channel);
}

/// Attaches the files of drop `drop` to `task` or to wiki page `page`. A
/// drop can be attached once, only by the window it was made on.
#[tauri::command]
pub(crate) async fn attach_drop(
    window: WebviewWindow,
    drops: State<'_, Drops>,
    workspace: State<'_, Arc<Workspace>>,
    drop: u64,
    task: Option<String>,
    page: Option<String>,
) -> Result<Attached, OpError> {
    let owner = owner(task, page)?;
    let paths = drops.take(window.label(), drop).ok_or_else(|| {
        OpError::not_found("Those dropped files are no longer known.", &[])
            .with_hint("Drop them again.")
    })?;
    let workspace = Arc::clone(&workspace);
    blocking(move || Ok(attach_paths(&workspace, &owner, &paths))).await
}

/// Tauri's drop handler, for every window: tells the page, keeping the
/// paths in Rust.
pub(crate) fn on_drag_drop<R: Runtime>(window: &Window<R>, event: &DragDropEvent) {
    use tauri::Manager;

    let Some(drops) = window.try_state::<Drops>() else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let at = |p: &tauri::PhysicalPosition<f64>| (p.x / scale, p.y / scale);
    let label = window.label();
    let event = match event {
        DragDropEvent::Enter { paths, position } => {
            let (x, y) = at(position);
            DropEvent::Enter {
                count: paths.len(),
                x,
                y,
            }
        }
        DragDropEvent::Over { position } => {
            let (x, y) = at(position);
            DropEvent::Over { x, y }
        }
        DragDropEvent::Drop { paths, position } => {
            let (x, y) = at(position);
            let names = paths.iter().map(|p| display_name(p)).collect();
            let id = drops.keep(label, paths.clone());
            DropEvent::Drop { id, names, x, y }
        }
        _ => DropEvent::Leave,
    };
    drops.send(label, event);
}

// ---------- opening and saving ----------

/// The last part of a stored file name, safe as a file name of its own on
/// this platform.
pub(crate) fn safe_name(name: &str) -> String {
    safe_name_for(name, cfg!(windows))
}

/// Device names Windows reserves in every folder, with any extension.
const WINDOWS_DEVICES: &[&str] = &[
    "con", "prn", "aux", "nul", "conin$", "conout$", "com0", "com1", "com2", "com3", "com4",
    "com5", "com6", "com7", "com8", "com9", "lpt0", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6",
    "lpt7", "lpt8", "lpt9",
];

/// [`safe_name`], with Windows' rules when `windows`: no `:` (it would name
/// a stream of another file), none of `<>"|?*`, no trailing dots or spaces
/// (Windows drops them, so `x.exe.` would become `x.exe`), and no device
/// names.
fn safe_name_for(name: &str, windows: bool) -> String {
    let base = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control()
                || matches!(c, '/' | '\\')
                || (windows && matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*'))
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    if windows {
        cleaned.truncate(cleaned.trim_end_matches(['.', ' ']).len());
        let stem = cleaned.split('.').next().unwrap_or_default().trim_end();
        if WINDOWS_DEVICES.contains(&stem.to_lowercase().as_str()) {
            cleaned.insert(0, '_');
        }
    }
    if cleaned.trim_matches('.').is_empty() {
        "file".to_owned()
    } else {
        cleaned
    }
}

/// Types that Windows runs, installs, mounts or lets change the system when
/// opened, beyond the programs and scripts the engine already marks as
/// such.
const WINDOWS_RISKY: &[&str] = &[
    "ade",
    "adp",
    "appref-ms",
    "application",
    "appx",
    "appxbundle",
    "chm",
    "cpl",
    "crt",
    "dll",
    "diagcab",
    "drv",
    "gadget",
    "hlp",
    "hta",
    "img",
    "inf",
    "iso",
    "mde",
    "msc",
    "msh",
    "mshxml",
    "msix",
    "msixbundle",
    "msp",
    "mst",
    "ocx",
    "pif",
    "ps1xml",
    "ps2",
    "psc1",
    "psd1",
    "psm1",
    "pyc",
    "pyw",
    "reg",
    "scf",
    "searchconnector-ms",
    "settingcontent-ms",
    "library-ms",
    "sys",
    "url",
    "vbe",
    "vhd",
    "vhdx",
    "website",
    "ws",
    "wsc",
    "wsh",
    "xbap",
    "xll",
];

/// Whether Windows would run or install file `name` (a name from
/// [`safe_name`]) rather than show it.
fn windows_risky(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        WINDOWS_RISKY.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// Whether file `name` may be opened in its default program here: never
/// what the engine marks as a program or script, and on Windows also none
/// of the types Windows runs or installs.
fn may_open(name: &str, kind: &str) -> bool {
    kind != "executable" && !(cfg!(windows) && windows_risky(&safe_name(name)))
}

/// Opens an attached file in the desktop's default app, from a copy in the
/// workspace's cache. Executables and scripts are refused: save them
/// instead.
#[tauri::command]
pub(crate) async fn open_file(
    app: AppHandle,
    workspace: State<'_, Arc<Workspace>>,
    attachment: String,
) -> Result<(), OpError> {
    let workspace = Arc::clone(&workspace);
    let copy = blocking(move || {
        let info = file_info(&workspace, &attachment)?;
        if !may_open(&info.name, &info.kind) {
            return Err(OpError::forbidden(format!(
                "{} can run programs, so it isn't opened.",
                info.name
            ))
            .with_hint("Save it, and look at it first.")
            .with_detail("kind", info.kind));
        }
        let source = stored_path(&workspace, &info)?;
        let layout = workspace
            .layout()
            .ok_or_else(|| OpError::storage("The workspace is closed."))?;
        let folder = layout.cache_dir().join("open").join(&info.id);
        let target = folder.join(safe_name(&info.name));
        std::fs::create_dir_all(&folder)
            .and_then(|()| std::fs::copy(&source, &target))
            .map_err(|e| {
                OpError::storage(format!("{} can't be copied to open it: {e}.", info.name))
            })?;
        Ok(target)
    })
    .await?;
    system::open(&app, &copy).await
}

/// Saves a copy of an attached file where the person chooses. Returns
/// whether it was saved.
#[tauri::command]
pub(crate) async fn save_file(
    window: WebviewWindow,
    workspace: State<'_, Arc<Workspace>>,
    attachment: String,
) -> Result<bool, OpError> {
    let workspace = Arc::clone(&workspace);
    let info_workspace = Arc::clone(&workspace);
    let id = attachment.clone();
    let info = blocking(move || file_info(&info_workspace, &id)).await?;
    let chosen = dialog::choose(&window, dialog::Mode::Save(safe_name(&info.name))).await?;
    let Some(target) = chosen.into_iter().next() else {
        return Ok(false);
    };
    blocking(move || {
        let source = stored_path(&workspace, &info)?;
        std::fs::copy(&source, &target)
            .map(|_| true)
            .map_err(|e| OpError::storage(format!("{} can't be saved there: {e}.", info.name)))
    })
    .await
}

/// The largest export the page may hand over to be saved.
const EXPORT_MAX_BYTES: usize = 256 * 1024 * 1024;

/// Saves an export the page made (the list's tasks as CSV or JSON, the
/// Gantt chart as a PNG picture) where the person chooses, starting with
/// `name`. A picture comes as base64 text (`base64: true`). Returns whether
/// it was saved.
#[tauri::command]
pub(crate) async fn save_export(
    window: WebviewWindow,
    name: String,
    contents: String,
    base64: Option<bool>,
) -> Result<bool, OpError> {
    if contents.len() > EXPORT_MAX_BYTES {
        return Err(OpError::invalid("The export is too large to save."));
    }
    let bytes = if base64 == Some(true) {
        decode_base64(&contents).ok_or_else(|| OpError::invalid("The export can't be read."))?
    } else {
        contents.into_bytes()
    };
    let chosen = dialog::choose(&window, dialog::Mode::Save(safe_name(&name))).await?;
    let Some(target) = chosen.into_iter().next() else {
        return Ok(false);
    };
    blocking(move || {
        std::fs::write(&target, &bytes)
            .map(|()| true)
            .map_err(|e| OpError::storage(format!("{name} can't be saved there: {e}.")))
    })
    .await
}

/// Standard base64 with padding, as the page's `btoa` writes it; `None`
/// for anything else.
fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const fn value(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let quads = bytes.len() / 4;
    for (i, quad) in bytes.as_chunks::<4>().0.iter().enumerate() {
        let last = i + 1 == quads;
        let pad = if last {
            quad.iter().rev().take_while(|&&c| c == b'=').count()
        } else {
            0
        };
        if pad > 2 {
            return None;
        }
        let mut n = 0u32;
        for &c in &quad[..4 - pad] {
            n = (n << 6) | value(c)?;
        }
        n <<= 6 * pad as u32;
        let three = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&three[..3 - pad]);
    }
    Some(out)
}

// ---------- the platform's dialogs and launcher ----------

/// What the person chooses in a file chooser.
pub(crate) enum Mode {
    /// Choose files to read.
    Open,
    /// Choose where to write, starting with this name.
    Save(String),
    /// Choose where to write an export archive (a `.zip` file, or a new
    /// folder when `folder`), starting with this name.
    Export { name: String, folder: bool },
    /// Choose an archive to import: a `.zip` file, or a folder.
    Import { folder: bool },
    /// Choose the folder backups go to.
    BackupFolder,
    /// Choose a folder of Markdown files to import into the wiki.
    WikiFolder,
}

/// What kind of chooser a [`Mode`] shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choose {
    /// Several files to read.
    Files,
    /// One file to read.
    File,
    /// Where to write a file, asking before it replaces one.
    Save,
    /// A folder.
    Folder,
}

/// A file chooser as every platform shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Spec {
    pub(crate) choose: Choose,
    /// Message IDs of the title and of the confirming button.
    pub(crate) title: &'static str,
    /// (The system's chooser on Windows names its own button.)
    pub(crate) accept: &'static str,
    /// The name to start with, when saving.
    pub(crate) name: Option<String>,
    /// Whether only `.zip` archives are shown.
    pub(crate) zip_only: bool,
}

impl Mode {
    pub(crate) fn spec(self) -> Spec {
        let spec = |choose, title, accept| Spec {
            choose,
            title,
            accept,
            name: None,
            zip_only: false,
        };
        match self {
            Mode::Open => spec(Choose::Files, "chooser-attach-title", "chooser-attach"),
            Mode::Save(name) => Spec {
                name: Some(name),
                ..spec(Choose::Save, "chooser-save-title", "chooser-save")
            },
            Mode::Export {
                name,
                folder: false,
            } => Spec {
                name: Some(name),
                zip_only: true,
                ..spec(Choose::Save, "chooser-export-title", "chooser-export")
            },
            // A new folder is named like a file; the export creates it.
            Mode::Export { name, folder: true } => Spec {
                name: Some(name),
                ..spec(
                    Choose::Save,
                    "chooser-export-folder-title",
                    "chooser-export",
                )
            },
            Mode::Import { folder: false } => Spec {
                zip_only: true,
                ..spec(Choose::File, "chooser-import-title", "chooser-import")
            },
            Mode::Import { folder: true } => spec(
                Choose::Folder,
                "chooser-import-folder-title",
                "chooser-import",
            ),
            Mode::BackupFolder => spec(
                Choose::Folder,
                "chooser-backup-folder-title",
                "chooser-backup-folder",
            ),
            Mode::WikiFolder => spec(
                Choose::Folder,
                "chooser-wiki-folder-title",
                "chooser-import",
            ),
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) mod dialog {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use gk_api::OpError;
    use gtk::prelude::*;
    use gtk::{FileChooserAction, FileChooserNative, ResponseType};
    use tauri::WebviewWindow;

    pub(crate) use super::Mode;
    use super::{Choose, Spec};
    use crate::messages::text;

    /// Shows GTK's native file chooser (the desktop portal's, where there is
    /// one) over `window`, and waits for the answer. Cancel gives no paths.
    pub(crate) async fn choose(
        window: &WebviewWindow,
        mode: Mode,
    ) -> Result<Vec<PathBuf>, OpError> {
        let (answer, answered) = std::sync::mpsc::channel();
        let target = window.clone();
        window
            .run_on_main_thread(move || show(&target, mode, answer))
            .map_err(|e| OpError::storage(format!("The file chooser can't be shown: {e}.")))?;
        super::blocking(move || Ok(answered.recv().unwrap_or_default())).await
    }

    fn show(window: &WebviewWindow, mode: Mode, answer: std::sync::mpsc::Sender<Vec<PathBuf>>) {
        let parent = window.gtk_window().ok();
        let Spec {
            choose,
            title,
            accept,
            name,
            zip_only,
        } = mode.spec();
        let action = match choose {
            Choose::Files | Choose::File => FileChooserAction::Open,
            Choose::Save => FileChooserAction::Save,
            Choose::Folder => FileChooserAction::SelectFolder,
        };
        let dialog = FileChooserNative::new(
            Some(&text(title, &[])),
            parent.as_ref(),
            action,
            Some(&text(accept, &[])),
            Some(&text("chooser-cancel", &[])),
        );
        dialog.set_modal(true);
        if choose == Choose::Files {
            dialog.set_select_multiple(true);
        }
        if let Some(name) = name {
            dialog.set_current_name(&name);
        }
        if zip_only {
            let filter = gtk::FileFilter::new();
            filter.set_name(Some(&text("chooser-zip-filter", &[])));
            filter.add_pattern("*.zip");
            filter.add_mime_type("application/zip");
            dialog.add_filter(filter);
        }
        if choose == Choose::Save {
            dialog.set_do_overwrite_confirmation(true);
        }
        // The dialog lives until it answers; then the answer is sent once.
        let keep = RefCell::new(Some((dialog.clone(), answer)));
        dialog.connect_response(move |dialog, response| {
            let paths = if response == ResponseType::Accept {
                dialog.filenames()
            } else {
                Vec::new()
            };
            if let Some((_, answer)) = keep.borrow_mut().take() {
                let _ = answer.send(paths);
            }
        });
        dialog.show();
    }
}

#[cfg(windows)]
pub(crate) mod dialog {
    pub(crate) use super::Mode;
    pub(crate) use crate::platform::windows::choose;
}

#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) mod dialog {
    use std::path::PathBuf;

    use gk_api::OpError;
    use tauri::WebviewWindow;

    pub(crate) use super::Mode;

    pub(crate) async fn choose(
        _window: &WebviewWindow,
        _mode: Mode,
    ) -> Result<Vec<PathBuf>, OpError> {
        Err(OpError::forbidden(
            "Files can't be chosen on this platform yet.",
        ))
    }
}

#[cfg(target_os = "linux")]
mod system {
    use std::path::Path;

    use gk_api::OpError;
    use gtk::gio;
    use tauri::AppHandle;

    /// Opens `path` in the desktop's default app for its type (through the
    /// portal inside a sandbox).
    pub(crate) async fn open(app: &AppHandle, path: &Path) -> Result<(), OpError> {
        let uri = gtk::glib::filename_to_uri(path, None)
            .map_err(|e| OpError::storage(format!("The file can't be named for opening: {e}.")))?
            .to_string();
        let (answer, answered) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let result = gio::AppInfo::launch_default_for_uri(
                &uri,
                Some(&crate::memory_env::launch_context()),
            )
            .map_err(|e| e.to_string());
            let _ = answer.send(result);
        })
        .map_err(|e| OpError::storage(format!("The file can't be opened: {e}.")))?;
        super::blocking(move || {
            answered
                .recv()
                .unwrap_or_else(|_| Err("the app is quitting".to_owned()))
                .map_err(|e| {
                    OpError::storage(format!("No app opened the file: {e}."))
                        .with_hint("Save it and open it yourself.")
                })
        })
        .await
    }
}

#[cfg(windows)]
mod system {
    use std::path::Path;

    use gk_api::OpError;
    use tauri::AppHandle;

    /// Opens `path` in the default program for its type. The copy is first
    /// marked as coming from elsewhere, so that Windows and the program
    /// treat it with the care they give a download.
    pub(crate) async fn open(_app: &AppHandle, path: &Path) -> Result<(), OpError> {
        let path = path.to_owned();
        super::blocking(move || {
            crate::platform::windows::mark_as_downloaded(&path);
            crate::platform::windows::open_path(&path).map_err(|e| {
                OpError::storage(format!("No app opened the file: {e}."))
                    .with_hint("Save it and open it yourself.")
            })
        })
        .await
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod system {
    use std::path::Path;

    use gk_api::OpError;
    use tauri::AppHandle;

    pub(crate) async fn open(_app: &AppHandle, _path: &Path) -> Result<(), OpError> {
        Err(OpError::forbidden(
            "Files can't be opened on this platform yet.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gk_engine::{Confirmation, EngineConfig, Role};
    use gk_store::WorkspaceLayout;
    use tempfile::TempDir;

    #[test]
    fn base64_exports_decode_and_damaged_ones_are_refused() {
        assert_eq!(decode_base64("").unwrap(), b"");
        assert_eq!(decode_base64("Zg==").unwrap(), b"f");
        assert_eq!(decode_base64("Zm8=").unwrap(), b"fo");
        assert_eq!(decode_base64("Zm9v").unwrap(), b"foo");
        assert_eq!(decode_base64("iVBORw0KGgo=").unwrap(), b"\x89PNG\r\n\x1a\n");
        assert!(decode_base64("Zm9").is_none());
        assert!(decode_base64("Zm=v").is_none());
        assert!(decode_base64("Z===").is_none());
        assert!(decode_base64("Zm9v!A==").is_none());
    }

    /// The smallest PNG: one transparent pixel.
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

    fn workspace(dir: &TempDir) -> Workspace {
        let ws = Workspace::open(
            &WorkspaceLayout::new(dir.path().join("Personal")),
            EngineConfig::new(Role::App),
            Confirmation::Refuse,
        );
        ws.call(
            "project.create",
            json!({ "name": "Demo", "key": "GS" }),
            None,
        )
        .unwrap();
        ws.call(
            "task.create",
            json!({ "project": "GS", "title": "One" }),
            None,
        )
        .unwrap();
        ws
    }

    fn attach(ws: &Workspace, name: &str, base64: &str) -> String {
        let change = ws
            .call(
                "file.attach",
                json!({ "task": "GS-1", "name": name, "data_base64": base64 }),
                None,
            )
            .unwrap();
        change["file"]["id"].as_str().unwrap().to_owned()
    }

    #[test]
    fn only_an_attachment_id_or_its_thumbnail_is_a_path() {
        let id = "01a208d6-2413-7095-976e-c39bc5e073ec";
        assert_eq!(attachment_in(&format!("/{id}")), Some((id, Wanted::Image)));
        assert_eq!(
            attachment_in(&format!("/{id}/thumb")),
            Some((id, Wanted::Thumbnail))
        );
        for path in [
            "/",
            "",
            id,
            "/../workspace.db",
            "/01A208D6-2413-7095-976E-C39BC5E073EC",
            "/01a208d6-2413-7095-976e-c39bc5e073ec/thumb/",
            "/01a208d6-2413-7095-976e-c39bc5e073ec/full",
            "/01a208d6-2413-7095-976e-c39bc5e073ec/../thumb",
            "/thumb",
            "/01a208d6x2413-7095-976e-c39bc5e073ec",
        ] {
            assert_eq!(attachment_in(path), None, "{path}");
        }
    }

    #[test]
    fn images_are_served_with_their_sniffed_type_and_nosniff() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);
        // The name says text; the content is a PNG.
        let id = attach(&ws, "picture.txt", PNG);
        let response = respond(&ws, "GET", &format!("/{id}"));
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.headers()["X-Content-Type-Options"], "nosniff");
        assert!(
            response.headers()[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("sandbox")
        );
        assert!(response.body().starts_with(b"\x89PNG"));
        ws.close();
    }

    #[test]
    fn thumbnails_are_served_as_png_and_rebuilt_when_the_cache_lost_them() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);
        let id = attach(&ws, "dot.png", PNG);
        let response = respond(&ws, "GET", &format!("/{id}/thumb"));
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.headers()["X-Content-Type-Options"], "nosniff");
        assert!(response.body().starts_with(b"\x89PNG"));
        // The cache is rebuildable: without it the thumbnail is made again.
        std::fs::remove_dir_all(WorkspaceLayout::new(dir.path().join("Personal")).thumbs_dir())
            .unwrap();
        let again = respond(&ws, "GET", &format!("/{id}/thumb"));
        assert_eq!(again.status(), StatusCode::OK);
        assert!(again.body().starts_with(b"\x89PNG"));
        ws.close();
    }

    #[test]
    fn thumbnails_of_other_files_unknown_ids_and_other_methods_are_refused() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);
        let text = attach(&ws, "notes.txt", "aGVsbG8K");
        let image = attach(&ws, "dot.png", PNG);
        assert_eq!(
            respond(&ws, "GET", &format!("/{text}/thumb")).status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            respond(&ws, "GET", "/01a208d6-2413-7095-976e-c39bc5e073ec/thumb").status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            respond(&ws, "POST", &format!("/{image}/thumb")).status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
        ws.close();
    }

    #[test]
    fn other_files_unknown_ids_and_other_methods_are_refused() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);
        // "<script>alert(1)</script>" as an HTML file.
        let html = attach(&ws, "page.html", "PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==");
        let text = attach(&ws, "notes.txt", "aGVsbG8K");
        let image = attach(&ws, "dot.png", PNG);
        assert_eq!(
            respond(&ws, "GET", &format!("/{html}")).status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            respond(&ws, "GET", &format!("/{text}")).status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            respond(&ws, "GET", "/01a208d6-2413-7095-976e-c39bc5e073ec").status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            respond(&ws, "GET", "/workspace.db").status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            respond(&ws, "POST", &format!("/{image}")).status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
        ws.close();
    }

    #[test]
    fn dropped_paths_are_taken_once_by_their_own_window() {
        let drops = Drops::default();
        let id = drops.keep("main", vec![PathBuf::from("/tmp/a.txt")]);
        assert_eq!(drops.take("task-GS-1", id), None);
        assert_eq!(
            drops.take("main", id),
            Some(vec![PathBuf::from("/tmp/a.txt")])
        );
        assert_eq!(drops.take("main", id), None);
        for _ in 0..=KEPT_DROPS {
            drops.keep("main", Vec::new());
        }
        assert_eq!(drops.lock().pending.len(), KEPT_DROPS);
    }

    #[test]
    fn attaching_paths_reports_each_failure_by_name() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "hello\n").unwrap();
        let task = owner(Some("GS-1".into()), None).unwrap();
        let attached = attach_paths(&ws, &task, &[file.clone(), dir.path().join("missing.txt")]);
        assert_eq!(attached.files.len(), 1);
        assert_eq!(attached.failed.len(), 1);
        assert_eq!(attached.failed[0].name, "missing.txt");
        // A wiki page takes files the same way.
        let page = ws
            .call(
                "wiki.create",
                json!({ "project": "GS", "title": "Notes" }),
                None,
            )
            .unwrap()["page"]["id"]
            .clone();
        let page = owner(None, Some(page.as_str().unwrap().to_owned())).unwrap();
        let attached = attach_paths(&ws, &page, &[file]);
        assert_eq!(attached.files.len(), 1, "{:?}", attached.failed);
        assert!(attached.files[0]["file"]["page"].is_string());
        assert!(owner(None, None).is_err());
        assert!(owner(Some("GS-1".into()), Some("x".into())).is_err());
        ws.close();
    }

    #[test]
    fn saved_and_opened_copies_get_a_plain_file_name() {
        assert_eq!(safe_name("report.pdf"), "report.pdf");
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name(".."), "file");
        assert_eq!(safe_name("a\u{7}b.txt"), "a_b.txt");
        for windows in [false, true] {
            assert_eq!(safe_name_for("report.pdf", windows), "report.pdf");
            assert_eq!(safe_name_for("../../etc/passwd", windows), "passwd");
            assert_eq!(safe_name_for("...", windows), "file");
        }
    }

    #[test]
    fn copies_on_windows_follow_its_file_name_rules() {
        // A colon would name a stream of `a.txt`; the others aren't allowed.
        assert_eq!(safe_name_for("a.txt:evil.exe", true), "a.txt_evil.exe");
        assert_eq!(safe_name_for("what?<x>|\"*\".md", true), "what__x_____.md");
        // Windows drops trailing dots and spaces, so they go first.
        assert_eq!(safe_name_for("run.exe. . ", true), "run.exe");
        assert_eq!(safe_name_for("CON.txt", true), "_CON.txt");
        assert_eq!(safe_name_for("lpt1", true), "_lpt1");
        assert_eq!(safe_name_for("console.txt", true), "console.txt");
        // Elsewhere these names are fine as they are.
        assert_eq!(safe_name_for("ab:c. ", false), "ab:c. ");
        assert_eq!(safe_name_for("CON.txt", false), "CON.txt");
    }

    #[test]
    fn types_that_windows_runs_or_installs_are_never_opened_there() {
        for name in [
            "x.hta",
            "x.msc",
            "x.CPL",
            "x.reg",
            "x.url",
            "x.iso",
            "x.vhdx",
            "x.chm",
            "x.settingcontent-ms",
            "x.appref-ms",
            "x.psm1",
            "x.msix",
            "x.library-ms",
        ] {
            assert!(windows_risky(name), "{name}");
        }
        // The trailing dot is gone by the time the copy is named.
        assert!(windows_risky(&safe_name_for("x.hta.", true)));
        for name in ["x.pdf", "x.png", "x.docx", "notes.txt", "hta", "x.url.txt"] {
            assert!(!windows_risky(name), "{name}");
        }
        assert!(!may_open("run.exe", "executable"));
        assert!(may_open("report.pdf", "document"));
        assert_eq!(may_open("x.hta", "document"), !cfg!(windows));
    }

    #[test]
    fn every_chooser_has_its_kind_title_and_filter() {
        let spec = Mode::Open.spec();
        assert_eq!(
            (
                spec.choose,
                spec.title,
                spec.accept,
                spec.name,
                spec.zip_only
            ),
            (
                Choose::Files,
                "chooser-attach-title",
                "chooser-attach",
                None,
                false
            )
        );
        let spec = Mode::Save("a.pdf".into()).spec();
        assert_eq!(
            (spec.choose, spec.name.as_deref(), spec.zip_only),
            (Choose::Save, Some("a.pdf"), false)
        );
        let spec = Mode::Export {
            name: "x.zip".into(),
            folder: false,
        }
        .spec();
        assert_eq!(
            (spec.choose, spec.title, spec.zip_only),
            (Choose::Save, "chooser-export-title", true)
        );
        let spec = Mode::Export {
            name: "x".into(),
            folder: true,
        }
        .spec();
        assert_eq!(
            (spec.choose, spec.title, spec.zip_only),
            (Choose::Save, "chooser-export-folder-title", false)
        );
        let spec = Mode::Import { folder: false }.spec();
        assert_eq!((spec.choose, spec.zip_only), (Choose::File, true));
        for (mode, title) in [
            (Mode::Import { folder: true }, "chooser-import-folder-title"),
            (Mode::BackupFolder, "chooser-backup-folder-title"),
            (Mode::WikiFolder, "chooser-wiki-folder-title"),
        ] {
            let spec = mode.spec();
            assert_eq!(
                (spec.choose, spec.title, spec.zip_only),
                (Choose::Folder, title, false)
            );
        }
        // Every title, button and filter has its message.
        for mode in [
            Mode::Open,
            Mode::Save(String::new()),
            Mode::Export {
                name: String::new(),
                folder: false,
            },
            Mode::Export {
                name: String::new(),
                folder: true,
            },
            Mode::Import { folder: false },
            Mode::Import { folder: true },
            Mode::BackupFolder,
            Mode::WikiFolder,
        ] {
            let spec = mode.spec();
            for id in [
                spec.title,
                spec.accept,
                "chooser-zip-filter",
                "chooser-cancel",
            ] {
                assert_ne!(crate::messages::text(id, &[]), id);
            }
        }
    }
}
