//! SQLite persistence for the journal, the quarantine index and the hash cache.
//!
//! The database is a single local file. Schema changes are applied through
//! numbered migrations tracked by `PRAGMA user_version`, so a partially written
//! upgrade can be retried safely.

use std::{
    fmt::Display,
    fs,
    path::{Path, PathBuf},
};

use freespace_core::{Error, Result, Timestamp, Uuid};
use rusqlite::{Connection, OptionalExtension, Row, params, types::Type};

use crate::{
    operation::{JournalEntry, JournalResult},
    quarantine::{QuarantineItem, QuarantineStatus},
};

/// Current schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// Returns the default database location: `<exe_dir>\data\freespace.db`.
///
/// The journal, quarantine index and hash cache share this application-owned
/// file. FreeSpace never resolves it through `%LOCALAPPDATA%`, `%APPDATA%`,
/// `%ProgramData%` or a temporary directory (FR-093).
///
/// # Errors
/// Returns [`Error::Config`] when the executable location is unknown.
pub fn default_path() -> Result<PathBuf> {
    Ok(freespace_core::settings::data_root()?.join("freespace.db"))
}

const MIGRATION_1: &str = r"
CREATE TABLE journal (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id   TEXT    NOT NULL,
    timestamp      INTEGER NOT NULL,
    user           TEXT    NOT NULL,
    summary        TEXT    NOT NULL,
    affected_count INTEGER NOT NULL,
    affected_bytes INTEGER NOT NULL,
    result         TEXT    NOT NULL,
    undo_available INTEGER NOT NULL,
    details        TEXT    NOT NULL
);

CREATE INDEX journal_timestamp_idx ON journal (timestamp DESC, id DESC);
CREATE INDEX journal_operation_idx ON journal (operation_id);

CREATE TABLE quarantine (
    id            TEXT    PRIMARY KEY,
    original_path TEXT    NOT NULL,
    stored_path   TEXT    NOT NULL,
    volume_id     TEXT    NOT NULL,
    size          INTEGER NOT NULL,
    deleted_at    INTEGER NOT NULL,
    expires_at    INTEGER NOT NULL,
    reason        TEXT    NOT NULL,
    status        TEXT    NOT NULL,
    checksum      TEXT    NOT NULL
);

CREATE INDEX quarantine_status_idx ON quarantine (status);
CREATE INDEX quarantine_expiry_idx ON quarantine (expires_at);

CREATE TABLE hash_cache (
    volume_serial INTEGER NOT NULL,
    frn           INTEGER NOT NULL,
    size          INTEGER NOT NULL,
    mtime         INTEGER NOT NULL,
    algorithm     TEXT    NOT NULL,
    hash          BLOB    NOT NULL,
    PRIMARY KEY (volume_serial, frn, size, mtime, algorithm)
) WITHOUT ROWID;
";

const MIGRATIONS: &[&str] = &[MIGRATION_1];

/// Identifies a cached file hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HashKey {
    /// Volume serial number.
    pub volume_serial: u32,
    /// NTFS file reference number.
    pub frn: u64,
    /// File size.
    pub size: u64,
    /// Last modification time.
    pub mtime: Timestamp,
}

