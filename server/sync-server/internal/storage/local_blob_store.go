package storage

import (
	"bytes"
	"context"
	"io"
	"os"
	"path"
	"path/filepath"
	"strings"
	"sync"
)

type ObjectBlobStore interface {
	StageObjectBlob(ctx context.Context, finalRef string, payload []byte) (StagedObjectBlob, error)
	ReadObjectBlob(ctx context.Context, finalRef string) ([]byte, error)
	DeleteObjectBlob(ctx context.Context, finalRef string) error
}

type StagedObjectBlob interface {
	FinalRef() string
	TempRef() string
	Commit(ctx context.Context) error
	Cleanup(ctx context.Context) error
}

type LocalObjectBlobStore struct {
	root string
}

func NewLocalObjectBlobStore(root string) (*LocalObjectBlobStore, error) {
	if root == "" {
		return nil, newError(ErrInvalidRequest, "blob store root is required")
	}
	absoluteRoot, err := filepath.Abs(root)
	if err != nil {
		return nil, newError(ErrStorageUnavailable, "blob store root cannot be resolved")
	}
	return &LocalObjectBlobStore{root: absoluteRoot}, nil
}

func (s *LocalObjectBlobStore) StageObjectBlob(ctx context.Context, finalRef string, payload []byte) (StagedObjectBlob, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	if len(payload) == 0 {
		return nil, newError(ErrInvalidCiphertextMetadata, "blob payload is required")
	}
	finalPath, err := s.resolveBlobRef(finalRef)
	if err != nil {
		return nil, err
	}

	tmpDir := filepath.Join(s.root, ".tmp")
	if err := os.MkdirAll(tmpDir, 0o700); err != nil {
		return nil, newError(ErrStorageUnavailable, "blob temp directory cannot be created")
	}
	tempFile, err := os.CreateTemp(tmpDir, "blob-*")
	if err != nil {
		return nil, newError(ErrStorageUnavailable, "blob temp file cannot be created")
	}
	tempPath := tempFile.Name()
	writeErr := writeAndCloseTempBlob(tempFile, payload)
	if writeErr != nil {
		_ = os.Remove(tempPath)
		return nil, writeErr
	}

	return &localStagedObjectBlob{
		finalRef:  finalRef,
		tempRef:   s.relativeRef(tempPath),
		finalPath: finalPath,
		tempPath:  tempPath,
	}, nil
}

func (s *LocalObjectBlobStore) ReadObjectBlob(ctx context.Context, finalRef string) ([]byte, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	finalPath, err := s.resolveBlobRef(finalRef)
	if err != nil {
		return nil, err
	}
	payload, err := os.ReadFile(finalPath)
	if os.IsNotExist(err) {
		return nil, newError(ErrNotFound, "blob not found")
	}
	if err != nil {
		return nil, newError(ErrStorageUnavailable, "blob cannot be read")
	}
	return payload, nil
}

func (s *LocalObjectBlobStore) DeleteObjectBlob(ctx context.Context, finalRef string) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	finalPath, err := s.resolveBlobRef(finalRef)
	if err != nil {
		return err
	}
	if err := os.Remove(finalPath); err != nil && !os.IsNotExist(err) {
		return newError(ErrStorageUnavailable, "blob cannot be deleted")
	}
	return nil
}

func (s *LocalObjectBlobStore) resolveBlobRef(ref string) (string, error) {
	cleanRef, err := validateBlobRef(ref)
	if err != nil {
		return "", err
	}
	finalPath := filepath.Join(s.root, filepath.FromSlash(cleanRef))
	relativePath, err := filepath.Rel(s.root, finalPath)
	if err != nil || strings.HasPrefix(relativePath, ".."+string(filepath.Separator)) || relativePath == ".." || filepath.IsAbs(relativePath) {
		return "", newError(ErrInvalidRequest, "blob ref escapes store root")
	}
	return finalPath, nil
}

func (s *LocalObjectBlobStore) relativeRef(filePath string) string {
	relativePath, err := filepath.Rel(s.root, filePath)
	if err != nil {
		return filepath.ToSlash(filepath.Base(filePath))
	}
	return filepath.ToSlash(relativePath)
}

