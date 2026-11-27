// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Reading an archive, a `.zip` file or a folder, without trusting it.
//!
//! Before any content is read, the list of entries is checked: every path
//! must be a plain relative path (no `..`, no absolute or drive paths, no
//! backslashes or control characters), unique, and not a link; there must
//! not be too many entries or too many bytes in all; and no compressed
//! entry may expand far beyond its compressed size. Every read is capped at
//! the size the entry declares and at the limit for its kind, so an entry
//! that lies about its size can't exhaust memory.
//!
//! An encrypted export (a sealed `.zip`) is opened on the fly with a
//! passphrase this program knows (the workspace's, one given for the
//! import, or the one in the environment) or with a file key of an open
//! workspace; it is never written out plain.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::{Limits, Rejected};

/// One entry of an archive.
#[derive(Debug, Clone)]
pub(crate) struct EntryInfo {
    /// Its size, as declared.
    pub(crate) size: u64,
    index: usize,
}

/// An open archive.
pub(crate) enum Source {
    Folder {
        root: PathBuf,
        entries: BTreeMap<String, EntryInfo>,
    },
    Zip {
        zip: Box<zip::ZipArchive<gk_store::Content>>,
        entries: BTreeMap<String, EntryInfo>,
    },
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Source")
            .field("entries", &self.entries().len())
            .finish_non_exhaustive()
    }
}

/// Whether `path` is a safe relative path inside an archive.
pub(crate) fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && !path.chars().any(char::is_control)
        && path.split('/').count() <= 32
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

impl Source {
    /// Opens the archive at `path`: a folder, or a `.zip` file.
    pub(crate) fn open(path: &Path, limits: &Limits) -> Result<Self, Rejected> {
        let meta = fs::symlink_metadata(path).map_err(|e| Rejected::Unreadable(e.to_string()))?;
        if meta.file_type().is_symlink() {
            return Err(Rejected::Link(path.to_string_lossy().into_owned()));
        }
        if meta.is_dir() {
            Self::open_folder(path, limits)
        } else {
            Self::open_zip(path, limits)
        }
    }

    fn open_folder(root: &Path, limits: &Limits) -> Result<Self, Rejected> {
        let mut entries = BTreeMap::new();
        let mut total = 0u64;
        let mut stack = vec![(root.to_path_buf(), String::new())];
        while let Some((dir, prefix)) = stack.pop() {
            let list = fs::read_dir(&dir).map_err(|e| Rejected::Unreadable(e.to_string()))?;
            for item in list {
                let item = item.map_err(|e| Rejected::Unreadable(e.to_string()))?;
                let name = item
                    .file_name()
                    .into_string()
                    .map_err(|n| Rejected::BadPath(n.to_string_lossy().into_owned()))?;
                let rel = if prefix.is_empty() {
                    name
                } else {
                    format!("{prefix}/{name}")
                };
                if !safe_path(&rel) {
                    return Err(Rejected::BadPath(rel));
                }
                let kind = item
                    .file_type()
                    .map_err(|e| Rejected::Unreadable(e.to_string()))?;
                if kind.is_symlink() {
                    return Err(Rejected::Link(rel));
                }
                if kind.is_dir() {
                    stack.push((item.path(), rel));
                    continue;
                }
                if !kind.is_file() {
                    return Err(Rejected::Link(rel));
                }
                let size = item
                    .metadata()
                    .map_err(|e| Rejected::Unreadable(e.to_string()))?
                    .len();
                total = total.saturating_add(size);
                let index = entries.len();
                entries.insert(rel, EntryInfo { size, index });
                if entries.len() > limits.max_entries {
                    return Err(Rejected::TooManyEntries(limits.max_entries));
                }
                if total > limits.max_total {
                    return Err(Rejected::TooLarge(limits.max_total));
                }
            }
        }
        Ok(Self::Folder {
            root: root.to_path_buf(),
            entries,
        })
    }

    fn open_zip(path: &Path, limits: &Limits) -> Result<Self, Rejected> {
        let file = open_sealed(path)?;
        let mut zip =
            zip::ZipArchive::new(file).map_err(|e| Rejected::NotAnArchive(e.to_string()))?;
        if zip.len() > limits.max_entries {
            return Err(Rejected::TooManyEntries(limits.max_entries));
        }
        // The reader keeps one entry per name, so a name given twice is
        // only visible here.
        if zip.has_duplicate_names() {
            return Err(Rejected::DuplicateEntries);
        }
        let mut entries = BTreeMap::new();
        let mut total = 0u64;
        for index in 0..zip.len() {
            let entry = zip
                .by_index_raw(index)
                .map_err(|e| Rejected::NotAnArchive(e.to_string()))?;
            let name = std::str::from_utf8(entry.name_raw())
                .map_err(|_| Rejected::BadPath(String::from_utf8_lossy(entry.name_raw()).into()))?
                .to_owned();
            if entry.is_dir() {
                let trimmed = name.trim_end_matches('/');
                if !safe_path(trimmed) {
                    return Err(Rejected::BadPath(name));
                }
                continue;
            }
            if !safe_path(&name) {
                return Err(Rejected::BadPath(name));
            }
            if entry.is_symlink()
                || entry
                    .unix_mode()
                    .is_some_and(|m| m & 0o170_000 == 0o120_000)
            {
                return Err(Rejected::Link(name));
            }
            if entry.encrypted() {
                return Err(Rejected::Encrypted(name));
            }
            let size = entry.size();
            let packed = entry.compressed_size().max(1);
            if size > limits.ratio_floor && size / packed > limits.max_ratio {
                return Err(Rejected::Bomb(name));
            }
            total = total.saturating_add(size);
            if total > limits.max_total {
                return Err(Rejected::TooLarge(limits.max_total));
            }
            if entries
                .insert(name.clone(), EntryInfo { size, index })
                .is_some()
            {
                return Err(Rejected::Duplicate(name));
            }
        }
        Ok(Self::Zip {
            zip: Box::new(zip),
            entries,
        })
    }

