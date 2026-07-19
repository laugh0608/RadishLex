use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, ErrorCode, Transaction, TransactionBehavior};

use crate::error::{UserDbError, UserDbResult};

use super::identity::legacy_stable_hash_hex;
use super::UserDb;

pub(super) const SCHEMA_VERSION: i64 = 8;
pub(super) const BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);
pub(super) const MAX_LEARNING_COUNT: i64 = 1_000_000;

impl UserDb {
    pub fn open(path: impl AsRef<Path>) -> UserDbResult<Self> {
        let path = path.as_ref().to_path_buf();
        let connection = Connection::open(&path)
            .map_err(|error| preserved_database_error(&path, "open", error))?;
        configure_common_connection(&connection)
            .map_err(|error| preserved_database_error(&path, "configure", error))?;
        let version = read_schema_version(&connection)
            .map_err(|error| preserved_database_error(&path, "read schema version", error))?;
        reject_future_schema(version)?;
        verify_integrity(&connection)
            .map_err(|error| preserved_database_error(&path, "integrity check", error))?;
        restrict_database_permissions(&path)?;
        configure_file_connection(&connection)
            .map_err(|error| preserved_database_error(&path, "configure", error))?;

        let mut db = Self { connection };
        db.migrate_from(version)
            .map_err(|error| preserved_userdb_error(&path, "migrate", error))?;
        db.validate_current_schema()
            .map_err(|error| preserved_userdb_error(&path, "validate schema", error))?;
        restrict_database_permissions(&path)?;
        restrict_sqlite_sidecar_permissions(&path)?;
        Ok(db)
    }

    pub fn open_in_memory() -> UserDbResult<Self> {
        let connection = Connection::open_in_memory()?;
        configure_common_connection(&connection)?;
        let mut db = Self { connection };
        db.migrate_from(0)?;
        db.validate_current_schema()?;
        Ok(db)
    }

    pub fn schema_version(&self) -> UserDbResult<i64> {
        read_schema_version(&self.connection).map_err(Into::into)
    }

