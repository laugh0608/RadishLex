package storage

import (
	"bytes"
	"context"
	"database/sql"
)

func (s *SQLiteStore) RecoverDevice(ctx context.Context, upload RecoveredDeviceActivationUpload) (RecoveredDeviceActivationResult, error) {
	if err := checkContext(ctx); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	if err := validateRecoveredDeviceActivationUpload(upload); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return RecoveredDeviceActivationResult{}, newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)

	activation := upload.Activation
	domain, err := domainTx(ctx, tx, activation.DomainID)
	if err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	if domain.CurrentKeyEpoch != activation.KeyEpoch {
		return RecoveredDeviceActivationResult{}, newError(ErrConflictRecoveryRecord, "recovery activation key epoch is not current")
	}
	chainHead, err := latestRecoveryChainRecordQuerier(ctx, tx, activation.DomainID)
	if err != nil || chainHead.RecoveryRecordID != activation.RecoveryRecordID {
		return RecoveredDeviceActivationResult{}, newError(ErrConflictRecoveryRecord, "recovery record is not the chain head")
	}
	if chainHead.Status != RecoveryRecordActive || chainHead.RecordSchemaVersion != RecoveryRecordSchemaVersionV2 || chainHead.KeyEpoch != activation.KeyEpoch {
		return RecoveredDeviceActivationResult{}, newError(ErrConflictRecoveryRecord, "recovery record cannot activate current epoch")
	}
	if _, err := deviceTx(ctx, tx, activation.DomainID, activation.DeviceID); err == nil {
		return RecoveredDeviceActivationResult{}, newError(ErrForbiddenDevice, "recovered device id is already registered")
	} else if !IsCode(err, ErrNotFound) {
		return RecoveredDeviceActivationResult{}, err
	}
	if err := verifyRecoveredDeviceActivationSignature(activation, chainHead); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	device := recoveredDeviceFromActivation(activation)
	if err := validateDevice(device); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}

	active, err := activeDeviceKeyAgreementCohortTx(ctx, tx, activation.DomainID)
	if err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	active[device.DeviceID] = device.KeyAgreementPublicKeyID
	if len(active) != len(upload.Distribution.Records) {
		return RecoveredDeviceActivationResult{}, newError(ErrInvalidRequest, "recovery epoch distribution must cover every active device")
	}
	missing := make([]DeviceWrappingUpload, 0, len(upload.Distribution.Records))
	for _, item := range upload.Distribution.Records {
		record := item.Record
		keyID, ok := active[record.RecipientDeviceID]
		if !ok || keyID != record.RecipientKeyAgreementKeyID {
			return RecoveredDeviceActivationResult{}, newError(ErrForbiddenDevice, "recovery epoch recipient is not active with the signed key")
		}
		if err := verifyEpochDistributionSignature(record, device); err != nil {
			return RecoveredDeviceActivationResult{}, err
		}
		existing, err := wrappingRecordQuerier(ctx, tx, record.DomainID, record.RecipientDeviceID, record.KeyEpoch, record.WrappingKeyID)
		if err == nil {
			existingBytes, readErr := s.blobs.ReadObjectBlob(ctx, existing.BlobRef)
			if readErr != nil || !sameWrappingRecord(existing, record) || !bytes.Equal(existingBytes, item.WrappedKey) {
				return RecoveredDeviceActivationResult{}, newError(ErrConflictEpochDistribution, "recovery epoch distribution locator conflicts")
			}
			continue
		}
		if !IsCode(err, ErrNotFound) {
			return RecoveredDeviceActivationResult{}, err
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
			return RecoveredDeviceActivationResult{}, err
		}
		staged = append(staged, blob)
	}
	if err := insertDeviceTx(ctx, tx, device); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	if err := insertRecoveredDeviceActivationTx(ctx, tx, activation); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	result, err := tx.ExecContext(ctx, `UPDATE recovery_records
		SET status = ?, revoked_at_ms = ?
		WHERE domain_id = ? AND recovery_record_id = ? AND status = ?`,
		string(RecoveryRecordSuperseded), activation.CreatedAtMs,
		activation.DomainID, activation.RecoveryRecordID, string(RecoveryRecordActive))
	if err != nil {
		return RecoveredDeviceActivationResult{}, newError(ErrStorageUnavailable, "recovery record cannot be consumed")
	}
	if rows, rowsErr := result.RowsAffected(); rowsErr != nil || rows != 1 {
		return RecoveredDeviceActivationResult{}, newError(ErrConflictRecoveryRecord, "recovery record was already consumed")
	}
	for _, item := range missing {
		if err := insertWrappingRecordTx(ctx, tx, item.Record); err != nil {
			return RecoveredDeviceActivationResult{}, err
		}
	}
	lifecycleSequence, err := nextLifecycleSequenceTx(ctx, tx, activation.DomainID)
	if err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	if err := insertLifecycleEventTx(ctx, tx, LifecycleEvent{
		DomainID: activation.DomainID, LifecycleSequence: lifecycleSequence,
		EventType: LifecycleDeviceRecovered, RecordID: activation.RecoveryRecordID,
		KeyEpoch: activation.KeyEpoch, CreatedAtMs: activation.CreatedAtMs,
	}); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	for _, blob := range staged {
		if err := blob.Commit(ctx); err != nil {
			return RecoveredDeviceActivationResult{}, err
		}
	}
	if err := tx.Commit(); err != nil {
		for _, item := range missing {
			_ = s.blobs.DeleteObjectBlob(context.Background(), item.Record.BlobRef)
		}
		return RecoveredDeviceActivationResult{}, newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return RecoveredDeviceActivationResult{
		Device: device, LifecycleSequence: lifecycleSequence,
		DistributedRecords: len(upload.Distribution.Records),
	}, nil
}

