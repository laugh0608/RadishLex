use radishlex_ime_crypto::{
    AlgorithmId, CiphertextHash, DeviceSignature, EncryptedObjectEnvelope, Nonce,
    SignatureAlgorithmId, SignedSyncObjectManifest, SIGNATURE_SCHEMA_VERSION,
};
use radishlex_ime_sync::{
    AssembledSyncObject, DecryptedSyncObject, EncryptedSyncObjectDraft, LocalSyncSnapshot,
    OpaqueSyncCursor, PlaintextSyncPayload, PreparedSyncOutbox, SyncApplyPageSummary,
    SyncCyclePhase, SyncLocalRepository, SyncObjectType, SyncOrchestrationError,
    SyncOrchestrationErrorCode,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::model::{UserDbSyncPayloadObjectType, UserDbSyncPlaintextPayload};
use crate::sync_decode::{decode_userdb_sync_objects, UserDbDecryptedSyncObject};

use super::{sync_apply, sync_payload, UserDb};

const DICTIONARY_USER_TERMS_OBJECT_ID: &str = "dictionary-user-terms-v1";
const DICTIONARY_DELETED_TERMS_OBJECT_ID: &str = "dictionary-deleted-terms-v1";
const RANKER_WEIGHTS_OBJECT_ID: &str = "ranker-weights-v1";

impl SyncLocalRepository for UserDb {
    fn begin_cycle(
        &mut self,
        domain_id: &str,
        started_at_ms: i64,
        lease_expires_at_ms: i64,
    ) -> Result<(), SyncOrchestrationError> {
        validate_required(domain_id, SyncCyclePhase::Preflight)?;
        if lease_expires_at_ms <= started_at_ms {
            return Err(local_error(SyncCyclePhase::Preflight));
        }
        let changed = self
            .connection
            .execute(
                "INSERT INTO sync_cycle_journal (
                    domain_id, phase, started_at_ms, lease_expires_at_ms, cancel_requested
                 ) VALUES (?1, ?2, ?3, ?4, 0)
                 ON CONFLICT(domain_id) DO UPDATE SET
                    phase = excluded.phase,
                    started_at_ms = excluded.started_at_ms,
                    lease_expires_at_ms = excluded.lease_expires_at_ms,
                    cancel_requested = 0
                 WHERE sync_cycle_journal.lease_expires_at_ms <= excluded.started_at_ms",
                params![
                    domain_id,
                    SyncCyclePhase::Preflight.as_str(),
                    started_at_ms,
                    lease_expires_at_ms
                ],
            )
            .map_err(|_| local_error(SyncCyclePhase::Preflight))?;
        if changed != 1 {
            return Err(local_error(SyncCyclePhase::Preflight));
        }
        Ok(())
    }

    fn record_cycle_phase(
        &mut self,
        domain_id: &str,
        phase: SyncCyclePhase,
    ) -> Result<(), SyncOrchestrationError> {
        let changed = self
            .connection
            .execute(
                "UPDATE sync_cycle_journal SET phase = ?2 WHERE domain_id = ?1",
                params![domain_id, phase.as_str()],
            )
            .map_err(|_| local_error(phase))?;
        if changed != 1 {
            return Err(local_error(phase));
        }
        Ok(())
    }

    fn cycle_cancel_requested(&self, domain_id: &str) -> Result<bool, SyncOrchestrationError> {
        self.connection
            .query_row(
                "SELECT cancel_requested FROM sync_cycle_journal WHERE domain_id = ?1",
                [domain_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| local_error(SyncCyclePhase::Preflight))
    }

    fn finish_cycle(&mut self, domain_id: &str) -> Result<(), SyncOrchestrationError> {
        self.connection
            .execute(
                "DELETE FROM sync_cycle_journal WHERE domain_id = ?1",
                [domain_id],
            )
            .map_err(|_| local_error(SyncCyclePhase::Complete))?;
        Ok(())
    }

    fn current_cursor(
        &self,
        domain_id: &str,
    ) -> Result<Option<OpaqueSyncCursor>, SyncOrchestrationError> {
        validate_required(domain_id, SyncCyclePhase::Discover)?;
        let cursor = self
            .connection
            .query_row(
                "SELECT cursor FROM sync_domain_state WHERE domain_id = ?1",
                [domain_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|_| local_error(SyncCyclePhase::Discover))?
            .flatten();
        cursor
            .map(|value| {
                OpaqueSyncCursor::new(value).map_err(|_| {
                    SyncOrchestrationError::new(
                        SyncOrchestrationErrorCode::CursorInvalid,
                        SyncCyclePhase::Discover,
                        false,
                    )
                })
            })
            .transpose()
    }

    fn apply_download_page(
        &mut self,
        domain_id: &str,
        next_cursor: &OpaqueSyncCursor,
        objects: &[DecryptedSyncObject],
    ) -> Result<SyncApplyPageSummary, SyncOrchestrationError> {
        validate_required(domain_id, SyncCyclePhase::ApplyAndAdvanceCursor)?;
        let decoded_objects = objects
            .iter()
            .map(|object| {
                if object.remote.domain_id != domain_id {
                    return Err(local_error(SyncCyclePhase::ApplyAndAdvanceCursor));
                }
                UserDbDecryptedSyncObject::new(
                    to_userdb_object_type(object.remote.object_type)?,
                    object.remote.key_epoch,
                    object.plaintext_payload.clone(),
                )
                .map_err(|_| decode_error())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let batch = decode_userdb_sync_objects(decoded_objects).map_err(|_| decode_error())?;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| local_error(SyncCyclePhase::ApplyAndAdvanceCursor))?;
        let apply = sync_apply::apply_decoded_sync_payload_batch_on(&transaction, &batch)
            .map_err(|_| local_error(SyncCyclePhase::ApplyAndAdvanceCursor))?;
        for object in objects {
            store_remote_observation(&transaction, &object.remote)?;
        }
        let payloads = sync_payload::collect_p2_plaintext_payloads_on(&transaction)
            .map_err(|_| local_error(SyncCyclePhase::ApplyAndAdvanceCursor))?;
        refresh_local_revisions(&transaction, domain_id, &payloads)?;
        transaction
            .execute(
                "INSERT INTO sync_domain_state (domain_id, state_version, cursor)
                 VALUES (?1, 1, ?2)
                 ON CONFLICT(domain_id) DO UPDATE SET cursor = excluded.cursor",
                params![domain_id, next_cursor.as_str()],
            )
            .map_err(|_| local_error(SyncCyclePhase::ApplyAndAdvanceCursor))?;
        transaction
            .commit()
            .map_err(|_| local_error(SyncCyclePhase::ApplyAndAdvanceCursor))?;

        Ok(SyncApplyPageSummary {
            discovered: objects.len(),
            applied_user_terms: apply.user_terms_written,
            applied_deleted_terms: apply.deleted_terms_written,
            applied_ranker_weights: apply.ranker_weights_written,
        })
    }

    fn outbound_snapshots(
        &mut self,
        domain_id: &str,
    ) -> Result<Vec<LocalSyncSnapshot>, SyncOrchestrationError> {
        validate_required(domain_id, SyncCyclePhase::PlanUpload)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| local_error(SyncCyclePhase::PlanUpload))?;
        let payloads = sync_payload::collect_p2_plaintext_payloads_on(&transaction)
            .map_err(|_| local_error(SyncCyclePhase::PlanUpload))?;
        refresh_local_revisions(&transaction, domain_id, &payloads)?;

        let mut snapshots = Vec::new();
        for payload in payloads {
            let object_type = to_sync_object_type(payload.object_type);
            let object_id = object_id_for(object_type);
            let state = transaction
                .query_row(
                    "SELECT local_revision, dirty
                     FROM sync_local_objects
                     WHERE domain_id = ?1 AND object_id = ?2",
                    params![domain_id, object_id],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
                .map_err(|_| local_error(SyncCyclePhase::PlanUpload))?;
            if state.1 == 0 {
                continue;
            }
            let remote_base_version = transaction
                .query_row(
                    "SELECT latest_version FROM sync_remote_objects
                     WHERE domain_id = ?1 AND object_id = ?2",
                    params![domain_id, object_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(|_| local_error(SyncCyclePhase::PlanUpload))?
                .map(|value| from_i64(value, SyncCyclePhase::PlanUpload))
                .transpose()?;
            snapshots.push(LocalSyncSnapshot {
                domain_id: domain_id.to_owned(),
                object_id: object_id.to_owned(),
                object_type,
                local_revision: from_i64(state.0, SyncCyclePhase::PlanUpload)?,
                remote_base_version,
                payload: PlaintextSyncPayload::new(
                    object_type,
                    payload.record_count,
                    payload.bytes,
                )
                .map_err(|_| local_error(SyncCyclePhase::PlanUpload))?,
            });
        }
        transaction
            .commit()
            .map_err(|_| local_error(SyncCyclePhase::PlanUpload))?;
        Ok(snapshots)
    }

    fn prepared_outboxes(
        &self,
        domain_id: &str,
    ) -> Result<Vec<PreparedSyncOutbox>, SyncOrchestrationError> {
        validate_required(domain_id, SyncCyclePhase::Upload)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT object_id, object_type, local_revision, version, base_version,
                        owner_device_id, key_id, key_epoch, algorithm, nonce,
                        encrypted_payload, ciphertext_hash, record_count,
                        signature_schema_version, signature_algorithm, signature_key_id,
                        signer_device_id, signature, created_at_ms, updated_at_ms, attempt_count
                 FROM sync_prepared_outbox
                 WHERE domain_id = ?1
                 ORDER BY object_id, version",
            )
            .map_err(|_| local_error(SyncCyclePhase::Upload))?;
        let rows = statement
            .query_map([domain_id], StoredOutbox::from_row)
            .map_err(|_| local_error(SyncCyclePhase::Upload))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| local_error(SyncCyclePhase::Upload))?;
        rows.into_iter()
            .map(|row| row.into_prepared(domain_id))
            .collect()
    }

    fn store_prepared_outbox(
        &mut self,
        outbox: &PreparedSyncOutbox,
    ) -> Result<(), SyncOrchestrationError> {
        validate_prepared_outbox(outbox)?;
        let draft = &outbox.object.draft;
        let signature = &outbox.manifest.signature;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| local_error(SyncCyclePhase::PrepareSignedOutbox))?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO sync_prepared_outbox (
                    domain_id, object_id, object_type, local_revision, version, base_version,
                    owner_device_id, key_id, key_epoch, algorithm, nonce, encrypted_payload,
                    ciphertext_hash, record_count, signature_schema_version, signature_algorithm,
                    signature_key_id, signer_device_id, signature, created_at_ms, updated_at_ms,
                    attempt_count
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                    ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
                 )",
                params![
                    outbox.domain_id,
                    draft.object_id,
                    draft.object_type.as_str(),
                    to_i64(outbox.local_revision, SyncCyclePhase::PrepareSignedOutbox)?,
                    to_i64(draft.version, SyncCyclePhase::PrepareSignedOutbox)?,
                    optional_to_i64(draft.base_version, SyncCyclePhase::PrepareSignedOutbox)?,
                    draft.owner_device_id,
                    draft.key_id,
                    to_i64(draft.key_epoch, SyncCyclePhase::PrepareSignedOutbox)?,
                    draft.algorithm,
                    draft.nonce,
                    outbox.object.envelope.encrypted_payload,
                    draft.ciphertext_hash,
                    usize_to_i64(
                        outbox.object.record_count,
                        SyncCyclePhase::PrepareSignedOutbox
                    )?,
                    i64::from(signature.signature_schema_version),
                    signature.signature_algorithm.as_str(),
                    signature.signature_key_id,
                    signature.signer_device_id,
                    signature.signature,
                    draft.created_at_ms,
                    draft.updated_at_ms,
                    i64::from(outbox.attempt_count),
                ],
            )
            .map_err(|_| local_error(SyncCyclePhase::PrepareSignedOutbox))?;
        if inserted == 0 {
            let existing: (String, i64) = transaction
                .query_row(
                    "SELECT ciphertext_hash, local_revision FROM sync_prepared_outbox
                     WHERE domain_id = ?1 AND object_id = ?2 AND version = ?3",
                    params![
                        outbox.domain_id,
                        draft.object_id,
                        to_i64(draft.version, SyncCyclePhase::PrepareSignedOutbox)?
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|_| local_error(SyncCyclePhase::PrepareSignedOutbox))?;
            if existing.0 != draft.ciphertext_hash
                || existing.1 != to_i64(outbox.local_revision, SyncCyclePhase::PrepareSignedOutbox)?
            {
                return Err(local_error(SyncCyclePhase::PrepareSignedOutbox));
            }
        }
        transaction
            .commit()
            .map_err(|_| local_error(SyncCyclePhase::PrepareSignedOutbox))?;
        Ok(())
    }

    fn acknowledge_outbox(
        &mut self,
        domain_id: &str,
        object_id: &str,
        version: u64,
        ciphertext_hash: &str,
        change_sequence: u64,
        acknowledged_at_ms: i64,
    ) -> Result<(), SyncOrchestrationError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| local_error(SyncCyclePhase::AcknowledgeOutbox))?;
        let stored = transaction
            .query_row(
                "SELECT object_type, local_revision, owner_device_id, key_epoch, ciphertext_hash
                 FROM sync_prepared_outbox
                 WHERE domain_id = ?1 AND object_id = ?2 AND version = ?3",
                params![
                    domain_id,
                    object_id,
                    to_i64(version, SyncCyclePhase::AcknowledgeOutbox)?
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .map_err(|_| local_error(SyncCyclePhase::AcknowledgeOutbox))?;
        if stored.4 != ciphertext_hash {
            return Err(local_error(SyncCyclePhase::AcknowledgeOutbox));
        }
        transaction
            .execute(
                "INSERT INTO sync_remote_objects (
                    domain_id, object_id, object_type, latest_version, ciphertext_hash,
                    owner_device_id, key_epoch, change_sequence
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(domain_id, object_id) DO UPDATE SET
                    object_type = excluded.object_type,
                    latest_version = excluded.latest_version,
                    ciphertext_hash = excluded.ciphertext_hash,
                    owner_device_id = excluded.owner_device_id,
                    key_epoch = excluded.key_epoch,
                    change_sequence = excluded.change_sequence
                 WHERE excluded.change_sequence >= sync_remote_objects.change_sequence",
                params![
                    domain_id,
                    object_id,
                    stored.0,
                    to_i64(version, SyncCyclePhase::AcknowledgeOutbox)?,
                    ciphertext_hash,
                    stored.2,
                    stored.3,
                    to_i64(change_sequence, SyncCyclePhase::AcknowledgeOutbox)?,
                ],
            )
            .map_err(|_| local_error(SyncCyclePhase::AcknowledgeOutbox))?;
        transaction
            .execute(
                "UPDATE sync_local_objects
                 SET acknowledged_revision = MAX(acknowledged_revision, ?3),
                     dirty = CASE WHEN local_revision <= ?3 THEN 0 ELSE 1 END
                 WHERE domain_id = ?1 AND object_id = ?2",
                params![domain_id, object_id, stored.1],
            )
            .map_err(|_| local_error(SyncCyclePhase::AcknowledgeOutbox))?;
        transaction
            .execute(
                "DELETE FROM sync_prepared_outbox
                 WHERE domain_id = ?1 AND object_id = ?2 AND version = ?3",
                params![
                    domain_id,
                    object_id,
                    to_i64(version, SyncCyclePhase::AcknowledgeOutbox)?
                ],
            )
            .map_err(|_| local_error(SyncCyclePhase::AcknowledgeOutbox))?;
        transaction
            .execute(
                "INSERT INTO sync_domain_state (domain_id, state_version, last_success_at_ms)
                 VALUES (?1, 1, ?2)
                 ON CONFLICT(domain_id) DO UPDATE SET last_success_at_ms = excluded.last_success_at_ms",
                params![domain_id, acknowledged_at_ms],
            )
            .map_err(|_| local_error(SyncCyclePhase::AcknowledgeOutbox))?;
        transaction
            .commit()
            .map_err(|_| local_error(SyncCyclePhase::AcknowledgeOutbox))?;
        Ok(())
    }

    fn record_outbox_attempt(
        &mut self,
        domain_id: &str,
        object_id: &str,
        version: u64,
        error_code: Option<SyncOrchestrationErrorCode>,
    ) -> Result<(), SyncOrchestrationError> {
        let changed = self
            .connection
            .execute(
                "UPDATE sync_prepared_outbox
                 SET attempt_count = attempt_count + 1, last_error_code = ?4
                 WHERE domain_id = ?1 AND object_id = ?2 AND version = ?3",
                params![
                    domain_id,
                    object_id,
                    to_i64(version, SyncCyclePhase::Upload)?,
                    error_code.map(SyncOrchestrationErrorCode::as_str),
                ],
            )
            .map_err(|_| local_error(SyncCyclePhase::Upload))?;
        if changed != 1 {
            return Err(local_error(SyncCyclePhase::Upload));
        }
        Ok(())
    }

    fn supersede_outbox(
        &mut self,
        domain_id: &str,
        object_id: &str,
        version: u64,
        ciphertext_hash: &str,
    ) -> Result<(), SyncOrchestrationError> {
        let changed = self
            .connection
            .execute(
                "DELETE FROM sync_prepared_outbox
                 WHERE domain_id = ?1 AND object_id = ?2 AND version = ?3
                   AND ciphertext_hash = ?4",
                params![
                    domain_id,
                    object_id,
                    to_i64(version, SyncCyclePhase::Upload)?,
                    ciphertext_hash,
                ],
            )
            .map_err(|_| local_error(SyncCyclePhase::Upload))?;
        if changed != 1 {
            return Err(local_error(SyncCyclePhase::Upload));
        }
        Ok(())
    }
}

impl UserDb {
    pub fn request_sync_cycle_cancel(&self, domain_id: &str) -> crate::UserDbResult<bool> {
        Ok(self.connection.execute(
            "UPDATE sync_cycle_journal SET cancel_requested = 1 WHERE domain_id = ?1",
            [domain_id],
        )? == 1)
    }
}

fn refresh_local_revisions(
    transaction: &Transaction<'_>,
    domain_id: &str,
    payloads: &[UserDbSyncPlaintextPayload],
) -> Result<(), SyncOrchestrationError> {
    for payload in payloads {
        let object_type = to_sync_object_type(payload.object_type);
        let object_id = object_id_for(object_type);
        let payload_hash = format!("{:x}", Sha256::digest(&payload.bytes));
        transaction
            .execute(
                "INSERT INTO sync_local_objects (
                    domain_id, object_id, object_type, payload_hash,
                    local_revision, acknowledged_revision, dirty
                 ) VALUES (?1, ?2, ?3, ?4, 1, 0, 1)
                 ON CONFLICT(domain_id, object_id) DO UPDATE SET
                    object_type = excluded.object_type,
                    payload_hash = excluded.payload_hash,
                    local_revision = sync_local_objects.local_revision + 1,
                    dirty = 1
                 WHERE sync_local_objects.payload_hash != excluded.payload_hash",
                params![domain_id, object_id, object_type.as_str(), payload_hash],
            )
            .map_err(|_| local_error(SyncCyclePhase::ApplyAndAdvanceCursor))?;
    }
    Ok(())
}

fn store_remote_observation(
    transaction: &Transaction<'_>,
    remote: &radishlex_ime_sync::RemoteObjectVersion,
) -> Result<(), SyncOrchestrationError> {
    transaction
        .execute(
            "INSERT INTO sync_remote_objects (
                domain_id, object_id, object_type, latest_version, ciphertext_hash,
                owner_device_id, key_epoch, change_sequence
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(domain_id, object_id) DO UPDATE SET
                object_type = excluded.object_type,
                latest_version = excluded.latest_version,
                ciphertext_hash = excluded.ciphertext_hash,
                owner_device_id = excluded.owner_device_id,
                key_epoch = excluded.key_epoch,
                change_sequence = excluded.change_sequence
             WHERE excluded.change_sequence > sync_remote_objects.change_sequence",
            params![
                remote.domain_id,
                remote.object_id,
                remote.object_type.as_str(),
                to_i64(remote.version, SyncCyclePhase::ApplyAndAdvanceCursor)?,
                remote.ciphertext_hash,
                remote.owner_device_id,
                to_i64(remote.key_epoch, SyncCyclePhase::ApplyAndAdvanceCursor)?,
                to_i64(
                    remote.change_sequence,
                    SyncCyclePhase::ApplyAndAdvanceCursor
                )?,
            ],
        )
        .map_err(|_| local_error(SyncCyclePhase::ApplyAndAdvanceCursor))?;
    Ok(())
}

fn validate_prepared_outbox(outbox: &PreparedSyncOutbox) -> Result<(), SyncOrchestrationError> {
    outbox
        .object
        .envelope
        .validate()
        .map_err(|_| local_error(SyncCyclePhase::PrepareSignedOutbox))?;
    outbox
        .object
        .draft
        .validate()
        .map_err(|_| local_error(SyncCyclePhase::PrepareSignedOutbox))?;
    outbox
        .manifest
        .validate()
        .map_err(|_| local_error(SyncCyclePhase::PrepareSignedOutbox))?;
    let expected_draft = EncryptedSyncObjectDraft::from_crypto_envelope(&outbox.object.envelope)
        .map_err(|_| local_error(SyncCyclePhase::PrepareSignedOutbox))?;
    let expected_manifest = SignedSyncObjectManifest::new(
        &outbox.domain_id,
        &outbox.object.envelope,
        outbox.manifest.signature.clone(),
    )
    .map_err(|_| local_error(SyncCyclePhase::PrepareSignedOutbox))?;
    if outbox.local_revision == 0
        || expected_draft != outbox.object.draft
        || expected_manifest != outbox.manifest
    {
        return Err(local_error(SyncCyclePhase::PrepareSignedOutbox));
    }
    Ok(())
}

struct StoredOutbox {
    object_id: String,
    object_type: String,
    local_revision: i64,
    version: i64,
    base_version: Option<i64>,
    owner_device_id: String,
    key_id: String,
    key_epoch: i64,
    algorithm: String,
    nonce: Vec<u8>,
    encrypted_payload: Vec<u8>,
    ciphertext_hash: String,
    record_count: i64,
    signature_schema_version: i64,
    signature_algorithm: String,
    signature_key_id: String,
    signer_device_id: String,
    signature: Vec<u8>,
    created_at_ms: i64,
    updated_at_ms: i64,
    attempt_count: i64,
}

impl StoredOutbox {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            object_id: row.get(0)?,
            object_type: row.get(1)?,
            local_revision: row.get(2)?,
            version: row.get(3)?,
            base_version: row.get(4)?,
            owner_device_id: row.get(5)?,
            key_id: row.get(6)?,
            key_epoch: row.get(7)?,
            algorithm: row.get(8)?,
            nonce: row.get(9)?,
            encrypted_payload: row.get(10)?,
            ciphertext_hash: row.get(11)?,
            record_count: row.get(12)?,
            signature_schema_version: row.get(13)?,
            signature_algorithm: row.get(14)?,
            signature_key_id: row.get(15)?,
            signer_device_id: row.get(16)?,
            signature: row.get(17)?,
            created_at_ms: row.get(18)?,
            updated_at_ms: row.get(19)?,
            attempt_count: row.get(20)?,
        })
    }

    fn into_prepared(self, domain_id: &str) -> Result<PreparedSyncOutbox, SyncOrchestrationError> {
        if self.signature_schema_version != i64::from(SIGNATURE_SCHEMA_VERSION) {
            return Err(local_error(SyncCyclePhase::Upload));
        }
        let object_type = parse_sync_object_type(&self.object_type)?;
        let envelope = EncryptedObjectEnvelope {
            schema_version: radishlex_ime_crypto::ENVELOPE_SCHEMA_VERSION,
            object_id: self.object_id,
            object_type: object_type.to_crypto_object_type(),
            owner_device_id: self.owner_device_id,
            key_id: self.key_id,
            key_epoch: from_i64(self.key_epoch, SyncCyclePhase::Upload)?,
            algorithm: AlgorithmId::new(self.algorithm)
                .map_err(|_| local_error(SyncCyclePhase::Upload))?,
            nonce: Nonce::new(self.nonce).map_err(|_| local_error(SyncCyclePhase::Upload))?,
            version: from_i64(self.version, SyncCyclePhase::Upload)?,
            base_version: self
                .base_version
                .map(|value| from_i64(value, SyncCyclePhase::Upload))
                .transpose()?,
            encrypted_payload: self.encrypted_payload,
            ciphertext_hash: CiphertextHash::new(self.ciphertext_hash)
                .map_err(|_| local_error(SyncCyclePhase::Upload))?,
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        };
        envelope
            .validate()
            .map_err(|_| local_error(SyncCyclePhase::Upload))?;
        let draft = EncryptedSyncObjectDraft::from_crypto_envelope(&envelope)
            .map_err(|_| local_error(SyncCyclePhase::Upload))?;
        let signature = DeviceSignature::new_for_algorithm(
            SignatureAlgorithmId::new(self.signature_algorithm)
                .map_err(|_| local_error(SyncCyclePhase::Upload))?,
            self.signature_key_id,
            self.signer_device_id,
            self.signature,
        )
        .map_err(|_| local_error(SyncCyclePhase::Upload))?;
        let manifest = SignedSyncObjectManifest::new(domain_id, &envelope, signature)
            .map_err(|_| local_error(SyncCyclePhase::Upload))?;
        let prepared = PreparedSyncOutbox {
            domain_id: domain_id.to_owned(),
            local_revision: from_i64(self.local_revision, SyncCyclePhase::Upload)?,
            object: AssembledSyncObject {
                envelope,
                draft,
                record_count: usize::try_from(self.record_count)
                    .map_err(|_| local_error(SyncCyclePhase::Upload))?,
            },
            manifest,
            attempt_count: u32::try_from(self.attempt_count)
                .map_err(|_| local_error(SyncCyclePhase::Upload))?,
        };
        validate_prepared_outbox(&prepared)?;
        Ok(prepared)
    }
}

