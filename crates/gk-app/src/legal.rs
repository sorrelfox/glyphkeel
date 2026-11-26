// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The licence and the third-party notices, for Preferences › About. Every
//! package installs both files, and the page reads them through here:
//!
//! - the rpm in `/usr/share/licenses/glyphkeel/`, the deb in
//!   `/usr/share/doc/glyphkeel/`, the Flatpak in
//!   `/app/share/licenses/<app id>/`: looked for next to the program's
//!   `bin` folder, so any prefix works;
//! - the Windows installer next to the program, as Tauri resources;
//! - a development build also reads the repository's copies.
//!
//! The page only says which of the two texts it wants, never a path.

use std::path::{Path, PathBuf};

use gk_api::OpError;
use serde::Deserialize;

use crate::messages::text;

/// The two texts the page can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegalText {
    /// The GNU General Public License.
    Licence,
    /// THIRD-PARTY-NOTICES.txt.
    Notices,
}

impl LegalText {
    /// The installed file names; the Windows installer adds `.txt` to the
    /// licence so that it opens in a text editor.
    fn file_names(self) -> &'static [&'static str] {
        match self {
            Self::Licence => &["LICENSE", "LICENSE.txt"],
            Self::Notices => &["THIRD-PARTY-NOTICES.txt"],
        }
    }
}

/// The folders, below the installation prefix, where the packages put the
/// files.
const SHARE_FOLDERS: &[&str] = &[
    "share/licenses/glyphkeel",
    "share/doc/glyphkeel",
    "share/licenses/io.github.sorrelfox.Glyphkeel",
];

/// The places to look, in order, for a program at `exe`.
fn candidates(exe: &Path, resources: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = resources {
        dirs.push(dir.to_path_buf());
    }
    if let Some(bin) = exe.parent() {
        dirs.push(bin.to_path_buf());
        if let Some(prefix) = bin.parent() {
            dirs.extend(SHARE_FOLDERS.iter().map(|folder| prefix.join(folder)));
        }
    }
    // A development build, run from the repository's target folder.
    if cfg!(debug_assertions) {
        dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    }
    dirs
}

fn read(which: LegalText, dirs: &[PathBuf]) -> Result<String, OpError> {
    for dir in dirs {
        for name in which.file_names() {
            if let Ok(contents) = std::fs::read_to_string(dir.join(name)) {
                return Ok(contents);
            }
        }
    }
    let name = which.file_names()[0];
    Err(OpError::not_found(
        text("legal-text-missing", &[("file", name.to_owned())]),
        &[],
    ))
}

/// The text of the licence or of the third-party notices.
#[tauri::command]
pub(crate) fn legal_text(app: tauri::AppHandle, which: LegalText) -> Result<String, OpError> {
    use tauri::Manager as _;
    let resources = app.path().resource_dir().ok();
    let exe = std::env::current_exe().unwrap_or_default();
    read(which, &candidates(&exe, resources.as_deref()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{LegalText, candidates, read};

    #[test]
    fn the_package_folders_are_looked_for_beside_the_bin_folder() {
        let dirs = candidates(Path::new("/usr/bin/glyphkeel-app"), None);
        assert_eq!(dirs[0], Path::new("/usr/bin"));
        assert!(dirs.contains(&Path::new("/usr/share/licenses/glyphkeel").to_path_buf()));
        assert!(dirs.contains(&Path::new("/usr/share/doc/glyphkeel").to_path_buf()));
        let flatpak = candidates(Path::new("/app/bin/glyphkeel-app"), None);
        assert!(flatpak.contains(
            &Path::new("/app/share/licenses/io.github.sorrelfox.Glyphkeel").to_path_buf()
        ));
        let windows = candidates(
            Path::new("C:/Programs/Glyphkeel/glyphkeel-app.exe"),
            Some(Path::new("C:/Programs/Glyphkeel")),
        );
        assert_eq!(windows[0], Path::new("C:/Programs/Glyphkeel"));
    }

    #[test]
    fn the_first_folder_with_the_file_wins() {
        let empty = tempfile::tempdir().unwrap();
        let full = tempfile::tempdir().unwrap();
        std::fs::write(full.path().join("THIRD-PARTY-NOTICES.txt"), "notices").unwrap();
        std::fs::write(full.path().join("LICENSE.txt"), "gpl").unwrap();
        let dirs = [empty.path().to_path_buf(), full.path().to_path_buf()];
        assert_eq!(read(LegalText::Notices, &dirs).unwrap(), "notices");
        assert_eq!(read(LegalText::Licence, &dirs).unwrap(), "gpl");
        let error = read(LegalText::Notices, &dirs[..1]).unwrap_err();
        assert!(
            error.message.contains("THIRD-PARTY-NOTICES.txt"),
            "{}",
            error.message
        );
    }

    #[test]
    fn the_main_windows_may_read_the_texts_and_the_page_names_only_which() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        let permissions = capability["permissions"].as_array().unwrap();
        assert!(permissions.iter().any(|p| p == "allow-legal-text"));
        let which: LegalText = serde_json::from_str("\"notices\"").unwrap();
        assert_eq!(which, LegalText::Notices);
        assert!(serde_json::from_str::<LegalText>("\"/etc/passwd\"").is_err());
    }

    #[test]
    fn the_repository_has_both_files_for_development_builds() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(read(LegalText::Licence, std::slice::from_ref(&root)).is_ok());
        let notices = read(LegalText::Notices, &[root]).unwrap();
        assert!(notices.starts_with("Third-party notices for Glyphkeel"));
    }
}
