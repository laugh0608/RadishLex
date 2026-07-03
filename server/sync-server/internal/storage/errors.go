package storage

import "fmt"

type ErrorCode string

const (
	ErrInvalidRequest            ErrorCode = "invalid_request"
	ErrUnauthenticated           ErrorCode = "unauthenticated"
	ErrForbiddenDevice           ErrorCode = "forbidden_device"
	ErrNotFound                  ErrorCode = "not_found"
	ErrConflictStaleBaseVersion  ErrorCode = "conflict_stale_base_version"
	ErrConflictObjectVersion     ErrorCode = "conflict_object_version"
	ErrInvalidSignature          ErrorCode = "invalid_signature"
	ErrInvalidCiphertextMetadata ErrorCode = "invalid_ciphertext_metadata"
	ErrPayloadTooLarge           ErrorCode = "payload_too_large"
	ErrRecoveryRateLimited       ErrorCode = "recovery_rate_limited"
	ErrStorageUnavailable        ErrorCode = "storage_unavailable"
)

type Error struct {
	Code                 ErrorCode
	Message              string
	Retryable            bool
	LatestVersion        uint64
	LatestCiphertextHash string
}

func (e *Error) Error() string {
	if e == nil {
		return ""
	}
	if e.Message == "" {
		return string(e.Code)
	}
	return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

func newError(code ErrorCode, message string) *Error {
	return &Error{Code: code, Message: message}
}

func conflictStaleBaseVersion(latestVersion uint64, latestCiphertextHash string) *Error {
	return &Error{
		Code:                 ErrConflictStaleBaseVersion,
		Message:              "object upload is based on an older version",
		LatestVersion:        latestVersion,
		LatestCiphertextHash: latestCiphertextHash,
	}
}

func IsCode(err error, code ErrorCode) bool {
	storageErr, ok := err.(*Error)
	return ok && storageErr.Code == code
}
