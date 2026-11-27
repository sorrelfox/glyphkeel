// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The file store: attached files, each stored once under its SHA-256 in
//! `files/<first two digits>/<sha256>`.
//!
//! A file is written to `files/tmp/` first, flushed to disk, then renamed to
//! its final name, and the folder is flushed too. A crash leaves either the
//! complete file or none (and perhaps a stray temporary file). Stored files
//! never change: the same content always has the same name, so writing it
//! again is harmless and several processes can store the same file at once.
//!
//! Thumbnails live in the rebuildable cache, named by the hash of the file
//! they show. They are written the same way but not flushed: losing one only
//! means making it again.
//!
//! In an encrypted workspace both are sealed (see the `sealed` module): the
//! name is still the SHA-256 of the content, and every read here goes
//! through [`open_file`], which opens plain and sealed files alike.

use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::layout::WorkspaceLayout;
use crate::sealed::{Content, FileKey, SealWriter};

/// The key that seals new stored files and thumbnails of the workspace at
/// `layout`: `None` for a plain workspace, or one this process hasn't
/// opened a store for.
#[must_use]
pub fn workspace_seals(layout: &WorkspaceLayout) -> Option<FileKey> {
    crate::sealed::workspace_keys(layout.root()).and_then(|(current, _)| current)
}

/// Whether the file at `path` holds `size` bytes of content, sealed or not.
fn holds(path: &Path, size: u64) -> bool {
    crate::sealed::content_len(path).is_ok_and(|n| n == Some(size))
}

/// Writes `bytes` to the new file `path`, sealed with `key` when given,
/// and flushes it to disk when `sync`.
fn write_new(path: &Path, bytes: &[u8], key: Option<&FileKey>, sync: bool) -> io::Result<()> {
    let file = File::create(path)?;
    let file = match key {
        Some(key) => {
            let mut sealed = SealWriter::new(io::BufWriter::new(file), key)?;
            sealed.write_all(bytes)?;
            sealed
                .finish()?
                .into_inner()
                .map_err(io::IntoInnerError::into_error)?
        }
        None => {
            let mut file = file;
            file.write_all(bytes)?;
            file
        }
    };
    if sync {
        file.sync_all()?;
    }
    Ok(())
}

/// A file in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    /// Its SHA-256, 64 lower-case hexadecimal digits.
    pub sha256: String,
    /// Its size in bytes.
    pub size: u64,
}

/// The SHA-256 of `bytes` as 64 lower-case hexadecimal digits.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    gk_core::hex(&Sha256::digest(bytes))
}

fn temp_name() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}.part",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// Flushes a folder's entries to disk, so a rename inside it survives a
/// crash. Does nothing where folders can't be opened (Windows).
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(dir)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

/// Opens a file that is already written in order to flush it to disk.
///
/// The handle must be open for writing: Windows flushes only through a
/// handle that may write and refuses a read-only one with "Access denied".
/// Nothing is written through it.
fn open_to_flush(path: &Path) -> io::Result<File> {
    fs::OpenOptions::new().write(true).open(path)
}

/// Flushes a written file's contents to disk.
pub(crate) fn sync_file(path: &Path) -> io::Result<()> {
    open_to_flush(path)?.sync_all()
}

