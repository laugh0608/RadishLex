use crate::{
    DecryptedSyncObject, LocalSyncSnapshot, PreparedSyncOutbox, SyncCycleOutcome, SyncCyclePhase,
    SyncCycleSummary, SyncLocalRepository, SyncOrchestrationError, SyncOrchestrationErrorCode,
    SyncRemoteClient, SyncRemoteError, SyncRemoteTransport, SyncServerErrorCode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncOnceConfig {
    pub discovery_limit: u16,
    pub max_transport_attempts: u32,
    pub max_conflicts: u32,
    pub lease_duration_ms: i64,
}

impl Default for SyncOnceConfig {
    fn default() -> Self {
        Self {
            discovery_limit: 100,
            max_transport_attempts: 3,
            max_conflicts: 3,
            lease_duration_ms: 60_000,
        }
    }
}

impl SyncOnceConfig {
    fn validate(self) -> Result<Self, SyncOrchestrationError> {
        if self.discovery_limit == 0
            || self.discovery_limit > 200
            || self.max_transport_attempts == 0
            || self.max_conflicts == 0
            || self.lease_duration_ms <= 0
        {
            return Err(SyncOrchestrationError::new(
                SyncOrchestrationErrorCode::InvalidMetadata,
                SyncCyclePhase::Preflight,
                false,
            ));
        }
        Ok(self)
    }
}

pub trait SyncObjectProcessor {
    fn preflight(&mut self, domain_id: &str) -> Result<(), SyncOrchestrationError>;

    fn verify_and_decrypt(
        &mut self,
        expected: &crate::RemoteObjectVersion,
        downloaded: crate::RemoteObjectPayload,
    ) -> Result<DecryptedSyncObject, SyncOrchestrationError>;

    fn prepare_outbox(
        &mut self,
        snapshot: LocalSyncSnapshot,
        version: u64,
        prepared_at_ms: i64,
    ) -> Result<PreparedSyncOutbox, SyncOrchestrationError>;
}

#[derive(Debug)]
pub struct SyncOrchestrationService<T, P> {
    remote: SyncRemoteClient<T>,
    processor: P,
    config: SyncOnceConfig,
}

impl<T: SyncRemoteTransport, P: SyncObjectProcessor> SyncOrchestrationService<T, P> {
    pub fn new(
        remote: SyncRemoteClient<T>,
        processor: P,
        config: SyncOnceConfig,
    ) -> Result<Self, SyncOrchestrationError> {
        Ok(Self {
            remote,
            processor,
            config: config.validate()?,
        })
    }

    pub fn remote(&self) -> &SyncRemoteClient<T> {
        &self.remote
    }

    pub fn processor(&self) -> &P {
        &self.processor
    }

    pub fn sync_once<R: SyncLocalRepository>(
        &mut self,
        repository: &mut R,
        domain_id: &str,
        started_at_ms: i64,
    ) -> SyncCycleSummary {
        let mut summary = empty_summary();
        let lease_expires_at_ms = match started_at_ms.checked_add(self.config.lease_duration_ms) {
            Some(value) => value,
            None => {
                return failed_summary(SyncOrchestrationError::new(
                    SyncOrchestrationErrorCode::InvalidMetadata,
                    SyncCyclePhase::Preflight,
                    false,
                ));
            }
        };
        if let Err(error) = repository.begin_cycle(domain_id, started_at_ms, lease_expires_at_ms) {
            return failed_summary(error);
        }

        let result = self.run_cycle(repository, domain_id, started_at_ms, &mut summary);
        let finish_result = repository.finish_cycle(domain_id);
        match (result, finish_result) {
            (Ok(()), Ok(())) => {
                summary.outcome = SyncCycleOutcome::Completed;
                summary.final_phase = SyncCyclePhase::Complete;
                summary.last_success_at_ms = Some(started_at_ms);
                summary
            }
            (Err(error), _) => finish_with_error(summary, error),
            (Ok(()), Err(error)) => finish_with_error(summary, error),
        }
    }

    fn run_cycle<R: SyncLocalRepository>(
        &mut self,
        repository: &mut R,
        domain_id: &str,
        started_at_ms: i64,
        summary: &mut SyncCycleSummary,
    ) -> Result<(), SyncOrchestrationError> {
        repository.record_cycle_phase(domain_id, SyncCyclePhase::Preflight)?;
        self.processor.preflight(domain_id)?;
        self.check_cancel(repository, domain_id, SyncCyclePhase::Preflight)?;

        self.discover_download_apply(repository, domain_id, summary)?;
        self.prepare_missing_outboxes(repository, domain_id, started_at_ms)?;

        loop {
            let outboxes = repository.prepared_outboxes(domain_id)?;
            if outboxes.is_empty() {
                break;
            }
            let mut restart_after_conflict = false;
            for outbox in outboxes {
                self.check_cancel(repository, domain_id, SyncCyclePhase::Upload)?;
                repository.record_cycle_phase(domain_id, SyncCyclePhase::Upload)?;
                match self.upload_with_retry(repository, &outbox, summary)? {
                    UploadDisposition::Acknowledged => {}
                    UploadDisposition::StaleConflict => {
                        summary.conflicts += 1;
                        if summary.conflicts >= self.config.max_conflicts as usize {
                            return Err(SyncOrchestrationError::new(
                                SyncOrchestrationErrorCode::ConflictRetryExhausted,
                                SyncCyclePhase::Upload,
                                false,
                            ));
                        }
                        self.discover_download_apply(repository, domain_id, summary)?;
                        repository.supersede_outbox(
                            domain_id,
                            &outbox.object.draft.object_id,
                            outbox.object.draft.version,
                            &outbox.object.draft.ciphertext_hash,
                        )?;
                        self.prepare_missing_outboxes(repository, domain_id, started_at_ms)?;
                        restart_after_conflict = true;
                        break;
                    }
                }
            }
            if !restart_after_conflict {
                break;
            }
        }
        repository.record_cycle_phase(domain_id, SyncCyclePhase::Complete)?;
        Ok(())
    }

    fn discover_download_apply<R: SyncLocalRepository>(
        &mut self,
        repository: &mut R,
        domain_id: &str,
        summary: &mut SyncCycleSummary,
    ) -> Result<(), SyncOrchestrationError> {
        let mut cursor = repository.current_cursor(domain_id)?;
        loop {
            self.check_cancel(repository, domain_id, SyncCyclePhase::Discover)?;
            repository.record_cycle_phase(domain_id, SyncCyclePhase::Discover)?;
            let page = self
                .remote
                .discover_object_versions(domain_id, cursor.as_ref(), self.config.discovery_limit)
                .map_err(|error| map_remote_error(error, SyncCyclePhase::Discover))?;
            if page.has_more && cursor.as_ref() == Some(&page.next_cursor) {
                return Err(SyncOrchestrationError::new(
                    SyncOrchestrationErrorCode::CursorInvalid,
                    SyncCyclePhase::Discover,
                    false,
                ));
            }
            summary.discovered += page.entries.len();

            let mut decrypted = Vec::with_capacity(page.entries.len());
            for expected in &page.entries {
                repository.record_cycle_phase(domain_id, SyncCyclePhase::Download)?;
                let downloaded = self
                    .remote
                    .object_payload(domain_id, &expected.object_id, expected.version)
                    .map_err(|error| map_remote_error(error, SyncCyclePhase::Download))?;
                if downloaded.object != *expected {
                    return Err(SyncOrchestrationError::new(
                        SyncOrchestrationErrorCode::InvalidMetadata,
                        SyncCyclePhase::Verify,
                        false,
                    ));
                }
                summary.downloaded += 1;
                repository.record_cycle_phase(domain_id, SyncCyclePhase::Verify)?;
                decrypted.push(self.processor.verify_and_decrypt(expected, downloaded)?);
            }
            repository.record_cycle_phase(domain_id, SyncCyclePhase::DecryptAndDecode)?;
            repository.record_cycle_phase(domain_id, SyncCyclePhase::ApplyAndAdvanceCursor)?;
            let applied =
                repository.apply_download_page(domain_id, &page.next_cursor, &decrypted)?;
            summary.applied += applied.applied_records();
            cursor = Some(page.next_cursor);
            if !page.has_more {
                return Ok(());
            }
        }
    }

    fn prepare_missing_outboxes<R: SyncLocalRepository>(
        &mut self,
        repository: &mut R,
        domain_id: &str,
        prepared_at_ms: i64,
    ) -> Result<(), SyncOrchestrationError> {
        repository.record_cycle_phase(domain_id, SyncCyclePhase::PlanUpload)?;
        let existing = repository.prepared_outboxes(domain_id)?;
        let snapshots = repository.outbound_snapshots(domain_id)?;
        for snapshot in snapshots {
            if existing
                .iter()
                .any(|outbox| outbox.object.draft.object_id == snapshot.object_id)
            {
                continue;
            }
            let version = snapshot
                .remote_base_version
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| {
                    SyncOrchestrationError::new(
                        SyncOrchestrationErrorCode::InvalidMetadata,
                        SyncCyclePhase::PrepareSignedOutbox,
                        false,
                    )
                })?;
            repository.record_cycle_phase(domain_id, SyncCyclePhase::PrepareSignedOutbox)?;
            let prepared = self
                .processor
                .prepare_outbox(snapshot, version, prepared_at_ms)?;
            repository.store_prepared_outbox(&prepared)?;
        }
        Ok(())
    }

    fn upload_with_retry<R: SyncLocalRepository>(
        &self,
        repository: &mut R,
        outbox: &PreparedSyncOutbox,
        summary: &mut SyncCycleSummary,
    ) -> Result<UploadDisposition, SyncOrchestrationError> {
        let mut attempts = 0;
        while attempts < self.config.max_transport_attempts {
            let result = self.remote.upload_object_version(
                &outbox.domain_id,
                &outbox.object,
                &outbox.manifest,
            );
            attempts += 1;
            match result {
                Ok(remote) => {
                    repository.record_outbox_attempt(
                        &outbox.domain_id,
                        &outbox.object.draft.object_id,
                        outbox.object.draft.version,
                        None,
                    )?;
                    repository
                        .record_cycle_phase(&outbox.domain_id, SyncCyclePhase::AcknowledgeOutbox)?;
                    repository.acknowledge_outbox(
                        &outbox.domain_id,
                        &remote.object_id,
                        remote.version,
                        &remote.ciphertext_hash,
                        remote.change_sequence,
                        remote.server_received_at_ms,
                    )?;
                    summary.uploaded += 1;
                    return Ok(UploadDisposition::Acknowledged);
                }
                Err(error) if is_stale_conflict(&error) => {
                    repository.record_outbox_attempt(
                        &outbox.domain_id,
                        &outbox.object.draft.object_id,
                        outbox.object.draft.version,
                        None,
                    )?;
                    return Ok(UploadDisposition::StaleConflict);
                }
                Err(error) => {
                    let mapped = map_remote_error(error, SyncCyclePhase::Upload);
                    repository.record_outbox_attempt(
                        &outbox.domain_id,
                        &outbox.object.draft.object_id,
                        outbox.object.draft.version,
                        Some(mapped.code),
                    )?;
                    if !mapped.retryable || attempts >= self.config.max_transport_attempts {
                        return Err(mapped);
                    }
                    summary.retries += 1;
                }
            }
        }
        Err(SyncOrchestrationError::new(
            SyncOrchestrationErrorCode::TransportTimeout,
            SyncCyclePhase::Upload,
            true,
        ))
    }

    fn check_cancel<R: SyncLocalRepository>(
        &self,
        repository: &R,
        domain_id: &str,
        phase: SyncCyclePhase,
    ) -> Result<(), SyncOrchestrationError> {
        if repository.cycle_cancel_requested(domain_id)? {
            return Err(SyncOrchestrationError::new(
                SyncOrchestrationErrorCode::Cancelled,
                phase,
                false,
            ));
        }
        Ok(())
    }
}

