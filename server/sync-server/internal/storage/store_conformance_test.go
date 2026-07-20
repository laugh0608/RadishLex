package storage

import (
	"bytes"
	"context"
	"crypto/ed25519"
	"crypto/elliptic"
	"strconv"
	"testing"
)

type storeFactory func(t *testing.T) Store

func runStoreConformanceTests(t *testing.T, newStore storeFactory) {
	t.Helper()

	t.Run("requires an explicit supported device signing algorithm", func(t *testing.T) {
		for _, algorithm := range []string{"", "future-signature-v1"} {
			store := newStore(t)
			device := Device{
				DomainID:                "domain-profile",
				DeviceID:                "device-profile",
				SigningAlgorithm:        algorithm,
				SigningPublicKeyID:      "signing-key-profile",
				SigningPublicKey:        signingPublicKeyForTest("device-a"),
				KeyAgreementPublicKeyID: "agreement-key-profile",
				KeyAgreementPublicKey:   []byte{0x02},
				Status:                  DeviceActive,
				AuthorizedAtMs:          10,
			}
			err := store.CreateDomain(context.Background(), Domain{
				DomainID:        "domain-profile",
				CurrentKeyEpoch: 1,
				ActiveKeyID:     "sync-key-profile",
				CreatedAtMs:     10,
				UpdatedAtMs:     10,
			}, device)
			if algorithm == "" && !IsCode(err, ErrInvalidRequest) {
				t.Fatalf("missing algorithm must fail closed, got %v", err)
			}
			if algorithm != "" && !IsCode(err, ErrInvalidSignature) {
				t.Fatalf("unsupported algorithm must fail closed, got %v", err)
			}
		}
	})

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

		stale := objectUpload("domain-a", "object-a", "device-a", 3, 1, 1, []byte{0x94})
		err = putExpectError(store, stale)
		var storageErr *Error
		if !IsCode(err, ErrConflictStaleBaseVersion) {
			t.Fatalf("stale base should return conflict_stale_base_version, got %v", err)
		}
		if !errorAs(err, &storageErr) || storageErr.LatestVersion != 2 || storageErr.LatestCiphertextHash == "" {
			t.Fatalf("stale conflict should include latest metadata, got %#v", storageErr)
		}
	})

	t.Run("discovers committed object versions by stable change sequence", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		first := objectUpload("domain-a", "object-a", "device-a", 1, 0, 1, []byte{0x91})
		createdFirst, err := store.PutObjectVersion(ctx, first)
		if err != nil {
			t.Fatalf("put first version: %v", err)
		}
		if createdFirst.ChangeSequence != 1 {
			t.Fatalf("unexpected first change sequence: %d", createdFirst.ChangeSequence)
		}
		retry, err := store.PutObjectVersion(ctx, first)
		if err != nil || retry.ChangeSequence != createdFirst.ChangeSequence {
			t.Fatalf("idempotent retry changed sequence: metadata=%#v err=%v", retry, err)
		}
		second := objectUpload("domain-a", "object-b", "device-a", 1, 0, 1, []byte{0x92})
		if _, err := store.PutObjectVersion(ctx, second); err != nil {
			t.Fatalf("put second object: %v", err)
		}
		third := objectUpload("domain-a", "object-a", "device-a", 2, 1, 1, []byte{0x93})
		if _, err := store.PutObjectVersion(ctx, third); err != nil {
			t.Fatalf("put third version: %v", err)
		}

		page, err := store.ObjectVersionsAfter(ctx, "domain-a", 0, 2)
		if err != nil {
			t.Fatalf("discover first page: %v", err)
		}
		if len(page) != 2 || page[0].ChangeSequence != 1 || page[1].ChangeSequence != 2 {
			t.Fatalf("unexpected first discovery page: %#v", page)
		}
		next, err := store.ObjectVersionsAfter(ctx, "domain-a", page[1].ChangeSequence, 2)
		if err != nil {
			t.Fatalf("discover next page: %v", err)
		}
		if len(next) != 1 || next[0].ChangeSequence != 3 || next[0].ObjectID != "object-a" || next[0].Version != 2 {
			t.Fatalf("unexpected next discovery page: %#v", next)
		}
	})

	t.Run("discovers signed device lifecycle independently from object changes", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		saveJoinAndAuthorize(t, store, "domain-a", "join-b", "device-b", 20)
		if _, err := store.PutObjectVersion(ctx, objectUpload("domain-a", "object-a", "device-a", 1, 0, 1, []byte{0x91})); err != nil {
			t.Fatalf("put object before revocation: %v", err)
		}
		revocation := DeviceRevocation{
			DomainID: "domain-a", RevokedDeviceID: "device-b", RevokerDeviceID: "device-a",
			PreviousKeyEpoch: 1, NewKeyEpoch: 2, Reason: "lost", CreatedAtMs: 30,
		}
		signRevocationForTest(&revocation)
		if err := store.RevokeDevice(ctx, revocation); err != nil {
			t.Fatalf("revoke lifecycle device: %v", err)
		}

		snapshot, err := store.LifecycleSnapshot(ctx, "domain-a")
		if err != nil {
			t.Fatalf("read lifecycle snapshot: %v", err)
		}
		if snapshot.Domain.CurrentKeyEpoch != 2 || len(snapshot.Events) != 3 {
			t.Fatalf("unexpected lifecycle snapshot: %#v", snapshot)
		}
		if snapshot.Events[0].EventType != LifecycleInitialDevice || snapshot.Events[0].Device == nil || snapshot.Events[0].Device.DeviceID != "device-a" {
			t.Fatalf("unexpected initial lifecycle event: %#v", snapshot.Events[0])
		}
		if snapshot.Events[1].EventType != LifecycleDeviceAuthorized || snapshot.Events[1].Authorization == nil || snapshot.Events[1].Device == nil {
			t.Fatalf("unexpected authorization lifecycle event: %#v", snapshot.Events[1])
		}
		if snapshot.Events[2].EventType != LifecycleDeviceRevoked || snapshot.Events[2].Revocation == nil || snapshot.Events[2].RejectFromObjectChangeSequence != 2 {
			t.Fatalf("unexpected revocation lifecycle event: %#v", snapshot.Events[2])
		}

		firstPage, err := store.LifecycleEventsAfter(ctx, "domain-a", 0, 2)
		if err != nil || len(firstPage) != 2 {
			t.Fatalf("read first lifecycle page: events=%#v err=%v", firstPage, err)
		}
		secondPage, err := store.LifecycleEventsAfter(ctx, "domain-a", firstPage[1].LifecycleSequence, 2)
		if err != nil || len(secondPage) != 1 || secondPage[0].LifecycleSequence != 3 {
			t.Fatalf("read second lifecycle page: events=%#v err=%v", secondPage, err)
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
		if _, _, err := store.DeviceWrappedKey(ctx, "domain-a", "device-b", 1, "wrapping-key-device-b"); !IsCode(err, ErrForbiddenDevice) {
			t.Fatalf("revoked device should be blocked before wrapped key read, got %v", err)
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

	t.Run("atomically distributes current epoch to the complete active cohort", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		saveJoinAndAuthorize(t, store, "domain-a", "join-b", "device-b", 20)
		saveJoinAndAuthorize(t, store, "domain-a", "join-c", "device-c", 25)
		revocation := DeviceRevocation{
			DomainID: "domain-a", RevokedDeviceID: "device-c", RevokerDeviceID: "device-a",
			PreviousKeyEpoch: 1, NewKeyEpoch: 2, Reason: "lost", CreatedAtMs: 30,
		}
		signRevocationForTest(&revocation)
		if err := store.RevokeDevice(ctx, revocation); err != nil {
			t.Fatalf("revoke device before distribution: %v", err)
		}

		upload := epochDistributionUploadForTest(2, "device-a", "device-b")
		partialFailure := upload
		partialFailure.Records = append([]DeviceWrappingUpload(nil), upload.Records...)
		partialFailure.Records[1].Record.Signature = cloneBytes(upload.Records[1].Record.Signature)
		partialFailure.Records[1].Record.Signature[0] ^= 0x80
		if _, err := store.PutEpochDistribution(ctx, partialFailure); !IsCode(err, ErrInvalidSignature) {
			t.Fatalf("invalid record should reject the whole distribution, got %v", err)
		}
		if _, _, err := store.DeviceWrappedKey(ctx, "domain-a", "device-a", 2, "epoch-2-device-a"); !IsCode(err, ErrNotFound) {
			t.Fatalf("partial failure must not expose an accepted record, got %v", err)
		}

		missingRecipient := upload
		missingRecipient.Records = upload.Records[:1]
		if _, err := store.PutEpochDistribution(ctx, missingRecipient); !IsCode(err, ErrInvalidRequest) {
			t.Fatalf("incomplete active cohort must fail, got %v", err)
		}

		result, err := store.PutEpochDistribution(ctx, upload)
		if err != nil {
			t.Fatalf("put complete epoch distribution: %v", err)
		}
		if result.AcceptedRecords != 2 || result.InsertedRecords != 2 || result.KeyEpoch != 2 {
			t.Fatalf("unexpected distribution result: %#v", result)
		}
		retry, err := store.PutEpochDistribution(ctx, upload)
		if err != nil || retry.InsertedRecords != 0 || retry.AcceptedRecords != 2 {
			t.Fatalf("exact distribution retry must be idempotent: result=%#v err=%v", retry, err)
		}
		if _, wrapped, err := store.DeviceWrappedKey(ctx, "domain-a", "device-b", 2, "epoch-2-device-b"); err != nil || string(wrapped) != string(upload.Records[1].WrappedKey) {
			t.Fatalf("active recipient should read distributed material: wrapped=%x err=%v", wrapped, err)
		}
		if _, _, err := store.DeviceWrappedKey(ctx, "domain-a", "device-c", 2, "epoch-2-device-c"); !IsCode(err, ErrForbiddenDevice) {
			t.Fatalf("revoked recipient must remain blocked, got %v", err)
		}

		conflict := upload
		conflict.Records = append([]DeviceWrappingUpload(nil), upload.Records...)
		conflict.Records[1].WrappedKey = []byte{0x99, 0x98, 0x97}
		conflict.Records[1].Record.WrappedKeyLen = int64(len(conflict.Records[1].WrappedKey))
		conflict.Records[1].Record.CiphertextHash = DeviceWrappedKeyCiphertextHash(conflict.Records[1].Record, conflict.Records[1].WrappedKey)
		signEpochDistributionForTest(&conflict.Records[1].Record)
		if _, err := store.PutEpochDistribution(ctx, conflict); !IsCode(err, ErrConflictEpochDistribution) {
			t.Fatalf("same locator with different material must conflict, got %v", err)
		}
	})

	t.Run("join authorization activates device and stores wrapped key bytes", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		request, upload := joinAuthorizationFixture("domain-a", "join-b", "device-b", 20)
		if err := store.SaveJoinRequest(ctx, request); err != nil {
			t.Fatalf("save join request: %v", err)
		}
		pending, err := store.PendingJoinRequests(ctx, "domain-a")
		if err != nil {
			t.Fatalf("list pending join requests: %v", err)
		}
		if len(pending) != 1 || pending[0].JoinRequestID != "join-b" || pending[0].Status != DevicePending {
			t.Fatalf("unexpected pending join requests: %#v", pending)
		}
		if err := store.AuthorizeJoinRequest(ctx, upload); err != nil {
			t.Fatalf("authorize join request: %v", err)
		}
		pending, err = store.PendingJoinRequests(ctx, "domain-a")
		if err != nil {
			t.Fatalf("list pending join requests after authorization: %v", err)
		}
		if len(pending) != 0 {
			t.Fatalf("authorized join request should not remain pending: %#v", pending)
		}

		device, err := store.Device(ctx, "domain-a", "device-b")
		if err != nil {
			t.Fatalf("load authorized device: %v", err)
		}
		if device.Status != DeviceActive || device.AuthorizedAtMs != 20 {
			t.Fatalf("device should be active after authorization: %#v", device)
		}
		wrapping, wrappedKey, err := store.DeviceWrappedKey(ctx, "domain-a", "device-b", 1, "wrapping-key-device-b")
		if err != nil {
			t.Fatalf("read wrapped key: %v", err)
		}
		if wrapping.BlobRef == "" {
			t.Fatal("wrapping record should receive blob ref")
		}
		if wrapping.RecipientKeyAgreementKeyID != "agreement-key-device-b" {
			t.Fatalf("unexpected wrapping recipient key id: %q", wrapping.RecipientKeyAgreementKeyID)
		}
		if string(wrappedKey) != string(wrappedKeyForTest()) {
			t.Fatalf("wrapped key mismatch: got %x want %x", wrappedKey, wrappedKeyForTest())
		}
	})

	t.Run("join authorization rejects wrapped key above resource limit", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		request, upload := joinAuthorizationFixture("domain-a", "join-large", "device-large", 20)
		upload.WrappedKey = make([]byte, MaxDeviceWrappedKeyBytes+1)
		upload.Wrapping.WrappedKeyLen = int64(len(upload.WrappedKey))
		upload.Wrapping.CiphertextHash = DeviceWrappedKeyCiphertextHash(upload.Wrapping, upload.WrappedKey)
		signAuthorizationForTest(&upload.Authorization, upload.Wrapping, request)
		if err := store.SaveJoinRequest(ctx, request); err != nil {
			t.Fatalf("save join request: %v", err)
		}
		if err := store.AuthorizeJoinRequest(ctx, upload); !IsCode(err, ErrInvalidCiphertextMetadata) {
			t.Fatalf("oversized wrapped key should fail, got %v", err)
		}
	})

	t.Run("recovery record stores only wrapped material metadata", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		wrapped := bytes.Repeat([]byte{0xa1}, RecoveryWrappedMaterialBytes)
		record := RecoveryRecord{
			RecordSchemaVersion:   RecoveryRecordSchemaVersionV2,
			DomainID:              "domain-a",
			RecoveryRecordID:      "recovery-a",
			KeyEpoch:              1,
			KDFProfile:            "argon2id-v1",
			KDFVersion:            1,
			MemoryKiB:             65536,
			Iterations:            3,
			Parallelism:           4,
			OutputLen:             32,
			Salt:                  bytes.Repeat([]byte{0x01}, RecoverySaltBytes),
			Algorithm:             AlgorithmXChaCha20Poly1305HKDFSHA256,
			Nonce:                 bytes.Repeat([]byte{0x03}, RecoveryNonceBytes),
			WrappedMaterialLen:    int64(len(wrapped)),
			CiphertextHash:        CiphertextHash(wrapped),
			ActivationAlgorithm:   SignatureAlgorithmEd25519V1,
			ActivationPublicKeyID: "recovery-activation-key-a",
			ActivationPublicKey:   recoveryActivationPrivateKeyForTest("recovery-a").Public().(ed25519.PublicKey),
			Status:                RecoveryRecordActive,
			CreatedAtMs:           40,
			UpdatedAtMs:           40,
			SignerDeviceID:        "device-a",
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
		metadata, material, err := store.LatestRecoveryWrappedMaterial(ctx, "domain-a")
		if err != nil {
			t.Fatalf("latest recovery wrapped material: %v", err)
		}
		if metadata.RecoveryRecordID != "recovery-a" || metadata.BlobRef == "" {
			t.Fatalf("latest recovery wrapped material metadata mismatch: %#v", metadata)
		}
		if string(material) != string(wrapped) {
			t.Fatalf("wrapped material mismatch: got %x want %x", material, wrapped)
		}
	})

	t.Run("recovery rotation is atomic idempotent and predecessor guarded", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		firstWrapped := bytes.Repeat([]byte{0xb1}, RecoveryWrappedMaterialBytes)
		first := recoveryRecordForTest("recovery-a", "", 40, firstWrapped)
		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: first, WrappedMaterial: firstWrapped}); err != nil {
			t.Fatalf("put first recovery: %v", err)
		}
		secondWrapped := bytes.Repeat([]byte{0xc1}, RecoveryWrappedMaterialBytes)
		second := recoveryRecordForTest("recovery-b", "recovery-a", 50, secondWrapped)
		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: second, WrappedMaterial: secondWrapped}); err != nil {
			t.Fatalf("rotate recovery: %v", err)
		}
		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: second, WrappedMaterial: secondWrapped}); err != nil {
			t.Fatalf("idempotent rotation retry: %v", err)
		}
		latest, wrapped, err := store.LatestRecoveryWrappedMaterial(ctx, "domain-a")
		if err != nil {
			t.Fatalf("read rotated recovery: %v", err)
		}
		if latest.RecoveryRecordID != "recovery-b" || string(wrapped) != string(secondWrapped) {
			t.Fatalf("rotated recovery mismatch: %#v %x", latest, wrapped)
		}
		staleWrapped := bytes.Repeat([]byte{0xd1}, RecoveryWrappedMaterialBytes)
		stale := recoveryRecordForTest("recovery-c", "recovery-a", 60, staleWrapped)
		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: stale, WrappedMaterial: staleWrapped}); !IsCode(err, ErrConflictRecoveryRecord) {
			t.Fatalf("stale predecessor should conflict, got %v", err)
		}
		collision := second
		collision.UpdatedAtMs++
		signRecoveryForTest(&collision)
		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: collision, WrappedMaterial: secondWrapped}); !IsCode(err, ErrConflictRecoveryRecord) {
			t.Fatalf("recovery id collision should conflict, got %v", err)
		}
		tampered := append([]byte(nil), secondWrapped...)
		tampered[0] ^= 0xff
		third := recoveryRecordForTest("recovery-d", "recovery-b", 70, secondWrapped)
		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: third, WrappedMaterial: tampered}); !IsCode(err, ErrInvalidCiphertextMetadata) {
			t.Fatalf("tampered wrapped recovery should fail metadata, got %v", err)
		}
	})

	t.Run("recovery revocation is signed idempotent and blocks activation", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		wrapped := bytes.Repeat([]byte{0xe1}, RecoveryWrappedMaterialBytes)
		record := recoveryRecordForTest("recovery-a", "", 40, wrapped)
		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: record, WrappedMaterial: wrapped}); err != nil {
			t.Fatalf("put recovery before revocation: %v", err)
		}
		revocation := recoveryRecordRevocationForTest("recovery-a", 1, "compromised", 50)
		tampered := revocation
		tampered.Reason = "lost"
		if _, err := store.RevokeRecoveryRecord(ctx, tampered); !IsCode(err, ErrInvalidSignature) {
			t.Fatalf("tampered recovery revocation must fail signature, got %v", err)
		}
		if latest, err := store.LatestRecoveryRecord(ctx, "domain-a"); err != nil || latest.RecoveryRecordID != "recovery-a" {
			t.Fatalf("failed revocation must preserve active recovery: record=%#v err=%v", latest, err)
		}
		result, err := store.RevokeRecoveryRecord(ctx, revocation)
		if err != nil {
			t.Fatalf("revoke recovery record: %v", err)
		}
		replayed, err := store.RevokeRecoveryRecord(ctx, revocation)
		if err != nil || replayed.LifecycleSequence != result.LifecycleSequence {
			t.Fatalf("exact revocation replay must be idempotent: result=%#v err=%v", replayed, err)
		}
		divergent := recoveryRecordRevocationForTest("recovery-a", 1, "disable_recovery", 51)
		if _, err := store.RevokeRecoveryRecord(ctx, divergent); !IsCode(err, ErrConflictRecoveryRecord) {
			t.Fatalf("divergent revocation must conflict, got %v", err)
		}
		if _, err := store.LatestRecoveryRecord(ctx, "domain-a"); !IsCode(err, ErrNotFound) {
			t.Fatalf("revoked recovery must disappear from latest, got %v", err)
		}
		activation := recoveredDeviceActivationUploadForTest(record, "device-recovered", 60, "device-a", "device-recovered")
		if _, err := store.RecoverDevice(ctx, activation); !IsCode(err, ErrConflictRecoveryRecord) {
			t.Fatalf("revoked recovery must not activate a device, got %v", err)
		}
		nextWrapped := bytes.Repeat([]byte{0xe2}, RecoveryWrappedMaterialBytes)
		next := recoveryRecordForTest("recovery-b", "recovery-a", 70, nextWrapped)
		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: next, WrappedMaterial: nextWrapped}); err != nil {
			t.Fatalf("rotation may explicitly succeed from revoked chain head: %v", err)
		}
		snapshot, err := store.LifecycleSnapshot(ctx, "domain-a")
		if err != nil {
			t.Fatalf("read revocation lifecycle: %v", err)
		}
		if len(snapshot.Events) != 4 || snapshot.Events[1].EventType != LifecycleRecoveryRecordRotated ||
			snapshot.Events[2].EventType != LifecycleRecoveryRecordRevoked || snapshot.Events[2].RecoveryRevocation == nil ||
			snapshot.Events[3].EventType != LifecycleRecoveryRecordRotated {
			t.Fatalf("unexpected recovery revocation lifecycle: %#v", snapshot.Events)
		}
	})

	t.Run("recovered device activation consumes record and distributes epoch atomically", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		saveJoinAndAuthorize(t, store, "domain-a", "join-b", "device-b", 20)
		saveJoinAndAuthorize(t, store, "domain-a", "join-c", "device-c", 25)
		revocation := DeviceRevocation{
			DomainID: "domain-a", RevokedDeviceID: "device-c", RevokerDeviceID: "device-a",
			PreviousKeyEpoch: 1, NewKeyEpoch: 2, Reason: "lost", CreatedAtMs: 30,
		}
		signRevocationForTest(&revocation)
		if err := store.RevokeDevice(ctx, revocation); err != nil {
			t.Fatalf("revoke device before recovery: %v", err)
		}
		wrapped := bytes.Repeat([]byte{0x51}, RecoveryWrappedMaterialBytes)
		record := recoveryRecordForTest("recovery-a", "", 40, wrapped)
		record.KeyEpoch = 2
		signRecoveryForTest(&record)
		if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: record, WrappedMaterial: wrapped}); err != nil {
			t.Fatalf("put recovery record: %v", err)
		}
		upload := recoveredDeviceActivationUploadForTest(record, "device-recovered", 50, "device-a", "device-b", "device-recovered")

		tampered := upload
		tampered.Activation.ActivationSignature = cloneBytes(upload.Activation.ActivationSignature)
		tampered.Activation.ActivationSignature[0] ^= 0x80
		if _, err := store.RecoverDevice(ctx, tampered); !IsCode(err, ErrInvalidSignature) {
			t.Fatalf("tampered activation must fail, got %v", err)
		}
		if _, err := store.Device(ctx, "domain-a", "device-recovered"); !IsCode(err, ErrNotFound) {
			t.Fatalf("failed activation must not create device, got %v", err)
		}
		if latest, err := store.LatestRecoveryRecord(ctx, "domain-a"); err != nil || latest.RecoveryRecordID != "recovery-a" {
			t.Fatalf("failed activation must not consume recovery: record=%#v err=%v", latest, err)
		}

		incomplete := upload
		incomplete.Distribution.Records = append([]DeviceWrappingUpload(nil), upload.Distribution.Records[:2]...)
		if _, err := store.RecoverDevice(ctx, incomplete); !IsCode(err, ErrInvalidRequest) {
			t.Fatalf("incomplete cohort must fail, got %v", err)
		}
		result, err := store.RecoverDevice(ctx, upload)
		if err != nil {
			t.Fatalf("recover device: %v", err)
		}
		if result.Device.DeviceID != "device-recovered" || result.Device.Status != DeviceActive || result.DistributedRecords != 3 {
			t.Fatalf("unexpected recovery result: %#v", result)
		}
		if _, err := store.LatestRecoveryRecord(ctx, "domain-a"); !IsCode(err, ErrNotFound) {
			t.Fatalf("used recovery record must stop being latest active, got %v", err)
		}
		if _, err := store.RecoverDevice(ctx, upload); !IsCode(err, ErrConflictRecoveryRecord) {
			t.Fatalf("recovery record reuse must conflict, got %v", err)
		}
		for _, recipient := range []string{"device-a", "device-b", "device-recovered"} {
			if _, _, err := store.DeviceWrappedKey(ctx, "domain-a", recipient, 2, "epoch-2-"+recipient); err != nil {
				t.Fatalf("recipient %s must read recovered epoch material: %v", recipient, err)
			}
		}
		if _, _, err := store.DeviceWrappedKey(ctx, "domain-a", "device-c", 2, "epoch-2-device-c"); !IsCode(err, ErrForbiddenDevice) {
			t.Fatalf("revoked device must not obtain recovered epoch, got %v", err)
		}
		snapshot, err := store.LifecycleSnapshot(ctx, "domain-a")
		if err != nil {
			t.Fatalf("read recovery lifecycle: %v", err)
		}
		if len(snapshot.Events) != 6 || snapshot.Events[4].EventType != LifecycleRecoveryRecordRotated ||
			snapshot.Events[4].RecoveryRecord == nil || snapshot.Events[5].EventType != LifecycleDeviceRecovered ||
			snapshot.Events[5].RecoveredActivation == nil || snapshot.Events[5].Device == nil {
			t.Fatalf("unexpected recovery lifecycle: %#v", snapshot.Events)
		}
	})

	t.Run("rejects signed object manifest tampering", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		upload := objectUpload("domain-a", "object-a", "device-a", 1, 0, 1, []byte{0x91})
		upload.Version.Nonce = []byte{0x99}

		if _, err := store.PutObjectVersion(ctx, upload); !IsCode(err, ErrInvalidSignature) {
			t.Fatalf("tampered object manifest should fail signature verification, got %v", err)
		}
	})

	t.Run("rejects signed authorization tampering", func(t *testing.T) {
		ctx := context.Background()
		store := newReadyStore(t, newStore)
		request, upload := joinAuthorizationFixture("domain-a", "join-b", "device-b", 20)
		if err := store.SaveJoinRequest(ctx, request); err != nil {
			t.Fatalf("save join request: %v", err)
		}
		upload.Authorization.JoinShortCode = "654321"

		if err := store.AuthorizeJoinRequest(ctx, upload); !IsCode(err, ErrInvalidSignature) {
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
		wrapped := bytes.Repeat([]byte{0xa1}, RecoveryWrappedMaterialBytes)
		record := RecoveryRecord{
			RecordSchemaVersion:   RecoveryRecordSchemaVersionV2,
			DomainID:              "domain-a",
			RecoveryRecordID:      "recovery-a",
			KeyEpoch:              1,
			KDFProfile:            "argon2id-v1",
			KDFVersion:            1,
			MemoryKiB:             65536,
			Iterations:            3,
			Parallelism:           4,
			OutputLen:             32,
			Salt:                  bytes.Repeat([]byte{0x01}, RecoverySaltBytes),
			Algorithm:             AlgorithmXChaCha20Poly1305HKDFSHA256,
			Nonce:                 bytes.Repeat([]byte{0x03}, RecoveryNonceBytes),
			WrappedMaterialLen:    int64(len(wrapped)),
			CiphertextHash:        CiphertextHash(wrapped),
			ActivationAlgorithm:   SignatureAlgorithmEd25519V1,
			ActivationPublicKeyID: "recovery-activation-key-a",
			ActivationPublicKey:   make([]byte, ed25519.PublicKeySize),
			Status:                RecoveryRecordActive,
			CreatedAtMs:           40,
			UpdatedAtMs:           40,
			SignerDeviceID:        "device-a",
		}
		signRecoveryForTest(&record)
		record.ActivationPublicKeyID = "recovery-activation-key-b"

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
		SigningAlgorithm:        SignatureAlgorithmEd25519V1,
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
	request, upload := joinAuthorizationFixture(domainID, joinID, deviceID, atMs)
	if err := store.SaveJoinRequest(ctx, request); err != nil {
		t.Fatalf("save join request: %v", err)
	}
	if err := store.AuthorizeJoinRequest(ctx, upload); err != nil {
		t.Fatalf("authorize join request: %v", err)
	}
}

func joinAuthorizationFixture(domainID string, joinID string, deviceID string, atMs int64) (JoinRequest, DeviceAuthorizationUpload) {
	request := JoinRequest{
		DomainID:                domainID,
		JoinRequestID:           joinID,
		DeviceID:                deviceID,
		SigningAlgorithm:        SignatureAlgorithmEd25519V1,
		SigningPublicKeyID:      signingKeyIDForTest(deviceID),
		SigningPublicKey:        signingPublicKeyForTest(deviceID),
		KeyAgreementPublicKeyID: "agreement-key-" + deviceID,
		KeyAgreementPublicKey:   []byte{0x12},
		Challenge:               []byte{0x13},
		CreatedAtMs:             atMs - 1,
		ExpiresAtMs:             atMs + 100,
		Status:                  DevicePending,
	}
	wrapped := wrappedKeyForTest()
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
		DomainID:                   domainID,
		RecipientDeviceID:          deviceID,
		RecipientKeyAgreementKeyID: request.KeyAgreementPublicKeyID,
		AuthorizerDeviceID:         "device-a",
		KeyEpoch:                   1,
		WrappingKeyID:              "wrapping-key-" + deviceID,
		Algorithm:                  AlgorithmXChaCha20Poly1305HKDFSHA256,
		Nonce:                      []byte{0x23},
		WrappedKeyLen:              int64(len(wrapped)),
		CiphertextHash:             CiphertextHash(wrapped),
		CreatedAtMs:                atMs,
		Signature:                  []byte{0x24},
	}
	signAuthorizationForTest(&authorization, wrapping, request)
	return request, DeviceAuthorizationUpload{
		Authorization: authorization,
		Wrapping:      wrapping,
		WrappedKey:    wrapped,
	}
}

func wrappedKeyForTest() []byte {
	return []byte{0x21, 0x22}
}

func epochDistributionUploadForTest(keyEpoch uint64, recipients ...string) EpochDistributionUpload {
	records := make([]DeviceWrappingUpload, 0, len(recipients))
	for index, recipient := range recipients {
		wrapped := []byte{byte(0x70 + index), byte(keyEpoch), byte(index + 1)}
		recipientKeyID := "agreement-key-" + recipient
		if recipient == "device-a" {
			recipientKeyID = "agreement-key-a"
		}
		record := DeviceWrappingRecord{
			DomainID:                   "domain-a",
			RecipientDeviceID:          recipient,
			RecipientKeyAgreementKeyID: recipientKeyID,
			AuthorizerDeviceID:         "device-a",
			KeyEpoch:                   keyEpoch,
			WrappingKeyID:              "epoch-" + strconv.FormatUint(keyEpoch, 10) + "-" + recipient,
			Algorithm:                  AlgorithmWrappedEpochP256ECDHV1,
			Nonce:                      []byte{byte(keyEpoch), byte(index + 1)},
			WrappedKeyLen:              int64(len(wrapped)),
			CreatedAtMs:                40,
			SignatureRecordType:        WrappingSignatureEpochDistribution,
		}
		record.CiphertextHash = DeviceWrappedKeyCiphertextHash(record, wrapped)
		signEpochDistributionForTest(&record)
		records = append(records, DeviceWrappingUpload{Record: record, WrappedKey: wrapped})
	}
	return EpochDistributionUpload{
		DomainID:            "domain-a",
		DistributorDeviceID: "device-a",
		KeyEpoch:            keyEpoch,
		Records:             records,
	}
}

func signEpochDistributionForTest(record *DeviceWrappingRecord) {
	fields := signatureFieldsForTest(record.AuthorizerDeviceID)
	record.SignatureSchemaVersion = fields.SchemaVersion
	record.SignatureAlgorithm = fields.Algorithm
	record.SignatureKeyID = fields.KeyID
	record.Signature = ed25519.Sign(signingPrivateKeyForTest(record.AuthorizerDeviceID), canonicalSignatureBytes("epoch_distribution", []signatureField{
		textField("signature_schema_version", "1"),
		textField("signature_algorithm", signatureAlgorithm),
		textField("signature_key_id", fields.KeyID),
		textField("distributor_device_id", record.AuthorizerDeviceID),
		textField("domain_id", record.DomainID),
		textField("recipient_device_id", record.RecipientDeviceID),
		textField("recipient_key_agreement_key_id", record.RecipientKeyAgreementKeyID),
		textField("key_epoch", uint64String(record.KeyEpoch)),
		textField("wrapping_key_id", record.WrappingKeyID),
		textField("envelope_algorithm", record.Algorithm),
		bytesField("envelope_nonce", record.Nonce),
		textField("wrapped_key_len", int64String(record.WrappedKeyLen)),
		textField("ciphertext_hash", record.CiphertextHash),
		textField("created_at_ms", int64String(record.CreatedAtMs)),
	}))
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
			ServerReceivedAtMs:  0,
			ClientCreatedAtMs:   100 + int64(version),
			ClientUpdatedAtMs:   100 + int64(version),
		},
		Payload: payload,
	}
	upload.Version.CiphertextHash = ObjectCiphertextHash(upload.Version, payload)
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
	record.Signature = ed25519.Sign(signingPrivateKeyForTest(record.SignerDeviceID), canonicalSignatureBytes("recovery_record_v2", []signatureField{
		textField("signature_schema_version", "1"),
		textField("signature_algorithm", signatureAlgorithm),
		textField("signature_key_id", fields.KeyID),
		textField("signer_device_id", record.SignerDeviceID),
		textField("record_schema_version", uint16String(record.RecordSchemaVersion)),
		textField("recovery_id", record.RecoveryRecordID),
		textField("previous_recovery_id", record.PreviousRecoveryID),
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
		textField("ciphertext_hash", record.CiphertextHash),
		textField("activation_algorithm", record.ActivationAlgorithm),
		textField("activation_public_key_id", record.ActivationPublicKeyID),
		bytesField("activation_public_key", record.ActivationPublicKey),
		textField("created_at_ms", int64String(record.CreatedAtMs)),
		textField("updated_at_ms", int64String(record.UpdatedAtMs)),
	}))
}