/// Stores `bytes` in the workspace's file store and returns its hash and
/// size. A file that is already there with the right size is kept as is.
pub fn store_file(layout: &WorkspaceLayout, bytes: &[u8]) -> io::Result<StoredFile> {
    let sha256 = sha256_hex(bytes);
    let size = bytes.len() as u64;
    let target = layout
        .blob_path(&sha256)
        .ok_or_else(|| io::Error::other("a computed hash is not a hash"))?;
    if holds(&target, size) {
        return Ok(StoredFile { sha256, size });
    }
    let folder = target
        .parent()
        .ok_or_else(|| io::Error::other("a stored file has no folder"))?
        .to_path_buf();
    fs::create_dir_all(&folder)?;
    let tmp_dir = layout.files_dir().join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    let tmp = tmp_dir.join(temp_name());
    let key = workspace_seals(layout);
    let written = (|| {
        write_new(&tmp, bytes, key.as_ref(), true)?;
        fs::rename(&tmp, &target)?;
        sync_dir(&folder)
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(StoredFile { sha256, size })
}

/// The SHA-256 and size of the file at `path`, read in pieces.
pub fn sha256_file(path: &Path) -> io::Result<StoredFile> {
    sha256_reader(&mut File::open(path)?)
}

/// The SHA-256 and size of the content of the stored file at `path`
/// (in the workspace or a backup), sealed or not: checks that it opens and
/// is what its name says.
pub fn sha256_stored(path: &Path) -> io::Result<StoredFile> {
    sha256_reader(&mut crate::sealed::open_stored(path)?)
}

/// The SHA-256 and size of everything `reader` gives, read in pieces.
pub fn sha256_reader(file: &mut dyn io::Read) -> io::Result<StoredFile> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    Ok(StoredFile {
        sha256: gk_core::hex(&hasher.finalize()),
        size,
    })
}

/// Stores what `reader` gives, at most `limit` bytes, in the workspace's
/// file store, hashing it on the way; more than `limit` bytes is an error
/// and stores nothing.
pub fn store_reader(
    layout: &WorkspaceLayout,
    reader: &mut dyn io::Read,
    limit: u64,
) -> io::Result<StoredFile> {
    store_from(layout, reader, limit, true)
}

/// As [`store_reader`], without flushing the file to disk: for storing many
/// files at once, followed by one [`sync_files`] for all of them before
/// anything that refers to them is committed.
pub fn store_reader_unsynced(
    layout: &WorkspaceLayout,
    reader: &mut dyn io::Read,
    limit: u64,
) -> io::Result<StoredFile> {
    store_from(layout, reader, limit, false)
}

/// Flushes the stored files `sha256s` and their folders to disk, several at
/// a time.
pub fn sync_files(layout: &WorkspaceLayout, sha256s: &[String]) -> io::Result<()> {
    let paths: Vec<PathBuf> = sha256s.iter().filter_map(|s| layout.blob_path(s)).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().clamp(2, 16));
    let chunk = paths.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let jobs: Vec<_> = paths
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || -> io::Result<()> {
                    for p in part {
                        sync_file(p)?;
                    }
                    Ok(())
                })
            })
            .collect();
        for job in jobs {
            job.join()
                .map_err(|_| io::Error::other("a file sync thread failed"))??;
        }
        Ok::<_, io::Error>(())
    })?;
    let mut folders: Vec<&Path> = paths.iter().filter_map(|p| p.parent()).collect();
    folders.sort();
    folders.dedup();
    for folder in folders {
        sync_dir(folder)?;
    }
    Ok(())
}

fn store_from(
    layout: &WorkspaceLayout,
    reader: &mut dyn io::Read,
    limit: u64,
    sync: bool,
) -> io::Result<StoredFile> {
    use std::io::Read as _;
    let tmp_dir = layout.files_dir().join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    let tmp = tmp_dir.join(temp_name());
    let key = workspace_seals(layout);
    let written = (|| {
        let file = File::create(&tmp)?;
        let mut file: Box<dyn Sink> = match &key {
            Some(key) => Box::new(SealWriter::new(io::BufWriter::new(file), key)?),
            None => Box::new(file),
        };
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut size = 0u64;
        let mut limited = reader.take(limit.saturating_add(1));
        loop {
            let n = limited.read(&mut buf)?;
            if n == 0 {
                break;
            }
            size += n as u64;
            if size > limit {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "the file is larger than allowed",
                ));
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n])?;
        }
        let file = file.done()?;
        if sync {
            file.sync_all()?;
        }
        drop(file);
        let sha256 = gk_core::hex(&hasher.finalize());
        let target = layout
            .blob_path(&sha256)
            .ok_or_else(|| io::Error::other("a computed hash is not a hash"))?;
        let folder = target
            .parent()
            .ok_or_else(|| io::Error::other("a stored file has no folder"))?
            .to_path_buf();
        fs::create_dir_all(&folder)?;
        if holds(&target, size) {
            let _ = fs::remove_file(&tmp);
        } else {
            fs::rename(&tmp, &target)?;
            if sync {
                sync_dir(&folder)?;
            }
        }
        Ok(StoredFile { sha256, size })
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// A file being written by [`store_from`]: plain, or sealed.
trait Sink: io::Write {
    /// Finishes writing and hands back the file.
    fn done(self: Box<Self>) -> io::Result<File>;
}

