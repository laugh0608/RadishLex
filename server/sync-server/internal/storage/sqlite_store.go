package storage

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
	"fmt"
)

type SQLiteStore struct {
	db    *sql.DB
	blobs ObjectBlobStore
}

func NewSQLiteStore(db *sql.DB, blobs ObjectBlobStore) (*SQLiteStore, error) {
	if db == nil {
		return nil, newError(ErrInvalidRequest, "sqlite database is required")
	}
	if blobs == nil {
		return nil, newError(ErrInvalidRequest, "object blob store is required")
	}
	if _, err := db.Exec("PRAGMA foreign_keys = ON"); err != nil {
		return nil, newError(ErrStorageUnavailable, "sqlite foreign keys cannot be enabled")
	}
	return &SQLiteStore{db: db, blobs: blobs}, nil
}

func (s *SQLiteStore) RecordAuditEvent(ctx context.Context, event AuditEvent) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	if err := validateAuditEvent(event); err != nil {
		return err
	}
	if _, err := s.db.ExecContext(ctx, `
		INSERT INTO audit_events (
			domain_id, event_type, device_id, object_id, version,
			result_code, bytes, server_time_ms
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
	`, event.DomainID, event.EventType, event.DeviceID, event.ObjectID, int64(event.Version),
		event.ResultCode, event.Bytes, event.ServerTimeMs); err != nil {
		return newError(ErrStorageUnavailable, "audit event cannot be stored")
	}
	return nil
}

func (s *SQLiteStore) CreateDomain(ctx context.Context, domain Domain, firstDevice Device) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	if err := validateDomain(domain); err != nil {
		return err
	}
	if firstDevice.DomainID != domain.DomainID {
		return newError(ErrInvalidRequest, "first device domain must match domain")
	}
	if firstDevice.Status != DeviceActive {
		return newError(ErrInvalidRequest, "first device must be active")
	}
	if err := validateDevice(firstDevice); err != nil {
		return err
	}

	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)

	if _, err := domainTx(ctx, tx, domain.DomainID); err == nil {
		return newError(ErrInvalidRequest, "domain already exists")
	} else if !IsCode(err, ErrNotFound) {
		return err
	}
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO sync_domains (
			domain_id, current_key_epoch, active_key_id, created_at_ms, updated_at_ms
		) VALUES (?, ?, ?, ?, ?)
	`, domain.DomainID, int64(domain.CurrentKeyEpoch), domain.ActiveKeyID, domain.CreatedAtMs, domain.UpdatedAtMs); err != nil {
		return newError(ErrStorageUnavailable, "domain metadata cannot be stored")
	}
	if err := insertDeviceTx(ctx, tx, cloneDevice(firstDevice)); err != nil {
		return err
	}
	if err := insertLifecycleEventTx(ctx, tx, LifecycleEvent{
		DomainID:          domain.DomainID,
		LifecycleSequence: 1,
		EventType:         LifecycleInitialDevice,
		RecordID:          firstDevice.DeviceID,
		KeyEpoch:          domain.CurrentKeyEpoch,
		CreatedAtMs:       domain.CreatedAtMs,
	}); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		return newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return nil
}

func (s *SQLiteStore) Domain(ctx context.Context, domainID string) (Domain, error) {
	if err := checkContext(ctx); err != nil {
		return Domain{}, err
	}
	return domainQuerier(ctx, s.db, domainID)
}

func (s *SQLiteStore) Device(ctx context.Context, domainID string, deviceID string) (Device, error) {
	if err := checkContext(ctx); err != nil {
		return Device{}, err
	}
	return deviceQuerier(ctx, s.db, domainID, deviceID)
}

func (s *SQLiteStore) LifecycleSnapshot(ctx context.Context, domainID string) (LifecycleSnapshot, error) {
	if err := checkContext(ctx); err != nil {
		return LifecycleSnapshot{}, err
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return LifecycleSnapshot{}, newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)
	domain, err := domainTx(ctx, tx, domainID)
	if err != nil {
		return LifecycleSnapshot{}, err
	}
	events, err := lifecycleEventsAfterTx(ctx, tx, domainID, 0, -1)
	if err != nil {
		return LifecycleSnapshot{}, err
	}
	if err := tx.Commit(); err != nil {
		return LifecycleSnapshot{}, newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return LifecycleSnapshot{Domain: domain, Events: events}, nil
}

func (s *SQLiteStore) LifecycleEventsAfter(ctx context.Context, domainID string, afterSequence uint64, limit int) ([]LifecycleEvent, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	if !validOpaqueID(domainID) || limit <= 0 || limit > 201 {
		return nil, newError(ErrInvalidRequest, "lifecycle discovery parameters are invalid")
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return nil, newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)
	if _, err := domainTx(ctx, tx, domainID); err != nil {
		return nil, err
	}
	events, err := lifecycleEventsAfterTx(ctx, tx, domainID, afterSequence, limit)
	if err != nil {
		return nil, err
	}
	if err := tx.Commit(); err != nil {
		return nil, newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return events, nil
}

func (s *SQLiteStore) SaveJoinRequest(ctx context.Context, request JoinRequest) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	if request.Status == "" {
		request.Status = DevicePending
	}
	if err := validateJoinRequest(request); err != nil {
		return err
	}

	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)

	if _, err := domainTx(ctx, tx, request.DomainID); err != nil {
		return err
	}
	if _, err := joinRequestTx(ctx, tx, request.DomainID, request.JoinRequestID); err == nil {
		return newError(ErrInvalidRequest, "join request already exists")
	} else if !IsCode(err, ErrNotFound) {
		return err
	}
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO device_join_requests (
			domain_id, join_request_id, device_id,
			signing_algorithm, signing_public_key_id, signing_public_key,
			key_agreement_public_key_id, key_agreement_public_key,
			challenge, created_at_ms, expires_at_ms, status
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`,
		request.DomainID, request.JoinRequestID, request.DeviceID,
		request.SigningAlgorithm, request.SigningPublicKeyID, cloneBytes(request.SigningPublicKey),
		request.KeyAgreementPublicKeyID, cloneBytes(request.KeyAgreementPublicKey),
		cloneBytes(request.Challenge), request.CreatedAtMs, request.ExpiresAtMs, string(request.Status),
	); err != nil {
		return newError(ErrStorageUnavailable, "join request metadata cannot be stored")
	}
	device := Device{
		DomainID:                request.DomainID,
		DeviceID:                request.DeviceID,
		SigningAlgorithm:        request.SigningAlgorithm,
		SigningPublicKeyID:      request.SigningPublicKeyID,
		SigningPublicKey:        cloneBytes(request.SigningPublicKey),
		KeyAgreementPublicKeyID: request.KeyAgreementPublicKeyID,
		KeyAgreementPublicKey:   cloneBytes(request.KeyAgreementPublicKey),
		Status:                  DevicePending,
	}
	if err := insertDeviceTx(ctx, tx, device); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		return newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return nil
}

func (s *SQLiteStore) PendingJoinRequests(ctx context.Context, domainID string) ([]JoinRequest, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	if _, err := s.Domain(ctx, domainID); err != nil {
		return nil, err
	}
	rows, err := s.db.QueryContext(ctx, `
		SELECT domain_id, join_request_id, device_id,
			signing_algorithm, signing_public_key_id, signing_public_key,
			key_agreement_public_key_id, key_agreement_public_key,
			challenge, created_at_ms, expires_at_ms, status
		FROM device_join_requests
		WHERE domain_id = ? AND status = ?
		ORDER BY created_at_ms, join_request_id
	`, domainID, string(DevicePending))
	if err != nil {
		return nil, newError(ErrStorageUnavailable, "join request metadata cannot be read")
	}
	defer rows.Close()

	var requests []JoinRequest
	for rows.Next() {
		request, err := scanJoinRequestRows(rows)
		if err != nil {
			return nil, err
		}
		requests = append(requests, request)
	}
	if err := rows.Err(); err != nil {
		return nil, newError(ErrStorageUnavailable, "join request metadata cannot be read")
	}
	return requests, nil
}