func recoveryRecordRevocationForTest(recoveryID string, keyEpoch uint64, reason string, createdAtMs int64) RecoveryRecordRevocation {
	revocation := RecoveryRecordRevocation{
		RecoveryRecordID: recoveryID, DomainID: "domain-a", RevokerDeviceID: "device-a",
		KeyEpoch: keyEpoch, Reason: reason, CreatedAtMs: createdAtMs,
	}
	fields := signatureFieldsForTest(revocation.RevokerDeviceID)
	revocation.SignatureSchemaVersion = fields.SchemaVersion
	revocation.SignatureAlgorithm = fields.Algorithm
	revocation.SignatureKeyID = fields.KeyID
	revocation.Signature = ed25519.Sign(signingPrivateKeyForTest(revocation.RevokerDeviceID), canonicalSignatureBytes(RecoveryRecordRevocationRecordType, []signatureField{
		textField("signature_schema_version", "1"),
		textField("signature_algorithm", fields.Algorithm),
		textField("signature_key_id", fields.KeyID),
		textField("revoker_device_id", revocation.RevokerDeviceID),
		textField("recovery_record_id", revocation.RecoveryRecordID),
		textField("domain_id", revocation.DomainID),
		textField("key_epoch", uint64String(revocation.KeyEpoch)),
		textField("reason", revocation.Reason),
		textField("created_at_ms", int64String(revocation.CreatedAtMs)),
	}))
	return revocation
}

