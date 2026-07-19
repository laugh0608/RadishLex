package storage

import (
	"bytes"
	"context"
	"database/sql"
	"errors"
)

const recoveryRecordColumns = `
	record_schema_version, domain_id, recovery_record_id, previous_recovery_record_id,
	key_epoch, kdf_profile, kdf_version, memory_kib, iterations, parallelism, output_len,
	salt, algorithm, nonce, wrapped_material_len, ciphertext_hash,
	activation_algorithm, activation_public_key_id, activation_public_key,
	status, created_at_ms, updated_at_ms, revoked_at_ms, signer_device_id,
	signature_schema_version, signature_algorithm, signature_key_id, signature, blob_ref`

func (s *SQLiteStore) PutRecoveryRecord(ctx context.Context, upload RecoveryRecordUpload) (RecoveryRecord, error) {
	if err := checkContext(ctx); err != nil {
		return RecoveryRecord{}, err
	}
	if err := validateRecoveryRecordUpload(upload); err != nil {
		return RecoveryRecord{}, err
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return RecoveryRecord{}, newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)

	existing, err := recoveryRecordQuerier(ctx, tx, upload.Record.DomainID, upload.Record.RecoveryRecordID)
	if err == nil {
		wrapped, readErr := s.blobs.ReadObjectBlob(ctx, existing.BlobRef)
		if readErr != nil {
			return RecoveryRecord{}, newError(ErrStorageUnavailable, "recovery wrapped material is missing")
		}
		if sameRecoveryRecord(existing, upload.Record) && bytes.Equal(wrapped, upload.WrappedMaterial) {
			return existing, nil
		}
		return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery record id already exists with different content")
	}
	if !errors.Is(err, sql.ErrNoRows) {
		return RecoveryRecord{}, newError(ErrStorageUnavailable, "recovery metadata cannot be read")
	}
	domain, err := domainTx(ctx, tx, upload.Record.DomainID)
	if err != nil {
		return RecoveryRecord{}, err
	}
	if upload.Record.KeyEpoch != domain.CurrentKeyEpoch {
		return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery record key epoch is not current")
	}
	current, err := latestRecoveryChainRecordQuerier(ctx, tx, upload.Record.DomainID)
	if errors.Is(err, sql.ErrNoRows) {
		if upload.Record.PreviousRecoveryID != "" {
			return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "first recovery record cannot name a predecessor")
		}
	} else if err != nil {
		return RecoveryRecord{}, newError(ErrStorageUnavailable, "recovery metadata cannot be read")
	} else if upload.Record.PreviousRecoveryID != current.RecoveryRecordID {
		return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery record predecessor is stale")
	} else if upload.Record.CreatedAtMs <= current.CreatedAtMs {
		return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery record timestamp does not advance predecessor")
	}
	signer, err := activeDeviceTx(ctx, tx, upload.Record.DomainID, upload.Record.SignerDeviceID)
	if err != nil {
		return RecoveryRecord{}, err
	}
	if err := verifyRecoverySignature(upload.Record, signer); err != nil {
		return RecoveryRecord{}, err
	}
	record := cloneRecoveryRecord(upload.Record)
	record.BlobRef = recoveryBlobRef(record)
	staged, err := s.blobs.StageObjectBlob(ctx, record.BlobRef, upload.WrappedMaterial)
	if err != nil {
		return RecoveryRecord{}, err
	}
	defer cleanupStagedBlob(ctx, staged)

	if current.RecoveryRecordID != "" && current.Status == RecoveryRecordActive {
		result, updateErr := tx.ExecContext(ctx, `
			UPDATE recovery_records
			SET status = ?, revoked_at_ms = ?
			WHERE domain_id = ? AND recovery_record_id = ? AND status = ?
		`, string(RecoveryRecordSuperseded), record.CreatedAtMs,
			record.DomainID, current.RecoveryRecordID, string(RecoveryRecordActive))
		if updateErr != nil {
			return RecoveryRecord{}, newError(ErrStorageUnavailable, "previous recovery record cannot be superseded")
		}
		rows, rowsErr := result.RowsAffected()
		if rowsErr != nil || rows != 1 {
			return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery record predecessor changed")
		}
	}
	if _, err := tx.ExecContext(ctx, `
		INSERT INTO recovery_records (`+recoveryRecordColumns+`)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
	`, int64(record.RecordSchemaVersion), record.DomainID, record.RecoveryRecordID, record.PreviousRecoveryID,
		int64(record.KeyEpoch), record.KDFProfile, int64(record.KDFVersion), int64(record.MemoryKiB), int64(record.Iterations), int64(record.Parallelism), record.OutputLen,
		cloneBytes(record.Salt), record.Algorithm, cloneBytes(record.Nonce), record.WrappedMaterialLen, record.CiphertextHash,
		record.ActivationAlgorithm, record.ActivationPublicKeyID, cloneBytes(record.ActivationPublicKey),
		string(record.Status), record.CreatedAtMs, record.UpdatedAtMs, record.RevokedAtMs, record.SignerDeviceID,
		int64(record.SignatureSchemaVersion), record.SignatureAlgorithm, record.SignatureKeyID, cloneBytes(record.Signature), record.BlobRef,
	); err != nil {
		return RecoveryRecord{}, newError(ErrStorageUnavailable, "recovery metadata cannot be stored")
	}
	lifecycleSequence, err := nextLifecycleSequenceTx(ctx, tx, record.DomainID)
	if err != nil {
		return RecoveryRecord{}, err
	}
	if err := insertLifecycleEventTx(ctx, tx, LifecycleEvent{
		DomainID: record.DomainID, LifecycleSequence: lifecycleSequence,
		EventType: LifecycleRecoveryRecordRotated, RecordID: record.RecoveryRecordID,
		KeyEpoch: record.KeyEpoch, CreatedAtMs: record.CreatedAtMs,
	}); err != nil {
		return RecoveryRecord{}, err
	}
	if err := staged.Commit(ctx); err != nil {
		return RecoveryRecord{}, err
	}
	if err := tx.Commit(); err != nil {
		_ = s.blobs.DeleteObjectBlob(context.Background(), record.BlobRef)
		return RecoveryRecord{}, newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return record, nil
}

