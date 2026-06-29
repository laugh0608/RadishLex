package storage

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLocalObjectBlobStoreStagesCommitsAndReadsEncryptedBytes(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	store := newLocalObjectBlobStoreForTest(t, root)
	finalRef := "objects/domain-a/object-a/1/sha256-deadbeef"
	payload := []byte{0x91, 0x92, 0x93}

	staged, err := store.StageObjectBlob(ctx, finalRef, payload)
	if err != nil {
		t.Fatalf("stage blob: %v", err)
	}
	if staged.FinalRef() != finalRef || staged.TempRef() == "" {
		t.Fatalf("unexpected staged refs: final=%q temp=%q", staged.FinalRef(), staged.TempRef())
	}
	if _, err := os.Stat(filepath.Join(root, filepath.FromSlash(finalRef))); !os.IsNotExist(err) {
		t.Fatalf("final blob should not exist before commit, got %v", err)
	}

	if err := staged.Commit(ctx); err != nil {
		t.Fatalf("commit blob: %v", err)
	}
	readPayload, err := store.ReadObjectBlob(ctx, finalRef)
	if err != nil {
		t.Fatalf("read blob: %v", err)
	}
	if !bytes.Equal(readPayload, payload) {
		t.Fatalf("payload mismatch: got %x want %x", readPayload, payload)
	}
	if _, err := os.Stat(filepath.Join(root, filepath.FromSlash(staged.TempRef()))); !os.IsNotExist(err) {
		t.Fatalf("temp blob should be removed after commit, got %v", err)
	}
}

func TestLocalObjectBlobStoreCleanupRemovesTempWithoutPublishing(t *testing.T) {
	ctx := context.Background()
	root := t.TempDir()
	store := newLocalObjectBlobStoreForTest(t, root)
	finalRef := "objects/domain-a/object-a/1/sha256-deadbeef"

	staged, err := store.StageObjectBlob(ctx, finalRef, []byte{0xa1})
	if err != nil {
		t.Fatalf("stage blob: %v", err)
	}
	if err := staged.Cleanup(ctx); err != nil {
		t.Fatalf("cleanup blob: %v", err)
	}
	if _, err := store.ReadObjectBlob(ctx, finalRef); !IsCode(err, ErrNotFound) {
		t.Fatalf("cleaned staged blob should not publish final ref, got %v", err)
	}
	if _, err := os.Stat(filepath.Join(root, filepath.FromSlash(staged.TempRef()))); !os.IsNotExist(err) {
		t.Fatalf("temp blob should be removed after cleanup, got %v", err)
	}
}

func TestLocalObjectBlobStoreRejectsUnsafeBlobRefs(t *testing.T) {
	ctx := context.Background()
	store := newLocalObjectBlobStoreForTest(t, t.TempDir())
	for _, ref := range []string{
		"",
		"../secret",
		"/absolute",
		"objects/../secret",
		"objects//secret",
		"objects\\secret",
		"objects/a:b",
		".tmp/blob-a",
		"objects/domain-a/object-a/1/sha256 deadbeef",
	} {
		if _, err := store.StageObjectBlob(ctx, ref, []byte{0x01}); !IsCode(err, ErrInvalidRequest) {
			t.Fatalf("unsafe ref %q should be rejected, got %v", ref, err)
		}
	}
}

func TestLocalObjectBlobStoreRejectsEmptyPayload(t *testing.T) {
	store := newLocalObjectBlobStoreForTest(t, t.TempDir())
	if _, err := store.StageObjectBlob(context.Background(), "objects/domain-a/object-a/1/sha256-deadbeef", nil); !IsCode(err, ErrInvalidCiphertextMetadata) {
		t.Fatalf("empty payload should be rejected, got %v", err)
	}
}

func TestLocalObjectBlobStoreHandlesExistingBlobRefs(t *testing.T) {
	ctx := context.Background()
	store := newLocalObjectBlobStoreForTest(t, t.TempDir())
	finalRef := "objects/domain-a/object-a/1/sha256-deadbeef"
	payload := []byte{0x91}

	first, err := store.StageObjectBlob(ctx, finalRef, payload)
	if err != nil {
		t.Fatalf("stage first blob: %v", err)
	}
	if err := first.Commit(ctx); err != nil {
		t.Fatalf("commit first blob: %v", err)
	}

	idempotent, err := store.StageObjectBlob(ctx, finalRef, payload)
	if err != nil {
		t.Fatalf("stage idempotent blob: %v", err)
	}
	if err := idempotent.Commit(ctx); err != nil {
		t.Fatalf("commit idempotent blob: %v", err)
	}

	conflicting, err := store.StageObjectBlob(ctx, finalRef, []byte{0x92})
	if err != nil {
		t.Fatalf("stage conflicting blob: %v", err)
	}
	if err := conflicting.Commit(ctx); !IsCode(err, ErrConflictObjectVersion) {
		t.Fatalf("conflicting blob payload should be rejected, got %v", err)
	}
	if _, err := os.Stat(filepath.Join(store.root, filepath.FromSlash(conflicting.TempRef()))); !os.IsNotExist(err) {
		t.Fatalf("conflicting staged blob should be removed, got %v", err)
	}
	readPayload, err := store.ReadObjectBlob(ctx, finalRef)
	if err != nil {
		t.Fatalf("read existing blob: %v", err)
	}
	if !bytes.Equal(readPayload, payload) {
		t.Fatalf("existing blob should remain unchanged: got %x want %x", readPayload, payload)
	}
}

func TestGeneratedBlobRefsUseSafePathComponents(t *testing.T) {
	objectRef := objectBlobRef(ObjectVersion{
		DomainID:       "domain:a",
		ObjectID:       "object:a",
		Version:        7,
		CiphertextHash: "sha256:deadbeef",
	})
	if strings.Contains(objectRef, ":") {
		t.Fatalf("object blob ref should not contain raw hash or id separators: %q", objectRef)
	}
	if _, err := validateBlobRef(objectRef); err != nil {
		t.Fatalf("object blob ref should be valid for local object storage: %v", err)
	}

	recoveryRef := recoveryBlobRef(RecoveryRecord{
		DomainID:         "domain:a",
		RecoveryRecordID: "recovery:a",
		CiphertextHash:   "sha256:deadbeef",
	})
	if strings.Contains(recoveryRef, ":") {
		t.Fatalf("recovery blob ref should not contain raw hash or id separators: %q", recoveryRef)
	}
	if _, err := validateBlobRef(recoveryRef); err != nil {
		t.Fatalf("recovery blob ref should be valid for local object storage: %v", err)
	}

	wrappingRef := wrappingBlobRef(DeviceWrappingRecord{
		DomainID:          "domain:a",
		RecipientDeviceID: "device:a",
		KeyEpoch:          2,
		WrappingKeyID:     "wrapping:key",
		CiphertextHash:    "sha256:deadbeef",
	})
	if strings.Contains(wrappingRef, ":") {
		t.Fatalf("wrapping blob ref should not contain raw hash or id separators: %q", wrappingRef)
	}
	if _, err := validateBlobRef(wrappingRef); err != nil {
		t.Fatalf("wrapping blob ref should be valid for local object storage: %v", err)
	}
}

func newLocalObjectBlobStoreForTest(t *testing.T, root string) *LocalObjectBlobStore {
	t.Helper()
	store, err := NewLocalObjectBlobStore(root)
	if err != nil {
		t.Fatalf("create local object blob store: %v", err)
	}
	return store
}
