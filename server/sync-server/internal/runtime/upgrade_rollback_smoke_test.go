package runtime

import (
	"bytes"
	"log"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/api"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/config"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

func TestLocalServerUpgradeRollbackPreservesPreUpgradeBackup(t *testing.T) {
	cfg := runtimeConfigForTest(t)
	objectID := smokeObjectID(storage.ObjectDictionaryUserTerms)
	preUpgradePayload := []byte("encrypted-upgrade-user-terms-v1")

	var initialLogs bytes.Buffer
	initialBaseURL, closeInitial := startUpgradeRollbackSmokeServer(t, cfg, &initialLogs)
	createBackupSmokeDomain(t, initialBaseURL)
	authorizeBackupSmokeDevice(t, initialBaseURL)
	uploadUpgradeRollbackObject(t, initialBaseURL, objectID, "device-smoke", 1, 0, preUpgradePayload)
	closeInitial()

	writeBackupSmokeRecoveryRecord(t, cfg)

	sourceRoot := filepath.Dir(cfg.MetadataPath)
	preUpgradeBackupRoot := filepath.Join(t.TempDir(), "pre-upgrade-backup")
	if err := copyDirectory(sourceRoot, preUpgradeBackupRoot); err != nil {
		t.Fatalf("copy pre-upgrade data to backup: %v", err)
	}

	var upgradedLogs bytes.Buffer
	upgradedBaseURL, closeUpgraded := startUpgradeRollbackSmokeServer(t, cfg, &upgradedLogs)
	verifyUpgradeRollbackPreUpgradeState(t, upgradedBaseURL, preUpgradePayload)

	postUpgradePayload := []byte("encrypted-upgrade-user-terms-v2")
	postUpgrade := uploadUpgradeRollbackObject(t, upgradedBaseURL, objectID, "device-b", 2, 1, postUpgradePayload)
	if postUpgrade.Version != 2 || postUpgrade.BaseVersion != 1 || postUpgrade.OwnerDeviceID != "device-b" {
		t.Fatalf("unexpected post-upgrade object metadata: %#v", postUpgrade)
	}
	verifyUpgradeRollbackObjectPayload(t, upgradedBaseURL, objectID, 2, postUpgradePayload)
	closeUpgraded()

	rollbackRoot := filepath.Join(t.TempDir(), "rollback-data")
	if err := copyDirectory(preUpgradeBackupRoot, rollbackRoot); err != nil {
		t.Fatalf("restore pre-upgrade backup for rollback: %v", err)
	}
	rollbackCfg := cfg
	rollbackCfg.MetadataPath = filepath.Join(rollbackRoot, filepath.Base(cfg.MetadataPath))
	rollbackCfg.BlobDir = filepath.Join(rollbackRoot, filepath.Base(cfg.BlobDir))

	var rollbackLogs bytes.Buffer
	rollbackBaseURL, closeRollback := startUpgradeRollbackSmokeServer(t, rollbackCfg, &rollbackLogs)
	t.Cleanup(closeRollback)
	verifyUpgradeRollbackPreUpgradeState(t, rollbackBaseURL, preUpgradePayload)
	verifyUpgradeRollbackPostUpgradeVersionAbsent(t, rollbackBaseURL, objectID)

	staleRollbackPayload := []byte("encrypted-upgrade-rollback-stale")
	stale := smokeObjectUploadWithType("domain-backup", objectID, storage.ObjectDictionaryUserTerms, "device-b", 2, 0, 1, staleRollbackPayload)
	staleResponse := doJSONSmokeRequest(t, http.MethodPost, rollbackBaseURL+api.PrefixV1+"/domains/domain-backup/objects/"+objectID+"/versions", stale)
	if staleResponse.StatusCode != http.StatusConflict {
		t.Fatalf("unexpected rollback stale status: %d body=%s", staleResponse.StatusCode, string(staleResponse.Body))
	}
	var staleBody api.ErrorResponse
	decodeSmokeResponse(t, staleResponse.Body, &staleBody)
	expectedHash := upgradeRollbackObjectCiphertextHash(t, rollbackBaseURL, objectID, 1)
	if staleBody.ErrorCode != string(storage.ErrConflictStaleBaseVersion) ||
		staleBody.LatestVersion != 1 ||
		staleBody.LatestCiphertextHash != expectedHash {
		t.Fatalf("unexpected rollback stale response: %#v", staleBody)
	}
	if strings.Contains(string(staleResponse.Body), string(staleRollbackPayload)) {
		t.Fatalf("rollback stale response leaked payload: %s", string(staleResponse.Body))
	}

	for _, logText := range []string{initialLogs.String(), upgradedLogs.String(), rollbackLogs.String()} {
		for _, forbidden := range []string{
			string(preUpgradePayload),
			string(postUpgradePayload),
			string(staleRollbackPayload),
			string(backupSmokeWrappedMaterial()),
			string(backupSmokeWrappedKey()),
			string(postUpgrade.Signature),
			string(stale.Signature),
			"recovery-code",
			"sync-master-key",
		} {
			if forbidden != "" && strings.Contains(logText, forbidden) {
				t.Fatalf("runtime log leaked sensitive fixture %q in %s", forbidden, logText)
			}
		}
	}
}

func startUpgradeRollbackSmokeServer(t *testing.T, cfg config.Config, logs *bytes.Buffer) (string, func()) {
	t.Helper()
	server, closeStore, err := NewHTTPServer(cfg, log.New(logs, "", 0))
	if err != nil {
		t.Fatalf("create upgrade rollback http server: %v", err)
	}
	httpServer := httptest.NewServer(server.Handler)
	closed := false
	closeServer := func() {
		if closed {
			return
		}
		closed = true
		httpServer.Close()
		if err := closeStore(); err != nil {
			t.Fatalf("close upgrade rollback store: %v", err)
		}
	}
	t.Cleanup(closeServer)
	return httpServer.URL, closeServer
}

func uploadUpgradeRollbackObject(t *testing.T, baseURL string, objectID string, deviceID string, version uint64, baseVersion uint64, payload []byte) api.ObjectVersionResponse {
	t.Helper()
	upload := smokeObjectUploadWithType("domain-backup", objectID, storage.ObjectDictionaryUserTerms, deviceID, version, baseVersion, 1, payload)
	response := doJSONSmokeRequest(t, http.MethodPost, baseURL+api.PrefixV1+"/domains/domain-backup/objects/"+objectID+"/versions", upload)
	if response.StatusCode != http.StatusCreated {
		t.Fatalf("unexpected object upload status: %d body=%s", response.StatusCode, string(response.Body))
	}
	var metadata api.ObjectVersionResponse
	decodeSmokeResponse(t, response.Body, &metadata)
	if metadata.CiphertextHash != upload.CiphertextHash ||
		metadata.EncryptedPayloadLen != int64(len(payload)) {
		t.Fatalf("unexpected object upload metadata: %#v", metadata)
	}
	return metadata
}

func verifyUpgradeRollbackPreUpgradeState(t *testing.T, baseURL string, expectedPayload []byte) {
	t.Helper()
	stateResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/state", nil)
	if stateResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected domain state status: %d body=%s", stateResponse.StatusCode, string(stateResponse.Body))
	}
	var state api.DomainStateResponse
	decodeSmokeResponse(t, stateResponse.Body, &state)
	if state.Domain.CurrentKeyEpoch != 1 || state.Domain.ActiveKeyID != "sync-key-backup" {
		t.Fatalf("unexpected domain state after restart: %#v", state)
	}

	deviceResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/devices/device-b", nil)
	if deviceResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected device status: %d body=%s", deviceResponse.StatusCode, string(deviceResponse.Body))
	}
	var device api.DeviceResponse
	decodeSmokeResponse(t, deviceResponse.Body, &device)
	if device.Status != storage.DeviceActive || device.AuthorizedAtMs != 160 {
		t.Fatalf("unexpected device state after restart: %#v", device)
	}

	recoveryResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/recovery-records/latest", nil)
	if recoveryResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected recovery status: %d body=%s", recoveryResponse.StatusCode, string(recoveryResponse.Body))
	}
	var recovery api.RecoveryRecordResponse
	decodeSmokeResponse(t, recoveryResponse.Body, &recovery)
	if recovery.RecoveryRecordID != "recovery-backup" ||
		recovery.CiphertextHash != storage.CiphertextHash(backupSmokeWrappedMaterial()) ||
		!bytes.Equal(recovery.WrappedMaterial, backupSmokeWrappedMaterial()) {
		t.Fatalf("unexpected recovery state after restart: %#v", recovery)
	}
	if strings.Contains(string(recoveryResponse.Body), "blob_ref") {
		t.Fatalf("recovery response leaked internal blob ref: %s", string(recoveryResponse.Body))
	}

	objectID := smokeObjectID(storage.ObjectDictionaryUserTerms)
	verifyUpgradeRollbackObjectPayload(t, baseURL, objectID, 1, expectedPayload)
}

