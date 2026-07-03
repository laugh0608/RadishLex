package runtime

import (
	"bytes"
	"database/sql"
	"encoding/json"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	_ "modernc.org/sqlite"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/api"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/config"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

func TestOpenStoreAppliesMigrationIdempotently(t *testing.T) {
	cfg := runtimeConfigForTest(t)

	_, closeFirst, err := OpenStore(cfg)
	if err != nil {
		t.Fatalf("open store first time: %v", err)
	}
	if err := closeFirst(); err != nil {
		t.Fatalf("close first store: %v", err)
	}
	_, closeSecond, err := OpenStore(cfg)
	if err != nil {
		t.Fatalf("open store second time: %v", err)
	}
	t.Cleanup(func() {
		_ = closeSecond()
	})

	db, err := sql.Open("sqlite", cfg.MetadataPath)
	if err != nil {
		t.Fatalf("open sqlite for inspection: %v", err)
	}
	defer db.Close()
	var tableName string
	if err := db.QueryRow("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'sync_domains'").Scan(&tableName); err != nil {
		t.Fatalf("migration did not create sync_domains: %v", err)
	}
}

func TestNewHTTPServerConfiguresHandlerTimeoutsAndRedactedAuditLog(t *testing.T) {
	cfg := runtimeConfigForTest(t)
	cfg.MaxObjectBytes = 4
	var logs bytes.Buffer
	server, closeStore, err := NewHTTPServer(cfg, log.New(&logs, "", 0))
	if err != nil {
		t.Fatalf("create http server: %v", err)
	}
	t.Cleanup(func() {
		_ = closeStore()
	})

	if server.Addr != cfg.ListenAddress ||
		server.ReadHeaderTimeout != readHeaderTimeout ||
		server.ReadTimeout != readTimeout ||
		server.WriteTimeout != writeTimeout ||
		server.IdleTimeout != idleTimeout {
		t.Fatalf("unexpected server config: %#v", server)
	}

	createDomain := api.CreateDomainRequest{
		DomainID:        "domain-a",
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "sync-key-a",
		FirstDevice: api.DeviceMetadata{
			DeviceID:                "device-a",
			SigningPublicKeyID:      "signing-key-a",
			SigningPublicKey:        []byte("sensitive-signing-public-key"),
			KeyAgreementPublicKeyID: "agreement-key-a",
			KeyAgreementPublicKey:   []byte("sensitive-agreement-public-key"),
			Status:                  string(storage.DeviceActive),
		},
		CreatedAtMs: 100,
		UpdatedAtMs: 100,
	}
	createResponse := performJSONRequest(t, server.Handler, http.MethodPost, api.PrefixV1+"/domains", createDomain)
	if createResponse.Code != http.StatusCreated {
		t.Fatalf("unexpected create domain status: %d body=%s", createResponse.Code, createResponse.Body.String())
	}

	oversizedPayload := []byte("encrypted-payload-over-limit")
	upload := api.ObjectVersionUploadRequest{
		ObjectType:             storage.ObjectDictionaryUserTerms,
		Version:                1,
		BaseVersion:            0,
		OwnerDeviceID:          "device-a",
		KeyID:                  "object-key-a",
		KeyEpoch:               1,
		Algorithm:              storage.AlgorithmXChaCha20Poly1305HKDFSHA256,
		Nonce:                  []byte{1, 2, 3},
		EncryptedPayloadLen:    int64(len(oversizedPayload)),
		CiphertextHash:         storage.CiphertextHash(oversizedPayload),
		SignatureSchemaVersion: 1,
		SignatureAlgorithm:     "ed25519-v1",
		SignatureKeyID:         "signing-key-a",
		Signature:              []byte("not-a-real-signature"),
		ClientCreatedAtMs:      101,
		ClientUpdatedAtMs:      101,
		Payload:                oversizedPayload,
	}
	response := performJSONRequest(t, server.Handler, http.MethodPost, api.PrefixV1+"/domains/domain-a/objects/object-a/versions", upload)
	if response.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("unexpected oversized status: %d body=%s", response.Code, response.Body.String())
	}
	var body api.ErrorResponse
	decodeResponse(t, response, &body)
	if body.ErrorCode != string(storage.ErrPayloadTooLarge) {
		t.Fatalf("unexpected oversized response: %#v", body)
	}

	logText := logs.String()
	for _, forbidden := range []string{
		"sensitive-signing-public-key",
		"sensitive-agreement-public-key",
		string(oversizedPayload),
		"ZW5jcnlwdGVkLXBheWxvYWQtb3Zlci1saW1pdA",
		"not-a-real-signature",
	} {
		if strings.Contains(logText, forbidden) {
			t.Fatalf("audit log leaked request body content %q in %s", forbidden, logText)
		}
	}
	if !strings.Contains(logText, `route="objects.versions.create"`) ||
		!strings.Contains(logText, `result_code="payload_too_large"`) {
		t.Fatalf("audit log missing non-sensitive object event fields: %s", logText)
	}
	if _, err := os.Stat(cfg.MetadataPath); err != nil {
		t.Fatalf("metadata database was not created: %v", err)
	}
	if _, err := os.Stat(cfg.BlobDir); err != nil {
		t.Fatalf("blob directory was not created: %v", err)
	}
}

