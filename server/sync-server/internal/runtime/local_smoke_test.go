package runtime

import (
	"bytes"
	"crypto/ed25519"
	"encoding/binary"
	"encoding/json"
	"log"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/api"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

func TestLocalServerSmokeUploadsReadsAndConflicts(t *testing.T) {
	cfg := runtimeConfigForTest(t)
	var logs bytes.Buffer
	server, closeStore, err := NewHTTPServer(cfg, log.New(&logs, "", 0))
	if err != nil {
		t.Fatalf("create http server: %v", err)
	}
	t.Cleanup(func() {
		_ = closeStore()
	})
	httpServer := httptest.NewServer(server.Handler)
	t.Cleanup(httpServer.Close)

	createDomain := api.CreateDomainRequest{
		DomainID:        "domain-smoke",
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "sync-key-smoke",
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
	createResponse := doJSONSmokeRequest(t, http.MethodPost, httpServer.URL+api.PrefixV1+"/domains", createDomain)
	if createResponse.StatusCode != http.StatusCreated {
		t.Fatalf("unexpected create domain status: %d body=%s", createResponse.StatusCode, string(createResponse.Body))
	}

	firstPayload := []byte{0x91, 0x92, 0x93, 0x94}
	first := smokeObjectUpload("domain-smoke", "object-smoke", "device-smoke", 1, 0, 1, firstPayload)
	firstResponse := doJSONSmokeRequest(t, http.MethodPost, httpServer.URL+api.PrefixV1+"/domains/domain-smoke/objects/object-smoke/versions", first)
	if firstResponse.StatusCode != http.StatusCreated {
		t.Fatalf("unexpected first upload status: %d body=%s", firstResponse.StatusCode, string(firstResponse.Body))
	}
	var firstBody api.ObjectVersionResponse
	decodeSmokeResponse(t, firstResponse.Body, &firstBody)
	if firstBody.CiphertextHash != storage.CiphertextHash(firstPayload) ||
		firstBody.EncryptedPayloadLen != int64(len(firstPayload)) {
		t.Fatalf("unexpected first metadata: %#v", firstBody)
	}

	metadataResponse := doSmokeRequest(t, http.MethodGet, httpServer.URL+api.PrefixV1+"/domains/domain-smoke/objects/object-smoke/versions/1", nil)
	if metadataResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected metadata status: %d body=%s", metadataResponse.StatusCode, string(metadataResponse.Body))
	}
	var metadata api.ObjectVersionResponse
	decodeSmokeResponse(t, metadataResponse.Body, &metadata)
	if metadata.Version != 1 || metadata.CiphertextHash != first.CiphertextHash {
		t.Fatalf("unexpected metadata response: %#v", metadata)
	}
	if strings.Contains(string(metadataResponse.Body), string(firstPayload)) {
		t.Fatalf("metadata response leaked payload: %s", string(metadataResponse.Body))
	}

	payloadResponse := doSmokeRequest(t, http.MethodGet, httpServer.URL+api.PrefixV1+"/domains/domain-smoke/objects/object-smoke/versions/1/payload", nil)
	if payloadResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected payload status: %d body=%s", payloadResponse.StatusCode, string(payloadResponse.Body))
	}
	if !bytes.Equal(payloadResponse.Body, firstPayload) {
		t.Fatalf("payload mismatch: got %x want %x", payloadResponse.Body, firstPayload)
	}

	secondPayload := []byte{0xa1, 0xa2, 0xa3, 0xa4}
	second := smokeObjectUpload("domain-smoke", "object-smoke", "device-smoke", 2, 1, 1, secondPayload)
	secondResponse := doJSONSmokeRequest(t, http.MethodPost, httpServer.URL+api.PrefixV1+"/domains/domain-smoke/objects/object-smoke/versions", second)
	if secondResponse.StatusCode != http.StatusCreated {
		t.Fatalf("unexpected second upload status: %d body=%s", secondResponse.StatusCode, string(secondResponse.Body))
	}

	stalePayload := []byte{0xb1, 0xb2, 0xb3, 0xb4}
	stale := smokeObjectUpload("domain-smoke", "object-smoke", "device-smoke", 3, 1, 1, stalePayload)
	staleResponse := doJSONSmokeRequest(t, http.MethodPost, httpServer.URL+api.PrefixV1+"/domains/domain-smoke/objects/object-smoke/versions", stale)
	if staleResponse.StatusCode != http.StatusConflict {
		t.Fatalf("unexpected stale status: %d body=%s", staleResponse.StatusCode, string(staleResponse.Body))
	}
	var staleBody api.ErrorResponse
	decodeSmokeResponse(t, staleResponse.Body, &staleBody)
	if staleBody.ErrorCode != string(storage.ErrConflictStaleBaseVersion) ||
		staleBody.LatestVersion != 2 ||
		staleBody.LatestCiphertextHash != second.CiphertextHash {
		t.Fatalf("unexpected stale response: %#v", staleBody)
	}
	if strings.Contains(string(staleResponse.Body), string(stalePayload)) {
		t.Fatalf("stale response leaked payload: %s", string(staleResponse.Body))
	}

	logText := logs.String()
	for _, forbidden := range []string{
		string(firstPayload),
		string(secondPayload),
		string(stalePayload),
		string(first.Signature),
		string(second.Signature),
		string(stale.Signature),
	} {
		if forbidden != "" && strings.Contains(logText, forbidden) {
			t.Fatalf("runtime audit log leaked sensitive fixture %q in %s", forbidden, logText)
		}
	}
	for _, required := range []string{
		`route="domains.create"`,
		`route="objects.versions.create"`,
		`route="objects.versions.get"`,
		`route="objects.versions.payload"`,
		`result_code="conflict_stale_base_version"`,
	} {
		if !strings.Contains(logText, required) {
			t.Fatalf("runtime audit log missing %s in %s", required, logText)
		}
	}
}