func verifyUpgradeRollbackObjectPayload(t *testing.T, baseURL string, objectID string, version uint64, expectedPayload []byte) {
	t.Helper()
	metadataResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/objects/"+objectID+"/versions/"+smokeUint64String(version), nil)
	if metadataResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected object metadata status: %d body=%s", metadataResponse.StatusCode, string(metadataResponse.Body))
	}
	var metadata api.ObjectVersionResponse
	decodeSmokeResponse(t, metadataResponse.Body, &metadata)
	if metadata.ObjectType != storage.ObjectDictionaryUserTerms ||
		metadata.Version != version ||
		metadata.CiphertextHash != storage.ObjectCiphertextHash(metadataToStorageVersion(metadata), expectedPayload) {
		t.Fatalf("unexpected object metadata after restart: %#v", metadata)
	}
	if strings.Contains(string(metadataResponse.Body), string(expectedPayload)) {
		t.Fatalf("metadata response leaked payload: %s", string(metadataResponse.Body))
	}

	payloadResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/objects/"+objectID+"/versions/"+smokeUint64String(version)+"/payload", nil)
	if payloadResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected object payload status: %d body=%s", payloadResponse.StatusCode, string(payloadResponse.Body))
	}
	if !bytes.Equal(payloadResponse.Body, expectedPayload) {
		t.Fatalf("payload mismatch after restart: got %x want %x", payloadResponse.Body, expectedPayload)
	}
}

