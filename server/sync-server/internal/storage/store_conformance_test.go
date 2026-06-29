package storage

import (
	"context"
	"crypto/ed25519"
	"strconv"
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
		signObjectForTest(&stale.Version)
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
		}
		signRevocationForTest(&revocation)
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

	t.Run("rejects signed object manifest tampering", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		upload := objectUpload("domain-a", "object-a", "device-a", 1, 0, 1, []byte{0x91})
		upload.Version.ObjectID = "object-replaced"

		if _, err := store.PutObjectVersion(ctx, upload); !IsCode(err, ErrInvalidSignature) {
			t.Fatalf("tampered object manifest should fail signature verification, got %v", err)
		}
	})

	t.Run("rejects signed authorization tampering", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		request, authorization, wrapping := joinAuthorizationFixture("domain-a", "join-b", "device-b", 20)
		if err := store.SaveJoinRequest(ctx, request); err != nil {
			t.Fatalf("save join request: %v", err)
		}
		authorization.JoinShortCode = "654321"

		if err := store.AuthorizeJoinRequest(ctx, authorization, wrapping); !IsCode(err, ErrInvalidSignature) {
			t.Fatalf("tampered authorization should fail signature verification, got %v", err)
		}
	})

	t.Run("rejects signed revocation tampering", func(t *testing.T) {
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
		}
		signRevocationForTest(&revocation)
		revocation.Reason = "user_requested"

		if err := store.RevokeDevice(ctx, revocation); !IsCode(err, ErrInvalidSignature) {
			t.Fatalf("tampered revocation should fail signature verification, got %v", err)
		}
	})

	t.Run("rejects signed recovery record tampering", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
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
		record.MemoryKiB = 32768

		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: record, WrappedMaterial: wrapped}); !IsCode(err, ErrInvalidSignature) {
			t.Fatalf("tampered recovery record should fail signature verification, got %v", err)
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
		SigningPublicKey:        signingPublicKeyForTest("device-a"),
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
	request, authorization, wrapping := joinAuthorizationFixture(domainID, joinID, deviceID, atMs)
	if err := store.SaveJoinRequest(ctx, request); err != nil {
		t.Fatalf("save join request: %v", err)
	}
	if err := store.AuthorizeJoinRequest(ctx, authorization, wrapping); err != nil {
		t.Fatalf("authorize join request: %v", err)
	}
}

func joinAuthorizationFixture(domainID string, joinID string, deviceID string, atMs int64) (JoinRequest, DeviceAuthorization, DeviceWrappingRecord) {
	request := JoinRequest{
		DomainID:                domainID,
		JoinRequestID:           joinID,
		DeviceID:                deviceID,
		SigningPublicKeyID:      signingKeyIDForTest(deviceID),
		SigningPublicKey:        signingPublicKeyForTest(deviceID),
		KeyAgreementPublicKeyID: "agreement-key-" + deviceID,
		KeyAgreementPublicKey:   []byte{0x12},
		Challenge:               []byte{0x13},
		CreatedAtMs:             atMs - 1,
		ExpiresAtMs:             atMs + 100,
		Status:                  DevicePending,
	}
	wrapped := []byte{0x21, 0x22}
	authorization := DeviceAuthorization{
		DomainID:                    domainID,
		JoinRequestID:               joinID,
		AuthorizerDeviceID:          "device-a",
		RecipientDeviceID:           deviceID,
		RecipientSigningPublicKeyID: request.SigningPublicKeyID,
		RecipientKeyAgreementKeyID:  request.KeyAgreementPublicKeyID,
		JoinShortCode:               "123456",
		KeyEpoch:                    1,
		CreatedAtMs:                 atMs,
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
	signAuthorizationForTest(&authorization, wrapping, request)
	return request, authorization, wrapping
}

func objectUpload(domainID string, objectID string, deviceID string, version uint64, baseVersion uint64, keyEpoch uint64, payload []byte) ObjectVersionUpload {
	upload := ObjectVersionUpload{
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
			ServerReceivedAtMs:  0,
			ClientCreatedAtMs:   100 + int64(version),
			ClientUpdatedAtMs:   100 + int64(version),
		},
		Payload: payload,
	}
	signObjectForTest(&upload.Version)
	return upload
}

func signObjectForTest(version *ObjectVersion) {
	fields := signatureFieldsForTest(version.OwnerDeviceID)
	version.SignatureSchemaVersion = fields.SchemaVersion
	version.SignatureAlgorithm = fields.Algorithm
	version.SignatureKeyID = fields.KeyID
	version.Signature = ed25519.Sign(signingPrivateKeyForTest(version.OwnerDeviceID), canonicalSignatureBytes("sync_object_manifest", []signatureField{
		textField("signature_schema_version", "1"),
		textField("signature_algorithm", signatureAlgorithm),
		textField("signature_key_id", fields.KeyID),
		textField("signer_device_id", version.OwnerDeviceID),
		textField("domain_id", version.DomainID),
		textField("object_id", version.ObjectID),
		textField("object_type", version.ObjectType),
		textField("version", uint64String(version.Version)),
		textField("base_version", optionalBaseVersionString(version.BaseVersion)),
		textField("key_id", version.KeyID),
		textField("key_epoch", uint64String(version.KeyEpoch)),
		textField("envelope_algorithm", version.Algorithm),
		bytesField("nonce", version.Nonce),
		textField("encrypted_payload_len", int64String(version.EncryptedPayloadLen)),
		textField("ciphertext_hash", version.CiphertextHash),
		textField("created_at_ms", int64String(version.ClientCreatedAtMs)),
		textField("updated_at_ms", int64String(version.ClientUpdatedAtMs)),
	}))
}