fn to_userdb_object_type(
    object_type: SyncObjectType,
) -> Result<UserDbSyncPayloadObjectType, SyncOrchestrationError> {
    match object_type {
        SyncObjectType::DictionaryUserTerms => Ok(UserDbSyncPayloadObjectType::DictionaryUserTerms),
        SyncObjectType::DictionaryDeletedTerms => {
            Ok(UserDbSyncPayloadObjectType::DictionaryDeletedTerms)
        }
        SyncObjectType::RankerWeights => Ok(UserDbSyncPayloadObjectType::RankerWeights),
        _ => Err(decode_error()),
    }
}

fn to_sync_object_type(object_type: UserDbSyncPayloadObjectType) -> SyncObjectType {
    match object_type {
        UserDbSyncPayloadObjectType::DictionaryUserTerms => SyncObjectType::DictionaryUserTerms,
        UserDbSyncPayloadObjectType::DictionaryDeletedTerms => {
            SyncObjectType::DictionaryDeletedTerms
        }
        UserDbSyncPayloadObjectType::RankerWeights => SyncObjectType::RankerWeights,
    }
}

fn parse_sync_object_type(value: &str) -> Result<SyncObjectType, SyncOrchestrationError> {
    match value {
        "dictionary.user_terms" => Ok(SyncObjectType::DictionaryUserTerms),
        "dictionary.deleted_terms" => Ok(SyncObjectType::DictionaryDeletedTerms),
        "ranker.weights" => Ok(SyncObjectType::RankerWeights),
        _ => Err(local_error(SyncCyclePhase::Upload)),
    }
}

