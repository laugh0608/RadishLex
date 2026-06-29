-- RadishLex sync server metadata schema.
-- This schema stores only encrypted object metadata, device public keys, signatures,
-- recovery metadata, blob references and non-sensitive audit summaries.

CREATE TABLE sync_domains (
    domain_id TEXT PRIMARY KEY,
    current_key_epoch INTEGER NOT NULL CHECK (current_key_epoch > 0),
    active_key_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms)
);

CREATE TABLE devices (
    domain_id TEXT NOT NULL REFERENCES sync_domains(domain_id),
    device_id TEXT NOT NULL,
    signing_public_key_id TEXT NOT NULL,
    signing_public_key BLOB NOT NULL,
    key_agreement_public_key_id TEXT NOT NULL,
    key_agreement_public_key BLOB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'revoked', 'lost')),
    authorized_at_ms INTEGER NOT NULL DEFAULT 0,
    revoked_at_ms INTEGER NOT NULL DEFAULT 0,
    last_seen_at_ms INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (domain_id, device_id)
);

CREATE TABLE device_join_requests (
    domain_id TEXT NOT NULL REFERENCES sync_domains(domain_id),
    join_request_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    signing_public_key_id TEXT NOT NULL,
    signing_public_key BLOB NOT NULL,
    key_agreement_public_key_id TEXT NOT NULL,
    key_agreement_public_key BLOB NOT NULL,
    challenge BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > created_at_ms),
    status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'revoked', 'lost')),
    PRIMARY KEY (domain_id, join_request_id)
);

CREATE TABLE device_authorizations (
    domain_id TEXT NOT NULL REFERENCES sync_domains(domain_id),
    join_request_id TEXT NOT NULL,
    authorizer_device_id TEXT NOT NULL,
    recipient_device_id TEXT NOT NULL,
    recipient_signing_public_key_id TEXT NOT NULL,
    recipient_key_agreement_key_id TEXT NOT NULL,
    join_short_code TEXT NOT NULL,
    key_epoch INTEGER NOT NULL CHECK (key_epoch > 0),
    created_at_ms INTEGER NOT NULL,
    signature_schema_version INTEGER NOT NULL CHECK (signature_schema_version = 1),
    signature_algorithm TEXT NOT NULL,
    signature_key_id TEXT NOT NULL,
    signature BLOB NOT NULL,
    PRIMARY KEY (domain_id, join_request_id)
);

CREATE TABLE device_wrapping_records (
    domain_id TEXT NOT NULL REFERENCES sync_domains(domain_id),
    recipient_device_id TEXT NOT NULL,
    authorizer_device_id TEXT NOT NULL,
    key_epoch INTEGER NOT NULL CHECK (key_epoch > 0),
    wrapping_key_id TEXT NOT NULL,
    algorithm TEXT NOT NULL,
    nonce BLOB NOT NULL,
    wrapped_key_len INTEGER NOT NULL CHECK (wrapped_key_len > 0),
    ciphertext_hash TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    signature BLOB NOT NULL,
    PRIMARY KEY (domain_id, recipient_device_id, key_epoch, wrapping_key_id)
);

CREATE TABLE device_revocations (
    domain_id TEXT NOT NULL REFERENCES sync_domains(domain_id),
    revoked_device_id TEXT NOT NULL,
    revoker_device_id TEXT NOT NULL,
    previous_key_epoch INTEGER NOT NULL CHECK (previous_key_epoch > 0),
    new_key_epoch INTEGER NOT NULL CHECK (new_key_epoch > previous_key_epoch),
    reason TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    signature_schema_version INTEGER NOT NULL CHECK (signature_schema_version = 1),
    signature_algorithm TEXT NOT NULL,
    signature_key_id TEXT NOT NULL,
    signature BLOB NOT NULL,
    PRIMARY KEY (domain_id, revoked_device_id, new_key_epoch)
);

