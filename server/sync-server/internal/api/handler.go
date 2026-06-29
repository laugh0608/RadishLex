package api

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

const deviceIDHeader = "X-RadishLex-Device-ID"
const requestIDHeader = "X-Request-ID"

type HandlerConfig struct {
	RecoveryReadLimit  int
	RecoveryReadWindow time.Duration
	Now                func() time.Time
	RequestID          func() string
	AuditSink          AuditSink
}

type RecoveryReadLimiter interface {
	AllowRecoveryRead(domainID string, clientKey string, now time.Time) bool
}

type AuditSink interface {
	RecordAuditEvent(event AuditEvent)
}

type persistentAuditRecorder interface {
	RecordAuditEvent(ctx context.Context, event storage.AuditEvent) error
}

type AuditEvent struct {
	RequestID    string `json:"request_id"`
	RouteName    string `json:"route_name"`
	DomainID     string `json:"domain_id,omitempty"`
	DeviceID     string `json:"device_id,omitempty"`
	ObjectID     string `json:"object_id,omitempty"`
	ObjectType   string `json:"object_type,omitempty"`
	Version      uint64 `json:"version,omitempty"`
	ResultCode   string `json:"result_code"`
	StatusCode   int    `json:"status_code"`
	Bytes        int64  `json:"bytes,omitempty"`
	ServerTimeMs int64  `json:"server_time_ms"`
	LatencyMs    int64  `json:"latency_ms"`
}

type Handler struct {
	store           storage.Store
	recoveryLimiter RecoveryReadLimiter
	now             func() time.Time
	requestID       func() string
	auditSink       AuditSink
}

var generatedRequestIDCounter atomic.Uint64

func NewHandler(store storage.Store, cfg HandlerConfig) *Handler {
	now := cfg.Now
	if now == nil {
		now = time.Now
	}
	requestID := cfg.RequestID
	if requestID == nil {
		requestID = nextRequestID
	}
	var limiter RecoveryReadLimiter
	if cfg.RecoveryReadLimit > 0 && cfg.RecoveryReadWindow > 0 {
		limiter = NewMemoryRecoveryReadLimiter(cfg.RecoveryReadLimit, cfg.RecoveryReadWindow)
	}
	return &Handler{
		store:           store,
		recoveryLimiter: limiter,
		now:             now,
		requestID:       requestID,
		auditSink:       cfg.AuditSink,
	}
}

func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	start := h.now()
	recorder := newStatusRecorder(w)
	audit := AuditEvent{
		RequestID:    requestIDFor(r, h.requestID),
		RouteName:    "unknown",
		DeviceID:     r.Header.Get(deviceIDHeader),
		ResultCode:   "ok",
		ServerTimeMs: start.UnixMilli(),
	}
	if r.ContentLength > 0 {
		audit.Bytes = r.ContentLength
	}
	recorder.Header().Set(requestIDHeader, audit.RequestID)

	defer func() {
		if recovered := recover(); recovered != nil {
			recorder.setResultCode(string(storage.ErrStorageUnavailable))
			if !recorder.wroteHeader {
				h.writeError(recorder, publicStorageError(storage.ErrStorageUnavailable, "request failed", true))
			}
		}
		h.recordAuditEvent(recorder, audit, start)
	}()

	h.serveHTTP(recorder, r, &audit)
}