func recoveryRecordForTest(recoveryID string, previousID string, createdAtMs int64, wrapped []byte) RecoveryRecord {
	profileByte := recoveryID[len(recoveryID)-1]
	record := RecoveryRecord{
		RecordSchemaVersion:   RecoveryRecordSchemaVersionV2,
		DomainID:              "domain-a",
		RecoveryRecordID:      recoveryID,
		PreviousRecoveryID:    previousID,
		KeyEpoch:              1,
		KDFProfile:            "argon2id-v1",
		KDFVersion:            1,
		MemoryKiB:             65536,
		Iterations:            3,
		Parallelism:           4,
		OutputLen:             32,
		Salt:                  bytes.Repeat([]byte{profileByte}, RecoverySaltBytes),
		Algorithm:             AlgorithmXChaCha20Poly1305HKDFSHA256,
		Nonce:                 bytes.Repeat([]byte{profileByte ^ 0x5a}, RecoveryNonceBytes),
		WrappedMaterialLen:    int64(len(wrapped)),
		CiphertextHash:        CiphertextHash(wrapped),
		ActivationAlgorithm:   SignatureAlgorithmEd25519V1,
		ActivationPublicKeyID: "recovery-activation-key-" + recoveryID,
		ActivationPublicKey:   recoveryActivationPrivateKeyForTest(recoveryID).Public().(ed25519.PublicKey),
		Status:                RecoveryRecordActive,
		CreatedAtMs:           createdAtMs,
		UpdatedAtMs:           createdAtMs,
		SignerDeviceID:        "device-a",
	}
	signRecoveryForTest(&record)
	return record
}