impl Sink for File {
    fn done(self: Box<Self>) -> io::Result<File> {
        Ok(*self)
    }
}

impl Sink for SealWriter<io::BufWriter<File>> {
    fn done(self: Box<Self>) -> io::Result<File> {
        self.finish()?
            .into_inner()
            .map_err(io::IntoInnerError::into_error)
    }
}

/// Whether the stored file `sha256` is there with `size` bytes of content.
#[must_use]
pub fn has_file(layout: &WorkspaceLayout, sha256: &str, size: u64) -> bool {
    layout.blob_path(sha256).is_some_and(|p| holds(&p, size))
}

/// Opens the stored file `sha256` for reading its content, sealed or not.
pub fn open_file(layout: &WorkspaceLayout, sha256: &str) -> io::Result<Content> {
    let path = layout
        .blob_path(sha256)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file hash"))?;
    crate::sealed::open_stored(&path)
}

/// Writes the content of the stored file `sha256` to the new file
/// `target`, plain, and returns its size: for opening it in another
/// program, saving a copy or a folder export.
pub fn copy_out(layout: &WorkspaceLayout, sha256: &str, target: &Path) -> io::Result<u64> {
    let mut source = open_file(layout, sha256)?;
    let mut out = File::create(target)?;
    let copied = io::copy(&mut source, &mut out);
    if copied.is_err() {
        drop(out);
        let _ = fs::remove_file(target);
    }
    copied
}

/// The path of the stored file `sha256`, if it is there.
#[must_use]
pub fn file_path(layout: &WorkspaceLayout, sha256: &str) -> Option<PathBuf> {
    layout.blob_path(sha256).filter(|p| p.is_file())
}

/// Reads at most `limit` bytes of the stored file `sha256`.
pub fn read_file(layout: &WorkspaceLayout, sha256: &str, limit: u64) -> io::Result<Vec<u8>> {
    use std::io::Read as _;
    let path = layout
        .blob_path(sha256)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file hash"))?;
    let mut out = Vec::new();
    crate::sealed::open_stored(&path)?
        .take(limit)
        .read_to_end(&mut out)?;
    Ok(out)
}

