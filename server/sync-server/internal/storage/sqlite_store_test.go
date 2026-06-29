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
