package api

import (
	"encoding/base64"
	"fmt"
	"math"
	"strconv"
	"strings"
)

const lifecycleCursorPrefix = "lifecycle-v1."

func encodeLifecycleCursor(domainID string, sequence uint64) string {
	payload := domainID + "\x00" + strconv.FormatUint(sequence, 10)
	return lifecycleCursorPrefix + base64.RawURLEncoding.EncodeToString([]byte(payload))
}

func decodeLifecycleCursor(domainID string, cursor string) (uint64, error) {
	if len(cursor) > 256 || !strings.HasPrefix(cursor, lifecycleCursorPrefix) {
		return 0, fmt.Errorf("unsupported lifecycle cursor")
	}
	payload, err := base64.RawURLEncoding.DecodeString(strings.TrimPrefix(cursor, lifecycleCursorPrefix))
	if err != nil {
		return 0, fmt.Errorf("decode lifecycle cursor: %w", err)
	}
	encodedDomain, sequenceText, ok := strings.Cut(string(payload), "\x00")
	if !ok || encodedDomain != domainID || sequenceText == "" {
		return 0, fmt.Errorf("lifecycle cursor domain mismatch")
	}
	sequence, err := strconv.ParseUint(sequenceText, 10, 64)
	if err != nil || sequence > math.MaxInt64 {
		return 0, fmt.Errorf("decode lifecycle cursor sequence: %w", err)
	}
	return sequence, nil
}
