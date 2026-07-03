package api

import (
	"errors"
	"time"

	"github.com/laugh0608/RadishLex/server/sync-server/internal/storage"
)

const PrefixV1 = "/api/v1"

type ErrorResponse struct {
	ErrorCode            string `json:"error_code"`
	Message              string `json:"message"`
	Retryable            bool   `json:"retryable"`
	ServerTimeMs         int64  `json:"server_time_ms"`
	LatestVersion        uint64 `json:"latest_version,omitempty"`
	LatestCiphertextHash string `json:"latest_ciphertext_hash,omitempty"`
}

func ErrorResponseFrom(err error, now time.Time) ErrorResponse {
	var storageErr *storage.Error
	if errors.As(err, &storageErr) {
		return ErrorResponse{
			ErrorCode:            string(storageErr.Code),
			Message:              storageErr.Message,
			Retryable:            storageErr.Retryable,
			ServerTimeMs:         now.UnixMilli(),
			LatestVersion:        storageErr.LatestVersion,
			LatestCiphertextHash: storageErr.LatestCiphertextHash,
		}
	}
	return ErrorResponse{
		ErrorCode:    string(storage.ErrStorageUnavailable),
		Message:      "storage operation failed",
		Retryable:    true,
		ServerTimeMs: now.UnixMilli(),
	}
}