func (s *SQLiteStore) AuthorizeJoinRequest(ctx context.Context, upload DeviceAuthorizationUpload) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	authorization := upload.Authorization
	wrapping := upload.Wrapping
	wrapping.SignatureRecordType = WrappingSignatureDeviceAuthorization
	wrapping.SignatureSchemaVersion = authorization.SignatureSchemaVersion
	wrapping.SignatureAlgorithm = authorization.SignatureAlgorithm
	wrapping.SignatureKeyID = authorization.SignatureKeyID
	wrapping.Signature = cloneBytes(authorization.Signature)
	upload.Wrapping = wrapping
	if err := validateAuthorization(authorization); err != nil {
		return err
	}
	if err := validateAuthorizationUpload(upload); err != nil {
		return err
	}
	if wrapping.DomainID != authorization.DomainID ||
		wrapping.AuthorizerDeviceID != authorization.AuthorizerDeviceID ||
		wrapping.RecipientDeviceID != authorization.RecipientDeviceID ||
		wrapping.RecipientKeyAgreementKeyID != authorization.RecipientKeyAgreementKeyID ||
		wrapping.KeyEpoch != authorization.KeyEpoch {
		return newError(ErrInvalidRequest, "wrapping record must match authorization")
	}

	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)

	authorizer, err := activeDeviceTx(ctx, tx, authorization.DomainID, authorization.AuthorizerDeviceID)
	if err != nil {
		return err
	}
	join, err := joinRequestTx(ctx, tx, authorization.DomainID, authorization.JoinRequestID)
	if err != nil {
		return err
	}
	if join.Status != DevicePending {
		return newError(ErrForbiddenDevice, "join request is not pending")
	}
	if authorization.CreatedAtMs > join.ExpiresAtMs {
		return newError(ErrForbiddenDevice, "join request expired")
	}
	if authorization.RecipientDeviceID != join.DeviceID ||
		authorization.RecipientSigningPublicKeyID != join.SigningPublicKeyID ||
		authorization.RecipientKeyAgreementKeyID != join.KeyAgreementPublicKeyID {
		return newError(ErrForbiddenDevice, "recipient public key does not match join request")
	}
	if err := verifyAuthorizationSignature(authorization, wrapping, join, authorizer); err != nil {
		return err
	}
	wrapping.BlobRef = wrappingBlobRef(wrapping)
	staged, err := s.blobs.StageObjectBlob(ctx, wrapping.BlobRef, upload.WrappedKey)
	if err != nil {
		return err
	}
	defer cleanupStagedBlob(ctx, staged)
	if _, err := tx.ExecContext(ctx, `
		UPDATE device_join_requests
		SET status = ?
		WHERE domain_id = ? AND join_request_id = ?
	`, string(DeviceActive), authorization.DomainID, authorization.JoinRequestID); err != nil {
		return newError(ErrStorageUnavailable, "join request metadata cannot be updated")
	}
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO device_authorizations (
			domain_id, join_request_id, authorizer_device_id, recipient_device_id,
			recipient_signing_public_key_id, recipient_key_agreement_key_id,
			join_short_code, key_epoch, created_at_ms,
			signature_schema_version, signature_algorithm, signature_key_id, signature
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`,
		authorization.DomainID, authorization.JoinRequestID, authorization.AuthorizerDeviceID, authorization.RecipientDeviceID,
		authorization.RecipientSigningPublicKeyID, authorization.RecipientKeyAgreementKeyID,
		authorization.JoinShortCode, int64(authorization.KeyEpoch), authorization.CreatedAtMs,
		int64(authorization.SignatureSchemaVersion), authorization.SignatureAlgorithm, authorization.SignatureKeyID, cloneBytes(authorization.Signature),
	); err != nil {
		return newError(ErrStorageUnavailable, "authorization metadata cannot be stored")
	}
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO device_wrapping_records (
			domain_id, recipient_device_id, recipient_key_agreement_key_id, authorizer_device_id, key_epoch,
			wrapping_key_id, algorithm, nonce, wrapped_key_len,
			ciphertext_hash, created_at_ms, signature_record_type, signature_schema_version,
			signature_algorithm, signature_key_id, signature, blob_ref
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`,
		wrapping.DomainID, wrapping.RecipientDeviceID, wrapping.RecipientKeyAgreementKeyID, wrapping.AuthorizerDeviceID, int64(wrapping.KeyEpoch),
		wrapping.WrappingKeyID, wrapping.Algorithm, cloneBytes(wrapping.Nonce), wrapping.WrappedKeyLen,
		wrapping.CiphertextHash, wrapping.CreatedAtMs, wrapping.SignatureRecordType, int64(wrapping.SignatureSchemaVersion),
		wrapping.SignatureAlgorithm, wrapping.SignatureKeyID, cloneBytes(wrapping.Signature), wrapping.BlobRef,
	); err != nil {
		return newError(ErrStorageUnavailable, "wrapping metadata cannot be stored")
	}
	if _, err := tx.ExecContext(ctx, `
		UPDATE devices
		SET status = ?, authorized_at_ms = ?
		WHERE domain_id = ? AND device_id = ?
	`, string(DeviceActive), authorization.CreatedAtMs, join.DomainID, join.DeviceID); err != nil {
		return newError(ErrStorageUnavailable, "device metadata cannot be updated")
	}
	lifecycleSequence, err := nextLifecycleSequenceTx(ctx, tx, authorization.DomainID)
	if err != nil {
		return err
	}
	if err := insertLifecycleEventTx(ctx, tx, LifecycleEvent{
		DomainID:          authorization.DomainID,
		LifecycleSequence: lifecycleSequence,
		EventType:         LifecycleDeviceAuthorized,
		RecordID:          authorization.JoinRequestID,
		KeyEpoch:          authorization.KeyEpoch,
		CreatedAtMs:       authorization.CreatedAtMs,
	}); err != nil {
		return err
	}
	if err := staged.Commit(ctx); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		_ = s.blobs.DeleteObjectBlob(context.Background(), wrapping.BlobRef)
		return newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return nil
}

