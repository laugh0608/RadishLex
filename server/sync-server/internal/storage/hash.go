package storage

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"strconv"
)

const ciphertextHashPrefix = "sha256:"
const objectCiphertextHashDomain = "radishlex-ciphertext-hash-v1"
const objectEnvelopeSchemaVersion = "1"

func CiphertextHash(ciphertext []byte) string {
	sum := sha256.Sum256(ciphertext)
	return ciphertextHashPrefix + hex.EncodeToString(sum[:])
}

func ObjectCiphertextHash(version ObjectVersion, ciphertext []byte) string {
	associatedData := objectAssociatedData(version)
	hasher := sha256.New()
	hasher.Write([]byte(objectCiphertextHashDomain))
	writeLengthPrefixedHashBytes(hasher, associatedData)
	writeLengthPrefixedHashBytes(hasher, ciphertext)
	return hex.EncodeToString(hasher.Sum(nil))
}

func objectAssociatedData(version ObjectVersion) []byte {
	var data []byte
	data = appendAADField(data, "schema_version", []byte(objectEnvelopeSchemaVersion))
	data = appendAADField(data, "object_id", []byte(version.ObjectID))
	data = appendAADField(data, "object_type", []byte(version.ObjectType))
	data = appendAADField(data, "owner_device_id", []byte(version.OwnerDeviceID))
	data = appendAADField(data, "key_id", []byte(version.KeyID))
	data = appendAADField(data, "key_epoch", []byte(strconv.FormatUint(version.KeyEpoch, 10)))
	data = appendAADField(data, "version", []byte(strconv.FormatUint(version.Version, 10)))
	data = appendAADField(data, "base_version", []byte(objectBaseVersionAADString(version.BaseVersion)))
	data = appendAADField(data, "created_at_ms", []byte(strconv.FormatInt(version.ClientCreatedAtMs, 10)))
	data = appendAADField(data, "updated_at_ms", []byte(strconv.FormatInt(version.ClientUpdatedAtMs, 10)))
	return data
}

func appendAADField(output []byte, name string, value []byte) []byte {
	output = append(output, []byte(name)...)
	output = append(output, '=')
	output = appendUint64(output, uint64(len(value)))
	output = append(output, value...)
	output = append(output, 0)
	return output
}

func objectBaseVersionAADString(value uint64) string {
	if value == 0 {
		return ""
	}
	return strconv.FormatUint(value, 10)
}

func writeLengthPrefixedHashBytes(hasher interface{ Write([]byte) (int, error) }, value []byte) {
	var length [8]byte
	binary.BigEndian.PutUint64(length[:], uint64(len(value)))
	_, _ = hasher.Write(length[:])
	_, _ = hasher.Write(value)
}

func appendUint64(output []byte, value uint64) []byte {
	var buffer [8]byte
	binary.BigEndian.PutUint64(buffer[:], value)
	return append(output, buffer[:]...)
}
