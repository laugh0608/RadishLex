package api

import (
	"testing"
	"time"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

func TestErrorResponseFromStorageErrorKeepsConflictMetadata(t *testing.T) {
	now := time.UnixMilli(1234)
	err := &storage.Error{
		Code:                 storage.ErrConflictStaleBaseVersion,
		Message:              "object upload is based on an older version",
		LatestVersion:        7,
		LatestCiphertextHash: "sha256:abc",
	}

	response := ErrorResponseFrom(err, now)
	if response.ErrorCode != string(storage.ErrConflictStaleBaseVersion) {
		t.Fatalf("unexpected error code: %s", response.ErrorCode)
	}
	if response.ServerTimeMs != 1234 {
		t.Fatalf("unexpected server time: %d", response.ServerTimeMs)
	}
	if response.LatestVersion != 7 || response.LatestCiphertextHash != "sha256:abc" {
		t.Fatalf("latest metadata missing from conflict response: %#v", response)
	}
}

func TestObjectUploadRequestDoesNotCarryCleartextPayload(t *testing.T) {
	request := ObjectVersionUploadRequest{
		ObjectType:          storage.ObjectDictionaryUserTerms,
		Version:             1,
		BaseVersion:         0,
		OwnerDeviceID:       "device-a",
		KeyID:               "object-key-a",
		KeyEpoch:            1,
		Algorithm:           storage.AlgorithmXChaCha20Poly1305HKDFSHA256,
		Nonce:               []byte{1, 2, 3},
		EncryptedPayloadLen: 3,
		CiphertextHash:      "sha256:abc",
		Signature:           []byte{4, 5, 6},
		ClientCreatedAtMs:   10,
		ClientUpdatedAtMs:   10,
	}
	version := request.StorageVersion("domain-a", "object-a")

	if version.DomainID != "domain-a" || version.ObjectID != "object-a" {
		t.Fatalf("domain/object mapping failed: %#v", version)
	}
	if version.EncryptedPayloadLen != 3 || version.CiphertextHash == "" {
		t.Fatalf("encrypted metadata missing: %#v", version)
	}
}
