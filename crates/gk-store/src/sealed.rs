// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Sealed files: the stored files, thumbnails and exports of an encrypted
//! workspace, encrypted with ChaCha20-Poly1305 in chunks.
//!
//! # Format (version 1)
//!
//! ```text
//! magic    7 bytes  0x89 "GKSEAL"
//! version  1 byte   1
//! kind     1 byte   1 = the workspace's file key, 2 = a passphrase
//! chunk    1 byte   log2 of the chunk size (16: 64 KiB)
//! reserved 2 bytes  0
//! key id  16 bytes  which key: HKDF-SHA256 of the key, "glyphkeel file key id"
//! salt    32 bytes  random, new for every file
//! [kind 2: KDF salt 16 bytes, PBKDF2-HMAC-SHA512 rounds u32 little-endian]
//! chunks            each: ciphertext + 16-byte tag
//! ```
//!
//! Each file has a key of its own, derived with HKDF-SHA256 from the key
//! the header names and the file's random salt. Chunk `i` is sealed with
//! the nonce `i` (8 bytes, big-endian), three zero bytes and a last-chunk
//! flag, and with the whole header as associated data: a changed header,
//! a changed, moved, dropped or added chunk, and a cut-off file all fail to
//! open. Every chunk but the last holds exactly the chunk size; the last
//! holds less (nothing at all when the content fills whole chunks), so the
//! content's size follows from the file's size and any part of it can be
//! read without reading what comes before.
//!
//! # Keys
//!
//! An encrypted workspace keeps random file keys inside its encrypted
//! database (`meta.file_keys`, the newest first): the newest seals new
//! files, the older ones still open files sealed before a change of
//! passphrase. Since the keys travel inside the database, a file opens with
//! the workspace's passphrase and nothing else, and backups keep the keys
//! their files need. A sealed export (kind 2) has a key from the passphrase
//! itself, so it opens on another computer too.
//!
//! The process keeps the keys of each open workspace in memory
//! ([`set_workspace_keys`], called when a store opens).

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce, Tag};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::cipher::Passphrase;

/// The first seven bytes of every sealed file. The first byte is never the
/// start of UTF-8 text, so no CSV or JSON file looks sealed.
pub const MAGIC: &[u8; 7] = b"\x89GKSEAL";
/// The format version this program writes and reads.
const VERSION: u8 = 1;
/// The chunk size this program writes, as a power of two.
const CHUNK_LOG: u8 = 16;
/// The smallest and largest chunk sizes a file may name.
const CHUNK_LOG_RANGE: std::ops::RangeInclusive<u8> = 10..=24;
/// Length of an authentication tag.
const TAG_LEN: usize = 16;
/// Length of the fixed part of the header.
const FIXED_LEN: usize = 12;
const KEY_ID_LEN: usize = 16;
const SALT_LEN: usize = 32;
const KDF_SALT_LEN: usize = 16;
/// PBKDF2-HMAC-SHA512 rounds for a passphrase-sealed file, as many as
/// SQLCipher uses for the database.
const KDF_ROUNDS: u32 = 256_000;
/// The fewest rounds a file may name, so a crafted header can't make the
/// passphrase cheap to guess against a forged file.
const KDF_ROUNDS_MIN: u32 = 100_000;
/// The most rounds a file may name, so a crafted header can't hang the program.
const KDF_ROUNDS_MAX: u32 = 10_000_000;
const INFO_FILE: &[u8] = b"glyphkeel sealed file v1";
const INFO_ID: &[u8] = b"glyphkeel file key id";

/// What a sealed file's key is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealKind {
    /// One of an encrypted workspace's file keys.
    Workspace,
    /// A key derived from a passphrase (an export).
    Passphrase,
}

/// Why a sealed file can't be opened. Carried inside the [`io::Error`]
/// that reading returns; [`problem_of`] finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealProblem {
    /// No key this program knows opens it.
    NoKey,
    /// Its content was changed or cut off.
    Damaged,
    /// A later version of the format, or not a sealed file at all.
    Unsupported,
}

impl std::fmt::Display for SealProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoKey => "no known key opens this encrypted file",
            Self::Damaged => "the encrypted file is damaged or was changed",
            Self::Unsupported => "the encrypted file has an unknown format",
        })
    }
}

impl std::error::Error for SealProblem {}

fn error(problem: SealProblem) -> io::Error {
    let kind = match problem {
        SealProblem::NoKey => io::ErrorKind::PermissionDenied,
        SealProblem::Damaged | SealProblem::Unsupported => io::ErrorKind::InvalidData,
    };
    io::Error::new(kind, problem)
}

/// The [`SealProblem`] inside `e`, if it is one.
#[must_use]
pub fn problem_of(e: &io::Error) -> Option<SealProblem> {
    e.get_ref()
        .and_then(|inner| inner.downcast_ref::<SealProblem>())
        .copied()
}

/// A 256-bit key: a workspace's file key, or one derived from a
/// passphrase. Wiped from memory when dropped and never shown.
#[derive(Clone, PartialEq, Eq)]
pub struct FileKey(Zeroizing<[u8; 32]>);

impl std::fmt::Debug for FileKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FileKey(…)")
    }
}

impl FileKey {
    /// A new random key.
    pub fn generate() -> io::Result<Self> {
        let mut key = Zeroizing::new([0u8; 32]);
        random(key.as_mut())?;
        Ok(Self(key))
    }

