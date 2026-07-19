package api

import "github.com/laugh0608/RadishLex/server/sync-server/internal/storage"

type CreateDomainRequest struct {
	DomainID        string         `json:"domain_id"`
	CurrentKeyEpoch uint64         `json:"current_key_epoch"`
	ActiveKeyID     string         `json:"active_key_id"`
	FirstDevice     DeviceMetadata `json:"first_device"`
	CreatedAtMs     int64          `json:"created_at_ms"`
	UpdatedAtMs     int64          `json:"updated_at_ms"`
}

type DeviceMetadata struct {
	DeviceID                string `json:"device_id"`
	SigningAlgorithm        string `json:"signing_algorithm"`
	SigningPublicKeyID      string `json:"signing_public_key_id"`
	SigningPublicKey        []byte `json:"signing_public_key"`
	KeyAgreementPublicKeyID string `json:"key_agreement_public_key_id"`
	KeyAgreementPublicKey   []byte `json:"key_agreement_public_key"`
	Status                  string `json:"status"`
}

type CreateJoinRequestRequest struct {
	JoinRequestID           string `json:"join_request_id"`
	DeviceID                string `json:"device_id"`
	SigningAlgorithm        string `json:"signing_algorithm"`
	SigningPublicKeyID      string `json:"signing_public_key_id"`
	SigningPublicKey        []byte `json:"signing_public_key"`
	KeyAgreementPublicKeyID string `json:"key_agreement_public_key_id"`
	KeyAgreementPublicKey   []byte `json:"key_agreement_public_key"`
	Challenge               []byte `json:"challenge"`
	CreatedAtMs             int64  `json:"created_at_ms"`
	ExpiresAtMs             int64  `json:"expires_at_ms"`
}

type AuthorizeJoinRequestRequest struct {
	Authorization DeviceAuthorizationRequest `json:"authorization"`
	Wrapping      DeviceWrappingRequest      `json:"wrapping"`
	WrappedKey    []byte                     `json:"wrapped_key"`
}

type DeviceAuthorizationRequest struct {
	AuthorizerDeviceID          string `json:"authorizer_device_id"`
	RecipientDeviceID           string `json:"recipient_device_id"`
	RecipientSigningPublicKeyID string `json:"recipient_signing_public_key_id"`
	RecipientKeyAgreementKeyID  string `json:"recipient_key_agreement_key_id"`
	JoinShortCode               string `json:"join_short_code"`
	KeyEpoch                    uint64 `json:"key_epoch"`
	CreatedAtMs                 int64  `json:"created_at_ms"`
	SignatureSchemaVersion      uint16 `json:"signature_schema_version"`
	SignatureAlgorithm          string `json:"signature_algorithm"`
	SignatureKeyID              string `json:"signature_key_id"`
	Signature                   []byte `json:"signature"`
}

type DeviceWrappingRequest struct {
	AuthorizerDeviceID         string `json:"authorizer_device_id"`
	RecipientDeviceID          string `json:"recipient_device_id"`
	RecipientKeyAgreementKeyID string `json:"recipient_key_agreement_key_id"`
	KeyEpoch                   uint64 `json:"key_epoch"`
	WrappingKeyID              string `json:"wrapping_key_id"`
	Algorithm                  string `json:"algorithm"`
	Nonce                      []byte `json:"nonce"`
	WrappedKeyLen              int64  `json:"wrapped_key_len"`
	CiphertextHash             string `json:"ciphertext_hash"`
	CreatedAtMs                int64  `json:"created_at_ms"`
	Signature                  []byte `json:"signature"`
}

type EpochDistributionRequest struct {
	DistributorDeviceID string                           `json:"distributor_device_id"`
	KeyEpoch            uint64                           `json:"key_epoch"`
	Records             []EpochDistributionRecordRequest `json:"records"`
}

type EpochDistributionRecordRequest struct {
	RecipientDeviceID          string `json:"recipient_device_id"`
	RecipientKeyAgreementKeyID string `json:"recipient_key_agreement_key_id"`
	WrappingKeyID              string `json:"wrapping_key_id"`
	Algorithm                  string `json:"algorithm"`
	Nonce                      []byte `json:"nonce"`
	WrappedKeyLen              int64  `json:"wrapped_key_len"`
	CiphertextHash             string `json:"ciphertext_hash"`
	CreatedAtMs                int64  `json:"created_at_ms"`
	SignatureSchemaVersion     uint16 `json:"signature_schema_version"`
	SignatureAlgorithm         string `json:"signature_algorithm"`
	SignatureKeyID             string `json:"signature_key_id"`
	Signature                  []byte `json:"signature"`
	WrappedKey                 []byte `json:"wrapped_key"`
}

