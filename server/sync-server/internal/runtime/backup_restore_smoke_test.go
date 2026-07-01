package runtime

import (
	"bytes"
	"context"
	"crypto/ed25519"
	"database/sql"
	"io"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/api"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/config"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

func TestLocalServerBackupRestorePreservesEncryptedSyncState(t *testing.T) {
	cfg := runtimeConfigForTest(t)
	var sourceLogs bytes.Buffer
	server, closeStore, err := NewHTTPServer(cfg, log.New(&sourceLogs, "", 0))
	if err != nil {
		t.Fatalf("create source http server: %v", err)
	}
	httpServer := httptest.NewServer(server.Handler)

	createBackupSmokeDomain(t, httpServer.URL)
	authorizeBackupSmokeDevice(t, httpServer.URL)

	payloads := map[string][]byte{
		storage.ObjectDictionaryUserTerms:    []byte("encrypted-user-terms-v1"),
		storage.ObjectRankerWeights:          []byte("encrypted-ranker-weights-v1"),
		storage.ObjectDictionaryDeletedTerms: []byte("encrypted-deleted-terms-tombstone-v1"),
	}
	for objectType, payload := range payloads {
		upload := smokeObjectUploadWithType("domain-backup", smokeObjectID(objectType), objectType, "device-smoke", 1, 0, 1, payload)
		response := doJSONSmokeRequest(t, http.MethodPost, httpServer.URL+api.PrefixV1+"/domains/domain-backup/objects/"+smokeObjectID(objectType)+"/versions", upload)
		if response.StatusCode != http.StatusCreated {
			t.Fatalf("unexpected %s upload status: %d body=%s", objectType, response.StatusCode, string(response.Body))
		}
	}

	stalePayload := []byte("stale-deleted-terms-would-revive-old-state")
	stale := smokeObjectUploadWithType("domain-backup", smokeObjectID(storage.ObjectDictionaryDeletedTerms), storage.ObjectDictionaryDeletedTerms, "device-b", 2, 0, 1, stalePayload)
	staleResponse := doJSONSmokeRequest(t, http.MethodPost, httpServer.URL+api.PrefixV1+"/domains/domain-backup/objects/"+smokeObjectID(storage.ObjectDictionaryDeletedTerms)+"/versions", stale)
	if staleResponse.StatusCode != http.StatusConflict {
		t.Fatalf("unexpected source stale status: %d body=%s", staleResponse.StatusCode, string(staleResponse.Body))
	}

	httpServer.Close()
	if err := closeStore(); err != nil {
		t.Fatalf("close source store before backup: %v", err)
	}

	writeBackupSmokeRecoveryRecord(t, cfg)

	sourceRoot := filepath.Dir(cfg.MetadataPath)
	backupRoot := filepath.Join(t.TempDir(), "backup-data")
	if err := copyDirectory(sourceRoot, backupRoot); err != nil {
		t.Fatalf("copy source data to backup: %v", err)
	}
	restoreRoot := filepath.Join(t.TempDir(), "restored-data")
	if err := copyDirectory(backupRoot, restoreRoot); err != nil {
		t.Fatalf("restore backup data to isolated directory: %v", err)
	}

	restoredCfg := cfg
	restoredCfg.MetadataPath = filepath.Join(restoreRoot, filepath.Base(cfg.MetadataPath))
	restoredCfg.BlobDir = filepath.Join(restoreRoot, filepath.Base(cfg.BlobDir))
	var restoredLogs bytes.Buffer
	restoredServer, closeRestoredStore, err := NewHTTPServer(restoredCfg, log.New(&restoredLogs, "", 0))
	if err != nil {
		t.Fatalf("create restored http server: %v", err)
	}
	t.Cleanup(func() {
		_ = closeRestoredStore()
	})
	restoredHTTPServer := httptest.NewServer(restoredServer.Handler)
	t.Cleanup(restoredHTTPServer.Close)

	verifyRestoredBackupSmokeState(t, restoredHTTPServer.URL, payloads, stalePayload)
	verifyRestoredAuditEvents(t, restoredCfg.MetadataPath)

	for _, logText := range []string{sourceLogs.String(), restoredLogs.String()} {
		for _, forbidden := range []string{
			string(payloads[storage.ObjectDictionaryUserTerms]),
			string(payloads[storage.ObjectRankerWeights]),
			string(payloads[storage.ObjectDictionaryDeletedTerms]),
			string(stalePayload),
			string(backupSmokeWrappedMaterial()),
			string(backupSmokeWrappedKey()),
			"recovery-code",
			"sync-master-key",
		} {
			if forbidden != "" && strings.Contains(logText, forbidden) {
				t.Fatalf("runtime log leaked sensitive fixture %q in %s", forbidden, logText)
			}
		}
	}
}

func createBackupSmokeDomain(t *testing.T, baseURL string) {
	t.Helper()
	createDomain := api.CreateDomainRequest{
		DomainID:        "domain-backup",
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "sync-key-backup",
		FirstDevice: api.DeviceMetadata{
			DeviceID:                "device-smoke",
			SigningPublicKeyID:      smokeSigningKeyID("device-smoke"),
			SigningPublicKey:        smokeSigningPublicKey("device-smoke"),
			KeyAgreementPublicKeyID: "agreement-key-smoke",
			KeyAgreementPublicKey:   []byte{0x41, 0x42},
			Status:                  string(storage.DeviceActive),
		},
		CreatedAtMs: 100,
		UpdatedAtMs: 100,
	}
	response := doJSONSmokeRequest(t, http.MethodPost, baseURL+api.PrefixV1+"/domains", createDomain)
	if response.StatusCode != http.StatusCreated {
		t.Fatalf("unexpected create domain status: %d body=%s", response.StatusCode, string(response.Body))
	}
}

func authorizeBackupSmokeDevice(t *testing.T, baseURL string) {
	t.Helper()
	join := smokeJoinRequest("join-device-b", "device-b", 150)
	joinResponse := doJSONSmokeRequest(t, http.MethodPost, baseURL+api.PrefixV1+"/domains/domain-backup/join-requests", join)
	if joinResponse.StatusCode != http.StatusCreated {
		t.Fatalf("unexpected join request status: %d body=%s", joinResponse.StatusCode, string(joinResponse.Body))
	}
	authorization := smokeJoinAuthorization(join, 160)
	authorizationResponse := doJSONSmokeRequest(t, http.MethodPost, baseURL+api.PrefixV1+"/domains/domain-backup/join-requests/join-device-b/authorization", authorization)
	if authorizationResponse.StatusCode != http.StatusNoContent {
		t.Fatalf("unexpected authorization status: %d body=%s", authorizationResponse.StatusCode, string(authorizationResponse.Body))
	}
}

func writeBackupSmokeRecoveryRecord(t *testing.T, cfg config.Config) {
	t.Helper()
	store, closeStore, err := OpenStore(cfg)
	if err != nil {
		t.Fatalf("open store for recovery record: %v", err)
	}
	defer func() {
		if err := closeStore(); err != nil {
			t.Fatalf("close recovery store: %v", err)
		}
	}()

	wrapped := backupSmokeWrappedMaterial()
	record := storage.RecoveryRecord{
		DomainID:           "domain-backup",
		RecoveryRecordID:   "recovery-backup",
		KeyEpoch:           1,
		KDFProfile:         "argon2id-v1",
		KDFVersion:         1,
		MemoryKiB:          65536,
		Iterations:         3,
		Parallelism:        1,
		OutputLen:          32,
		Salt:               []byte{0x31, 0x32},
		Algorithm:          storage.AlgorithmXChaCha20Poly1305HKDFSHA256,
		Nonce:              []byte{0x33, 0x34},
		WrappedMaterialLen: int64(len(wrapped)),
		CiphertextHash:     storage.CiphertextHash(wrapped),
		Status:             storage.RecoveryRecordActive,
		CreatedAtMs:        180,
		SignerDeviceID:     "device-smoke",
	}
	signSmokeRecoveryRecord(&record)
	if _, err := store.PutRecoveryRecord(context.Background(), storage.RecoveryRecordUpload{Record: record, WrappedMaterial: wrapped}); err != nil {
		t.Fatalf("put recovery record: %v", err)
	}
}

func verifyRestoredBackupSmokeState(t *testing.T, baseURL string, payloads map[string][]byte, stalePayload []byte) {
	t.Helper()
	stateResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/state", nil)
	if stateResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected restored domain status: %d body=%s", stateResponse.StatusCode, string(stateResponse.Body))
	}
	var state api.DomainStateResponse
	decodeSmokeResponse(t, stateResponse.Body, &state)
	if state.Domain.CurrentKeyEpoch != 1 || state.Domain.ActiveKeyID != "sync-key-backup" {
		t.Fatalf("unexpected restored domain state: %#v", state)
	}

	deviceResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/devices/device-b", nil)
	if deviceResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected restored device status: %d body=%s", deviceResponse.StatusCode, string(deviceResponse.Body))
	}
	var device api.DeviceResponse
	decodeSmokeResponse(t, deviceResponse.Body, &device)
	if device.Status != storage.DeviceActive || device.AuthorizedAtMs != 160 {
		t.Fatalf("unexpected restored device state: %#v", device)
	}

	recoveryResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/recovery-records/latest", nil)
	if recoveryResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected restored recovery status: %d body=%s", recoveryResponse.StatusCode, string(recoveryResponse.Body))
	}
	var recovery api.RecoveryRecordResponse
	decodeSmokeResponse(t, recoveryResponse.Body, &recovery)
	if recovery.RecoveryRecordID != "recovery-backup" ||
		recovery.CiphertextHash != storage.CiphertextHash(backupSmokeWrappedMaterial()) ||
		!bytes.Equal(recovery.WrappedMaterial, backupSmokeWrappedMaterial()) {
		t.Fatalf("unexpected restored recovery response: %#v", recovery)
	}
	if strings.Contains(string(recoveryResponse.Body), "blob_ref") {
		t.Fatalf("restored recovery response leaked internal blob ref: %s", string(recoveryResponse.Body))
	}

	for objectType, payload := range payloads {
		objectID := smokeObjectID(objectType)
		metadataResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/objects/"+objectID+"/versions/1", nil)
		if metadataResponse.StatusCode != http.StatusOK {
			t.Fatalf("unexpected restored %s metadata status: %d body=%s", objectType, metadataResponse.StatusCode, string(metadataResponse.Body))
		}
		var metadata api.ObjectVersionResponse
		decodeSmokeResponse(t, metadataResponse.Body, &metadata)
		if metadata.ObjectType != objectType ||
			metadata.Version != 1 ||
			metadata.CiphertextHash != storage.ObjectCiphertextHash(metadataToStorageVersion(metadata), payload) {
			t.Fatalf("unexpected restored %s metadata: %#v", objectType, metadata)
		}
		if strings.Contains(string(metadataResponse.Body), string(payload)) {
			t.Fatalf("restored metadata response leaked %s payload: %s", objectType, string(metadataResponse.Body))
		}

		payloadResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/objects/"+objectID+"/versions/1/payload", nil)
		if payloadResponse.StatusCode != http.StatusOK {
			t.Fatalf("unexpected restored %s payload status: %d body=%s", objectType, payloadResponse.StatusCode, string(payloadResponse.Body))
		}
		if !bytes.Equal(payloadResponse.Body, payload) {
			t.Fatalf("restored %s payload mismatch: got %x want %x", objectType, payloadResponse.Body, payload)
		}
	}

	deletedObjectID := smokeObjectID(storage.ObjectDictionaryDeletedTerms)
	stale := smokeObjectUploadWithType("domain-backup", deletedObjectID, storage.ObjectDictionaryDeletedTerms, "device-b", 2, 0, 1, stalePayload)
	staleResponse := doJSONSmokeRequest(t, http.MethodPost, baseURL+api.PrefixV1+"/domains/domain-backup/objects/"+deletedObjectID+"/versions", stale)
	if staleResponse.StatusCode != http.StatusConflict {
		t.Fatalf("unexpected restored stale status: %d body=%s", staleResponse.StatusCode, string(staleResponse.Body))
	}
	var staleBody api.ErrorResponse
	decodeSmokeResponse(t, staleResponse.Body, &staleBody)
	expectedHash := restoredObjectCiphertextHash(t, baseURL, deletedObjectID)
	if staleBody.ErrorCode != string(storage.ErrConflictStaleBaseVersion) ||
		staleBody.LatestVersion != 1 ||
		staleBody.LatestCiphertextHash != expectedHash {
		t.Fatalf("unexpected restored stale response: %#v", staleBody)
	}
	if strings.Contains(string(staleResponse.Body), string(stalePayload)) {
		t.Fatalf("restored stale response leaked payload: %s", string(staleResponse.Body))
	}
}

