package config

import (
	"fmt"
	"os"
	"strconv"
	"time"
)

const defaultMaxObjectBytes int64 = 16 * 1024 * 1024

type Config struct {
	ListenAddress       string
	MetadataPath        string
	BlobDir             string
	MaxObjectBytes      int64
	RecoveryReadWindow  time.Duration
	RecoveryReadPerHour int
}

func Default() Config {
	return Config{
		ListenAddress:       "127.0.0.1:7319",
		MetadataPath:        "data/sync-server.sqlite",
		BlobDir:             "data/objects",
		MaxObjectBytes:      defaultMaxObjectBytes,
		RecoveryReadWindow:  time.Hour,
		RecoveryReadPerHour: 12,
	}
}

func LoadFromEnv() (Config, error) {
	cfg := Default()
	cfg.ListenAddress = envOrDefault("RADISHLEX_SYNC_LISTEN", cfg.ListenAddress)
	cfg.MetadataPath = envOrDefault("RADISHLEX_SYNC_METADATA_PATH", cfg.MetadataPath)
	cfg.BlobDir = envOrDefault("RADISHLEX_SYNC_BLOB_DIR", cfg.BlobDir)

	if value := os.Getenv("RADISHLEX_SYNC_MAX_OBJECT_BYTES"); value != "" {
		parsed, err := strconv.ParseInt(value, 10, 64)
		if err != nil {
			return Config{}, fmt.Errorf("parse RADISHLEX_SYNC_MAX_OBJECT_BYTES: %w", err)
		}
		cfg.MaxObjectBytes = parsed
	}
	if value := os.Getenv("RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR"); value != "" {
		parsed, err := strconv.Atoi(value)
		if err != nil {
			return Config{}, fmt.Errorf("parse RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR: %w", err)
		}
		cfg.RecoveryReadPerHour = parsed
	}
	if err := cfg.Validate(); err != nil {
		return Config{}, err
	}
	return cfg, nil
}

func (c Config) Validate() error {
	if c.ListenAddress == "" {
		return fmt.Errorf("listen address is required")
	}
	if c.MetadataPath == "" {
		return fmt.Errorf("metadata path is required")
	}
	if c.BlobDir == "" {
		return fmt.Errorf("blob dir is required")
	}
	if c.MaxObjectBytes <= 0 {
		return fmt.Errorf("max object bytes must be positive")
	}
	if c.RecoveryReadWindow <= 0 {
		return fmt.Errorf("recovery read window must be positive")
	}
	if c.RecoveryReadPerHour <= 0 {
		return fmt.Errorf("recovery reads per hour must be positive")
	}
	return nil
}

func envOrDefault(name string, fallback string) string {
	if value := os.Getenv(name); value != "" {
		return value
	}
	return fallback
}
