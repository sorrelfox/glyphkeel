// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Encryption at rest: the workspace database encrypted with SQLCipher.
//!
//! A workspace is either plain or encrypted; the file tells which. A plain
//! database starts with SQLite's header, an encrypted one with 16 random
//! bytes, the salt of its key. Both are opened by the same code: every
//! connection reads those 16 bytes first ([`prepare`]) and, for an
//! encrypted file, sets the key before anything else.
//!
//! # Passphrase and key
//!
//! The key is derived from the person's passphrase the way SQLCipher 4
//! derives it itself (PBKDF2-HMAC-SHA512, 256 000 rounds, the file's salt),
//! so the file also opens with the passphrase in SQLCipher's own tools. The
//! derivation is slow on purpose; this module does it once per file and
//! process and hands every further connection the derived key, so the
//! writer, the readers and the watcher open as fast as for a plain file.
//!
//! Passphrases are kept in memory for the life of the process
//! ([`remember_passphrase`]), and every file that one of them opens can be
//! opened: the live database, its backups (which keep its salt) and copies
//! made while restoring one. Nothing is written to disk, and the system's
//! keyring is never used: a passphrase comes from the person (the app's
//! lock screen, a prompt on the terminal) or from the environment
//! ([`PASSPHRASE_ENV`], [`unlock_without_asking`]).
//!
//! # Encrypting and decrypting
//!
//! [`encrypt_workspace`] and [`decrypt_workspace`] write the whole database
//! into a new file with `sqlcipher_export`, check the new file and put it in
//! place with one rename. They need the workspace to themselves: no other
//! program may have it open ([`StoreError::InUse`]). The stored files in
//! `files/` are not encrypted.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, Once, OnceLock};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use zeroize::Zeroizing;

use crate::connection::{self, Purpose};
use crate::error::{Result, StoreError};
use crate::layout::{LockFile, WorkspaceLayout};

/// The environment variable a passphrase can be given in, for the command
/// line and agent servers that can't ask for one.
pub const PASSPHRASE_ENV: &str = "GLYPHKEEL_PASSPHRASE";

/// The first 16 bytes of every plain SQLite database.
const PLAIN_HEADER: &[u8; 16] = b"SQLite format 3\0";
/// SQLCipher 4's page size: an encrypted file is a whole number of them.
const PAGE_SIZE: u64 = 4096;
/// Length of the salt at the start of an encrypted file.
const SALT_LEN: usize = 16;
/// SQLCipher 4's default rounds of PBKDF2-HMAC-SHA512.
const KDF_ROUNDS: u32 = 256_000;
/// The `meta` key that marks a database file replaced by its encrypted or
/// decrypted copy.
pub(crate) const RETIRED_KEY: &str = "retired";

/// A passphrase. Its text is wiped from memory when it is dropped and never
/// shown by `Debug`.
#[derive(Clone)]
pub struct Passphrase(Zeroizing<String>);

impl Passphrase {
    /// The passphrase `text`; `None` when it is empty.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Option<Self> {
        let text = Zeroizing::new(text.into());
        (!text.is_empty()).then_some(Self(text))
    }

    /// The text, to hand to SQLCipher or the keyring.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Passphrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Passphrase(…)")
    }
}

impl PartialEq for Passphrase {
    fn eq(&self, other: &Self) -> bool {
        // Not in constant time; passphrases are only compared with others
        // the same process already holds.
        self.0.as_str() == other.0.as_str()
    }
}

/// Whether a database file is encrypted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Encryption {
    /// A plain SQLite file, or no file yet.
    Plain,
    /// Encrypted with SQLCipher.
    Encrypted,
}

/// A stored check of a passphrase that guards something other than a
/// database, such as the app lock of a plain workspace: a random salt and
/// the PBKDF2 key of the passphrase with it, never the passphrase itself.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PassphraseCheck {
    /// The salt, in hexadecimal.
    salt: String,
    /// The derived key, in hexadecimal.
    hash: String,
}