fn object_id_for(object_type: SyncObjectType) -> &'static str {
    match object_type {
        SyncObjectType::DictionaryUserTerms => DICTIONARY_USER_TERMS_OBJECT_ID,
        SyncObjectType::DictionaryDeletedTerms => DICTIONARY_DELETED_TERMS_OBJECT_ID,
        SyncObjectType::RankerWeights => RANKER_WEIGHTS_OBJECT_ID,
        _ => unreachable!("userdb only exposes P2 dictionary and ranker objects"),
    }
}

fn validate_required(value: &str, phase: SyncCyclePhase) -> Result<(), SyncOrchestrationError> {
    if value.trim().is_empty() {
        return Err(local_error(phase));
    }
    Ok(())
}

fn decode_error() -> SyncOrchestrationError {
    SyncOrchestrationError::new(
        SyncOrchestrationErrorCode::DecodeFailed,
        SyncCyclePhase::DecryptAndDecode,
        false,
    )
}

fn local_error(phase: SyncCyclePhase) -> SyncOrchestrationError {
    SyncOrchestrationError::new(
        SyncOrchestrationErrorCode::LocalTransactionFailed,
        phase,
        true,
    )
}

fn to_i64(value: u64, phase: SyncCyclePhase) -> Result<i64, SyncOrchestrationError> {
    i64::try_from(value).map_err(|_| local_error(phase))
}

