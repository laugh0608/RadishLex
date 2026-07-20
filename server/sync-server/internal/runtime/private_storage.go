package runtime

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/config"
)

const (
	privateDirectoryMode os.FileMode = 0o700
	privateFileMode      os.FileMode = 0o600
)

func preparePrivateStorePaths(cfg config.Config) error {
	metadataDirectory, blobDirectory, err := validateDedicatedStoreLayout(cfg.MetadataPath, cfg.BlobDir)
	if err != nil {
		return err
	}
	if err := requirePrivateDirectory(metadataDirectory, "metadata directory"); err != nil {
		return err
	}
	if err := requirePrivateDirectory(blobDirectory, "blob directory"); err != nil {
		return err
	}
	return preparePrivateRegularFile(cfg.MetadataPath, "metadata database")
}

func validateDedicatedStoreLayout(metadataPath string, blobDir string) (string, string, error) {
	metadataAbsolute, err := filepath.Abs(metadataPath)
	if err != nil {
		return "", "", fmt.Errorf("metadata path cannot be resolved")
	}
	blobAbsolute, err := filepath.Abs(blobDir)
	if err != nil {
		return "", "", fmt.Errorf("blob path cannot be resolved")
	}
	metadataDirectory := filepath.Dir(metadataAbsolute)
	if metadataDirectory == filepath.Clean(string(filepath.Separator)) || filepath.Dir(blobAbsolute) != metadataDirectory {
		return "", "", fmt.Errorf("metadata and blob paths must share a dedicated storage directory")
	}
	entries, err := os.ReadDir(metadataDirectory)
	if err != nil && !os.IsNotExist(err) {
		return "", "", fmt.Errorf("storage directory cannot be inspected")
	}
	allowed := map[string]struct{}{
		filepath.Base(metadataAbsolute):              {},
		filepath.Base(metadataAbsolute) + "-journal": {},
		filepath.Base(metadataAbsolute) + "-shm":     {},
		filepath.Base(metadataAbsolute) + "-wal":     {},
		filepath.Base(blobAbsolute):                  {},
	}
	for _, entry := range entries {
		if _, ok := allowed[entry.Name()]; !ok {
			return "", "", fmt.Errorf("storage directory contains unexpected entries")
		}
	}
	return metadataDirectory, blobAbsolute, nil
}

func requirePrivateDirectory(path string, label string) error {
	if err := os.MkdirAll(path, privateDirectoryMode); err != nil {
		return fmt.Errorf("%s cannot be created", label)
	}
	info, err := os.Lstat(path)
	if err != nil {
		return fmt.Errorf("%s cannot be inspected", label)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return fmt.Errorf("%s must be a non-symlink directory", label)
	}
	if err := os.Chmod(path, privateDirectoryMode); err != nil {
		return fmt.Errorf("%s permissions cannot be restricted", label)
	}
	info, err = os.Lstat(path)
	if err != nil || info.Mode().Perm() != privateDirectoryMode {
		return fmt.Errorf("%s permissions are not private", label)
	}
	return nil
}

func preparePrivateRegularFile(path string, label string) error {
	info, err := os.Lstat(path)
	if os.IsNotExist(err) {
		file, createErr := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_RDWR, privateFileMode)
		if createErr != nil {
			return fmt.Errorf("%s cannot be created", label)
		}
		if closeErr := file.Close(); closeErr != nil {
			return fmt.Errorf("%s cannot be closed", label)
		}
		return requirePrivateRegularFile(path, label)
	}
	if err != nil {
		return fmt.Errorf("%s cannot be inspected", label)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return fmt.Errorf("%s must be a non-symlink regular file", label)
	}
	return requirePrivateRegularFile(path, label)
}

func requirePrivateRegularFile(path string, label string) error {
	info, err := os.Lstat(path)
	if err != nil {
		return fmt.Errorf("%s cannot be inspected", label)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return fmt.Errorf("%s must be a non-symlink regular file", label)
	}
	if err := os.Chmod(path, privateFileMode); err != nil {
		return fmt.Errorf("%s permissions cannot be restricted", label)
	}
	info, err = os.Lstat(path)
	if err != nil || info.Mode().Perm() != privateFileMode {
		return fmt.Errorf("%s permissions are not private", label)
	}
	return nil
}
