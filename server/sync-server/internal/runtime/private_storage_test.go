package runtime

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/config"
)

func TestOpenStoreCreatesPrivateStoragePaths(t *testing.T) {
	root := t.TempDir()
	cfg := privateStorageConfig(root)
	store, closeStore, err := OpenStore(cfg)
	if err != nil {
		t.Fatalf("open private store: %v", err)
	}
	if store == nil {
		t.Fatal("open private store returned nil store")
	}
	if err := closeStore(); err != nil {
		t.Fatalf("close private store: %v", err)
	}

	assertPathMode(t, filepath.Dir(cfg.MetadataPath), privateDirectoryMode)
	assertPathMode(t, cfg.BlobDir, privateDirectoryMode)
	assertPathMode(t, cfg.MetadataPath, privateFileMode)
}

func TestOpenStoreRestrictsExistingStoragePermissions(t *testing.T) {
	root := t.TempDir()
	cfg := privateStorageConfig(root)
	if err := os.MkdirAll(filepath.Dir(cfg.MetadataPath), 0o777); err != nil {
		t.Fatalf("create broad metadata directory: %v", err)
	}
	if err := os.MkdirAll(cfg.BlobDir, 0o777); err != nil {
		t.Fatalf("create broad blob directory: %v", err)
	}
	if err := os.WriteFile(cfg.MetadataPath, nil, 0o666); err != nil {
		t.Fatalf("create broad metadata file: %v", err)
	}
	for _, path := range []string{filepath.Dir(cfg.MetadataPath), cfg.BlobDir} {
		if err := os.Chmod(path, 0o777); err != nil {
			t.Fatalf("broaden directory permissions: %v", err)
		}
	}
	if err := os.Chmod(cfg.MetadataPath, 0o666); err != nil {
		t.Fatalf("broaden metadata permissions: %v", err)
	}

	_, closeStore, err := OpenStore(cfg)
	if err != nil {
		t.Fatalf("open store with broad permissions: %v", err)
	}
	if err := closeStore(); err != nil {
		t.Fatalf("close restricted store: %v", err)
	}
	assertPathMode(t, filepath.Dir(cfg.MetadataPath), privateDirectoryMode)
	assertPathMode(t, cfg.BlobDir, privateDirectoryMode)
	assertPathMode(t, cfg.MetadataPath, privateFileMode)
}

func TestOpenStoreRejectsSymlinkedStorageLeavesWithoutLeakingPaths(t *testing.T) {
	root := t.TempDir()
	t.Run("metadata", func(t *testing.T) {
		cfg := privateStorageConfig(filepath.Join(root, "metadata-case"))
		if err := os.MkdirAll(filepath.Dir(cfg.MetadataPath), 0o700); err != nil {
			t.Fatalf("create metadata directory: %v", err)
		}
		target := filepath.Join(root, "metadata-target")
		if err := os.WriteFile(target, []byte("synthetic"), 0o600); err != nil {
			t.Fatalf("create metadata target: %v", err)
		}
		if err := os.Symlink(target, cfg.MetadataPath); err != nil {
			t.Fatalf("create metadata symlink: %v", err)
		}
		_, _, err := OpenStore(cfg)
		assertPrivateStorageError(t, err, root, "metadata database must be a non-symlink regular file")
	})

	t.Run("blob", func(t *testing.T) {
		cfg := privateStorageConfig(filepath.Join(root, "blob-case"))
		if err := os.MkdirAll(filepath.Dir(cfg.BlobDir), 0o700); err != nil {
			t.Fatalf("create blob parent: %v", err)
		}
		target := filepath.Join(root, "blob-target")
		if err := os.Mkdir(target, 0o700); err != nil {
			t.Fatalf("create blob target: %v", err)
		}
		if err := os.Symlink(target, cfg.BlobDir); err != nil {
			t.Fatalf("create blob symlink: %v", err)
		}
		_, _, err := OpenStore(cfg)
		assertPrivateStorageError(t, err, root, "blob directory must be a non-symlink directory")
	})
}

func TestOpenStoreRejectsSharedOrSplitStorageDirectoriesWithoutChangingSharedMode(t *testing.T) {
	root := t.TempDir()
	t.Run("shared directory", func(t *testing.T) {
		shared := filepath.Join(root, "shared")
		if err := os.Mkdir(shared, 0o777); err != nil {
			t.Fatalf("create shared directory: %v", err)
		}
		if err := os.Chmod(shared, 0o777); err != nil {
			t.Fatalf("broaden shared directory: %v", err)
		}
		if err := os.WriteFile(filepath.Join(shared, "unrelated"), []byte("synthetic"), 0o600); err != nil {
			t.Fatalf("create unrelated file: %v", err)
		}
		cfg := config.Default()
		cfg.MetadataPath = filepath.Join(shared, "sync-server.sqlite")
		cfg.BlobDir = filepath.Join(shared, "objects")
		_, _, err := OpenStore(cfg)
		assertPrivateStorageError(t, err, root, "storage directory contains unexpected entries")
		assertPathMode(t, shared, 0o777)
	})

	t.Run("split directories", func(t *testing.T) {
		cfg := config.Default()
		cfg.MetadataPath = filepath.Join(root, "metadata", "sync-server.sqlite")
		cfg.BlobDir = filepath.Join(root, "blobs", "objects")
		_, _, err := OpenStore(cfg)
		assertPrivateStorageError(t, err, root, "metadata and blob paths must share a dedicated storage directory")
	})
}

func privateStorageConfig(root string) config.Config {
	cfg := config.Default()
	cfg.MetadataPath = filepath.Join(root, "data", "sync-server.sqlite")
	cfg.BlobDir = filepath.Join(root, "data", "objects")
	return cfg
}

func assertPathMode(t *testing.T, path string, expected os.FileMode) {
	t.Helper()
	info, err := os.Lstat(path)
	if err != nil {
		t.Fatalf("inspect private path: %v", err)
	}
	if info.Mode().Perm() != expected {
		t.Fatalf("unexpected private path mode: got %o want %o", info.Mode().Perm(), expected)
	}
}

func assertPrivateStorageError(t *testing.T, err error, forbiddenPath string, expected string) {
	t.Helper()
	if err == nil || err.Error() != expected {
		t.Fatalf("unexpected private storage error: %v", err)
	}
	if strings.Contains(err.Error(), forbiddenPath) {
		t.Fatalf("private storage error leaked absolute path: %v", err)
	}
}