func (s *SQLiteStore) LatestRecoveryRecord(ctx context.Context, domainID string) (RecoveryRecord, error) {
	if err := checkContext(ctx); err != nil {
		return RecoveryRecord{}, err
	}
	record, err := latestRecoveryRecordQuerier(ctx, s.db, domainID)
	if errors.Is(err, sql.ErrNoRows) {
		return RecoveryRecord{}, newError(ErrNotFound, "active recovery record not found")
	}
	if err != nil {
		return RecoveryRecord{}, newError(ErrStorageUnavailable, "recovery metadata cannot be read")
	}
	return record, nil
}

func (s *SQLiteStore) LatestRecoveryWrappedMaterial(ctx context.Context, domainID string) (RecoveryRecord, []byte, error) {
	record, err := s.LatestRecoveryRecord(ctx, domainID)
	if err != nil {
		return RecoveryRecord{}, nil, err
	}
	wrapped, err := s.blobs.ReadObjectBlob(ctx, record.BlobRef)
	if err != nil {
		return RecoveryRecord{}, nil, newError(ErrStorageUnavailable, "recovery wrapped material is missing")
	}
	if int64(len(wrapped)) != record.WrappedMaterialLen || CiphertextHash(wrapped) != record.CiphertextHash {
		return RecoveryRecord{}, nil, newError(ErrStorageUnavailable, "recovery wrapped material metadata mismatch")
	}
	return record, cloneBytes(wrapped), nil
}

func latestRecoveryRecordQuerier(ctx context.Context, querier sqlQuerier, domainID string) (RecoveryRecord, error) {
	return scanRecoveryRecord(querier.QueryRowContext(ctx, `SELECT `+recoveryRecordColumns+`
		FROM recovery_records WHERE domain_id = ? AND status = ?
		ORDER BY created_at_ms DESC, recovery_record_id DESC LIMIT 1`, domainID, string(RecoveryRecordActive)))
}

func latestRecoveryChainRecordQuerier(ctx context.Context, querier sqlQuerier, domainID string) (RecoveryRecord, error) {
	return scanRecoveryRecord(querier.QueryRowContext(ctx, `SELECT `+recoveryRecordColumns+`
		FROM recovery_records WHERE domain_id = ?
		ORDER BY created_at_ms DESC, recovery_record_id DESC LIMIT 1`, domainID))
}

func recoveryRecordQuerier(ctx context.Context, querier sqlQuerier, domainID string, recoveryID string) (RecoveryRecord, error) {
	return scanRecoveryRecord(querier.QueryRowContext(ctx, `SELECT `+recoveryRecordColumns+`
		FROM recovery_records WHERE domain_id = ? AND recovery_record_id = ?`, domainID, recoveryID))
}

func scanRecoveryRecord(row sqlRow) (RecoveryRecord, error) {
	var record RecoveryRecord
	var schemaVersion, keyEpoch, kdfVersion, memoryKiB, iterations, parallelism, signatureSchemaVersion int64
	var status string
	if err := row.Scan(
		&schemaVersion, &record.DomainID, &record.RecoveryRecordID, &record.PreviousRecoveryID,
		&keyEpoch, &record.KDFProfile, &kdfVersion, &memoryKiB, &iterations, &parallelism, &record.OutputLen,
		&record.Salt, &record.Algorithm, &record.Nonce, &record.WrappedMaterialLen, &record.CiphertextHash,
		&record.ActivationAlgorithm, &record.ActivationPublicKeyID, &record.ActivationPublicKey,
		&status, &record.CreatedAtMs, &record.UpdatedAtMs, &record.RevokedAtMs, &record.SignerDeviceID,
		&signatureSchemaVersion, &record.SignatureAlgorithm, &record.SignatureKeyID, &record.Signature, &record.BlobRef,
	); err != nil {
		return RecoveryRecord{}, err
	}
	record.RecordSchemaVersion = uint16(schemaVersion)
	record.KeyEpoch = uint64(keyEpoch)
	record.KDFVersion = uint16(kdfVersion)
	record.MemoryKiB = uint32(memoryKiB)
	record.Iterations = uint32(iterations)
	record.Parallelism = uint32(parallelism)
	record.SignatureSchemaVersion = uint16(signatureSchemaVersion)
	record.Status = RecoveryRecordStatus(status)
	return cloneRecoveryRecord(record), nil
}
