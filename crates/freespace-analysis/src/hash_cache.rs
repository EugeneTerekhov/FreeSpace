//! Persistent cache of file hashes.
//!
//! Full hashing is the bottleneck of duplicate search, so results are stored in
//! the local SQLite database. The key is the absolute path together with the
//! size and modification time: changing either invalidates the entry, which is
//! what keeps a stale hash from ever being compared (FR-026).
//!
//! The table lives next to the operation journal in the same database file but
//! under its own name, because the journal cache is keyed by NTFS file
//! reference numbers while analysis only has paths.

use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use freespace_core::{Error, Result, Timestamp};
use rusqlite::{Connection, OptionalExtension, params};

use crate::hash::HashAlgorithm;

const SCHEMA: &str = r"
CREATE TABLE IF NOT EXISTS analysis_hash_cache (
    path      TEXT    NOT NULL,
    size      INTEGER NOT NULL,
    mtime     INTEGER NOT NULL,
    algorithm TEXT    NOT NULL,
    hash      BLOB    NOT NULL,
    PRIMARY KEY (path, size, mtime, algorithm)
) WITHOUT ROWID;
";

/// SQLite-backed cache of file hashes.
#[derive(Debug)]
pub struct HashCache {
    connection: Mutex<Connection>,
    path: Option<PathBuf>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl HashCache {
    /// Opens (and creates) the cache in an existing FreeSpace database.
    ///
    /// # Errors
    /// Returns an error when the parent directory cannot be created or SQLite
    /// cannot open the file.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let connection = Connection::open(path).map_err(db)?;
        Self::from_connection(connection, Some(path.to_path_buf()))
    }

    /// Opens a non-persistent cache; used by tests and short-lived commands.
    ///
    /// # Errors
    /// Returns an error when the schema cannot be created.
    pub fn open_in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory().map_err(db)?;
        Self::from_connection(connection, None)
    }

    fn from_connection(connection: Connection, path: Option<PathBuf>) -> Result<Self> {
        let _ = connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get::<_, String>(0));
        connection.execute_batch(SCHEMA).map_err(db)?;

        Ok(Self {
            connection: Mutex::new(connection),
            path,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        })
    }

    /// Database file the cache uses, when it is persistent.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Number of cache hits since the cache was opened.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Number of cache misses since the cache was opened.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Looks up a digest for a file identity.
    ///
    /// # Errors
    /// Returns an error when the query fails.
    pub fn get(
        &self,
        path: &Path,
        size: u64,
        mtime: Timestamp,
        algorithm: HashAlgorithm,
    ) -> Result<Option<Vec<u8>>> {
        let key = key_path(path)?;
        let connection = self.connection.lock().map_err(poisoned)?;
        let hash: Option<Vec<u8>> = connection
            .query_row(
                "SELECT hash FROM analysis_hash_cache
                 WHERE path = ?1 AND size = ?2 AND mtime = ?3 AND algorithm = ?4",
                params![key, to_i64(size), mtime.as_millis(), algorithm.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(db)?;

        if hash.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }

        Ok(hash)
    }

    /// Stores a digest for a file identity.
    ///
    /// # Errors
    /// Returns an error when the record cannot be written.
    pub fn put(
        &self,
        path: &Path,
        size: u64,
        mtime: Timestamp,
        algorithm: HashAlgorithm,
        hash: &[u8],
    ) -> Result<()> {
        let key = key_path(path)?;
        let connection = self.connection.lock().map_err(poisoned)?;
        connection
            .execute(
                "INSERT OR REPLACE INTO analysis_hash_cache
                    (path, size, mtime, algorithm, hash)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    key,
                    to_i64(size),
                    mtime.as_millis(),
                    algorithm.as_str(),
                    hash
                ],
            )
            .map_err(db)?;

        Ok(())
    }

    /// Removes every cached digest.
    ///
    /// # Errors
    /// Returns an error when the table cannot be cleared.
    pub fn clear(&self) -> Result<()> {
        let connection = self.connection.lock().map_err(poisoned)?;
        connection
            .execute("DELETE FROM analysis_hash_cache", [])
            .map_err(db)?;

        Ok(())
    }

    /// Number of stored digests.
    ///
    /// # Errors
    /// Returns an error when the query fails.
    pub fn len(&self) -> Result<u64> {
        let connection = self.connection.lock().map_err(poisoned)?;
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM analysis_hash_cache", [], |row| {
                row.get(0)
            })
            .map_err(db)?;

        Ok(u64::try_from(count).unwrap_or(0))
    }

    /// Returns `true` when nothing is cached.
    ///
    /// # Errors
    /// Returns an error when the query fails.
    pub fn is_empty(&self) -> Result<bool> {
        self.len().map(|count| count == 0)
    }
}

fn key_path(path: &Path) -> Result<String> {
    let absolute = std::path::absolute(path).map_err(|error| Error::InvalidPath {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;

    Ok(absolute.to_string_lossy().into_owned())
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn db(error: impl std::fmt::Display) -> Error {
    Error::Other(format!("hash cache error: {error}"))
}

fn poisoned<T>(_error: std::sync::PoisonError<T>) -> Error {
    Error::Other("hash cache connection is poisoned".to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use freespace_core::Timestamp;

    use super::HashCache;
    use crate::hash::HashAlgorithm;

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("file.bin");
        std::fs::write(&path, b"payload").expect("write");

        (directory, path)
    }

    #[test]
    fn caches_by_path_size_and_mtime() {
        let (_directory, path) = fixture();
        let cache = HashCache::open_in_memory().expect("cache");
        let mtime = Timestamp::from_millis(1_000);

        assert!(
            cache
                .get(&path, 7, mtime, HashAlgorithm::Sha256)
                .expect("get")
                .is_none()
        );
        assert_eq!(cache.misses(), 1);

        cache
            .put(&path, 7, mtime, HashAlgorithm::Sha256, b"digest")
            .expect("put");

        assert_eq!(
            cache
                .get(&path, 7, mtime, HashAlgorithm::Sha256)
                .expect("get"),
            Some(b"digest".to_vec())
        );
        assert_eq!(cache.hits(), 1);
        assert!(
            cache
                .get(&path, 8, mtime, HashAlgorithm::Sha256)
                .expect("get")
                .is_none(),
            "a size change invalidates the entry"
        );
        assert!(
            cache
                .get(
                    &path,
                    7,
                    Timestamp::from_millis(2_000),
                    HashAlgorithm::Sha256
                )
                .expect("get")
                .is_none(),
            "an mtime change invalidates the entry"
        );
        assert!(
            cache
                .get(&path, 7, mtime, HashAlgorithm::Md5)
                .expect("get")
                .is_none(),
            "algorithms never mix"
        );
    }

    #[test]
    fn persists_and_clears() {
        let directory = tempfile::tempdir().expect("temp dir");
        let database = directory.path().join("nested").join("freespace.db");
        let (_files, path) = fixture();
        let mtime = Timestamp::from_millis(1_000);

        {
            let cache = HashCache::open(&database).expect("open");
            cache
                .put(&path, 7, mtime, HashAlgorithm::Sha256, b"digest")
                .expect("put");
            assert_eq!(cache.len().expect("len"), 1);
        }

        let cache = HashCache::open(&database).expect("reopen");
        assert_eq!(
            cache
                .get(&path, 7, mtime, HashAlgorithm::Sha256)
                .expect("get"),
            Some(b"digest".to_vec())
        );
        assert!(cache.path().is_some());
        assert!(!cache.is_empty().expect("empty"));

        cache.clear().expect("clear");
        assert!(cache.is_empty().expect("empty"));
    }
}