func restoredObjectCiphertextHash(t *testing.T, baseURL string, objectID string) string {
	t.Helper()
	response := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/objects/"+objectID+"/versions/1", nil)
	if response.StatusCode != http.StatusOK {
		t.Fatalf("unexpected restored metadata status: %d body=%s", response.StatusCode, string(response.Body))
	}
	var metadata api.ObjectVersionResponse
	decodeSmokeResponse(t, response.Body, &metadata)
	return metadata.CiphertextHash
}

func verifyRestoredAuditEvents(t *testing.T, metadataPath string) {
	t.Helper()
	db, err := sql.Open("sqlite", metadataPath)
	if err != nil {
		t.Fatalf("open restored sqlite for audit inspection: %v", err)
	}
	defer db.Close()

	var objectEvents int
	if err := db.QueryRow(`SELECT COUNT(*) FROM audit_events WHERE domain_id = ? AND event_type = ? AND result_code = ?`, "domain-backup", "objects.versions.create", "ok").Scan(&objectEvents); err != nil {
		t.Fatalf("query restored object audit events: %v", err)
	}
	if objectEvents < 3 {
		t.Fatalf("expected restored audit events for three P2 uploads, got %d", objectEvents)
	}

	var staleEvents int
	if err := db.QueryRow(`SELECT COUNT(*) FROM audit_events WHERE domain_id = ? AND result_code = ?`, "domain-backup", string(storage.ErrConflictStaleBaseVersion)).Scan(&staleEvents); err != nil {
		t.Fatalf("query restored stale audit event: %v", err)
	}
	if staleEvents < 1 {
		t.Fatalf("expected restored stale conflict audit event, got %d", staleEvents)
	}
}

