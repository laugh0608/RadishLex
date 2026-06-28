package storage

import (
	"context"
	"testing"
)

type storeFactory func(t *testing.T) Store

func runStoreConformanceTests(t *testing.T, newStore storeFactory) {
	t.Helper()

	t.Run("accepts encrypted object and returns payload by metadata", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		upload := objectUpload("domain-a", "object-a", "device-a", 1, 0, 1, []byte{0x91, 0x92, 0x93})

		metadata, err := store.PutObjectVersion(ctx, upload)
		if err != nil {
			t.Fatalf("put object version: %v", err)
		}
		if metadata.BlobRef == "" {
			t.Fatal("blob ref should be assigned by storage")
		}
		if metadata.CiphertextHash != upload.Version.CiphertextHash {
			t.Fatalf("ciphertext hash mismatch: got %q", metadata.CiphertextHash)
		}

		payload, err := store.ObjectPayload(ctx, "domain-a", "object-a", 1)
		if err != nil {
			t.Fatalf("read object payload: %v", err)
		}
		if string(payload) != string(upload.Payload) {
			t.Fatalf("payload mismatch: got %x want %x", payload, upload.Payload)
		}
	})

	t.Run("detects object version conflicts and idempotent retry", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		first := objectUpload("domain-a", "object-a", "device-a", 1, 0, 1, []byte{0x91})
		if _, err := store.PutObjectVersion(ctx, first); err != nil {
			t.Fatalf("put first version: %v", err)
		}

		retry, err := store.PutObjectVersion(ctx, first)
		if err != nil {
			t.Fatalf("idempotent retry: %v", err)
		}
		if retry.CiphertextHash != first.Version.CiphertextHash {
			t.Fatalf("retry returned different hash: got %q", retry.CiphertextHash)
		}

		conflicting := objectUpload("domain-a", "object-a", "device-a", 1, 0, 1, []byte{0x92})
		if _, err := store.PutObjectVersion(ctx, conflicting); !IsCode(err, ErrConflictObjectVersion) {
			t.Fatalf("same version with different hash should conflict, got %v", err)
		}

		second := objectUpload("domain-a", "object-a", "device-a", 2, 1, 1, []byte{0x93})
		if _, err := store.PutObjectVersion(ctx, second); err != nil {
			t.Fatalf("put second version: %v", err)
		}

		stale := objectUpload("domain-a", "object-a", "device-a", 2, 1, 1, []byte{0x94})
		stale.Version.Version = 3
		err = putExpectError(store, stale)
		var storageErr *Error
		if !IsCode(err, ErrConflictStaleBaseVersion) {
			t.Fatalf("stale base should return conflict_stale_base_version, got %v", err)
		}
		if !errorAs(err, &storageErr) || storageErr.LatestVersion != 2 || storageErr.LatestCiphertextHash == "" {
			t.Fatalf("stale conflict should include latest metadata, got %#v", storageErr)
		}
	})

	t.Run("blocks revoked devices and old key epoch writes", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		saveJoinAndAuthorize(t, store, "domain-a", "join-b", "device-b", 20)

		revocation := DeviceRevocation{
			DomainID:         "domain-a",
			RevokedDeviceID:  "device-b",
			RevokerDeviceID:  "device-a",
			PreviousKeyEpoch: 1,
			NewKeyEpoch:      2,
			Reason:           "lost",
			CreatedAtMs:      30,
			Signature:        []byte{0x30},
		}
		if err := store.RevokeDevice(ctx, revocation); err != nil {
			t.Fatalf("revoke device: %v", err)
		}

		revokedUpload := objectUpload("domain-a", "object-b", "device-b", 1, 0, 2, []byte{0x95})
		if _, err := store.PutObjectVersion(ctx, revokedUpload); !IsCode(err, ErrForbiddenDevice) {
			t.Fatalf("revoked device should not upload, got %v", err)
		}

		oldEpochUpload := objectUpload("domain-a", "object-c", "device-a", 1, 0, 1, []byte{0x96})
		if _, err := store.PutObjectVersion(ctx, oldEpochUpload); !IsCode(err, ErrForbiddenDevice) {
			t.Fatalf("old epoch upload should be forbidden, got %v", err)
		}

		newEpochUpload := objectUpload("domain-a", "object-c", "device-a", 1, 0, 2, []byte{0x97})
		if _, err := store.PutObjectVersion(ctx, newEpochUpload); err != nil {
			t.Fatalf("active device should upload with current epoch: %v", err)
		}
	})

	t.Run("join authorization activates device and stores wrapping metadata", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		saveJoinAndAuthorize(t, store, "domain-a", "join-b", "device-b", 20)

		device, err := store.Device(ctx, "domain-a", "device-b")
		if err != nil {
			t.Fatalf("load authorized device: %v", err)
		}
		if device.Status != DeviceActive || device.AuthorizedAtMs != 20 {
			t.Fatalf("device should be active after authorization: %#v", device)
		}
	})

	t.Run("recovery record stores only wrapped material metadata", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		wrapped := []byte{0xa1, 0xa2, 0xa3}
		record := RecoveryRecord{
			DomainID:           "domain-a",
			RecoveryRecordID:   "recovery-a",
			KeyEpoch:           1,
			KDFProfile:         "argon2id-v1",
			Salt:               []byte{0x01, 0x02},
			Algorithm:          AlgorithmXChaCha20Poly1305HKDFSHA256,
			Nonce:              []byte{0x03, 0x04},
			WrappedMaterialLen: int64(len(wrapped)),
			CiphertextHash:     CiphertextHash(wrapped),
			Status:             RecoveryRecordActive,
			CreatedAtMs:        40,
			SignerDeviceID:     "device-a",
			Signature:          []byte{0x40},
		}

		stored, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: record, WrappedMaterial: wrapped})
		if err != nil {
			t.Fatalf("put recovery record: %v", err)
		}
		if stored.BlobRef == "" {
			t.Fatal("recovery record should receive blob ref")
		}
		latest, err := store.LatestRecoveryRecord(ctx, "domain-a")
		if err != nil {
			t.Fatalf("latest recovery record: %v", err)
		}
		if latest.RecoveryRecordID != "recovery-a" || latest.CiphertextHash != record.CiphertextHash {
			t.Fatalf("latest recovery record mismatch: %#v", latest)
		}
	})
}