func (s *SQLiteStore) PutEpochDistribution(ctx context.Context, upload EpochDistributionUpload) (EpochDistributionResult, error) {
	if err := checkContext(ctx); err != nil {
		return EpochDistributionResult{}, err
	}
	if err := validateEpochDistributionUpload(upload); err != nil {
		return EpochDistributionResult{}, err
	}

	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return EpochDistributionResult{}, newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)

	domain, err := domainTx(ctx, tx, upload.DomainID)
	if err != nil {
		return EpochDistributionResult{}, err
	}
	if domain.CurrentKeyEpoch != upload.KeyEpoch {
		return EpochDistributionResult{}, newError(ErrInvalidRequest, "epoch distribution must target current domain epoch")
	}
	distributor, err := activeDeviceTx(ctx, tx, upload.DomainID, upload.DistributorDeviceID)
	if err != nil {
		return EpochDistributionResult{}, err
	}
	rows, err := tx.QueryContext(ctx, `
		SELECT device_id, key_agreement_public_key_id
		FROM devices WHERE domain_id = ? AND status = ?
	`, upload.DomainID, string(DeviceActive))
	if err != nil {
		return EpochDistributionResult{}, newError(ErrStorageUnavailable, "active device cohort cannot be read")
	}
	active := make(map[string]string)
	for rows.Next() {
		var deviceID string
		var keyID string
		if err := rows.Scan(&deviceID, &keyID); err != nil {
			_ = rows.Close()
			return EpochDistributionResult{}, newError(ErrStorageUnavailable, "active device cohort cannot be read")
		}
		active[deviceID] = keyID
	}
	if err := rows.Close(); err != nil {
		return EpochDistributionResult{}, newError(ErrStorageUnavailable, "active device cohort cannot be read")
	}
	if len(active) != len(upload.Records) {
		return EpochDistributionResult{}, newError(ErrInvalidRequest, "epoch distribution must cover every active device")
	}

	missing := make([]DeviceWrappingUpload, 0, len(upload.Records))
	for _, item := range upload.Records {
		record := item.Record
		keyID, ok := active[record.RecipientDeviceID]
		if !ok || keyID != record.RecipientKeyAgreementKeyID {
			return EpochDistributionResult{}, newError(ErrForbiddenDevice, "epoch distribution recipient is not active with the signed key")
		}
		if err := verifyEpochDistributionSignature(record, distributor); err != nil {
			return EpochDistributionResult{}, err
		}
		existing, err := wrappingRecordQuerier(ctx, tx, record.DomainID, record.RecipientDeviceID, record.KeyEpoch, record.WrappingKeyID)
		if err == nil {
			existingBytes, readErr := s.blobs.ReadObjectBlob(ctx, existing.BlobRef)
			if readErr != nil || !sameWrappingRecord(existing, record) || !bytes.Equal(existingBytes, item.WrappedKey) {
				return EpochDistributionResult{}, newError(ErrConflictEpochDistribution, "epoch distribution locator already contains different material")
			}
			continue
		}
		if !IsCode(err, ErrNotFound) {
			return EpochDistributionResult{}, err
		}
		record.BlobRef = wrappingBlobRef(record)
		missing = append(missing, DeviceWrappingUpload{Record: record, WrappedKey: cloneBytes(item.WrappedKey)})
	}

	staged := make([]StagedObjectBlob, 0, len(missing))
	defer func() {
		for _, blob := range staged {
			_ = blob.Cleanup(context.Background())
		}
	}()
	for _, item := range missing {
		blob, err := s.blobs.StageObjectBlob(ctx, item.Record.BlobRef, item.WrappedKey)
		if err != nil {
			return EpochDistributionResult{}, err
		}
		staged = append(staged, blob)
	}
	for _, blob := range staged {
		if err := blob.Commit(ctx); err != nil {
			return EpochDistributionResult{}, err
		}
	}
	for _, item := range missing {
		record := item.Record
		if _, err := tx.ExecContext(ctx, `
			INSERT INTO device_wrapping_records (
				domain_id, recipient_device_id, recipient_key_agreement_key_id, authorizer_device_id, key_epoch,
				wrapping_key_id, algorithm, nonce, wrapped_key_len, ciphertext_hash, created_at_ms,
				signature_record_type, signature_schema_version, signature_algorithm, signature_key_id,
				signature, blob_ref
			) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
		`, record.DomainID, record.RecipientDeviceID, record.RecipientKeyAgreementKeyID, record.AuthorizerDeviceID, int64(record.KeyEpoch),
			record.WrappingKeyID, record.Algorithm, cloneBytes(record.Nonce), record.WrappedKeyLen,
			record.CiphertextHash, record.CreatedAtMs, record.SignatureRecordType, int64(record.SignatureSchemaVersion),
			record.SignatureAlgorithm, record.SignatureKeyID, cloneBytes(record.Signature), record.BlobRef); err != nil {
			return EpochDistributionResult{}, newError(ErrStorageUnavailable, "epoch distribution metadata cannot be stored")
		}
	}
	if err := tx.Commit(); err != nil {
		return EpochDistributionResult{}, newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return EpochDistributionResult{
		KeyEpoch:        upload.KeyEpoch,
		AcceptedRecords: len(upload.Records),
		InsertedRecords: len(missing),
	}, nil
}