    fn migrate_from(&mut self, version: i64) -> UserDbResult<()> {
        reject_future_schema(version)?;
        if version == SCHEMA_VERSION {
            return Ok(());
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version = read_schema_version(&transaction)?;
        reject_future_schema(version)?;
        if version == SCHEMA_VERSION {
            transaction.commit()?;
            return Ok(());
        }
        if version == 0 && !has_any_user_table(&transaction)? {
            create_schema_v5(&transaction)?;
            ensure_trusted_lifecycle_tables(&transaction)?;
        } else {
            if version < 3 {
                create_legacy_schema_if_missing(&transaction)?;
                ensure_import_batch_v2_columns(&transaction)?;
                ensure_user_term_restore_version(&transaction)?;
                migrate_deleted_term_identity(&transaction)?;
                migrate_ranker_last_used_time(&transaction)?;
            }
            ensure_user_term_import_batch(&transaction)?;
            ensure_sync_orchestration_tables(&transaction)?;
            ensure_trusted_lifecycle_tables(&transaction)?;
        }
        ensure_trusted_device_key_agreement_columns(&transaction)?;
        ensure_recovery_lifecycle_event_types(&transaction)?;
        ensure_wrapped_epoch_material_table(&transaction)?;
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        validate_current_schema_on(&transaction)?;
        transaction.commit()?;

        let actual = self.schema_version()?;
        if actual != SCHEMA_VERSION {
            return Err(UserDbError::invalid_input(
                "schema_version",
                format!("expected {SCHEMA_VERSION}, got {actual}"),
            ));
        }
        Ok(())
    }

    fn validate_current_schema(&self) -> UserDbResult<()> {
        validate_current_schema_on(&self.connection)
    }
}

fn validate_current_schema_on(connection: &Connection) -> UserDbResult<()> {
    let schema_version = read_schema_version(connection)?;
    if schema_version != SCHEMA_VERSION {
        return Err(UserDbError::invalid_input(
            "schema_version",
            format!("expected {SCHEMA_VERSION}"),
        ));
    }

    require_columns(
        connection,
        "user_terms",
        &[
            "id",
            "text",
            "reading",
            "input_code",
            "source",
            "weight",
            "status",
            "created_at_ms",
            "updated_at_ms",
            "last_used_at_ms",
            "restored_at_ms",
            "import_batch_id",
        ],
    )?;
    require_columns(
        connection,
        "deleted_terms",
        &[
            "id",
            "term_id",
            "input_code",
            "text",
            "reading",
            "deleted_at_ms",
            "reason",
        ],
    )?;
    require_columns(
        connection,
        "ranker_weights",
        &[
            "id",
            "input_code",
            "text",
            "reading",
            "frequency",
            "last_used_at_ms",
            "negative_score",
            "context_kind",
            "updated_at_ms",
        ],
    )?;
    require_columns(
        connection,
        "sync_domain_state",
        &["domain_id", "state_version", "cursor", "last_success_at_ms"],
    )?;
    require_columns(
        connection,
        "sync_remote_objects",
        &[
            "domain_id",
            "object_id",
            "object_type",
            "latest_version",
            "ciphertext_hash",
            "owner_device_id",
            "key_epoch",
            "change_sequence",
        ],
    )?;
    require_columns(
        connection,
        "sync_local_objects",
        &[
            "domain_id",
            "object_id",
            "object_type",
            "payload_hash",
            "local_revision",
            "acknowledged_revision",
            "dirty",
        ],
    )?;
    require_columns(
        connection,
        "sync_prepared_outbox",
        &[
            "domain_id",
            "object_id",
            "object_type",
            "local_revision",
            "version",
            "base_version",
            "owner_device_id",
            "key_id",
            "key_epoch",
            "algorithm",
            "nonce",
            "encrypted_payload",
            "ciphertext_hash",
            "record_count",
            "signature_schema_version",
            "signature_algorithm",
            "signature_key_id",
            "signer_device_id",
            "signature",
            "created_at_ms",
            "updated_at_ms",
            "attempt_count",
            "last_error_code",
        ],
    )?;
    require_columns(
        connection,
        "sync_cycle_journal",
        &[
            "domain_id",
            "phase",
            "started_at_ms",
            "lease_expires_at_ms",
            "cancel_requested",
        ],
    )?;
    require_columns(
        connection,
        "sync_trusted_domains",
        &[
            "domain_id",
            "current_key_epoch",
            "active_key_id",
            "created_at_ms",
            "updated_at_ms",
            "lifecycle_cursor",
            "lifecycle_sequence",
        ],
    )?;
    require_columns(
        connection,
        "sync_trusted_devices",
        &[
            "domain_id",
            "device_id",
            "signing_algorithm",
            "signing_public_key_id",
            "signing_public_key",
            "signing_key_created_at_ms",
            "key_agreement_public_key_id",
            "key_agreement_public_key",
            "status",
            "authorized_at_ms",
            "revoked_at_ms",
            "last_seen_at_ms",
            "reject_from_change_sequence",
        ],
    )?;
    require_columns(
        connection,
        "sync_trusted_lifecycle_events",
        &[
            "domain_id",
            "lifecycle_sequence",
            "event_type",
            "record_id",
            "key_epoch",
            "reject_from_object_change_sequence",
            "created_at_ms",
            "record_json",
        ],
    )?;
    require_columns(
        connection,
        "sync_wrapped_epoch_materials",
        &[
            "domain_id",
            "recipient_device_id",
            "recipient_key_agreement_key_id",
            "wrapping_key_id",
            "key_epoch",
            "schema_version",
            "algorithm",
            "nonce",
            "wrapped_key",
            "ciphertext_hash",
            "created_at_ms",
        ],
    )?;
    Ok(())
}

fn configure_file_connection(connection: &Connection) -> rusqlite::Result<()> {
    enable_wal_with_busy_retry(connection)?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

fn enable_wal_with_busy_retry(connection: &Connection) -> rusqlite::Result<()> {
    const RETRY_DELAY: Duration = Duration::from_millis(10);

    let started_at = Instant::now();
    loop {
        let result = connection.query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        });
        match result {
            Ok(journal_mode) if journal_mode.eq_ignore_ascii_case("wal") => return Ok(()),
            Ok(_) if started_at.elapsed() < BUSY_TIMEOUT => thread::sleep(RETRY_DELAY),
            Ok(_) => return Err(rusqlite::Error::InvalidQuery),
            Err(error) if sqlite_error_is_busy(&error) && started_at.elapsed() < BUSY_TIMEOUT => {
                thread::sleep(RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}

fn sqlite_error_is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(sqlite_error, _)
            if matches!(
                sqlite_error.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            )
    )
}

fn configure_common_connection(connection: &Connection) -> rusqlite::Result<()> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

fn read_schema_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row("PRAGMA user_version", [], |row| row.get(0))
}

fn reject_future_schema(version: i64) -> UserDbResult<()> {
    if version > SCHEMA_VERSION {
        return Err(UserDbError::invalid_input(
            "schema_version",
            format!("database version {version} is newer than supported version {SCHEMA_VERSION}"),
        ));
    }
    if version < 0 {
        return Err(UserDbError::invalid_input(
            "schema_version",
            format!("database version must be non-negative, got {version}"),
        ));
    }
    Ok(())
}

fn verify_integrity(connection: &Connection) -> rusqlite::Result<()> {
    let result: String = connection.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
    if result == "ok" {
        Ok(())
    } else {
        Err(rusqlite::Error::InvalidQuery)
    }
}

fn preserved_database_error(
    path: &Path,
    stage: &'static str,
    source: rusqlite::Error,
) -> UserDbError {
    UserDbError::DatabaseFile {
        path: path.to_path_buf(),
        stage,
        message: source.to_string(),
    }
}

fn preserved_userdb_error(path: &Path, stage: &'static str, source: UserDbError) -> UserDbError {
    UserDbError::DatabaseFile {
        path: path.to_path_buf(),
        stage,
        message: source.to_string(),
    }
}

fn create_schema_v5(transaction: &Transaction<'_>) -> UserDbResult<()> {
    transaction.execute_batch(&schema_v5_sql())?;
    Ok(())
}

fn schema_v5_sql() -> String {
    format!(
        "
        CREATE TABLE user_terms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            reading TEXT NOT NULL DEFAULT '',
            input_code TEXT NOT NULL,
            source TEXT NOT NULL,
            weight REAL NOT NULL DEFAULT 0.0 CHECK(weight >= 0.0),
            status TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            last_used_at_ms INTEGER,
            restored_at_ms INTEGER,
            import_batch_id INTEGER REFERENCES import_batches(id) ON DELETE SET NULL
        );
        CREATE UNIQUE INDEX idx_user_terms_identity
            ON user_terms(input_code, text, reading);

        CREATE TABLE selection_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            input_code TEXT NOT NULL,
            selected_text TEXT NOT NULL,
            selected_reading TEXT NOT NULL DEFAULT '',
            candidate_index INTEGER NOT NULL,
            candidate_count INTEGER NOT NULL,
            context_kind TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );

        CREATE TABLE negative_feedback (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            input_code TEXT NOT NULL,
            text TEXT NOT NULL,
            reading TEXT NOT NULL DEFAULT '',
            reason TEXT NOT NULL,
            context_kind TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );

        CREATE TABLE deleted_terms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            term_id INTEGER REFERENCES user_terms(id) ON DELETE SET NULL,
            input_code TEXT NOT NULL,
            text TEXT NOT NULL,
            reading TEXT NOT NULL DEFAULT '',
            deleted_at_ms INTEGER NOT NULL,
            reason TEXT NOT NULL,
            UNIQUE(input_code, text, reading)
        );

        CREATE TABLE ranker_weights (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            input_code TEXT NOT NULL,
            text TEXT NOT NULL,
            reading TEXT NOT NULL DEFAULT '',
            frequency INTEGER NOT NULL DEFAULT 0 CHECK(frequency >= 0 AND frequency <= {MAX_LEARNING_COUNT}),
            last_used_at_ms INTEGER,
            negative_score REAL NOT NULL DEFAULT 0.0 CHECK(negative_score >= 0.0 AND negative_score <= {MAX_LEARNING_COUNT}),
            context_kind TEXT NOT NULL DEFAULT 'general',
            updated_at_ms INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX idx_ranker_weights_identity
            ON ranker_weights(input_code, text, reading, context_kind);

        CREATE TABLE import_batches (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_name TEXT NOT NULL,
            term_count INTEGER NOT NULL,
            total_count INTEGER NOT NULL DEFAULT 0,
            inserted_count INTEGER NOT NULL DEFAULT 0,
            updated_count INTEGER NOT NULL DEFAULT 0,
            skipped_deleted_count INTEGER NOT NULL DEFAULT 0,
            skipped_duplicate_count INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL,
            notes TEXT NOT NULL DEFAULT ''
        );

        {SYNC_ORCHESTRATION_SCHEMA_SQL}
        "
    )
}