type smokeHTTPResponse struct {
	StatusCode int
	Body       []byte
}

func doJSONSmokeRequest(t *testing.T, method string, url string, value any) smokeHTTPResponse {
	t.Helper()
	body, err := json.Marshal(value)
	if err != nil {
		t.Fatalf("marshal smoke request: %v", err)
	}
	return doSmokeRequest(t, method, url, bytes.NewReader(body))
}

func doSmokeRequest(t *testing.T, method string, url string, body *bytes.Reader) smokeHTTPResponse {
	t.Helper()
	var requestBody *bytes.Reader
	if body == nil {
		requestBody = bytes.NewReader(nil)
	} else {
		requestBody = body
	}
	request, err := http.NewRequest(method, url, requestBody)
	if err != nil {
		t.Fatalf("create smoke request: %v", err)
	}
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		t.Fatalf("send smoke request: %v", err)
	}
	defer response.Body.Close()
	var out bytes.Buffer
	if _, err := out.ReadFrom(response.Body); err != nil {
		t.Fatalf("read smoke response: %v", err)
	}
	return smokeHTTPResponse{StatusCode: response.StatusCode, Body: out.Bytes()}
}

func decodeSmokeResponse(t *testing.T, body []byte, value any) {
	t.Helper()
	if err := json.Unmarshal(body, value); err != nil {
		t.Fatalf("decode smoke response: %v body=%s", err, string(body))
	}
}

func smokeObjectUpload(domainID string, objectID string, deviceID string, version uint64, baseVersion uint64, keyEpoch uint64, payload []byte) api.ObjectVersionUploadRequest {
	request := api.ObjectVersionUploadRequest{
		ObjectType:          storage.ObjectDictionaryUserTerms,
		Version:             version,
		BaseVersion:         baseVersion,
		OwnerDeviceID:       deviceID,
		KeyID:               "object-key-smoke",
		KeyEpoch:            keyEpoch,
		Algorithm:           storage.AlgorithmXChaCha20Poly1305HKDFSHA256,
		Nonce:               []byte{byte(version), byte(baseVersion), byte(keyEpoch), 0x55},
		EncryptedPayloadLen: int64(len(payload)),
		CiphertextHash:      storage.CiphertextHash(payload),
		ClientCreatedAtMs:   200 + int64(version),
		ClientUpdatedAtMs:   200 + int64(version),
		Payload:             cloneSmokeBytes(payload),
	}
	signSmokeObjectUpload(&request, domainID, objectID)
	return request
}

