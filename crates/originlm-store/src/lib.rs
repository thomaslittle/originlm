//! Broker-owned durable state. There is no separate database server and no
//! client SQL access. The store commits normalized, sanitized observations,
//! the last ready-to-serve snapshot, a bounded change journal, issue state,
//! and notification delivery state in one SQLite file.
use originlm_contracts::{
    ConnectionMetadata, DomainError, ErrorCode, InventoryChange, InventoryEventPage, Issue,
    IssueState, NotificationPreferences, ResourceChange, Snapshot, Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Bump only with a tested migration. A store from the future fails closed.
pub const SCHEMA_VERSION: u32 = 1;
const MAX_JOURNAL_ROWS: i64 = 2048;
const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;
const MAX_JOURNAL_PAGE: usize = 256;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage is read-only")]
    ReadOnly,
    #[error("storage is full")]
    DiskFull,
    #[error("storage is busy")]
    Busy,
    #[error("storage is corrupt")]
    Corrupt,
    #[error("storage schema {0} is newer than this runtime")]
    UnsupportedSchema(u32),
    #[error("storage {0}")]
    Io(String),
    #[error("storage query failed: {0}")]
    Database(String),
    #[error("journal cursor is no longer replayable")]
    EventGap,
    #[error("stored state exceeds its bound")]
    TooLarge,
}
impl StoreError {
    /// Stable public classification. Messages never echo a SQLite statement.
    pub fn domain(&self) -> DomainError {
        let (code, message) = match self {
            Self::ReadOnly => (
                ErrorCode::StorageUnavailable,
                "Local state is read-only; observations were not persisted",
            ),
            Self::DiskFull => (
                ErrorCode::StorageUnavailable,
                "Local state storage is full; free space and retry",
            ),
            Self::Busy => (
                ErrorCode::StorageUnavailable,
                "Local state is busy; retry shortly",
            ),
            Self::Corrupt => (
                ErrorCode::StorageUnavailable,
                "Local state was preserved, but it could not be read",
            ),
            Self::UnsupportedSchema(_) => (
                ErrorCode::ProtocolIncompatible,
                "Local state was written by a newer runtime",
            ),
            Self::Io(_) => (
                ErrorCode::StorageUnavailable,
                "Local state location is unavailable",
            ),
            Self::Database(_) => (
                ErrorCode::StorageUnavailable,
                "Local state operation failed",
            ),
            Self::EventGap => (
                ErrorCode::EventGap,
                "Inventory journal cursor is no longer replayable; resynchronize",
            ),
            Self::TooLarge => (
                ErrorCode::FrameTooLarge,
                "A durable state record exceeds its bound",
            ),
        };
        let mut error = DomainError::new(code, message);
        error.retryable = matches!(self, Self::Busy | Self::DiskFull);
        error
    }
}

fn classify(error: rusqlite::Error) -> StoreError {
    use rusqlite::ErrorCode as Sqlite;
    match &error {
        rusqlite::Error::SqliteFailure(code, _) => match code.code {
            Sqlite::ReadOnly => StoreError::ReadOnly,
            Sqlite::DiskFull => StoreError::DiskFull,
            Sqlite::DatabaseBusy | Sqlite::DatabaseLocked => StoreError::Busy,
            Sqlite::DatabaseCorrupt | Sqlite::NotADatabase => StoreError::Corrupt,
            _ => StoreError::Database(code.to_string()),
        },
        _ => StoreError::Database(error.to_string()),
    }
}