func signAuthorizationForTest(authorization *DeviceAuthorization, wrapping DeviceWrappingRecord, join JoinRequest) {
	fields := signatureFieldsForTest(authorization.AuthorizerDeviceID)
	authorization.SignatureSchemaVersion = fields.SchemaVersion
	authorization.SignatureAlgorithm = fields.Algorithm
	authorization.SignatureKeyID = fields.KeyID
	authorization.Signature = ed25519.Sign(signingPrivateKeyForTest(authorization.AuthorizerDeviceID), canonicalSignatureBytes("device_authorization", []signatureField{
		textField("signature_schema_version", "1"),
		textField("signature_algorithm", signatureAlgorithm),
		textField("signature_key_id", fields.KeyID),
		textField("authorizer_device_id", authorization.AuthorizerDeviceID),
		textField("recipient_device_id", authorization.RecipientDeviceID),
		textField("recipient_public_key_id", authorization.RecipientSigningPublicKeyID),
		bytesField("join_challenge", join.Challenge),
		textField("join_short_code", authorization.JoinShortCode),
		textField("key_epoch", uint64String(authorization.KeyEpoch)),
		textField("wrapping_key_id", wrapping.WrappingKeyID),
		textField("encrypted_key_len", int64String(wrapping.WrappedKeyLen)),
		textField("created_at_ms", int64String(authorization.CreatedAtMs)),
	}))
}

func signRevocationForTest(revocation *DeviceRevocation) {
	fields := signatureFieldsForTest(revocation.RevokerDeviceID)
	revocation.SignatureSchemaVersion = fields.SchemaVersion
	revocation.SignatureAlgorithm = fields.Algorithm
	revocation.SignatureKeyID = fields.KeyID
	revocation.Signature = ed25519.Sign(signingPrivateKeyForTest(revocation.RevokerDeviceID), canonicalSignatureBytes("device_revocation", []signatureField{
		textField("signature_schema_version", "1"),
		textField("signature_algorithm", signatureAlgorithm),
		textField("signature_key_id", fields.KeyID),
		textField("revoked_by_device_id", revocation.RevokerDeviceID),
		textField("revoked_device_id", revocation.RevokedDeviceID),
		textField("previous_key_epoch", uint64String(revocation.PreviousKeyEpoch)),
		textField("new_key_epoch", uint64String(revocation.NewKeyEpoch)),
		textField("reason", revocation.Reason),
		textField("revoked_at_ms", int64String(revocation.CreatedAtMs)),
	}))
}

func signRecoveryForTest(record *RecoveryRecord) {
	fields := signatureFieldsForTest(record.SignerDeviceID)
	record.SignatureSchemaVersion = fields.SchemaVersion
	record.SignatureAlgorithm = fields.Algorithm
	record.SignatureKeyID = fields.KeyID
	record.Signature = ed25519.Sign(signingPrivateKeyForTest(record.SignerDeviceID), canonicalSignatureBytes("recovery_record", []signatureField{
		textField("signature_schema_version", "1"),
		textField("signature_algorithm", signatureAlgorithm),
		textField("signature_key_id", fields.KeyID),
		textField("signer_device_id", record.SignerDeviceID),
		textField("recovery_id", record.RecoveryRecordID),
		textField("domain_id", record.DomainID),
		textField("key_epoch", uint64String(record.KeyEpoch)),
		textField("kdf_id", record.KDFProfile),
		textField("kdf_version", "1"),
		bytesField("salt", record.Salt),
		textField("memory_kib", uint32String(record.MemoryKiB)),
		textField("iterations", uint32String(record.Iterations)),
		textField("parallelism", uint32String(record.Parallelism)),
		textField("output_len", int64String(record.OutputLen)),
		textField("envelope_algorithm", record.Algorithm),
		bytesField("envelope_nonce", record.Nonce),
		textField("encrypted_recovery_key_len", int64String(record.WrappedMaterialLen)),
		textField("created_at_ms", int64String(record.CreatedAtMs)),
		textField("updated_at_ms", int64String(record.CreatedAtMs)),
	}))
}

func signingPublicKeyForTest(deviceID string) []byte {
	return signingPrivateKeyForTest(deviceID).Public().(ed25519.PublicKey)
}

func signingPrivateKeyForTest(deviceID string) ed25519.PrivateKey {
	return ed25519.NewKeyFromSeed(signingSeedForTest(deviceID))
}

func signingSeedForTest(deviceID string) []byte {
	seed := make([]byte, ed25519.SeedSize)
	fill := byte(7)
	if deviceID != "device-a" {
		fill = 11
	}
	for index := range seed {
		seed[index] = fill
	}
	return seed
}

func signingKeyIDForTest(deviceID string) string {
	if deviceID == "device-a" {
		return "signing-key-a"
	}
	return "signing-key-" + deviceID
}

func signatureFieldsForTest(deviceID string) signatureFields {
	return signatureFields{
		SchemaVersion:  signatureSchemaVersion,
		Algorithm:      signatureAlgorithm,
		KeyID:          signingKeyIDForTest(deviceID),
		SignerDeviceID: deviceID,
	}
}

func optionalBaseVersionString(value uint64) string {
	if value == 0 {
		return ""
	}
	return uint64String(value)
}

func uint64String(value uint64) string {
	return strconv.FormatUint(value, 10)
}

func uint32String(value uint32) string {
	return strconv.FormatUint(uint64(value), 10)
}

func int64String(value int64) string {
	return strconv.FormatInt(value, 10)
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