const SYNC_ORCHESTRATION_SCHEMA_SQL: &str = "
        CREATE TABLE IF NOT EXISTS sync_domain_state (
            domain_id TEXT PRIMARY KEY,
            state_version INTEGER NOT NULL DEFAULT 1 CHECK(state_version = 1),
            cursor TEXT,
            last_success_at_ms INTEGER
        );

        CREATE TABLE IF NOT EXISTS sync_remote_objects (
            domain_id TEXT NOT NULL,
            object_id TEXT NOT NULL,
            object_type TEXT NOT NULL,
            latest_version INTEGER NOT NULL CHECK(latest_version > 0),
            ciphertext_hash TEXT NOT NULL,
            owner_device_id TEXT NOT NULL,
            key_epoch INTEGER NOT NULL CHECK(key_epoch > 0),
            change_sequence INTEGER NOT NULL CHECK(change_sequence > 0),
            PRIMARY KEY(domain_id, object_id)
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_remote_change_sequence
            ON sync_remote_objects(domain_id, change_sequence);

        CREATE TABLE IF NOT EXISTS sync_local_objects (
            domain_id TEXT NOT NULL,
            object_id TEXT NOT NULL,
            object_type TEXT NOT NULL,
            payload_hash TEXT NOT NULL,
            local_revision INTEGER NOT NULL CHECK(local_revision > 0),
            acknowledged_revision INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged_revision >= 0),
            dirty INTEGER NOT NULL CHECK(dirty IN (0, 1)),
            PRIMARY KEY(domain_id, object_id)
        );

        CREATE TABLE IF NOT EXISTS sync_prepared_outbox (
            domain_id TEXT NOT NULL,
            object_id TEXT NOT NULL,
            object_type TEXT NOT NULL,
            local_revision INTEGER NOT NULL CHECK(local_revision > 0),
            version INTEGER NOT NULL CHECK(version > 0),
            base_version INTEGER,
            owner_device_id TEXT NOT NULL,
            key_id TEXT NOT NULL,
            key_epoch INTEGER NOT NULL CHECK(key_epoch > 0),
            algorithm TEXT NOT NULL,
            nonce BLOB NOT NULL,
            encrypted_payload BLOB NOT NULL,
            ciphertext_hash TEXT NOT NULL,
            record_count INTEGER NOT NULL CHECK(record_count > 0),
            signature_schema_version INTEGER NOT NULL,
            signature_algorithm TEXT NOT NULL,
            signature_key_id TEXT NOT NULL,
            signer_device_id TEXT NOT NULL,
            signature BLOB NOT NULL,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
            last_error_code TEXT,
            PRIMARY KEY(domain_id, object_id, version),
            UNIQUE(domain_id, object_id, local_revision)
        );

        CREATE TABLE IF NOT EXISTS sync_cycle_journal (
            domain_id TEXT PRIMARY KEY,
            phase TEXT NOT NULL,
            started_at_ms INTEGER NOT NULL,
            lease_expires_at_ms INTEGER NOT NULL,
            cancel_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancel_requested IN (0, 1))
        );
";

fn ensure_sync_orchestration_tables(transaction: &Transaction<'_>) -> UserDbResult<()> {
    transaction.execute_batch(SYNC_ORCHESTRATION_SCHEMA_SQL)?;
    Ok(())
}

const TRUSTED_LIFECYCLE_SCHEMA_SQL: &str = "
        CREATE TABLE IF NOT EXISTS sync_trusted_domains (
            domain_id TEXT PRIMARY KEY,
            current_key_epoch INTEGER NOT NULL CHECK(current_key_epoch > 0),
            active_key_id TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms >= created_at_ms),
            lifecycle_cursor TEXT NOT NULL CHECK(length(lifecycle_cursor) > 0),
            lifecycle_sequence INTEGER NOT NULL CHECK(lifecycle_sequence > 0)
        );

        CREATE TABLE IF NOT EXISTS sync_trusted_devices (
            domain_id TEXT NOT NULL REFERENCES sync_trusted_domains(domain_id) ON DELETE CASCADE,
            device_id TEXT NOT NULL,
            signing_algorithm TEXT NOT NULL,
            signing_public_key_id TEXT NOT NULL,
            signing_public_key BLOB NOT NULL,
            signing_key_created_at_ms INTEGER NOT NULL,
            key_agreement_public_key_id TEXT NOT NULL,
            key_agreement_public_key BLOB NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('active', 'revoked', 'lost')),
            authorized_at_ms INTEGER NOT NULL,
            revoked_at_ms INTEGER,
            last_seen_at_ms INTEGER,
            reject_from_change_sequence INTEGER,
            PRIMARY KEY(domain_id, device_id)
        );

        CREATE TABLE IF NOT EXISTS sync_trusted_lifecycle_events (
            domain_id TEXT NOT NULL REFERENCES sync_trusted_domains(domain_id) ON DELETE CASCADE,
            lifecycle_sequence INTEGER NOT NULL CHECK(lifecycle_sequence > 0),
            event_type TEXT NOT NULL CHECK(event_type IN ('initial_device', 'device_authorized', 'device_revoked', 'recovery_record_rotated', 'device_recovered')),
            record_id TEXT NOT NULL,
            key_epoch INTEGER NOT NULL CHECK(key_epoch > 0),
            reject_from_object_change_sequence INTEGER,
            created_at_ms INTEGER NOT NULL,
            record_json TEXT NOT NULL CHECK(length(record_json) > 0),
            PRIMARY KEY(domain_id, lifecycle_sequence),
            UNIQUE(domain_id, event_type, record_id)
        );