func (s *SQLiteStore) DeviceWrappedKey(ctx context.Context, domainID string, recipientDeviceID string, keyEpoch uint64, wrappingKeyID string) (DeviceWrappingRecord, []byte, error) {
	if err := checkContext(ctx); err != nil {
		return DeviceWrappingRecord{}, nil, err
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return DeviceWrappingRecord{}, nil, newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)
	if _, err := activeDeviceTx(ctx, tx, domainID, recipientDeviceID); err != nil {
		return DeviceWrappingRecord{}, nil, err
	}
	record, err := wrappingRecordQuerier(ctx, tx, domainID, recipientDeviceID, keyEpoch, wrappingKeyID)
	if err != nil {
		return DeviceWrappingRecord{}, nil, err
	}
	if err := tx.Commit(); err != nil {
		return DeviceWrappingRecord{}, nil, newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	wrappedKey, err := s.blobs.ReadObjectBlob(ctx, record.BlobRef)
	if err != nil {
		if IsCode(err, ErrNotFound) {
			return DeviceWrappingRecord{}, nil, newError(ErrStorageUnavailable, "device wrapped key is missing")
		}
		return DeviceWrappingRecord{}, nil, err
	}
	if int64(len(wrappedKey)) != record.WrappedKeyLen || DeviceWrappedKeyCiphertextHash(record, wrappedKey) != record.CiphertextHash {
		return DeviceWrappingRecord{}, nil, newError(ErrStorageUnavailable, "device wrapped key metadata mismatch")
	}
	return cloneWrappingRecord(record), cloneBytes(wrappedKey), nil
}

func (s *SQLiteStore) RevokeDevice(ctx context.Context, revocation DeviceRevocation) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	if err := validateRevocation(revocation); err != nil {
		return err
	}

	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)

	domain, err := domainTx(ctx, tx, revocation.DomainID)
	if err != nil {
		return err
	}
	revoker, err := activeDeviceTx(ctx, tx, revocation.DomainID, revocation.RevokerDeviceID)
	if err != nil {
		return err
	}
	target, err := deviceTx(ctx, tx, revocation.DomainID, revocation.RevokedDeviceID)
	if err != nil {
		return newError(ErrNotFound, "revoked device not found")
	}
	if target.Status != DeviceActive {
		return newError(ErrForbiddenDevice, "revoked device is not active")
	}
	if revocation.PreviousKeyEpoch != domain.CurrentKeyEpoch {
		return newError(ErrInvalidRequest, "previous key epoch must match domain")
	}
	if revocation.NewKeyEpoch <= domain.CurrentKeyEpoch {
		return newError(ErrInvalidRequest, "new key epoch must advance domain")
	}
	if err := verifyRevocationSignature(revocation, revoker); err != nil {
		return err
	}
	rejectFromObjectSequence, err := nextObjectChangeSequenceTx(ctx, tx, revocation.DomainID)
	if err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `
		UPDATE devices
		SET status = ?, revoked_at_ms = ?
		WHERE domain_id = ? AND device_id = ?
	`, string(DeviceRevoked), revocation.CreatedAtMs, revocation.DomainID, revocation.RevokedDeviceID); err != nil {
		return newError(ErrStorageUnavailable, "device revocation metadata cannot be stored")
	}
	if _, err := tx.ExecContext(ctx, `
		UPDATE sync_domains
		SET current_key_epoch = ?, updated_at_ms = ?
		WHERE domain_id = ?
	`, int64(revocation.NewKeyEpoch), revocation.CreatedAtMs, revocation.DomainID); err != nil {
		return newError(ErrStorageUnavailable, "domain key epoch cannot be updated")
	}
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO device_revocations (
			domain_id, revoked_device_id, revoker_device_id,
			previous_key_epoch, new_key_epoch, reason, created_at_ms,
			signature_schema_version, signature_algorithm, signature_key_id, signature
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`,
		revocation.DomainID, revocation.RevokedDeviceID, revocation.RevokerDeviceID,
		int64(revocation.PreviousKeyEpoch), int64(revocation.NewKeyEpoch),
		revocation.Reason, revocation.CreatedAtMs,
		int64(revocation.SignatureSchemaVersion), revocation.SignatureAlgorithm, revocation.SignatureKeyID, cloneBytes(revocation.Signature),
	); err != nil {
		return newError(ErrStorageUnavailable, "revocation metadata cannot be stored")
	}
	lifecycleSequence, err := nextLifecycleSequenceTx(ctx, tx, revocation.DomainID)
	if err != nil {
		return err
	}
	if err := insertLifecycleEventTx(ctx, tx, LifecycleEvent{
		DomainID:                       revocation.DomainID,
		LifecycleSequence:              lifecycleSequence,
		EventType:                      LifecycleDeviceRevoked,
		RecordID:                       fmt.Sprintf("%s:%d", revocation.RevokedDeviceID, revocation.NewKeyEpoch),
		KeyEpoch:                       revocation.NewKeyEpoch,
		RejectFromObjectChangeSequence: rejectFromObjectSequence,
		CreatedAtMs:                    revocation.CreatedAtMs,
	}); err != nil {
		return err
	}
	if err := tx.Commit(); err != nil {
		return newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return nil
}

func (s *SQLiteStore) PutObjectVersion(ctx context.Context, upload ObjectVersionUpload) (ObjectVersion, error) {
	if err := checkContext(ctx); err != nil {
		return ObjectVersion{}, err
	}
	if err := validateObjectUpload(upload); err != nil {
		return ObjectVersion{}, err
	}

	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return ObjectVersion{}, newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)

	domain, err := domainTx(ctx, tx, upload.Version.DomainID)
	if err != nil {
		return ObjectVersion{}, err
	}
	signer, err := activeDeviceTx(ctx, tx, upload.Version.DomainID, upload.Version.OwnerDeviceID)
	if err != nil {
		return ObjectVersion{}, err
	}
	if upload.Version.KeyEpoch < domain.CurrentKeyEpoch {
		return ObjectVersion{}, newError(ErrForbiddenDevice, "object key epoch is older than domain")
	}
	if err := verifyObjectSignature(upload.Version, signer); err != nil {
		return ObjectVersion{}, err
	}

	existing, err := objectVersionTx(ctx, tx, upload.Version.DomainID, upload.Version.ObjectID, upload.Version.Version)
	if err == nil {
		if existing.CiphertextHash == upload.Version.CiphertextHash {
			if err := tx.Commit(); err != nil {
				return ObjectVersion{}, newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
			}
			return existing, nil
		}
		return ObjectVersion{}, newError(ErrConflictObjectVersion, "object version exists with different ciphertext hash")
	}
	if !IsCode(err, ErrNotFound) {
		return ObjectVersion{}, err
	}

	object, exists, err := syncObjectTx(ctx, tx, upload.Version.DomainID, upload.Version.ObjectID)
	if err != nil {
		return ObjectVersion{}, err
	}
	if !exists {
		if upload.Version.Version != 1 || upload.Version.BaseVersion != 0 {
			return ObjectVersion{}, newError(ErrInvalidRequest, "new object must start at version 1 with base version 0")
		}
		object = SyncObject{
			DomainID:    upload.Version.DomainID,
			ObjectID:    upload.Version.ObjectID,
			ObjectType:  upload.Version.ObjectType,
			CreatedAtMs: upload.Version.ClientCreatedAtMs,
		}
	} else {
		if upload.Version.ObjectType != object.ObjectType {
			return ObjectVersion{}, newError(ErrInvalidRequest, "object type cannot change")
		}
		if upload.Version.BaseVersion < object.LatestVersion {
			return ObjectVersion{}, conflictStaleBaseVersion(object.LatestVersion, object.LatestCiphertextHash)
		}
		if upload.Version.BaseVersion > object.LatestVersion {
			return ObjectVersion{}, newError(ErrInvalidRequest, "base version cannot exceed latest version")
		}
		if upload.Version.Version != object.LatestVersion+1 {
			return ObjectVersion{}, newError(ErrInvalidRequest, "object version must advance by one")
		}
	}

	version := cloneObjectVersion(upload.Version)
	version.ChangeSequence, err = nextObjectChangeSequenceTx(ctx, tx, version.DomainID)
	if err != nil {
		return ObjectVersion{}, err
	}
	version.BlobRef = objectBlobRef(version)
	staged, err := s.blobs.StageObjectBlob(ctx, version.BlobRef, upload.Payload)
	if err != nil {
		return ObjectVersion{}, err
	}
	defer cleanupStagedBlob(ctx, staged)

	object.LatestVersion = version.Version
	object.LatestCiphertextHash = version.CiphertextHash
	object.LatestKeyEpoch = version.KeyEpoch
	object.LatestChangeSequence = version.ChangeSequence
	object.UpdatedAtMs = version.ClientUpdatedAtMs
	if exists {
		if err := updateSyncObjectTx(ctx, tx, object); err != nil {
			return ObjectVersion{}, err
		}
	} else {
		if err := insertSyncObjectTx(ctx, tx, object); err != nil {
			return ObjectVersion{}, err
		}
	}
	if err := insertObjectVersionTx(ctx, tx, version); err != nil {
		return ObjectVersion{}, err
	}
	if err := staged.Commit(ctx); err != nil {
		return ObjectVersion{}, err
	}
	if err := tx.Commit(); err != nil {
		_ = s.blobs.DeleteObjectBlob(context.Background(), version.BlobRef)
		return ObjectVersion{}, newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return cloneObjectVersion(version), nil
}

func (s *SQLiteStore) ObjectVersionsAfter(ctx context.Context, domainID string, afterSequence uint64, limit int) ([]ObjectVersion, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	if !validOpaqueID(domainID) || limit <= 0 || limit > 201 {
		return nil, newError(ErrInvalidRequest, "object discovery parameters are invalid")
	}
	if _, err := s.Domain(ctx, domainID); err != nil {
		return nil, err
	}
	rows, err := s.db.QueryContext(ctx, `
		SELECT domain_id, object_id, object_type, version, base_version, change_sequence,
			owner_device_id, key_id, key_epoch, algorithm, nonce,
			encrypted_payload_len, ciphertext_hash,
			signature_schema_version, signature_algorithm, signature_key_id, signature,
			server_received_at_ms, client_created_at_ms, client_updated_at_ms, blob_ref
		FROM sync_object_versions
		WHERE domain_id = ? AND change_sequence > ?
		ORDER BY change_sequence
		LIMIT ?
	`, domainID, int64(afterSequence), limit)
	if err != nil {
		return nil, newError(ErrStorageUnavailable, "object discovery metadata cannot be read")
	}
	defer rows.Close()
	versions := make([]ObjectVersion, 0, limit)
	for rows.Next() {
		version, err := objectVersionFromRow(rows)
		if err != nil {
			return nil, err
		}
		versions = append(versions, version)
	}
	if err := rows.Err(); err != nil {
		return nil, newError(ErrStorageUnavailable, "object discovery metadata cannot be read")
	}
	return versions, nil
}

func (s *SQLiteStore) ObjectVersion(ctx context.Context, domainID string, objectID string, version uint64) (ObjectVersion, error) {
	if err := checkContext(ctx); err != nil {
		return ObjectVersion{}, err
	}
	return objectVersionQuerier(ctx, s.db, domainID, objectID, version)
}

func (s *SQLiteStore) ObjectPayload(ctx context.Context, domainID string, objectID string, version uint64) ([]byte, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	metadata, err := objectVersionQuerier(ctx, s.db, domainID, objectID, version)
	if err != nil {
		return nil, err
	}
	payload, err := s.blobs.ReadObjectBlob(ctx, metadata.BlobRef)
	if err != nil {
		if IsCode(err, ErrNotFound) {
			return nil, newError(ErrStorageUnavailable, "object payload is missing")
		}
		return nil, err
	}
	if int64(len(payload)) != metadata.EncryptedPayloadLen || ObjectCiphertextHash(metadata, payload) != metadata.CiphertextHash {
		return nil, newError(ErrStorageUnavailable, "object payload metadata mismatch")
	}
	return cloneBytes(payload), nil
}

type sqlRow interface {
	Scan(dest ...any) error
}

type sqlQuerier interface {
	QueryRowContext(ctx context.Context, query string, args ...any) *sql.Row
}

func domainQuerier(ctx context.Context, querier sqlQuerier, domainID string) (Domain, error) {
	return domainFromRow(querier.QueryRowContext(ctx, `
		SELECT domain_id, current_key_epoch, active_key_id, created_at_ms, updated_at_ms
		FROM sync_domains
		WHERE domain_id = ?
	`, domainID))
}

func domainTx(ctx context.Context, tx *sql.Tx, domainID string) (Domain, error) {
	return domainQuerier(ctx, tx, domainID)
}

func domainFromRow(row sqlRow) (Domain, error) {
	var domain Domain
	var currentKeyEpoch int64
	if err := row.Scan(&domain.DomainID, &currentKeyEpoch, &domain.ActiveKeyID, &domain.CreatedAtMs, &domain.UpdatedAtMs); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return Domain{}, newError(ErrNotFound, "domain not found")
		}
		return Domain{}, newError(ErrStorageUnavailable, "domain metadata cannot be read")
	}
	domain.CurrentKeyEpoch = uint64(currentKeyEpoch)
	return domain, nil
}

func deviceQuerier(ctx context.Context, querier sqlQuerier, domainID string, deviceID string) (Device, error) {
	return deviceFromRow(querier.QueryRowContext(ctx, `
		SELECT domain_id, device_id, signing_algorithm, signing_public_key_id, signing_public_key,
			key_agreement_public_key_id, key_agreement_public_key,
			status, authorized_at_ms, revoked_at_ms, last_seen_at_ms
		FROM devices
		WHERE domain_id = ? AND device_id = ?
	`, domainID, deviceID))
}

func deviceTx(ctx context.Context, tx *sql.Tx, domainID string, deviceID string) (Device, error) {
	return deviceQuerier(ctx, tx, domainID, deviceID)
}

func activeDeviceTx(ctx context.Context, tx *sql.Tx, domainID string, deviceID string) (Device, error) {
	device, err := deviceTx(ctx, tx, domainID, deviceID)
	if err != nil {
		if IsCode(err, ErrNotFound) {
			return Device{}, newError(ErrForbiddenDevice, "device is not registered")
		}
		return Device{}, err
	}
	if device.Status != DeviceActive {
		return Device{}, newError(ErrForbiddenDevice, "device is not active")
	}
	return device, nil
}

func deviceFromRow(row sqlRow) (Device, error) {
	var device Device
	var status string
	if err := row.Scan(
		&device.DomainID, &device.DeviceID, &device.SigningAlgorithm, &device.SigningPublicKeyID, &device.SigningPublicKey,
		&device.KeyAgreementPublicKeyID, &device.KeyAgreementPublicKey,
		&status, &device.AuthorizedAtMs, &device.RevokedAtMs, &device.LastSeenAtMs,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return Device{}, newError(ErrNotFound, "device not found")
		}
		return Device{}, newError(ErrStorageUnavailable, "device metadata cannot be read")
	}
	device.Status = DeviceStatus(status)
	return cloneDevice(device), nil
}

func insertDeviceTx(ctx context.Context, tx *sql.Tx, device Device) error {
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO devices (
			domain_id, device_id, signing_algorithm, signing_public_key_id, signing_public_key,
			key_agreement_public_key_id, key_agreement_public_key,
			status, authorized_at_ms, revoked_at_ms, last_seen_at_ms
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`,
		device.DomainID, device.DeviceID, device.SigningAlgorithm, device.SigningPublicKeyID, cloneBytes(device.SigningPublicKey),
		device.KeyAgreementPublicKeyID, cloneBytes(device.KeyAgreementPublicKey),
		string(device.Status), device.AuthorizedAtMs, device.RevokedAtMs, device.LastSeenAtMs,
	); err != nil {
		return newError(ErrStorageUnavailable, "device metadata cannot be stored")
	}
	return nil
}