func verifyUpgradeRollbackPostUpgradeVersionAbsent(t *testing.T, baseURL string, objectID string) {
	t.Helper()
	versionTwoResponse := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/objects/"+objectID+"/versions/2", nil)
	if versionTwoResponse.StatusCode != http.StatusNotFound {
		t.Fatalf("unexpected rollback version 2 status: %d body=%s", versionTwoResponse.StatusCode, string(versionTwoResponse.Body))
	}
	var body api.ErrorResponse
	decodeSmokeResponse(t, versionTwoResponse.Body, &body)
	if body.ErrorCode != string(storage.ErrNotFound) {
		t.Fatalf("unexpected rollback version 2 response: %#v", body)
	}
}

func upgradeRollbackObjectCiphertextHash(t *testing.T, baseURL string, objectID string, version uint64) string {
	t.Helper()
	response := doSmokeRequest(t, http.MethodGet, baseURL+api.PrefixV1+"/domains/domain-backup/objects/"+objectID+"/versions/"+smokeUint64String(version), nil)
	if response.StatusCode != http.StatusOK {
		t.Fatalf("unexpected object metadata status: %d body=%s", response.StatusCode, string(response.Body))
	}
	var metadata api.ObjectVersionResponse
	decodeSmokeResponse(t, response.Body, &metadata)
	return metadata.CiphertextHash
}