    /// Every entry by path.
    pub(crate) fn entries(&self) -> &BTreeMap<String, EntryInfo> {
        match self {
            Self::Folder { entries, .. } | Self::Zip { entries, .. } => entries,
        }
    }

    /// A reader of entry `path` that fails beyond `limit` bytes or beyond
    /// the entry's declared size.
    pub(crate) fn open_entry<'a>(
        &'a mut self,
        path: &str,
        limit: u64,
    ) -> Result<Capped<Box<dyn Read + 'a>>, Rejected> {
        let info = self
            .entries()
            .get(path)
            .cloned()
            .ok_or_else(|| Rejected::Missing(path.to_owned()))?;
        if info.size > limit {
            return Err(Rejected::EntryTooLarge(path.to_owned(), limit));
        }
        let reader: Box<dyn Read + 'a> = match self {
            Self::Folder { root, .. } => {
                let mut full = root.clone();
                full.extend(path.split('/'));
                Box::new(File::open(full).map_err(|e| Rejected::Unreadable(e.to_string()))?)
            }
            Self::Zip { zip, .. } => Box::new(
                zip.by_index(info.index)
                    .map_err(|e| Rejected::NotAnArchive(e.to_string()))?,
            ),
        };
        Ok(Capped {
            inner: reader,
            left: info.size,
            declared: info.size,
            path: path.to_owned(),
            read: 0,
        })
    }

    /// The whole of entry `path`, at most `limit` bytes.
    pub(crate) fn read(&mut self, path: &str, limit: u64) -> Result<Vec<u8>, Rejected> {
        let mut reader = self.open_entry(path, limit)?;
        let mut out = Vec::with_capacity(usize::try_from(reader.declared.min(limit)).unwrap_or(0));
        reader
            .read_to_end(&mut out)
            .map_err(|e| Rejected::Unreadable(format!("{path}: {e}")))?;
        reader.finish()?;
        Ok(out)
    }
}

/// The file at `path` for reading: a sealed one opened with a key this
/// program knows, anything else as it is.
fn open_sealed(path: &Path) -> Result<gk_store::Content, Rejected> {
    let passphrases = gk_store::known_passphrases();
    let opened = match gk_store::open_with_passphrases(path, &passphrases) {
        Err(e) if gk_store::seal_problem(&e) == Some(gk_store::SealProblem::NoKey) => {
            gk_store::open_stored(path)
        }
        other => other,
    };
    opened.map_err(|e| match gk_store::seal_problem(&e) {
        Some(gk_store::SealProblem::NoKey) => {
            Rejected::Passphrase(path.to_string_lossy().into_owned())
        }
        Some(gk_store::SealProblem::Damaged) => {
            Rejected::SealDamaged(path.to_string_lossy().into_owned())
        }
        _ => Rejected::Unreadable(e.to_string()),
    })
}

/// A reader that stops at the declared size of its entry and reports an
/// entry that holds more, or less.
pub(crate) struct Capped<R> {
    inner: R,
    left: u64,
    declared: u64,
    read: u64,
    path: String,
}

impl<R: Read> Capped<R> {
    /// Checks that the entry held exactly its declared size.
    pub(crate) fn finish(mut self) -> Result<(), Rejected> {
        let mut probe = [0u8; 1];
        let more = self
            .inner
            .read(&mut probe)
            .map_err(|e| Rejected::Unreadable(format!("{}: {e}", self.path)))?;
        if more > 0 || self.read != self.declared {
            return Err(Rejected::SizeMismatch(self.path.clone()));
        }
        Ok(())
    }
}

impl<R: Read> Read for Capped<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Ok(0);
        }
        let want = usize::try_from(self.left).map_or(buf.len(), |l| l.min(buf.len()));
        let n = self.inner.read(&mut buf[..want])?;
        self.left -= n as u64;
        self.read += n as u64;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_relative_paths_are_safe() {
        for ok in ["manifest.json", "projects/TM/tasks/TM-1.md", "files/ab"] {
            assert!(safe_path(ok), "{ok}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../up",
            "projects/../../x",
            "a//b",
            "./a",
            "C:/x",
            "a\\b",
            "a\nb",
            "a/",
        ] {
            assert!(!safe_path(bad), "{bad:?}");
        }
    }
}