func joinRequestTx(ctx context.Context, tx *sql.Tx, domainID string, joinRequestID string) (JoinRequest, error) {
	row := tx.QueryRowContext(ctx, `
		SELECT domain_id, join_request_id, device_id,
			signing_algorithm, signing_public_key_id, signing_public_key,
			key_agreement_public_key_id, key_agreement_public_key,
			challenge, created_at_ms, expires_at_ms, status
		FROM device_join_requests
		WHERE domain_id = ? AND join_request_id = ?
	`, domainID, joinRequestID)
	var request JoinRequest
	var status string
	if err := row.Scan(
		&request.DomainID, &request.JoinRequestID, &request.DeviceID,
		&request.SigningAlgorithm, &request.SigningPublicKeyID, &request.SigningPublicKey,
		&request.KeyAgreementPublicKeyID, &request.KeyAgreementPublicKey,
		&request.Challenge, &request.CreatedAtMs, &request.ExpiresAtMs, &status,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return JoinRequest{}, newError(ErrNotFound, "join request not found")
		}
		return JoinRequest{}, newError(ErrStorageUnavailable, "join request metadata cannot be read")
	}
	request.Status = DeviceStatus(status)
	return cloneJoinRequest(request), nil
}

func scanJoinRequestRows(rows *sql.Rows) (JoinRequest, error) {
	var request JoinRequest
	var status string
	if err := rows.Scan(
		&request.DomainID, &request.JoinRequestID, &request.DeviceID,
		&request.SigningAlgorithm, &request.SigningPublicKeyID, &request.SigningPublicKey,
		&request.KeyAgreementPublicKeyID, &request.KeyAgreementPublicKey,
		&request.Challenge, &request.CreatedAtMs, &request.ExpiresAtMs, &status,
	); err != nil {
		return JoinRequest{}, newError(ErrStorageUnavailable, "join request metadata cannot be read")
	}
	request.Status = DeviceStatus(status)
	return cloneJoinRequest(request), nil
}

func wrappingRecordQuerier(ctx context.Context, querier sqlQuerier, domainID string, recipientDeviceID string, keyEpoch uint64, wrappingKeyID string) (DeviceWrappingRecord, error) {
	row := querier.QueryRowContext(ctx, `
		SELECT domain_id, recipient_device_id, recipient_key_agreement_key_id, authorizer_device_id, key_epoch,
			wrapping_key_id, algorithm, nonce, wrapped_key_len,
			ciphertext_hash, created_at_ms, signature_record_type, signature_schema_version,
			signature_algorithm, signature_key_id, signature, blob_ref
		FROM device_wrapping_records
		WHERE domain_id = ? AND recipient_device_id = ? AND key_epoch = ? AND wrapping_key_id = ?
	`, domainID, recipientDeviceID, int64(keyEpoch), wrappingKeyID)
	var record DeviceWrappingRecord
	var keyEpochValue int64
	var signatureSchemaVersion int64
	if err := row.Scan(
		&record.DomainID, &record.RecipientDeviceID, &record.RecipientKeyAgreementKeyID, &record.AuthorizerDeviceID, &keyEpochValue,
		&record.WrappingKeyID, &record.Algorithm, &record.Nonce, &record.WrappedKeyLen,
		&record.CiphertextHash, &record.CreatedAtMs, &record.SignatureRecordType, &signatureSchemaVersion,
		&record.SignatureAlgorithm, &record.SignatureKeyID, &record.Signature, &record.BlobRef,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return DeviceWrappingRecord{}, newError(ErrNotFound, "device wrapping record not found")
		}
		return DeviceWrappingRecord{}, newError(ErrStorageUnavailable, "device wrapping metadata cannot be read")
	}
	record.KeyEpoch = uint64(keyEpochValue)
	record.SignatureSchemaVersion = uint16(signatureSchemaVersion)
	return cloneWrappingRecord(record), nil
}