func activeDeviceKeyAgreementCohortTx(ctx context.Context, tx *sql.Tx, domainID string) (map[string]string, error) {
	rows, err := tx.QueryContext(ctx, `SELECT device_id, key_agreement_public_key_id
		FROM devices WHERE domain_id = ? AND status = ?`, domainID, string(DeviceActive))
	if err != nil {
		return nil, newError(ErrStorageUnavailable, "active device cohort cannot be read")
	}
	defer rows.Close()
	active := make(map[string]string)
	for rows.Next() {
		var deviceID, keyID string
		if err := rows.Scan(&deviceID, &keyID); err != nil {
			return nil, newError(ErrStorageUnavailable, "active device cohort cannot be read")
		}
		active[deviceID] = keyID
	}
	if err := rows.Err(); err != nil {
		return nil, newError(ErrStorageUnavailable, "active device cohort cannot be read")
	}
	return active, nil
}

func insertRecoveredDeviceActivationTx(ctx context.Context, tx *sql.Tx, activation RecoveredDeviceActivation) error {
	if _, err := tx.ExecContext(ctx, `INSERT INTO recovered_device_activations (
		domain_id, recovery_record_id, device_id, signing_algorithm, signing_public_key_id,
		signing_public_key, key_agreement_algorithm, key_agreement_public_key_id,
		key_agreement_public_key, key_epoch, created_at_ms, signature_schema_version,
		activation_algorithm, activation_public_key_id, activation_signature
	) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		activation.DomainID, activation.RecoveryRecordID, activation.DeviceID,
		activation.SigningAlgorithm, activation.SigningPublicKeyID, cloneBytes(activation.SigningPublicKey),
		activation.KeyAgreementAlgorithm, activation.KeyAgreementPublicKeyID, cloneBytes(activation.KeyAgreementPublicKey),
		int64(activation.KeyEpoch), activation.CreatedAtMs, int64(activation.SignatureSchemaVersion),
		activation.ActivationAlgorithm, activation.ActivationPublicKeyID, cloneBytes(activation.ActivationSignature)); err != nil {
		return newError(ErrStorageUnavailable, "recovered device activation cannot be stored")
	}
	return nil
}

func insertWrappingRecordTx(ctx context.Context, tx *sql.Tx, record DeviceWrappingRecord) error {
	if _, err := tx.ExecContext(ctx, `INSERT INTO device_wrapping_records (
		domain_id, recipient_device_id, recipient_key_agreement_key_id, authorizer_device_id, key_epoch,
		wrapping_key_id, algorithm, nonce, wrapped_key_len, ciphertext_hash, created_at_ms,
		signature_record_type, signature_schema_version, signature_algorithm, signature_key_id,
		signature, blob_ref
	) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		record.DomainID, record.RecipientDeviceID, record.RecipientKeyAgreementKeyID, record.AuthorizerDeviceID, int64(record.KeyEpoch),
		record.WrappingKeyID, record.Algorithm, cloneBytes(record.Nonce), record.WrappedKeyLen,
		record.CiphertextHash, record.CreatedAtMs, record.SignatureRecordType, int64(record.SignatureSchemaVersion),
		record.SignatureAlgorithm, record.SignatureKeyID, cloneBytes(record.Signature), record.BlobRef); err != nil {
		return newError(ErrStorageUnavailable, "recovery epoch distribution metadata cannot be stored")
	}
	return nil
}
