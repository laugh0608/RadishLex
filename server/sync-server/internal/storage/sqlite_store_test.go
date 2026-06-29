package storage

import (
	"context"
	"database/sql"
	"os"
	"path/filepath"
	"testing"

	_ "modernc.org/sqlite"
)

func TestSQLiteStoreConformance(t *testing.T) {
	runStoreConformanceTests(t, func(t *testing.T) Store {
		t.Helper()
		return newSQLiteStoreForTest(t)
	})
}

func TestSQLiteStoreCleansStagedBlobWhenMetadataValidationFails(t *testing.T) {
	store := newSQLiteStoreForTest(t)
	upload := objectUpload("domain-missing", "object-a", "device-a", 1, 0, 1, []byte{0x91})

	if _, err := store.PutObjectVersion(context.Background(), upload); !IsCode(err, ErrNotFound) {
		t.Fatalf("missing domain should reject object metadata, got %v", err)
	}
	payloadRoot := store.blobs.(*LocalObjectBlobStore).root
	entries, err := os.ReadDir(filepath.Join(payloadRoot, ".tmp"))
	if err != nil && !os.IsNotExist(err) {
		t.Fatalf("read temp blob dir: %v", err)
	}
	if len(entries) != 0 {
		t.Fatalf("metadata validation failure should not leave temp blobs: %d", len(entries))
	}
}

func TestSQLiteStoreObjectPayloadDetectsMissingBlob(t *testing.T) {
	store := newSQLiteStoreForTest(t)
	_ = newReadyStore(t, func(t *testing.T) Store {
		t.Helper()
		return store
	})
	upload := objectUpload("domain-a", "object-a", "device-a", 1, 0, 1, []byte{0x91})

	metadata, err := store.PutObjectVersion(context.Background(), upload)
	if err != nil {
		t.Fatalf("put object version: %v", err)
	}
	if err := store.blobs.DeleteObjectBlob(context.Background(), metadata.BlobRef); err != nil {
		t.Fatalf("delete object blob: %v", err)
	}
	if _, err := store.ObjectPayload(context.Background(), "domain-a", "object-a", 1); !IsCode(err, ErrStorageUnavailable) {
		t.Fatalf("missing object blob should be reported by blob store, got %v", err)
	}
}

func TestSQLiteStoreDeviceWrappedKeyDetectsMissingBlob(t *testing.T) {
	store := newSQLiteStoreForTest(t)
	_ = newReadyStore(t, func(t *testing.T) Store {
		t.Helper()
		return store
	})
	saveJoinAndAuthorize(t, store, "domain-a", "join-b", "device-b", 20)
	metadata, _, err := store.DeviceWrappedKey(context.Background(), "domain-a", "device-b", 1, "wrapping-key-device-b")
	if err != nil {
		t.Fatalf("read wrapped key: %v", err)
	}
	if err := store.blobs.DeleteObjectBlob(context.Background(), metadata.BlobRef); err != nil {
		t.Fatalf("delete wrapped key blob: %v", err)
	}
	if _, _, err := store.DeviceWrappedKey(context.Background(), "domain-a", "device-b", 1, "wrapping-key-device-b"); !IsCode(err, ErrStorageUnavailable) {
		t.Fatalf("missing wrapped key blob should be reported by blob store, got %v", err)
	}
}

func TestSQLiteStoreLatestRecoveryWrappedMaterialDetectsMissingBlob(t *testing.T) {
	store := newSQLiteStoreForTest(t)
	_ = newReadyStore(t, func(t *testing.T) Store {
		t.Helper()
		return store
	})
	wrapped := []byte{0xa1, 0xa2, 0xa3}
	record := RecoveryRecord{
		DomainID:           "domain-a",
		RecoveryRecordID:   "recovery-a",
		KeyEpoch:           1,
		KDFProfile:         "argon2id-v1",
		KDFVersion:         1,
		MemoryKiB:          65536,
		Iterations:         3,
		Parallelism:        4,
		OutputLen:          32,
		Salt:               []byte{0x01, 0x02},
		Algorithm:          AlgorithmXChaCha20Poly1305HKDFSHA256,
		Nonce:              []byte{0x03, 0x04},
		WrappedMaterialLen: int64(len(wrapped)),
		CiphertextHash:     CiphertextHash(wrapped),
		Status:             RecoveryRecordActive,
		CreatedAtMs:        40,
		SignerDeviceID:     "device-a",
	}
	signRecoveryForTest(&record)
	metadata, err := store.PutRecoveryRecord(context.Background(), RecoveryRecordUpload{Record: record, WrappedMaterial: wrapped})
	if err != nil {
		t.Fatalf("put recovery record: %v", err)
	}
	if err := store.blobs.DeleteObjectBlob(context.Background(), metadata.BlobRef); err != nil {
		t.Fatalf("delete recovery wrapped material blob: %v", err)
	}
	if _, _, err := store.LatestRecoveryWrappedMaterial(context.Background(), "domain-a"); !IsCode(err, ErrStorageUnavailable) {
		t.Fatalf("missing recovery wrapped material should be reported by blob store, got %v", err)
	}
}

func TestSQLiteStoreRecordsAuditEvent(t *testing.T) {
	store := newSQLiteStoreForTest(t)
	event := AuditEvent{
		DomainID:     "domain-a",
		EventType:    "domains.create",
		DeviceID:     "device-a",
		ObjectID:     "",
		Version:      0,
		ResultCode:   "ok",
		Bytes:        123,
		ServerTimeMs: 456,
	}

	if err := store.RecordAuditEvent(context.Background(), event); err != nil {
		t.Fatalf("record audit event: %v", err)
	}

	row := store.db.QueryRow(`
		SELECT domain_id, event_type, device_id, object_id, version, result_code, bytes, server_time_ms
		FROM audit_events
	`)
	var got AuditEvent
	var version int64
	if err := row.Scan(
		&got.DomainID, &got.EventType, &got.DeviceID, &got.ObjectID,
		&version, &got.ResultCode, &got.Bytes, &got.ServerTimeMs,
	); err != nil {
		t.Fatalf("read audit event: %v", err)
	}
	got.Version = uint64(version)
	if got != event {
		t.Fatalf("unexpected audit event: got %#v want %#v", got, event)
	}
}

func newSQLiteStoreForTest(t *testing.T) *SQLiteStore {
	t.Helper()
	root := t.TempDir()
	dbPath := filepath.Join(root, "metadata.sqlite")
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		t.Fatalf("open sqlite: %v", err)
	}
	db.SetMaxOpenConns(1)
	t.Cleanup(func() {
		_ = db.Close()
	})
	applySQLiteMigrationForTest(t, db)

	blobStore, err := NewLocalObjectBlobStore(filepath.Join(root, "objects"))
	if err != nil {
		t.Fatalf("create blob store: %v", err)
	}
	store, err := NewSQLiteStore(db, blobStore)
	if err != nil {
		t.Fatalf("create sqlite store: %v", err)
	}
	return store
}

func applySQLiteMigrationForTest(t *testing.T, db *sql.DB) {
	t.Helper()
	migrationPath := filepath.Join("..", "..", "migrations", "0001_init.sql")
	migration, err := os.ReadFile(migrationPath)
	if err != nil {
		t.Fatalf("read migration: %v", err)
	}
	if _, err := db.Exec(string(migration)); err != nil {
		t.Fatalf("apply migration: %v", err)
	}
}