/// Local SQLite store.
#[derive(Debug)]
pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens (and creates) the database file, applies migrations and enables
    /// write-ahead logging.
    ///
    /// # Errors
    /// Returns an error when the directory cannot be created or the database
    /// cannot be opened or migrated.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let connection = Connection::open(path).map_err(db)?;

        Self::from_connection(connection)
    }

    /// Opens an in-memory database with the full schema; used by tests and by
    /// short-lived CLI commands that do not need persistence.
    ///
    /// # Errors
    /// Returns an error when the schema cannot be created.
    pub fn open_in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory().map_err(db)?;

        Self::from_connection(connection)
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(db)?;
        // In-memory databases do not support WAL; the result is irrelevant there.
        let _ = connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get::<_, String>(0));

        let mut store = Self { connection };
        store.migrate()?;

        Ok(store)
    }

    /// Returns the underlying connection.
    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Current schema version stored in the database.
    ///
    /// # Errors
    /// Returns an error when SQLite cannot read `PRAGMA user_version`.
    pub fn schema_version(&self) -> Result<u32> {
        let version: i64 = self
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db)?;

        Ok(u32::try_from(version).unwrap_or(0))
    }

    /// Applies pending migrations.
    ///
    /// # Errors
    /// Returns an error when a migration fails; the transaction is rolled back
    /// and the previous version remains active.
    pub fn migrate(&mut self) -> Result<()> {
        let current = self.schema_version()?;

        for (index, migration) in MIGRATIONS.iter().enumerate() {
            let version = u32::try_from(index + 1).unwrap_or(u32::MAX);
            if version <= current {
                continue;
            }

            let transaction = self.connection.transaction().map_err(db)?;
            transaction.execute_batch(migration).map_err(db)?;
            transaction
                .pragma_update(None, "user_version", version)
                .map_err(db)?;
            transaction.commit().map_err(db)?;
        }

        Ok(())
    }

    /// Appends a journal entry and returns its assigned identifier.
    ///
    /// # Errors
    /// Returns an error when the record cannot be inserted.
    pub fn journal_append(&self, entry: &JournalEntry) -> Result<u64> {
        self.connection
            .execute(
                "INSERT INTO journal (
                    operation_id, timestamp, user, summary, affected_count,
                    affected_bytes, result, undo_available, details
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    entry.operation_id.to_string(),
                    entry.timestamp.as_millis(),
                    entry.user,
                    entry.summary,
                    i64::from(entry.affected_count),
                    to_i64(entry.affected_bytes),
                    entry.result.as_str(),
                    entry.undo_available,
                    entry.details.to_string(),
                ],
            )
            .map_err(db)?;

        Ok(u64::try_from(self.connection.last_insert_rowid()).unwrap_or(0))
    }

    /// Returns journal entries, newest first.
    ///
    /// # Errors
    /// Returns an error when the query fails or a row is malformed.
    pub fn journal_list(&self, limit: u32) -> Result<Vec<JournalEntry>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, operation_id, timestamp, user, summary, affected_count,
                        affected_bytes, result, undo_available, details
                 FROM journal
                 ORDER BY timestamp DESC, id DESC
                 LIMIT ?1",
            )
            .map_err(db)?;

        let rows = statement
            .query_map([i64::from(limit)], read_journal_entry)
            .map_err(db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db)
    }

    /// Looks up a journal entry by operation identifier.
    ///
    /// # Errors
    /// Returns an error when the query fails or the row is malformed.
    pub fn journal_by_operation(&self, operation_id: Uuid) -> Result<Option<JournalEntry>> {
        self.connection
            .query_row(
                "SELECT id, operation_id, timestamp, user, summary, affected_count,
                        affected_bytes, result, undo_available, details
                 FROM journal
                 WHERE operation_id = ?1
                 ORDER BY id DESC
                 LIMIT 1",
                [operation_id.to_string()],
                read_journal_entry,
            )
            .optional()
            .map_err(db)
    }

    /// Enables or disables undo for a journal entry.
    ///
    /// # Errors
    /// Returns [`Error::NotFound`] when the entry does not exist.
    pub fn journal_set_undo_available(&self, id: u64, available: bool) -> Result<()> {
        let updated = self
            .connection
            .execute(
                "UPDATE journal SET undo_available = ?2 WHERE id = ?1",
                params![to_i64(id), available],
            )
            .map_err(db)?;

        if updated == 0 {
            return Err(Error::NotFound(format!("journal entry `{id}`")));
        }

        Ok(())
    }

    /// Looks up one quarantine entry.
    ///
    /// # Errors
    /// Returns an error when the query fails or the row is malformed.
    pub fn quarantine_get(&self, id: Uuid) -> Result<Option<QuarantineItem>> {
        self.connection
            .query_row(
                "SELECT id, original_path, stored_path, volume_id, size,
                        deleted_at, expires_at, reason, status, checksum
                 FROM quarantine
                 WHERE id = ?1",
                [id.to_string()],
                read_quarantine_item,
            )
            .optional()
            .map_err(db)
    }

    /// Inserts a quarantine index entry.
    ///
    /// # Errors
    /// Returns an error when the record cannot be inserted.
    pub fn quarantine_insert(&self, item: &QuarantineItem) -> Result<()> {
        self.connection
            .execute(
                "INSERT INTO quarantine (
                    id, original_path, stored_path, volume_id, size,
                    deleted_at, expires_at, reason, status, checksum
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    item.id.to_string(),
                    item.original_path.to_string_lossy(),
                    item.stored_path.to_string_lossy(),
                    item.volume_id.to_string(),
                    to_i64(item.size),
                    item.deleted_at.as_millis(),
                    item.expires_at.as_millis(),
                    item.reason,
                    item.status.as_str(),
                    item.checksum,
                ],
            )
            .map_err(db)?;

        Ok(())
    }

    /// Returns quarantine entries, newest first.
    ///
    /// # Errors
    /// Returns an error when the query fails or a row is malformed.
    pub fn quarantine_list(&self) -> Result<Vec<QuarantineItem>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, original_path, stored_path, volume_id, size,
                        deleted_at, expires_at, reason, status, checksum
                 FROM quarantine
                 ORDER BY deleted_at DESC, id",
            )
            .map_err(db)?;

        let rows = statement.query_map([], read_quarantine_item).map_err(db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db)
    }

    /// Returns stored entries whose retention window has elapsed.
    ///
    /// # Errors
    /// Returns an error when the query fails or a row is malformed.
    pub fn quarantine_expired(&self, now: Timestamp) -> Result<Vec<QuarantineItem>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, original_path, stored_path, volume_id, size,
                        deleted_at, expires_at, reason, status, checksum
                 FROM quarantine
                 WHERE status = ?1 AND expires_at <= ?2
                 ORDER BY expires_at",
            )
            .map_err(db)?;

        let rows = statement
            .query_map(
                params![QuarantineStatus::Stored.as_str(), now.as_millis()],
                read_quarantine_item,
            )
            .map_err(db)?;

        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db)
    }

    /// Updates the status of a quarantine entry.
    ///
    /// # Errors
    /// Returns [`Error::NotFound`] when the entry does not exist.
    pub fn quarantine_update_status(&self, id: Uuid, status: QuarantineStatus) -> Result<()> {
        let updated = self
            .connection
            .execute(
                "UPDATE quarantine SET status = ?2 WHERE id = ?1",
                params![id.to_string(), status.as_str()],
            )
            .map_err(db)?;

        if updated == 0 {
            return Err(Error::NotFound(format!("quarantine item `{id}`")));
        }

        Ok(())
    }

    /// Looks up a cached hash.
    ///
    /// # Errors
    /// Returns an error when the query fails.
    pub fn hash_get(&self, key: &HashKey, algorithm: &str) -> Result<Option<Vec<u8>>> {
        self.connection
            .query_row(
                "SELECT hash FROM hash_cache
                 WHERE volume_serial = ?1 AND frn = ?2 AND size = ?3
                   AND mtime = ?4 AND algorithm = ?5",
                params![
                    i64::from(key.volume_serial),
                    to_i64(key.frn),
                    to_i64(key.size),
                    key.mtime.as_millis(),
                    algorithm,
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(db)
    }

    /// Stores a hash in the cache, replacing an older value for the same key.
    ///
    /// # Errors
    /// Returns an error when the record cannot be written.
    pub fn hash_put(&self, key: &HashKey, algorithm: &str, hash: &[u8]) -> Result<()> {
        self.connection
            .execute(
                "INSERT OR REPLACE INTO hash_cache
                    (volume_serial, frn, size, mtime, algorithm, hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    i64::from(key.volume_serial),
                    to_i64(key.frn),
                    to_i64(key.size),
                    key.mtime.as_millis(),
                    algorithm,
                    hash,
                ],
            )
            .map_err(db)?;

        Ok(())
    }

    /// Removes a cached hash.
    ///
    /// # Errors
    /// Returns an error when the query fails.
    pub fn hash_remove(&self, key: &HashKey, algorithm: &str) -> Result<()> {
        self.connection
            .execute(
                "DELETE FROM hash_cache
                 WHERE volume_serial = ?1 AND frn = ?2 AND size = ?3
                   AND mtime = ?4 AND algorithm = ?5",
                params![
                    i64::from(key.volume_serial),
                    to_i64(key.frn),
                    to_i64(key.size),
                    key.mtime.as_millis(),
                    algorithm,
                ],
            )
            .map_err(db)?;

        Ok(())
    }
}

fn read_journal_entry(row: &Row<'_>) -> rusqlite::Result<JournalEntry> {
    let operation_id: String = row.get(1)?;
    let result: String = row.get(7)?;
    let details: String = row.get(9)?;

    Ok(JournalEntry {
        id: u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
        operation_id: Uuid::parse_str(&operation_id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(1, Type::Text, error.into())
        })?,
        timestamp: Timestamp::from_millis(row.get(2)?),
        user: row.get(3)?,
        summary: row.get(4)?,
        affected_count: u32::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
        affected_bytes: row.get::<_, i64>(6)?.max(0) as u64,
        result: JournalResult::parse_value(&result).unwrap_or(JournalResult::Error),
        undo_available: row.get::<_, i64>(8)? != 0,
        details: serde_json::from_str(&details).unwrap_or(serde_json::Value::Null),
    })
}