type DeviceRevocationRequest struct {
	RevokerDeviceID        string `json:"revoker_device_id"`
	PreviousKeyEpoch       uint64 `json:"previous_key_epoch"`
	NewKeyEpoch            uint64 `json:"new_key_epoch"`
	Reason                 string `json:"reason"`
	CreatedAtMs            int64  `json:"created_at_ms"`
	SignatureSchemaVersion uint16 `json:"signature_schema_version"`
	SignatureAlgorithm     string `json:"signature_algorithm"`
	SignatureKeyID         string `json:"signature_key_id"`
	Signature              []byte `json:"signature"`
}

type RecoveryRecordUploadRequest struct {
	RecordSchemaVersion    uint16                       `json:"record_schema_version"`
	RecoveryRecordID       string                       `json:"recovery_record_id"`
	PreviousRecoveryID     string                       `json:"previous_recovery_id"`
	KeyEpoch               uint64                       `json:"key_epoch"`
	KDFProfile             string                       `json:"kdf_profile"`
	KDFVersion             uint16                       `json:"kdf_version"`
	MemoryKiB              uint32                       `json:"memory_kib"`
	Iterations             uint32                       `json:"iterations"`
	Parallelism            uint32                       `json:"parallelism"`
	OutputLen              int64                        `json:"output_len"`
	Salt                   []byte                       `json:"salt"`
	Algorithm              string                       `json:"algorithm"`
	Nonce                  []byte                       `json:"nonce"`
	WrappedMaterialLen     int64                        `json:"wrapped_material_len"`
	CiphertextHash         string                       `json:"ciphertext_hash"`
	ActivationAlgorithm    string                       `json:"activation_algorithm"`
	ActivationPublicKeyID  string                       `json:"activation_public_key_id"`
	ActivationPublicKey    []byte                       `json:"activation_public_key"`
	Status                 storage.RecoveryRecordStatus `json:"status"`
	CreatedAtMs            int64                        `json:"created_at_ms"`
	UpdatedAtMs            int64                        `json:"updated_at_ms"`
	SignerDeviceID         string                       `json:"signer_device_id"`
	SignatureSchemaVersion uint16                       `json:"signature_schema_version"`
	SignatureAlgorithm     string                       `json:"signature_algorithm"`
	SignatureKeyID         string                       `json:"signature_key_id"`
	Signature              []byte                       `json:"signature"`
	WrappedMaterial        []byte                       `json:"wrapped_material"`
}

func (r RecoveryRecordUploadRequest) Upload(domainID string) storage.RecoveryRecordUpload {
	return storage.RecoveryRecordUpload{
		Record: storage.RecoveryRecord{
			RecordSchemaVersion: r.RecordSchemaVersion, DomainID: domainID,
			RecoveryRecordID: r.RecoveryRecordID, PreviousRecoveryID: r.PreviousRecoveryID,
			KeyEpoch: r.KeyEpoch, KDFProfile: r.KDFProfile, KDFVersion: r.KDFVersion,
			MemoryKiB: r.MemoryKiB, Iterations: r.Iterations, Parallelism: r.Parallelism,
			OutputLen: r.OutputLen, Salt: cloneBytes(r.Salt), Algorithm: r.Algorithm,
			Nonce: cloneBytes(r.Nonce), WrappedMaterialLen: r.WrappedMaterialLen,
			CiphertextHash: r.CiphertextHash, ActivationAlgorithm: r.ActivationAlgorithm,
			ActivationPublicKeyID: r.ActivationPublicKeyID, ActivationPublicKey: cloneBytes(r.ActivationPublicKey),
			Status: r.Status, CreatedAtMs: r.CreatedAtMs, UpdatedAtMs: r.UpdatedAtMs,
			SignerDeviceID: r.SignerDeviceID, SignatureSchemaVersion: r.SignatureSchemaVersion,
			SignatureAlgorithm: r.SignatureAlgorithm, SignatureKeyID: r.SignatureKeyID,
			Signature: cloneBytes(r.Signature),
		},
		WrappedMaterial: cloneBytes(r.WrappedMaterial),
	}
}

