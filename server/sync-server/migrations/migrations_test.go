package migrations

import (
	"database/sql"
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
		INSERT INTO device_join_requests (
			domain_id, join_request_id, device_id, signing_public_key_id,
			signing_public_key, key_agreement_public_key_id,
			key_agreement_public_key, challenge, created_at_ms, expires_at_ms, status
		) VALUES (
			'domain-history', 'join-history', 'device-pending', 'signing-key-pending',
			x'03', 'agreement-key-pending', x'04', x'05', 110, 210, 'pending'
		);
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
	var schemaVersion int
	if err := db.QueryRow("PRAGMA user_version").Scan(&schemaVersion); err != nil {
		t.Fatalf("read schema version: %v", err)
	}
	if schemaVersion != 2 {
		t.Fatalf("unexpected schema version: %d", schemaVersion)
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
`