func smokeObjectUploadWithType(domainID string, objectID string, objectType string, deviceID string, version uint64, baseVersion uint64, keyEpoch uint64, payload []byte) api.ObjectVersionUploadRequest {
	request := smokeObjectUpload(domainID, objectID, deviceID, version, baseVersion, keyEpoch, payload)
	request.ObjectType = objectType
	request.CiphertextHash = storage.ObjectCiphertextHash(
		request.StorageVersion(domainID, objectID),
		payload,
	)
	signSmokeObjectUpload(&request, domainID, objectID)
	return request
}

func smokeObjectID(objectType string) string {
	switch objectType {
	case storage.ObjectDictionaryUserTerms:
		return "dictionary-user-terms"
	case storage.ObjectRankerWeights:
		return "ranker-weights"
	case storage.ObjectDictionaryDeletedTerms:
		return "dictionary-deleted-terms"
	default:
		return "object-" + strings.ReplaceAll(objectType, ".", "-")
	}
}

func metadataToStorageVersion(metadata api.ObjectVersionResponse) storage.ObjectVersion {
	return storage.ObjectVersion{
		DomainID:            metadata.DomainID,
		ObjectID:            metadata.ObjectID,
		ObjectType:          metadata.ObjectType,
		Version:             metadata.Version,
		BaseVersion:         metadata.BaseVersion,
		OwnerDeviceID:       metadata.OwnerDeviceID,
		KeyID:               metadata.KeyID,
		KeyEpoch:            metadata.KeyEpoch,
		Algorithm:           metadata.Algorithm,
		Nonce:               metadata.Nonce,
		EncryptedPayloadLen: metadata.EncryptedPayloadLen,
		ClientCreatedAtMs:   metadata.ClientCreatedAtMs,
		ClientUpdatedAtMs:   metadata.ClientUpdatedAtMs,
	}
}