func signSmokeObjectUpload(request *api.ObjectVersionUploadRequest, domainID string, objectID string) {
	request.SignatureSchemaVersion = 1
	request.SignatureAlgorithm = "ed25519-v1"
	request.SignatureKeyID = smokeSigningKeyID(request.OwnerDeviceID)
	request.Signature = ed25519.Sign(smokeSigningPrivateKey(request.OwnerDeviceID), smokeCanonicalSignatureBytes("sync_object_manifest", []smokeSignatureField{
		smokeTextField("signature_schema_version", "1"),
		smokeTextField("signature_algorithm", "ed25519-v1"),
		smokeTextField("signature_key_id", request.SignatureKeyID),
		smokeTextField("signer_device_id", request.OwnerDeviceID),
		smokeTextField("domain_id", domainID),
		smokeTextField("object_id", objectID),
		smokeTextField("object_type", request.ObjectType),
		smokeTextField("version", smokeUint64String(request.Version)),
		smokeTextField("base_version", smokeOptionalBaseVersionString(request.BaseVersion)),
		smokeTextField("key_id", request.KeyID),
		smokeTextField("key_epoch", smokeUint64String(request.KeyEpoch)),
		smokeTextField("envelope_algorithm", request.Algorithm),
		smokeBytesField("nonce", request.Nonce),
		smokeTextField("encrypted_payload_len", smokeInt64String(request.EncryptedPayloadLen)),
		smokeTextField("ciphertext_hash", request.CiphertextHash),
		smokeTextField("created_at_ms", smokeInt64String(request.ClientCreatedAtMs)),
		smokeTextField("updated_at_ms", smokeInt64String(request.ClientUpdatedAtMs)),
	}))
}

type smokeSignatureField struct {
	name  string
	value []byte
}

func smokeCanonicalSignatureBytes(recordType string, fields []smokeSignatureField) []byte {
	var out []byte
	out = appendSmokeSignatureField(out, "domain_separator", []byte("radishlex-signature-v1"))
	out = appendSmokeSignatureField(out, "record_type", []byte(recordType))
	for _, field := range fields {
		out = appendSmokeSignatureField(out, field.name, field.value)
	}
	return out
}

func appendSmokeSignatureField(out []byte, name string, value []byte) []byte {
	out = append(out, []byte(name)...)
	out = append(out, '=')
	var length [8]byte
	binary.BigEndian.PutUint64(length[:], uint64(len(value)))
	out = append(out, length[:]...)
	out = append(out, value...)
	out = append(out, 0)
	return out
}

func smokeTextField(name string, value string) smokeSignatureField {
	return smokeSignatureField{name: name, value: []byte(value)}
}

func smokeBytesField(name string, value []byte) smokeSignatureField {
	return smokeSignatureField{name: name, value: cloneSmokeBytes(value)}
}

func smokeSigningPublicKey(deviceID string) []byte {
	return smokeSigningPrivateKey(deviceID).Public().(ed25519.PublicKey)
}

func smokeSigningPrivateKey(deviceID string) ed25519.PrivateKey {
	return ed25519.NewKeyFromSeed(smokeSigningSeed(deviceID))
}

func smokeSigningSeed(deviceID string) []byte {
	seed := make([]byte, ed25519.SeedSize)
	fill := byte(19)
	if deviceID != "device-smoke" {
		fill = 23
	}
	for index := range seed {
		seed[index] = fill
	}
	return seed
}

func smokeSigningKeyID(deviceID string) string {
	return "signing-key-" + deviceID
}

func smokeOptionalBaseVersionString(value uint64) string {
	if value == 0 {
		return ""
	}
	return smokeUint64String(value)
}

func smokeUint64String(value uint64) string {
	return strconv.FormatUint(value, 10)
}

func smokeInt64String(value int64) string {
	return strconv.FormatInt(value, 10)
}

func cloneSmokeBytes(value []byte) []byte {
	if value == nil {
		return nil
	}
	out := make([]byte, len(value))
	copy(out, value)
	return out
}