impl std::fmt::Debug for PassphraseCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PassphraseCheck(…)")
    }
}

impl PassphraseCheck {
    /// A check of `passphrase` with a new random salt.
    pub fn new(passphrase: &Passphrase) -> Result<Self> {
        let mut salt = [0u8; SALT_LEN];
        getrandom::fill(&mut salt)
            .map_err(|e| StoreError::io("making a salt", "", io::Error::other(e.to_string())))?;
        Ok(Self {
            salt: hex(&salt),
            hash: hex(derive(passphrase, &salt).as_ref()),
        })
    }

    /// Whether `passphrase` is the one checked.
    #[must_use]
    pub fn matches(&self, passphrase: &Passphrase) -> bool {
        let Some(salt) = unhex(&self.salt).and_then(|s| <[u8; SALT_LEN]>::try_from(s).ok()) else {
            return false;
        };
        let Some(hash) = unhex(&self.hash) else {
            return false;
        };
        let key = derive(passphrase, &salt);
        // In constant time over the key's length.
        hash.len() == key.len()
            && hash
                .iter()
                .zip(key.iter())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, b| {
            let _ = write!(text, "{b:02x}");
            text
        })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Whether this build can encrypt: it is linked with SQLCipher. Builds
/// without it open plain workspaces only.
#[must_use]
pub fn encryption_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Connection::open_in_memory()
            .and_then(|conn| {
                conn.query_row("PRAGMA cipher_version", [], |r| r.get::<_, String>(0))
                    .optional()
            })
            .is_ok_and(|version| version.is_some())
    })
}

/// Whether the database file at `path` is encrypted. A missing or empty
/// file is plain: it becomes a plain database when it is created.
pub fn encryption_of(path: &Path) -> Result<Encryption> {
    Ok(match read_salt(path)? {
        Some(_) => Encryption::Encrypted,
        None => Encryption::Plain,
    })
}

/// The salt at the start of an encrypted file; `None` for a plain, empty or
/// missing one.
fn read_salt(path: &Path) -> Result<Option<[u8; SALT_LEN]>> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(StoreError::io("opening", path, e)),
    };
    let mut head = [0u8; SALT_LEN];
    let mut read = 0;
    while read < SALT_LEN {
        match file.read(&mut head[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(StoreError::io("reading", path, e)),
        }
    }
    if read < SALT_LEN || &head == PLAIN_HEADER {
        return Ok(None);
    }
    // Only a file that could be SQLCipher's counts as encrypted: whole
    // pages, at least one. Anything else (a text file, a cut-off copy) is
    // left to SQLite, which reports it as not a database.
    let size = file
        .metadata()
        .map_err(|e| StoreError::io("reading", path, e))?
        .len();
    if size < PAGE_SIZE || size % PAGE_SIZE != 0 {
        return Ok(None);
    }
    Ok(Some(head))
}

/// The passphrases and derived keys this process knows.
#[derive(Default)]
struct Keys {
    passphrases: Vec<Passphrase>,
    derived: Vec<([u8; SALT_LEN], Zeroizing<[u8; 32]>)>,
}

static KEYS: Mutex<Keys> = Mutex::new(Keys {
    passphrases: Vec::new(),
    derived: Vec::new(),
});

fn keys() -> MutexGuard<'static, Keys> {
    KEYS.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Keeps `passphrase` for this process: every encrypted file it opens can
/// be opened from now on.
pub fn remember_passphrase(passphrase: &Passphrase) {
    let mut keys = keys();
    if !keys.passphrases.contains(passphrase) {
        keys.passphrases.push(passphrase.clone());
    }
}

/// Forgets every passphrase and key this process knows. Connections that
/// are open stay open.
pub fn forget_passphrases() {
    let mut keys = keys();
    keys.passphrases.clear();
    keys.derived.clear();
}

