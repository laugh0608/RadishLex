package api

import (
	"encoding/json"
	"errors"
	"net"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

const deviceIDHeader = "X-RadishLex-Device-ID"

type HandlerConfig struct {
	RecoveryReadLimit  int
	RecoveryReadWindow time.Duration
	Now                func() time.Time
}

type RecoveryReadLimiter interface {
	AllowRecoveryRead(domainID string, clientKey string, now time.Time) bool
}

type Handler struct {
	store           storage.Store
	recoveryLimiter RecoveryReadLimiter
	now             func() time.Time
}

func NewHandler(store storage.Store, cfg HandlerConfig) *Handler {
	now := cfg.Now
	if now == nil {
		now = time.Now
	}
	var limiter RecoveryReadLimiter
	if cfg.RecoveryReadLimit > 0 && cfg.RecoveryReadWindow > 0 {
		limiter = NewMemoryRecoveryReadLimiter(cfg.RecoveryReadLimit, cfg.RecoveryReadWindow)
	}
	return &Handler{
		store:           store,
		recoveryLimiter: limiter,
		now:             now,
	}
}

func (h *Handler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if h.store == nil {
		h.writeError(w, publicStorageError(storage.ErrStorageUnavailable, "storage is not configured", true))
		return
	}
	route, ok := recoveryLatestRoute(r.URL.Path)
	if !ok {
		h.writeError(w, publicStorageError(storage.ErrNotFound, "api route not found", false))
		return
	}
	if r.Method != http.MethodGet {
		w.Header().Set("Allow", http.MethodGet)
		h.writeError(w, publicStorageError(storage.ErrInvalidRequest, "method is not allowed", false))
		return
	}
	h.handleLatestRecovery(w, r, route.domainID)
}

func (h *Handler) handleLatestRecovery(w http.ResponseWriter, r *http.Request, domainID string) {
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

func publicStorageError(code storage.ErrorCode, message string, retryable bool) *storage.Error {
	return &storage.Error{Code: code, Message: message, Retryable: retryable}
}

func (h *Handler) writeError(w http.ResponseWriter, err error) {
	writeJSON(w, statusCodeFromError(err), ErrorResponseFrom(err, h.now()))
}

type recoveryRoute struct {
	domainID string
}

func recoveryLatestRoute(path string) (recoveryRoute, bool) {
	prefix := PrefixV1 + "/domains/"
	if !strings.HasPrefix(path, prefix) {
		return recoveryRoute{}, false
	}
	parts := strings.Split(strings.TrimPrefix(path, prefix), "/")
	if len(parts) != 3 || parts[0] == "" || parts[1] != "recovery-records" || parts[2] != "latest" {
		return recoveryRoute{}, false
	}
	return recoveryRoute{domainID: parts[0]}, true
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
