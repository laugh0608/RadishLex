package api

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

func TestLatestRecoveryHandlerReturnsMetadataAndWrappedMaterial(t *testing.T) {
	wrappedMaterial := []byte("encrypted wrapped material")
	store := &recoveryStoreStub{
		record: storage.RecoveryRecord{
			DomainID:               "domain-a",
			RecoveryRecordID:       "recovery-a",
			KeyEpoch:               2,
			KDFProfile:             "argon2id-recovery-v1",
			KDFVersion:             1,
			MemoryKiB:              65536,
			Iterations:             3,
			Parallelism:            1,
			OutputLen:              32,
			Salt:                   []byte{1, 2, 3},
			Algorithm:              storage.AlgorithmXChaCha20Poly1305HKDFSHA256,
			Nonce:                  []byte{4, 5, 6},
			WrappedMaterialLen:     int64(len(wrappedMaterial)),
			CiphertextHash:         "sha256:wrapped",
			Status:                 storage.RecoveryRecordActive,
			CreatedAtMs:            100,
			SignerDeviceID:         "device-a",
			SignatureSchemaVersion: 1,
			SignatureAlgorithm:     "ed25519-v1",
			SignatureKeyID:         "signing-key-a",
			Signature:              []byte{7, 8, 9},
			BlobRef:                "internal/blob/ref",
		},
		material: wrappedMaterial,
	}
	handler := NewHandler(store, HandlerConfig{Now: fixedNow})

	request := httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/recovery-records/latest", nil)
	request.RemoteAddr = "203.0.113.10:45100"
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)

	if response.Code != http.StatusOK {
		t.Fatalf("unexpected status: %d body=%s", response.Code, response.Body.String())
	}
	var body RecoveryRecordResponse
	if err := json.Unmarshal(response.Body.Bytes(), &body); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	if body.RecoveryRecordID != "recovery-a" || body.CiphertextHash != "sha256:wrapped" {
		t.Fatalf("metadata missing from response: %#v", body)
	}
	if string(body.WrappedMaterial) != string(wrappedMaterial) {
		t.Fatalf("wrapped material mismatch: %q", string(body.WrappedMaterial))
	}
	if strings.Contains(response.Body.String(), "internal/blob/ref") {
		t.Fatalf("response leaked internal blob ref: %s", response.Body.String())
	}
}

func TestLatestRecoveryHandlerMapsStorageError(t *testing.T) {
	store := &recoveryStoreStub{
		err: &storage.Error{Code: storage.ErrNotFound, Message: "active recovery record not found"},
	}
	handler := NewHandler(store, HandlerConfig{Now: fixedNow})

	request := httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/recovery-records/latest", nil)
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)

	if response.Code != http.StatusNotFound {
		t.Fatalf("unexpected status: %d body=%s", response.Code, response.Body.String())
	}
	var body ErrorResponse
	if err := json.Unmarshal(response.Body.Bytes(), &body); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	if body.ErrorCode != string(storage.ErrNotFound) || body.ServerTimeMs != 1234 {
		t.Fatalf("unexpected error response: %#v", body)
	}
}

func TestLatestRecoveryHandlerRateLimitsBeforeStorageRead(t *testing.T) {
	store := &recoveryStoreStub{record: storage.RecoveryRecord{DomainID: "domain-a"}, material: []byte("wrapped")}
	handler := NewHandler(store, HandlerConfig{
		RecoveryReadLimit:  1,
		RecoveryReadWindow: time.Hour,
		Now:                fixedNow,
	})

	first := httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/recovery-records/latest", nil)
	first.RemoteAddr = "203.0.113.10:45100"
	first.Header.Set(deviceIDHeader, "device-a")
	firstResponse := httptest.NewRecorder()
	handler.ServeHTTP(firstResponse, first)
	if firstResponse.Code != http.StatusOK {
		t.Fatalf("unexpected first status: %d body=%s", firstResponse.Code, firstResponse.Body.String())
	}

	second := httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/recovery-records/latest", nil)
	second.RemoteAddr = "203.0.113.10:45199"
	second.Header.Set(deviceIDHeader, "device-a")
	secondResponse := httptest.NewRecorder()
	handler.ServeHTTP(secondResponse, second)
	if secondResponse.Code != http.StatusTooManyRequests {
		t.Fatalf("unexpected second status: %d body=%s", secondResponse.Code, secondResponse.Body.String())
	}
	if store.calls != 1 {
		t.Fatalf("rate limited request should not read storage, calls=%d", store.calls)
	}

	var body ErrorResponse
	if err := json.Unmarshal(secondResponse.Body.Bytes(), &body); err != nil {
		t.Fatalf("decode rate limit response: %v", err)
	}
	if body.ErrorCode != string(storage.ErrRecoveryRateLimited) || !body.Retryable {
		t.Fatalf("unexpected rate limit response: %#v", body)
	}
}

func fixedNow() time.Time {
	return time.UnixMilli(1234)
}

type recoveryStoreStub struct {
	storage.Store
	record   storage.RecoveryRecord
	material []byte
	err      error
	calls    int
}

func (s *recoveryStoreStub) LatestRecoveryWrappedMaterial(ctx context.Context, domainID string) (storage.RecoveryRecord, []byte, error) {
	s.calls++
	if s.err != nil {
		return storage.RecoveryRecord{}, nil, s.err
	}
	record := s.record
	record.DomainID = domainID
	return record, cloneBytes(s.material), nil
}