/// Whether the file at `path` can be opened: it is plain, or a passphrase
/// this process knows opens it.
pub fn can_open(path: &Path) -> Result<bool> {
    match key_for(path) {
        Ok(_) => Ok(true),
        Err(StoreError::PassphraseRequired(_) | StoreError::WrongPassphrase(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Succeeds when the file at `path` can be opened; fails with
/// [`StoreError::PassphraseRequired`] or [`StoreError::WrongPassphrase`]
/// when it can't.
fn check_key(path: &Path) -> Result<()> {
    key_for(path).map(|_| ())
}

/// Whether `passphrase` opens the encrypted file at `path`. A plain file
/// has no passphrase: `false`. When it does open the file, its key is kept
/// as [`remember_passphrase`] would keep it.
pub fn passphrase_opens(path: &Path, passphrase: &Passphrase) -> Result<bool> {
    let Some(salt) = read_salt(path)? else {
        return Ok(false);
    };
    let key = derive(passphrase, &salt);
    if !key_opens(path, &key)? {
        return Ok(false);
    }
    let mut keys = keys();
    if !keys.derived.iter().any(|(s, _)| *s == salt) {
        keys.derived.push((salt, key));
    }
    if !keys.passphrases.contains(passphrase) {
        keys.passphrases.push(passphrase.clone());
    }
    Ok(true)
}

/// The key of the encrypted file at `path`, from the keys and passphrases
/// this process knows; `None` for a plain file.
fn key_for(path: &Path) -> Result<Option<Zeroizing<[u8; 32]>>> {
    let Some(salt) = read_salt(path)? else {
        return Ok(None);
    };
    if !encryption_available() {
        return Err(StoreError::EncryptionUnavailable);
    }
    let candidates = {
        let keys = keys();
        if let Some((_, key)) = keys.derived.iter().find(|(s, _)| *s == salt) {
            return Ok(Some(key.clone()));
        }
        keys.passphrases.clone()
    };
    if candidates.is_empty() {
        return Err(StoreError::PassphraseRequired(path.to_path_buf()));
    }
    for passphrase in &candidates {
        let key = derive(passphrase, &salt);
        if key_opens(path, &key)? {
            let mut keys = keys();
            if !keys.derived.iter().any(|(s, _)| *s == salt) {
                keys.derived.push((salt, key.clone()));
            }
            return Ok(Some(key));
        }
    }
    Err(StoreError::WrongPassphrase(path.to_path_buf()))
}

/// SQLCipher 4's key for `passphrase` and `salt`.
fn derive(passphrase: &Passphrase, salt: &[u8; SALT_LEN]) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    pbkdf2::pbkdf2_hmac::<sha2::Sha512>(
        passphrase.expose().as_bytes(),
        salt,
        KDF_ROUNDS,
        key.as_mut(),
    );
    key
}

/// Whether `key` decrypts the file at `path`, tried on a connection of its
/// own (a connection that failed to decrypt can't be given another key).
fn key_opens(path: &Path, key: &[u8; 32]) -> Result<bool> {
    quiet();
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_secs(5))?;
    set_key(&conn, key)?;
    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    }) {
        Ok(_) => Ok(true),
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.code == rusqlite::ErrorCode::NotADatabase =>
        {
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
}

/// Gives `conn` the raw key `key`, skipping SQLCipher's own derivation.
fn set_key(conn: &Connection, key: &[u8; 32]) -> Result<()> {
    conn.execute_batch(&raw_key_pragma(key, None))?;
    Ok(())
}

/// The key as SQLCipher's blob literal, `x'…'`, optionally for an attached
/// schema.
fn raw_key_literal(key: &[u8; 32]) -> Zeroizing<String> {
    use std::fmt::Write;
    let mut text = Zeroizing::new(String::with_capacity(67));
    text.push_str("x'");
    for byte in key {
        let _ = write!(text, "{byte:02x}");
    }
    text.push('\'');
    text
}

fn raw_key_pragma(key: &[u8; 32], schema: Option<&str>) -> Zeroizing<String> {
    let literal = raw_key_literal(key);
    let prefix = schema.map_or_else(String::new, |s| format!("{s}."));
    Zeroizing::new(format!("PRAGMA {prefix}key = \"{}\";", literal.as_str()))
}

/// SQLCipher logs failed decryptions to standard error, which is the
/// command line's output; the error itself is reported instead.
fn quiet() {
    static QUIET: Once = Once::new();
    QUIET.call_once(|| {
        if let Ok(conn) = Connection::open_in_memory() {
            let _ = conn.execute_batch("PRAGMA cipher_log_level = NONE;");
        }
    });
}

/// Readies the new connection `conn` to the file at `path`: an encrypted
/// file gets its key. Called before any other statement on it.
pub(crate) fn prepare(conn: &Connection, path: &Path) -> Result<()> {
    quiet();
    if let Some(key) = key_for(path)? {
        set_key(conn, &key)?;
    }
    Ok(())
}

/// Opens the database file at `path` with `flags` and its key, with a busy
/// timeout of five seconds: for copies and backups, not the live database.
pub(crate) fn open_file(path: &Path, flags: OpenFlags) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    prepare(&conn, path)?;
    Ok(conn)
}

/// The key a new file gets in [`export`].
enum Target<'a> {
    /// No key: a plain file.
    Plain,
    /// SQLCipher derives the key from the passphrase and a new salt.
    Passphrase(&'a Passphrase),
    /// This raw key, with a new salt.
    Raw(Zeroizing<[u8; 32]>),
}

/// Writes the whole database on `conn` into the new file `target` with the
/// key `key`, and carries over its schema version.
fn export(conn: &Connection, target: &Path, key: &Target<'_>) -> Result<()> {
    remove_with_journal(target);
    let key_text = match key {
        Target::Plain => Zeroizing::new(String::new()),
        Target::Passphrase(p) => Zeroizing::new(p.expose().to_owned()),
        Target::Raw(k) => raw_key_literal(k),
    };
    conn.execute(
        "ATTACH DATABASE ?1 AS gk_export KEY ?2",
        params![target.to_string_lossy(), key_text.as_str()],
    )?;
    let exported = (|| -> Result<()> {
        conn.query_row("SELECT sqlcipher_export('gk_export')", [], |_| Ok(()))?;
        let version: i64 = conn.query_row("PRAGMA main.user_version", [], |r| r.get(0))?;
        conn.execute_batch(&format!("PRAGMA gk_export.user_version = {version};"))?;
        Ok(())
    })();
    let detached = conn.execute_batch("DETACH DATABASE gk_export;");
    if let Err(e) = exported.and(detached.map_err(StoreError::from)) {
        remove_with_journal(target);
        return Err(e);
    }
    Ok(())
}

fn remove_with_journal(path: &Path) {
    let _ = fs::remove_file(path);
    let mut journal = path.as_os_str().to_owned();
    journal.push("-journal");
    let _ = fs::remove_file(PathBuf::from(journal));
}

/// Encrypts the workspace at `layout` with `passphrase`, or gives an
/// encrypted one this new passphrase. The passphrase is remembered for this
/// process. Needs the workspace to itself; make a backup first.
pub fn encrypt_workspace(layout: &WorkspaceLayout, passphrase: &Passphrase) -> Result<()> {
    if !encryption_available() {
        return Err(StoreError::EncryptionUnavailable);
    }
    convert(layout, &Target::Passphrase(passphrase))?;
    remember_passphrase(passphrase);
    Ok(())
}

/// Turns the encrypted workspace at `layout` back into a plain one. Needs
/// the workspace to itself; make a backup first.
pub fn decrypt_workspace(layout: &WorkspaceLayout) -> Result<()> {
    if encryption_of(&layout.database())? == Encryption::Plain {
        return Ok(());
    }
    convert(layout, &Target::Plain)
}

/// Replaces the workspace database with a copy written with `target`'s key:
///
/// 1. Takes the migrate lock, and the database to itself: switching it out
///    of the write-ahead log fails while another program has it open.
/// 2. Holds an exclusive lock, so no one can open it meanwhile, and writes
///    the copy next to it.
/// 3. Checks the copy (integrity, schema version, workspace ID) and flushes
///    it to disk.
/// 4. Renames the copy over the database: one atomic step. A program that
///    opened the old file while it was locked finds it marked as replaced.
fn convert(layout: &WorkspaceLayout, target: &Target<'_>) -> Result<()> {
    let database = layout.database();
    if !database.is_file() {
        return Err(StoreError::NotAWorkspace(database));
    }
    let _migrate = LockFile::acquire(&layout.migrate_lock())?;
    let conn = connection::open(&database, Purpose::Write, Duration::from_secs(1))?;
    let in_use = |e: StoreError| {
        if e.is_busy() {
            StoreError::InUse(database.clone())
        } else {
            e
        }
    };
    let mode: String = conn
        .query_row("PRAGMA journal_mode = DELETE", [], |r| r.get(0))
        .map_err(|e| in_use(e.into()))?;
    if !mode.eq_ignore_ascii_case("delete") {
        return Err(StoreError::InUse(database));
    }
    conn.execute_batch("PRAGMA locking_mode = EXCLUSIVE; BEGIN EXCLUSIVE; COMMIT;")
        .map_err(|e| in_use(e.into()))?;
    let workspace_id: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'workspace_id'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    let copy = layout
        .root()
        .join(format!(".{}.converting", crate::DATABASE_FILE));
    export(&conn, &copy, target)?;
    let checked = (|| -> Result<()> {
        if let Target::Passphrase(p) = target {
            remember_passphrase(p);
        }
        let check = open_file(&copy, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let problems: Vec<String> = check
            .prepare("PRAGMA integrity_check")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if problems != ["ok"] {
            return Err(StoreError::Corrupt(format!(
                "the converted copy failed its check: {}",
                problems.join("; ")
            )));
        }
        let copy_version: i64 = check.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let copy_id: Option<String> = check
            .query_row(
                "SELECT value FROM meta WHERE key = 'workspace_id'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if copy_version != version || copy_id != workspace_id {
            return Err(StoreError::Corrupt(
                "the converted copy differs from the workspace".into(),
            ));
        }
        drop(check);
        crate::files::sync_file(&copy).map_err(|e| StoreError::io("flushing", &copy, e))?;
        Ok(())
    })();
    if let Err(e) = checked {
        remove_with_journal(&copy);
        return Err(e);
    }
    replace(conn, &copy, &database)?;
    sync_dir(layout.root());
    for leftover in ["-wal", "-shm", "-journal"] {
        let mut name = database.as_os_str().to_owned();
        name.push(leftover);
        let _ = fs::remove_file(PathBuf::from(name));
    }
    Ok(())
}

/// Marks the database on `conn` as replaced, or no longer replaced.
fn set_retired(conn: &Connection, retired: bool) -> Result<()> {
    if retired {
        conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, '1')
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [RETIRED_KEY],
        )?;
    } else {
        conn.execute("DELETE FROM meta WHERE key = ?1", [RETIRED_KEY])?;
    }
    Ok(())
}