fn optional_to_i64(
    value: Option<u64>,
    phase: SyncCyclePhase,
) -> Result<Option<i64>, SyncOrchestrationError> {
    value.map(|value| to_i64(value, phase)).transpose()
}

fn usize_to_i64(value: usize, phase: SyncCyclePhase) -> Result<i64, SyncOrchestrationError> {
    i64::try_from(value).map_err(|_| local_error(phase))
}

fn from_i64(value: i64, phase: SyncCyclePhase) -> Result<u64, SyncOrchestrationError> {
    u64::try_from(value).map_err(|_| local_error(phase))
}

#[cfg(test)]
mod tests {
    use radishlex_ime_crypto::{
        DeviceSignature, KeyDescriptor, KeyRole, SignedSyncObjectManifest, SyncMasterKeyMaterial,
        TestMemoryDeviceKeyStore, ED25519_SIGNATURE_LEN,
    };
    use radishlex_ime_sync::{
        DecryptedSyncObject, OpaqueSyncCursor, PreparedSyncOutbox, RemoteObjectVersion,
        SyncEnvelopeAssembler, SyncLocalRepository, SyncObjectAssemblySpec, SyncObjectType,
    };

    use crate::{TermSource, UserDb};

    const DOMAIN_ID: &str = "domain-a";