CREATE TABLE recovery_records (
    domain_id TEXT NOT NULL REFERENCES sync_domains(domain_id),
    recovery_record_id TEXT NOT NULL,
    key_epoch INTEGER NOT NULL CHECK (key_epoch > 0),
    kdf_profile TEXT NOT NULL,
    kdf_version INTEGER NOT NULL CHECK (kdf_version > 0),
    memory_kib INTEGER NOT NULL CHECK (memory_kib > 0),
    iterations INTEGER NOT NULL CHECK (iterations > 0),
    parallelism INTEGER NOT NULL CHECK (parallelism > 0),
    output_len INTEGER NOT NULL CHECK (output_len > 0),
    salt BLOB NOT NULL,
    algorithm TEXT NOT NULL,
    nonce BLOB NOT NULL,
    wrapped_material_len INTEGER NOT NULL CHECK (wrapped_material_len > 0),
    ciphertext_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'revoked')),
    created_at_ms INTEGER NOT NULL,
    revoked_at_ms INTEGER NOT NULL DEFAULT 0,
    signer_device_id TEXT NOT NULL,
    signature_schema_version INTEGER NOT NULL CHECK (signature_schema_version = 1),
    signature_algorithm TEXT NOT NULL,
    signature_key_id TEXT NOT NULL,
    signature BLOB NOT NULL,
    blob_ref TEXT NOT NULL,
    PRIMARY KEY (domain_id, recovery_record_id)
);

CREATE TABLE sync_objects (
    domain_id TEXT NOT NULL REFERENCES sync_domains(domain_id),
    object_id TEXT NOT NULL,
    object_type TEXT NOT NULL,
    latest_version INTEGER NOT NULL CHECK (latest_version > 0),
    latest_ciphertext_hash TEXT NOT NULL,
    latest_key_epoch INTEGER NOT NULL CHECK (latest_key_epoch > 0),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
    PRIMARY KEY (domain_id, object_id)
);

CREATE TABLE sync_object_versions (
    domain_id TEXT NOT NULL REFERENCES sync_domains(domain_id),
    object_id TEXT NOT NULL,
    object_type TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    base_version INTEGER NOT NULL CHECK (base_version + 1 = version),
    owner_device_id TEXT NOT NULL,
    key_id TEXT NOT NULL,
    key_epoch INTEGER NOT NULL CHECK (key_epoch > 0),
    algorithm TEXT NOT NULL,
    nonce BLOB NOT NULL,
    encrypted_payload_len INTEGER NOT NULL CHECK (encrypted_payload_len > 0),
    ciphertext_hash TEXT NOT NULL,
    signature_schema_version INTEGER NOT NULL CHECK (signature_schema_version = 1),
    signature_algorithm TEXT NOT NULL,
    signature_key_id TEXT NOT NULL,
    signature BLOB NOT NULL,
    server_received_at_ms INTEGER NOT NULL,
    client_created_at_ms INTEGER NOT NULL,
    client_updated_at_ms INTEGER NOT NULL CHECK (client_updated_at_ms >= client_created_at_ms),
    blob_ref TEXT NOT NULL,
    PRIMARY KEY (domain_id, object_id, version)
);

CREATE TABLE audit_events (
    audit_event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    device_id TEXT NOT NULL,
    object_id TEXT NOT NULL DEFAULT '',
    version INTEGER NOT NULL DEFAULT 0,
    result_code TEXT NOT NULL,
    bytes INTEGER NOT NULL DEFAULT 0,
    server_time_ms INTEGER NOT NULL
);

CREATE INDEX idx_devices_domain_status
    ON devices(domain_id, status);

CREATE INDEX idx_join_requests_domain_status
    ON device_join_requests(domain_id, status, expires_at_ms);

CREATE INDEX idx_recovery_records_domain_status
    ON recovery_records(domain_id, status, created_at_ms);

CREATE INDEX idx_sync_objects_domain_type
    ON sync_objects(domain_id, object_type, updated_at_ms);

CREATE INDEX idx_audit_events_domain_time
    ON audit_events(domain_id, server_time_ms);