";

fn ensure_trusted_lifecycle_tables(transaction: &Transaction<'_>) -> UserDbResult<()> {
    transaction.execute_batch(TRUSTED_LIFECYCLE_SCHEMA_SQL)?;
    Ok(())
}

fn ensure_recovery_lifecycle_event_types(transaction: &Transaction<'_>) -> UserDbResult<()> {
    let schema: String = transaction.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'sync_trusted_lifecycle_events'",
        [],
        |row| row.get(0),
    )?;
    if schema.contains("recovery_record_rotated") {
        return Ok(());
    }
    transaction.execute_batch(
        "CREATE TABLE sync_trusted_lifecycle_events_v8 (
            domain_id TEXT NOT NULL REFERENCES sync_trusted_domains(domain_id) ON DELETE CASCADE,
            lifecycle_sequence INTEGER NOT NULL CHECK(lifecycle_sequence > 0),
            event_type TEXT NOT NULL CHECK(event_type IN ('initial_device', 'device_authorized', 'device_revoked', 'recovery_record_rotated', 'device_recovered')),
            record_id TEXT NOT NULL,
            key_epoch INTEGER NOT NULL CHECK(key_epoch > 0),
            reject_from_object_change_sequence INTEGER,
            created_at_ms INTEGER NOT NULL,
            record_json TEXT NOT NULL CHECK(length(record_json) > 0),
            PRIMARY KEY(domain_id, lifecycle_sequence),
            UNIQUE(domain_id, event_type, record_id)
        );
        INSERT INTO sync_trusted_lifecycle_events_v8 (
            domain_id, lifecycle_sequence, event_type, record_id, key_epoch,
            reject_from_object_change_sequence, created_at_ms, record_json
        ) SELECT domain_id, lifecycle_sequence, event_type, record_id, key_epoch,
            reject_from_object_change_sequence, created_at_ms, record_json
          FROM sync_trusted_lifecycle_events;
        DROP TABLE sync_trusted_lifecycle_events;
        ALTER TABLE sync_trusted_lifecycle_events_v8 RENAME TO sync_trusted_lifecycle_events;",
    )?;
    Ok(())
}

fn ensure_trusted_device_key_agreement_columns(transaction: &Transaction<'_>) -> UserDbResult<()> {
    if !table_columns(transaction, "sync_trusted_devices")?.contains("key_agreement_public_key_id")
    {
        transaction.execute_batch(
            "ALTER TABLE sync_trusted_devices ADD COLUMN key_agreement_public_key_id TEXT;
             ALTER TABLE sync_trusted_devices ADD COLUMN key_agreement_public_key BLOB;",
        )?;
    }

    let missing_count: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM sync_trusted_devices
         WHERE key_agreement_public_key_id IS NULL OR key_agreement_public_key IS NULL",
        [],
        |row| row.get(0),
    )?;
    if missing_count == 0 {
        return Ok(());
    }

    let mut statement = transaction.prepare(
        "SELECT domain_id, record_json FROM sync_trusted_lifecycle_events
         ORDER BY domain_id, lifecycle_sequence",
    )?;
    let records = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for (domain_id, record_json) in records {
        let value: serde_json::Value = serde_json::from_str(&record_json)
            .map_err(|error| UserDbError::invalid_input("lifecycle_record", error.to_string()))?;
        let device = value
            .get("device")
            .ok_or_else(|| UserDbError::invalid_input("lifecycle_record", "device is missing"))?;
        let device_id = device
            .get("device_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                UserDbError::invalid_input("lifecycle_record", "device id is invalid")
            })?;
        let key_id = device
            .get("key_agreement_public_key_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                UserDbError::invalid_input("lifecycle_record", "key agreement id is invalid")
            })?;
        let public_key = serde_json::from_value::<Vec<u8>>(
            device
                .get("key_agreement_public_key")
                .cloned()
                .ok_or_else(|| {
                    UserDbError::invalid_input(
                        "lifecycle_record",
                        "key agreement public key is missing",
                    )
                })?,
        )
        .map_err(|error| UserDbError::invalid_input("lifecycle_record", error.to_string()))?;
        transaction.execute(
            "UPDATE sync_trusted_devices
             SET key_agreement_public_key_id = ?3, key_agreement_public_key = ?4
             WHERE domain_id = ?1 AND device_id = ?2",
            params![domain_id, device_id, key_id, public_key],
        )?;
    }
    let remaining: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM sync_trusted_devices
         WHERE key_agreement_public_key_id IS NULL OR key_agreement_public_key IS NULL",
        [],
        |row| row.get(0),
    )?;
    if remaining != 0 {
        return Err(UserDbError::invalid_input(
            "sync_trusted_devices",
            "schema v6 cache cannot be migrated without signed key-agreement profile",
        ));
    }
    Ok(())
}