func TestNewHTTPServerEnforcesConfiguredAccessToken(t *testing.T) {
	cfg := runtimeConfigForTest(t)
	cfg.AccessToken = testAccessToken
	var logs bytes.Buffer
	server, closeStore, err := NewHTTPServer(cfg, log.New(&logs, "", 0))
	if err != nil {
		t.Fatalf("create http server: %v", err)
	}
	t.Cleanup(func() {
		_ = closeStore()
	})

	missing := httptest.NewRequest(http.MethodGet, api.PrefixV1+"/domains/domain-a/state", nil)
	missingResponse := httptest.NewRecorder()
	server.Handler.ServeHTTP(missingResponse, missing)
	if missingResponse.Code != http.StatusUnauthorized {
		t.Fatalf("unexpected missing token status: %d body=%s", missingResponse.Code, missingResponse.Body.String())
	}
	var missingBody api.ErrorResponse
	decodeResponse(t, missingResponse, &missingBody)
	if missingBody.ErrorCode != string(storage.ErrUnauthenticated) {
		t.Fatalf("unexpected missing token response: %#v", missingBody)
	}

	createDomain := api.CreateDomainRequest{
		DomainID:        "domain-a",
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "sync-key-a",
		FirstDevice: api.DeviceMetadata{
			DeviceID:                "device-a",
			SigningPublicKeyID:      "signing-key-a",
			SigningPublicKey:        []byte("signing-public-key"),
			KeyAgreementPublicKeyID: "agreement-key-a",
			KeyAgreementPublicKey:   []byte("agreement-public-key"),
			Status:                  string(storage.DeviceActive),
		},
		CreatedAtMs: 100,
		UpdatedAtMs: 100,
	}
	createResponse := performAuthorizedJSONRequest(t, server.Handler, http.MethodPost, api.PrefixV1+"/domains", testAccessToken, createDomain)
	if createResponse.Code != http.StatusCreated {
		t.Fatalf("unexpected authorized create domain status: %d body=%s", createResponse.Code, createResponse.Body.String())
	}
	if strings.Contains(logs.String(), testAccessToken) {
		t.Fatalf("audit log leaked access token: %s", logs.String())
	}
}

func runtimeConfigForTest(t *testing.T) config.Config {
	t.Helper()
	root := t.TempDir()
	cfg := config.Default()
	cfg.ListenAddress = "127.0.0.1:0"
	cfg.MetadataPath = filepath.Join(root, "metadata.sqlite")
	cfg.BlobDir = filepath.Join(root, "objects")
	return cfg
}

const testAccessToken = "test-access-token-12345678901234567890"

func performJSONRequest(t *testing.T, handler http.Handler, method string, path string, value any) *httptest.ResponseRecorder {
	t.Helper()
	return performAuthorizedJSONRequest(t, handler, method, path, "", value)
}

func performAuthorizedJSONRequest(t *testing.T, handler http.Handler, method string, path string, accessToken string, value any) *httptest.ResponseRecorder {
	t.Helper()
	body, err := json.Marshal(value)
	if err != nil {
		t.Fatalf("marshal request: %v", err)
	}
	request := httptest.NewRequest(method, path, bytes.NewReader(body))
	if accessToken != "" {
		request.Header.Set("Authorization", "Bearer "+accessToken)
	}
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
