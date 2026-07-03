package runtime

import (
	"bytes"
	"crypto/tls"
	"encoding/json"
	"io"
	"log"
	"net/http"
	"net/http/httptest"
	"net/http/httputil"
	"net/url"
	"strings"
	"sync"
	"testing"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/api"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

func TestExternalTLSProxySmokePreservesAuthAndEncryptedObjectFlow(t *testing.T) {
	cfg := runtimeConfigForTest(t)
	cfg.AccessToken = testAccessToken
	cfg.MaxObjectBytes = 8
	var logs bytes.Buffer
	server, closeStore, err := NewHTTPServer(cfg, log.New(&logs, "", 0))
	if err != nil {
		t.Fatalf("create http server: %v", err)
	}
	t.Cleanup(func() {
		_ = closeStore()
	})

	observed := &externalTLSProxyObservation{}
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		observed.recordUpstream(r)
		server.Handler.ServeHTTP(w, r)
	}))
	t.Cleanup(upstream.Close)

	tlsProxy := newExternalTLSProxyForSmoke(t, upstream.URL, observed, 4096)
	t.Cleanup(tlsProxy.Close)
	client := tlsProxy.Client()

	missingToken := doTLSProxyRequest(t, client, http.MethodGet, tlsProxy.URL+api.PrefixV1+"/domains/tls-smoke/state", "", nil)
	if missingToken.StatusCode != http.StatusUnauthorized {
		t.Fatalf("unexpected missing token status: %d body=%s", missingToken.StatusCode, string(missingToken.Body))
	}
	if missingToken.TLSVersion < tls.VersionTLS12 {
		t.Fatalf("TLS proxy used unsupported TLS version: %x", missingToken.TLSVersion)
	}
	var missingBody api.ErrorResponse
	decodeSmokeResponse(t, missingToken.Body, &missingBody)
	if missingBody.ErrorCode != string(storage.ErrUnauthenticated) {
		t.Fatalf("unexpected missing token response: %#v", missingBody)
	}

	missingDomain := doTLSProxyRequest(t, client, http.MethodGet, tlsProxy.URL+api.PrefixV1+"/domains/tls-smoke/state", testAccessToken, nil)
	if missingDomain.StatusCode != http.StatusNotFound {
		t.Fatalf("unexpected missing domain status: %d body=%s", missingDomain.StatusCode, string(missingDomain.Body))
	}

	createDomain := api.CreateDomainRequest{
		DomainID:        "tls-smoke",
		CurrentKeyEpoch: 1,
		ActiveKeyID:     "sync-key-tls",
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
	createResponse := doTLSProxyJSONRequest(t, client, http.MethodPost, tlsProxy.URL+api.PrefixV1+"/domains", testAccessToken, createDomain)
	if createResponse.StatusCode != http.StatusCreated {
		t.Fatalf("unexpected create domain status: %d body=%s", createResponse.StatusCode, string(createResponse.Body))
	}

	payload := []byte{0x91, 0x92, 0x93, 0x94}
	upload := smokeObjectUploadWithType("tls-smoke", "object-tls", storage.ObjectDictionaryUserTerms, "device-smoke", 1, 0, 1, payload)
	uploadResponse := doTLSProxyJSONRequest(t, client, http.MethodPost, tlsProxy.URL+api.PrefixV1+"/domains/tls-smoke/objects/object-tls/versions", testAccessToken, upload)
	if uploadResponse.StatusCode != http.StatusCreated {
		t.Fatalf("unexpected upload status: %d body=%s", uploadResponse.StatusCode, string(uploadResponse.Body))
	}

	payloadResponse := doTLSProxyRequest(t, client, http.MethodGet, tlsProxy.URL+api.PrefixV1+"/domains/tls-smoke/objects/object-tls/versions/1/payload", testAccessToken, nil)
	if payloadResponse.StatusCode != http.StatusOK {
		t.Fatalf("unexpected payload status: %d body=%s", payloadResponse.StatusCode, string(payloadResponse.Body))
	}
	if !bytes.Equal(payloadResponse.Body, payload) {
		t.Fatalf("payload mismatch through TLS proxy: got %x want %x", payloadResponse.Body, payload)
	}

	oversizedPayload := []byte{0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9}
	oversized := smokeObjectUploadWithType("tls-smoke", "object-oversized", storage.ObjectDictionaryUserTerms, "device-smoke", 1, 0, 1, oversizedPayload)
	oversizedResponse := doTLSProxyJSONRequest(t, client, http.MethodPost, tlsProxy.URL+api.PrefixV1+"/domains/tls-smoke/objects/object-oversized/versions", testAccessToken, oversized)
	if oversizedResponse.StatusCode != http.StatusRequestEntityTooLarge {
		t.Fatalf("unexpected oversized status: %d body=%s", oversizedResponse.StatusCode, string(oversizedResponse.Body))
	}
	var oversizedBody api.ErrorResponse
	decodeSmokeResponse(t, oversizedResponse.Body, &oversizedBody)
	if oversizedBody.ErrorCode != string(storage.ErrPayloadTooLarge) {
		t.Fatalf("unexpected oversized response: %#v", oversizedBody)
	}

	snapshot := observed.snapshot()
	if !snapshot.proxySawTLS {
		t.Fatal("external proxy did not receive TLS traffic")
	}
	if snapshot.upstreamSawTLS {
		t.Fatal("Go sync server upstream should remain plain HTTP behind TLS proxy")
	}
	if snapshot.lastAuthorization != "Bearer "+testAccessToken {
		t.Fatalf("Authorization header was not preserved to upstream: %q", snapshot.lastAuthorization)
	}
	if snapshot.lastForwardedProto != "https" {
		t.Fatalf("X-Forwarded-Proto was not set to https: %q", snapshot.lastForwardedProto)
	}

	logText := logs.String()
	for _, forbidden := range []string{
		testAccessToken,
		string(payload),
		string(oversizedPayload),
		string(upload.Signature),
		string(oversized.Signature),
	} {
		if forbidden != "" && strings.Contains(logText, forbidden) {
			t.Fatalf("runtime log leaked sensitive fixture %q in %s", forbidden, logText)
		}
	}
}