const WRAPPED_EPOCH_MATERIAL_SCHEMA_SQL: &str = "
        CREATE TABLE IF NOT EXISTS sync_wrapped_epoch_materials (
            domain_id TEXT NOT NULL REFERENCES sync_trusted_domains(domain_id) ON DELETE CASCADE,
            recipient_device_id TEXT NOT NULL,
            recipient_key_agreement_key_id TEXT NOT NULL,
            wrapping_key_id TEXT NOT NULL,
            key_epoch INTEGER NOT NULL CHECK(key_epoch > 0),
            schema_version INTEGER NOT NULL CHECK(schema_version > 0),
            algorithm TEXT NOT NULL,
            nonce BLOB NOT NULL,
            wrapped_key BLOB NOT NULL,
            ciphertext_hash TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            PRIMARY KEY(domain_id, recipient_device_id, key_epoch)
        );
";

fn ensure_wrapped_epoch_material_table(transaction: &Transaction<'_>) -> UserDbResult<()> {
    transaction.execute_batch(WRAPPED_EPOCH_MATERIAL_SCHEMA_SQL)?;
    Ok(())
}

fn create_legacy_schema_if_missing(transaction: &Transaction<'_>) -> UserDbResult<()> {
    transaction.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS user_terms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            reading TEXT NOT NULL DEFAULT '',
            input_code TEXT NOT NULL,
            source TEXT NOT NULL,
            weight REAL NOT NULL DEFAULT 0.0,
            status TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            last_used_at_ms INTEGER
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_user_terms_identity
            ON user_terms(input_code, text, reading);
        CREATE TABLE IF NOT EXISTS selection_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            input_code TEXT NOT NULL,
            selected_text TEXT NOT NULL,
            selected_reading TEXT NOT NULL DEFAULT '',
            candidate_index INTEGER NOT NULL,
            candidate_count INTEGER NOT NULL,
            context_kind TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS negative_feedback (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            input_code TEXT NOT NULL,
            text TEXT NOT NULL,
            reading TEXT NOT NULL DEFAULT '',
            reason TEXT NOT NULL,
            context_kind TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS deleted_terms (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            term_id INTEGER,
            text_hash TEXT NOT NULL,
            reading_hash TEXT NOT NULL,
            input_code_hash TEXT NOT NULL,
            deleted_at_ms INTEGER NOT NULL,
            reason TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ranker_weights (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            input_code TEXT NOT NULL,
            text TEXT NOT NULL,
            reading TEXT NOT NULL DEFAULT '',
            frequency INTEGER NOT NULL DEFAULT 0,
            recency_score REAL NOT NULL DEFAULT 0.0,
            negative_score REAL NOT NULL DEFAULT 0.0,
            context_kind TEXT NOT NULL DEFAULT 'general',
            updated_at_ms INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_ranker_weights_identity
            ON ranker_weights(input_code, text, reading, context_kind);
        CREATE TABLE IF NOT EXISTS import_batches (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_name TEXT NOT NULL,
            term_count INTEGER NOT NULL,
            created_at_ms INTEGER NOT NULL,
            notes TEXT NOT NULL DEFAULT ''
        );
        ",
    )?;
    Ok(())
}

fn ensure_import_batch_v2_columns(transaction: &Transaction<'_>) -> UserDbResult<()> {
    let columns = table_columns(transaction, "import_batches")?;
    let migrations = [
        (
            "total_count",
            "ALTER TABLE import_batches ADD COLUMN total_count INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "inserted_count",
            "ALTER TABLE import_batches ADD COLUMN inserted_count INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "updated_count",
            "ALTER TABLE import_batches ADD COLUMN updated_count INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "skipped_deleted_count",
            "ALTER TABLE import_batches ADD COLUMN skipped_deleted_count INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "skipped_duplicate_count",
            "ALTER TABLE import_batches ADD COLUMN skipped_duplicate_count INTEGER NOT NULL DEFAULT 0",
        ),
    ];
    for (column, sql) in migrations {
        if !columns.contains(column) {
            transaction.execute(sql, [])?;
        }
    }
    transaction.execute(
        "UPDATE import_batches
         SET total_count = CASE WHEN total_count = 0 THEN term_count ELSE total_count END,
             inserted_count = CASE WHEN inserted_count = 0 THEN term_count ELSE inserted_count END",
        [],
    )?;
    Ok(())
}

fn ensure_user_term_restore_version(transaction: &Transaction<'_>) -> UserDbResult<()> {
    if !table_columns(transaction, "user_terms")?.contains("restored_at_ms") {
        transaction.execute(
            "ALTER TABLE user_terms ADD COLUMN restored_at_ms INTEGER",
            [],
        )?;
    }
    Ok(())
}

fn ensure_user_term_import_batch(transaction: &Transaction<'_>) -> UserDbResult<()> {
    if !table_columns(transaction, "user_terms")?.contains("import_batch_id") {
        transaction.execute(
            "ALTER TABLE user_terms ADD COLUMN import_batch_id INTEGER REFERENCES import_batches(id) ON DELETE SET NULL",
            [],
        )?;
    }
    Ok(())
}

#[derive(Debug)]
struct LegacyTermIdentity {
    term_id: i64,
    input_code: String,
    text: String,
    reading: String,
}

#[derive(Debug)]
struct LegacyTombstone {
    term_id: Option<i64>,
    text_hash: String,
    reading_hash: String,
    input_code_hash: String,
    deleted_at_ms: i64,
    reason: String,
}

fn migrate_deleted_term_identity(transaction: &Transaction<'_>) -> UserDbResult<()> {
    let terms = load_legacy_term_identities(transaction)?;
    let tombstones = load_legacy_tombstones(transaction)?;
    transaction.execute_batch(
        "
        CREATE TABLE deleted_terms_v3 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            term_id INTEGER REFERENCES user_terms(id) ON DELETE SET NULL,
            input_code TEXT NOT NULL,
            text TEXT NOT NULL,
            reading TEXT NOT NULL DEFAULT '',
            deleted_at_ms INTEGER NOT NULL,
            reason TEXT NOT NULL,
            UNIQUE(input_code, text, reading)
        );
        ",
    )?;

    for tombstone in tombstones {
        let identity = resolve_legacy_tombstone_identity(&terms, &tombstone)?;
        transaction.execute(
            "INSERT INTO deleted_terms_v3 (
                term_id, input_code, text, reading, deleted_at_ms, reason
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(input_code, text, reading) DO UPDATE SET
                term_id = CASE
                    WHEN excluded.deleted_at_ms >= deleted_terms_v3.deleted_at_ms THEN excluded.term_id
                    ELSE deleted_terms_v3.term_id
                END,
                deleted_at_ms = MAX(deleted_terms_v3.deleted_at_ms, excluded.deleted_at_ms),
                reason = CASE
                    WHEN excluded.deleted_at_ms >= deleted_terms_v3.deleted_at_ms THEN excluded.reason
                    ELSE deleted_terms_v3.reason
                END",
            params![
                identity.term_id,
                identity.input_code,
                identity.text,
                identity.reading,
                tombstone.deleted_at_ms,
                tombstone.reason
            ],
        )?;
    }

    transaction.execute_batch(
        "
        DROP TABLE deleted_terms;
        ALTER TABLE deleted_terms_v3 RENAME TO deleted_terms;
        CREATE UNIQUE INDEX idx_deleted_terms_identity
            ON deleted_terms(input_code, text, reading);
        ",
    )?;
    Ok(())
}

fn load_legacy_term_identities(
    transaction: &Transaction<'_>,
) -> UserDbResult<Vec<LegacyTermIdentity>> {
    let mut statement =
        transaction.prepare("SELECT id, input_code, text, reading FROM user_terms ORDER BY id")?;
    let records = statement
        .query_map([], |row| {
            Ok(LegacyTermIdentity {
                term_id: row.get(0)?,
                input_code: row.get(1)?,
                text: row.get(2)?,
                reading: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(records)
}

fn load_legacy_tombstones(transaction: &Transaction<'_>) -> UserDbResult<Vec<LegacyTombstone>> {
    let mut statement = transaction.prepare(
        "SELECT term_id, text_hash, reading_hash, input_code_hash, deleted_at_ms, reason
         FROM deleted_terms ORDER BY id",
    )?;
    let records = statement
        .query_map([], |row| {
            Ok(LegacyTombstone {
                term_id: row.get(0)?,
                text_hash: row.get(1)?,
                reading_hash: row.get(2)?,
                input_code_hash: row.get(3)?,
                deleted_at_ms: row.get(4)?,
                reason: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(records)
}

fn resolve_legacy_tombstone_identity<'a>(
    terms: &'a [LegacyTermIdentity],
    tombstone: &LegacyTombstone,
) -> UserDbResult<&'a LegacyTermIdentity> {
    let matches_hashes = |term: &&LegacyTermIdentity| {
        legacy_stable_hash_hex(&term.input_code) == tombstone.input_code_hash
            && legacy_stable_hash_hex(&term.text) == tombstone.text_hash
            && legacy_stable_hash_hex(&term.reading) == tombstone.reading_hash
    };

    if let Some(term_id) = tombstone.term_id {
        if let Some(term) = terms.iter().find(|term| term.term_id == term_id) {
            if matches_hashes(&term) {
                return Ok(term);
            }
        }
    }

    let candidates = terms.iter().filter(matches_hashes).collect::<Vec<_>>();
    match candidates.as_slice() {
        [identity] => Ok(*identity),
        [] => Err(UserDbError::invalid_input(
            "deleted_term_identity",
            "legacy tombstone cannot be matched to a canonical user term identity",
        )),
        _ => Err(UserDbError::invalid_input(
            "deleted_term_identity",
            "legacy tombstone hash matches multiple canonical identities",
        )),
    }
}

fn migrate_ranker_last_used_time(transaction: &Transaction<'_>) -> UserDbResult<()> {
    let columns = table_columns(transaction, "ranker_weights")?;
    let last_used_expression = if columns.contains("last_used_at_ms") {
        "last_used_at_ms"
    } else if columns.contains("recency_score") {
        "CASE WHEN recency_score > 0 THEN CAST(recency_score AS INTEGER) ELSE NULL END"
    } else {
        "NULL"
    };
    transaction.execute_batch(&format!(
        "
        CREATE TABLE ranker_weights_v3 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            input_code TEXT NOT NULL,
            text TEXT NOT NULL,
            reading TEXT NOT NULL DEFAULT '',
            frequency INTEGER NOT NULL DEFAULT 0 CHECK(frequency >= 0 AND frequency <= {MAX_LEARNING_COUNT}),
            last_used_at_ms INTEGER,
            negative_score REAL NOT NULL DEFAULT 0.0 CHECK(negative_score >= 0.0 AND negative_score <= {MAX_LEARNING_COUNT}),
            context_kind TEXT NOT NULL DEFAULT 'general',
            updated_at_ms INTEGER NOT NULL
        );
        INSERT INTO ranker_weights_v3 (
            id, input_code, text, reading, frequency, last_used_at_ms,
            negative_score, context_kind, updated_at_ms
        )
        SELECT id, input_code, text, reading,
               MIN(MAX(frequency, 0), {MAX_LEARNING_COUNT}),
               {last_used_expression},
               MIN(MAX(negative_score, 0.0), {MAX_LEARNING_COUNT}),
               context_kind, updated_at_ms
        FROM ranker_weights;
        DROP TABLE ranker_weights;
        ALTER TABLE ranker_weights_v3 RENAME TO ranker_weights;
        CREATE UNIQUE INDEX idx_ranker_weights_identity
            ON ranker_weights(input_code, text, reading, context_kind);
        "
    ))?;
    Ok(())
}

fn has_any_user_table(connection: &Connection) -> UserDbResult<bool> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn table_columns(connection: &Connection, table: &str) -> UserDbResult<BTreeSet<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(columns)
}

fn require_columns(
    connection: &Connection,
    table: &'static str,
    required: &[&'static str],
) -> UserDbResult<()> {
    let columns = table_columns(connection, table)?;
    let missing = required
        .iter()
        .filter(|column| !columns.contains(**column))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(UserDbError::invalid_input(
            "schema",
            format!("table {table} is missing columns: {}", missing.join(", ")),
        ))
    }
}

#[cfg(unix)]
fn restrict_database_permissions(path: &Path) -> UserDbResult<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| UserDbError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn restrict_database_permissions(_path: &Path) -> UserDbResult<()> {
    Ok(())
}

fn restrict_sqlite_sidecar_permissions(path: &Path) -> UserDbResult<()> {
    for sidecar in sqlite_sidecar_paths(path) {
        if sidecar.exists() {
            restrict_database_permissions(&sidecar)?;
        }
    }
    Ok(())
}

fn sqlite_sidecar_paths(path: &Path) -> [PathBuf; 2] {
    let path = path.as_os_str().to_string_lossy();
    [
        PathBuf::from(format!("{path}-wal")),
        PathBuf::from(format!("{path}-shm")),
    ]
}