type ObjectVersionUploadRequest struct {
	ObjectType             string `json:"object_type"`
	Version                uint64 `json:"version"`
	BaseVersion            uint64 `json:"base_version"`
	OwnerDeviceID          string `json:"owner_device_id"`
	KeyID                  string `json:"key_id"`
	KeyEpoch               uint64 `json:"key_epoch"`
	Algorithm              string `json:"algorithm"`
	Nonce                  []byte `json:"nonce"`
	EncryptedPayloadLen    int64  `json:"encrypted_payload_len"`
	CiphertextHash         string `json:"ciphertext_hash"`
	SignatureSchemaVersion uint16 `json:"signature_schema_version"`
	SignatureAlgorithm     string `json:"signature_algorithm"`
	SignatureKeyID         string `json:"signature_key_id"`
	Signature              []byte `json:"signature"`
	ClientCreatedAtMs      int64  `json:"client_created_at_ms"`
	ClientUpdatedAtMs      int64  `json:"client_updated_at_ms"`
	Payload                []byte `json:"payload"`
}

func (r AuthorizeJoinRequestRequest) Upload(domainID string, joinRequestID string) storage.DeviceAuthorizationUpload {
	return storage.DeviceAuthorizationUpload{
		Authorization: storage.DeviceAuthorization{
			DomainID:                    domainID,
			JoinRequestID:               joinRequestID,
			AuthorizerDeviceID:          r.Authorization.AuthorizerDeviceID,
			RecipientDeviceID:           r.Authorization.RecipientDeviceID,
			RecipientSigningPublicKeyID: r.Authorization.RecipientSigningPublicKeyID,
			RecipientKeyAgreementKeyID:  r.Authorization.RecipientKeyAgreementKeyID,
			JoinShortCode:               r.Authorization.JoinShortCode,
			KeyEpoch:                    r.Authorization.KeyEpoch,
			CreatedAtMs:                 r.Authorization.CreatedAtMs,
			SignatureSchemaVersion:      r.Authorization.SignatureSchemaVersion,
			SignatureAlgorithm:          r.Authorization.SignatureAlgorithm,
			SignatureKeyID:              r.Authorization.SignatureKeyID,
			Signature:                   r.Authorization.Signature,
		},
		Wrapping: storage.DeviceWrappingRecord{
			DomainID:                   domainID,
			RecipientDeviceID:          r.Wrapping.RecipientDeviceID,
			RecipientKeyAgreementKeyID: r.Wrapping.RecipientKeyAgreementKeyID,
			AuthorizerDeviceID:         r.Wrapping.AuthorizerDeviceID,
			KeyEpoch:                   r.Wrapping.KeyEpoch,
			WrappingKeyID:              r.Wrapping.WrappingKeyID,
			Algorithm:                  r.Wrapping.Algorithm,
			Nonce:                      r.Wrapping.Nonce,
			WrappedKeyLen:              r.Wrapping.WrappedKeyLen,
			CiphertextHash:             r.Wrapping.CiphertextHash,
			CreatedAtMs:                r.Wrapping.CreatedAtMs,
			Signature:                  r.Wrapping.Signature,
		},
		WrappedKey: r.WrappedKey,
	}
}

func (r EpochDistributionRequest) Upload(domainID string) storage.EpochDistributionUpload {
	records := make([]storage.DeviceWrappingUpload, 0, len(r.Records))
	for _, item := range r.Records {
		records = append(records, storage.DeviceWrappingUpload{
			Record: storage.DeviceWrappingRecord{
				DomainID:                   domainID,
				RecipientDeviceID:          item.RecipientDeviceID,
				RecipientKeyAgreementKeyID: item.RecipientKeyAgreementKeyID,
				AuthorizerDeviceID:         r.DistributorDeviceID,
				KeyEpoch:                   r.KeyEpoch,
				WrappingKeyID:              item.WrappingKeyID,
				Algorithm:                  item.Algorithm,
				Nonce:                      item.Nonce,
				WrappedKeyLen:              item.WrappedKeyLen,
				CiphertextHash:             item.CiphertextHash,
				CreatedAtMs:                item.CreatedAtMs,
				SignatureRecordType:        storage.WrappingSignatureEpochDistribution,
				SignatureSchemaVersion:     item.SignatureSchemaVersion,
				SignatureAlgorithm:         item.SignatureAlgorithm,
				SignatureKeyID:             item.SignatureKeyID,
				Signature:                  item.Signature,
			},
			WrappedKey: item.WrappedKey,
		})
	}
	return storage.EpochDistributionUpload{
		DomainID:            domainID,
		DistributorDeviceID: r.DistributorDeviceID,
		KeyEpoch:            r.KeyEpoch,
		Records:             records,
	}
}