func newReadyStore(t *testing.T, newStore storeFactory) Store {
	t.Helper()
	store := newStore(t)
	if store == nil {
		t.Fatal("store factory returned nil")
	}
	err := store.CreateDomain(context.Background(), Domain{
		DomainID:        "domain-a",
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "sync-key-a",
		CreatedAtMs:     10,
		UpdatedAtMs:     10,
	}, Device{
		DomainID:                "domain-a",
		DeviceID:                "device-a",
		SigningPublicKeyID:      "signing-key-a",
		SigningPublicKey:        []byte{0x01},
		KeyAgreementPublicKeyID: "agreement-key-a",
		KeyAgreementPublicKey:   []byte{0x02},
		Status:                  DeviceActive,
		AuthorizedAtMs:          10,
	})
	if err != nil {
		t.Fatalf("create ready store: %v", err)
	}
	return store
}

func saveJoinAndAuthorize(t *testing.T, store Store, domainID string, joinID string, deviceID string, atMs int64) {
	t.Helper()
	ctx := context.Background()
	request := JoinRequest{
		DomainID:                domainID,
		JoinRequestID:           joinID,
		DeviceID:                deviceID,
		SigningPublicKeyID:      "signing-key-" + deviceID,
		SigningPublicKey:        []byte{0x11},
		KeyAgreementPublicKeyID: "agreement-key-" + deviceID,
		KeyAgreementPublicKey:   []byte{0x12},
		Challenge:               []byte{0x13},
		CreatedAtMs:             atMs - 1,
		ExpiresAtMs:             atMs + 100,
		Status:                  DevicePending,
	}
	if err := store.SaveJoinRequest(ctx, request); err != nil {
		t.Fatalf("save join request: %v", err)
	}
	wrapped := []byte{0x21, 0x22}
	authorization := DeviceAuthorization{
		DomainID:                    domainID,
		JoinRequestID:               joinID,
		AuthorizerDeviceID:          "device-a",
		RecipientDeviceID:           deviceID,
		RecipientSigningPublicKeyID: request.SigningPublicKeyID,
		RecipientKeyAgreementKeyID:  request.KeyAgreementPublicKeyID,
		KeyEpoch:                    1,
		CreatedAtMs:                 atMs,
		Signature:                   []byte{0x20},
	}
	wrapping := DeviceWrappingRecord{
		DomainID:           domainID,
		RecipientDeviceID:  deviceID,
		AuthorizerDeviceID: "device-a",
		KeyEpoch:           1,
		WrappingKeyID:      "wrapping-key-" + deviceID,
		Algorithm:          AlgorithmXChaCha20Poly1305HKDFSHA256,
		Nonce:              []byte{0x23},
		WrappedKeyLen:      int64(len(wrapped)),
		CiphertextHash:     CiphertextHash(wrapped),
		CreatedAtMs:        atMs,
		Signature:          []byte{0x24},
	}
	if err := store.AuthorizeJoinRequest(ctx, authorization, wrapping); err != nil {
		t.Fatalf("authorize join request: %v", err)
	}
}

func objectUpload(domainID string, objectID string, deviceID string, version uint64, baseVersion uint64, keyEpoch uint64, payload []byte) ObjectVersionUpload {
	return ObjectVersionUpload{
		Version: ObjectVersion{
			DomainID:            domainID,
			ObjectID:            objectID,
			ObjectType:          ObjectDictionaryUserTerms,
			Version:             version,
			BaseVersion:         baseVersion,
			OwnerDeviceID:       deviceID,
			KeyID:               "object-key-a",
			KeyEpoch:            keyEpoch,
			Algorithm:           AlgorithmXChaCha20Poly1305HKDFSHA256,
			Nonce:               []byte{byte(version), byte(baseVersion), byte(keyEpoch)},
			EncryptedPayloadLen: int64(len(payload)),
			CiphertextHash:      CiphertextHash(payload),
			Signature:           []byte{0x41, byte(version)},
			ServerReceivedAtMs:  0,
			ClientCreatedAtMs:   100 + int64(version),
			ClientUpdatedAtMs:   100 + int64(version),
		},
		Payload: payload,
	}
}

func putExpectError(store Store, upload ObjectVersionUpload) error {
	_, err := store.PutObjectVersion(context.Background(), upload)
	return err
}

func errorAs(err error, target **Error) bool {
	if err == nil {
		return false
	}
	storageErr, ok := err.(*Error)
	if !ok {
		return false
	}
	*target = storageErr
	return true
}
