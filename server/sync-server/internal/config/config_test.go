package config

import (
	"path/filepath"
	"testing"
	"time"
)

func TestDefaultConfigMatchesDocumentedSelfHostedValues(t *testing.T) {
	cfg := Default()
	if cfg.ListenAddress != "127.0.0.1:7319" {
		t.Fatalf("unexpected listen address: %q", cfg.ListenAddress)
	}
	if cfg.MetadataPath != "data/sync-server.sqlite" {
		t.Fatalf("unexpected metadata path: %q", cfg.MetadataPath)
	}
	if cfg.BlobDir != "data/objects" {
		t.Fatalf("unexpected blob dir: %q", cfg.BlobDir)
	}
	if cfg.MaxObjectBytes != 16*1024*1024 {
		t.Fatalf("unexpected max object bytes: %d", cfg.MaxObjectBytes)
	}
	if cfg.RecoveryReadWindow != time.Hour || cfg.RecoveryReadPerHour != 12 {
		t.Fatalf("unexpected recovery limit: window=%s reads=%d", cfg.RecoveryReadWindow, cfg.RecoveryReadPerHour)
	}
}

func TestLoadFromEnvOverridesConfig(t *testing.T) {
	root := t.TempDir()
	t.Setenv("RADISHLEX_SYNC_LISTEN", "127.0.0.1:9000")
	t.Setenv("RADISHLEX_SYNC_METADATA_PATH", filepath.Join(root, "metadata.sqlite"))
	t.Setenv("RADISHLEX_SYNC_BLOB_DIR", filepath.Join(root, "objects"))
	t.Setenv("RADISHLEX_SYNC_MAX_OBJECT_BYTES", "1024")
	t.Setenv("RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR", "6")

	cfg, err := LoadFromEnv()
	if err != nil {
		t.Fatalf("load config: %v", err)
	}
	if cfg.ListenAddress != "127.0.0.1:9000" ||
		cfg.MetadataPath != filepath.Join(root, "metadata.sqlite") ||
		cfg.BlobDir != filepath.Join(root, "objects") ||
		cfg.MaxObjectBytes != 1024 ||
		cfg.RecoveryReadPerHour != 6 {
		t.Fatalf("unexpected config: %#v", cfg)
	}
}

func TestLoadFromEnvRejectsInvalidNumericValues(t *testing.T) {
	t.Setenv("RADISHLEX_SYNC_MAX_OBJECT_BYTES", "not-a-number")

	if _, err := LoadFromEnv(); err == nil {
		t.Fatal("expected invalid max object bytes to fail")
	}
}
