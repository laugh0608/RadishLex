package storage

import (
	"crypto/sha256"
	"encoding/hex"
)

const ciphertextHashPrefix = "sha256:"

func CiphertextHash(ciphertext []byte) string {
	sum := sha256.Sum256(ciphertext)
	return ciphertextHashPrefix + hex.EncodeToString(sum[:])
}
