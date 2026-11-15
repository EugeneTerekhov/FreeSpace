//! Hashing primitives of the duplicate-detection pipeline.
//!
//! Reading every byte of every file is the expensive part of duplicate search,
//! so the pipeline narrows candidates down before hashing them completely
//! (FR-025, FR-026):
//!
//! 1. files are grouped by size;
//! 2. a partial digest of the first and the last 64 KiB discards almost every
//!    size collision;
//! 3. only survivors get a full SHA-256 or MD5 digest;
//! 4. the survivors are compared byte by byte, which is what actually proves
//!    equality — a hash collision must never mark two different files as
//!    duplicates.
//!
//! Long paths are opened through verbatim forms from `freespace-platform`, so
//! the pipeline does not depend on `LongPathsEnabled`.

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

use freespace_core::{Error, Result};
use freespace_platform::paths;
use md5::Md5;
use sha2::{Digest, Sha256};

/// Number of leading and trailing bytes used by the partial digest.
pub const PARTIAL_CHUNK: usize = 64 * 1024;

/// Buffer size used by the streaming helpers.
const BUFFER: usize = 256 * 1024;

/// Hash algorithm supported by the duplicate search.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    /// SHA-256, the default.
    #[default]
    Sha256,
    /// MD5, kept for compatibility with other tools.
    Md5,
}

impl HashAlgorithm {
    /// Stable machine-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::Md5 => "md5",
        }
    }

    /// Parses a command-line value.
    #[must_use]
    pub fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "sha256" | "sha-256" | "sha2" => Some(Self::Sha256),
            "md5" => Some(Self::Md5),
            _ => None,
        }
    }

    /// Length of a digest in bytes.
    #[must_use]
    pub const fn digest_len(self) -> usize {
        match self {
            Self::Sha256 => 32,
            Self::Md5 => 16,
        }
    }

    fn hasher(self) -> Hasher {
        match self {
            Self::Sha256 => Hasher::Sha256(Sha256::new()),
            Self::Md5 => Hasher::Md5(Md5::new()),
        }
    }
}

/// Result of hashing one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHash {
    /// Digest of the leading and trailing bytes plus the file length.
    pub partial: Vec<u8>,
    /// Full digest with the selected algorithm.
    pub full: Vec<u8>,
}

/// Computes both digests of `path`.
///
/// # Errors
/// Returns [`Error::Io`] when the file cannot be opened or read.
pub fn hash_file(path: &Path, algorithm: HashAlgorithm) -> Result<FileHash> {
    let mut file = open(path)?;
    let length = file.metadata()?.len();

    let partial = partial_digest_of(&mut file, length)?;
    file.seek(SeekFrom::Start(0))?;
    let full = digest_reader(&mut file, algorithm)?;

    Ok(FileHash { partial, full })
}

/// Computes the partial digest of `path`.
///
/// # Errors
/// Returns [`Error::Io`] when the file cannot be opened or read.
pub fn partial_digest(path: &Path) -> Result<Vec<u8>> {
    let mut file = open(path)?;
    let length = file.metadata()?.len();

    partial_digest_of(&mut file, length)
}

/// Computes the full digest of `path`.
///
/// # Errors
/// Returns [`Error::Io`] when the file cannot be opened or read.
pub fn full_digest(path: &Path, algorithm: HashAlgorithm) -> Result<Vec<u8>> {
    let mut file = open(path)?;

    digest_reader(&mut file, algorithm)
}

/// Compares two files byte by byte.
///
/// # Errors
/// Returns [`Error::Io`] when either file cannot be opened or read.
pub fn files_equal(left: &Path, right: &Path) -> Result<bool> {
    let mut left_file = open(left)?;
    let mut right_file = open(right)?;
    let left_len = left_file.metadata()?.len();
    let right_len = right_file.metadata()?.len();
    if left_len != right_len {
        return Ok(false);
    }

    let mut left_buffer = vec![0u8; BUFFER];
    let mut right_buffer = vec![0u8; BUFFER];
    loop {
        let read = read_up_to(&mut left_file, &mut left_buffer)?;
        let right_read = read_up_to(&mut right_file, &mut right_buffer)?;
        if read != right_read {
            return Ok(false);
        }
        if read == 0 {
            return Ok(true);
        }
        if left_buffer[..read] != right_buffer[..right_read] {
            return Ok(false);
        }
    }
}

/// Encodes a digest as lower-case hexadecimal.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }

    text
}

fn open(path: &Path) -> Result<File> {
    let verbatim = paths::to_verbatim(path)?;

    File::open(verbatim).map_err(Error::Io)
}

fn partial_digest_of(file: &mut File, length: u64) -> Result<Vec<u8>> {
    let mut hasher = HashAlgorithm::Sha256.hasher();
    hasher.update(&length.to_le_bytes());

    let chunk = u64::try_from(PARTIAL_CHUNK).unwrap_or(u64::MAX);
    if length <= chunk.saturating_mul(2) {
        file.seek(SeekFrom::Start(0))?;
        hash_prefix(file, u64::MAX, &mut hasher)?;
    } else {
        file.seek(SeekFrom::Start(0))?;
        hash_prefix(file, chunk, &mut hasher)?;
        file.seek(SeekFrom::Start(length - chunk))?;
        hash_prefix(file, chunk, &mut hasher)?;
    }

    Ok(hasher.finalize())
}