type externalTLSProxyObservation struct {
	mu                 sync.Mutex
	proxySawTLS        bool
	upstreamSawTLS     bool
	lastAuthorization  string
	lastForwardedProto string
}

type externalTLSProxySnapshot struct {
	proxySawTLS        bool
	upstreamSawTLS     bool
	lastAuthorization  string
	lastForwardedProto string
}

func (o *externalTLSProxyObservation) recordProxyRequest(r *http.Request) {
	o.mu.Lock()
	defer o.mu.Unlock()
	if r.TLS != nil {
		o.proxySawTLS = true
	}
}

func (o *externalTLSProxyObservation) recordUpstream(r *http.Request) {
	o.mu.Lock()
	defer o.mu.Unlock()
	if r.TLS != nil {
		o.upstreamSawTLS = true
	}
	if value := r.Header.Get("Authorization"); value != "" {
		o.lastAuthorization = value
	}
	if value := r.Header.Get("X-Forwarded-Proto"); value != "" {
		o.lastForwardedProto = value
	}
}

func (o *externalTLSProxyObservation) snapshot() externalTLSProxySnapshot {
	o.mu.Lock()
	defer o.mu.Unlock()
	return externalTLSProxySnapshot{
		proxySawTLS:        o.proxySawTLS,
		upstreamSawTLS:     o.upstreamSawTLS,
		lastAuthorization:  o.lastAuthorization,
		lastForwardedProto: o.lastForwardedProto,
	}
}

func newExternalTLSProxyForSmoke(t *testing.T, upstreamURL string, observed *externalTLSProxyObservation, maxBodyBytes int64) *httptest.Server {
	t.Helper()
	target, err := url.Parse(upstreamURL)
	if err != nil {
		t.Fatalf("parse upstream URL: %v", err)
	}
	proxy := httputil.NewSingleHostReverseProxy(target)
	originalDirector := proxy.Director
	proxy.Director = func(r *http.Request) {
		originalDirector(r)
		r.Header.Set("X-Forwarded-Proto", "https")
		r.Header.Set("X-Forwarded-Host", r.Host)
	}
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		observed.recordProxyRequest(r)
		if maxBodyBytes > 0 && r.ContentLength > maxBodyBytes {
			http.Error(w, "request body too large", http.StatusRequestEntityTooLarge)
			return
		}
		proxy.ServeHTTP(w, r)
	})
	server := httptest.NewUnstartedServer(handler)
	server.TLS = &tls.Config{MinVersion: tls.VersionTLS12}
	server.StartTLS()
	return server
}

type tlsProxyHTTPResponse struct {
	StatusCode int
	Body       []byte
	TLSVersion uint16
}

func doTLSProxyJSONRequest(t *testing.T, client *http.Client, method string, url string, accessToken string, value any) tlsProxyHTTPResponse {
	t.Helper()
	body, err := json.Marshal(value)
	if err != nil {
		t.Fatalf("marshal TLS proxy request: %v", err)
	}
	return doTLSProxyRequest(t, client, method, url, accessToken, bytes.NewReader(body))
}

func doTLSProxyRequest(t *testing.T, client *http.Client, method string, url string, accessToken string, body io.Reader) tlsProxyHTTPResponse {
	t.Helper()
	if body == nil {
		body = bytes.NewReader(nil)
	}
	request, err := http.NewRequest(method, url, body)
	if err != nil {
		t.Fatalf("create TLS proxy request: %v", err)
	}
	if accessToken != "" {
		request.Header.Set("Authorization", "Bearer "+accessToken)
	}
	response, err := client.Do(request)
	if err != nil {
		t.Fatalf("send TLS proxy request: %v", err)
	}
	defer response.Body.Close()
	var out bytes.Buffer
	if _, err := out.ReadFrom(response.Body); err != nil {
		t.Fatalf("read TLS proxy response: %v", err)
	}
	var tlsVersion uint16
	if response.TLS != nil {
		tlsVersion = response.TLS.Version
	}
	return tlsProxyHTTPResponse{StatusCode: response.StatusCode, Body: out.Bytes(), TLSVersion: tlsVersion}
}
