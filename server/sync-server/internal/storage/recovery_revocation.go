package storage

import (
	"bytes"
	"context"
	"unicode"
)

func (s *MemoryStore) RevokeRecoveryRecord(ctx context.Context, revocation RecoveryRecordRevocation) (RecoveryRecordRevocationResult, error) {
	if err := checkContext(ctx); err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	if err := validateRecoveryRecordRevocation(revocation); err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	key := recoveryKey{domainID: revocation.DomainID, recoveryRecordID: revocation.RecoveryRecordID}
	if existing, ok := s.recoveryRevocations[key]; ok {
		if sameRecoveryRecordRevocation(existing.Revocation, revocation) {
			return cloneRecoveryRecordRevocationResult(existing), nil
		}
		return RecoveryRecordRevocationResult{}, newError(ErrConflictRecoveryRecord, "recovery record already has a different revocation")
	}
	domain, ok := s.domains[revocation.DomainID]
	if !ok {
		return RecoveryRecordRevocationResult{}, newError(ErrNotFound, "domain not found")
	}
	if domain.CurrentKeyEpoch != revocation.KeyEpoch || s.latestRecovery[revocation.DomainID] != revocation.RecoveryRecordID {
		return RecoveryRecordRevocationResult{}, newError(ErrConflictRecoveryRecord, "recovery revocation target is not the current chain head")
	}
	record, ok := s.recoveries[key]
	if !ok || record.Status != RecoveryRecordActive || record.KeyEpoch != revocation.KeyEpoch || revocation.CreatedAtMs <= record.CreatedAtMs {
		return RecoveryRecordRevocationResult{}, newError(ErrConflictRecoveryRecord, "recovery record cannot be revoked")
	}
	revoker, err := s.activeDeviceLocked(revocation.DomainID, revocation.RevokerDeviceID)
	if err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	if err := verifyRecoveryRecordRevocationSignature(revocation, revoker); err != nil {
		return RecoveryRecordRevocationResult{}, err
	}
	record.Status = RecoveryRecordRevoked
	record.RevokedAtMs = revocation.CreatedAtMs
	s.recoveries[key] = record
	s.appendLifecycleLocked(LifecycleEvent{
		DomainID: revocation.DomainID, EventType: LifecycleRecoveryRecordRevoked,
		RecordID: revocation.RecoveryRecordID, KeyEpoch: revocation.KeyEpoch,
		CreatedAtMs: revocation.CreatedAtMs, Device: devicePointer(revoker),
		RecoveryRevocation: recoveryRevocationPointer(revocation),
	})
	result := RecoveryRecordRevocationResult{
		Revocation:        cloneRecoveryRecordRevocation(revocation),
		LifecycleSequence: s.nextLifecycle[revocation.DomainID],
	}
	s.recoveryRevocations[key] = cloneRecoveryRecordRevocationResult(result)
	return result, nil
}

func validateRecoveryRecordRevocation(revocation RecoveryRecordRevocation) error {
	if !validOpaqueID(revocation.DomainID) || !validOpaqueID(revocation.RecoveryRecordID) || !validOpaqueID(revocation.RevokerDeviceID) {
		return newError(ErrInvalidRequest, "recovery revocation ids must be opaque ids")
	}
	if revocation.KeyEpoch == 0 || revocation.CreatedAtMs <= 0 {
		return newError(ErrInvalidRequest, "recovery revocation counters are invalid")
	}
	if revocation.Reason == "" || len(revocation.Reason) > 64 {
		return newError(ErrInvalidRequest, "recovery revocation reason is invalid")
	}
	for _, value := range revocation.Reason {
		if unicode.IsControl(value) {
			return newError(ErrInvalidRequest, "recovery revocation reason is invalid")
		}
	}
	return validateSignatureFields(
		revocation.SignatureSchemaVersion,
		revocation.SignatureAlgorithm,
		revocation.SignatureKeyID,
		revocation.Signature,
	)
}

func cloneRecoveryRecordRevocation(value RecoveryRecordRevocation) RecoveryRecordRevocation {
	value.Signature = cloneBytes(value.Signature)
	return value
}

func cloneRecoveryRecordRevocationResult(value RecoveryRecordRevocationResult) RecoveryRecordRevocationResult {
	value.Revocation = cloneRecoveryRecordRevocation(value.Revocation)
	return value
}

func sameRecoveryRecordRevocation(left RecoveryRecordRevocation, right RecoveryRecordRevocation) bool {
	return left.RecoveryRecordID == right.RecoveryRecordID &&
		left.DomainID == right.DomainID &&
		left.RevokerDeviceID == right.RevokerDeviceID &&
		left.KeyEpoch == right.KeyEpoch &&
		left.Reason == right.Reason &&
		left.CreatedAtMs == right.CreatedAtMs &&
		left.SignatureSchemaVersion == right.SignatureSchemaVersion &&
		left.SignatureAlgorithm == right.SignatureAlgorithm &&
		left.SignatureKeyID == right.SignatureKeyID &&
		bytes.Equal(left.Signature, right.Signature)
}

func recoveryRecordRotatesPublicMaterial(previous RecoveryRecord, next RecoveryRecord) bool {
	return !bytes.Equal(previous.Salt, next.Salt) &&
		!bytes.Equal(previous.Nonce, next.Nonce) &&
		previous.CiphertextHash != next.CiphertextHash &&
		previous.ActivationPublicKeyID != next.ActivationPublicKeyID &&
		!bytes.Equal(previous.ActivationPublicKey, next.ActivationPublicKey)
}

func recoveryRevocationPointer(value RecoveryRecordRevocation) *RecoveryRecordRevocation {
	cloned := cloneRecoveryRecordRevocation(value)
	return &cloned
}