func syncObjectTx(ctx context.Context, tx *sql.Tx, domainID string, objectID string) (SyncObject, bool, error) {
	row := tx.QueryRowContext(ctx, `
		SELECT domain_id, object_id, object_type, latest_version,
			latest_ciphertext_hash, latest_key_epoch, latest_change_sequence, created_at_ms, updated_at_ms
		FROM sync_objects
		WHERE domain_id = ? AND object_id = ?
	`, domainID, objectID)
	var object SyncObject
	var latestVersion int64
	var latestKeyEpoch int64
	var latestChangeSequence int64
	if err := row.Scan(
		&object.DomainID, &object.ObjectID, &object.ObjectType, &latestVersion,
		&object.LatestCiphertextHash, &latestKeyEpoch, &latestChangeSequence, &object.CreatedAtMs, &object.UpdatedAtMs,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return SyncObject{}, false, nil
		}
		return SyncObject{}, false, newError(ErrStorageUnavailable, "object metadata cannot be read")
	}
	object.LatestVersion = uint64(latestVersion)
	object.LatestKeyEpoch = uint64(latestKeyEpoch)
	object.LatestChangeSequence = uint64(latestChangeSequence)
	return object, true, nil
}

func insertSyncObjectTx(ctx context.Context, tx *sql.Tx, object SyncObject) error {
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO sync_objects (
			domain_id, object_id, object_type, latest_version,
			latest_ciphertext_hash, latest_key_epoch, latest_change_sequence, created_at_ms, updated_at_ms
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
	`,
		object.DomainID, object.ObjectID, object.ObjectType, int64(object.LatestVersion),
		object.LatestCiphertextHash, int64(object.LatestKeyEpoch), int64(object.LatestChangeSequence), object.CreatedAtMs, object.UpdatedAtMs,
	); err != nil {
		return newError(ErrStorageUnavailable, "object metadata cannot be stored")
	}
	return nil
}

func updateSyncObjectTx(ctx context.Context, tx *sql.Tx, object SyncObject) error {
	if _, err := tx.ExecContext(ctx, `
		UPDATE sync_objects
		SET latest_version = ?, latest_ciphertext_hash = ?, latest_key_epoch = ?, latest_change_sequence = ?, updated_at_ms = ?
		WHERE domain_id = ? AND object_id = ?
	`,
		int64(object.LatestVersion), object.LatestCiphertextHash, int64(object.LatestKeyEpoch), int64(object.LatestChangeSequence), object.UpdatedAtMs,
		object.DomainID, object.ObjectID,
	); err != nil {
		return newError(ErrStorageUnavailable, "object metadata cannot be updated")
	}
	return nil
}

func objectVersionQuerier(ctx context.Context, querier sqlQuerier, domainID string, objectID string, version uint64) (ObjectVersion, error) {
	return objectVersionFromRow(querier.QueryRowContext(ctx, `
		SELECT domain_id, object_id, object_type, version, base_version, change_sequence,
			owner_device_id, key_id, key_epoch, algorithm, nonce,
			encrypted_payload_len, ciphertext_hash,
			signature_schema_version, signature_algorithm, signature_key_id, signature,
			server_received_at_ms, client_created_at_ms, client_updated_at_ms, blob_ref
		FROM sync_object_versions
		WHERE domain_id = ? AND object_id = ? AND version = ?
	`, domainID, objectID, int64(version)))
}

func objectVersionTx(ctx context.Context, tx *sql.Tx, domainID string, objectID string, version uint64) (ObjectVersion, error) {
	return objectVersionQuerier(ctx, tx, domainID, objectID, version)
}

func objectVersionFromRow(row sqlRow) (ObjectVersion, error) {
	var version ObjectVersion
	var versionNumber int64
	var baseVersion int64
	var changeSequence int64
	var keyEpoch int64
	var signatureSchemaVersion int64
	if err := row.Scan(
		&version.DomainID, &version.ObjectID, &version.ObjectType, &versionNumber, &baseVersion, &changeSequence,
		&version.OwnerDeviceID, &version.KeyID, &keyEpoch, &version.Algorithm, &version.Nonce,
		&version.EncryptedPayloadLen, &version.CiphertextHash,
		&signatureSchemaVersion, &version.SignatureAlgorithm, &version.SignatureKeyID, &version.Signature,
		&version.ServerReceivedAtMs, &version.ClientCreatedAtMs, &version.ClientUpdatedAtMs, &version.BlobRef,
	); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return ObjectVersion{}, newError(ErrNotFound, "object version not found")
		}
		return ObjectVersion{}, newError(ErrStorageUnavailable, "object version metadata cannot be read")
	}
	version.Version = uint64(versionNumber)
	version.BaseVersion = uint64(baseVersion)
	version.ChangeSequence = uint64(changeSequence)
	version.KeyEpoch = uint64(keyEpoch)
	version.SignatureSchemaVersion = uint16(signatureSchemaVersion)
	return cloneObjectVersion(version), nil
}

func insertObjectVersionTx(ctx context.Context, tx *sql.Tx, version ObjectVersion) error {
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO sync_object_versions (
			domain_id, object_id, object_type, version, base_version, change_sequence,
			owner_device_id, key_id, key_epoch, algorithm, nonce,
			encrypted_payload_len, ciphertext_hash,
			signature_schema_version, signature_algorithm, signature_key_id, signature,
			server_received_at_ms, client_created_at_ms, client_updated_at_ms, blob_ref
		) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`,
		version.DomainID, version.ObjectID, version.ObjectType, int64(version.Version), int64(version.BaseVersion), int64(version.ChangeSequence),
		version.OwnerDeviceID, version.KeyID, int64(version.KeyEpoch), version.Algorithm, cloneBytes(version.Nonce),
		version.EncryptedPayloadLen, version.CiphertextHash,
		int64(version.SignatureSchemaVersion), version.SignatureAlgorithm, version.SignatureKeyID, cloneBytes(version.Signature),
		version.ServerReceivedAtMs, version.ClientCreatedAtMs, version.ClientUpdatedAtMs, version.BlobRef,
	); err != nil {
		return newError(ErrStorageUnavailable, "object version metadata cannot be stored")
	}
	return nil
}

