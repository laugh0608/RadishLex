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
	SigningPublicKeyID      string `json:"signing_public_key_id"`
	SigningPublicKey        []byte `json:"signing_public_key"`
	KeyAgreementPublicKeyID string `json:"key_agreement_public_key_id"`
	KeyAgreementPublicKey   []byte `json:"key_agreement_public_key"`
	Status                  string `json:"status"`
}

type CreateJoinRequestRequest struct {
	JoinRequestID           string `json:"join_request_id"`
	DeviceID                string `json:"device_id"`
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
	AuthorizerDeviceID string `json:"authorizer_device_id"`
	RecipientDeviceID  string `json:"recipient_device_id"`
	KeyEpoch           uint64 `json:"key_epoch"`
	WrappingKeyID      string `json:"wrapping_key_id"`
	Algorithm          string `json:"algorithm"`
	Nonce              []byte `json:"nonce"`
	WrappedKeyLen      int64  `json:"wrapped_key_len"`
	CiphertextHash     string `json:"ciphertext_hash"`
	CreatedAtMs        int64  `json:"created_at_ms"`
	Signature          []byte `json:"signature"`
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
			DomainID:           domainID,
			RecipientDeviceID:  r.Wrapping.RecipientDeviceID,
			AuthorizerDeviceID: r.Wrapping.AuthorizerDeviceID,
			KeyEpoch:           r.Wrapping.KeyEpoch,
			WrappingKeyID:      r.Wrapping.WrappingKeyID,
			Algorithm:          r.Wrapping.Algorithm,
			Nonce:              r.Wrapping.Nonce,
			WrappedKeyLen:      r.Wrapping.WrappedKeyLen,
			CiphertextHash:     r.Wrapping.CiphertextHash,
			CreatedAtMs:        r.Wrapping.CreatedAtMs,
			Signature:          r.Wrapping.Signature,
		},
		WrappedKey: r.WrappedKey,
	}
}

func (r CreateJoinRequestRequest) JoinRequest(domainID string) storage.JoinRequest {
	return storage.JoinRequest{
		DomainID:                domainID,
		JoinRequestID:           r.JoinRequestID,
		DeviceID:                r.DeviceID,
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