func (r CreateJoinRequestRequest) JoinRequest(domainID string) storage.JoinRequest {
	return storage.JoinRequest{
		DomainID:                domainID,
		JoinRequestID:           r.JoinRequestID,
		DeviceID:                r.DeviceID,
		SigningAlgorithm:        r.SigningAlgorithm,
		SigningPublicKeyID:      r.SigningPublicKeyID,
		SigningPublicKey:        r.SigningPublicKey,
		KeyAgreementPublicKeyID: r.KeyAgreementPublicKeyID,
		KeyAgreementPublicKey:   r.KeyAgreementPublicKey,
		Challenge:               r.Challenge,
		CreatedAtMs:             r.CreatedAtMs,
		ExpiresAtMs:             r.ExpiresAtMs,
		Status:                  storage.DevicePending,
	}
}

func (r DeviceRevocationRequest) Revocation(domainID string, revokedDeviceID string) storage.DeviceRevocation {
	return storage.DeviceRevocation{
		DomainID: domainID, RevokedDeviceID: revokedDeviceID, RevokerDeviceID: r.RevokerDeviceID,
		PreviousKeyEpoch: r.PreviousKeyEpoch, NewKeyEpoch: r.NewKeyEpoch,
		Reason: r.Reason, CreatedAtMs: r.CreatedAtMs,
		SignatureSchemaVersion: r.SignatureSchemaVersion,
		SignatureAlgorithm:     r.SignatureAlgorithm, SignatureKeyID: r.SignatureKeyID,
		Signature: r.Signature,
	}
}

func (r CreateDomainRequest) Domain() storage.Domain {
	return storage.Domain{
		DomainID:        r.DomainID,
		CurrentKeyEpoch: r.CurrentKeyEpoch,
		ActiveKeyID:     r.ActiveKeyID,
		CreatedAtMs:     r.CreatedAtMs,
		UpdatedAtMs:     r.UpdatedAtMs,
	}
}

func (r CreateDomainRequest) Device() storage.Device {
	return storage.Device{
		DomainID:                r.DomainID,
		DeviceID:                r.FirstDevice.DeviceID,
		SigningAlgorithm:        r.FirstDevice.SigningAlgorithm,
		SigningPublicKeyID:      r.FirstDevice.SigningPublicKeyID,
		SigningPublicKey:        r.FirstDevice.SigningPublicKey,
		KeyAgreementPublicKeyID: r.FirstDevice.KeyAgreementPublicKeyID,
		KeyAgreementPublicKey:   r.FirstDevice.KeyAgreementPublicKey,
		Status:                  storage.DeviceStatus(r.FirstDevice.Status),
		AuthorizedAtMs:          r.CreatedAtMs,
	}
}

func (r ObjectVersionUploadRequest) StorageVersion(domainID string, objectID string) storage.ObjectVersion {
	return storage.ObjectVersion{
		DomainID:               domainID,
		ObjectID:               objectID,
		ObjectType:             r.ObjectType,
		Version:                r.Version,
		BaseVersion:            r.BaseVersion,
		OwnerDeviceID:          r.OwnerDeviceID,
		KeyID:                  r.KeyID,
		KeyEpoch:               r.KeyEpoch,
		Algorithm:              r.Algorithm,
		Nonce:                  r.Nonce,
		EncryptedPayloadLen:    r.EncryptedPayloadLen,
		CiphertextHash:         r.CiphertextHash,
		SignatureSchemaVersion: r.SignatureSchemaVersion,
		SignatureAlgorithm:     r.SignatureAlgorithm,
		SignatureKeyID:         r.SignatureKeyID,
		Signature:              r.Signature,
		ClientCreatedAtMs:      r.ClientCreatedAtMs,
		ClientUpdatedAtMs:      r.ClientUpdatedAtMs,
	}
}

func (r ObjectVersionUploadRequest) Upload(domainID string, objectID string) storage.ObjectVersionUpload {
	return storage.ObjectVersionUpload{
		Version: r.StorageVersion(domainID, objectID),
		Payload: cloneBytes(r.Payload),
	}
}
