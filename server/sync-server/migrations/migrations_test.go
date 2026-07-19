package migrations

import (
	"database/sql"
	"fmt"
	"reflect"
	"testing"

	_ "modernc.org/sqlite"
)

func TestApplyBackfillsHistoricalDeviceAlgorithmsAndIsIdempotent(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open sqlite: %v", err)
	}
	db.SetMaxOpenConns(1)
	t.Cleanup(func() { _ = db.Close() })

	if _, err := db.Exec(historicalDeviceSchema); err != nil {
		t.Fatalf("create historical schema: %v", err)
	}
	if _, err := db.Exec(`
		INSERT INTO sync_domains (
			domain_id, current_key_epoch, active_key_id, created_at_ms, updated_at_ms
		) VALUES ('domain-history', 1, 'sync-key-history', 100, 100);
		INSERT INTO devices (
			domain_id, device_id, signing_public_key_id, signing_public_key,
			key_agreement_public_key_id, key_agreement_public_key, status,
			authorized_at_ms, revoked_at_ms, last_seen_at_ms
		) VALUES (
			'domain-history', 'device-history', 'signing-key-history', x'01',
			'agreement-key-history', x'02', 'active', 100, 0, 0
		);
		INSERT INTO device_wrapping_records (
			domain_id, recipient_device_id, authorizer_device_id, key_epoch,
			wrapping_key_id, algorithm, nonce, wrapped_key_len, ciphertext_hash,
			created_at_ms, signature, blob_ref
		) VALUES (
			'domain-history', 'device-history', 'device-history', 1,
			'wrapping-history', 'xchacha20poly1305-hkdf-sha256-v1', x'01', 1,
			'sha256:history', 100, x'02', 'wrapping/history'
		);
		INSERT INTO device_join_requests (
			domain_id, join_request_id, device_id, signing_public_key_id,
			signing_public_key, key_agreement_public_key_id,
			key_agreement_public_key, challenge, created_at_ms, expires_at_ms, status
		) VALUES (
			'domain-history', 'join-history', 'device-pending', 'signing-key-pending',
			x'03', 'agreement-key-pending', x'04', x'05', 110, 210, 'pending'
		);
		INSERT INTO sync_objects (
			domain_id, object_id, object_type, latest_version, latest_ciphertext_hash,
			latest_key_epoch, created_at_ms, updated_at_ms
		) VALUES
			('domain-history', 'object-a', 'dictionary.user_terms', 2, 'hash-a2', 1, 100, 200),
			('domain-history', 'object-b', 'ranker.weights', 1, 'hash-b1', 1, 100, 100);
		INSERT INTO sync_object_versions (
			domain_id, object_id, object_type, version, base_version, owner_device_id,
			key_id, key_epoch, algorithm, nonce, encrypted_payload_len, ciphertext_hash,
			signature_schema_version, signature_algorithm, signature_key_id, signature,
			server_received_at_ms, client_created_at_ms, client_updated_at_ms, blob_ref
		) VALUES
			('domain-history', 'object-a', 'dictionary.user_terms', 1, 0, 'device-history',
			 'object-key', 1, 'xchacha20poly1305-hkdf-sha256-v1', x'01', 1, 'hash-a1',
			 1, 'ed25519-v1', 'signing-key-history', x'01', 100, 90, 90, 'blob-a1'),
			('domain-history', 'object-b', 'ranker.weights', 1, 0, 'device-history',
			 'object-key', 1, 'xchacha20poly1305-hkdf-sha256-v1', x'02', 1, 'hash-b1',
			 1, 'ed25519-v1', 'signing-key-history', x'02', 100, 90, 90, 'blob-b1'),
			('domain-history', 'object-a', 'dictionary.user_terms', 2, 1, 'device-history',
			 'object-key', 1, 'xchacha20poly1305-hkdf-sha256-v1', x'03', 1, 'hash-a2',
			 1, 'ed25519-v1', 'signing-key-history', x'03', 200, 190, 190, 'blob-a2');
	`); err != nil {
		t.Fatalf("insert historical rows: %v", err)
	}

	if err := Apply(db); err != nil {
		t.Fatalf("apply migration: %v", err)
	}
	if err := Apply(db); err != nil {
		t.Fatalf("apply migration a second time: %v", err)
	}

	for _, table := range []string{"devices", "device_join_requests"} {
		var algorithm string
		if err := db.QueryRow("SELECT signing_algorithm FROM " + table + " LIMIT 1").Scan(&algorithm); err != nil {
			t.Fatalf("read %s signing algorithm: %v", table, err)
		}
		if algorithm != "ed25519-v1" {
			t.Fatalf("unexpected %s historical signing algorithm: %q", table, algorithm)
		}
	}
	rows, err := db.Query(`
		SELECT object_id, version, change_sequence
		FROM sync_object_versions
		ORDER BY change_sequence
	`)
	if err != nil {
		t.Fatalf("read historical object sequences: %v", err)
	}
	var got []string
	for rows.Next() {
		var objectID string
		var version int
		var sequence int
		if err := rows.Scan(&objectID, &version, &sequence); err != nil {
			t.Fatalf("scan historical object sequence: %v", err)
		}
		got = append(got, fmt.Sprintf("%s/%d=%d", objectID, version, sequence))
	}
	if err := rows.Err(); err != nil {
		t.Fatalf("iterate historical object sequences: %v", err)
	}
	if err := rows.Close(); err != nil {
		t.Fatalf("close historical object sequences: %v", err)
	}
	want := []string{"object-a/1=1", "object-b/1=2", "object-a/2=3"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("unexpected historical object sequences: got %v want %v", got, want)
	}
	for objectID, wantSequence := range map[string]int{"object-a": 3, "object-b": 2} {
		var gotSequence int
		if err := db.QueryRow(
			"SELECT latest_change_sequence FROM sync_objects WHERE domain_id = 'domain-history' AND object_id = ?",
			objectID,
		).Scan(&gotSequence); err != nil {
			t.Fatalf("read %s latest sequence: %v", objectID, err)
		}
		if gotSequence != wantSequence {
			t.Fatalf("unexpected %s latest sequence: got %d want %d", objectID, gotSequence, wantSequence)
		}
	}
	var schemaVersion int
	if err := db.QueryRow("PRAGMA user_version").Scan(&schemaVersion); err != nil {
		t.Fatalf("read schema version: %v", err)
	}
	if schemaVersion != 6 {
		t.Fatalf("unexpected schema version: %d", schemaVersion)
	}
	var wrappingRecipientKeyID string
	if err := db.QueryRow(`
		SELECT recipient_key_agreement_key_id FROM device_wrapping_records
		WHERE domain_id = 'domain-history' AND recipient_device_id = 'device-history'
	`).Scan(&wrappingRecipientKeyID); err != nil {
		t.Fatalf("read migrated wrapping recipient key id: %v", err)
	}
	if wrappingRecipientKeyID != "agreement-key-history" {
		t.Fatalf("unexpected migrated wrapping recipient key id: %q", wrappingRecipientKeyID)
	}
	var signatureRecordType string
	var signatureSchemaVersion int
	var signatureAlgorithm string
	var signatureKeyID string
	if err := db.QueryRow(`
		SELECT signature_record_type, signature_schema_version, signature_algorithm, signature_key_id
		FROM device_wrapping_records
		WHERE domain_id = 'domain-history' AND recipient_device_id = 'device-history'
	`).Scan(&signatureRecordType, &signatureSchemaVersion, &signatureAlgorithm, &signatureKeyID); err != nil {
		t.Fatalf("read migrated wrapping signature metadata: %v", err)
	}
	if signatureRecordType != "device_authorization" || signatureSchemaVersion != 1 ||
		signatureAlgorithm != "ed25519-v1" || signatureKeyID != "signing-key-history" {
		t.Fatalf("unexpected migrated wrapping signature metadata: %q/%d/%q/%q", signatureRecordType, signatureSchemaVersion, signatureAlgorithm, signatureKeyID)
	}
	var lifecycleCount int
	if err := db.QueryRow("SELECT COUNT(*) FROM domain_lifecycle_events WHERE domain_id = 'domain-history'").Scan(&lifecycleCount); err != nil {
		t.Fatalf("read lifecycle backfill count: %v", err)
	}
	if lifecycleCount != 1 {
		t.Fatalf("unexpected lifecycle backfill count: %d", lifecycleCount)
	}
}