func lifecycleEventsAfterTx(ctx context.Context, tx *sql.Tx, domainID string, afterSequence uint64, limit int) ([]LifecycleEvent, error) {
	rows, err := tx.QueryContext(ctx, `
		SELECT domain_id, lifecycle_sequence, event_type, record_id, key_epoch,
			reject_from_object_change_sequence, created_at_ms
		FROM domain_lifecycle_events
		WHERE domain_id = ? AND lifecycle_sequence > ?
		ORDER BY lifecycle_sequence
		LIMIT ?
	`, domainID, int64(afterSequence), limit)
	if err != nil {
		return nil, newError(ErrStorageUnavailable, "lifecycle metadata cannot be read")
	}
	var events []LifecycleEvent
	for rows.Next() {
		var event LifecycleEvent
		var sequence int64
		var eventType string
		var keyEpoch int64
		var rejectFrom int64
		if err := rows.Scan(
			&event.DomainID, &sequence, &eventType, &event.RecordID, &keyEpoch,
			&rejectFrom, &event.CreatedAtMs,
		); err != nil {
			_ = rows.Close()
			return nil, newError(ErrStorageUnavailable, "lifecycle metadata cannot be read")
		}
		if sequence <= 0 || keyEpoch <= 0 || rejectFrom < 0 {
			_ = rows.Close()
			return nil, newError(ErrStorageUnavailable, "lifecycle metadata is invalid")
		}
		event.LifecycleSequence = uint64(sequence)
		event.EventType = LifecycleEventType(eventType)
		event.KeyEpoch = uint64(keyEpoch)
		event.RejectFromObjectChangeSequence = uint64(rejectFrom)
		events = append(events, event)
	}
	if err := rows.Close(); err != nil {
		return nil, newError(ErrStorageUnavailable, "lifecycle metadata cannot be read")
	}
	for index := range events {
		if err := hydrateLifecycleEventTx(ctx, tx, &events[index]); err != nil {
			return nil, err
		}
	}
	return events, nil
}

func hydrateLifecycleEventTx(ctx context.Context, tx *sql.Tx, event *LifecycleEvent) error {
	switch event.EventType {
	case LifecycleInitialDevice:
		device, err := deviceTx(ctx, tx, event.DomainID, event.RecordID)
		if err != nil {
			return newError(ErrStorageUnavailable, "initial lifecycle device cannot be read")
		}
		device.Status = DeviceActive
		device.AuthorizedAtMs = event.CreatedAtMs
		device.RevokedAtMs = 0
		event.Device = devicePointer(device)
	case LifecycleDeviceAuthorized:
		authorization, err := authorizationTx(ctx, tx, event.DomainID, event.RecordID)
		if err != nil {
			return err
		}
		device, err := deviceTx(ctx, tx, event.DomainID, authorization.RecipientDeviceID)
		if err != nil {
			return newError(ErrStorageUnavailable, "authorized lifecycle device cannot be read")
		}
		device.Status = DeviceActive
		device.AuthorizedAtMs = authorization.CreatedAtMs
		device.RevokedAtMs = 0
		join, err := joinRequestTx(ctx, tx, event.DomainID, authorization.JoinRequestID)
		if err != nil {
			return newError(ErrStorageUnavailable, "authorization join request cannot be read")
		}
		wrapping, err := wrappingForAuthorizationTx(ctx, tx, authorization)
		if err != nil {
			return err
		}
		event.Device = devicePointer(device)
		event.JoinRequest = joinRequestPointer(join)
		event.Authorization = authorizationPointer(authorization)
		event.Wrapping = wrappingPointer(wrapping)
	case LifecycleDeviceRevoked:
		revocation, err := revocationByRecordIDTx(ctx, tx, event.DomainID, event.RecordID)
		if err != nil {
			return err
		}
		device, err := deviceTx(ctx, tx, event.DomainID, revocation.RevokedDeviceID)
		if err != nil {
			return newError(ErrStorageUnavailable, "revoked lifecycle device cannot be read")
		}
		event.Device = devicePointer(device)
		event.Revocation = revocationPointer(revocation)
	case LifecycleRecoveryRecordRotated:
		record, err := recoveryRecordQuerier(ctx, tx, event.DomainID, event.RecordID)
		if err != nil {
			return newError(ErrStorageUnavailable, "recovery lifecycle metadata cannot be read")
		}
		signer, err := deviceTx(ctx, tx, event.DomainID, record.SignerDeviceID)
		if err != nil {
			return newError(ErrStorageUnavailable, "recovery lifecycle signer cannot be read")
		}
		signer.Status = DeviceActive
		signer.RevokedAtMs = 0
		event.Device = devicePointer(signer)
		event.RecoveryRecord = recoveryRecordPointer(record)
	case LifecycleDeviceRecovered:
		activation, err := recoveredDeviceActivationTx(ctx, tx, event.DomainID, event.RecordID)
		if err != nil {
			return err
		}
		device, err := deviceTx(ctx, tx, event.DomainID, activation.DeviceID)
		if err != nil {
			return newError(ErrStorageUnavailable, "recovered lifecycle device cannot be read")
		}
		event.Device = devicePointer(device)
		event.RecoveredActivation = recoveredActivationPointer(activation)
	case LifecycleRecoveryRecordRevoked:
		revocation, err := recoveryRecordRevocationQuerier(ctx, tx, event.DomainID, event.RecordID)
		if err != nil {
			return newError(ErrStorageUnavailable, "recovery revocation lifecycle metadata cannot be read")
		}
		revoker, err := deviceTx(ctx, tx, event.DomainID, revocation.RevokerDeviceID)
		if err != nil {
			return newError(ErrStorageUnavailable, "recovery revocation signer cannot be read")
		}
		revoker.Status = DeviceActive
		revoker.RevokedAtMs = 0
		event.Device = devicePointer(revoker)
		event.RecoveryRevocation = recoveryRevocationPointer(revocation)
	default:
		return newError(ErrStorageUnavailable, "lifecycle event type is invalid")
	}
	return nil
}

func recoveredDeviceActivationTx(ctx context.Context, tx *sql.Tx, domainID string, recoveryRecordID string) (RecoveredDeviceActivation, error) {
	row := tx.QueryRowContext(ctx, `SELECT domain_id, recovery_record_id, device_id,
		signing_algorithm, signing_public_key_id, signing_public_key,
		key_agreement_algorithm, key_agreement_public_key_id, key_agreement_public_key,
		key_epoch, created_at_ms, signature_schema_version,
		activation_algorithm, activation_public_key_id, activation_signature
		FROM recovered_device_activations WHERE domain_id = ? AND recovery_record_id = ?`,
		domainID, recoveryRecordID)
	var activation RecoveredDeviceActivation
	var keyEpoch, signatureSchemaVersion int64
	if err := row.Scan(&activation.DomainID, &activation.RecoveryRecordID, &activation.DeviceID,
		&activation.SigningAlgorithm, &activation.SigningPublicKeyID, &activation.SigningPublicKey,
		&activation.KeyAgreementAlgorithm, &activation.KeyAgreementPublicKeyID, &activation.KeyAgreementPublicKey,
		&keyEpoch, &activation.CreatedAtMs, &signatureSchemaVersion,
		&activation.ActivationAlgorithm, &activation.ActivationPublicKeyID, &activation.ActivationSignature); err != nil {
		return RecoveredDeviceActivation{}, newError(ErrStorageUnavailable, "recovered device activation cannot be read")
	}
	if keyEpoch <= 0 || signatureSchemaVersion <= 0 {
		return RecoveredDeviceActivation{}, newError(ErrStorageUnavailable, "recovered device activation metadata is invalid")
	}
	activation.KeyEpoch = uint64(keyEpoch)
	activation.SignatureSchemaVersion = uint16(signatureSchemaVersion)
	return cloneRecoveredDeviceActivation(activation), nil
}

