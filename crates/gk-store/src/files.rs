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

use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::layout::WorkspaceLayout;

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

/// Stores `bytes` in the workspace's file store and returns its hash and
/// size. A file that is already there with the right size is kept as is.
pub fn store_file(layout: &WorkspaceLayout, bytes: &[u8]) -> io::Result<StoredFile> {
    let sha256 = sha256_hex(bytes);
    let size = bytes.len() as u64;
    let target = layout
        .blob_path(&sha256)
        .ok_or_else(|| io::Error::other("a computed hash is not a hash"))?;
    if fs::metadata(&target).is_ok_and(|m| m.len() == size) {
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
    let written = (|| {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, &target)?;
        sync_dir(&folder)
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(StoredFile { sha256, size })
}

/// Whether the stored file `sha256` is there with `size` bytes.
#[must_use]
pub fn has_file(layout: &WorkspaceLayout, sha256: &str, size: u64) -> bool {
    layout
        .blob_path(sha256)
        .and_then(|p| fs::metadata(p).ok())
        .is_some_and(|m| m.len() == size)
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
    File::open(path)?.take(limit).read_to_end(&mut out)?;
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
    let written = fs::write(&tmp, png).and_then(|()| fs::rename(&tmp, &target));
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(target)
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