    /// The key from its 64 hexadecimal digits.
    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        let bytes = Zeroizing::new(crate::cipher::unhex(text)?);
        let array: [u8; 32] = bytes.as_slice().try_into().ok()?;
        Some(Self(Zeroizing::new(array)))
    }

    /// The key as 64 hexadecimal digits, to keep in the encrypted database.
    #[must_use]
    pub fn to_hex(&self) -> Zeroizing<String> {
        Zeroizing::new(crate::cipher::hex(self.0.as_ref()))
    }

    /// The key for `passphrase` with `salt` and `rounds`.
    fn from_passphrase(passphrase: &Passphrase, salt: &[u8; KDF_SALT_LEN], rounds: u32) -> Self {
        let mut key = Zeroizing::new([0u8; 32]);
        pbkdf2::pbkdf2_hmac::<sha2::Sha512>(
            passphrase.expose().as_bytes(),
            salt,
            rounds,
            key.as_mut(),
        );
        Self(key)
    }

    /// The ID a header names the key by: tells which key a file needs
    /// without trying them all, and nothing about the key itself.
    #[must_use]
    pub fn id(&self) -> [u8; KEY_ID_LEN] {
        let mut id = [0u8; KEY_ID_LEN];
        let hk = hkdf::Hkdf::<Sha256>::new(None, self.0.as_ref());
        // 16 bytes are always within HKDF's limit.
        let _ = hk.expand(INFO_ID, &mut id);
        id
    }

    /// The key of the file with `salt`.
    fn file_cipher(&self, salt: &[u8; SALT_LEN]) -> ChaCha20Poly1305 {
        let mut key = Zeroizing::new([0u8; 32]);
        let hk = hkdf::Hkdf::<Sha256>::new(Some(salt), self.0.as_ref());
        let _ = hk.expand(INFO_FILE, key.as_mut());
        let array = chacha20poly1305::Key::from(*key);
        ChaCha20Poly1305::new(&array)
    }
}

fn random(out: &mut [u8]) -> io::Result<()> {
    getrandom::fill(out).map_err(|e| io::Error::other(e.to_string()))
}

/// A sealed file's header, as read.
#[derive(Debug, Clone)]
pub struct Header {
    kind: SealKind,
    chunk: usize,
    key_id: [u8; KEY_ID_LEN],
    salt: [u8; SALT_LEN],
    kdf: Option<([u8; KDF_SALT_LEN], u32)>,
    bytes: Vec<u8>,
}

impl Header {
    fn new(
        kind: SealKind,
        key: &FileKey,
        kdf: Option<([u8; KDF_SALT_LEN], u32)>,
    ) -> io::Result<Self> {
        let mut salt = [0u8; SALT_LEN];
        random(&mut salt)?;
        let key_id = key.id();
        let mut bytes = Vec::with_capacity(FIXED_LEN + KEY_ID_LEN + SALT_LEN + 20);
        bytes.extend_from_slice(MAGIC);
        bytes.push(VERSION);
        bytes.push(match kind {
            SealKind::Workspace => 1,
            SealKind::Passphrase => 2,
        });
        bytes.push(CHUNK_LOG);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&key_id);
        bytes.extend_from_slice(&salt);
        if let Some((kdf_salt, rounds)) = &kdf {
            bytes.extend_from_slice(kdf_salt);
            bytes.extend_from_slice(&rounds.to_le_bytes());
        }
        Ok(Self {
            kind,
            chunk: 1 << CHUNK_LOG,
            key_id,
            salt,
            kdf,
            bytes,
        })
    }

    /// Reads a header from the start of `reader`. `Ok(None)` when it
    /// doesn't start with [`MAGIC`]: not a sealed file.
    pub fn read(reader: &mut dyn Read) -> io::Result<Option<Self>> {
        let mut fixed = [0u8; FIXED_LEN];
        let got = read_full(reader, &mut fixed)?;
        if got < MAGIC.len() || &fixed[..MAGIC.len()] != MAGIC {
            return Ok(None);
        }
        if got < FIXED_LEN {
            return Err(error(SealProblem::Damaged));
        }
        if fixed[7] != VERSION || fixed[10] != 0 || fixed[11] != 0 {
            return Err(error(SealProblem::Unsupported));
        }
        let kind = match fixed[8] {
            1 => SealKind::Workspace,
            2 => SealKind::Passphrase,
            _ => return Err(error(SealProblem::Unsupported)),
        };
        if !CHUNK_LOG_RANGE.contains(&fixed[9]) {
            return Err(error(SealProblem::Unsupported));
        }
        let rest_len = KEY_ID_LEN
            + SALT_LEN
            + if kind == SealKind::Passphrase {
                KDF_SALT_LEN + 4
            } else {
                0
            };
        let mut rest = vec![0u8; rest_len];
        if read_full(reader, &mut rest)? < rest_len {
            return Err(error(SealProblem::Damaged));
        }
        let mut key_id = [0u8; KEY_ID_LEN];
        key_id.copy_from_slice(&rest[..KEY_ID_LEN]);
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&rest[KEY_ID_LEN..KEY_ID_LEN + SALT_LEN]);
        let kdf = if kind == SealKind::Passphrase {
            let at = KEY_ID_LEN + SALT_LEN;
            let mut kdf_salt = [0u8; KDF_SALT_LEN];
            kdf_salt.copy_from_slice(&rest[at..at + KDF_SALT_LEN]);
            let mut rounds = [0u8; 4];
            rounds.copy_from_slice(&rest[at + KDF_SALT_LEN..]);
            let rounds = u32::from_le_bytes(rounds);
            if !(KDF_ROUNDS_MIN..=KDF_ROUNDS_MAX).contains(&rounds) {
                return Err(error(SealProblem::Unsupported));
            }
            Some((kdf_salt, rounds))
        } else {
            None
        };
        let mut bytes = fixed.to_vec();
        bytes.extend_from_slice(&rest);
        Ok(Some(Self {
            kind,
            chunk: 1 << fixed[9],
            key_id,
            salt,
            kdf,
            bytes,
        }))
    }

    /// What key the file needs.
    #[must_use]
    pub fn kind(&self) -> SealKind {
        self.kind
    }

    /// The ID of the key the file needs.
    #[must_use]
    pub fn key_id(&self) -> [u8; KEY_ID_LEN] {
        self.key_id
    }

    /// The header's length in bytes.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Never: a header has its fixed fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The content's size for a sealed file of `file_len` bytes; `None` when
    /// no sealed file has that size (it was cut off).
    #[must_use]
    pub fn plain_len(&self, file_len: u64) -> Option<u64> {
        let body = file_len.checked_sub(self.len())?;
        let sealed_chunk = (self.chunk + TAG_LEN) as u64;
        let full = body / sealed_chunk;
        let rest = (body % sealed_chunk).checked_sub(TAG_LEN as u64)?;
        Some(full * self.chunk as u64 + rest)
    }

    /// The key for this file among `passphrases`, derived as its header
    /// says; `None` when none is the one.
    #[must_use]
    pub fn passphrase_key(&self, passphrases: &[Passphrase]) -> Option<FileKey> {
        let (salt, rounds) = self.kdf.as_ref()?;
        passphrases
            .iter()
            .map(|p| FileKey::from_passphrase(p, salt, *rounds))
            .find(|key| key.id() == self.key_id)
    }
}

