package api

import (
	"bytes"
	"context"
	"crypto/ed25519"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

func TestMetadataHandlersCreateDomainReadDeviceAndSaveJoinRequest(t *testing.T) {
	store := storage.NewMemoryStore()
	handler := NewHandler(store, HandlerConfig{Now: fixedNow})

	createDomain := CreateDomainRequest{
		DomainID:        "domain-a",
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "epoch-key-a",
		FirstDevice: DeviceMetadata{
			DeviceID:                "device-a",
			SigningPublicKeyID:      "signing-key-a",
			SigningPublicKey:        []byte("signing-public-key-a"),
			KeyAgreementPublicKeyID: "agreement-key-a",
			KeyAgreementPublicKey:   []byte("agreement-public-key-a"),
			Status:                  string(storage.DeviceActive),
		},
		CreatedAtMs: 100,
		UpdatedAtMs: 100,
	}
	createResponse := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains", createDomain)
	if createResponse.Code != http.StatusCreated {
		t.Fatalf("unexpected create domain status: %d body=%s", createResponse.Code, createResponse.Body.String())
	}
	var domainBody DomainStateResponse
	decodeResponse(t, createResponse, &domainBody)
	if domainBody.Domain.DomainID != "domain-a" || domainBody.Domain.CurrentKeyEpoch != 1 {
		t.Fatalf("unexpected domain response: %#v", domainBody)
	}

	stateResponse := httptest.NewRecorder()
	handler.ServeHTTP(stateResponse, httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/state", nil))
	if stateResponse.Code != http.StatusOK {
		t.Fatalf("unexpected domain state status: %d body=%s", stateResponse.Code, stateResponse.Body.String())
	}
	decodeResponse(t, stateResponse, &domainBody)
	if domainBody.Domain.ActiveKeyID != "epoch-key-a" {
		t.Fatalf("unexpected domain state response: %#v", domainBody)
	}

	deviceResponse := httptest.NewRecorder()
	handler.ServeHTTP(deviceResponse, httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/devices/device-a", nil))
	if deviceResponse.Code != http.StatusOK {
		t.Fatalf("unexpected device status: %d body=%s", deviceResponse.Code, deviceResponse.Body.String())
	}
	var deviceBody DeviceResponse
	decodeResponse(t, deviceResponse, &deviceBody)
	if deviceBody.DeviceID != "device-a" || deviceBody.Status != storage.DeviceActive {
		t.Fatalf("unexpected device response: %#v", deviceBody)
	}

	joinRequest := CreateJoinRequestRequest{
		JoinRequestID:           "join-a",
		DeviceID:                "device-b",
		SigningPublicKeyID:      "signing-key-b",
		SigningPublicKey:        []byte("signing-public-key-b"),
		KeyAgreementPublicKeyID: "agreement-key-b",
		KeyAgreementPublicKey:   []byte("agreement-public-key-b"),
		Challenge:               []byte("join-challenge"),
		CreatedAtMs:             110,
		ExpiresAtMs:             210,
	}
	joinResponse := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains/domain-a/join-requests", joinRequest)
	if joinResponse.Code != http.StatusCreated {
		t.Fatalf("unexpected join request status: %d body=%s", joinResponse.Code, joinResponse.Body.String())
	}
	var joinBody JoinRequestResponse
	decodeResponse(t, joinResponse, &joinBody)
	if joinBody.JoinRequestID != "join-a" || joinBody.Status != storage.DevicePending {
		t.Fatalf("unexpected join response: %#v", joinBody)
	}
	listResponse := httptest.NewRecorder()
	handler.ServeHTTP(listResponse, httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/join-requests", nil))
	if listResponse.Code != http.StatusOK {
		t.Fatalf("unexpected join request list status: %d body=%s", listResponse.Code, listResponse.Body.String())
	}
	var listBody JoinRequestsResponse
	decodeResponse(t, listResponse, &listBody)
	if len(listBody.JoinRequests) != 1 || listBody.JoinRequests[0].JoinRequestID != "join-a" {
		t.Fatalf("unexpected join request list: %#v", listBody)
	}

	pendingDeviceResponse := httptest.NewRecorder()
	handler.ServeHTTP(pendingDeviceResponse, httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/devices/device-b", nil))
	if pendingDeviceResponse.Code != http.StatusOK {
		t.Fatalf("unexpected pending device status: %d body=%s", pendingDeviceResponse.Code, pendingDeviceResponse.Body.String())
	}
	decodeResponse(t, pendingDeviceResponse, &deviceBody)
	if deviceBody.DeviceID != "device-b" || deviceBody.Status != storage.DevicePending {
		t.Fatalf("join request should create a pending device, got %#v", deviceBody)
	}
}

func TestJoinAuthorizationHandlerPassesSignedMetadataToStorage(t *testing.T) {
	store := &authorizationStoreStub{}
	handler := NewHandler(store, HandlerConfig{Now: fixedNow})
	request := AuthorizeJoinRequestRequest{
		Authorization: DeviceAuthorizationRequest{
			AuthorizerDeviceID:          "device-a",
			RecipientDeviceID:           "device-b",
			RecipientSigningPublicKeyID: "signing-key-b",
			RecipientKeyAgreementKeyID:  "agreement-key-b",
			JoinShortCode:               "123456",
			KeyEpoch:                    1,
			CreatedAtMs:                 200,
			SignatureSchemaVersion:      1,
			SignatureAlgorithm:          "ed25519-v1",
			SignatureKeyID:              "signing-key-a",
			Signature:                   []byte("signature"),
		},
		Wrapping: DeviceWrappingRequest{
			AuthorizerDeviceID: "device-a",
			RecipientDeviceID:  "device-b",
			KeyEpoch:           1,
			WrappingKeyID:      "wrapping-key-b",
			Algorithm:          storage.AlgorithmXChaCha20Poly1305HKDFSHA256,
			Nonce:              []byte("nonce"),
			WrappedKeyLen:      int64(len("wrapped-key")),
			CiphertextHash:     "sha256:wrapped",
			CreatedAtMs:        200,
			Signature:          []byte("wrapping-signature"),
		},
		WrappedKey: []byte("wrapped-key"),
	}
	response := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains/domain-a/join-requests/join-a/authorization", request)
	if response.Code != http.StatusNoContent {
		t.Fatalf("unexpected authorization status: %d body=%s", response.Code, response.Body.String())
	}
	if store.calls != 1 {
		t.Fatalf("expected one authorization call, got %d", store.calls)
	}
	upload := store.upload
	if upload.Authorization.DomainID != "domain-a" ||
		upload.Authorization.JoinRequestID != "join-a" ||
		upload.Authorization.AuthorizerDeviceID != "device-a" ||
		upload.Authorization.RecipientDeviceID != "device-b" ||
		upload.Wrapping.DomainID != "domain-a" ||
		upload.Wrapping.WrappingKeyID != "wrapping-key-b" ||
		string(upload.WrappedKey) != "wrapped-key" {
		t.Fatalf("unexpected authorization upload: %#v", upload)
	}
}

func TestHandlerAddsRequestIDAndRecordsAuditEvent(t *testing.T) {
	audit := &auditSinkStub{}
	handler := NewHandler(storage.NewMemoryStore(), HandlerConfig{
		Now:       fixedNow,
		RequestID: fixedRequestID,
		AuditSink: audit,
	})

	createDomain := CreateDomainRequest{
		DomainID:        "domain-a",
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "epoch-key-a",
		FirstDevice: DeviceMetadata{
			DeviceID:                "device-a",
			SigningPublicKeyID:      "signing-key-a",
			SigningPublicKey:        []byte("signing-public-key-a"),
			KeyAgreementPublicKeyID: "agreement-key-a",
			KeyAgreementPublicKey:   []byte("agreement-public-key-a"),
			Status:                  string(storage.DeviceActive),
		},
		CreatedAtMs: 100,
		UpdatedAtMs: 100,
	}
	requestBody, err := json.Marshal(createDomain)
	if err != nil {
		t.Fatalf("marshal request: %v", err)
	}
	request := httptest.NewRequest(http.MethodPost, PrefixV1+"/domains", bytes.NewReader(requestBody))
	request.Header.Set(requestIDHeader, "client-request-a")
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)

	if response.Code != http.StatusCreated {
		t.Fatalf("unexpected status: %d body=%s", response.Code, response.Body.String())
	}
	if response.Header().Get(requestIDHeader) != "client-request-a" {
		t.Fatalf("request id header missing: %q", response.Header().Get(requestIDHeader))
	}
	if len(audit.events) != 1 {
		t.Fatalf("expected one audit event, got %d", len(audit.events))
	}
	event := audit.events[0]
	if event.RequestID != "client-request-a" ||
		event.RouteName != "domains.create" ||
		event.DomainID != "domain-a" ||
		event.DeviceID != "device-a" ||
		event.ResultCode != "ok" ||
		event.StatusCode != http.StatusCreated {
		t.Fatalf("unexpected audit event: %#v", event)
	}
}

func TestAuditEventDoesNotIncludeRequestBody(t *testing.T) {
	audit := &auditSinkStub{}
	handler := NewHandler(storage.NewMemoryStore(), HandlerConfig{
		Now:       fixedNow,
		RequestID: fixedRequestID,
		AuditSink: audit,
	})

	createDomain := CreateDomainRequest{
		DomainID:        "domain-a",
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "epoch-key-a",
		FirstDevice: DeviceMetadata{
			DeviceID:                "device-a",
			SigningPublicKeyID:      "signing-key-a",
			SigningPublicKey:        []byte("sensitive-signing-public-key-fixture"),
			KeyAgreementPublicKeyID: "agreement-key-a",
			KeyAgreementPublicKey:   []byte("sensitive-agreement-public-key-fixture"),
			Status:                  string(storage.DeviceActive),
		},
		CreatedAtMs: 100,
		UpdatedAtMs: 100,
	}
	response := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains", createDomain)
	if response.Code != http.StatusCreated {
		t.Fatalf("unexpected status: %d body=%s", response.Code, response.Body.String())
	}
	if len(audit.events) != 1 {
		t.Fatalf("expected one audit event, got %d", len(audit.events))
	}
	eventText := fmt.Sprintf("%#v", audit.events[0])
	if strings.Contains(eventText, "sensitive-signing-public-key-fixture") ||
		strings.Contains(eventText, "sensitive-agreement-public-key-fixture") {
		t.Fatalf("audit event leaked request body fields: %s", eventText)
	}
}

func TestHandlerRecordsPersistentAuditEventWhenStoreSupportsIt(t *testing.T) {
	store := &persistentAuditStoreStub{}
	handler := NewHandler(store, HandlerConfig{
		Now:       fixedNow,
		RequestID: fixedRequestID,
	})

	request := httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/state", nil)
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)

	if response.Code != http.StatusOK {
		t.Fatalf("unexpected status: %d body=%s", response.Code, response.Body.String())
	}
	if len(store.auditEvents) != 1 {
		t.Fatalf("expected one persistent audit event, got %d", len(store.auditEvents))
	}
	event := store.auditEvents[0]
	if event.DomainID != "domain-a" ||
		event.EventType != "domains.state" ||
		event.ResultCode != "ok" ||
		event.ServerTimeMs != 1234 {
		t.Fatalf("unexpected persistent audit event: %#v", event)
	}
}

func TestHandlerRecoversPanicWithStructuredErrorAndAudit(t *testing.T) {
	audit := &auditSinkStub{}
	handler := NewHandler(&panicDomainStore{}, HandlerConfig{
		Now:       fixedNow,
		RequestID: fixedRequestID,
		AuditSink: audit,
	})

	request := httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/state", nil)
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)

	if response.Code != http.StatusServiceUnavailable {
		t.Fatalf("unexpected status: %d body=%s", response.Code, response.Body.String())
	}
	if response.Header().Get(requestIDHeader) != "req-fixed" {
		t.Fatalf("generated request id missing: %q", response.Header().Get(requestIDHeader))
	}
	var body ErrorResponse
	decodeResponse(t, response, &body)
	if body.ErrorCode != string(storage.ErrStorageUnavailable) {
		t.Fatalf("unexpected error response: %#v", body)
	}
	if len(audit.events) != 1 {
		t.Fatalf("expected one audit event, got %d", len(audit.events))
	}
	event := audit.events[0]
	if event.RequestID != "req-fixed" ||
		event.RouteName != "domains.state" ||
		event.DomainID != "domain-a" ||
		event.ResultCode != string(storage.ErrStorageUnavailable) ||
		event.StatusCode != http.StatusServiceUnavailable {
		t.Fatalf("unexpected audit event: %#v", event)
	}
}

func TestMetadataHandlerRejectsMalformedJSON(t *testing.T) {
	handler := NewHandler(storage.NewMemoryStore(), HandlerConfig{Now: fixedNow})

	request := httptest.NewRequest(http.MethodPost, PrefixV1+"/domains", strings.NewReader("{bad json"))
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)

	if response.Code != http.StatusBadRequest {
		t.Fatalf("unexpected status: %d body=%s", response.Code, response.Body.String())
	}
	var body ErrorResponse
	decodeResponse(t, response, &body)
	if body.ErrorCode != string(storage.ErrInvalidRequest) {
		t.Fatalf("unexpected error response: %#v", body)
	}
}

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

func TestObjectVersionHandlersUploadReadMetadataAndPayload(t *testing.T) {
	store := storage.NewMemoryStore()
	createDomainForObjectHandlerTest(t, store, "domain-a", "device-a", 1)
	handler := NewHandler(store, HandlerConfig{Now: fixedNow})
	payload := []byte("encrypted object payload")
	upload := objectUploadRequestForHandlerTest("domain-a", "object-a", "device-a", 1, 0, 1, payload)

	createResponse := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains/domain-a/objects/object-a/versions", upload)
	if createResponse.Code != http.StatusCreated {
		t.Fatalf("unexpected upload status: %d body=%s", createResponse.Code, createResponse.Body.String())
	}
	var created ObjectVersionResponse
	decodeResponse(t, createResponse, &created)
	if created.DomainID != "domain-a" ||
		created.ObjectID != "object-a" ||
		created.Version != 1 ||
		created.CiphertextHash != storage.CiphertextHash(payload) ||
		created.EncryptedPayloadLen != int64(len(payload)) {
		t.Fatalf("unexpected object metadata: %#v", created)
	}
	if strings.Contains(createResponse.Body.String(), string(payload)) ||
		strings.Contains(createResponse.Body.String(), "blob_ref") {
		t.Fatalf("metadata response leaked payload or blob ref: %s", createResponse.Body.String())
	}

	metadataResponse := httptest.NewRecorder()
	handler.ServeHTTP(metadataResponse, httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/objects/object-a/versions/1", nil))
	if metadataResponse.Code != http.StatusOK {
		t.Fatalf("unexpected metadata status: %d body=%s", metadataResponse.Code, metadataResponse.Body.String())
	}
	var metadata ObjectVersionResponse
	decodeResponse(t, metadataResponse, &metadata)
	if metadata.CiphertextHash != storage.CiphertextHash(payload) || metadata.SignatureKeyID != "signing-key-a" {
		t.Fatalf("unexpected read metadata: %#v", metadata)
	}
	if strings.Contains(metadataResponse.Body.String(), string(payload)) {
		t.Fatalf("metadata read leaked payload: %s", metadataResponse.Body.String())
	}

	payloadResponse := httptest.NewRecorder()
	handler.ServeHTTP(payloadResponse, httptest.NewRequest(http.MethodGet, PrefixV1+"/domains/domain-a/objects/object-a/versions/1/payload", nil))
	if payloadResponse.Code != http.StatusOK {
		t.Fatalf("unexpected payload status: %d body=%s", payloadResponse.Code, payloadResponse.Body.String())
	}
	if payloadResponse.Header().Get("Content-Type") != "application/octet-stream" {
		t.Fatalf("unexpected payload content type: %q", payloadResponse.Header().Get("Content-Type"))
	}
	if !bytes.Equal(payloadResponse.Body.Bytes(), payload) {
		t.Fatalf("payload mismatch: got %x want %x", payloadResponse.Body.Bytes(), payload)
	}
}

func TestObjectVersionUploadRejectsCiphertextMetadataMismatchWithoutLeakingPayload(t *testing.T) {
	audit := &auditSinkStub{}
	store := storage.NewMemoryStore()
	createDomainForObjectHandlerTest(t, store, "domain-a", "device-a", 1)
	handler := NewHandler(store, HandlerConfig{
		Now:       fixedNow,
		RequestID: fixedRequestID,
		AuditSink: audit,
	})
	payload := []byte("sensitive encrypted payload fixture")
	upload := objectUploadRequestForHandlerTest("domain-a", "object-a", "device-a", 1, 0, 1, payload)
	upload.CiphertextHash = storage.CiphertextHash([]byte("different encrypted bytes"))
	signObjectUploadRequestForHandlerTest(&upload, "domain-a", "object-a")

	response := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains/domain-a/objects/object-a/versions", upload)
	if response.Code != http.StatusBadRequest {
		t.Fatalf("unexpected status: %d body=%s", response.Code, response.Body.String())
	}
	var body ErrorResponse
	decodeResponse(t, response, &body)
	if body.ErrorCode != string(storage.ErrInvalidCiphertextMetadata) {
		t.Fatalf("unexpected error response: %#v", body)
	}
	if strings.Contains(response.Body.String(), string(payload)) {
		t.Fatalf("error response leaked payload: %s", response.Body.String())
	}
	if len(audit.events) != 1 {
		t.Fatalf("expected one audit event, got %d", len(audit.events))
	}
	eventText := fmt.Sprintf("%#v", audit.events[0])
	if strings.Contains(eventText, string(payload)) || strings.Contains(eventText, "payload") {
		t.Fatalf("audit event leaked payload fields: %s", eventText)
	}
	if audit.events[0].ObjectID != "object-a" ||
		audit.events[0].Version != 1 ||
		audit.events[0].ResultCode != string(storage.ErrInvalidCiphertextMetadata) {
		t.Fatalf("unexpected audit event: %#v", audit.events[0])
	}
}

func TestObjectVersionUploadRejectsPlaintextFields(t *testing.T) {
	store := storage.NewMemoryStore()
	createDomainForObjectHandlerTest(t, store, "domain-a", "device-a", 1)
	handler := NewHandler(store, HandlerConfig{Now: fixedNow})
	request := httptest.NewRequest(http.MethodPost, PrefixV1+"/domains/domain-a/objects/object-a/versions", strings.NewReader(`{"plaintext_user_term":"不能进入服务端"}`))
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)

	if response.Code != http.StatusBadRequest {
		t.Fatalf("unexpected status: %d body=%s", response.Code, response.Body.String())
	}
	var body ErrorResponse
	decodeResponse(t, response, &body)
	if body.ErrorCode != string(storage.ErrInvalidRequest) {
		t.Fatalf("unexpected error response: %#v", body)
	}
}

func TestObjectVersionUploadReturnsLatestMetadataForStaleBase(t *testing.T) {
	store := storage.NewMemoryStore()
	createDomainForObjectHandlerTest(t, store, "domain-a", "device-a", 1)
	handler := NewHandler(store, HandlerConfig{Now: fixedNow})
	first := objectUploadRequestForHandlerTest("domain-a", "object-a", "device-a", 1, 0, 1, []byte("encrypted-v1"))
	second := objectUploadRequestForHandlerTest("domain-a", "object-a", "device-a", 2, 1, 1, []byte("encrypted-v2"))
	for _, upload := range []ObjectVersionUploadRequest{first, second} {
		response := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains/domain-a/objects/object-a/versions", upload)
		if response.Code != http.StatusCreated {
			t.Fatalf("setup upload failed: status=%d body=%s", response.Code, response.Body.String())
		}
	}

	stalePayload := []byte("stale encrypted payload")
	stale := objectUploadRequestForHandlerTest("domain-a", "object-a", "device-a", 3, 1, 1, stalePayload)
	response := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains/domain-a/objects/object-a/versions", stale)
	if response.Code != http.StatusConflict {
		t.Fatalf("unexpected stale status: %d body=%s", response.Code, response.Body.String())
	}
	var body ErrorResponse
	decodeResponse(t, response, &body)
	if body.ErrorCode != string(storage.ErrConflictStaleBaseVersion) ||
		body.LatestVersion != 2 ||
		body.LatestCiphertextHash != second.CiphertextHash {
		t.Fatalf("latest metadata missing from stale response: %#v", body)
	}
	if strings.Contains(response.Body.String(), string(stalePayload)) {
		t.Fatalf("stale response leaked payload: %s", response.Body.String())
	}
}

func TestObjectVersionUploadIsIdempotentForSameVersionHashAndRejectsDifferentHash(t *testing.T) {
	store := storage.NewMemoryStore()
	createDomainForObjectHandlerTest(t, store, "domain-a", "device-a", 1)
	handler := NewHandler(store, HandlerConfig{Now: fixedNow})
	upload := objectUploadRequestForHandlerTest("domain-a", "object-a", "device-a", 1, 0, 1, []byte("encrypted-v1"))

	first := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains/domain-a/objects/object-a/versions", upload)
	if first.Code != http.StatusCreated {
		t.Fatalf("unexpected first status: %d body=%s", first.Code, first.Body.String())
	}
	retry := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains/domain-a/objects/object-a/versions", upload)
	if retry.Code != http.StatusCreated {
		t.Fatalf("unexpected retry status: %d body=%s", retry.Code, retry.Body.String())
	}
	var retryBody ObjectVersionResponse
	decodeResponse(t, retry, &retryBody)
	if retryBody.CiphertextHash != upload.CiphertextHash {
		t.Fatalf("idempotent retry returned wrong metadata: %#v", retryBody)
	}

	conflicting := objectUploadRequestForHandlerTest("domain-a", "object-a", "device-a", 1, 0, 1, []byte("encrypted-v1-conflict"))
	response := performJSONRequest(t, handler, http.MethodPost, PrefixV1+"/domains/domain-a/objects/object-a/versions", conflicting)
	if response.Code != http.StatusConflict {
		t.Fatalf("unexpected conflict status: %d body=%s", response.Code, response.Body.String())
	}
	var body ErrorResponse
	decodeResponse(t, response, &body)
	if body.ErrorCode != string(storage.ErrConflictObjectVersion) {
		t.Fatalf("unexpected conflict response: %#v", body)
	}
}

func TestObjectVersionUploadRejectsRevokedPendingAndUnknownDevices(t *testing.T) {
	t.Run("pending device", func(t *testing.T) {
		store := storage.NewMemoryStore()
		createDomainForObjectHandlerTest(t, store, "domain-a", "device-a", 1)
		if err := store.SaveJoinRequest(context.Background(), storage.JoinRequest{
			DomainID:                "domain-a",
			JoinRequestID:           "join-b",
			DeviceID:                "device-b",
			SigningPublicKeyID:      signingKeyIDForHandlerTest("device-b"),
			SigningPublicKey:        signingPublicKeyForHandlerTest("device-b"),
			KeyAgreementPublicKeyID: "agreement-key-b",
			KeyAgreementPublicKey:   []byte{0x12},
			Challenge:               []byte{0x13},
			CreatedAtMs:             10,
			ExpiresAtMs:             110,
			Status:                  storage.DevicePending,
		}); err != nil {
			t.Fatalf("save join request: %v", err)
		}
		handler := NewHandler(store, HandlerConfig{Now: fixedNow})
		upload := objectUploadRequestForHandlerTest("domain-a", "object-b", "device-b", 1, 0, 1, []byte("pending-device-payload"))
		assertForbiddenObjectUpload(t, handler, PrefixV1+"/domains/domain-a/objects/object-b/versions", upload)
	})

	t.Run("unknown device", func(t *testing.T) {
		store := storage.NewMemoryStore()
		createDomainForObjectHandlerTest(t, store, "domain-a", "device-a", 1)
		handler := NewHandler(store, HandlerConfig{Now: fixedNow})
		upload := objectUploadRequestForHandlerTest("domain-a", "object-c", "device-c", 1, 0, 1, []byte("unknown-device-payload"))
		assertForbiddenObjectUpload(t, handler, PrefixV1+"/domains/domain-a/objects/object-c/versions", upload)
	})

	t.Run("revoked device", func(t *testing.T) {
		store := storage.NewMemoryStore()
		createDomainForObjectHandlerTest(t, store, "domain-b", "device-b", 1)
		revocation := storage.DeviceRevocation{
			DomainID:         "domain-b",
			RevokedDeviceID:  "device-b",
			RevokerDeviceID:  "device-b",
			PreviousKeyEpoch: 1,
			NewKeyEpoch:      2,
			Reason:           "lost",
			CreatedAtMs:      20,
		}
		signRevocationForHandlerTest(&revocation)
		if err := store.RevokeDevice(context.Background(), revocation); err != nil {
			t.Fatalf("revoke device: %v", err)
		}
		handler := NewHandler(store, HandlerConfig{Now: fixedNow})
		upload := objectUploadRequestForHandlerTest("domain-b", "object-d", "device-b", 1, 0, 2, []byte("revoked-device-payload"))
		assertForbiddenObjectUpload(t, handler, PrefixV1+"/domains/domain-b/objects/object-d/versions", upload)
	})
}

func fixedNow() time.Time {
	return time.UnixMilli(1234)
}

func fixedRequestID() string {
	return "req-fixed"
}

func performJSONRequest(t *testing.T, handler http.Handler, method string, path string, value any) *httptest.ResponseRecorder {
	t.Helper()
	body, err := json.Marshal(value)
	if err != nil {
		t.Fatalf("marshal request: %v", err)
	}
	request := httptest.NewRequest(method, path, bytes.NewReader(body))
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, request)
	return response
}