/// Feeds up to `limit` bytes from the current position into `hasher`.
fn hash_prefix(file: &mut File, limit: u64, hasher: &mut Hasher) -> Result<()> {
    let mut buffer = vec![0u8; PARTIAL_CHUNK.min(BUFFER)];
    let mut remaining = limit;

    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = file.read(&mut buffer[..wanted])?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        remaining = remaining.saturating_sub(read as u64);
    }

    Ok(())
}

fn digest_reader(file: &mut File, algorithm: HashAlgorithm) -> Result<Vec<u8>> {
    let mut hasher = algorithm.hasher();
    let mut buffer = vec![0u8; BUFFER];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hasher.finalize());
        }
        hasher.update(&buffer[..read]);
    }
}

/// Reads until the buffer is full or the file ends.
fn read_up_to(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        let read = file.read(&mut buffer[filled..])?;
        if read == 0 {
            break;
        }
        filled += read;
    }

    Ok(filled)
}

/// Type-erased streaming hasher.
enum Hasher {
    Sha256(Sha256),
    Md5(Md5),
}

impl Hasher {
    fn update(&mut self, data: &[u8]) {
        match self {
            Self::Sha256(hasher) => Digest::update(hasher, data),
            Self::Md5(hasher) => Digest::update(hasher, data),
        }
    }

    fn finalize(self) -> Vec<u8> {
        match self {
            Self::Sha256(hasher) => hasher.finalize().to_vec(),
            Self::Md5(hasher) => hasher.finalize().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        HashAlgorithm, PARTIAL_CHUNK, files_equal, full_digest, hash_file, hex, partial_digest,
    };

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let directory = tempfile::tempdir().expect("temp dir");
        let left = directory.path().join("left.bin");
        let right = directory.path().join("right.bin");
        let payload: Vec<u8> = (0..10_000).map(|index| (index % 251) as u8).collect();
        std::fs::write(&left, &payload).expect("left");
        std::fs::write(&right, &payload).expect("right");

        (directory, left, right)
    }

    #[test]
    fn parses_algorithms() {
        assert_eq!(
            HashAlgorithm::parse_value("SHA256"),
            Some(HashAlgorithm::Sha256)
        );
        assert_eq!(HashAlgorithm::parse_value("md5"), Some(HashAlgorithm::Md5));
        assert_eq!(HashAlgorithm::parse_value("crc32"), None);
        assert_eq!(HashAlgorithm::Sha256.digest_len(), 32);
        assert_eq!(HashAlgorithm::Md5.digest_len(), 16);
        assert_eq!(HashAlgorithm::Sha256.as_str(), "sha256");
    }

    #[test]
    fn hashes_are_stable_and_algorithm_specific() {
        let (_directory, left, _right) = fixture();
        let sha = hash_file(&left, HashAlgorithm::Sha256).expect("sha256");
        let md5 = hash_file(&left, HashAlgorithm::Md5).expect("md5");

        assert_eq!(sha.full.len(), 32);
        assert_eq!(md5.full.len(), 16);
        assert_eq!(sha.partial, md5.partial, "the partial digest is fixed");
        assert_eq!(hex(&sha.full).len(), 64);
        assert_eq!(
            sha.full,
            full_digest(&left, HashAlgorithm::Sha256).expect("digest")
        );
        assert_eq!(sha.partial, partial_digest(&left).expect("partial"));
        assert_ne!(sha.full, md5.full);
    }

    #[test]
    fn partial_digest_uses_the_edges() {
        let directory = tempfile::tempdir().expect("temp dir");
        let mut payload = vec![b'a'; PARTIAL_CHUNK * 3];
        payload[0] = b'z';
        let first = directory.path().join("first.bin");
        std::fs::write(&first, &payload).expect("first");

        let mut changed = payload.clone();
        changed[PARTIAL_CHUNK + 10] = b'z';
        let middle = directory.path().join("middle.bin");
        std::fs::write(&middle, &changed).expect("middle");

        assert_eq!(
            partial_digest(&first).expect("first"),
            partial_digest(&middle).expect("middle"),
            "the middle is not part of the partial digest"
        );

        let mut tail = payload;
        let last = tail.len() - 1;
        tail[last] = b'z';
        let tailed = directory.path().join("tail.bin");
        std::fs::write(&tailed, &tail).expect("tail");
        assert_ne!(
            partial_digest(&first).expect("first"),
            partial_digest(&tailed).expect("tail")
        );
    }

    #[test]
    fn compares_files_byte_by_byte() {
        let (directory, left, right) = fixture();

        assert!(files_equal(&left, &right).expect("equal"));

        std::fs::write(&right, vec![0u8; 10_000]).expect("different");
        assert!(!files_equal(&left, &right).expect("different"));

        let shorter = directory.path().join("shorter.bin");
        std::fs::write(&shorter, b"short").expect("shorter");
        assert!(!files_equal(&left, &shorter).expect("length mismatch"));
    }

    #[test]
    fn reports_missing_files() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = partial_digest(&directory.path().join("missing.bin")).expect_err("missing");

        assert_eq!(error.code().as_str(), "E_NOT_FOUND");
    }
}