type localStagedObjectBlob struct {
	mu        sync.Mutex
	finalRef  string
	tempRef   string
	finalPath string
	tempPath  string
	committed bool
	cleaned   bool
}

func (s *localStagedObjectBlob) FinalRef() string {
	return s.finalRef
}

func (s *localStagedObjectBlob) TempRef() string {
	return s.tempRef
}

func (s *localStagedObjectBlob) Commit(ctx context.Context) error {
	if err := checkContext(ctx); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	if s.committed {
		return nil
	}
	if s.cleaned {
		return newError(ErrStorageUnavailable, "staged blob was already cleaned")
	}
	if err := os.MkdirAll(filepath.Dir(s.finalPath), 0o700); err != nil {
		return newError(ErrStorageUnavailable, "blob final directory cannot be created")
	}

	if err := os.Link(s.tempPath, s.finalPath); err != nil {
		if os.IsExist(err) {
			return s.commitExistingBlob()
		}
		return newError(ErrStorageUnavailable, "blob cannot be committed")
	}
	if err := os.Remove(s.tempPath); err != nil && !os.IsNotExist(err) {
		return newError(ErrStorageUnavailable, "blob temp file cannot be removed")
	}
	s.committed = true
	return nil
}

func (s *localStagedObjectBlob) Cleanup(ctx context.Context) error {
	if err := checkContext(ctx); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	if s.committed || s.cleaned {
		return nil
	}
	if err := os.Remove(s.tempPath); err != nil && !os.IsNotExist(err) {
		return newError(ErrStorageUnavailable, "blob temp file cannot be removed")
	}
	s.cleaned = true
	return nil
}

func (s *localStagedObjectBlob) commitExistingBlob() error {
	stagedPayload, err := os.ReadFile(s.tempPath)
	if err != nil {
		return newError(ErrStorageUnavailable, "staged blob cannot be read")
	}
	existingPayload, err := os.ReadFile(s.finalPath)
	if err != nil {
		return newError(ErrStorageUnavailable, "existing blob cannot be read")
	}
	if !bytes.Equal(stagedPayload, existingPayload) {
		_ = os.Remove(s.tempPath)
		s.cleaned = true
		return newError(ErrConflictObjectVersion, "blob ref already exists with different payload")
	}
	if err := os.Remove(s.tempPath); err != nil && !os.IsNotExist(err) {
		return newError(ErrStorageUnavailable, "blob temp file cannot be removed")
	}
	s.committed = true
	return nil
}

func writeAndCloseTempBlob(tempFile *os.File, payload []byte) error {
	_, writeErr := io.Copy(tempFile, bytes.NewReader(payload))
	closeErr := tempFile.Close()
	if writeErr != nil {
		return newError(ErrStorageUnavailable, "blob temp file cannot be written")
	}
	if closeErr != nil {
		return newError(ErrStorageUnavailable, "blob temp file cannot be closed")
	}
	return nil
}

func validateBlobRef(ref string) (string, error) {
	if ref == "" {
		return "", newError(ErrInvalidRequest, "blob ref is required")
	}
	if strings.Contains(ref, "\\") || strings.Contains(ref, ":") || path.IsAbs(ref) {
		return "", newError(ErrInvalidRequest, "blob ref must be a safe relative path")
	}
	cleanRef := path.Clean(ref)
	if cleanRef == "." || cleanRef != ref || cleanRef == ".." || strings.HasPrefix(cleanRef, "../") || strings.Contains(cleanRef, "/../") {
		return "", newError(ErrInvalidRequest, "blob ref must be canonical")
	}
	if cleanRef == ".tmp" || strings.HasPrefix(cleanRef, ".tmp/") {
		return "", newError(ErrInvalidRequest, "blob ref uses reserved temp namespace")
	}
	for _, char := range cleanRef {
		if !safeBlobRefChar(char) {
			return "", newError(ErrInvalidRequest, "blob ref contains unsupported characters")
		}
	}
	return cleanRef, nil
}

func safeBlobRefChar(char rune) bool {
	return (char >= 'a' && char <= 'z') ||
		(char >= 'A' && char <= 'Z') ||
		(char >= '0' && char <= '9') ||
		char == '/' ||
		char == '.' ||
		char == '_' ||
		char == '-'
}