func signSmokeRecoveryRecord(record *storage.RecoveryRecord) {
	record.SignatureSchemaVersion = 1
	record.SignatureAlgorithm = "ed25519-v1"
	record.SignatureKeyID = smokeSigningKeyID(record.SignerDeviceID)
	record.Signature = ed25519.Sign(smokeSigningPrivateKey(record.SignerDeviceID), smokeCanonicalSignatureBytes("recovery_record", []smokeSignatureField{
		smokeTextField("signature_schema_version", "1"),
		smokeTextField("signature_algorithm", "ed25519-v1"),
		smokeTextField("signature_key_id", record.SignatureKeyID),
		smokeTextField("signer_device_id", record.SignerDeviceID),
		smokeTextField("recovery_id", record.RecoveryRecordID),
		smokeTextField("domain_id", record.DomainID),
		smokeTextField("key_epoch", smokeUint64String(record.KeyEpoch)),
		smokeTextField("kdf_id", record.KDFProfile),
		smokeTextField("kdf_version", "1"),
		smokeBytesField("salt", record.Salt),
		smokeTextField("memory_kib", smokeUint64String(uint64(record.MemoryKiB))),
		smokeTextField("iterations", smokeUint64String(uint64(record.Iterations))),
		smokeTextField("parallelism", smokeUint64String(uint64(record.Parallelism))),
		smokeTextField("output_len", smokeInt64String(record.OutputLen)),
		smokeTextField("envelope_algorithm", record.Algorithm),
		smokeBytesField("envelope_nonce", record.Nonce),
		smokeTextField("encrypted_recovery_key_len", smokeInt64String(record.WrappedMaterialLen)),
		smokeTextField("created_at_ms", smokeInt64String(record.CreatedAtMs)),
		smokeTextField("updated_at_ms", smokeInt64String(record.CreatedAtMs)),
	}))
}

func backupSmokeWrappedMaterial() []byte {
	return []byte("encrypted-recovery-wrapped-material")
}

func backupSmokeWrappedKey() []byte {
	return []byte{0x61, 0x62, 0x63}
}

func copyDirectory(source string, destination string) error {
	return filepath.WalkDir(source, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		relativePath, err := filepath.Rel(source, path)
		if err != nil {
			return err
		}
		target := filepath.Join(destination, relativePath)
		if entry.IsDir() {
			return os.MkdirAll(target, 0o700)
		}
		if entry.Type()&os.ModeType != 0 {
			return nil
		}
		return copyFile(path, target)
	})
}

func copyFile(source string, destination string) error {
	if err := os.MkdirAll(filepath.Dir(destination), 0o700); err != nil {
		return err
	}
	sourceFile, err := os.Open(source)
	if err != nil {
		return err
	}
	defer sourceFile.Close()

	destinationFile, err := os.OpenFile(destination, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o600)
	if err != nil {
		return err
	}
	if _, err := io.Copy(destinationFile, sourceFile); err != nil {
		_ = destinationFile.Close()
		return err
	}
	return destinationFile.Close()
}