func recoveredDeviceActivationUploadForTest(record RecoveryRecord, deviceID string, createdAtMs int64, recipients ...string) RecoveredDeviceActivationUpload {
	activation := RecoveredDeviceActivation{
		RecoveryRecordID: record.RecoveryRecordID, DomainID: record.DomainID, DeviceID: deviceID,
		SigningAlgorithm: SignatureAlgorithmEd25519V1, SigningPublicKeyID: signingKeyIDForTest(deviceID),
		SigningPublicKey: signingPublicKeyForTest(deviceID), KeyAgreementAlgorithm: "p256-ecdh-v1",
		KeyAgreementPublicKeyID: "agreement-key-" + deviceID,
		KeyAgreementPublicKey: elliptic.Marshal(
			elliptic.P256(), elliptic.P256().Params().Gx, elliptic.P256().Params().Gy,
		),
		KeyEpoch: record.KeyEpoch, CreatedAtMs: createdAtMs, SignatureSchemaVersion: 1,
		ActivationAlgorithm: record.ActivationAlgorithm, ActivationPublicKeyID: record.ActivationPublicKeyID,
	}
	activation.ActivationSignature = ed25519.Sign(recoveryActivationPrivateKeyForTest(record.RecoveryRecordID), canonicalSignatureBytes(RecoveredDeviceActivationRecordType, []signatureField{
		textField("signature_schema_version", "1"),
		textField("activation_algorithm", activation.ActivationAlgorithm),
		textField("activation_public_key_id", activation.ActivationPublicKeyID),
		textField("recovery_record_id", activation.RecoveryRecordID),
		textField("domain_id", activation.DomainID),
		textField("device_id", activation.DeviceID),
		textField("signing_algorithm", activation.SigningAlgorithm),
		textField("signing_public_key_id", activation.SigningPublicKeyID),
		bytesField("signing_public_key", activation.SigningPublicKey),
		textField("key_agreement_algorithm", activation.KeyAgreementAlgorithm),
		textField("key_agreement_public_key_id", activation.KeyAgreementPublicKeyID),
		bytesField("key_agreement_public_key", activation.KeyAgreementPublicKey),
		textField("key_epoch", uint64String(activation.KeyEpoch)),
		textField("created_at_ms", int64String(activation.CreatedAtMs)),
	}))
	distribution := epochDistributionUploadForTest(record.KeyEpoch, recipients...)
	distribution.DistributorDeviceID = deviceID
	for index := range distribution.Records {
		distribution.Records[index].Record.AuthorizerDeviceID = deviceID
		distribution.Records[index].Record.CreatedAtMs = createdAtMs
		signEpochDistributionForTest(&distribution.Records[index].Record)
	}
	return RecoveredDeviceActivationUpload{Activation: activation, Distribution: distribution}
}

func recoveryActivationPrivateKeyForTest(recoveryID string) ed25519.PrivateKey {
	return ed25519.NewKeyFromSeed(bytes.Repeat([]byte{recoveryID[len(recoveryID)-1]}, ed25519.SeedSize))
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

func uint16String(value uint16) string {
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
