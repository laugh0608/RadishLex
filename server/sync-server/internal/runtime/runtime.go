package runtime

import (
	"database/sql"
	"fmt"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"time"

	_ "modernc.org/sqlite"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/api"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/config"
	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
	"github.com/laugh0608/RadishLex/server/sync-server/migrations"
)

const (
	readHeaderTimeout = 5 * time.Second
	readTimeout       = 15 * time.Second
	writeTimeout      = 30 * time.Second
	idleTimeout       = 60 * time.Second
)

type CloseFunc func() error

func NewHTTPServer(cfg config.Config, logger *log.Logger) (*http.Server, CloseFunc, error) {
	if err := cfg.Validate(); err != nil {
		return nil, nil, err
	}
	store, closeStore, err := OpenStore(cfg)
	if err != nil {
		return nil, nil, err
	}

	handler := api.NewHandler(store, api.HandlerConfig{
		RecoveryReadLimit:  cfg.RecoveryReadPerHour,
		RecoveryReadWindow: cfg.RecoveryReadWindow,
		MaxObjectBytes:     cfg.MaxObjectBytes,
		AccessToken:        cfg.AccessToken,
		AuditSink:          NewAuditLogger(logger),
	})
	server := &http.Server{
		Addr:              cfg.ListenAddress,
		Handler:           handler,
		ReadHeaderTimeout: readHeaderTimeout,
		ReadTimeout:       readTimeout,
		WriteTimeout:      writeTimeout,
		IdleTimeout:       idleTimeout,
	}
	return server, closeStore, nil
}

func OpenStore(cfg config.Config) (storage.Store, CloseFunc, error) {
	if err := cfg.Validate(); err != nil {
		return nil, nil, err
	}
	if err := os.MkdirAll(filepath.Dir(cfg.MetadataPath), 0o700); err != nil {
		return nil, nil, fmt.Errorf("create metadata directory: %w", err)
	}
	if err := os.MkdirAll(cfg.BlobDir, 0o700); err != nil {
		return nil, nil, fmt.Errorf("create blob directory: %w", err)
	}

	db, err := sql.Open("sqlite", cfg.MetadataPath)
	if err != nil {
		return nil, nil, fmt.Errorf("open sqlite metadata store: %w", err)
	}
	db.SetMaxOpenConns(1)

	if _, err := db.Exec(migrations.InitialSchema()); err != nil {
		_ = db.Close()
		return nil, nil, fmt.Errorf("apply sqlite metadata migration: %w", err)
	}
	blobStore, err := storage.NewLocalObjectBlobStore(cfg.BlobDir)
	if err != nil {
		_ = db.Close()
		return nil, nil, err
	}
	store, err := storage.NewSQLiteStore(db, blobStore)
	if err != nil {
		_ = db.Close()
		return nil, nil, err
	}
	return store, db.Close, nil
}

type AuditLogger struct {
	logger *log.Logger
}

func NewAuditLogger(logger *log.Logger) *AuditLogger {
	if logger == nil {
		return nil
	}
	return &AuditLogger{logger: logger}
}

func (l *AuditLogger) RecordAuditEvent(event api.AuditEvent) {
	if l == nil || l.logger == nil {
		return
	}
	l.logger.Printf(
		"request_id=%q route=%q domain_id=%q device_id=%q object_id=%q object_type=%q version=%d result_code=%q status=%d bytes=%d server_time_ms=%d latency_ms=%d",
		event.RequestID,
		event.RouteName,
		event.DomainID,
		event.DeviceID,
		event.ObjectID,
		event.ObjectType,
		event.Version,
		event.ResultCode,
		event.StatusCode,
		event.Bytes,
		event.ServerTimeMs,
		event.LatencyMs,
	)
}
