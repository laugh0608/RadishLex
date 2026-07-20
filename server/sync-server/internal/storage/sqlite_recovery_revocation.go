package storage

import (
	"context"
	"database/sql"
	"errors"
)

const recoveryRecordRevocationColumns = `
	recovery_record_id, domain_id, revoker_device_id, key_epoch, reason, created_at_ms,
	signature_schema_version, signature_algorithm, signature_key_id, signature`

func (s *SQLiteStore) RevokeRecoveryRecord(ctx context.Context, revocation RecoveryRecordRevocation) (RecoveryRecordRevocationResult, error) {
	if err := checkContext(ctx); err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	if err := validateRecoveryRecordRevocation(revocation); err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return RecoveryRecordRevocationResult{}, newError(ErrStorageUnavailable, "sqlite transaction cannot start")
	}
	defer rollbackTx(tx)

	existing, err := recoveryRecordRevocationQuerier(ctx, tx, revocation.DomainID, revocation.RecoveryRecordID)
	if err == nil {
		if !sameRecoveryRecordRevocation(existing, revocation) {
			return RecoveryRecordRevocationResult{}, newError(ErrConflictRecoveryRecord, "recovery record already has a different revocation")
		}
		sequence, sequenceErr := lifecycleSequenceForRecordTx(ctx, tx, revocation.DomainID, LifecycleRecoveryRecordRevoked, revocation.RecoveryRecordID)
		if sequenceErr != nil {
			return RecoveryRecordRevocationResult{}, sequenceErr
		}
		return RecoveryRecordRevocationResult{Revocation: existing, LifecycleSequence: sequence}, nil
	}
	if !errors.Is(err, sql.ErrNoRows) {
		return RecoveryRecordRevocationResult{}, newError(ErrStorageUnavailable, "recovery revocation metadata cannot be read")
	}
	domain, err := domainTx(ctx, tx, revocation.DomainID)
	if err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	chainHead, err := latestRecoveryChainRecordQuerier(ctx, tx, revocation.DomainID)
	if err != nil || chainHead.RecoveryRecordID != revocation.RecoveryRecordID || domain.CurrentKeyEpoch != revocation.KeyEpoch {
		return RecoveryRecordRevocationResult{}, newError(ErrConflictRecoveryRecord, "recovery revocation target is not the current chain head")
	}
	if chainHead.Status != RecoveryRecordActive || chainHead.KeyEpoch != revocation.KeyEpoch || revocation.CreatedAtMs <= chainHead.CreatedAtMs {
		return RecoveryRecordRevocationResult{}, newError(ErrConflictRecoveryRecord, "recovery record cannot be revoked")
	}
	revoker, err := activeDeviceTx(ctx, tx, revocation.DomainID, revocation.RevokerDeviceID)
	if err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	if err := verifyRecoveryRecordRevocationSignature(revocation, revoker); err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO recovery_record_revocations (`+recoveryRecordRevocationColumns+`)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		revocation.RecoveryRecordID, revocation.DomainID, revocation.RevokerDeviceID,
		int64(revocation.KeyEpoch), revocation.Reason, revocation.CreatedAtMs,
		int64(revocation.SignatureSchemaVersion), revocation.SignatureAlgorithm,
		revocation.SignatureKeyID, cloneBytes(revocation.Signature)); err != nil {
		return RecoveryRecordRevocationResult{}, newError(ErrStorageUnavailable, "recovery revocation metadata cannot be stored")
	}
	result, err := tx.ExecContext(ctx, `UPDATE recovery_records SET status = ?, revoked_at_ms = ?
		WHERE domain_id = ? AND recovery_record_id = ? AND status = ?`,
		string(RecoveryRecordRevoked), revocation.CreatedAtMs, revocation.DomainID,
		revocation.RecoveryRecordID, string(RecoveryRecordActive))
	if err != nil {
		return RecoveryRecordRevocationResult{}, newError(ErrStorageUnavailable, "recovery record cannot be revoked")
	}
	rows, err := result.RowsAffected()
	if err != nil || rows != 1 {
		return RecoveryRecordRevocationResult{}, newError(ErrConflictRecoveryRecord, "recovery record state changed")
	}
	lifecycleSequence, err := nextLifecycleSequenceTx(ctx, tx, revocation.DomainID)
	if err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	if err := insertLifecycleEventTx(ctx, tx, LifecycleEvent{
		DomainID: revocation.DomainID, LifecycleSequence: lifecycleSequence,
		EventType: LifecycleRecoveryRecordRevoked, RecordID: revocation.RecoveryRecordID,
		KeyEpoch: revocation.KeyEpoch, CreatedAtMs: revocation.CreatedAtMs,
	}); err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	if err := tx.Commit(); err != nil {
		return RecoveryRecordRevocationResult{}, newError(ErrStorageUnavailable, "sqlite transaction cannot commit")
	}
	return RecoveryRecordRevocationResult{
		Revocation: cloneRecoveryRecordRevocation(revocation), LifecycleSequence: lifecycleSequence,
	}, nil
}

func recoveryRecordRevocationQuerier(ctx context.Context, querier sqlQuerier, domainID string, recoveryID string) (RecoveryRecordRevocation, error) {
	var revocation RecoveryRecordRevocation
	var keyEpoch, signatureSchemaVersion int64
	err := querier.QueryRowContext(ctx, `SELECT `+recoveryRecordRevocationColumns+`
		FROM recovery_record_revocations WHERE domain_id = ? AND recovery_record_id = ?`, domainID, recoveryID).Scan(
		&revocation.RecoveryRecordID, &revocation.DomainID, &revocation.RevokerDeviceID,
		&keyEpoch, &revocation.Reason, &revocation.CreatedAtMs, &signatureSchemaVersion,
		&revocation.SignatureAlgorithm, &revocation.SignatureKeyID, &revocation.Signature,
	)
	revocation.KeyEpoch = uint64(keyEpoch)
	revocation.SignatureSchemaVersion = uint16(signatureSchemaVersion)
	return cloneRecoveryRecordRevocation(revocation), err
}

func lifecycleSequenceForRecordTx(ctx context.Context, tx *sql.Tx, domainID string, eventType LifecycleEventType, recordID string) (uint64, error) {
	var sequence int64
	if err := tx.QueryRowContext(ctx, `SELECT lifecycle_sequence FROM domain_lifecycle_events
		WHERE domain_id = ? AND event_type = ? AND record_id = ?`, domainID, string(eventType), recordID).Scan(&sequence); err != nil {
		return 0, newError(ErrStorageUnavailable, "recovery revocation lifecycle event cannot be read")
	}
	return uint64(sequence), nil
}