/// Renames the checked `copy` over `database`, which `conn` holds open
/// with an exclusive lock, and closes `conn`.
#[cfg(not(windows))]
fn replace(conn: Connection, copy: &Path, database: &Path) -> Result<()> {
    fs::rename(copy, database).map_err(|e| {
        remove_with_journal(copy);
        StoreError::io("replacing", database, e)
    })?;
    // The old file is gone from the folder but still open here: whoever
    // opened it while it was locked must not write to it. Its rollback
    // journal stays in memory: on disk it would carry the new file's name.
    if let Err(e) = conn
        .execute_batch("PRAGMA journal_mode = MEMORY;")
        .map_err(StoreError::from)
        .and_then(|()| set_retired(&conn, true))
    {
        tracing::debug!(error = %e, "marking the replaced database failed");
    }
    Ok(())
}

/// Renames the checked `copy` over `database`, which `conn` holds open
/// with an exclusive lock, and closes `conn`.
///
/// Windows doesn't replace a file that is open, so the old file is marked
/// as replaced first and closed. A program that opens it before the rename
/// finds the mark, and its open file makes the rename fail: then the mark
/// is taken off again and nothing has changed.
#[cfg(windows)]
fn replace(conn: Connection, copy: &Path, database: &Path) -> Result<()> {
    if let Err(e) = set_retired(&conn, true) {
        remove_with_journal(copy);
        return Err(e);
    }
    drop(conn);
    let Err(e) = fs::rename(copy, database) else {
        return Ok(());
    };
    remove_with_journal(copy);
    let restored = open_file(database, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .and_then(|conn| set_retired(&conn, false));
    if let Err(restore) = restored {
        tracing::warn!(error = %restore, "taking off the replaced mark failed");
    }
    Err(
        if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32) {
            // Access denied, or a sharing violation: someone has it open.
            StoreError::InUse(database.to_path_buf())
        } else {
            StoreError::io("replacing", database, e)
        },
    )
}