/// Reads until `buf` is full or the input ends; how much was read.
fn read_full(reader: &mut dyn Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        match reader.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(got)
}

fn nonce(index: u64, last: bool) -> Nonce {
    let mut n = [0u8; 12];
    n[..8].copy_from_slice(&index.to_be_bytes());
    n[11] = u8::from(last);
    Nonce::from(n)
}

/// Writes a sealed file: whatever is written is sealed chunk by chunk.
/// [`SealWriter::finish`] seals the last chunk; a writer dropped without it
/// leaves a file that fails to open, never one that looks complete.
pub struct SealWriter<W: Write> {
    inner: W,
    cipher: ChaCha20Poly1305,
    header: Header,
    buf: Zeroizing<Vec<u8>>,
    index: u64,
}

impl<W: Write> std::fmt::Debug for SealWriter<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealWriter").finish_non_exhaustive()
    }
}

impl<W: Write> SealWriter<W> {
    /// Starts a file sealed with the workspace file key `key`.
    pub fn new(inner: W, key: &FileKey) -> io::Result<Self> {
        let header = Header::new(SealKind::Workspace, key, None)?;
        Self::start(inner, key, header)
    }

    /// Starts a file sealed with a key from `passphrase` (an export).
    pub fn with_passphrase(inner: W, passphrase: &Passphrase) -> io::Result<Self> {
        let mut kdf_salt = [0u8; KDF_SALT_LEN];
        random(&mut kdf_salt)?;
        let key = FileKey::from_passphrase(passphrase, &kdf_salt, KDF_ROUNDS);
        let header = Header::new(SealKind::Passphrase, &key, Some((kdf_salt, KDF_ROUNDS)))?;
        Self::start(inner, &key, header)
    }

    fn start(mut inner: W, key: &FileKey, header: Header) -> io::Result<Self> {
        inner.write_all(&header.bytes)?;
        let cipher = key.file_cipher(&header.salt);
        let chunk = header.chunk;
        Ok(Self {
            inner,
            cipher,
            header,
            buf: Zeroizing::new(Vec::with_capacity(chunk + TAG_LEN)),
            index: 0,
        })
    }

    fn seal_chunk(&mut self, last: bool) -> io::Result<()> {
        let tag = self
            .cipher
            .encrypt_inout_detached(
                &nonce(self.index, last),
                &self.header.bytes,
                self.buf.as_mut_slice().into(),
            )
            .map_err(|_| io::Error::other("sealing failed"))?;
        self.inner.write_all(&self.buf)?;
        self.inner.write_all(tag.as_slice())?;
        self.buf.clear();
        self.index = self
            .index
            .checked_add(1)
            .ok_or_else(|| io::Error::other("file too large to seal"))?;
        Ok(())
    }

    /// Seals the last chunk and hands back the writer underneath.
    pub fn finish(mut self) -> io::Result<W> {
        self.seal_chunk(true)?;
        self.inner.flush()?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for SealWriter<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let room = self.header.chunk - self.buf.len();
        let n = room.min(data.len());
        self.buf.extend_from_slice(&data[..n]);
        // A full chunk is never the last one: the last holds less.
        if self.buf.len() == self.header.chunk {
            self.seal_chunk(false)?;
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Reads a sealed file's content, from anywhere in it.
pub struct UnsealReader<R: Read + Seek> {
    inner: R,
    cipher: ChaCha20Poly1305,
    header: Header,
    plain_len: u64,
    chunks: u64,
    pos: u64,
    /// The chunk in `buf`, if any.
    loaded: Option<u64>,
    buf: Zeroizing<Vec<u8>>,
}

impl<R: Read + Seek> std::fmt::Debug for UnsealReader<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnsealReader")
            .field("plain_len", &self.plain_len)
            .finish_non_exhaustive()
    }
}

impl<R: Read + Seek> UnsealReader<R> {
    /// Opens the sealed file `inner` (at its start), whose header was read
    /// already as `header`, with `key`.
    pub fn new(mut inner: R, header: Header, key: &FileKey) -> io::Result<Self> {
        if key.id() != header.key_id {
            return Err(error(SealProblem::NoKey));
        }
        let file_len = inner.seek(SeekFrom::End(0))?;
        let plain_len = header
            .plain_len(file_len)
            .ok_or_else(|| error(SealProblem::Damaged))?;
        let chunks = plain_len / header.chunk as u64 + 1;
        let cipher = key.file_cipher(&header.salt);
        let chunk = header.chunk;
        let mut reader = Self {
            inner,
            cipher,
            header,
            plain_len,
            chunks,
            pos: 0,
            loaded: None,
            buf: Zeroizing::new(Vec::with_capacity(chunk + TAG_LEN)),
        };
        // The first chunk proves the key and the header.
        reader.load(0)?;
        Ok(reader)
    }