/// Removes the stored file `sha256` and its cached thumbnail; a file that
/// is already gone is fine.
pub fn remove_file(layout: &WorkspaceLayout, sha256: &str) -> io::Result<()> {
    let Some(path) = layout.blob_path(sha256) else {
        return Ok(());
    };
    if let Some(thumb) = layout.thumb_path(sha256) {
        let _ = fs::remove_file(thumb);
    }
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The cached thumbnail of the stored file `sha256`, if it is there.
#[must_use]
pub fn thumb_path(layout: &WorkspaceLayout, sha256: &str) -> Option<PathBuf> {
    layout.thumb_path(sha256).filter(|p| p.is_file())
}

/// Caches `png` as the thumbnail of the stored file `sha256` and returns
/// its path. Written to a temporary name and renamed, so a reader never sees
/// half of it.
pub fn store_thumb(layout: &WorkspaceLayout, sha256: &str, png: &[u8]) -> io::Result<PathBuf> {
    let target = layout
        .thumb_path(sha256)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file hash"))?;
    let folder = target
        .parent()
        .ok_or_else(|| io::Error::other("a thumbnail has no folder"))?;
    fs::create_dir_all(folder)?;
    let tmp = folder.join(temp_name());
    let key = workspace_seals(layout);
    let written =
        write_new(&tmp, png, key.as_ref(), false).and_then(|()| fs::rename(&tmp, &target));
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(target)
}

/// Reads at most `limit` bytes of the cached thumbnail of the stored file
/// `sha256`, sealed or not; `None` when there is none.
pub fn read_thumb(
    layout: &WorkspaceLayout,
    sha256: &str,
    limit: u64,
) -> io::Result<Option<Vec<u8>>> {
    use std::io::Read as _;
    let Some(path) = thumb_path(layout, sha256) else {
        return Ok(None);
    };
    let mut out = Vec::new();
    crate::sealed::open_stored(&path)?
        .take(limit)
        .read_to_end(&mut out)?;
    Ok(Some(out))
}

/// Removes every cached thumbnail, to be made again as needed: after the
/// workspace's encryption changed.
pub fn clear_thumbs(layout: &WorkspaceLayout) -> io::Result<()> {
    match fs::remove_dir_all(layout.thumbs_dir()) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn a_file_is_stored_once_under_its_hash() {
        let dir = tempfile::tempdir().unwrap();
        let layout = WorkspaceLayout::new(dir.path().join("W"));
        let a = store_file(&layout, b"hello").unwrap();
        assert_eq!(
            a.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(a.size, 5);
        let path = layout.blob_path(&a.sha256).unwrap();
        assert!(path.ends_with(format!("files/2c/{}", a.sha256)));
        assert_eq!(fs::read(&path).unwrap(), b"hello");
        assert_eq!(store_file(&layout, b"hello").unwrap(), a);
        assert!(has_file(&layout, &a.sha256, 5));
        assert!(!has_file(&layout, &a.sha256, 6));
        assert_eq!(read_file(&layout, &a.sha256, 3).unwrap(), b"hel");
        // No temporary file is left behind.
        let tmp: Vec<_> = fs::read_dir(layout.files_dir().join("tmp"))
            .unwrap()
            .collect();
        assert!(tmp.is_empty());
        remove_file(&layout, &a.sha256).unwrap();
        remove_file(&layout, &a.sha256).unwrap();
        assert!(file_path(&layout, &a.sha256).is_none());
    }

    #[test]
    fn files_are_flushed_through_a_handle_that_may_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("written");
        fs::write(&path, b"content").unwrap();
        // A handle that can't write fails even an empty write. Windows
        // flushes only through one that can.
        assert!(File::open(&path).unwrap().write(&[]).is_err());
        assert_eq!(open_to_flush(&path).unwrap().write(&[]).unwrap(), 0);
        sync_file(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"content");
    }

    #[test]
    fn stored_files_are_synced_together() {
        let dir = tempfile::tempdir().unwrap();
        let layout = WorkspaceLayout::new(dir.path().join("W"));
        let mut hashes = Vec::new();
        for n in 0..20 {
            let content = format!("file {n}");
            let stored = store_reader_unsynced(&layout, &mut content.as_bytes(), 1024).unwrap();
            hashes.push(stored.sha256);
        }
        sync_files(&layout, &hashes).unwrap();
        sync_files(&layout, &[]).unwrap();
        for hash in &hashes {
            assert!(file_path(&layout, hash).is_some());
        }
    }

    #[test]
    fn a_truncated_file_is_written_again() {
        let dir = tempfile::tempdir().unwrap();
        let layout = WorkspaceLayout::new(dir.path().join("W"));
        let a = store_file(&layout, b"content").unwrap();
        let path = layout.blob_path(&a.sha256).unwrap();
        fs::write(&path, b"cont").unwrap();
        store_file(&layout, b"content").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"content");
    }

    #[test]
    fn an_encrypted_workspace_seals_what_it_stores_and_reads_it_back() {
        use std::io::{Read as _, Seek as _, SeekFrom};
        let dir = tempfile::tempdir().unwrap();
        let layout = WorkspaceLayout::new(dir.path().join("W"));
        fs::create_dir_all(layout.root()).unwrap();
        let key = FileKey::generate().unwrap();
        crate::sealed::set_workspace_keys(layout.root(), Some(key), vec![]);
        assert!(workspace_seals(&layout).is_some());

        let a = store_file(&layout, b"hello").unwrap();
        let path = layout.blob_path(&a.sha256).unwrap();
        assert!(crate::sealed::looks_sealed(&path).unwrap());
        assert_ne!(fs::metadata(&path).unwrap().len(), 5);
        assert!(has_file(&layout, &a.sha256, 5));
        assert!(!has_file(&layout, &a.sha256, 6));
        assert_eq!(read_file(&layout, &a.sha256, 3).unwrap(), b"hel");
        assert_eq!(store_file(&layout, b"hello").unwrap(), a);
        assert_eq!(sha256_stored(&path).unwrap(), a);

        // A large file, streamed in and read from anywhere.
        let big: Vec<u8> = (0..5_000_000u32).map(|i| (i % 253) as u8).collect();
        let stored = store_reader(&layout, &mut big.as_slice(), u64::MAX).unwrap();
        assert_eq!(stored.sha256, sha256_hex(&big));
        let mut content = open_file(&layout, &stored.sha256).unwrap();
        assert!(content.is_sealed());
        assert_eq!(content.len().unwrap(), big.len() as u64);
        content.seek(SeekFrom::Start(4_000_000)).unwrap();
        let mut part = [0u8; 16];
        content.read_exact(&mut part).unwrap();
        assert_eq!(&part, &big[4_000_000..4_000_016]);
        let out = dir.path().join("copy.bin");
        assert_eq!(
            copy_out(&layout, &stored.sha256, &out).unwrap(),
            big.len() as u64
        );
        assert!(fs::read(&out).unwrap() == big);

        // Thumbnails too.
        let thumb = store_thumb(&layout, &a.sha256, b"small png").unwrap();
        assert!(crate::sealed::looks_sealed(&thumb).unwrap());
        assert_eq!(
            read_thumb(&layout, &a.sha256, 1024).unwrap().unwrap(),
            b"small png"
        );
        clear_thumbs(&layout).unwrap();
        assert!(read_thumb(&layout, &a.sha256, 1024).unwrap().is_none());
        clear_thumbs(&layout).unwrap();

        // Without the key nothing opens.
        crate::sealed::set_workspace_keys(layout.root(), None, vec![]);
        let e = read_file(&layout, &a.sha256, 10).unwrap_err();
        assert_eq!(
            crate::sealed::problem_of(&e),
            Some(crate::sealed::SealProblem::NoKey)
        );
    }

    #[test]
    fn crafted_hashes_name_no_path() {
        let dir = tempfile::tempdir().unwrap();
        let layout = WorkspaceLayout::new(dir.path().join("W"));
        assert!(read_file(&layout, "../../etc/passwd", 10).is_err());
        assert!(file_path(&layout, "..").is_none());
        assert!(layout.thumb_path("..").is_none());
        assert!(store_thumb(&layout, "../x", b"png").is_err());
    }

    #[test]
    fn a_thumbnail_is_cached_by_hash_and_goes_with_its_file() {
        let dir = tempfile::tempdir().unwrap();
        let layout = WorkspaceLayout::new(dir.path().join("W"));
        let a = store_file(&layout, b"picture").unwrap();
        assert!(thumb_path(&layout, &a.sha256).is_none());
        let path = store_thumb(&layout, &a.sha256, b"small").unwrap();
        assert!(path.ends_with(format!("cache/thumbs/{}/{}.png", &a.sha256[..2], a.sha256)));
        assert_eq!(thumb_path(&layout, &a.sha256), Some(path.clone()));
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        remove_file(&layout, &a.sha256).unwrap();
        assert!(thumb_path(&layout, &a.sha256).is_none());
    }
}