    #[test]
    fn apply_and_cursor_commit_atomically_and_idempotent_payload_keeps_revision() {
        let mut db = UserDb::open_in_memory().expect("userdb");
        let cursor_1 = OpaqueSyncCursor::new("v1.cursor_1").expect("cursor");
        let object_1 = decrypted_user_terms(1, 1);

        let first = db
            .apply_download_page(DOMAIN_ID, &cursor_1, std::slice::from_ref(&object_1))
            .expect("first page");

        assert_eq!(first.applied_user_terms, 1);
        assert_eq!(
            db.current_cursor(DOMAIN_ID)
                .expect("cursor")
                .expect("stored cursor")
                .as_str(),
            "v1.cursor_1"
        );
        assert!(db.fetch_term("cihe", "词核", "").expect("term").is_some());
        let first_snapshots = db.outbound_snapshots(DOMAIN_ID).expect("snapshots");
        assert_eq!(first_snapshots.len(), 1);
        assert_eq!(first_snapshots[0].local_revision, 1);

        let cursor_2 = OpaqueSyncCursor::new("v1.cursor_2").expect("cursor");
        let mut replay = object_1;
        replay.remote.change_sequence = 2;
        db.apply_download_page(DOMAIN_ID, &cursor_2, &[replay])
            .expect("replay page");
        let replay_snapshots = db.outbound_snapshots(DOMAIN_ID).expect("snapshots");
        assert_eq!(replay_snapshots[0].local_revision, 1);
    }

