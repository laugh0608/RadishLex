package storage

import (
	"crypto/ecdsa"
	"crypto/ed25519"
	"crypto/elliptic"
	"crypto/sha256"
	"encoding/binary"
	"math/big"
	"strconv"
)

const (
	signatureSchemaVersion   = 1
	signatureAlgorithm       = SignatureAlgorithmEd25519V1
	ed25519PublicKeyLen      = ed25519.PublicKeySize
	ed25519SignatureLen      = ed25519.SignatureSize
	p256PublicKeyLen         = 65
	p256SignatureLen         = 64
	signatureDetailAlgorithm = "unsupported_signature_algorithm"
	signatureDetailMismatch  = "signature_algorithm_mismatch"
	signatureDetailPublicKey = "invalid_signing_public_key"
	signatureDetailEncoding  = "invalid_signature_encoding"
	signatureDetailVerify    = "signature_verification_failed"
	signatureDetailInactive  = "signature_key_not_active"
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

func verifyEpochDistributionSignature(record DeviceWrappingRecord, distributor Device) error {
	fields := signatureFields{
		SchemaVersion:  record.SignatureSchemaVersion,
		Algorithm:      record.SignatureAlgorithm,
		KeyID:          record.SignatureKeyID,
		SignerDeviceID: record.AuthorizerDeviceID,
		Signature:      record.Signature,
	}
	if err := verifySignatureMetadata(fields, distributor, record.CreatedAtMs); err != nil {
		return err
	}
	return verifyCanonicalSignature(fields, distributor, "epoch_distribution", []signatureField{
		textField("signature_schema_version", strconv.Itoa(int(fields.SchemaVersion))),
		textField("signature_algorithm", fields.Algorithm),
		textField("signature_key_id", fields.KeyID),
		textField("distributor_device_id", record.AuthorizerDeviceID),
		textField("domain_id", record.DomainID),
		textField("recipient_device_id", record.RecipientDeviceID),
		textField("recipient_key_agreement_key_id", record.RecipientKeyAgreementKeyID),
		textField("key_epoch", strconv.FormatUint(record.KeyEpoch, 10)),
		textField("wrapping_key_id", record.WrappingKeyID),
		textField("envelope_algorithm", record.Algorithm),
		bytesField("envelope_nonce", record.Nonce),
		textField("wrapped_key_len", strconv.FormatInt(record.WrappedKeyLen, 10)),
		textField("ciphertext_hash", record.CiphertextHash),
		textField("created_at_ms", strconv.FormatInt(record.CreatedAtMs, 10)),
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
	return verifyCanonicalSignature(fields, signer, "recovery_record_v2", []signatureField{
		textField("signature_schema_version", strconv.Itoa(int(fields.SchemaVersion))),
		textField("signature_algorithm", fields.Algorithm),
		textField("signature_key_id", fields.KeyID),
		textField("signer_device_id", fields.SignerDeviceID),
		textField("record_schema_version", strconv.Itoa(int(record.RecordSchemaVersion))),
		textField("recovery_id", record.RecoveryRecordID),
		textField("previous_recovery_id", record.PreviousRecoveryID),
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
		textField("ciphertext_hash", record.CiphertextHash),
		textField("activation_algorithm", record.ActivationAlgorithm),
		textField("activation_public_key_id", record.ActivationPublicKeyID),
		bytesField("activation_public_key", record.ActivationPublicKey),
		textField("created_at_ms", strconv.FormatInt(record.CreatedAtMs, 10)),
		textField("updated_at_ms", strconv.FormatInt(record.UpdatedAtMs, 10)),
	})
}

func verifyRecoveredDeviceActivationSignature(activation RecoveredDeviceActivation, record RecoveryRecord) error {
	if activation.SignatureSchemaVersion != signatureSchemaVersion ||
		activation.ActivationAlgorithm != SignatureAlgorithmEd25519V1 ||
		activation.ActivationAlgorithm != record.ActivationAlgorithm ||
		activation.ActivationPublicKeyID != record.ActivationPublicKeyID {
		return newSignatureError(signatureDetailMismatch, "recovery activation signature profile does not match recovery record")
	}
	if len(record.ActivationPublicKey) != ed25519.PublicKeySize || len(activation.ActivationSignature) != ed25519.SignatureSize {
		return newSignatureError(signatureDetailEncoding, "recovery activation signature encoding is invalid")
	}
	canonical := canonicalSignatureBytes(RecoveredDeviceActivationRecordType, []signatureField{
		textField("signature_schema_version", strconv.Itoa(int(activation.SignatureSchemaVersion))),
		textField("activation_algorithm", activation.ActivationAlgorithm),
		textField("activation_public_key_id", activation.ActivationPublicKeyID),
		textField("recovery_record_id", activation.RecoveryRecordID),
		textField("domain_id", activation.DomainID),
		textField("device_id", activation.DeviceID),
		textField("signing_algorithm", activation.SigningAlgorithm),
		textField("signing_public_key_id", activation.SigningPublicKeyID),
		bytesField("signing_public_key", activation.SigningPublicKey),
		textField("key_agreement_algorithm", activation.KeyAgreementAlgorithm),
		textField("key_agreement_public_key_id", activation.KeyAgreementPublicKeyID),
		bytesField("key_agreement_public_key", activation.KeyAgreementPublicKey),
		textField("key_epoch", strconv.FormatUint(activation.KeyEpoch, 10)),
		textField("created_at_ms", strconv.FormatInt(activation.CreatedAtMs, 10)),
	})
	if !ed25519.Verify(ed25519.PublicKey(record.ActivationPublicKey), canonical, activation.ActivationSignature) {
		return newSignatureError(signatureDetailVerify, "recovery activation signature verification failed")
	}
	return nil
}

func verifyRecoveryRecordRevocationSignature(revocation RecoveryRecordRevocation, revoker Device) error {
	fields := signatureFields{
		SchemaVersion: revocation.SignatureSchemaVersion, Algorithm: revocation.SignatureAlgorithm,
		KeyID: revocation.SignatureKeyID, SignerDeviceID: revocation.RevokerDeviceID,
		Signature: revocation.Signature,
	}
	if err := verifySignatureMetadata(fields, revoker, revocation.CreatedAtMs); err != nil {
		return err
	}
	return verifyCanonicalSignature(fields, revoker, RecoveryRecordRevocationRecordType, []signatureField{
		textField("signature_schema_version", strconv.Itoa(int(fields.SchemaVersion))),
		textField("signature_algorithm", fields.Algorithm),
		textField("signature_key_id", fields.KeyID),
		textField("revoker_device_id", revocation.RevokerDeviceID),
		textField("recovery_record_id", revocation.RecoveryRecordID),
		textField("domain_id", revocation.DomainID),
		textField("key_epoch", strconv.FormatUint(revocation.KeyEpoch, 10)),
		textField("reason", revocation.Reason),
		textField("created_at_ms", strconv.FormatInt(revocation.CreatedAtMs, 10)),
	})
}

func verifySignatureMetadata(fields signatureFields, signer Device, signedAtMs int64) error {
	if fields.SchemaVersion != signatureSchemaVersion {
		return newSignatureError(signatureDetailAlgorithm, "signature schema version is unsupported")
	}
	if !supportedSignatureAlgorithm(fields.Algorithm) {
		return newSignatureError(signatureDetailAlgorithm, "signature algorithm is unsupported")
	}
	if fields.Algorithm != signer.SigningAlgorithm {
		return newSignatureError(signatureDetailMismatch, "signature algorithm does not match signer device")
	}
	if fields.KeyID == "" || fields.KeyID != signer.SigningPublicKeyID {
		return newSignatureError(signatureDetailVerify, "signature key id does not match signer device")
	}
	if fields.SignerDeviceID == "" || fields.SignerDeviceID != signer.DeviceID {
		return newSignatureError(signatureDetailVerify, "signature signer does not match device")
	}
	if signer.Status != DeviceActive {
		return newError(ErrForbiddenDevice, "signature signer device is not active")
	}
	if signedAtMs < signer.AuthorizedAtMs || (signer.RevokedAtMs > 0 && signedAtMs >= signer.RevokedAtMs) {
		return newSignatureError(signatureDetailInactive, "signature timestamp is outside signer lifetime")
	}
	if err := validateSigningPublicKeyEncoding(fields.Algorithm, signer.SigningPublicKey); err != nil {
		return err
	}
	if err := validateSignatureEncoding(fields.Algorithm, fields.Signature); err != nil {
		return err
	}
	return nil
}

func verifyCanonicalSignature(fields signatureFields, signer Device, recordType string, fieldsToSign []signatureField) error {
	canonical := canonicalSignatureBytes(recordType, fieldsToSign)
	return verifySignatureProfile(fields.Algorithm, signer.SigningPublicKey, fields.Signature, canonical)
}

func verifySignatureProfile(algorithm string, publicKey []byte, signature []byte, canonical []byte) error {
	if !supportedSignatureAlgorithm(algorithm) {
		return newSignatureError(signatureDetailAlgorithm, "signature algorithm is unsupported")
	}
	if err := validateSigningPublicKeyEncoding(algorithm, publicKey); err != nil {
		return err
	}
	if err := validateSignatureEncoding(algorithm, signature); err != nil {
		return err
	}

	switch algorithm {
	case SignatureAlgorithmEd25519V1:
		if !ed25519.Verify(ed25519.PublicKey(publicKey), canonical, signature) {
			return newSignatureError(signatureDetailVerify, "signature verification failed")
		}
	case SignatureAlgorithmECDSAP256SHA256V1:
		x, y := elliptic.Unmarshal(elliptic.P256(), publicKey)
		if x == nil || y == nil {
			return newSignatureError(signatureDetailPublicKey, "signing public key encoding is invalid")
		}
		r := new(big.Int).SetBytes(signature[:32])
		s := new(big.Int).SetBytes(signature[32:])
		digest := sha256.Sum256(canonical)
		if !ecdsa.Verify(&ecdsa.PublicKey{Curve: elliptic.P256(), X: x, Y: y}, digest[:], r, s) {
			return newSignatureError(signatureDetailVerify, "signature verification failed")
		}
	}
	return nil
}

func validateP256KeyAgreementPublicKey(publicKey []byte) error {
	if len(publicKey) != p256PublicKeyLen {
		return newError(ErrInvalidRequest, "P-256 key agreement public key length is invalid")
	}
	x, y := elliptic.Unmarshal(elliptic.P256(), publicKey)
	if x == nil || y == nil {
		return newError(ErrInvalidRequest, "P-256 key agreement public key encoding is invalid")
	}
	return nil
}

func validateSigningPublicKeyEncoding(algorithm string, publicKey []byte) error {
	switch algorithm {
	case SignatureAlgorithmEd25519V1:
		if len(publicKey) != ed25519PublicKeyLen {
			return newSignatureError(signatureDetailPublicKey, "signing public key encoding is invalid")
		}
	case SignatureAlgorithmECDSAP256SHA256V1:
		if len(publicKey) != p256PublicKeyLen || publicKey[0] != 0x04 {
			return newSignatureError(signatureDetailPublicKey, "signing public key encoding is invalid")
		}
		x, y := elliptic.Unmarshal(elliptic.P256(), publicKey)
		if x == nil || y == nil || !elliptic.P256().IsOnCurve(x, y) {
			return newSignatureError(signatureDetailPublicKey, "signing public key encoding is invalid")
		}
	default:
		return newSignatureError(signatureDetailAlgorithm, "signature algorithm is unsupported")
	}
	return nil
}

func validateSignatureEncoding(algorithm string, signature []byte) error {
	switch algorithm {
	case SignatureAlgorithmEd25519V1:
		if len(signature) != ed25519SignatureLen {
			return newSignatureError(signatureDetailEncoding, "signature encoding is invalid")
		}
	case SignatureAlgorithmECDSAP256SHA256V1:
		if len(signature) != p256SignatureLen {
			return newSignatureError(signatureDetailEncoding, "signature encoding is invalid")
		}
		r := new(big.Int).SetBytes(signature[:32])
		s := new(big.Int).SetBytes(signature[32:])
		order := elliptic.P256().Params().N
		if r.Sign() <= 0 || s.Sign() <= 0 || r.Cmp(order) >= 0 || s.Cmp(order) >= 0 {
			return newSignatureError(signatureDetailEncoding, "signature encoding is invalid")
		}
	default:
		return newSignatureError(signatureDetailAlgorithm, "signature algorithm is unsupported")
	}
	return nil
}

func supportedSignatureAlgorithm(algorithm string) bool {
	return algorithm == SignatureAlgorithmEd25519V1 || algorithm == SignatureAlgorithmECDSAP256SHA256V1
}

func newSignatureError(detailCode string, message string) *Error {
	return &Error{Code: ErrInvalidSignature, DetailCode: detailCode, Message: message}
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