fn read_quarantine_item(row: &Row<'_>) -> rusqlite::Result<QuarantineItem> {
    let id: String = row.get(0)?;
    let volume_id: String = row.get(3)?;
    let status: String = row.get(8)?;

    Ok(QuarantineItem {
        id: Uuid::parse_str(&id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(0, Type::Text, error.into())
        })?,
        original_path: row.get::<_, String>(1)?.into(),
        stored_path: row.get::<_, String>(2)?.into(),
        volume_id: Uuid::parse_str(&volume_id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(3, Type::Text, error.into())
        })?,
        size: row.get::<_, i64>(4)?.max(0) as u64,
        deleted_at: Timestamp::from_millis(row.get(5)?),
        expires_at: Timestamp::from_millis(row.get(6)?),
        reason: row.get(7)?,
        status: QuarantineStatus::parse_value(&status).unwrap_or(QuarantineStatus::Corrupted),
        checksum: row.get(9)?,
    })
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn db(error: impl Display) -> Error {
    Error::Other(format!("database error: {error}"))
}

#[cfg(test)]
mod tests {
    use freespace_core::{Timestamp, Uuid};

    use super::{HashKey, SCHEMA_VERSION, Store};
    use crate::{
        operation::{JournalEntry, JournalResult},
        quarantine::{QuarantineItem, QuarantineStatus},
    };