enum UploadDisposition {
    Acknowledged,
    StaleConflict,
}

fn is_stale_conflict(error: &SyncRemoteError) -> bool {
    matches!(
        error,
        SyncRemoteError::Server {
            code: SyncServerErrorCode::ConflictStaleBaseVersion,
            ..
        }
    )
}

fn map_remote_error(error: SyncRemoteError, phase: SyncCyclePhase) -> SyncOrchestrationError {
    let (code, retryable) = match error {
        SyncRemoteError::Transport { .. } => (SyncOrchestrationErrorCode::TransportTimeout, true),
        SyncRemoteError::InvalidRequest { .. } | SyncRemoteError::InvalidResponse { .. } => {
            (SyncOrchestrationErrorCode::InvalidMetadata, false)
        }
        SyncRemoteError::Server {
            code: SyncServerErrorCode::Unauthenticated,
            ..
        } => (SyncOrchestrationErrorCode::Unauthenticated, false),
        SyncRemoteError::Server {
            code: SyncServerErrorCode::ForbiddenDevice,
            ..
        } => (SyncOrchestrationErrorCode::RevokedDevice, false),
        SyncRemoteError::Server {
            code: SyncServerErrorCode::StorageUnavailable,
            retryable,
            ..
        } => (SyncOrchestrationErrorCode::ServerUnavailable, retryable),
        SyncRemoteError::Server { retryable, .. } => {
            (SyncOrchestrationErrorCode::InvalidMetadata, retryable)
        }
    };
    SyncOrchestrationError::new(code, phase, retryable)
}