    #[test]
    fn observation_constraint_failure_rolls_back_payload_and_cursor() {
        let mut db = UserDb::open_in_memory().expect("userdb");
        let cursor = OpaqueSyncCursor::new("v1.cursor_1").expect("cursor");
        let terms = decrypted_user_terms(1, 1);
        let weights = DecryptedSyncObject {
            remote: remote_object("ranker-weights-v1", SyncObjectType::RankerWeights, 1, 1),
            plaintext_payload: r#"{"payload_schema_version":1,"object_type":"ranker.weights","weights":[{"input_code":"cihe","text":"词核","reading":"","frequency":2,"recency_score":30,"negative_score":0.0,"context_kind":"general","updated_at_ms":30}]}"#.as_bytes().to_vec(),
        };

        let error = db
            .apply_download_page(DOMAIN_ID, &cursor, &[terms, weights])
            .expect_err("duplicate change sequence must rollback");

        assert_eq!(
            error.code,
            radishlex_ime_sync::SyncOrchestrationErrorCode::LocalTransactionFailed
        );
        assert!(db.current_cursor(DOMAIN_ID).expect("cursor").is_none());
        assert!(db.fetch_term("cihe", "词核", "").expect("term").is_none());
    }

    #[test]
    fn prepared_outbox_round_trips_and_acknowledgement_clears_only_its_revision() {
        let database_path = temporary_database_path("outbox-restart");
        let mut db = UserDb::open(&database_path).expect("userdb");
        db.add_term("cihe", "词核", None, TermSource::ManualAdd)
            .expect("local term");
        let snapshot = db
            .outbound_snapshots(DOMAIN_ID)
            .expect("snapshots")
            .pop()
            .expect("user terms snapshot");
        let object_key =
            KeyDescriptor::new("object-key-v1", KeyRole::ObjectKey, 1).expect("object key");
        let master_key = SyncMasterKeyMaterial::new([9u8; 32]).expect("master key");
        let spec = SyncObjectAssemblySpec::new(
            &snapshot.object_id,
            "device-a",
            object_key,
            1,
            snapshot.remote_base_version,
            100,
        )
        .expect("assembly spec");
        let object = SyncEnvelopeAssembler::new()
            .assemble_payload(snapshot.payload, spec, &master_key)
            .expect("assembled object");
        let mut signing_store = TestMemoryDeviceKeyStore::new();
        signing_store
            .insert_signing_key("device-a", "signing-key-a", [7u8; 32], 90)
            .expect("signing key");
        let placeholder = DeviceSignature::new(
            "signing-key-a",
            "device-a",
            vec![1u8; ED25519_SIGNATURE_LEN],
        )
        .expect("placeholder");
        let unsigned = SignedSyncObjectManifest::new(DOMAIN_ID, &object.envelope, placeholder)
            .expect("unsigned manifest");
        let handle = signing_store
            .handle("device-a", "signing-key-a")
            .expect("handle");
        let signature = signing_store
            .sign(&handle, &unsigned.canonical_bytes())
            .expect("signature");
        let manifest = SignedSyncObjectManifest::new(DOMAIN_ID, &object.envelope, signature)
            .expect("manifest");
        let prepared = PreparedSyncOutbox {
            domain_id: DOMAIN_ID.to_owned(),
            local_revision: snapshot.local_revision,
            object,
            manifest,
            attempt_count: 0,
        };

        db.store_prepared_outbox(&prepared).expect("store outbox");
        drop(db);
        let mut db = UserDb::open(&database_path).expect("reopen userdb");
        let restored = db.prepared_outboxes(DOMAIN_ID).expect("restore outbox");
        assert_eq!(restored, vec![prepared.clone()]);

        db.acknowledge_outbox(
            DOMAIN_ID,
            &prepared.object.draft.object_id,
            prepared.object.draft.version,
            &prepared.object.draft.ciphertext_hash,
            1,
            120,
        )
        .expect("acknowledge");
        assert!(db
            .prepared_outboxes(DOMAIN_ID)
            .expect("outboxes")
            .is_empty());
        assert!(db
            .outbound_snapshots(DOMAIN_ID)
            .expect("snapshots")
            .is_empty());
        drop(db);
        remove_temporary_database(&database_path);
    }