/// Where the passphrase that opens a workspace came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlocked {
    /// The workspace is plain.
    Plain,
    /// This process knew it already.
    Known,
    /// The environment variable [`PASSPHRASE_ENV`].
    Environment,
}

/// Makes the database at `database` openable without asking the person:
/// with a passphrase this process knows, else the one in
/// [`PASSPHRASE_ENV`]. Fails with [`StoreError::PassphraseRequired`] when
/// there is none, and with [`StoreError::WrongPassphrase`] when the one
/// there doesn't open it.
pub fn unlock_without_asking(database: &Path) -> Result<Unlocked> {
    if encryption_of(database)? == Encryption::Plain {
        return Ok(Unlocked::Plain);
    }
    // Wrong when this process tried a passphrase, else required.
    let missing = match check_key(database) {
        Ok(()) => return Ok(Unlocked::Known),
        Err(e @ (StoreError::PassphraseRequired(_) | StoreError::WrongPassphrase(_))) => e,
        Err(e) => return Err(e),
    };
    if let Some(passphrase) = std::env::var(PASSPHRASE_ENV).ok().and_then(Passphrase::new) {
        return if passphrase_opens(database, &passphrase)? {
            Ok(Unlocked::Environment)
        } else {
            Err(StoreError::WrongPassphrase(database.to_path_buf()))
        };
    }
    Err(missing)
}

