// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! An archive held in memory, and writing it out as a folder.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use anyhow::{Context as _, bail};
use sha2::{Digest, Sha256};

use crate::generate::hex;
use crate::model::{Counts, FORMAT_NAME, FORMAT_VERSION, Generator, Manifest, ManifestProject};

/// Path of the manifest inside the archive.
pub const MANIFEST: &str = "manifest.json";

/// Every file of an archive, by path. Paths use `/` and are relative.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Archive {
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug)]
pub(crate) struct ManifestInput {
    pub workspace_id: String,
    pub exported_at: i64,
    pub set: String,
    pub seed: u64,
    pub projects: Vec<ManifestProject>,
    pub counts: Counts,
}

impl Archive {
    /// Adds `manifest.json` with the hash of every other file.
    pub(crate) fn finish(mut self, input: ManifestInput) -> anyhow::Result<Self> {
        let hashes = self
            .files
            .iter()
            .map(|(path, bytes)| (path.clone(), hex(&Sha256::digest(bytes))))
            .collect();
        let manifest = Manifest {
            format: FORMAT_NAME.into(),
            format_version: FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").into(),
            workspace_id: input.workspace_id,
            exported_at: input.exported_at,
            generator: Generator {
                tool: "gk-seed".into(),
                set: input.set,
                seed: input.seed,
            },
            projects: input.projects,
            counts: input.counts,
            hashes,
        };
        let mut bytes = serde_json::to_vec_pretty(&manifest)?;
        bytes.push(b'\n');
        self.files.insert(MANIFEST.into(), bytes);
        Ok(self)
    }

    /// Total size of all files in bytes.
    pub fn size(&self) -> u64 {
        self.files.values().map(|b| b.len() as u64).sum()
    }

    /// Writes every file below `dir`, which must not exist or be empty.
    pub fn write_to(&self, dir: &Path) -> anyhow::Result<()> {
        if dir.exists() {
            let mut entries =
                fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))?;
            if entries.next().is_some() {
                bail!("{} is not empty; choose a new folder", dir.display());
            }
        }
        for (path, bytes) in &self.files {
            check_relative(path)?;
            let target = dir.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("cannot create {}", parent.display()))?;
            }
            fs::write(&target, bytes)
                .with_context(|| format!("cannot write {}", target.display()))?;
        }
        Ok(())
    }

    /// Reads a folder written by [`Archive::write_to`].
    pub fn read_from(dir: &Path) -> anyhow::Result<Self> {
        let mut files = BTreeMap::new();
        read_dir(dir, dir, &mut files)?;
        Ok(Self { files })
    }
}

fn read_dir(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            read_dir(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.insert(rel, fs::read(&path)?);
        }
    }
    Ok(())
}

/// Generated paths come from fixed names and slugs, but a slip must never
/// write outside the target folder.
fn check_relative(path: &str) -> anyhow::Result<()> {
    let ok = !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
    if !ok {
        bail!("refusing to write unsafe path {path:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_paths_are_refused() {
        assert!(check_relative("projects/TM/tasks.jsonl").is_ok());
        assert!(check_relative("../x").is_err());
        assert!(check_relative("/etc/passwd").is_err());
        assert!(check_relative("a/./b").is_ok());
        assert!(check_relative("").is_err());
    }
}