func decodeResponse(t *testing.T, response *httptest.ResponseRecorder, value any) {
	t.Helper()
	if err := json.Unmarshal(response.Body.Bytes(), value); err != nil {
		t.Fatalf("decode response: %v body=%s", err, response.Body.String())
	}
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

type auditSinkStub struct {
	events []AuditEvent
}

func (s *auditSinkStub) RecordAuditEvent(event AuditEvent) {
	s.events = append(s.events, event)
}

type panicDomainStore struct {
	storage.Store
}

func (s *panicDomainStore) Domain(ctx context.Context, domainID string) (storage.Domain, error) {
	panic("domain panic should be recovered")
}

type authorizationStoreStub struct {
	storage.Store
	upload storage.DeviceAuthorizationUpload
	calls  int
}

func (s *authorizationStoreStub) AuthorizeJoinRequest(ctx context.Context, upload storage.DeviceAuthorizationUpload) error {
	s.calls++
	s.upload = upload
	return nil
}

type persistentAuditStoreStub struct {
	storage.Store
	auditEvents []storage.AuditEvent
}

func (s *persistentAuditStoreStub) Domain(ctx context.Context, domainID string) (storage.Domain, error) {
	return storage.Domain{
		DomainID:        domainID,
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "epoch-key-a",
		CreatedAtMs:     100,
		UpdatedAtMs:     100,
	}, nil
}

func (s *persistentAuditStoreStub) RecordAuditEvent(ctx context.Context, event storage.AuditEvent) error {
	s.auditEvents = append(s.auditEvents, event)
	return nil
}

func createDomainForObjectHandlerTest(t *testing.T, store storage.Store, domainID string, deviceID string, keyEpoch uint64) {
	t.Helper()
	err := store.CreateDomain(context.Background(), storage.Domain{
		DomainID:        domainID,
		CurrentKeyEpoch: keyEpoch,
		ActiveKeyID:     "sync-key-" + deviceID,
		CreatedAtMs:     1,
		UpdatedAtMs:     1,
	}, storage.Device{
		DomainID:                domainID,
		DeviceID:                deviceID,
		SigningPublicKeyID:      signingKeyIDForHandlerTest(deviceID),
		SigningPublicKey:        signingPublicKeyForHandlerTest(deviceID),
		KeyAgreementPublicKeyID: "agreement-key-" + deviceID,
		KeyAgreementPublicKey:   []byte{0x02},
		Status:                  storage.DeviceActive,
		AuthorizedAtMs:          1,
	})
	if err != nil {
		t.Fatalf("create domain: %v", err)
	}
}

func objectUploadRequestForHandlerTest(domainID string, objectID string, deviceID string, version uint64, baseVersion uint64, keyEpoch uint64, payload []byte) ObjectVersionUploadRequest {
	request := ObjectVersionUploadRequest{
		ObjectType:             storage.ObjectDictionaryUserTerms,
		Version:                version,
		BaseVersion:            baseVersion,
		OwnerDeviceID:          deviceID,
		KeyID:                  "object-key-a",
		KeyEpoch:               keyEpoch,
		Algorithm:              storage.AlgorithmXChaCha20Poly1305HKDFSHA256,
		Nonce:                  []byte{byte(version), byte(baseVersion), byte(keyEpoch)},
		EncryptedPayloadLen:    int64(len(payload)),
		CiphertextHash:         storage.CiphertextHash(payload),
		ClientCreatedAtMs:      100 + int64(version),
		ClientUpdatedAtMs:      100 + int64(version),
		Payload:                cloneBytes(payload),
		SignatureSchemaVersion: 1,
		SignatureAlgorithm:     "ed25519-v1",
		SignatureKeyID:         signingKeyIDForHandlerTest(deviceID),
	}
	signObjectUploadRequestForHandlerTest(&request, domainID, objectID)
	return request
}

func signObjectUploadRequestForHandlerTest(request *ObjectVersionUploadRequest, domainID string, objectID string) {
	request.SignatureSchemaVersion = 1
	request.SignatureAlgorithm = "ed25519-v1"
	request.SignatureKeyID = signingKeyIDForHandlerTest(request.OwnerDeviceID)
	request.Signature = ed25519.Sign(signingPrivateKeyForHandlerTest(request.OwnerDeviceID), canonicalSignatureBytesForHandlerTest("sync_object_manifest", []signatureFieldForHandlerTest{
		textFieldForHandlerTest("signature_schema_version", "1"),
		textFieldForHandlerTest("signature_algorithm", "ed25519-v1"),
		textFieldForHandlerTest("signature_key_id", request.SignatureKeyID),
		textFieldForHandlerTest("signer_device_id", request.OwnerDeviceID),
		textFieldForHandlerTest("domain_id", domainID),
		textFieldForHandlerTest("object_id", objectID),
		textFieldForHandlerTest("object_type", request.ObjectType),
		textFieldForHandlerTest("version", uint64StringForHandlerTest(request.Version)),
		textFieldForHandlerTest("base_version", optionalBaseVersionStringForHandlerTest(request.BaseVersion)),
		textFieldForHandlerTest("key_id", request.KeyID),
		textFieldForHandlerTest("key_epoch", uint64StringForHandlerTest(request.KeyEpoch)),
		textFieldForHandlerTest("envelope_algorithm", request.Algorithm),
		bytesFieldForHandlerTest("nonce", request.Nonce),
		textFieldForHandlerTest("encrypted_payload_len", int64StringForHandlerTest(request.EncryptedPayloadLen)),
		textFieldForHandlerTest("ciphertext_hash", request.CiphertextHash),
		textFieldForHandlerTest("created_at_ms", int64StringForHandlerTest(request.ClientCreatedAtMs)),
		textFieldForHandlerTest("updated_at_ms", int64StringForHandlerTest(request.ClientUpdatedAtMs)),
	}))
}

func signRevocationForHandlerTest(revocation *storage.DeviceRevocation) {
	revocation.SignatureSchemaVersion = 1
	revocation.SignatureAlgorithm = "ed25519-v1"
	revocation.SignatureKeyID = signingKeyIDForHandlerTest(revocation.RevokerDeviceID)
	revocation.Signature = ed25519.Sign(signingPrivateKeyForHandlerTest(revocation.RevokerDeviceID), canonicalSignatureBytesForHandlerTest("device_revocation", []signatureFieldForHandlerTest{
		textFieldForHandlerTest("signature_schema_version", "1"),
		textFieldForHandlerTest("signature_algorithm", "ed25519-v1"),
		textFieldForHandlerTest("signature_key_id", revocation.SignatureKeyID),
		textFieldForHandlerTest("revoked_by_device_id", revocation.RevokerDeviceID),
		textFieldForHandlerTest("revoked_device_id", revocation.RevokedDeviceID),
		textFieldForHandlerTest("previous_key_epoch", uint64StringForHandlerTest(revocation.PreviousKeyEpoch)),
		textFieldForHandlerTest("new_key_epoch", uint64StringForHandlerTest(revocation.NewKeyEpoch)),
		textFieldForHandlerTest("reason", revocation.Reason),
		textFieldForHandlerTest("revoked_at_ms", int64StringForHandlerTest(revocation.CreatedAtMs)),
	}))
}

func assertForbiddenObjectUpload(t *testing.T, handler http.Handler, path string, upload ObjectVersionUploadRequest) {
	t.Helper()
	response := performJSONRequest(t, handler, http.MethodPost, path, upload)
	if response.Code != http.StatusForbidden {
		t.Fatalf("unexpected forbidden status: %d body=%s", response.Code, response.Body.String())
	}
	var body ErrorResponse
	decodeResponse(t, response, &body)
	if body.ErrorCode != string(storage.ErrForbiddenDevice) {
		t.Fatalf("unexpected forbidden response: %#v", body)
	}
}

type signatureFieldForHandlerTest struct {
	name  string
	value []byte
}

func canonicalSignatureBytesForHandlerTest(recordType string, fields []signatureFieldForHandlerTest) []byte {
	var out []byte
	out = appendSignatureFieldForHandlerTest(out, "domain_separator", []byte("radishlex-signature-v1"))
	out = appendSignatureFieldForHandlerTest(out, "record_type", []byte(recordType))
	for _, field := range fields {
		out = appendSignatureFieldForHandlerTest(out, field.name, field.value)
	}
	return out
}

func appendSignatureFieldForHandlerTest(out []byte, name string, value []byte) []byte {
	out = append(out, []byte(name)...)
	out = append(out, '=')
	var length [8]byte
	binary.BigEndian.PutUint64(length[:], uint64(len(value)))
	out = append(out, length[:]...)
	out = append(out, value...)
	out = append(out, 0)
	return out
}

func textFieldForHandlerTest(name string, value string) signatureFieldForHandlerTest {
	return signatureFieldForHandlerTest{name: name, value: []byte(value)}
}

func bytesFieldForHandlerTest(name string, value []byte) signatureFieldForHandlerTest {
	return signatureFieldForHandlerTest{name: name, value: cloneBytes(value)}
}

func signingPublicKeyForHandlerTest(deviceID string) []byte {
	return signingPrivateKeyForHandlerTest(deviceID).Public().(ed25519.PublicKey)
}

func signingPrivateKeyForHandlerTest(deviceID string) ed25519.PrivateKey {
	return ed25519.NewKeyFromSeed(signingSeedForHandlerTest(deviceID))
}

func signingSeedForHandlerTest(deviceID string) []byte {
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

func signingKeyIDForHandlerTest(deviceID string) string {
	if deviceID == "device-a" {
		return "signing-key-a"
	}
	return "signing-key-" + deviceID
}

func optionalBaseVersionStringForHandlerTest(value uint64) string {
	if value == 0 {
		return ""
	}
	return uint64StringForHandlerTest(value)
}

func uint64StringForHandlerTest(value uint64) string {
	return strconv.FormatUint(value, 10)
}

func int64StringForHandlerTest(value int64) string {
	return strconv.FormatInt(value, 10)
}