/// Encrypts the plain database file at `path` (a backup) with
/// `passphrase`, in place: written to a new file, flushed and renamed over
/// it. An encrypted file is left as it is.
pub fn encrypt_file(path: &Path, passphrase: &Passphrase) -> Result<()> {
    if !encryption_available() {
        return Err(StoreError::EncryptionUnavailable);
    }
    if encryption_of(path)? == Encryption::Encrypted {
        return Ok(());
    }
    let converted = path.with_extension("encrypting");
    {
        let conn = open_file(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        export(&conn, &converted, &Target::Passphrase(passphrase))?;
    }
    let flushed = crate::files::sync_file(&converted);
    if let Err(e) = flushed.and_then(|()| fs::rename(&converted, path)) {
        remove_with_journal(&converted);
        return Err(StoreError::io("replacing", path, e));
    }
    if let Some(dir) = path.parent() {
        sync_dir(dir);
    }
    Ok(())
}

/// Flushes a folder's entries, so a rename in it survives a power cut.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(dir) = fs::File::open(dir) {
        let _ = dir.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Makes the database copy `copy` encrypted, or plain, like the live
/// database `live`, so the backup API can copy it into the live one (it
/// copies only between files of the same kind). A copy that is already
/// like it is left as it is.
pub(crate) fn match_encryption(copy: &Path, live: &Path) -> Result<()> {
    let target = match (encryption_of(copy)?, key_for(live)?) {
        (Encryption::Plain, None) => return Ok(()),
        (Encryption::Encrypted, Some(_)) => return Ok(()),
        (Encryption::Encrypted, None) => Target::Plain,
        (Encryption::Plain, Some(key)) => Target::Raw(key),
    };
    let converted = copy.with_extension("matched");
    {
        // `CREATE`: the attached file inherits the flags.
        let conn = open_file(
            copy,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        export(&conn, &converted, &target)?;
    }
    if let Target::Raw(key) = &target
        && let Some(salt) = read_salt(&converted)?
    {
        // The copy has a salt of its own but the live database's key.
        let mut keys = keys();
        if !keys.derived.iter().any(|(s, _)| *s == salt) {
            keys.derived.push((salt, key.clone()));
        }
    }
    fs::rename(&converted, copy).map_err(|e| {
        remove_with_journal(&converted);
        StoreError::io("replacing", copy, e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passphrases_are_not_shown_and_not_empty() {
        assert!(Passphrase::new("").is_none());
        let p = Passphrase::new("hunter2 correct horse").unwrap();
        assert_eq!(format!("{p:?}"), "Passphrase(…)");
        assert_eq!(p.expose(), "hunter2 correct horse");
    }

    #[test]
    fn the_raw_key_is_sqlciphers_blob_literal() {
        let mut key = [0u8; 32];
        key[0] = 0xab;
        key[31] = 0x01;
        let text = raw_key_literal(&key);
        assert_eq!(text.len(), 67);
        assert!(text.starts_with("x'ab00"));
        assert!(text.ends_with("01'"));
        assert_eq!(
            raw_key_pragma(&key, Some("gk_export")).as_str(),
            format!("PRAGMA gk_export.key = \"{}\";", text.as_str())
        );
    }

    #[test]
    fn a_passphrase_check_matches_only_its_passphrase() {
        let right = Passphrase::new("lock the app").unwrap();
        let check = PassphraseCheck::new(&right).unwrap();
        assert!(check.matches(&right));
        assert!(!check.matches(&Passphrase::new("lock the ap").unwrap()));
        let json = serde_json::to_string(&check).unwrap();
        assert!(!json.contains("lock the app"));
        let back: PassphraseCheck = serde_json::from_str(&json).unwrap();
        assert!(back.matches(&right));
        assert_ne!(
            PassphraseCheck::new(&right).unwrap(),
            check,
            "a new salt each time"
        );
    }

    #[test]
    fn the_linux_and_windows_builds_can_encrypt() {
        assert!(encryption_available());
    }

    #[test]
    fn missing_short_and_plain_files_are_plain() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("none.db");
        assert_eq!(encryption_of(&missing).unwrap(), Encryption::Plain);
        let short = dir.path().join("short.db");
        fs::write(&short, b"abc").unwrap();
        assert_eq!(encryption_of(&short).unwrap(), Encryption::Plain);
        // Not SQLite's header, but not whole pages either: not a database
        // rather than an encrypted one.
        let text = dir.path().join("text.db");
        fs::write(&text, "this is not a database at all").unwrap();
        assert_eq!(encryption_of(&text).unwrap(), Encryption::Plain);
        let ragged = dir.path().join("ragged.db");
        fs::write(&ragged, vec![7u8; 5000]).unwrap();
        assert_eq!(encryption_of(&ragged).unwrap(), Encryption::Plain);
        let pages = dir.path().join("pages.db");
        fs::write(&pages, vec![7u8; 8192]).unwrap();
        assert_eq!(encryption_of(&pages).unwrap(), Encryption::Encrypted);
        let plain = dir.path().join("plain.db");
        Connection::open(&plain)
            .unwrap()
            .execute_batch("CREATE TABLE t (a)")
            .unwrap();
        assert_eq!(encryption_of(&plain).unwrap(), Encryption::Plain);
        assert!(can_open(&plain).unwrap());
    }
}