    /// The content's size.
    #[must_use]
    pub fn plain_len(&self) -> u64 {
        self.plain_len
    }

    fn load(&mut self, index: u64) -> io::Result<()> {
        if self.loaded == Some(index) {
            return Ok(());
        }
        self.loaded = None;
        let last = index + 1 == self.chunks;
        let len = if last {
            (self.plain_len - index * self.header.chunk as u64) as usize
        } else {
            self.header.chunk
        };
        let at = self.header.len() + index * (self.header.chunk + TAG_LEN) as u64;
        self.inner.seek(SeekFrom::Start(at))?;
        self.buf.resize(len + TAG_LEN, 0);
        if read_full(&mut self.inner, &mut self.buf)? < len + TAG_LEN {
            return Err(error(SealProblem::Damaged));
        }
        let tag = Tag::try_from(&self.buf[len..]).map_err(|_| error(SealProblem::Damaged))?;
        self.buf.truncate(len);
        self.cipher
            .decrypt_inout_detached(
                &nonce(index, last),
                &self.header.bytes,
                self.buf.as_mut_slice().into(),
                &tag,
            )
            .map_err(|_| error(SealProblem::Damaged))?;
        self.loaded = Some(index);
        Ok(())
    }
}

impl<R: Read + Seek> Read for UnsealReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.plain_len || out.is_empty() {
            return Ok(0);
        }
        let chunk = self.header.chunk as u64;
        let index = self.pos / chunk;
        if let Err(e) = self.load(index) {
            self.buf.clear();
            return Err(e);
        }
        let offset = (self.pos - index * chunk) as usize;
        let n = (self.buf.len() - offset).min(out.len());
        out[..n].copy_from_slice(&self.buf[offset..offset + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl<R: Read + Seek> Seek for UnsealReader<R> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::End(n) => i128::from(self.plain_len) + i128::from(n),
            SeekFrom::Current(n) => i128::from(self.pos) + i128::from(n),
        };
        let target = u64::try_from(target)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        self.pos = target;
        Ok(target)
    }
}

/// Something to read a file's content from: a plain file, or a sealed
/// one opened.
pub enum Content {
    /// The file as it is.
    Plain(File),
    /// A sealed file, opened.
    Sealed(Box<UnsealReader<File>>),
}

impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain(_) => f.write_str("Content::Plain"),
            Self::Sealed(_) => f.write_str("Content::Sealed"),
        }
    }
}

impl Content {
    /// Whether the file was sealed.
    #[must_use]
    pub fn is_sealed(&self) -> bool {
        matches!(self, Self::Sealed(_))
    }

    /// The content's size.
    pub fn len(&self) -> io::Result<u64> {
        match self {
            Self::Plain(file) => Ok(file.metadata()?.len()),
            Self::Sealed(reader) => Ok(reader.plain_len()),
        }
    }

    /// Whether there is no content.
    pub fn is_empty(&self) -> io::Result<bool> {
        self.len().map(|n| n == 0)
    }
}

impl Read for Content {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(f) => f.read(buf),
            Self::Sealed(r) => r.read(buf),
        }
    }
}

impl Seek for Content {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Plain(f) => f.seek(to),
            Self::Sealed(r) => r.seek(to),
        }
    }
}

/// Whether the file at `path` starts as a sealed file does.
pub fn looks_sealed(path: &Path) -> io::Result<bool> {
    let mut head = [0u8; 7];
    let mut file = File::open(path)?;
    Ok(read_full(&mut file, &mut head)? == head.len() && &head == MAGIC)
}

/// Opens the file at `path` for reading its content: a sealed file with
/// the key `find` gives for its header (`None`: no key), anything else as
/// it is.
pub(crate) fn open_with(
    path: &Path,
    find: &dyn Fn(&Header) -> Option<FileKey>,
) -> io::Result<Content> {
    let mut file = File::open(path)?;
    let Some(header) = Header::read(&mut file)? else {
        file.seek(SeekFrom::Start(0))?;
        return Ok(Content::Plain(file));
    };
    let key = find(&header).ok_or_else(|| error(SealProblem::NoKey))?;
    file.seek(SeekFrom::Start(0))?;
    Ok(Content::Sealed(Box::new(UnsealReader::new(
        file, header, &key,
    )?)))
}

/// Opens the file at `path`, sealed with a passphrase (an export) or
/// plain: a sealed one with the first of `passphrases` that opens it.
pub fn open_with_passphrases(path: &Path, passphrases: &[Passphrase]) -> io::Result<Content> {
    open_with(path, &|header| match header.kind() {
        SealKind::Passphrase => header.passphrase_key(passphrases),
        SealKind::Workspace => None,
    })
}

// ---------- the keys of open workspaces ----------

/// The file keys of one workspace this process has open.
#[derive(Debug, Clone)]
struct WorkspaceKeys {
    root: PathBuf,
    /// The key new files are sealed with; `None` for a plain workspace.
    current: Option<FileKey>,
    /// Every key the workspace keeps, the current one included.
    all: Vec<FileKey>,
}