    fn journal_entry() -> JournalEntry {
        JournalEntry {
            id: 0,
            operation_id: Uuid::new_v4(),
            timestamp: Timestamp::from_millis(1_700_000_000_000),
            user: "tester".to_owned(),
            summary: "delete: 2 succeeded".to_owned(),
            affected_count: 2,
            affected_bytes: 30,
            result: JournalResult::Success,
            undo_available: true,
            details: serde_json::json!({"targets": ["a", "b"]}),
        }
    }

    fn quarantine_item() -> QuarantineItem {
        QuarantineItem {
            id: Uuid::new_v4(),
            original_path: r"C:\temp\old.txt".into(),
            stored_path: r"C:\quarantine\old.txt".into(),
            volume_id: Uuid::new_v4(),
            size: 42,
            deleted_at: Timestamp::from_millis(1_700_000_000_000),
            expires_at: Timestamp::from_millis(1_700_000_000_000 + 86_400_000),
            reason: "test".to_owned(),
            status: QuarantineStatus::Stored,
            checksum: "abc".to_owned(),
        }
    }

    #[test]
    fn migrates_once_and_is_idempotent() {
        let mut store = Store::open_in_memory().expect("open store");

        assert_eq!(store.schema_version().expect("version"), SCHEMA_VERSION);
        store.migrate().expect("second migration");
        assert_eq!(store.schema_version().expect("version"), SCHEMA_VERSION);
    }