func wrappingForAuthorizationTx(ctx context.Context, tx *sql.Tx, authorization DeviceAuthorization) (DeviceWrappingRecord, error) {
	row := tx.QueryRowContext(ctx, `
		SELECT domain_id, recipient_device_id, recipient_key_agreement_key_id, authorizer_device_id, key_epoch,
			wrapping_key_id, algorithm, nonce, wrapped_key_len,
			ciphertext_hash, created_at_ms, signature_record_type, signature_schema_version,
			signature_algorithm, signature_key_id, signature, blob_ref
		FROM device_wrapping_records
		WHERE domain_id = ? AND recipient_device_id = ? AND authorizer_device_id = ? AND key_epoch = ?
		ORDER BY created_at_ms, wrapping_key_id LIMIT 1
	`, authorization.DomainID, authorization.RecipientDeviceID, authorization.AuthorizerDeviceID, int64(authorization.KeyEpoch))
	var record DeviceWrappingRecord
	var keyEpoch int64
	var signatureSchemaVersion int64
	if err := row.Scan(
		&record.DomainID, &record.RecipientDeviceID, &record.RecipientKeyAgreementKeyID, &record.AuthorizerDeviceID, &keyEpoch,
		&record.WrappingKeyID, &record.Algorithm, &record.Nonce, &record.WrappedKeyLen,
		&record.CiphertextHash, &record.CreatedAtMs, &record.SignatureRecordType, &signatureSchemaVersion,
		&record.SignatureAlgorithm, &record.SignatureKeyID, &record.Signature, &record.BlobRef,
	); err != nil {
		return DeviceWrappingRecord{}, newError(ErrStorageUnavailable, "authorization wrapping metadata cannot be read")
	}
	if keyEpoch <= 0 {
		return DeviceWrappingRecord{}, newError(ErrStorageUnavailable, "authorization wrapping metadata is invalid")
	}
	record.KeyEpoch = uint64(keyEpoch)
	record.SignatureSchemaVersion = uint16(signatureSchemaVersion)
	return cloneWrappingRecord(record), nil
}

func authorizationTx(ctx context.Context, tx *sql.Tx, domainID string, joinRequestID string) (DeviceAuthorization, error) {
	row := tx.QueryRowContext(ctx, `
		SELECT domain_id, join_request_id, authorizer_device_id, recipient_device_id,
			recipient_signing_public_key_id, recipient_key_agreement_key_id,
			join_short_code, key_epoch, created_at_ms,
			signature_schema_version, signature_algorithm, signature_key_id, signature
		FROM device_authorizations
		WHERE domain_id = ? AND join_request_id = ?
	`, domainID, joinRequestID)
	var authorization DeviceAuthorization
	var keyEpoch int64
	var signatureSchemaVersion int64
	if err := row.Scan(
		&authorization.DomainID, &authorization.JoinRequestID,
		&authorization.AuthorizerDeviceID, &authorization.RecipientDeviceID,
		&authorization.RecipientSigningPublicKeyID, &authorization.RecipientKeyAgreementKeyID,
		&authorization.JoinShortCode, &keyEpoch, &authorization.CreatedAtMs,
		&signatureSchemaVersion, &authorization.SignatureAlgorithm,
		&authorization.SignatureKeyID, &authorization.Signature,
	); err != nil {
		return DeviceAuthorization{}, newError(ErrStorageUnavailable, "authorization lifecycle metadata cannot be read")
	}
	if keyEpoch <= 0 || signatureSchemaVersion <= 0 {
		return DeviceAuthorization{}, newError(ErrStorageUnavailable, "authorization lifecycle metadata is invalid")
	}
	authorization.KeyEpoch = uint64(keyEpoch)
	authorization.SignatureSchemaVersion = uint16(signatureSchemaVersion)
	return cloneAuthorization(authorization), nil
}

func revocationByRecordIDTx(ctx context.Context, tx *sql.Tx, domainID string, recordID string) (DeviceRevocation, error) {
	row := tx.QueryRowContext(ctx, `
		SELECT domain_id, revoked_device_id, revoker_device_id,
			previous_key_epoch, new_key_epoch, reason, created_at_ms,
			signature_schema_version, signature_algorithm, signature_key_id, signature
		FROM device_revocations
		WHERE domain_id = ? AND revoked_device_id || ':' || new_key_epoch = ?
	`, domainID, recordID)
	var revocation DeviceRevocation
	var previousKeyEpoch int64
	var newKeyEpoch int64
	var signatureSchemaVersion int64
	if err := row.Scan(
		&revocation.DomainID, &revocation.RevokedDeviceID, &revocation.RevokerDeviceID,
		&previousKeyEpoch, &newKeyEpoch, &revocation.Reason, &revocation.CreatedAtMs,
		&signatureSchemaVersion, &revocation.SignatureAlgorithm,
		&revocation.SignatureKeyID, &revocation.Signature,
	); err != nil {
		return DeviceRevocation{}, newError(ErrStorageUnavailable, "revocation lifecycle metadata cannot be read")
	}
	if previousKeyEpoch <= 0 || newKeyEpoch <= previousKeyEpoch || signatureSchemaVersion <= 0 {
		return DeviceRevocation{}, newError(ErrStorageUnavailable, "revocation lifecycle metadata is invalid")
	}
	revocation.PreviousKeyEpoch = uint64(previousKeyEpoch)
	revocation.NewKeyEpoch = uint64(newKeyEpoch)
	revocation.SignatureSchemaVersion = uint16(signatureSchemaVersion)
	return cloneRevocation(revocation), nil
}

func insertLifecycleEventTx(ctx context.Context, tx *sql.Tx, event LifecycleEvent) error {
	if event.LifecycleSequence == 0 || event.EventType == "" || event.RecordID == "" || event.KeyEpoch == 0 || event.CreatedAtMs <= 0 {
		return newError(ErrInvalidRequest, "lifecycle event metadata is invalid")
	}
	if event.EventType != LifecycleDeviceRevoked && event.RejectFromObjectChangeSequence != 0 {
		return newError(ErrInvalidRequest, "only revocation lifecycle events can reject object sequences")
	}
	if event.EventType == LifecycleDeviceRevoked && event.RejectFromObjectChangeSequence == 0 {
		return newError(ErrInvalidRequest, "revocation lifecycle event requires object sequence cutoff")
	}
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO domain_lifecycle_events (
			domain_id, lifecycle_sequence, event_type, record_id, key_epoch,
			reject_from_object_change_sequence, created_at_ms
		) VALUES (?, ?, ?, ?, ?, ?, ?)
	`, event.DomainID, int64(event.LifecycleSequence), string(event.EventType), event.RecordID, int64(event.KeyEpoch),
		int64(event.RejectFromObjectChangeSequence), event.CreatedAtMs); err != nil {
		return newError(ErrStorageUnavailable, "lifecycle metadata cannot be stored")
	}
	return nil
}

func nextLifecycleSequenceTx(ctx context.Context, tx *sql.Tx, domainID string) (uint64, error) {
	var next int64
	if err := tx.QueryRowContext(ctx, `
		SELECT COALESCE(MAX(lifecycle_sequence), 0) + 1
		FROM domain_lifecycle_events WHERE domain_id = ?
	`, domainID).Scan(&next); err != nil || next <= 0 {
		return 0, newError(ErrStorageUnavailable, "lifecycle sequence cannot be allocated")
	}
	return uint64(next), nil
}

func nextObjectChangeSequenceTx(ctx context.Context, tx *sql.Tx, domainID string) (uint64, error) {
	var next int64
	if err := tx.QueryRowContext(ctx, `
		SELECT COALESCE(MAX(change_sequence), 0) + 1
		FROM sync_object_versions
		WHERE domain_id = ?
	`, domainID).Scan(&next); err != nil || next <= 0 {
		return 0, newError(ErrStorageUnavailable, "object change sequence cannot be allocated")
	}
	return uint64(next), nil
}

func rollbackTx(tx *sql.Tx) {
	_ = tx.Rollback()
}

func cleanupStagedBlob(ctx context.Context, staged StagedObjectBlob) {
	_ = staged.Cleanup(ctx)
}
