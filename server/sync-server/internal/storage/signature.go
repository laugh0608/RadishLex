package storage

import (
	"crypto/ed25519"
	"encoding/binary"
	"strconv"
)

const (
	signatureSchemaVersion = 1
	signatureAlgorithm     = "ed25519-v1"
	ed25519PublicKeyLen    = ed25519.PublicKeySize
	ed25519SignatureLen    = ed25519.SignatureSize
)

type signatureFields struct {
	SchemaVersion  uint16
	Algorithm      string
	KeyID          string
	SignerDeviceID string
	Signature      []byte
}

type signatureField struct {
	name  string
	value []byte
}

func verifyObjectSignature(version ObjectVersion, signer Device) error {
	fields := signatureFields{
		SchemaVersion:  version.SignatureSchemaVersion,
		Algorithm:      version.SignatureAlgorithm,
		KeyID:          version.SignatureKeyID,
		SignerDeviceID: version.OwnerDeviceID,
		Signature:      version.Signature,
	}
	if err := verifySignatureMetadata(fields, signer, version.ClientCreatedAtMs); err != nil {
		return err
	}
	baseVersion := ""
	if version.BaseVersion != 0 {
		baseVersion = strconv.FormatUint(version.BaseVersion, 10)
	}
	return verifyCanonicalSignature(fields, signer, "sync_object_manifest", []signatureField{
		textField("signature_schema_version", strconv.Itoa(int(fields.SchemaVersion))),
		textField("signature_algorithm", fields.Algorithm),
		textField("signature_key_id", fields.KeyID),
		textField("signer_device_id", fields.SignerDeviceID),
		textField("domain_id", version.DomainID),
		textField("object_id", version.ObjectID),
		textField("object_type", version.ObjectType),
		textField("version", strconv.FormatUint(version.Version, 10)),
		textField("base_version", baseVersion),
		textField("key_id", version.KeyID),
		textField("key_epoch", strconv.FormatUint(version.KeyEpoch, 10)),
		textField("envelope_algorithm", version.Algorithm),
		bytesField("nonce", version.Nonce),
		textField("encrypted_payload_len", strconv.FormatInt(version.EncryptedPayloadLen, 10)),
		textField("ciphertext_hash", version.CiphertextHash),
		textField("created_at_ms", strconv.FormatInt(version.ClientCreatedAtMs, 10)),
		textField("updated_at_ms", strconv.FormatInt(version.ClientUpdatedAtMs, 10)),
	})
}

func verifyAuthorizationSignature(authorization DeviceAuthorization, wrapping DeviceWrappingRecord, join JoinRequest, authorizer Device) error {
	fields := signatureFields{
		SchemaVersion:  authorization.SignatureSchemaVersion,
		Algorithm:      authorization.SignatureAlgorithm,
		KeyID:          authorization.SignatureKeyID,
		SignerDeviceID: authorization.AuthorizerDeviceID,
		Signature:      authorization.Signature,
	}
	if err := verifySignatureMetadata(fields, authorizer, authorization.CreatedAtMs); err != nil {
		return err
	}
	return verifyCanonicalSignature(fields, authorizer, "device_authorization", []signatureField{
		textField("signature_schema_version", strconv.Itoa(int(fields.SchemaVersion))),
		textField("signature_algorithm", fields.Algorithm),
		textField("signature_key_id", fields.KeyID),
		textField("authorizer_device_id", authorization.AuthorizerDeviceID),
		textField("recipient_device_id", authorization.RecipientDeviceID),
		textField("recipient_public_key_id", authorization.RecipientSigningPublicKeyID),
		bytesField("join_challenge", join.Challenge),
		textField("join_short_code", authorization.JoinShortCode),
		textField("key_epoch", strconv.FormatUint(authorization.KeyEpoch, 10)),
		textField("wrapping_key_id", wrapping.WrappingKeyID),
		textField("encrypted_key_len", strconv.FormatInt(wrapping.WrappedKeyLen, 10)),
		textField("created_at_ms", strconv.FormatInt(authorization.CreatedAtMs, 10)),
	})
}

func verifyRevocationSignature(revocation DeviceRevocation, revoker Device) error {
	fields := signatureFields{
		SchemaVersion:  revocation.SignatureSchemaVersion,
		Algorithm:      revocation.SignatureAlgorithm,
		KeyID:          revocation.SignatureKeyID,
		SignerDeviceID: revocation.RevokerDeviceID,
		Signature:      revocation.Signature,
	}
	if err := verifySignatureMetadata(fields, revoker, revocation.CreatedAtMs); err != nil {
		return err
	}
	return verifyCanonicalSignature(fields, revoker, "device_revocation", []signatureField{
		textField("signature_schema_version", strconv.Itoa(int(fields.SchemaVersion))),
		textField("signature_algorithm", fields.Algorithm),
		textField("signature_key_id", fields.KeyID),
		textField("revoked_by_device_id", revocation.RevokerDeviceID),
		textField("revoked_device_id", revocation.RevokedDeviceID),
		textField("previous_key_epoch", strconv.FormatUint(revocation.PreviousKeyEpoch, 10)),
		textField("new_key_epoch", strconv.FormatUint(revocation.NewKeyEpoch, 10)),
		textField("reason", revocation.Reason),
		textField("revoked_at_ms", strconv.FormatInt(revocation.CreatedAtMs, 10)),
	})
}

