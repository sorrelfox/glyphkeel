// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Writing an archive: a `.zip` file or a folder. Everything is written
//! under a temporary name next to the target and renamed when complete, so
//! a failed export leaves nothing that looks like an archive.
//!
//! An encrypted export is a `.zip` written straight into a sealed file (see
//! `gk_store`'s sealed files): with the workspace's passphrase, so it opens
//! on another computer with that passphrase, or with the workspace's file
//! key when this program doesn't know the passphrase.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use gk_api::ops::transfer::ArchiveFormat;
use gk_store::{FileKey, Passphrase, SealWriter, WorkspaceLayout};

/// How an encrypted export is sealed.
#[derive(Debug, Clone)]
pub(crate) enum Seal {
    /// With a key from this passphrase.
    Passphrase(Passphrase),
    /// With the workspace's file key.
    Key(FileKey),
}

type SealedZip = zip::ZipWriter<zip::write::StreamWriter<SealWriter<io::BufWriter<File>>>>;

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
    SealedZip(Box<SealedZip>),
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
    /// exist if it is empty). With `seal`, a sealed `.zip` (never a
    /// folder).
    pub(crate) fn create(
        target: &Path,
        format: ArchiveFormat,
        seal: Option<&Seal>,
    ) -> io::Result<Self> {
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
        let kind = match (format, seal) {
            (ArchiveFormat::Folder, None) => {
                fs::create_dir_all(&temp)?;
                Kind::Folder
            }
            (ArchiveFormat::Folder, Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "an encrypted export is a .zip file",
                ));
            }
            (ArchiveFormat::Zip, None) => {
                Kind::Zip(Box::new(zip::ZipWriter::new(File::create(&temp)?)))
            }
            (ArchiveFormat::Zip, Some(seal)) => {
                let file = io::BufWriter::new(File::create(&temp)?);
                let sealed = match seal {
                    Seal::Passphrase(p) => SealWriter::with_passphrase(file, p)?,
                    Seal::Key(key) => SealWriter::new(file, key)?,
                };
                Kind::SealedZip(Box::new(zip::ZipWriter::new_stream(sealed)))
            }
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
            Kind::SealedZip(zip) => {
                zip.start_file(path, Self::options(true, bytes.len() as u64))
                    .map_err(io::Error::other)?;
                zip.write_all(bytes)
            }
        }
    }

    /// Copies the stored file `sha256` of the workspace at `layout` as
    /// `path`, stored without compression in a `.zip`; its name is its
    /// hash, so it isn't hashed again. A sealed stored file is opened on
    /// the way: an archive holds the content.
    pub(crate) fn put_file(
        &mut self,
        path: &str,
        sha256: &str,
        layout: &WorkspaceLayout,
    ) -> io::Result<()> {
        self.hashes.insert(path.to_owned(), sha256.to_owned());
        let mut content = gk_store::open_file(layout, sha256)?;
        match &mut self.kind {
            Kind::Folder => {
                let mut full = self.temp.clone();
                full.extend(path.split('/'));
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent)?;
                }
                let linked = !content.is_sealed()
                    && layout
                        .blob_path(sha256)
                        .is_some_and(|source| fs::hard_link(source, &full).is_ok());
                if !linked {
                    io::copy(&mut content, &mut File::create(&full)?)?;
                }
                Ok(())
            }
            Kind::Zip(zip) => {
                let size = content.len()?;
                zip.start_file(path, Self::options(false, size))
                    .map_err(io::Error::other)?;
                io::copy(&mut content, zip.as_mut())?;
                Ok(())
            }
            Kind::SealedZip(zip) => {
                let size = content.len()?;
                zip.start_file(path, Self::options(false, size))
                    .map_err(io::Error::other)?;
                io::copy(&mut content, zip.as_mut())?;
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
            Kind::SealedZip(zip) => {
                let sealed = zip.finish().map_err(io::Error::other)?.into_inner();
                let file = sealed
                    .finish()?
                    .into_inner()
                    .map_err(io::IntoInnerError::into_error)?;
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