    #[test]
    fn stores_and_lists_journal_entries() {
        let store = Store::open_in_memory().expect("open store");
        let entry = journal_entry();

        let id = store.journal_append(&entry).expect("append");
        assert!(id > 0);

        let entries = store.journal_list(10).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].operation_id, entry.operation_id);
        assert_eq!(entries[0].result, JournalResult::Success);
        assert_eq!(entries[0].affected_bytes, 30);
        assert_eq!(entries[0].details, entry.details);
    }

    #[test]
    fn orders_journal_by_timestamp() {
        let store = Store::open_in_memory().expect("open store");
        let mut older = journal_entry();
        older.timestamp = Timestamp::from_millis(1_000);
        let mut newer = journal_entry();
        newer.timestamp = Timestamp::from_millis(2_000);

        store.journal_append(&older).expect("append older");
        store.journal_append(&newer).expect("append newer");

        let entries = store.journal_list(10).expect("list");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].timestamp.as_millis(), 2_000);
        assert_eq!(entries[1].timestamp.as_millis(), 1_000);
    }

    #[test]
    fn stores_quarantine_index_and_finds_expired_items() {
        let store = Store::open_in_memory().expect("open store");
        let item = quarantine_item();

        store.quarantine_insert(&item).expect("insert");
        let items = store.quarantine_list().expect("list");
        assert_eq!(items, vec![item.clone()]);

        let expired = store
            .quarantine_expired(Timestamp::from_millis(item.expires_at.as_millis() + 1))
            .expect("expired");
        assert_eq!(expired.len(), 1);
        assert!(
            store
                .quarantine_expired(Timestamp::from_millis(0))
                .expect("not expired")
                .is_empty()
        );

        store
            .quarantine_update_status(item.id, QuarantineStatus::Restored)
            .expect("update");
        assert!(
            store
                .quarantine_expired(Timestamp::from_millis(i64::MAX))
                .expect("expired")
                .is_empty(),
            "restored items are not purged"
        );

        assert!(
            store
                .quarantine_update_status(Uuid::new_v4(), QuarantineStatus::Purged)
                .is_err()
        );
    }

    #[test]
    fn caches_hashes_by_file_identity() {
        let store = Store::open_in_memory().expect("open store");
        let key = HashKey {
            volume_serial: 7,
            frn: 42,
            size: 1_024,
            mtime: Timestamp::from_millis(1_700_000_000_000),
        };

        assert!(store.hash_get(&key, "sha256").expect("get").is_none());
        store.hash_put(&key, "sha256", b"hash").expect("put");
        assert_eq!(
            store.hash_get(&key, "sha256").expect("get"),
            Some(b"hash".to_vec())
        );
        assert!(store.hash_get(&key, "md5").expect("get").is_none());

        store.hash_put(&key, "sha256", b"updated").expect("replace");
        assert_eq!(
            store.hash_get(&key, "sha256").expect("get"),
            Some(b"updated".to_vec())
        );

        store.hash_remove(&key, "sha256").expect("remove");
        assert!(store.hash_get(&key, "sha256").expect("get").is_none());
    }

    #[test]
    fn persists_data_in_a_file_database() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("nested").join("freespace.db");

        {
            let store = Store::open(&path).expect("open file store");
            store.journal_append(&journal_entry()).expect("append");
        }

        let store = Store::open(&path).expect("reopen store");
        assert_eq!(store.journal_list(10).expect("list").len(), 1);
    }

    #[test]
    fn storage_lives_in_the_application_data_directory() {
        let path = super::default_path().expect("default path");
        let executable = std::env::current_exe().expect("current exe");

        assert_eq!(
            path,
            executable
                .parent()
                .expect("parent")
                .join(freespace_core::settings::DATA_DIR_NAME)
                .join("freespace.db")
        );
    }
}