const historicalDeviceSchema = `
CREATE TABLE sync_domains (
    domain_id TEXT PRIMARY KEY,
    current_key_epoch INTEGER NOT NULL,
    active_key_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE TABLE devices (
    domain_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    signing_public_key_id TEXT NOT NULL,
    signing_public_key BLOB NOT NULL,
    key_agreement_public_key_id TEXT NOT NULL,
    key_agreement_public_key BLOB NOT NULL,
    status TEXT NOT NULL,
    authorized_at_ms INTEGER NOT NULL DEFAULT 0,
    revoked_at_ms INTEGER NOT NULL DEFAULT 0,
    last_seen_at_ms INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (domain_id, device_id)
);
CREATE TABLE device_join_requests (
    domain_id TEXT NOT NULL,
    join_request_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    signing_public_key_id TEXT NOT NULL,
    signing_public_key BLOB NOT NULL,
    key_agreement_public_key_id TEXT NOT NULL,
    key_agreement_public_key BLOB NOT NULL,
    challenge BLOB NOT NULL,
    created_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    status TEXT NOT NULL,
    PRIMARY KEY (domain_id, join_request_id)
);
CREATE TABLE device_wrapping_records (
    domain_id TEXT NOT NULL,
    recipient_device_id TEXT NOT NULL,
    authorizer_device_id TEXT NOT NULL,
    key_epoch INTEGER NOT NULL,
    wrapping_key_id TEXT NOT NULL,
    algorithm TEXT NOT NULL,
    nonce BLOB NOT NULL,
    wrapped_key_len INTEGER NOT NULL,
    ciphertext_hash TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    signature BLOB NOT NULL,
    blob_ref TEXT NOT NULL,
    PRIMARY KEY (domain_id, recipient_device_id, key_epoch, wrapping_key_id)
);
CREATE TABLE sync_objects (
    domain_id TEXT NOT NULL,
    object_id TEXT NOT NULL,
    object_type TEXT NOT NULL,
    latest_version INTEGER NOT NULL,
    latest_ciphertext_hash TEXT NOT NULL,
    latest_key_epoch INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    PRIMARY KEY (domain_id, object_id)
);
CREATE TABLE sync_object_versions (
    domain_id TEXT NOT NULL,
    object_id TEXT NOT NULL,
    object_type TEXT NOT NULL,
    version INTEGER NOT NULL,
    base_version INTEGER NOT NULL,
    owner_device_id TEXT NOT NULL,
    key_id TEXT NOT NULL,
    key_epoch INTEGER NOT NULL,
    algorithm TEXT NOT NULL,
    nonce BLOB NOT NULL,
    encrypted_payload_len INTEGER NOT NULL,
    ciphertext_hash TEXT NOT NULL,
    signature_schema_version INTEGER NOT NULL,
    signature_algorithm TEXT NOT NULL,
    signature_key_id TEXT NOT NULL,
    signature BLOB NOT NULL,
    server_received_at_ms INTEGER NOT NULL,
    client_created_at_ms INTEGER NOT NULL,
    client_updated_at_ms INTEGER NOT NULL,
    blob_ref TEXT NOT NULL,
    PRIMARY KEY (domain_id, object_id, version)
);
`
