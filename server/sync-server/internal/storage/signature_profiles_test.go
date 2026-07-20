package storage

import (
	"encoding/hex"
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"
)

func TestDeviceSignatureProfilesMatchSharedCrossLanguageVectors(t *testing.T) {
	_, sourceFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve test source path")
	}
	fixturePath := filepath.Join(filepath.Dir(sourceFile), "../../../../tests/fixtures/device-signature-profiles-v1.txt")
	fixture, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatalf("read signature profile vectors: %v", err)
	}
	for _, line := range strings.Split(string(fixture), "\n") {
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		vector := parseSignatureProfileVector(t, line)
		t.Run(vector.caseID, func(t *testing.T) {
			err := verifySignatureProfileVector(vector)
			actual := "ok"
			if err != nil {
				var storageErr *Error
				if !errors.As(err, &storageErr) {
					t.Fatalf("unexpected error type: %T %v", err, err)
				}
				actual = storageErr.DetailCode
			}
			if actual != vector.expectedDetail {
				t.Fatalf("unexpected result: got %q want %q", actual, vector.expectedDetail)
			}
		})
	}
}

func TestExternalP256SignatureSmoke(t *testing.T) {
	if os.Getenv("RADISHLEX_GO_P256_SMOKE") != "1" {
		t.Skip("gated by the Apple P-256 Keychain smoke")
	}
	publicKey := decodeProfileHex(t, os.Getenv("RADISHLEX_GO_P256_PUBLIC_KEY_HEX"))
	signature := decodeProfileHex(t, os.Getenv("RADISHLEX_GO_P256_SIGNATURE_HEX"))
	canonical := decodeProfileHex(t, os.Getenv("RADISHLEX_GO_P256_CANONICAL_HEX"))
	if err := verifySignatureProfile(
		SignatureAlgorithmECDSAP256SHA256V1,
		publicKey,
		signature,
		canonical,
	); err != nil {
		t.Fatalf("verify external Apple P-256 signature: %v", err)
	}
}

type signatureProfileVector struct {
	caseID             string
	signatureAlgorithm string
	keyAlgorithm       string
	publicKey          []byte
	signature          []byte
	canonical          []byte
	createdAtMs        int64
	revokedAtMs        int64
	signedAtMs         int64
	expectedDetail     string
}

func parseSignatureProfileVector(t *testing.T, line string) signatureProfileVector {
	t.Helper()
	fields := strings.Split(line, "|")
	if len(fields) != 10 {
		t.Fatalf("invalid signature profile vector: %q", line)
	}
	return signatureProfileVector{
		caseID:             fields[0],
		signatureAlgorithm: fields[1],
		keyAlgorithm:       fields[2],
		publicKey:          decodeProfileHex(t, fields[3]),
		signature:          decodeProfileHex(t, fields[4]),
		canonical:          decodeProfileHex(t, fields[5]),
		createdAtMs:        parseProfileTimestamp(t, fields[6]),
		revokedAtMs:        parseProfileTimestamp(t, fields[7]),
		signedAtMs:         parseProfileTimestamp(t, fields[8]),
		expectedDetail:     fields[9],
	}
}

func verifySignatureProfileVector(vector signatureProfileVector) error {
	if !supportedSignatureAlgorithm(vector.signatureAlgorithm) {
		return newSignatureError(signatureDetailAlgorithm, "signature algorithm is unsupported")
	}
	if vector.signatureAlgorithm != vector.keyAlgorithm {
		return newSignatureError(signatureDetailMismatch, "signature algorithm does not match key profile")
	}
	if vector.signedAtMs < vector.createdAtMs ||
		(vector.revokedAtMs > 0 && vector.signedAtMs >= vector.revokedAtMs) {
		return newSignatureError(signatureDetailInactive, "signature timestamp is outside key lifetime")
	}
	return verifySignatureProfile(
		vector.signatureAlgorithm,
		vector.publicKey,
		vector.signature,
		vector.canonical,
	)
}

func decodeProfileHex(t *testing.T, value string) []byte {
	t.Helper()
	decoded, err := hex.DecodeString(value)
	if err != nil {
		t.Fatalf("decode hex value: %v", err)
	}
	return decoded
}

func parseProfileTimestamp(t *testing.T, value string) int64 {
	t.Helper()
	if value == "" {
		return 0
	}
	parsed, err := strconv.ParseInt(value, 10, 64)
	if err != nil {
		t.Fatalf("parse timestamp: %v", err)
	}
	return parsed
}
