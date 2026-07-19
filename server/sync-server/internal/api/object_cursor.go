package api

import (
	"encoding/base64"
	"fmt"
	"math"
	"strconv"
	"strings"
)

const objectCursorPrefix = "v1."

func encodeObjectCursor(domainID string, sequence uint64) string {
	payload := domainID + "\x00" + strconv.FormatUint(sequence, 10)
	return objectCursorPrefix + base64.RawURLEncoding.EncodeToString([]byte(payload))
}

func decodeObjectCursor(domainID string, cursor string) (uint64, error) {
	if len(cursor) > 256 || !strings.HasPrefix(cursor, objectCursorPrefix) {
		return 0, fmt.Errorf("unsupported object cursor")
	}
	payload, err := base64.RawURLEncoding.DecodeString(strings.TrimPrefix(cursor, objectCursorPrefix))
	if err != nil {
		return 0, fmt.Errorf("decode object cursor: %w", err)
	}
	encodedDomain, sequenceText, ok := strings.Cut(string(payload), "\x00")
	if !ok || encodedDomain != domainID || sequenceText == "" {
		return 0, fmt.Errorf("object cursor domain mismatch")
	}
	sequence, err := strconv.ParseUint(sequenceText, 10, 64)
	if err != nil || sequence > math.MaxInt64 {
		return 0, fmt.Errorf("decode object cursor sequence: %w", err)
	}
	return sequence, nil
}