static WORKSPACES: Mutex<Vec<WorkspaceKeys>> = Mutex::new(Vec::new());

fn workspaces() -> MutexGuard<'static, Vec<WorkspaceKeys>> {
    WORKSPACES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The folder as the registry knows it: resolved, so two spellings of one
/// folder are one entry.
fn registry_root(root: &Path) -> PathBuf {
    fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

/// Records the file keys of the workspace at `root`: `current` seals new
/// files (none for a plain workspace), `all` opens files.
pub fn set_workspace_keys(root: &Path, current: Option<FileKey>, mut all: Vec<FileKey>) {
    if let Some(key) = &current
        && !all.contains(key)
    {
        all.insert(0, key.clone());
    }
    let root = registry_root(root);
    let mut list = workspaces();
    list.retain(|w| w.root != root);
    list.push(WorkspaceKeys { root, current, all });
}

/// The key that seals new files of the workspace at `root`, and every key
/// it keeps; `None` when this process hasn't opened it.
#[must_use]
pub fn workspace_keys(root: &Path) -> Option<(Option<FileKey>, Vec<FileKey>)> {
    let root = registry_root(root);
    workspaces()
        .iter()
        .find(|w| w.root == root)
        .map(|w| (w.current.clone(), w.all.clone()))
}

/// Adds `keys` to the keys that open files of the workspace at `root`
/// (from a backup about to be restored), without changing which seals.
pub fn learn_workspace_keys(root: &Path, keys: &[FileKey]) {
    let (current, mut all) = workspace_keys(root).unwrap_or((None, Vec::new()));
    for key in keys {
        if !all.contains(key) {
            all.push(key.clone());
        }
    }
    set_workspace_keys(root, current, all);
}

/// The workspace file key with ID `id`, among those of every workspace
/// this process has open.
fn known_key(id: &[u8; KEY_ID_LEN]) -> Option<FileKey> {
    workspaces()
        .iter()
        .flat_map(|w| w.all.iter())
        .find(|k| k.id() == *id)
        .cloned()
}

/// Opens a stored file, a thumbnail or a backup's stored file at `path`:
/// a sealed one with a workspace key this process knows.
pub fn open_stored(path: &Path) -> io::Result<Content> {
    open_with(path, &|header| match header.kind() {
        SealKind::Workspace => known_key(&header.key_id()),
        SealKind::Passphrase => None,
    })
}

/// The content's size of the file at `path`, plain or sealed, read from
/// its header and its size: no key needed. `None` for a sealed file that
/// was cut off.
pub fn content_len(path: &Path) -> io::Result<Option<u64>> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    match Header::read(&mut file) {
        Ok(Some(header)) => Ok(header.plain_len(file_len)),
        Ok(None) => Ok(Some(file_len)),
        Err(e) if problem_of(&e).is_some() => Ok(None),
        Err(e) => Err(e),
    }
}

/// What [`convert_blobs`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct Converted {
    /// Files rewritten.
    pub converted: u64,
    /// Files already as wanted.
    pub unchanged: u64,
    /// Files left as they were: no known key opens them, or their content
    /// doesn't match their name.
    pub failed: u64,
}

impl Converted {
    fn add(&mut self, other: Self) {
        self.converted += other.converted;
        self.unchanged += other.unchanged;
        self.failed += other.failed;
    }
}

/// What a stored file is now.
#[derive(Debug, PartialEq, Eq)]
enum State {
    Plain,
    SealedWith([u8; KEY_ID_LEN]),
}

fn state_of(path: &Path) -> io::Result<State> {
    let mut file = File::open(path)?;
    Ok(match Header::read(&mut file) {
        Ok(Some(header)) if header.kind() == SealKind::Workspace => {
            State::SealedWith(header.key_id)
        }
        // A passphrase-sealed file or one with a later format is a plain
        // attachment that happens to look sealed: someone attached an
        // encrypted export.
        Ok(_) | Err(_) => State::Plain,
    })
}

/// Puts every stored file in the content-addressed folder `dir`
/// (`<two digits>/<SHA-256>`, as `files/` and a backup folder's store) into
/// the wanted state: sealed with `target`, or plain when it is `None`.
///
/// Each file is written anew under a temporary name, its content checked
/// against its name (the SHA-256), flushed and renamed over the old one: a
/// crash leaves every file either as it was or as wanted, and both open.
/// Hard links to the old file (a backup's copy) keep the old content.
/// When `linked` names another such folder, a file there that is already
/// as wanted is linked instead of converted again.
pub fn convert_blobs(
    dir: &Path,
    target: Option<&FileKey>,
    linked: Option<&Path>,
) -> io::Result<Converted> {
    let mut done = Converted::default();
    let Ok(folders) = fs::read_dir(dir) else {
        return Ok(done);
    };
    let wanted = target
        .map(|k| State::SealedWith(k.id()))
        .unwrap_or(State::Plain);
    for folder in folders.flatten() {
        let name = folder.file_name().to_string_lossy().into_owned();
        if name.len() != 2 || !folder.path().is_dir() {
            continue;
        }
        let Ok(files) = fs::read_dir(folder.path()) else {
            continue;
        };
        let mut part = Converted::default();
        for file in files.flatten() {
            let sha = file.file_name().to_string_lossy().into_owned();
            if !is_sha256(&sha) || !sha.starts_with(&name) {
                continue;
            }
            let path = file.path();
            match state_of(&path) {
                Ok(state) if state == wanted => {
                    part.unchanged += 1;
                    continue;
                }
                Ok(_) => {}
                Err(_) => {
                    part.failed += 1;
                    continue;
                }
            }
            if let Some(other) = linked.map(|l| l.join(&name).join(&sha))
                && other.is_file()
                && state_of(&other).is_ok_and(|s| s == wanted)
                && link_over(&other, &path).is_ok()
            {
                part.converted += 1;
                continue;
            }
            match convert_one(&path, &sha, target) {
                Ok(()) => part.converted += 1,
                Err(e) => {
                    tracing::warn!(sha256 = %sha, error = %e, "a stored file could not be converted");
                    part.failed += 1;
                }
            }
        }
        if part.converted > 0 {
            sync_dir(&folder.path())?;
        }
        done.add(part);
    }
    Ok(done)
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A temporary name next to `path`.
fn temp_next_to(path: &Path, what: &str) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{name}.{}.{what}", std::process::id()))
}

