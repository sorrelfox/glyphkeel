// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! On Windows, gives `glyphkeel.exe` its manifest (UTF-8 code page, long
//! paths), the app's icon and a version resource. Nothing on other targets.
//!
//! The manifest and icon live with the app's Windows packaging, in
//! `crates/gk-app`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let app = manifest_dir.join("../gk-app");
    let manifest = app.join("packaging/windows/glyphkeel.manifest");
    let icon = app.join("icons/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed={}", icon.display());

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let version = env::var("CARGO_PKG_VERSION").expect("set by Cargo");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("set by Cargo"));
    let script = out.join("glyphkeel.rc");
    fs::write(&script, resource_script(&version, &manifest, &icon))
        .expect("cannot write the resource script");
    embed_resource::compile(&script, embed_resource::NONE)
        .manifest_required()
        .expect("cannot compile the Windows resources");
}

/// The resource script: icon, manifest and version information.
fn resource_script(version: &str, manifest: &Path, icon: &Path) -> String {
    let numbers = numeric_version(version);
    let manifest = rc_path(manifest);
    let icon = rc_path(icon);
    format!(
        r#"#pragma code_page(65001)
1 ICON "{icon}"
1 24 "{manifest}"
1 VERSIONINFO
FILEVERSION {numbers}
PRODUCTVERSION {numbers}
FILEFLAGSMASK 0x3f
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "sorrelfox"
      VALUE "FileDescription", "Glyphkeel CLI"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "glyphkeel"
      VALUE "LegalCopyright", "Copyright (C) 2026 Glyphkeel contributors"
      VALUE "OriginalFilename", "glyphkeel.exe"
      VALUE "ProductName", "Glyphkeel"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}

/// `1.2.3-rc.1` as `1,2,3,0`: the numeric form a version resource takes.
fn numeric_version(version: &str) -> String {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let mut parts: Vec<u16> = core
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    parts.resize(4, 0);
    parts
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// A path as a resource-script string: forward slashes, quotes escaped.
fn rc_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "/")
        .replace('"', "\"\"")
}
