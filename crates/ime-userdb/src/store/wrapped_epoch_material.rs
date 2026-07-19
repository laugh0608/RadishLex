use radishlex_ime_crypto::{Nonce, WrappedEpochMaterial};
use radishlex_ime_sync::{SyncCryptoLoadError, SyncWrappedEpochMaterialSource};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};

use crate::error::{UserDbError, UserDbResult};

use super::UserDb;

impl UserDb {
    pub fn store_wrapped_epoch_material(
        &mut self,
        record: &WrappedEpochMaterial,
    ) -> UserDbResult<()> {
        self.cache_wrapped_epoch_materials(std::slice::from_ref(record))?;
        Ok(())
    }

    pub fn cache_wrapped_epoch_materials(
        &mut self,
        records: &[WrappedEpochMaterial],
    ) -> UserDbResult<usize> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut inserted = 0;
        for record in records {
            validate_record_against_trusted_cache(&transaction, record)?;
            let existing = transaction
                .query_row(
                    "SELECT schema_version, algorithm, nonce, wrapped_key, ciphertext_hash, created_at_ms,
                        recipient_key_agreement_key_id
                     FROM sync_wrapped_epoch_materials
                     WHERE domain_id = ?1 AND recipient_device_id = ?2
                        AND key_epoch = ?3 AND wrapping_key_id = ?4",
                    params![
                        &record.domain_id,
                        &record.recipient_device_id,
                        i64::try_from(record.key_epoch).map_err(|_| UserDbError::invalid_input(
                            "key_epoch",
                            "value exceeds SQLite integer range",
                        ))?,
                        &record.wrapping_key_id,
                    ],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, String>(6)?,
                        ))
                    },
                )
                .optional()?;
            if let Some(existing) = existing {
                if existing.0 != i64::from(record.schema_version)
                    || existing.1 != record.algorithm
                    || existing.2 != record.nonce.as_bytes()
                    || existing.3 != record.wrapped_key
                    || existing.4 != record.ciphertext_hash
                    || existing.5 != record.created_at_ms
                    || existing.6 != record.recipient_key_agreement_key_id
                {
                    return Err(UserDbError::invalid_input(
                        "wrapped_epoch_material",
                        "existing locator has different ciphertext metadata",
                    ));
                }
                continue;
            }
            insert_wrapped_epoch_material(&transaction, record)?;
            inserted += 1;
        }
        transaction.commit()?;
        Ok(inserted)
    }

    pub fn wrapped_epoch_materials(
        &self,
        domain_id: &str,
        recipient_device_id: &str,
    ) -> UserDbResult<Vec<WrappedEpochMaterial>> {
        let mut statement = self.connection.prepare(
            "SELECT recipient_key_agreement_key_id, wrapping_key_id, key_epoch,
                schema_version, algorithm, nonce, wrapped_key, ciphertext_hash, created_at_ms
             FROM sync_wrapped_epoch_materials
             WHERE domain_id = ?1 AND recipient_device_id = ?2
             ORDER BY key_epoch",
        )?;
        let rows = statement.query_map(params![domain_id, recipient_device_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Vec<u8>>(5)?,
                row.get::<_, Vec<u8>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(8)?,
            ))
        })?;
        let mut records = Vec::new();
        for row in rows {
            let row = row?;
            let key_epoch = u64::try_from(row.2).map_err(|_| {
                UserDbError::invalid_input("key_epoch", "stored value must be positive")
            })?;
            let schema_version = u16::try_from(row.3).map_err(|_| {
                UserDbError::invalid_input("schema_version", "stored value is out of range")
            })?;
            let record = WrappedEpochMaterial {
                schema_version,
                algorithm: row.4,
                domain_id: domain_id.to_owned(),
                recipient_device_id: recipient_device_id.to_owned(),
                recipient_key_agreement_key_id: row.0,
                wrapping_key_id: row.1,
                key_epoch,
                nonce: Nonce::new(row.5).map_err(|error| {
                    UserDbError::invalid_input("wrapped_epoch_nonce", error.to_string())
                })?,
                wrapped_key: row.6,
                ciphertext_hash: row.7,
                created_at_ms: row.8,
            };
            record.validate().map_err(|error| {
                UserDbError::invalid_input("wrapped_epoch_material", error.to_string())
            })?;
            records.push(record);
        }
        Ok(records)
    }
}

fn validate_record_against_trusted_cache(
    transaction: &Transaction<'_>,
    record: &WrappedEpochMaterial,
) -> UserDbResult<()> {
    record
        .validate()
        .map_err(|error| UserDbError::invalid_input("wrapped_epoch_material", error.to_string()))?;
    let current_epoch: i64 = transaction
        .query_row(
            "SELECT current_key_epoch FROM sync_trusted_domains WHERE domain_id = ?1",
            [&record.domain_id],
            |row| row.get(0),
        )
        .map_err(|_| {
            UserDbError::invalid_input("wrapped_epoch_material", "trusted domain is unavailable")
        })?;
    let (trusted_key_id, status): (String, String) = transaction
        .query_row(
            "SELECT key_agreement_public_key_id, status
                 FROM sync_trusted_devices
                 WHERE domain_id = ?1 AND device_id = ?2",
            params![&record.domain_id, &record.recipient_device_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| {
            UserDbError::invalid_input(
                "wrapped_epoch_material",
                "trusted recipient device is unavailable",
            )
        })?;
    let key_epoch = i64::try_from(record.key_epoch).map_err(|_| {
        UserDbError::invalid_input("key_epoch", "value exceeds SQLite integer range")
    })?;
    if status != "active"
        || trusted_key_id != record.recipient_key_agreement_key_id
        || key_epoch > current_epoch
    {
        return Err(UserDbError::invalid_input(
            "wrapped_epoch_material",
            "record does not match the active trusted recipient profile",
        ));
    }
    Ok(())
}

fn insert_wrapped_epoch_material(
    transaction: &Transaction<'_>,
    record: &WrappedEpochMaterial,
) -> UserDbResult<()> {
    let key_epoch = i64::try_from(record.key_epoch).map_err(|_| {
        UserDbError::invalid_input("key_epoch", "value exceeds SQLite integer range")
    })?;
    transaction.execute(
        "INSERT INTO sync_wrapped_epoch_materials (
                domain_id, recipient_device_id, recipient_key_agreement_key_id,
                wrapping_key_id, key_epoch, schema_version, algorithm, nonce,
                wrapped_key, ciphertext_hash, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            &record.domain_id,
            &record.recipient_device_id,
            &record.recipient_key_agreement_key_id,
            &record.wrapping_key_id,
            key_epoch,
            i64::from(record.schema_version),
            &record.algorithm,
            record.nonce.as_bytes(),
            &record.wrapped_key,
            &record.ciphertext_hash,
            record.created_at_ms,
        ],
    )?;
    Ok(())
}