func (h *Handler) serveHTTP(w http.ResponseWriter, r *http.Request, audit *AuditEvent) {
	if h.store == nil {
		audit.RouteName = "storage.unconfigured"
		h.writeError(w, publicStorageError(storage.ErrStorageUnavailable, "storage is not configured", true))
		return
	}
	if r.URL.Path == PrefixV1+"/domains" {
		audit.RouteName = "domains.create"
		h.handleDomains(w, r, audit)
		return
	}
	route, ok := domainRoute(r.URL.Path)
	if !ok {
		audit.RouteName = "routes.not_found"
		h.writeError(w, publicStorageError(storage.ErrNotFound, "api route not found", false))
		return
	}
	audit.RouteName = route.name()
	audit.DomainID = route.domainID
	if route.deviceID != "" {
		audit.DeviceID = route.deviceID
	}
	if route.objectID != "" {
		audit.ObjectID = route.objectID
		audit.Version = route.version
	}
	switch route.kind {
	case domainStateRoute:
		h.handleDomainState(w, r, route.domainID)
	case deviceRoute:
		h.handleDevice(w, r, route.domainID, route.deviceID)
	case joinRequestsRoute:
		h.handleJoinRequests(w, r, route.domainID, audit)
	case joinAuthorizationRoute:
		h.handleJoinAuthorization(w, r, route.domainID, route.joinRequestID, audit)
	case recoveryLatestRoute:
		h.handleLatestRecovery(w, r, route.domainID)
	case objectVersionsRoute:
		h.handleObjectVersions(w, r, route.domainID, route.objectID, audit)
	case objectVersionRoute:
		h.handleObjectVersion(w, r, route.domainID, route.objectID, route.version)
	case objectPayloadRoute:
		h.handleObjectPayload(w, r, route.domainID, route.objectID, route.version, audit)
	default:
		h.writeError(w, publicStorageError(storage.ErrNotFound, "api route not found", false))
	}
}

func (h *Handler) handleDomains(w http.ResponseWriter, r *http.Request, audit *AuditEvent) {
	if r.Method != http.MethodPost {
		h.writeMethodError(w, http.MethodPost)
		return
	}
	var request CreateDomainRequest
	if err := decodeJSONRequest(r, &request); err != nil {
		h.writeError(w, err)
		return
	}
	audit.DomainID = request.DomainID
	audit.DeviceID = request.FirstDevice.DeviceID
	if err := h.store.CreateDomain(r.Context(), request.Domain(), request.Device()); err != nil {
		h.writeError(w, err)
		return
	}
	writeJSON(w, http.StatusCreated, DomainStateResponse{Domain: DomainResponseFrom(request.Domain())})
}

func (h *Handler) handleDomainState(w http.ResponseWriter, r *http.Request, domainID string) {
	if r.Method != http.MethodGet {
		h.writeMethodError(w, http.MethodGet)
		return
	}
	domain, err := h.store.Domain(r.Context(), domainID)
	if err != nil {
		h.writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, DomainStateResponse{Domain: DomainResponseFrom(domain)})
}

func (h *Handler) handleDevice(w http.ResponseWriter, r *http.Request, domainID string, deviceID string) {
	if r.Method != http.MethodGet {
		h.writeMethodError(w, http.MethodGet)
		return
	}
	device, err := h.store.Device(r.Context(), domainID, deviceID)
	if err != nil {
		h.writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, DeviceResponseFrom(device))
}

func (h *Handler) handleJoinRequests(w http.ResponseWriter, r *http.Request, domainID string, audit *AuditEvent) {
	switch r.Method {
	case http.MethodGet:
		requests, err := h.store.PendingJoinRequests(r.Context(), domainID)
		if err != nil {
			h.writeError(w, err)
			return
		}
		writeJSON(w, http.StatusOK, JoinRequestsResponseFrom(requests))
	case http.MethodPost:
		var request CreateJoinRequestRequest
		if err := decodeJSONRequest(r, &request); err != nil {
			h.writeError(w, err)
			return
		}
		joinRequest := request.JoinRequest(domainID)
		audit.DeviceID = joinRequest.DeviceID
		if err := h.store.SaveJoinRequest(r.Context(), joinRequest); err != nil {
			h.writeError(w, err)
			return
		}
		writeJSON(w, http.StatusCreated, JoinRequestResponseFrom(joinRequest))
	default:
		h.writeMethodError(w, http.MethodGet, http.MethodPost)
	}
}

