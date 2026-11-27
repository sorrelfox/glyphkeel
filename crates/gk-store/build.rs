// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The cryptography of encrypted workspaces on Windows.
//!
//! SQLCipher needs a crypto library. On Linux it is the system's OpenSSL
//! (libcrypto). Windows has none, so the Windows build compiles LibTomCrypt,
//! which comes with the SQLCipher source, and the SQLCipher build is told to
//! use it (`cargo xtask ci --distro windows-cross` sets that up; see
//! CONTRIBUTING.md). The file format is the same either way.
//!
//! The SQLCipher build always links a library called `crypto` (`libcrypto`
//! on a Windows host). On Windows, that library is the one built here.

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let source = sqlcipher_src::source_dir();
    let host_is_windows = env::var("HOST").is_ok_and(|host| host.contains("windows"));
    let name = if host_is_windows {
        "libcrypto"
    } else {
        "crypto"
    };
    cc::Build::new()
        .include(source.join(sqlcipher_src::LIBTOMCRYPT_INCLUDE_DIR))
        .define("LTC_SOURCE", None)
        .files(
            sqlcipher_src::LIBTOMCRYPT_SOURCES
                .iter()
                .map(|file| source.join(file)),
        )
        .warnings(false)
        // Only built here: the SQLCipher build already links it by name.
        .cargo_metadata(false)
        .compile(name);
    println!(
        "cargo:rustc-link-search=native={}",
        env::var("OUT_DIR").expect("cargo sets OUT_DIR")
    );
    // The system random generator LibTomCrypt seeds SQLCipher's from.
    println!("cargo:rustc-link-lib=advapi32");
}