impl SyncWrappedEpochMaterialSource for UserDb {
    fn load_wrapped_epoch_materials(
        &mut self,
        domain_id: &str,
        local_device_id: &str,
    ) -> Result<Vec<WrappedEpochMaterial>, SyncCryptoLoadError> {
        self.wrapped_epoch_materials(domain_id, local_device_id)
            .map_err(|error| match error {
                UserDbError::InvalidInput { .. } => SyncCryptoLoadError::InvalidState,
                _ => SyncCryptoLoadError::Unavailable,
            })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use p256::{elliptic_curve::sec1::ToEncodedPoint, SecretKey};
    use radishlex_ime_crypto::{
        DeviceKeyAgreementPublicKey, KeyDescriptor, KeyRole, SyncMasterKeyMaterial,
    };

    use super::*;
    use crate::store::trusted_lifecycle::tests::initial_lifecycle;

    #[test]
    fn wrapped_epoch_ciphertext_survives_restart_without_plaintext_columns() {
        let path = temporary_database_path();
        let (verified, _) = initial_lifecycle("cursor-wrapped");
        let recipient = agreement_profile();
        let record = wrapped_record(&recipient);
        {
            let mut db = UserDb::open(&path).expect("open database");
            db.store_verified_lifecycle(&verified)
                .expect("store lifecycle");
            db.store_wrapped_epoch_material(&record)
                .expect("store wrapped record");
        }
        {
            let db = UserDb::open(&path).expect("reopen database");
            let loaded = db
                .wrapped_epoch_materials("domain-a", "device-a")
                .expect("load wrapped record");
            assert_eq!(loaded, [record]);
            let mut statement = db
                .connection
                .prepare("PRAGMA table_info(sync_wrapped_epoch_materials)")
                .expect("column query");
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))
                .expect("columns")
                .collect::<Result<std::collections::BTreeSet<_>, _>>()
                .expect("column names");
            assert!(!columns.contains("sync_master_key"));
            assert!(!columns.contains("shared_secret"));
            assert!(!columns.contains("plaintext"));
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("db-wal"));
        let _ = fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn wrapped_epoch_cache_is_idempotent_and_rejects_locator_fork() {
        let (verified, _) = initial_lifecycle("cursor-wrapped-idempotent");
        let recipient = agreement_profile();
        let record = wrapped_record(&recipient);
        let conflicting = wrapped_record(&recipient);
        assert_ne!(record.wrapped_key, conflicting.wrapped_key);
        let mut db = UserDb::open_in_memory().expect("open database");
        db.store_verified_lifecycle(&verified)
            .expect("store lifecycle");

        assert_eq!(
            db.cache_wrapped_epoch_materials(std::slice::from_ref(&record))
                .expect("cache record"),
            1
        );
        assert_eq!(
            db.cache_wrapped_epoch_materials(std::slice::from_ref(&record))
                .expect("repeat record"),
            0
        );
        let error = db
            .cache_wrapped_epoch_materials(&[conflicting])
            .expect_err("locator fork must fail");
        assert!(error.to_string().contains("different ciphertext metadata"));
        assert_eq!(
            db.wrapped_epoch_materials("domain-a", "device-a")
                .expect("load cache"),
            vec![record]
        );
    }

    fn agreement_profile() -> DeviceKeyAgreementPublicKey {
        let mut secret = [0u8; 32];
        secret[31] = 1;
        let public = SecretKey::from_slice(&secret)
            .expect("secret")
            .public_key()
            .to_encoded_point(false);
        DeviceKeyAgreementPublicKey::p256(
            "device-a",
            "agreement-device-a",
            public.as_bytes(),
            10,
            None,
        )
        .expect("profile")
    }

    fn wrapped_record(recipient: &DeviceKeyAgreementPublicKey) -> WrappedEpochMaterial {
        let descriptor =
            KeyDescriptor::new("sync-key-1", KeyRole::ObjectKey, 1).expect("key descriptor");
        let master = SyncMasterKeyMaterial::new([7u8; 32]).expect("master key");
        WrappedEpochMaterial::seal_for_recipient(
            "domain-a",
            recipient,
            "wrapping-device-a-1",
            &descriptor,
            &master,
            Nonce::new(vec![3u8; 24]).expect("nonce"),
            20,
        )
        .expect("wrapped record")
    }

    fn temporary_database_path() -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "radishlex-wrapped-epoch-{}-{suffix}.db",
            std::process::id()
        ))
    }
}