func (h *Handler) handleJoinAuthorization(w http.ResponseWriter, r *http.Request, domainID string, joinRequestID string, audit *AuditEvent) {
	if r.Method != http.MethodPost {
		h.writeMethodError(w, http.MethodPost)
		return
	}
	var request AuthorizeJoinRequestRequest
	if err := decodeJSONRequest(r, &request); err != nil {
		h.writeError(w, err)
		return
	}
	upload := request.Upload(domainID, joinRequestID)
	audit.DeviceID = upload.Authorization.RecipientDeviceID
	if err := h.store.AuthorizeJoinRequest(r.Context(), upload); err != nil {
		h.writeError(w, err)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func (h *Handler) handleLatestRecovery(w http.ResponseWriter, r *http.Request, domainID string) {
	if r.Method != http.MethodGet {
		h.writeMethodError(w, http.MethodGet)
		return
	}
	now := h.now()
	if h.recoveryLimiter != nil && !h.recoveryLimiter.AllowRecoveryRead(domainID, recoveryClientKey(r), now) {
		h.writeError(w, publicStorageError(storage.ErrRecoveryRateLimited, "recovery record read rate limit exceeded", true))
		return
	}
	record, wrappedMaterial, err := h.store.LatestRecoveryWrappedMaterial(r.Context(), domainID)
	if err != nil {
		h.writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, RecoveryRecordResponseFrom(record, wrappedMaterial))
}

func (h *Handler) handleObjectVersions(w http.ResponseWriter, r *http.Request, domainID string, objectID string, audit *AuditEvent) {
	if r.Method != http.MethodPost {
		h.writeMethodError(w, http.MethodPost)
		return
	}
	var request ObjectVersionUploadRequest
	if err := decodeJSONRequest(r, &request); err != nil {
		h.writeError(w, err)
		return
	}
	upload := request.Upload(domainID, objectID)
	audit.DeviceID = upload.Version.OwnerDeviceID
	audit.ObjectID = objectID
	audit.ObjectType = upload.Version.ObjectType
	audit.Version = upload.Version.Version
	audit.Bytes = upload.Version.EncryptedPayloadLen
	metadata, err := h.store.PutObjectVersion(r.Context(), upload)
	if err != nil {
		h.writeError(w, err)
		return
	}
	writeJSON(w, http.StatusCreated, ObjectVersionResponseFrom(metadata))
}

func (h *Handler) handleObjectVersion(w http.ResponseWriter, r *http.Request, domainID string, objectID string, version uint64) {
	if r.Method != http.MethodGet {
		h.writeMethodError(w, http.MethodGet)
		return
	}
	metadata, err := h.store.ObjectVersion(r.Context(), domainID, objectID, version)
	if err != nil {
		h.writeError(w, err)
		return
	}
	writeJSON(w, http.StatusOK, ObjectVersionResponseFrom(metadata))
}

func (h *Handler) handleObjectPayload(w http.ResponseWriter, r *http.Request, domainID string, objectID string, version uint64, audit *AuditEvent) {
	if r.Method != http.MethodGet {
		h.writeMethodError(w, http.MethodGet)
		return
	}
	payload, err := h.store.ObjectPayload(r.Context(), domainID, objectID, version)
	if err != nil {
		h.writeError(w, err)
		return
	}
	audit.ObjectID = objectID
	audit.Version = version
	audit.Bytes = int64(len(payload))
	w.Header().Set("Content-Type", "application/octet-stream")
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(payload)
}

func publicStorageError(code storage.ErrorCode, message string, retryable bool) *storage.Error {
	return &storage.Error{Code: code, Message: message, Retryable: retryable}
}

func decodeJSONRequest(r *http.Request, value any) error {
	decoder := json.NewDecoder(r.Body)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(value); err != nil {
		return publicStorageError(storage.ErrInvalidRequest, "request body must be valid JSON metadata", false)
	}
	if decoder.Decode(&struct{}{}) != io.EOF {
		return publicStorageError(storage.ErrInvalidRequest, "request body must contain one JSON object", false)
	}
	return nil
}

func (h *Handler) writeMethodError(w http.ResponseWriter, allowedMethods ...string) {
	w.Header().Set("Allow", strings.Join(allowedMethods, ", "))
	h.writeError(w, publicStorageError(storage.ErrInvalidRequest, "method is not allowed", false))
}

func (h *Handler) writeError(w http.ResponseWriter, err error) {
	response := ErrorResponseFrom(err, h.now())
	if recorder, ok := w.(interface{ setResultCode(string) }); ok {
		recorder.setResultCode(response.ErrorCode)
	}
	writeJSON(w, statusCodeFromError(err), response)
}

type routeKind int

const (
	domainStateRoute routeKind = iota + 1
	deviceRoute
	joinRequestsRoute
	joinAuthorizationRoute
	recoveryLatestRoute
	objectVersionsRoute
	objectVersionRoute
	objectPayloadRoute
)

type route struct {
	kind          routeKind
	domainID      string
	deviceID      string
	joinRequestID string
	objectID      string
	version       uint64
}

func (r route) name() string {
	switch r.kind {
	case domainStateRoute:
		return "domains.state"
	case deviceRoute:
		return "devices.get"
	case joinRequestsRoute:
		return "join_requests.collection"
	case joinAuthorizationRoute:
		return "join_requests.authorize"
	case recoveryLatestRoute:
		return "recovery.latest"
	case objectVersionsRoute:
		return "objects.versions.create"
	case objectVersionRoute:
		return "objects.versions.get"
	case objectPayloadRoute:
		return "objects.versions.payload"
	default:
		return "routes.unknown"
	}
}

func domainRoute(path string) (route, bool) {
	prefix := PrefixV1 + "/domains/"
	if !strings.HasPrefix(path, prefix) {
		return route{}, false
	}
	parts := strings.Split(strings.TrimPrefix(path, prefix), "/")
	if len(parts) == 2 && parts[0] != "" && parts[1] == "state" {
		return route{kind: domainStateRoute, domainID: parts[0]}, true
	}
	if len(parts) == 3 && parts[0] != "" && parts[1] == "devices" && parts[2] != "" {
		return route{kind: deviceRoute, domainID: parts[0], deviceID: parts[2]}, true
	}
	if len(parts) == 2 && parts[0] != "" && parts[1] == "join-requests" {
		return route{kind: joinRequestsRoute, domainID: parts[0]}, true
	}
	if len(parts) == 4 && parts[0] != "" && parts[1] == "join-requests" && parts[2] != "" && parts[3] == "authorization" {
		return route{kind: joinAuthorizationRoute, domainID: parts[0], joinRequestID: parts[2]}, true
	}
	if len(parts) == 3 && parts[0] != "" && parts[1] == "recovery-records" && parts[2] == "latest" {
		return route{kind: recoveryLatestRoute, domainID: parts[0]}, true
	}
	if len(parts) == 4 && parts[0] != "" && parts[1] == "objects" && parts[2] != "" && parts[3] == "versions" {
		return route{kind: objectVersionsRoute, domainID: parts[0], objectID: parts[2]}, true
	}
	if len(parts) == 5 && parts[0] != "" && parts[1] == "objects" && parts[2] != "" && parts[3] == "versions" && parts[4] != "" {
		version, ok := parseRouteVersion(parts[4])
		if !ok {
			return route{}, false
		}
		return route{kind: objectVersionRoute, domainID: parts[0], objectID: parts[2], version: version}, true
	}
	if len(parts) == 6 && parts[0] != "" && parts[1] == "objects" && parts[2] != "" && parts[3] == "versions" && parts[4] != "" && parts[5] == "payload" {
		version, ok := parseRouteVersion(parts[4])
		if !ok {
			return route{}, false
		}
		return route{kind: objectPayloadRoute, domainID: parts[0], objectID: parts[2], version: version}, true
	}
	return route{}, false
}

func parseRouteVersion(value string) (uint64, bool) {
	version, err := strconv.ParseUint(value, 10, 64)
	return version, err == nil && version > 0
}

func recoveryClientKey(r *http.Request) string {
	host := r.RemoteAddr
	if parsedHost, _, err := net.SplitHostPort(r.RemoteAddr); err == nil {
		host = parsedHost
	}
	deviceID := r.Header.Get(deviceIDHeader)
	if deviceID == "" {
		deviceID = "unknown-device"
	}
	return host + "\x00" + deviceID
}

func statusCodeFromError(err error) int {
	var storageErr *storage.Error
	if !errors.As(err, &storageErr) {
		return http.StatusServiceUnavailable
	}
	switch storageErr.Code {
	case storage.ErrInvalidRequest, storage.ErrInvalidSignature, storage.ErrInvalidCiphertextMetadata:
		return http.StatusBadRequest
	case storage.ErrUnauthenticated:
		return http.StatusUnauthorized
	case storage.ErrForbiddenDevice:
		return http.StatusForbidden
	case storage.ErrNotFound:
		return http.StatusNotFound
	case storage.ErrConflictStaleBaseVersion, storage.ErrConflictObjectVersion:
		return http.StatusConflict
	case storage.ErrPayloadTooLarge:
		return http.StatusRequestEntityTooLarge
	case storage.ErrRecoveryRateLimited:
		return http.StatusTooManyRequests
	case storage.ErrStorageUnavailable:
		return http.StatusServiceUnavailable
	default:
		return http.StatusServiceUnavailable
	}
}

func writeJSON(w http.ResponseWriter, statusCode int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(statusCode)
	_ = json.NewEncoder(w).Encode(value)
}

type statusRecorder struct {
	http.ResponseWriter
	statusCode  int
	resultCode  string
	wroteHeader bool
}

func newStatusRecorder(w http.ResponseWriter) *statusRecorder {
	return &statusRecorder{
		ResponseWriter: w,
		statusCode:     http.StatusOK,
		resultCode:     "ok",
	}
}

func (r *statusRecorder) WriteHeader(statusCode int) {
	if r.wroteHeader {
		return
	}
	r.statusCode = statusCode
	r.wroteHeader = true
	r.ResponseWriter.WriteHeader(statusCode)
}

func (r *statusRecorder) Write(value []byte) (int, error) {
	if !r.wroteHeader {
		r.WriteHeader(r.statusCode)
	}
	return r.ResponseWriter.Write(value)
}

func (r *statusRecorder) setResultCode(resultCode string) {
	if resultCode != "" {
		r.resultCode = resultCode
	}
}

func (h *Handler) recordAuditEvent(recorder *statusRecorder, event AuditEvent, start time.Time) {
	event.ResultCode = recorder.resultCode
	event.StatusCode = recorder.statusCode
	event.LatencyMs = h.now().Sub(start).Milliseconds()
	if persistent, ok := h.store.(persistentAuditRecorder); ok {
		_ = persistent.RecordAuditEvent(context.Background(), storage.AuditEvent{
			DomainID:     event.DomainID,
			EventType:    event.RouteName,
			DeviceID:     event.DeviceID,
			ObjectID:     event.ObjectID,
			Version:      event.Version,
			ResultCode:   event.ResultCode,
			Bytes:        event.Bytes,
			ServerTimeMs: event.ServerTimeMs,
		})
	}
	if h.auditSink != nil {
		h.auditSink.RecordAuditEvent(event)
	}
}

func requestIDFor(r *http.Request, generate func() string) string {
	if value := strings.TrimSpace(r.Header.Get(requestIDHeader)); value != "" {
		return value
	}
	return generate()
}

func nextRequestID() string {
	next := generatedRequestIDCounter.Add(1)
	return "req-" + strconv.FormatUint(uint64(time.Now().UnixNano()), 36) + "-" + strconv.FormatUint(next, 36)
}

type MemoryRecoveryReadLimiter struct {
	limit  int
	window time.Duration
	mu     sync.Mutex
	reads  map[string][]time.Time
}

func NewMemoryRecoveryReadLimiter(limit int, window time.Duration) *MemoryRecoveryReadLimiter {
	return &MemoryRecoveryReadLimiter{
		limit:  limit,
		window: window,
		reads:  make(map[string][]time.Time),
	}
}

func (l *MemoryRecoveryReadLimiter) AllowRecoveryRead(domainID string, clientKey string, now time.Time) bool {
	if l == nil || l.limit <= 0 || l.window <= 0 {
		return true
	}
	key := domainID + "\x00" + clientKey
	cutoff := now.Add(-l.window)

	l.mu.Lock()
	defer l.mu.Unlock()

	kept := l.reads[key][:0]
	for _, readAt := range l.reads[key] {
		if readAt.After(cutoff) {
			kept = append(kept, readAt)
		}
	}
	if len(kept) >= l.limit {
		l.reads[key] = kept
		return false
	}
	l.reads[key] = append(kept, now)
	return true
}