fn empty_summary() -> SyncCycleSummary {
    SyncCycleSummary {
        outcome: SyncCycleOutcome::Failed,
        final_phase: SyncCyclePhase::Preflight,
        discovered: 0,
        downloaded: 0,
        applied: 0,
        uploaded: 0,
        conflicts: 0,
        retries: 0,
        last_success_at_ms: None,
        error: None,
    }
}

fn failed_summary(error: SyncOrchestrationError) -> SyncCycleSummary {
    finish_with_error(empty_summary(), error)
}

fn finish_with_error(
    mut summary: SyncCycleSummary,
    error: SyncOrchestrationError,
) -> SyncCycleSummary {
    summary.final_phase = error.phase;
    summary.outcome = if error.code == SyncOrchestrationErrorCode::Cancelled {
        SyncCycleOutcome::Cancelled
    } else {
        SyncCycleOutcome::Failed
    };
    summary.error = Some(error);
    summary
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use base64ct::{Base64, Encoding};
    use radishlex_ime_crypto::{
        AlgorithmId, CiphertextHash, DeviceSignature, DeviceSigningPublicKey,
        EncryptedObjectEnvelope, KeyDescriptor, KeyRole, Nonce, SignatureAlgorithmId,
        SignedSyncObjectManifest, SyncMasterKeyMaterial, TestMemoryDeviceKeyStore,
        ED25519_SIGNATURE_LEN, ENVELOPE_SCHEMA_VERSION,
    };

    use super::*;
    use crate::{
        AssembledSyncObject, OpaqueSyncCursor, PlaintextSyncPayload, RemoteObjectPayload,
        RemoteObjectVersion, SyncApplyPageSummary, SyncEnvelopeAssembler, SyncObjectAssemblySpec,
        SyncObjectType, SyncRemoteMethod, SyncRemoteRequest, SyncRemoteResponse,
    };

    #[test]
    fn stale_conflict_rediscovers_merges_and_prepares_a_new_signed_version() {
        let external = prepared_object(1, None, 9, "remote", 200);
        let first_local = prepared_object(1, None, 1, "local-v1", 100);
        let rebased = prepared_object(2, Some(1), 2, "local-v2", 300);
        let transport = ScriptedTransport::new([
            json_response(
                200,
                serde_json::json!({
                    "entries": [],
                    "next_cursor": "v1.cursor_0",
                    "has_more": false
                }),
            ),
            json_response(
                409,
                serde_json::json!({
                    "error_code": "conflict_stale_base_version",
                    "message": "stale",
                    "retryable": false,
                    "server_time_ms": 220,
                    "latest_version": 1,
                    "latest_ciphertext_hash": external.object.draft.ciphertext_hash
                }),
            ),
            json_response(
                200,
                serde_json::json!({
                    "entries": [response_for_prepared(&external, 1)],
                    "next_cursor": "v1.cursor_1",
                    "has_more": false
                }),
            ),
            json_response(200, response_for_prepared(&external, 1)),
            Ok(SyncRemoteResponse::new(
                200,
                Some("application/octet-stream".to_owned()),
                external.object.envelope.encrypted_payload.clone(),
            )),
            json_response(201, response_for_prepared(&rebased, 2)),
        ]);
        let processor = QueueProcessor::new([first_local, rebased]);
        let remote = SyncRemoteClient::new(transport);
        let mut service =
            SyncOrchestrationService::new(remote, processor, SyncOnceConfig::default())
                .expect("service");
        let mut repository = FakeRepository::new(local_snapshot(1, None, "local-v1"));

        let summary = service.sync_once(&mut repository, "domain-a", 100);

        assert_eq!(summary.outcome, SyncCycleOutcome::Completed);
        assert_eq!(summary.conflicts, 1);
        assert_eq!(summary.discovered, 1);
        assert_eq!(summary.downloaded, 1);
        assert_eq!(summary.applied, 1);
        assert_eq!(summary.uploaded, 1);
        assert!(repository.outboxes.is_empty());
        assert_eq!(repository.acknowledged_revision, 2);
        assert_eq!(
            repository.cursor.as_ref().expect("cursor").as_str(),
            "v1.cursor_1"
        );

        let requests = service.remote().transport().requests();
        let upload_bodies = requests
            .iter()
            .filter(|request| request.method() == SyncRemoteMethod::Post)
            .map(|request| {
                serde_json::from_slice::<serde_json::Value>(request.body()).expect("json")
            })
            .collect::<Vec<_>>();
        assert_eq!(upload_bodies.len(), 2);
        assert_eq!(upload_bodies[0]["version"], 1);
        assert_eq!(upload_bodies[0]["base_version"], 0);
        assert_eq!(upload_bodies[1]["version"], 2);
        assert_eq!(upload_bodies[1]["base_version"], 1);
        assert_ne!(
            upload_bodies[0]["ciphertext_hash"],
            upload_bodies[1]["ciphertext_hash"]
        );
    }

    #[test]
    fn cancellation_before_discovery_finishes_lease_without_network_or_outbox() {
        let transport = ScriptedTransport::new([]);
        let processor = QueueProcessor::new([]);
        let remote = SyncRemoteClient::new(transport);
        let mut service =
            SyncOrchestrationService::new(remote, processor, SyncOnceConfig::default())
                .expect("service");
        let mut repository = FakeRepository::new(local_snapshot(1, None, "local-v1"));
        repository.cancel_requested = true;

        let summary = service.sync_once(&mut repository, "domain-a", 100);

        assert_eq!(summary.outcome, SyncCycleOutcome::Cancelled);
        assert_eq!(summary.final_phase, SyncCyclePhase::Preflight);
        assert_eq!(
            summary.error.expect("cancel error").code,
            SyncOrchestrationErrorCode::Cancelled
        );
        assert!(!repository.journal_active);
        assert!(repository.outboxes.is_empty());
        assert!(service.remote().transport().requests().is_empty());
    }

    #[test]
    fn retry_exhaustion_preserves_identical_outbox_for_the_next_cycle() {
        let prepared = prepared_object(1, None, 1, "local-v1", 100);
        let first_transport = ScriptedTransport::new([
            empty_discovery_response("v1.cursor_0"),
            Err(SyncRemoteError::transport("timeout-1")),
            Err(SyncRemoteError::transport("timeout-2")),
            Err(SyncRemoteError::transport("timeout-3")),
        ]);
        let first_processor = QueueProcessor::new([prepared.clone()]);
        let first_remote = SyncRemoteClient::new(first_transport);
        let mut first_service =
            SyncOrchestrationService::new(first_remote, first_processor, SyncOnceConfig::default())
                .expect("service");
        let mut repository = FakeRepository::new(local_snapshot(1, None, "local-v1"));

        let failed = first_service.sync_once(&mut repository, "domain-a", 100);

        assert_eq!(failed.outcome, SyncCycleOutcome::Failed);
        assert_eq!(failed.retries, 2);
        assert_eq!(
            failed.error.expect("retry error").code,
            SyncOrchestrationErrorCode::TransportTimeout
        );
        assert_eq!(repository.outboxes.len(), 1);
        assert_eq!(repository.outboxes[0].attempt_count, 3);
        assert_eq!(repository.outboxes[0].object, prepared.object);
        assert_eq!(repository.outboxes[0].manifest, prepared.manifest);

        let retry_transport = ScriptedTransport::new([
            empty_discovery_response("v1.cursor_0"),
            json_response(201, response_for_prepared(&prepared, 1)),
        ]);
        let retry_processor = QueueProcessor::new([]);
        let retry_remote = SyncRemoteClient::new(retry_transport);
        let mut retry_service =
            SyncOrchestrationService::new(retry_remote, retry_processor, SyncOnceConfig::default())
                .expect("retry service");

        let completed = retry_service.sync_once(&mut repository, "domain-a", 200);

        assert_eq!(completed.outcome, SyncCycleOutcome::Completed);
        assert_eq!(completed.uploaded, 1);
        assert!(repository.outboxes.is_empty());
        let retry_requests = retry_service.remote().transport().requests();
        let upload = retry_requests
            .iter()
            .find(|request| request.method() == SyncRemoteMethod::Post)
            .expect("retry upload");
        let retry_body: serde_json::Value =
            serde_json::from_slice(upload.body()).expect("retry body");
        assert_eq!(
            retry_body["ciphertext_hash"],
            prepared.object.draft.ciphertext_hash
        );
        assert_eq!(
            retry_body["payload"],
            Base64::encode_string(&prepared.object.envelope.encrypted_payload)
        );
    }

    #[test]
    fn epoch_and_revocation_verification_failures_do_not_advance_cursor_or_prepare_outbox() {
        for code in [
            SyncOrchestrationErrorCode::KeyEpochRejected,
            SyncOrchestrationErrorCode::RevokedDevice,
        ] {
            let remote_object = prepared_object(1, None, 9, "remote", 200);
            let transport = ScriptedTransport::new([
                json_response(
                    200,
                    serde_json::json!({
                        "entries": [response_for_prepared(&remote_object, 1)],
                        "next_cursor": "v1.cursor_1",
                        "has_more": false
                    }),
                ),
                json_response(200, response_for_prepared(&remote_object, 1)),
                Ok(SyncRemoteResponse::new(
                    200,
                    Some("application/octet-stream".to_owned()),
                    remote_object.object.envelope.encrypted_payload.clone(),
                )),
            ]);
            let processor = QueueProcessor::new([]).reject_verification(code);
            let remote = SyncRemoteClient::new(transport);
            let mut service =
                SyncOrchestrationService::new(remote, processor, SyncOnceConfig::default())
                    .expect("service");
            let mut repository = FakeRepository::new(local_snapshot(1, None, "local-v1"));

            let summary = service.sync_once(&mut repository, "domain-a", 100);

            assert_eq!(summary.outcome, SyncCycleOutcome::Failed);
            assert_eq!(summary.final_phase, SyncCyclePhase::Verify);
            assert_eq!(summary.discovered, 1);
            assert_eq!(summary.downloaded, 1);
            assert_eq!(summary.applied, 0);
            assert_eq!(summary.error.expect("verification error").code, code);
            assert!(repository.cursor.is_none());
            assert!(repository.outboxes.is_empty());
            assert!(!repository.journal_active);
        }
    }

    struct ScriptedTransport {
        responses: RefCell<VecDeque<Result<SyncRemoteResponse, SyncRemoteError>>>,
        requests: RefCell<Vec<SyncRemoteRequest>>,
    }

    impl ScriptedTransport {
        fn new(
            responses: impl IntoIterator<Item = Result<SyncRemoteResponse, SyncRemoteError>>,
        ) -> Self {
            Self {
                responses: RefCell::new(responses.into_iter().collect()),
                requests: RefCell::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<SyncRemoteRequest> {
            self.requests.borrow().clone()
        }
    }

    impl SyncRemoteTransport for ScriptedTransport {
        fn send(&self, request: SyncRemoteRequest) -> Result<SyncRemoteResponse, SyncRemoteError> {
            self.requests.borrow_mut().push(request);
            self.responses
                .borrow_mut()
                .pop_front()
                .expect("scripted response")
        }
    }

    struct QueueProcessor {
        prepared: VecDeque<PreparedSyncOutbox>,
        verification_error: Option<SyncOrchestrationError>,
        master_key: SyncMasterKeyMaterial,
        object_key: KeyDescriptor,
        public_key: DeviceSigningPublicKey,
    }

    impl QueueProcessor {
        fn new(prepared: impl IntoIterator<Item = PreparedSyncOutbox>) -> Self {
            let mut store = TestMemoryDeviceKeyStore::new();
            let public_key = store
                .insert_signing_key("device-a", "signing-key-a", [8u8; 32], 50)
                .expect("public key");
            Self {
                prepared: prepared.into_iter().collect(),
                verification_error: None,
                master_key: SyncMasterKeyMaterial::new([11u8; 32]).expect("master key"),
                object_key: KeyDescriptor::new("object-key-v1", KeyRole::ObjectKey, 1)
                    .expect("object key"),
                public_key,
            }
        }

        fn reject_verification(mut self, code: SyncOrchestrationErrorCode) -> Self {
            self.verification_error = Some(SyncOrchestrationError::new(
                code,
                SyncCyclePhase::Verify,
                false,
            ));
            self
        }
    }

    impl SyncObjectProcessor for QueueProcessor {
        fn preflight(&mut self, _domain_id: &str) -> Result<(), SyncOrchestrationError> {
            Ok(())
        }

        fn verify_and_decrypt(
            &mut self,
            expected: &RemoteObjectVersion,
            downloaded: RemoteObjectPayload,
        ) -> Result<DecryptedSyncObject, SyncOrchestrationError> {
            if let Some(error) = &self.verification_error {
                return Err(error.clone());
            }
            assert_eq!(&downloaded.object, expected);
            let envelope = EncryptedObjectEnvelope {
                schema_version: ENVELOPE_SCHEMA_VERSION,
                object_id: downloaded.object.object_id.clone(),
                object_type: downloaded.object.object_type.to_crypto_object_type(),
                owner_device_id: downloaded.object.owner_device_id.clone(),
                key_id: downloaded.object.key_id.clone(),
                key_epoch: downloaded.object.key_epoch,
                algorithm: AlgorithmId::new(&downloaded.object.algorithm).expect("algorithm"),
                nonce: Nonce::new(downloaded.object.nonce.clone()).expect("nonce"),
                version: downloaded.object.version,
                base_version: downloaded.object.base_version,
                encrypted_payload: downloaded.payload,
                ciphertext_hash: CiphertextHash::new(&downloaded.object.ciphertext_hash)
                    .expect("hash"),
                created_at_ms: downloaded.object.client_created_at_ms,
                updated_at_ms: downloaded.object.client_updated_at_ms,
            };
            let signature = DeviceSignature::new_for_algorithm(
                SignatureAlgorithmId::new(&downloaded.object.signature_algorithm)
                    .expect("signature algorithm"),
                &downloaded.object.signature_key_id,
                &downloaded.object.owner_device_id,
                downloaded.object.signature.clone(),
            )
            .expect("device signature");
            let manifest =
                SignedSyncObjectManifest::new(&downloaded.object.domain_id, &envelope, signature)
                    .expect("manifest");
            manifest
                .verify(&self.public_key)
                .expect("signature verifies");
            let object_key = self
                .master_key
                .derive_object_key(&self.object_key, envelope.object_type, &envelope.object_id)
                .expect("derived object key");
            let plaintext = envelope.decrypt_payload(&object_key).expect("decrypt");
            Ok(DecryptedSyncObject {
                remote: downloaded.object,
                plaintext_payload: plaintext.bytes,
            })
        }

        fn prepare_outbox(
            &mut self,
            snapshot: LocalSyncSnapshot,
            version: u64,
            _prepared_at_ms: i64,
        ) -> Result<PreparedSyncOutbox, SyncOrchestrationError> {
            let prepared = self.prepared.pop_front().expect("prepared object");
            assert_eq!(prepared.local_revision, snapshot.local_revision);
            assert_eq!(prepared.object.draft.version, version);
            assert_eq!(
                prepared.object.draft.base_version,
                snapshot.remote_base_version
            );
            Ok(prepared)
        }
    }

    struct FakeRepository {
        cursor: Option<OpaqueSyncCursor>,
        snapshot: LocalSyncSnapshot,
        acknowledged_revision: u64,
        outboxes: Vec<PreparedSyncOutbox>,
        journal_active: bool,
        cancel_requested: bool,
    }

    impl FakeRepository {
        fn new(snapshot: LocalSyncSnapshot) -> Self {
            Self {
                cursor: None,
                snapshot,
                acknowledged_revision: 0,
                outboxes: Vec::new(),
                journal_active: false,
                cancel_requested: false,
            }
        }
    }

    impl SyncLocalRepository for FakeRepository {
        fn begin_cycle(
            &mut self,
            _domain_id: &str,
            _started_at_ms: i64,
            _lease_expires_at_ms: i64,
        ) -> Result<(), SyncOrchestrationError> {
            assert!(!self.journal_active);
            self.journal_active = true;
            Ok(())
        }

        fn record_cycle_phase(
            &mut self,
            _domain_id: &str,
            _phase: SyncCyclePhase,
        ) -> Result<(), SyncOrchestrationError> {
            assert!(self.journal_active);
            Ok(())
        }

        fn cycle_cancel_requested(&self, _domain_id: &str) -> Result<bool, SyncOrchestrationError> {
            Ok(self.cancel_requested)
        }

        fn finish_cycle(&mut self, _domain_id: &str) -> Result<(), SyncOrchestrationError> {
            self.journal_active = false;
            Ok(())
        }

        fn current_cursor(
            &self,
            _domain_id: &str,
        ) -> Result<Option<OpaqueSyncCursor>, SyncOrchestrationError> {
            Ok(self.cursor.clone())
        }

        fn apply_download_page(
            &mut self,
            _domain_id: &str,
            next_cursor: &OpaqueSyncCursor,
            objects: &[DecryptedSyncObject],
        ) -> Result<SyncApplyPageSummary, SyncOrchestrationError> {
            self.cursor = Some(next_cursor.clone());
            if let Some(latest) = objects.last() {
                self.snapshot.remote_base_version = Some(latest.remote.version);
                self.snapshot.local_revision += 1;
            }
            Ok(SyncApplyPageSummary {
                discovered: objects.len(),
                applied_user_terms: objects.len(),
                applied_deleted_terms: 0,
                applied_ranker_weights: 0,
            })
        }

        fn outbound_snapshots(
            &mut self,
            _domain_id: &str,
        ) -> Result<Vec<LocalSyncSnapshot>, SyncOrchestrationError> {
            if self.acknowledged_revision == self.snapshot.local_revision {
                Ok(Vec::new())
            } else {
                Ok(vec![self.snapshot.clone()])
            }
        }

        fn prepared_outboxes(
            &self,
            _domain_id: &str,
        ) -> Result<Vec<PreparedSyncOutbox>, SyncOrchestrationError> {
            Ok(self.outboxes.clone())
        }

        fn store_prepared_outbox(
            &mut self,
            outbox: &PreparedSyncOutbox,
        ) -> Result<(), SyncOrchestrationError> {
            self.outboxes.push(outbox.clone());
            Ok(())
        }

        fn record_outbox_attempt(
            &mut self,
            _domain_id: &str,
            object_id: &str,
            version: u64,
            _error_code: Option<SyncOrchestrationErrorCode>,
        ) -> Result<(), SyncOrchestrationError> {
            let outbox = self
                .outboxes
                .iter_mut()
                .find(|outbox| {
                    outbox.object.draft.object_id == object_id
                        && outbox.object.draft.version == version
                })
                .expect("outbox attempt");
            outbox.attempt_count += 1;
            Ok(())
        }

        fn supersede_outbox(
            &mut self,
            _domain_id: &str,
            object_id: &str,
            version: u64,
            ciphertext_hash: &str,
        ) -> Result<(), SyncOrchestrationError> {
            self.outboxes.retain(|outbox| {
                !(outbox.object.draft.object_id == object_id
                    && outbox.object.draft.version == version
                    && outbox.object.draft.ciphertext_hash == ciphertext_hash)
            });
            Ok(())
        }

        fn acknowledge_outbox(
            &mut self,
            _domain_id: &str,
            object_id: &str,
            version: u64,
            _ciphertext_hash: &str,
            _change_sequence: u64,
            _acknowledged_at_ms: i64,
        ) -> Result<(), SyncOrchestrationError> {
            let outbox = self
                .outboxes
                .iter()
                .find(|outbox| {
                    outbox.object.draft.object_id == object_id
                        && outbox.object.draft.version == version
                })
                .expect("ack outbox");
            self.acknowledged_revision = outbox.local_revision;
            self.outboxes.retain(|outbox| {
                !(outbox.object.draft.object_id == object_id
                    && outbox.object.draft.version == version)
            });
            Ok(())
        }
    }

    fn local_snapshot(
        local_revision: u64,
        remote_base_version: Option<u64>,
        text: &str,
    ) -> LocalSyncSnapshot {
        LocalSyncSnapshot {
            domain_id: "domain-a".to_owned(),
            object_id: "object-a".to_owned(),
            object_type: SyncObjectType::DictionaryUserTerms,
            local_revision,
            remote_base_version,
            payload: PlaintextSyncPayload::new(
                SyncObjectType::DictionaryUserTerms,
                1,
                format!("synthetic-{text}").into_bytes(),
            )
            .expect("payload"),
        }
    }

    fn prepared_object(
        version: u64,
        base_version: Option<u64>,
        local_revision: u64,
        text: &str,
        timestamp_ms: i64,
    ) -> PreparedSyncOutbox {
        let key = KeyDescriptor::new("object-key-v1", KeyRole::ObjectKey, 1).expect("key");
        let master = SyncMasterKeyMaterial::new([11u8; 32]).expect("master");
        let payload = PlaintextSyncPayload::new(
            SyncObjectType::DictionaryUserTerms,
            1,
            format!("synthetic-{text}").into_bytes(),
        )
        .expect("payload");
        let spec = SyncObjectAssemblySpec::new(
            "object-a",
            "device-a",
            key,
            version,
            base_version,
            timestamp_ms,
        )
        .expect("spec");
        let object = SyncEnvelopeAssembler::new()
            .assemble_payload(payload, spec, &master)
            .expect("object");
        let manifest = signed_manifest(&object);
        PreparedSyncOutbox {
            domain_id: "domain-a".to_owned(),
            local_revision,
            object,
            manifest,
            attempt_count: 0,
        }
    }

    fn signed_manifest(object: &AssembledSyncObject) -> SignedSyncObjectManifest {
        let mut store = TestMemoryDeviceKeyStore::new();
        store
            .insert_signing_key("device-a", "signing-key-a", [8u8; 32], 50)
            .expect("key");
        let placeholder = DeviceSignature::new(
            "signing-key-a",
            "device-a",
            vec![1u8; ED25519_SIGNATURE_LEN],
        )
        .expect("placeholder");
        let unsigned = SignedSyncObjectManifest::new("domain-a", &object.envelope, placeholder)
            .expect("unsigned");
        let handle = store.handle("device-a", "signing-key-a").expect("handle");
        let signature = store
            .sign(&handle, &unsigned.canonical_bytes())
            .expect("signature");
        SignedSyncObjectManifest::new("domain-a", &object.envelope, signature).expect("manifest")
    }

    fn json_response(
        status: u16,
        value: serde_json::Value,
    ) -> Result<SyncRemoteResponse, SyncRemoteError> {
        SyncRemoteResponse::json(status, &value)
    }

    fn empty_discovery_response(next_cursor: &str) -> Result<SyncRemoteResponse, SyncRemoteError> {
        json_response(
            200,
            serde_json::json!({
                "entries": [],
                "next_cursor": next_cursor,
                "has_more": false
            }),
        )
    }

    fn response_for_prepared(
        outbox: &PreparedSyncOutbox,
        change_sequence: u64,
    ) -> serde_json::Value {
        let draft = &outbox.object.draft;
        let signature = &outbox.manifest.signature;
        serde_json::json!({
            "domain_id": outbox.domain_id,
            "object_id": draft.object_id,
            "object_type": draft.object_type.as_str(),
            "version": draft.version,
            "base_version": draft.base_version.unwrap_or(0),
            "change_sequence": change_sequence,
            "owner_device_id": draft.owner_device_id,
            "key_id": draft.key_id,
            "key_epoch": draft.key_epoch,
            "algorithm": draft.algorithm,
            "nonce": Base64::encode_string(&draft.nonce),
            "encrypted_payload_len": draft.encrypted_payload_len,
            "ciphertext_hash": draft.ciphertext_hash,
            "signature_schema_version": signature.signature_schema_version,
            "signature_algorithm": signature.signature_algorithm.as_str(),
            "signature_key_id": signature.signature_key_id,
            "signature": Base64::encode_string(&signature.signature),
            "server_received_at_ms": 400,
            "client_created_at_ms": draft.created_at_ms,
            "client_updated_at_ms": draft.updated_at_ms
        })
    }
}