func verifyRecoverySignature(record RecoveryRecord, signer Device) error {
	fields := signatureFields{
		SchemaVersion:  record.SignatureSchemaVersion,
		Algorithm:      record.SignatureAlgorithm,
		KeyID:          record.SignatureKeyID,
		SignerDeviceID: record.SignerDeviceID,
		Signature:      record.Signature,
	}
	if err := verifySignatureMetadata(fields, signer, record.CreatedAtMs); err != nil {
		return err
	}
	return verifyCanonicalSignature(fields, signer, "recovery_record", []signatureField{
		textField("signature_schema_version", strconv.Itoa(int(fields.SchemaVersion))),
		textField("signature_algorithm", fields.Algorithm),
		textField("signature_key_id", fields.KeyID),
		textField("signer_device_id", fields.SignerDeviceID),
		textField("recovery_id", record.RecoveryRecordID),
		textField("domain_id", record.DomainID),
		textField("key_epoch", strconv.FormatUint(record.KeyEpoch, 10)),
		textField("kdf_id", record.KDFProfile),
		textField("kdf_version", strconv.Itoa(int(record.KDFVersion))),
		bytesField("salt", record.Salt),
		textField("memory_kib", strconv.FormatUint(uint64(record.MemoryKiB), 10)),
		textField("iterations", strconv.FormatUint(uint64(record.Iterations), 10)),
		textField("parallelism", strconv.FormatUint(uint64(record.Parallelism), 10)),
		textField("output_len", strconv.FormatInt(record.OutputLen, 10)),
		textField("envelope_algorithm", record.Algorithm),
		bytesField("envelope_nonce", record.Nonce),
		textField("encrypted_recovery_key_len", strconv.FormatInt(record.WrappedMaterialLen, 10)),
		textField("created_at_ms", strconv.FormatInt(record.CreatedAtMs, 10)),
		textField("updated_at_ms", strconv.FormatInt(record.CreatedAtMs, 10)),
	})
}

func verifySignatureMetadata(fields signatureFields, signer Device, signedAtMs int64) error {
	if fields.SchemaVersion != signatureSchemaVersion {
		return newError(ErrInvalidSignature, "signature schema version is unsupported")
	}
	if fields.Algorithm != signatureAlgorithm {
		return newError(ErrInvalidSignature, "signature algorithm is unsupported")
	}
	if fields.KeyID == "" || fields.KeyID != signer.SigningPublicKeyID {
		return newError(ErrInvalidSignature, "signature key id does not match signer device")
	}
	if fields.SignerDeviceID == "" || fields.SignerDeviceID != signer.DeviceID {
		return newError(ErrInvalidSignature, "signature signer does not match device")
	}
	if signer.Status != DeviceActive {
		return newError(ErrForbiddenDevice, "signature signer device is not active")
	}
	if signedAtMs < signer.AuthorizedAtMs || (signer.RevokedAtMs > 0 && signedAtMs >= signer.RevokedAtMs) {
		return newError(ErrInvalidSignature, "signature timestamp is outside signer lifetime")
	}
	if len(signer.SigningPublicKey) != ed25519PublicKeyLen {
		return newError(ErrInvalidSignature, "signing public key length is invalid")
	}
	if len(fields.Signature) != ed25519SignatureLen {
		return newError(ErrInvalidSignature, "signature length is invalid")
	}
	return nil
}

func verifyCanonicalSignature(fields signatureFields, signer Device, recordType string, fieldsToSign []signatureField) error {
	canonical := canonicalSignatureBytes(recordType, fieldsToSign)
	if !ed25519.Verify(ed25519.PublicKey(signer.SigningPublicKey), canonical, fields.Signature) {
		return newError(ErrInvalidSignature, "signature verification failed")
	}
	return nil
}

func canonicalSignatureBytes(recordType string, fields []signatureField) []byte {
	var out []byte
	out = appendSignatureField(out, "domain_separator", []byte("radishlex-signature-v1"))
	out = appendSignatureField(out, "record_type", []byte(recordType))
	for _, field := range fields {
		out = appendSignatureField(out, field.name, field.value)
	}
	return out
}

func appendSignatureField(out []byte, name string, value []byte) []byte {
	out = append(out, []byte(name)...)
	out = append(out, '=')
	var length [8]byte
	binary.BigEndian.PutUint64(length[:], uint64(len(value)))
	out = append(out, length[:]...)
	out = append(out, value...)
	out = append(out, 0)
	return out
}

func textField(name string, value string) signatureField {
	return signatureField{name: name, value: []byte(value)}
}

func bytesField(name string, value []byte) signatureField {
	return signatureField{name: name, value: cloneBytes(value)}
}