fn link_over(source: &Path, target: &Path) -> io::Result<()> {
    let temp = temp_next_to(target, "link");
    let _ = fs::remove_file(&temp);
    fs::hard_link(source, &temp)?;
    fs::rename(&temp, target).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

/// Rewrites the stored file `path` (named `sha`) plain or sealed with
/// `target`, checked against its name.
fn convert_one(path: &Path, sha: &str, target: Option<&FileKey>) -> io::Result<()> {
    let temp = temp_next_to(path, "converting");
    let written = (|| {
        let mut source = open_stored(path)?;
        let out = File::create(&temp)?;
        let mut hasher = Sha256::new();
        let mut buf = Zeroizing::new(vec![0u8; 64 * 1024]);
        let out = match target {
            Some(key) => {
                let mut sealed = SealWriter::new(io::BufWriter::new(out), key)?;
                copy_hashing(&mut source, &mut sealed, &mut hasher, &mut buf)?;
                sealed
                    .finish()?
                    .into_inner()
                    .map_err(io::IntoInnerError::into_error)?
            }
            None => {
                let mut out = io::BufWriter::new(out);
                copy_hashing(&mut source, &mut out, &mut hasher, &mut buf)?;
                out.into_inner().map_err(io::IntoInnerError::into_error)?
            }
        };
        if gk_core::hex(&hasher.finalize()) != sha {
            return Err(error(SealProblem::Damaged));
        }
        out.sync_all()?;
        drop(out);
        fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

fn copy_hashing(
    from: &mut dyn Read,
    to: &mut dyn Write,
    hasher: &mut Sha256,
    buf: &mut [u8],
) -> io::Result<u64> {
    let mut total = 0;
    loop {
        let n = match from.read(buf) {
            Ok(0) => return Ok(total),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        hasher.update(&buf[..n]);
        to.write_all(&buf[..n])?;
        total += n as u64;
    }
}

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

// ---------- the keys in the database ----------

/// The `meta` key that holds an encrypted workspace's file keys.
const KEYS_META: &str = "file_keys";

/// The file keys stored in the database on `conn`, the newest first.
fn read_keys(conn: &rusqlite::Connection) -> crate::Result<Vec<FileKey>> {
    use rusqlite::OptionalExtension as _;
    let text: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [KEYS_META], |r| {
            r.get(0)
        })
        .optional()?;
    let Some(text) = text.map(Zeroizing::new) else {
        return Ok(Vec::new());
    };
    let list: Vec<Zeroizing<String>> = serde_json::from_str::<Vec<String>>(&text)
        .map_err(|e| crate::StoreError::Corrupt(format!("the file keys are unreadable: {e}")))?
        .into_iter()
        .map(Zeroizing::new)
        .collect();
    list.iter()
        .map(|k| {
            FileKey::from_hex(k)
                .ok_or_else(|| crate::StoreError::Corrupt("a file key is unreadable".into()))
        })
        .collect()
}

/// Stores `keys` (the newest first) in the database on `conn`; none
/// removes the row.
fn write_keys(conn: &rusqlite::Connection, keys: &[FileKey]) -> crate::Result<()> {
    if keys.is_empty() {
        conn.execute("DELETE FROM meta WHERE key = ?1", [KEYS_META])?;
        return Ok(());
    }
    let list: Vec<Zeroizing<String>> = keys.iter().map(FileKey::to_hex).collect();
    let texts: Vec<&str> = list.iter().map(|t| t.as_str()).collect();
    let text = Zeroizing::new(
        serde_json::to_string(&texts)
            .map_err(|e| crate::StoreError::Corrupt(format!("the file keys: {e}")))?,
    );
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        rusqlite::params![KEYS_META, text.as_str()],
    )?;
    Ok(())
}

/// Readies the file keys of the workspace at `layout` as its store opens on
/// `conn`: an encrypted workspace without a key gets one, and this
/// process remembers them. A plain workspace seals nothing; keys it still
/// holds (an interrupted decryption) still open files.
pub(crate) fn load_for_store(
    conn: &rusqlite::Connection,
    layout: &crate::WorkspaceLayout,
) -> crate::Result<()> {
    let encrypted =
        crate::cipher::encryption_of(&layout.database())? == crate::Encryption::Encrypted;
    let mut keys = read_keys(conn)?;
    if encrypted && keys.is_empty() {
        let key = FileKey::generate()
            .map_err(|e| crate::StoreError::io("making a file key", layout.root(), e))?;
        // Two programs opening at once: the first key stays.
        conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO NOTHING",
            rusqlite::params![
                KEYS_META,
                serde_json::to_string(&[key.to_hex().as_str()]).unwrap_or_default()
            ],
        )?;
        keys = read_keys(conn)?;
    }
    let current = if encrypted {
        keys.first().cloned()
    } else {
        None
    };
    set_workspace_keys(layout.root(), current, keys);
    Ok(())
}

/// The file keys stored in the database file at `database` (a backup), the
/// newest first; none for a plain one or one without keys.
pub fn file_keys_of(database: &Path) -> crate::Result<Vec<FileKey>> {
    let conn = crate::cipher::open_file(database, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let has_meta: bool = conn.query_row(
        "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'meta'",
        [],
        |r| r.get(0),
    )?;
    if !has_meta {
        return Ok(Vec::new());
    }
    read_keys(&conn)
}

/// Adds `keys` to the file keys kept in the database file at `database`
/// (a backup), after those it has: so its stored files, converted with
/// them, still open from the backup alone.
pub fn add_file_keys_to(database: &Path, keys: &[FileKey]) -> crate::Result<()> {
    let conn = crate::cipher::open_file(database, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let mut all = read_keys(&conn)?;
    let before = all.len();
    for key in keys {
        if !all.contains(key) {
            all.push(key.clone());
        }
    }
    if all.len() != before {
        write_keys(&conn, &all)?;
    }
    Ok(())
}

/// Stores `keys` (the newest first, none to remove them) as the file keys
/// of the workspace at `layout`, and remembers them for this process: the
/// first seals new files when the workspace is encrypted.
pub fn save_file_keys(layout: &crate::WorkspaceLayout, keys: &[FileKey]) -> crate::Result<()> {
    let conn = crate::connection::open(
        &layout.database(),
        crate::connection::Purpose::Write,
        std::time::Duration::from_secs(5),
    )?;
    write_keys(&conn, keys)?;
    let encrypted =
        crate::cipher::encryption_of(&layout.database())? == crate::Encryption::Encrypted;
    let current = if encrypted {
        keys.first().cloned()
    } else {
        None
    };
    set_workspace_keys(layout.root(), current, keys.to_vec());
    Ok(())
}

/// The file keys of the workspace at `layout` as its database holds them,
/// remembered for this process as [`save_file_keys`] would. For a closed
/// workspace, before converting its files.
pub fn load_file_keys(layout: &crate::WorkspaceLayout) -> crate::Result<Vec<FileKey>> {
    let conn = crate::connection::open(
        &layout.database(),
        crate::connection::Purpose::Write,
        std::time::Duration::from_secs(5),
    )?;
    load_for_store(&conn, layout)?;
    read_keys(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seal(key: &FileKey, content: &[u8]) -> Vec<u8> {
        let mut w = SealWriter::new(Vec::new(), key).unwrap();
        w.write_all(content).unwrap();
        w.finish().unwrap()
    }

    fn unseal(key: &FileKey, sealed: &[u8]) -> io::Result<Vec<u8>> {
        let mut cursor = io::Cursor::new(sealed);
        let header = Header::read(&mut cursor)?.expect("sealed");
        cursor.set_position(0);
        let mut r = UnsealReader::new(cursor, header, key)?;
        let mut out = Vec::new();
        r.read_to_end(&mut out)?;
        Ok(out)
    }

    fn content(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 7 + i / 251) as u8).collect()
    }

    #[test]
    fn content_of_every_size_round_trips() {
        let key = FileKey::generate().unwrap();
        let chunk = 1usize << CHUNK_LOG;
        for n in [
            0,
            1,
            100,
            chunk - 1,
            chunk,
            chunk + 1,
            3 * chunk,
            3 * chunk + 17,
        ] {
            let plain = content(n);
            let sealed = seal(&key, &plain);
            assert_eq!(&sealed[..7], MAGIC);
            assert_ne!(&sealed[60..], &plain[..], "{n}: not the content itself");
            let mut cursor = io::Cursor::new(&sealed);
            let header = Header::read(&mut cursor).unwrap().unwrap();
            assert_eq!(header.plain_len(sealed.len() as u64), Some(n as u64));
            assert_eq!(unseal(&key, &sealed).unwrap(), plain, "{n}");
        }
    }

    #[test]
    fn the_same_content_seals_differently_each_time() {
        let key = FileKey::generate().unwrap();
        assert_ne!(seal(&key, b"same"), seal(&key, b"same"));
    }

    #[test]
    fn another_key_does_not_open_it() {
        let key = FileKey::generate().unwrap();
        let other = FileKey::generate().unwrap();
        let sealed = seal(&key, b"secret");
        let e = unseal(&other, &sealed).unwrap_err();
        assert_eq!(problem_of(&e), Some(SealProblem::NoKey));
    }

    #[test]
    fn any_change_is_found() {
        let key = FileKey::generate().unwrap();
        let chunk = 1usize << CHUNK_LOG;
        let sealed = seal(&key, &content(2 * chunk + 5));
        // A flipped bit anywhere after the key ID: salt, chunks, tags.
        for at in [30, 60, 61, 60 + chunk, sealed.len() - 1] {
            let mut bad = sealed.clone();
            bad[at] ^= 1;
            let e = unseal(&key, &bad).unwrap_err();
            assert_eq!(problem_of(&e), Some(SealProblem::Damaged), "at {at}");
        }
        // A changed header field.
        let mut bad = sealed.clone();
        bad[9] = 15;
        assert!(unseal(&key, &bad).is_err());
        // Cut off: at a chunk's end, inside one, and the last chunk gone.
        for len in [
            sealed.len() - 1,
            60 + chunk + TAG_LEN,
            60 + 2 * (chunk + TAG_LEN),
        ] {
            let e = unseal(&key, &sealed[..len]).unwrap_err();
            assert_eq!(problem_of(&e), Some(SealProblem::Damaged), "cut at {len}");
        }
        // Two chunks swapped.
        let mut swapped = sealed[..60].to_vec();
        swapped.extend_from_slice(&sealed[60 + chunk + TAG_LEN..60 + 2 * (chunk + TAG_LEN)]);
        swapped.extend_from_slice(&sealed[60..60 + chunk + TAG_LEN]);
        swapped.extend_from_slice(&sealed[60 + 2 * (chunk + TAG_LEN)..]);
        assert!(unseal(&key, &swapped).is_err());
    }

    #[test]
    fn any_part_reads_without_the_rest() {
        let key = FileKey::generate().unwrap();
        let chunk = 1usize << CHUNK_LOG;
        let plain = content(4 * chunk + 999);
        let sealed = seal(&key, &plain);
        let mut cursor = io::Cursor::new(&sealed);
        let header = Header::read(&mut cursor).unwrap().unwrap();
        cursor.set_position(0);
        let mut r = UnsealReader::new(cursor, header, &key).unwrap();
        for at in [3 * chunk + 10, 5, chunk - 2, 4 * chunk + 998] {
            r.seek(SeekFrom::Start(at as u64)).unwrap();
            let mut buf = [0u8; 8];
            let n = read_full(&mut r, &mut buf).unwrap();
            assert_eq!(&buf[..n], &plain[at..(at + 8).min(plain.len())]);
        }
        assert_eq!(r.seek(SeekFrom::End(0)).unwrap(), plain.len() as u64);
        assert_eq!(r.read(&mut [0u8; 4]).unwrap(), 0);
    }

    #[test]
    fn a_passphrase_sealed_file_opens_with_its_passphrase_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.zip.gkenc");
        let right = Passphrase::new("export passphrase").unwrap();
        let mut w = SealWriter::with_passphrase(File::create(&path).unwrap(), &right).unwrap();
        w.write_all(b"archive").unwrap();
        w.finish().unwrap();
        let wrong = Passphrase::new("another").unwrap();
        let e = open_with_passphrases(&path, std::slice::from_ref(&wrong)).unwrap_err();
        assert_eq!(problem_of(&e), Some(SealProblem::NoKey));
        let mut opened = open_with_passphrases(&path, &[wrong, right]).unwrap();
        assert!(opened.is_sealed());
        let mut out = String::new();
        opened.read_to_string(&mut out).unwrap();
        assert_eq!(out, "archive");
        // A workspace key never opens it.
        assert!(open_stored(&path).is_err());
        // A plain file reads as it is.
        let plain = dir.path().join("plain.csv");
        fs::write(&plain, "a,b").unwrap();
        assert!(!open_with_passphrases(&plain, &[]).unwrap().is_sealed());
    }

    #[test]
    fn keys_keep_to_their_hex_form_and_never_show() {
        let key = FileKey::generate().unwrap();
        let text = key.to_hex();
        assert_eq!(text.len(), 64);
        assert_eq!(FileKey::from_hex(&text), Some(key.clone()));
        assert_eq!(format!("{key:?}"), "FileKey(…)");
        assert!(FileKey::from_hex("abc").is_none());
    }

    #[test]
    fn stored_files_convert_both_ways_and_keep_hard_links_apart() {
        let dir = tempfile::tempdir().unwrap();
        let files = dir.path().join("files");
        let backup = dir.path().join("backup");
        let key = FileKey::generate().unwrap();
        set_workspace_keys(dir.path(), Some(key.clone()), vec![]);
        let mut names = Vec::new();
        for n in 0..5 {
            let plain = content(70_000 + n);
            let sha = crate::files::sha256_hex(&plain);
            let path = files.join(&sha[..2]).join(&sha);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, &plain).unwrap();
            let copy = backup.join(&sha[..2]).join(&sha);
            fs::create_dir_all(copy.parent().unwrap()).unwrap();
            fs::hard_link(&path, &copy).unwrap();
            names.push((sha, plain));
        }
        // A damaged file is left alone.
        let bad = files.join("ab").join("ab".repeat(32));
        fs::create_dir_all(bad.parent().unwrap()).unwrap();
        fs::write(&bad, b"not its hash").unwrap();

        let done = convert_blobs(&files, Some(&key), None).unwrap();
        assert_eq!((done.converted, done.failed), (5, 1));
        for (sha, plain) in &names {
            let path = files.join(&sha[..2]).join(sha);
            assert!(looks_sealed(&path).unwrap());
            let mut out = Vec::new();
            open_stored(&path).unwrap().read_to_end(&mut out).unwrap();
            assert_eq!(&out, plain);
            assert_eq!(content_len(&path).unwrap(), Some(plain.len() as u64));
            // The backup's link still has the plain content.
            assert_eq!(&fs::read(backup.join(&sha[..2]).join(sha)).unwrap(), plain);
        }
        // Again: nothing to do.
        assert_eq!(
            convert_blobs(&files, Some(&key), None).unwrap().converted,
            0
        );
        // The backup's store takes the converted files as links.
        let linked = convert_blobs(&backup, Some(&key), Some(&files)).unwrap();
        assert_eq!(linked.converted, 5);
        // And back to plain.
        let back = convert_blobs(&files, None, None).unwrap();
        assert_eq!(back.converted, 5);
        for (sha, plain) in &names {
            assert_eq!(&fs::read(files.join(&sha[..2]).join(sha)).unwrap(), plain);
        }
        let leftovers = fs::read_dir(files.join(&names[0].0[..2]))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with('.')
            })
            .count();
        assert_eq!(leftovers, 0);
    }
}