    fn decrypted_user_terms(version: u64, change_sequence: u64) -> DecryptedSyncObject {
        DecryptedSyncObject {
            remote: remote_object(
                "dictionary-user-terms-v1",
                SyncObjectType::DictionaryUserTerms,
                version,
                change_sequence,
            ),
            plaintext_payload: r#"{"payload_schema_version":1,"object_type":"dictionary.user_terms","terms":[{"input_code":"cihe","text":"词核","reading":"","source":"manual_import","weight":3.5,"status":"active","created_at_ms":20,"updated_at_ms":30,"last_used_at_ms":25}]}"#.as_bytes().to_vec(),
        }
    }

    fn remote_object(
        object_id: &str,
        object_type: SyncObjectType,
        version: u64,
        change_sequence: u64,
    ) -> RemoteObjectVersion {
        RemoteObjectVersion {
            domain_id: DOMAIN_ID.to_owned(),
            object_id: object_id.to_owned(),
            object_type,
            version,
            base_version: version.checked_sub(1).filter(|value| *value > 0),
            change_sequence,
            owner_device_id: "device-a".to_owned(),
            key_id: "object-key-v1".to_owned(),
            key_epoch: 1,
            algorithm: "xchacha20poly1305-hkdf-sha256-v1".to_owned(),
            nonce: vec![3u8; 24],
            encrypted_payload_len: 32,
            ciphertext_hash: format!("hash-{object_id}-{version}"),
            signature_schema_version: 1,
            signature_algorithm: "ed25519-v1".to_owned(),
            signature_key_id: "signing-key-a".to_owned(),
            signature: vec![4u8; 64],
            server_received_at_ms: 40,
            client_created_at_ms: 20,
            client_updated_at_ms: 30,
        }
    }

    fn temporary_database_path(label: &str) -> std::path::PathBuf {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "radishlex-sync-repository-{label}-{}-{timestamp}.sqlite",
            std::process::id()
        ))
    }

    fn remove_temporary_database(path: &std::path::Path) {
        for candidate in [
            path.to_path_buf(),
            std::path::PathBuf::from(format!("{}-wal", path.display())),
            std::path::PathBuf::from(format!("{}-shm", path.display())),
        ] {
            match std::fs::remove_file(&candidate) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("remove {}: {error}", candidate.display()),
            }
        }
    }
}
