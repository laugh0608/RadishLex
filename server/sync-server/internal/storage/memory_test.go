package storage

import (
	"bytes"
	"context"
	"testing"
)

func TestMemoryStoreConformance(t *testing.T) {
	runStoreConformanceTests(t, func(t *testing.T) Store {
		t.Helper()
		return NewMemoryStore()
	})
}

func TestMemoryRecoveryRotationPreservesRevokedPredecessorState(t *testing.T) {
	store := NewMemoryStore()
	_ = newReadyStore(t, func(t *testing.T) Store {
		t.Helper()
		return store
	})
	ctx := context.Background()
	firstWrapped := bytes.Repeat([]byte{0xe1}, RecoveryWrappedMaterialBytes)
	first := recoveryRecordForTest("recovery-a", "", 40, firstWrapped)
	if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: first, WrappedMaterial: firstWrapped}); err != nil {
		t.Fatalf("put recovery: %v", err)
	}
	revocation := recoveryRecordRevocationForTest("recovery-a", 1, "compromised", 50)
	if _, err := store.RevokeRecoveryRecord(ctx, revocation); err != nil {
		t.Fatalf("revoke recovery: %v", err)
	}
	secondWrapped := bytes.Repeat([]byte{0xe2}, RecoveryWrappedMaterialBytes)
	second := recoveryRecordForTest("recovery-b", "recovery-a", 60, secondWrapped)
	if _, err := store.PutRecoveryRecord(ctx, RecoveryRecordUpload{Record: second, WrappedMaterial: secondWrapped}); err != nil {
		t.Fatalf("rotate from revoked predecessor: %v", err)
	}

	predecessor := store.recoveries[recoveryKey{domainID: "domain-a", recoveryRecordID: "recovery-a"}]
	if predecessor.Status != RecoveryRecordRevoked || predecessor.RevokedAtMs != revocation.CreatedAtMs {
		t.Fatalf("rotation changed revoked predecessor state: %#v", predecessor)
	}
}
