# LibTomCrypt licence file

`LICENSE` is copied unchanged from the crate
[sqlcipher-src](https://crates.io/crates/sqlcipher-src)
420.0.0+sqlcipher-4.20.0-sqlite-3.54.1-libtomcrypt-1.18.2 (its file
`sqlcipher/LICENSE-libtomcrypt`), which carries the LibTomCrypt 1.18.2 source
(`https://github.com/libtom/libtomcrypt`). LibTomCrypt is public domain or,
at the user's choice, under the WTFPL.

No LibTomCrypt code is kept here. The Windows build compiles LibTomCrypt from
that crate as SQLCipher's cryptography (`crates/gk-store/build.rs`), so
`cargo xtask notices` puts this file in `THIRD-PARTY-NOTICES.txt`. Replace it
when sqlcipher-src moves to a newer LibTomCrypt.