#[derive(Debug, Clone)]
pub struct RecoveryNotice {
    /// A corrupt file is moved here, never silently deleted.
    pub backup_path: Option<PathBuf>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct OpenedStore {
    pub notice: Option<RecoveryNotice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableMetadata {
    pub observed_at_ms: u64,
    pub report: ConnectionMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct DurableState {
    pub snapshot: Option<Snapshot>,
    pub revision: u64,
    pub cursor: String,
    pub metadata: Vec<DurableMetadata>,
    pub issues: Vec<Issue>,
    pub notifications: NotificationPreferences,
    pub snapshot_error: Option<String>,
    pub tail_sequence: u64,
}

#[derive(Debug, Clone)]
pub struct Committed {
    pub revision: String,
    pub previous_revision: String,
    pub cursor: String,
    pub change: InventoryChange,
}

pub struct StateStore {
    path: PathBuf,
    epoch: String,
    connection: std::sync::Mutex<Connection>,
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Public snapshots already mask account identity. Durable metadata follows the
/// same rule so a local database, backup, or diagnostic export never becomes an
/// identity or credential export.
pub fn sanitized_metadata(mut report: ConnectionMetadata) -> ConnectionMetadata {
    if let Some(account) = report.account.as_mut() {
        account.display_label = originlm_contracts::Observation::unknown("NOT_PERSISTED");
        account.identity_source = originlm_contracts::Observation::unknown("NOT_PERSISTED");
    }
    report
}

fn private_options() -> std::fs::OpenOptions {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

fn ensure_private(path: &Path) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            if error.kind() == std::io::ErrorKind::StorageFull {
                StoreError::DiskFull
            } else {
                StoreError::Io(error.to_string())
            }
        })?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn integrity(connection: &Connection) -> Result<(), StoreError> {
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(classify)?;
    if result.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

impl StateStore {
    pub fn open(path: impl AsRef<Path>) -> Result<(Self, Option<RecoveryNotice>), StoreError> {
        let path = path.as_ref().to_path_buf();
        ensure_private(&path)?;
        // Create the file with owner-only permissions before SQLite opens it.
        private_options()
            .open(&path)
            .map_err(|error| StoreError::Io(error.to_string()))?;
        match Self::open_existing(&path) {
            Ok(store) => Ok((store, None)),
            Err(StoreError::Corrupt) | Err(StoreError::Database(_)) => {
                let backup = path.with_extension(format!("corrupt-{}", unix_ms()));
                std::fs::rename(&path, &backup)
                    .map_err(|error| StoreError::Io(error.to_string()))?;
                // A WAL sibling can not be merged into the preserved file once
                // the main database is damaged; keep it beside the backup.
                let wal = PathBuf::from(format!("{}-wal", path.display()));
                let shm = PathBuf::from(format!("{}-shm", path.display()));
                if wal.exists() {
                    let _ =
                        std::fs::rename(&wal, PathBuf::from(format!("{}-wal", backup.display())));
                }
                if shm.exists() {
                    let _ =
                        std::fs::rename(&shm, PathBuf::from(format!("{}-shm", backup.display())));
                }
                private_options()
                    .open(&path)
                    .map_err(|error| StoreError::Io(error.to_string()))?;
                let store = Self::open_existing(&path)?;
                Ok((
                    store,
                    Some(RecoveryNotice {
                        backup_path: Some(backup),
                        reason: "stored state failed an integrity check and was preserved".into(),
                    }),
                ))
            }
            Err(error) => Err(error),
        }
    }

    /// Read-only access for diagnostics and recovery. It never creates a file.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        let connection = Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(classify)?;
        integrity(&connection)?;
        let epoch = meta(&connection, "epoch")?.ok_or(StoreError::Corrupt)?;
        let version = meta(&connection, "schema_version")?
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        if version != SCHEMA_VERSION {
            return Err(StoreError::UnsupportedSchema(version));
        }
        Ok(Self {
            path,
            epoch,
            connection: std::sync::Mutex::new(connection),
        })
    }

    fn open_existing(path: &Path) -> Result<Self, StoreError> {
        let connection = Connection::open(path).map_err(classify)?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(classify)?;
        integrity(&connection)?;
        let has_meta: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta')",
                [],
                |row| row.get(0),
            )
            .map_err(classify)?;
        let version: Option<String> = if has_meta {
            meta(&connection, "schema_version")?
        } else {
            None
        };
        let version = version.and_then(|value| value.parse::<u32>().ok());
        let epoch = match version {
            Some(SCHEMA_VERSION) => meta(&connection, "epoch")?.ok_or(StoreError::Corrupt)?,
            Some(0) | None => create_schema(&connection)?,
            Some(other) => return Err(StoreError::UnsupportedSchema(other)),
        };
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(classify)?;
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(classify)?;
        connection
            .pragma_update(None, "wal_autocheckpoint", 512)
            .map_err(classify)?;
        connection
            .pragma_update(None, "temp_store", "MEMORY")
            .map_err(classify)?;
        Ok(Self {
            path: path.to_path_buf(),
            epoch,
            connection: std::sync::Mutex::new(connection),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, StoreError> {
        self.connection
            .lock()
            .map_err(|_| StoreError::Database("state lock poisoned".into()))
    }

    pub fn load(&self) -> Result<DurableState, StoreError> {
        let connection = self.lock()?;
        let revision = meta(&connection, "revision")?
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let tail = meta(&connection, "tail_sequence")?
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let row = connection
            .query_row(
                "SELECT observed_at, payload FROM snapshot WHERE singleton = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(classify)?;
        let mut snapshot_error = None;
        let snapshot = match row {
            Some((_observed_at, payload)) if payload.len() <= MAX_SNAPSHOT_BYTES => {
                match serde_json::from_str::<Snapshot>(&payload) {
                    Ok(mut snapshot) => {
                        snapshot.revision = revision.to_string();
                        snapshot.cursor = format!("{}:{tail}", self.epoch);
                        Some(snapshot)
                    }
                    Err(_) => {
                        snapshot_error = Some(
                            "persisted snapshot is unreadable; source data was preserved".into(),
                        );
                        None
                    }
                }
            }
            Some(_) => {
                snapshot_error =
                    Some("persisted snapshot exceeds the bound; source data was preserved".into());
                None
            }
            None => None,
        };
        let mut statement = connection
            .prepare("SELECT observed_at_ms, payload FROM metadata ORDER BY connection_id")
            .map_err(classify)?;
        let metadata = statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(classify)?
            .filter_map(|row| row.ok())
            .filter_map(|(observed_at_ms, payload)| {
                if payload.len() > MAX_METADATA_BYTES {
                    return None;
                }
                serde_json::from_str::<ConnectionMetadata>(&payload)
                    .ok()
                    .map(|report| DurableMetadata {
                        observed_at_ms: u64::try_from(observed_at_ms).unwrap_or(0),
                        report: sanitized_metadata(report),
                    })
            })
            .collect();
        let mut statement = connection
            .prepare("SELECT payload FROM issues ORDER BY id")
            .map_err(classify)?;
        let issues = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(classify)?
            .filter_map(|row| row.ok())
            .filter_map(|payload| serde_json::from_str::<Issue>(&payload).ok())
            .collect();
        let notifications = connection
            .query_row(
                "SELECT payload FROM preferences WHERE singleton = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(classify)?
            .and_then(|payload| serde_json::from_str(&payload).ok())
            .unwrap_or_default();
        Ok(DurableState {
            snapshot,
            revision,
            cursor: format!("{}:{tail}", self.epoch),
            metadata,
            issues,
            notifications,
            snapshot_error,
            tail_sequence: tail,
        })
    }

    /// Commit the ready-to-serve snapshot and, optionally, a metadata
    /// observation in the same transaction. The returned cursor identifies the
    /// exact committed revision and can be replayed from the journal.
    pub fn commit_snapshot(
        &self,
        snapshot: &mut Snapshot,
        metadata: Option<DurableMetadata>,
        reason: &str,
        changes: Vec<ResourceChange>,
        issues_changed: bool,
    ) -> Result<Committed, StoreError> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction().map_err(classify)?;
        let previous = meta(&transaction, "revision")?
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let revision = previous.saturating_add(1);
        let observed_at = snapshot.observed_at.as_str().to_owned();
        snapshot.revision = revision.to_string();
        let mut durable_snapshot = snapshot.clone();
        for account in &mut durable_snapshot.accounts {
            account.display_label = originlm_contracts::Observation::unknown("NOT_PERSISTED");
            account.identity_source = originlm_contracts::Observation::unknown("NOT_PERSISTED");
            for scope in &mut account.scopes {
                scope.label = originlm_contracts::Observation::unknown("NOT_PERSISTED");
            }
        }
        durable_snapshot.cursor = format!("{}:{revision}", self.epoch);
        let payload = serde_json::to_string(&durable_snapshot)
            .map_err(|_| StoreError::Database("snapshot serialization failed".into()))?;
        if payload.len() > MAX_SNAPSHOT_BYTES {
            return Err(StoreError::TooLarge);
        }
        transaction
            .execute(
                "INSERT INTO snapshot(singleton, observed_at, payload) VALUES(1, ?1, ?2)
                 ON CONFLICT(singleton) DO UPDATE SET observed_at=excluded.observed_at, payload=excluded.payload",
                params![observed_at, payload],
            )
            .map_err(classify)?;
        if let Some(metadata) = metadata {
            let sanitized = sanitized_metadata(metadata.report);
            let payload = serde_json::to_string(&sanitized)
                .map_err(|_| StoreError::Database("metadata serialization failed".into()))?;
            if payload.len() > MAX_METADATA_BYTES {
                return Err(StoreError::TooLarge);
            }
            transaction
                .execute(
                    "INSERT INTO metadata(connection_id, observed_at_ms, payload) VALUES(?1, ?2, ?3)
                     ON CONFLICT(connection_id) DO UPDATE SET observed_at_ms=excluded.observed_at_ms, payload=excluded.payload",
                    params![sanitized.connection_id, i64::try_from(metadata.observed_at_ms).unwrap_or(i64::MAX), payload],
                )
                .map_err(classify)?;
        }
        let changes_json = serde_json::to_string(&changes)
            .map_err(|_| StoreError::Database("change serialization failed".into()))?;
        transaction
            .execute(
                "INSERT INTO journal(revision, previous_revision, observed_at, reason, changes_json, issues_changed)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    i64::try_from(revision).unwrap_or(i64::MAX),
                    i64::try_from(previous).unwrap_or(i64::MAX),
                    observed_at,
                    reason,
                    changes_json,
                    i64::from(issues_changed),
                ],
            )
            .map_err(classify)?;
        let sequence = transaction.last_insert_rowid();
        transaction
            .execute(
                "DELETE FROM journal WHERE sequence <= ?1",
                params![sequence.saturating_sub(MAX_JOURNAL_ROWS)],
            )
            .map_err(classify)?;
        set_meta(&transaction, "revision", &revision.to_string())?;
        set_meta(&transaction, "tail_sequence", &sequence.to_string())?;
        transaction.commit().map_err(classify)?;
        let cursor = format!("{}:{sequence}", self.epoch);
        snapshot.cursor = cursor.clone();
        Ok(Committed {
            revision: revision.to_string(),
            previous_revision: previous.to_string(),
            cursor: cursor.clone(),
            change: InventoryChange {
                sequence: sequence.to_string(),
                cursor,
                revision: revision.to_string(),
                previous_revision: previous.to_string(),
                observed_at: snapshot.observed_at.clone(),
                reason: reason.to_owned(),
                changes,
                issues_changed,
            },
        })
    }

    /// Persist the last metadata observation without inventing a new inventory
    /// revision. Used by failure paths and by hydration maintenance.
    pub fn put_metadata(
        &self,
        observed_at_ms: u64,
        report: &ConnectionMetadata,
    ) -> Result<(), StoreError> {
        let report = sanitized_metadata(report.clone());
        let payload = serde_json::to_string(&report)
            .map_err(|_| StoreError::Database("metadata serialization failed".into()))?;
        if payload.len() > MAX_METADATA_BYTES {
            return Err(StoreError::TooLarge);
        }
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO metadata(connection_id, observed_at_ms, payload) VALUES(?1, ?2, ?3)
                 ON CONFLICT(connection_id) DO UPDATE SET observed_at_ms=excluded.observed_at_ms, payload=excluded.payload",
                params![report.connection_id, i64::try_from(observed_at_ms).unwrap_or(i64::MAX), payload],
            )
            .map_err(classify)?;
        Ok(())
    }

    pub fn remove_metadata(&self, connection_id: &str) -> Result<(), StoreError> {
        let connection = self.lock()?;
        connection
            .execute(
                "DELETE FROM metadata WHERE connection_id = ?1",
                params![connection_id],
            )
            .map_err(classify)?;
        Ok(())
    }

    pub fn replace_issues(&self, issues: &[Issue]) -> Result<(), StoreError> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction().map_err(classify)?;
        let mut retained = std::collections::BTreeSet::new();
        for issue in issues {
            retained.insert(issue.id.clone());
            let payload = serde_json::to_string(issue)
                .map_err(|_| StoreError::Database("issue serialization failed".into()))?;
            transaction
                .execute(
                    "INSERT INTO issues(id, payload) VALUES(?1, ?2)
                     ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
                    params![issue.id, payload],
                )
                .map_err(classify)?;
        }
        // Keep resolved issues for a bounded history instead of erasing them.
        let mut statement = transaction
            .prepare("SELECT id FROM issues")
            .map_err(classify)?;
        let existing: Vec<String> = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(classify)?
            .filter_map(|row| row.ok())
            .collect();
        drop(statement);
        for id in existing {
            if !retained.contains(&id) {
                let mut issue = transaction
                    .query_row(
                        "SELECT payload FROM issues WHERE id = ?1",
                        params![id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(classify)?
                    .and_then(|payload| serde_json::from_str::<Issue>(&payload).ok());
                if let Some(issue) = issue.as_mut()
                    && issue.state != IssueState::Resolved
                {
                    issue.state = IssueState::Resolved;
                    issue.resolved_at = Some(Timestamp::now());
                    if let Ok(payload) = serde_json::to_string(issue) {
                        let _ = transaction.execute(
                            "UPDATE issues SET payload = ?1 WHERE id = ?2",
                            params![payload, id],
                        );
                    }
                }
            }
        }
        transaction.commit().map_err(classify)?;
        Ok(())
    }

    pub fn update_issue(&self, issue: &Issue) -> Result<(), StoreError> {
        let payload = serde_json::to_string(issue)
            .map_err(|_| StoreError::Database("issue serialization failed".into()))?;
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO issues(id, payload) VALUES(?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
                params![issue.id, payload],
            )
            .map_err(classify)?;
        Ok(())
    }

    pub fn set_preferences(&self, preferences: &NotificationPreferences) -> Result<(), StoreError> {
        let payload = serde_json::to_string(preferences)
            .map_err(|_| StoreError::Database("preference serialization failed".into()))?;
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO preferences(singleton, payload) VALUES(1, ?1)
                 ON CONFLICT(singleton) DO UPDATE SET payload=excluded.payload",
                params![payload],
            )
            .map_err(classify)?;
        Ok(())
    }

    pub fn notification_seen(&self, issue_id: &str, dedupe_key: &str) -> Result<bool, StoreError> {
        let connection = self.lock()?;
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM notifications WHERE issue_id = ?1 AND dedupe_key = ?2",
                params![issue_id, dedupe_key],
                |row| row.get(0),
            )
            .map_err(classify)?;
        Ok(count > 0)
    }

    pub fn record_notification(
        &self,
        issue_id: &str,
        dedupe_key: &str,
        severity: &str,
        delivered_at_ms: u64,
    ) -> Result<(), StoreError> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT OR REPLACE INTO notifications(issue_id, dedupe_key, severity, delivered_at_ms)
                 VALUES(?1, ?2, ?3, ?4)",
                params![
                    issue_id,
                    dedupe_key,
                    severity,
                    i64::try_from(delivered_at_ms).unwrap_or(i64::MAX)
                ],
            )
            .map_err(classify)?;
        // Bounded delivery history.
        connection
            .execute(
                "DELETE FROM notifications WHERE rowid NOT IN
                 (SELECT rowid FROM notifications ORDER BY delivered_at_ms DESC LIMIT 1024)",
                [],
            )
            .map_err(classify)?;
        Ok(())
    }

    pub fn record_notification_time(
        &self,
        issue_id: &str,
        dedupe_key: &str,
    ) -> Result<Option<u64>, StoreError> {
        let connection = self.lock()?;
        let value: Option<i64> = connection
            .query_row(
                "SELECT delivered_at_ms FROM notifications WHERE issue_id = ?1 AND dedupe_key = ?2",
                params![issue_id, dedupe_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(classify)?;
        Ok(value.and_then(|value| u64::try_from(value).ok()))
    }

    pub fn read_journal(&self, after: &str) -> Result<InventoryEventPage, StoreError> {
        let (page_epoch, after_sequence) = parse_cursor(after)?;
        if page_epoch != self.epoch {
            return Err(StoreError::EventGap);
        }
        let connection = self.lock()?;
        let tail = meta(&connection, "tail_sequence")?
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let revision = meta(&connection, "revision")?
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0)
            .to_string();
        if after_sequence > tail {
            return Err(StoreError::EventGap);
        }
        let oldest: Option<i64> = connection
            .query_row("SELECT MIN(sequence) FROM journal", [], |row| row.get(0))
            .map_err(classify)?;
        if let Some(oldest) = oldest {
            let oldest = u64::try_from(oldest).unwrap_or(u64::MAX);
            if after_sequence.saturating_add(1) < oldest {
                return Err(StoreError::EventGap);
            }
        }
        let mut statement = connection
            .prepare(
                "SELECT sequence, revision, previous_revision, observed_at, reason, changes_json, issues_changed
                 FROM journal WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2",
            )
            .map_err(classify)?;
        let mut events = Vec::new();
        let rows = statement
            .query_map(
                params![
                    i64::try_from(after_sequence).unwrap_or(i64::MAX),
                    i64::try_from(MAX_JOURNAL_PAGE).unwrap_or(i64::MAX)
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .map_err(classify)?;
        for row in rows {
            let (
                sequence,
                revision,
                previous_revision,
                observed_at,
                reason,
                changes_json,
                issues_changed,
            ) = row.map_err(classify)?;
            let changes: Vec<ResourceChange> =
                serde_json::from_str(&changes_json).unwrap_or_default();
            events.push(InventoryChange {
                sequence: sequence.to_string(),
                cursor: format!("{}:{sequence}", self.epoch),
                revision: revision.to_string(),
                previous_revision: previous_revision.to_string(),
                observed_at: observed_at.try_into().map_err(|_| StoreError::Corrupt)?,
                reason,
                changes,
                issues_changed: issues_changed != 0,
            });
        }
        let cursor = events
            .last()
            .map(|event| event.sequence.clone())
            .unwrap_or_else(|| after_sequence.to_string());
        Ok(InventoryEventPage {
            events,
            cursor: format!("{}:{cursor}", self.epoch),
            tail_cursor: format!("{}:{tail}", self.epoch),
            has_more: cursor.parse::<u64>().unwrap_or(0) < tail,
            revision,
        })
    }

    pub fn checkpoint(&self) -> Result<(), StoreError> {
        let connection = self.lock()?;
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(classify)
    }
}

impl Drop for StateStore {
    fn drop(&mut self) {
        let _ = self.checkpoint();
    }
}

fn parse_cursor(cursor: &str) -> Result<(&str, u64), StoreError> {
    let (epoch, sequence) = cursor.split_once(':').ok_or(StoreError::EventGap)?;
    let sequence = sequence.parse::<u64>().map_err(|_| StoreError::EventGap)?;
    if epoch.is_empty() || epoch.len() > 64 {
        return Err(StoreError::EventGap);
    }
    Ok((epoch, sequence))
}

fn meta(connection: &Connection, key: &str) -> Result<Option<String>, StoreError> {
    connection
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(classify)
}

fn set_meta(transaction: &Transaction<'_>, key: &str, value: &str) -> Result<(), StoreError> {
    transaction
        .execute(
            "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
        .map_err(classify)?;
    Ok(())
}

fn create_schema(connection: &Connection) -> Result<String, StoreError> {
    let epoch = random_epoch()?;
    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS snapshot(
               singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
               observed_at TEXT NOT NULL,
               payload TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS metadata(
               connection_id TEXT PRIMARY KEY,
               observed_at_ms INTEGER NOT NULL,
               payload TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS journal(
               sequence INTEGER PRIMARY KEY AUTOINCREMENT,
               revision INTEGER NOT NULL,
               previous_revision INTEGER NOT NULL,
               observed_at TEXT NOT NULL,
               reason TEXT NOT NULL,
               changes_json TEXT NOT NULL,
               issues_changed INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS issues(id TEXT PRIMARY KEY, payload TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS preferences(singleton INTEGER PRIMARY KEY CHECK(singleton = 1), payload TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS notifications(
               issue_id TEXT NOT NULL,
               dedupe_key TEXT NOT NULL,
               severity TEXT NOT NULL,
               delivered_at_ms INTEGER NOT NULL,
               PRIMARY KEY(issue_id, dedupe_key)
             );
             INSERT INTO meta(key, value) VALUES('schema_version', '1');
             INSERT INTO meta(key, value) VALUES('revision', '0');
             INSERT INTO meta(key, value) VALUES('tail_sequence', '0');
             COMMIT;",
        )
        .map_err(classify)?;
    set_meta_connection(connection, "epoch", &epoch)?;
    Ok(epoch)
}

fn set_meta_connection(connection: &Connection, key: &str, value: &str) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
        .map_err(classify)?;
    Ok(())
}

fn random_epoch() -> Result<String, StoreError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| StoreError::Database("entropy unavailable".into()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
