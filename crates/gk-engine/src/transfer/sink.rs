// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Writing an archive: a `.zip` file or a folder. Everything is written
//! under a temporary name next to the target and renamed when complete, so
//! a failed export leaves nothing that looks like an archive.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use gk_api::ops::transfer::ArchiveFormat;

/// Where an archive is being written.
pub(crate) struct Sink {
    target: PathBuf,
    temp: PathBuf,
    kind: Kind,
    /// SHA-256 of every file written, by path.
    pub(crate) hashes: BTreeMap<String, String>,
}

enum Kind {
    Folder,
    Zip(Box<zip::ZipWriter<File>>),
}

impl std::fmt::Debug for Sink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sink")
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl Sink {
    /// Starts an archive at `target`, which must not exist (a folder may
    /// exist if it is empty).
    pub(crate) fn create(target: &Path, format: ArchiveFormat) -> io::Result<Self> {
        let exists = match fs::symlink_metadata(target) {
            Ok(m) => !(m.is_dir() && fs::read_dir(target)?.next().is_none()),
            Err(_) => false,
        };
        if exists {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} exists already", target.display()),
            ));
        }
        if let Some(parent) = target.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let name = target.file_name().map_or_else(
            || "archive".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let temp = target.with_file_name(format!(".{name}.part"));
        if temp.is_dir() {
            fs::remove_dir_all(&temp)?;
        } else if temp.exists() {
            fs::remove_file(&temp)?;
        }
        let kind = match format {
            ArchiveFormat::Folder => {
                fs::create_dir_all(&temp)?;
                Kind::Folder
            }
            ArchiveFormat::Zip => Kind::Zip(Box::new(zip::ZipWriter::new(File::create(&temp)?))),
        };
        Ok(Self {
            target: target.to_path_buf(),
            temp,
            kind,
            hashes: BTreeMap::new(),
        })
    }

    fn options(compress: bool, size: u64) -> zip::write::SimpleFileOptions {
        let method = if compress {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        zip::write::SimpleFileOptions::default()
            .compression_method(method)
            .large_file(size > u64::from(u32::MAX))
            .unix_permissions(0o644)
    }

    /// Writes `bytes` as the file `path`, compressed in a `.zip`.
    pub(crate) fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.hashes
            .insert(path.to_owned(), gk_store::sha256_hex(bytes));
        self.write_raw(path, bytes)
    }

    /// Writes the manifest, which is not in the hash list.
    pub(crate) fn put_manifest(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.write_raw("manifest.json", bytes)
    }

    fn write_raw(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        match &mut self.kind {
            Kind::Folder => {
                let mut full = self.temp.clone();
                full.extend(path.split('/'));
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(full, bytes)
            }
            Kind::Zip(zip) => {
                zip.start_file(path, Self::options(true, bytes.len() as u64))
                    .map_err(io::Error::other)?;
                zip.write_all(bytes)
            }
        }
    }

    /// Copies the stored file `source` as `path`, stored without
    /// compression in a `.zip`; its name is its hash, so it isn't hashed
    /// again.
    pub(crate) fn put_file(&mut self, path: &str, sha256: &str, source: &Path) -> io::Result<()> {
        self.hashes.insert(path.to_owned(), sha256.to_owned());
        match &mut self.kind {
            Kind::Folder => {
                let mut full = self.temp.clone();
                full.extend(path.split('/'));
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent)?;
                }
                if fs::hard_link(source, &full).is_err() {
                    fs::copy(source, &full)?;
                }
                Ok(())
            }
            Kind::Zip(zip) => {
                let size = fs::metadata(source)?.len();
                zip.start_file(path, Self::options(false, size))
                    .map_err(io::Error::other)?;
                io::copy(&mut File::open(source)?, zip.as_mut())?;
                Ok(())
            }
        }
    }

    /// Finishes the archive and puts it at its target. Returns its size.
    pub(crate) fn finish(self) -> io::Result<u64> {
        match self.kind {
            Kind::Folder => {
                if self.target.is_dir() {
                    fs::remove_dir(&self.target)?;
                }
                fs::rename(&self.temp, &self.target)?;
                Ok(folder_size(&self.target))
            }
            Kind::Zip(zip) => {
                let file = zip.finish().map_err(io::Error::other)?;
                file.sync_all()?;
                drop(file);
                fs::rename(&self.temp, &self.target)?;
                Ok(fs::metadata(&self.target)?.len())
            }
        }
    }

    /// Removes what was written so far.
    pub(crate) fn abandon(self) {
        let temp = self.temp.clone();
        drop(self.kind);
        if temp.is_dir() {
            let _ = fs::remove_dir_all(&temp);
        } else {
            let _ = fs::remove_file(&temp);
        }
    }
}

fn folder_size(path: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(list) = fs::read_dir(&dir) else {
            continue;
        };
        for item in list.flatten() {
            match item.file_type() {
                Ok(t) if t.is_dir() => stack.push(item.path()),
                Ok(_) => total += item.metadata().map_or(0, |m| m.len()),
                Err(_) => {}
            }
        }
    }
    total
}
