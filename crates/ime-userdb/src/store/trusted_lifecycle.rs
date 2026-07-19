use radishlex_ime_crypto::{DeviceSigningPublicKey, SignatureAlgorithmId};
use radishlex_ime_sync::{
    RemoteLifecycleEvent, RemoteLifecycleEventKind, SyncCryptoLoadError, SyncDevice,
    SyncDeviceStatus, SyncTrustedDeviceProfile, SyncTrustedDeviceSource, SyncTrustedDomainState,
    VerifiedSyncLifecycle,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::json;

use crate::error::{UserDbError, UserDbResult};

use super::UserDb;

impl UserDb {
    pub fn store_verified_lifecycle(
        &mut self,
        verified: &VerifiedSyncLifecycle,
    ) -> UserDbResult<()> {
        let domain = verified.trusted_domain().domain();
        let last_sequence = verified
            .events()
            .last()
            .map(|event| event.lifecycle_sequence)
            .ok_or_else(|| {
                UserDbError::invalid_input("lifecycle_events", "events cannot be empty")
            })?;
        let event_records = verified
            .events()
            .iter()
            .map(lifecycle_record_json)
            .collect::<UserDbResult<Vec<_>>>()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = transaction
            .query_row(
                "SELECT lifecycle_sequence, lifecycle_cursor, current_key_epoch, active_key_id,
                    created_at_ms, updated_at_ms
                 FROM sync_trusted_domains WHERE domain_id = ?1",
                [&domain.domain_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?;
        if let Some((
            existing_sequence,
            existing_cursor,
            existing_epoch,
            existing_key_id,
            existing_created_at,
            existing_updated_at,
        )) = existing
        {
            let new_sequence = to_i64(last_sequence, "lifecycle_sequence")?;
            if new_sequence < existing_sequence {
                return Err(UserDbError::invalid_input(
                    "lifecycle_sequence",
                    "verified lifecycle cannot roll back or fork an observed cursor",
                ));
            }
            let mut statement = transaction.prepare(
                "SELECT record_json FROM sync_trusted_lifecycle_events
                 WHERE domain_id = ?1 ORDER BY lifecycle_sequence",
            )?;
            let existing_records = statement
                .query_map([&domain.domain_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            if existing_records.len() > event_records.len()
                || existing_records
                    .iter()
                    .zip(event_records.iter())
                    .any(|(old, new)| old != new)
            {
                return Err(UserDbError::invalid_input(
                    "lifecycle_events",
                    "verified lifecycle conflicts with the observed signed history",
                ));
            }
            if new_sequence == existing_sequence
                && (existing_cursor != verified.cursor().as_str()
                    || existing_epoch != to_i64(domain.current_key_epoch, "current_key_epoch")?
                    || existing_key_id != domain.active_key_id
                    || existing_created_at != domain.created_at_ms
                    || existing_updated_at != domain.updated_at_ms)
            {
                return Err(UserDbError::invalid_input(
                    "lifecycle_sequence",
                    "verified lifecycle cannot roll back or fork an observed cursor",
                ));
            }
        }

        transaction.execute(
            "INSERT INTO sync_trusted_domains (
                domain_id, current_key_epoch, active_key_id, created_at_ms, updated_at_ms,
                lifecycle_cursor, lifecycle_sequence
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(domain_id) DO UPDATE SET
                current_key_epoch = excluded.current_key_epoch,
                active_key_id = excluded.active_key_id,
                created_at_ms = excluded.created_at_ms,
                updated_at_ms = excluded.updated_at_ms,
                lifecycle_cursor = excluded.lifecycle_cursor,
                lifecycle_sequence = excluded.lifecycle_sequence",
            params![
                &domain.domain_id,
                to_i64(domain.current_key_epoch, "current_key_epoch")?,
                &domain.active_key_id,
                domain.created_at_ms,
                domain.updated_at_ms,
                verified.cursor().as_str(),
                to_i64(last_sequence, "lifecycle_sequence")?,
            ],
        )?;
        transaction.execute(
            "DELETE FROM sync_trusted_devices WHERE domain_id = ?1",
            [&domain.domain_id],
        )?;
        transaction.execute(
            "DELETE FROM sync_trusted_lifecycle_events WHERE domain_id = ?1",
            [&domain.domain_id],
        )?;

        for profile in verified.trusted_domain().device_profiles() {
            let device = profile.device();
            let public_key = profile.signing_public_key();
            transaction.execute(
                "INSERT INTO sync_trusted_devices (
                    domain_id, device_id, signing_algorithm, signing_public_key_id,
                    signing_public_key, signing_key_created_at_ms, status,
                    authorized_at_ms, revoked_at_ms, last_seen_at_ms,
                    reject_from_change_sequence
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    &domain.domain_id,
                    &device.device_id,
                    public_key.signature_algorithm.as_str(),
                    &public_key.signing_key_id,
                    &public_key.public_key,
                    public_key.created_at_ms,
                    device.status.as_str(),
                    device.authorized_at_ms,
                    device.revoked_at_ms,
                    device.last_seen_at_ms,
                    profile
                        .reject_from_change_sequence()
                        .map(|value| to_i64(value, "reject_from_change_sequence"))
                        .transpose()?,
                ],
            )?;
        }
        for (event, record_json) in verified.events().iter().zip(event_records) {
            transaction.execute(
                "INSERT INTO sync_trusted_lifecycle_events (
                    domain_id, lifecycle_sequence, event_type, record_id, key_epoch,
                    reject_from_object_change_sequence, created_at_ms, record_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    &event.domain_id,
                    to_i64(event.lifecycle_sequence, "lifecycle_sequence")?,
                    event_type_name(event.event_type),
                    &event.record_id,
                    to_i64(event.key_epoch, "key_epoch")?,
                    event
                        .reject_from_object_change_sequence
                        .map(|value| to_i64(value, "reject_from_object_change_sequence"))
                        .transpose()?,
                    event.created_at_ms,
                    record_json,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn trusted_domain_state(&self, domain_id: &str) -> UserDbResult<SyncTrustedDomainState> {
        let domain_row = self
            .connection
            .query_row(
                "SELECT current_key_epoch, active_key_id, created_at_ms, updated_at_ms
                 FROM sync_trusted_domains WHERE domain_id = ?1",
                [domain_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| UserDbError::invalid_input("domain_id", "trusted domain not found"))?;
        let current_key_epoch = from_i64(domain_row.0, "current_key_epoch")?;
        let domain = radishlex_ime_sync::SyncDomain::new(
            domain_id,
            current_key_epoch,
            domain_row.1,
            domain_row.2,
            domain_row.3,
        )
        .map_err(|error| UserDbError::invalid_input("trusted_domain", error.to_string()))?;

        let mut statement = self.connection.prepare(
            "SELECT device_id, signing_algorithm, signing_public_key_id, signing_public_key,
                signing_key_created_at_ms, status, authorized_at_ms, revoked_at_ms,
                last_seen_at_ms, reject_from_change_sequence
             FROM sync_trusted_devices WHERE domain_id = ?1 ORDER BY device_id",
        )?;
        let rows = statement.query_map([domain_id], |row| {
            Ok(TrustedDeviceRow {
                device_id: row.get(0)?,
                signing_algorithm: row.get(1)?,
                signing_public_key_id: row.get(2)?,
                signing_public_key: row.get(3)?,
                signing_key_created_at_ms: row.get(4)?,
                status: row.get(5)?,
                authorized_at_ms: row.get(6)?,
                revoked_at_ms: row.get(7)?,
                last_seen_at_ms: row.get(8)?,
                reject_from_change_sequence: row.get(9)?,
            })
        })?;
        let mut profiles = Vec::new();
        for row in rows {
            profiles.push(row?.try_into_profile()?);
        }
        SyncTrustedDomainState::new(domain, profiles).map_err(|_| {
            UserDbError::invalid_input("trusted_devices", "trusted device cache is invalid")
        })
    }
}

impl SyncTrustedDeviceSource for UserDb {
    fn load_trusted_domain(
        &mut self,
        domain_id: &str,
    ) -> Result<SyncTrustedDomainState, SyncCryptoLoadError> {
        self.trusted_domain_state(domain_id)
            .map_err(|error| match error {
                UserDbError::InvalidInput { .. } => SyncCryptoLoadError::InvalidState,
                _ => SyncCryptoLoadError::Unavailable,
            })
    }
}

struct TrustedDeviceRow {
    device_id: String,
    signing_algorithm: String,
    signing_public_key_id: String,
    signing_public_key: Vec<u8>,
    signing_key_created_at_ms: i64,
    status: String,
    authorized_at_ms: i64,
    revoked_at_ms: Option<i64>,
    last_seen_at_ms: Option<i64>,
    reject_from_change_sequence: Option<i64>,
}

impl TrustedDeviceRow {
    fn try_into_profile(self) -> UserDbResult<SyncTrustedDeviceProfile> {
        let status = match self.status.as_str() {
            "active" => SyncDeviceStatus::Active,
            "revoked" => SyncDeviceStatus::Revoked,
            "lost" => SyncDeviceStatus::Lost,
            _ => {
                return Err(UserDbError::invalid_input(
                    "device_status",
                    "trusted device status is invalid",
                ));
            }
        };
        let device = SyncDevice::new(
            self.device_id.clone(),
            self.signing_public_key_id.clone(),
            status,
            Some(self.authorized_at_ms),
            self.revoked_at_ms,
            self.last_seen_at_ms,
        )
        .map_err(|error| UserDbError::invalid_input("trusted_device", error.to_string()))?;
        let algorithm = SignatureAlgorithmId::new(self.signing_algorithm)
            .map_err(|error| UserDbError::invalid_input("signing_algorithm", error.to_string()))?;
        let public_key = DeviceSigningPublicKey::new(
            self.device_id,
            self.signing_public_key_id,
            algorithm,
            self.signing_public_key,
            self.signing_key_created_at_ms,
            self.revoked_at_ms,
        )
        .map_err(|error| UserDbError::invalid_input("signing_public_key", error.to_string()))?;
        match self.reject_from_change_sequence {
            Some(sequence) => SyncTrustedDeviceProfile::revoked_from_change_sequence(
                device,
                public_key,
                from_i64(sequence, "reject_from_change_sequence")?,
            ),
            None => SyncTrustedDeviceProfile::active(device, public_key),
        }
        .map_err(|_| {
            UserDbError::invalid_input("trusted_device", "trusted device profile is invalid")
        })
    }
}

fn lifecycle_record_json(event: &RemoteLifecycleEvent) -> UserDbResult<String> {
    serde_json::to_string(&json!({
        "domain_id": event.domain_id,
        "lifecycle_sequence": event.lifecycle_sequence,
        "event_type": event_type_name(event.event_type),
        "record_id": event.record_id,
        "key_epoch": event.key_epoch,
        "reject_from_object_change_sequence": event.reject_from_object_change_sequence,
        "created_at_ms": event.created_at_ms,
        "device": {
            "device_id": event.device.device_id,
            "signing_algorithm": event.device.signing_algorithm,
            "signing_public_key_id": event.device.signing_public_key_id,
            "signing_public_key": event.device.signing_public_key,
            "key_agreement_public_key_id": event.device.key_agreement_public_key_id,
            "key_agreement_public_key": event.device.key_agreement_public_key,
        },
        "authorization": event.authorization.as_ref().map(|value| json!({
            "join_request_id": value.join_request_id,
            "authorizer_device_id": value.authorizer_device_id,
            "recipient_device_id": value.recipient_device_id,
            "recipient_signing_public_key_id": value.recipient_signing_public_key_id,
            "recipient_key_agreement_key_id": value.recipient_key_agreement_key_id,
            "join_short_code": value.join_short_code,
            "join_challenge": value.join_challenge,
            "join_created_at_ms": value.join_created_at_ms,
            "join_expires_at_ms": value.join_expires_at_ms,
            "wrapping_key_id": value.wrapping_key_id,
            "encrypted_key_len": value.encrypted_key_len,
            "signature_schema_version": value.signature_schema_version,
            "signature_algorithm": value.signature_algorithm,
            "signature_key_id": value.signature_key_id,
            "signature": value.signature,
        })),
        "revocation": event.revocation.as_ref().map(|value| json!({
            "revoked_device_id": value.revoked_device_id,
            "revoker_device_id": value.revoker_device_id,
            "previous_key_epoch": value.previous_key_epoch,
            "new_key_epoch": value.new_key_epoch,
            "reason": value.reason,
            "signature_schema_version": value.signature_schema_version,
            "signature_algorithm": value.signature_algorithm,
            "signature_key_id": value.signature_key_id,
            "signature": value.signature,
        })),
    }))
    .map_err(|error| UserDbError::invalid_input("lifecycle_record", error.to_string()))
}

fn event_type_name(event_type: RemoteLifecycleEventKind) -> &'static str {
    match event_type {
        RemoteLifecycleEventKind::InitialDevice => "initial_device",
        RemoteLifecycleEventKind::DeviceAuthorized => "device_authorized",
        RemoteLifecycleEventKind::DeviceRevoked => "device_revoked",
    }
}

fn to_i64(value: u64, field: &'static str) -> UserDbResult<i64> {
    i64::try_from(value)
        .map_err(|_| UserDbError::invalid_input(field, "value exceeds SQLite integer range"))
}

fn from_i64(value: i64, field: &'static str) -> UserDbResult<u64> {
    u64::try_from(value).map_err(|_| UserDbError::invalid_input(field, "value must be positive"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use radishlex_ime_crypto::TestMemoryDeviceKeyStore;
    use radishlex_ime_sync::{
        verify_lifecycle_snapshot, OpaqueSyncCursor, RemoteLifecycleDevice, RemoteLifecycleEvent,
        RemoteLifecycleEventKind, RemoteLifecycleSnapshot, SyncDomain,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn verified_lifecycle_cache_survives_restart_and_rejects_cursor_fork() {
        let path = temporary_database_path();
        let (verified, anchor) = initial_lifecycle("cursor-a");
        {
            let mut db = UserDb::open(&path).expect("open lifecycle cache");
            db.store_verified_lifecycle(&verified)
                .expect("store verified lifecycle");
            let loaded = db
                .trusted_domain_state("domain-a")
                .expect("load trusted lifecycle");
            assert_eq!(loaded.domain().current_key_epoch, 1);
            assert_eq!(
                loaded
                    .device_profile("device-a")
                    .expect("initial device")
                    .signing_public_key(),
                &anchor
            );
        }
        {
            let mut reopened = UserDb::open(&path).expect("reopen lifecycle cache");
            let loaded = SyncTrustedDeviceSource::load_trusted_domain(&mut reopened, "domain-a")
                .expect("provider source loads after restart");
            assert!(loaded.device_profile("device-a").is_some());

            let (fork, _) = initial_lifecycle("cursor-fork");
            let error = reopened
                .store_verified_lifecycle(&fork)
                .expect_err("same sequence with another cursor must fail");
            assert!(error.to_string().contains("roll back or fork"));

            let (history_fork, _) = initial_lifecycle_with_agreement("cursor-a", vec![9, 9, 9]);
            let error = reopened
                .store_verified_lifecycle(&history_fork)
                .expect_err("replacing an observed signed record must fail");
            assert!(error.to_string().contains("conflicts with the observed"));
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("db-wal"));
        let _ = fs::remove_file(path.with_extension("db-shm"));
    }

    fn initial_lifecycle(cursor: &str) -> (VerifiedSyncLifecycle, DeviceSigningPublicKey) {
        initial_lifecycle_with_agreement(cursor, vec![2, 3, 4])
    }

    fn initial_lifecycle_with_agreement(
        cursor: &str,
        key_agreement_public_key: Vec<u8>,
    ) -> (VerifiedSyncLifecycle, DeviceSigningPublicKey) {
        let mut store = TestMemoryDeviceKeyStore::new();
        let anchor = store
            .insert_signing_key("device-a", "signing-key-a", [7u8; 32], 10)
            .expect("anchor key");
        let device = RemoteLifecycleDevice {
            domain_id: "domain-a".to_owned(),
            device_id: "device-a".to_owned(),
            signing_algorithm: "ed25519-v1".to_owned(),
            signing_public_key_id: "signing-key-a".to_owned(),
            signing_public_key: anchor.public_key.clone(),
            key_agreement_public_key_id: "agreement-device-a".to_owned(),
            key_agreement_public_key,
            status: SyncDeviceStatus::Active,
            authorized_at_ms: Some(10),
            revoked_at_ms: None,
            last_seen_at_ms: None,
        };
        let snapshot = RemoteLifecycleSnapshot {
            domain: SyncDomain::new("domain-a", 1, "sync-key-1", 10, 10).expect("domain"),
            entries: vec![RemoteLifecycleEvent {
                domain_id: "domain-a".to_owned(),
                lifecycle_sequence: 1,
                event_type: RemoteLifecycleEventKind::InitialDevice,
                record_id: "device-a".to_owned(),
                key_epoch: 1,
                reject_from_object_change_sequence: None,
                created_at_ms: 10,
                device,
                authorization: None,
                revocation: None,
            }],
            next_cursor: OpaqueSyncCursor::new(cursor).expect("cursor"),
        };
        (
            verify_lifecycle_snapshot(snapshot, &anchor).expect("verified snapshot"),
            anchor,
        )
    }

    fn temporary_database_path() -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "radishlex-trusted-lifecycle-{}-{suffix}.db",
            std::process::id()
        ))
    }
}
