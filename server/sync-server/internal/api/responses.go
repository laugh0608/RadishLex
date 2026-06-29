package api

import "github.com/laugh0608/RadishLex/server/sync-server/internal/storage"

type DomainResponse struct {
	DomainID        string `json:"domain_id"`
	CurrentKeyEpoch uint64 `json:"current_key_epoch"`
	ActiveKeyID     string `json:"active_key_id"`
	CreatedAtMs     int64  `json:"created_at_ms"`
	UpdatedAtMs     int64  `json:"updated_at_ms"`
}

type DomainStateResponse struct {
	Domain DomainResponse `json:"domain"`
}

type DeviceResponse struct {
	DomainID                string               `json:"domain_id"`
	DeviceID                string               `json:"device_id"`
	SigningPublicKeyID      string               `json:"signing_public_key_id"`
	SigningPublicKey        []byte               `json:"signing_public_key"`
	KeyAgreementPublicKeyID string               `json:"key_agreement_public_key_id"`
	KeyAgreementPublicKey   []byte               `json:"key_agreement_public_key"`
	Status                  storage.DeviceStatus `json:"status"`
	AuthorizedAtMs          int64                `json:"authorized_at_ms,omitempty"`
	RevokedAtMs             int64                `json:"revoked_at_ms,omitempty"`
	LastSeenAtMs            int64                `json:"last_seen_at_ms,omitempty"`
}

type JoinRequestResponse struct {
	DomainID                string               `json:"domain_id"`
	JoinRequestID           string               `json:"join_request_id"`
	DeviceID                string               `json:"device_id"`
	SigningPublicKeyID      string               `json:"signing_public_key_id"`
	SigningPublicKey        []byte               `json:"signing_public_key"`
	KeyAgreementPublicKeyID string               `json:"key_agreement_public_key_id"`
	KeyAgreementPublicKey   []byte               `json:"key_agreement_public_key"`
	Challenge               []byte               `json:"challenge"`
	CreatedAtMs             int64                `json:"created_at_ms"`
	ExpiresAtMs             int64                `json:"expires_at_ms"`
	Status                  storage.DeviceStatus `json:"status"`
}

type JoinRequestsResponse struct {
	JoinRequests []JoinRequestResponse `json:"join_requests"`
}

type RecoveryRecordResponse struct {
	DomainID               string                       `json:"domain_id"`
	RecoveryRecordID       string                       `json:"recovery_record_id"`
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
	Status                 storage.RecoveryRecordStatus `json:"status"`
	CreatedAtMs            int64                        `json:"created_at_ms"`
	RevokedAtMs            int64                        `json:"revoked_at_ms,omitempty"`
	SignerDeviceID         string                       `json:"signer_device_id"`
	SignatureSchemaVersion uint16                       `json:"signature_schema_version"`
	SignatureAlgorithm     string                       `json:"signature_algorithm"`
	SignatureKeyID         string                       `json:"signature_key_id"`
	Signature              []byte                       `json:"signature"`
	WrappedMaterial        []byte                       `json:"wrapped_material"`
}

type ObjectVersionResponse struct {
	DomainID               string `json:"domain_id"`
	ObjectID               string `json:"object_id"`
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
	ServerReceivedAtMs     int64  `json:"server_received_at_ms"`
	ClientCreatedAtMs      int64  `json:"client_created_at_ms"`
	ClientUpdatedAtMs      int64  `json:"client_updated_at_ms"`
}

func DomainResponseFrom(domain storage.Domain) DomainResponse {
	return DomainResponse{
		DomainID:        domain.DomainID,
		CurrentKeyEpoch: domain.CurrentKeyEpoch,
		ActiveKeyID:     domain.ActiveKeyID,
		CreatedAtMs:     domain.CreatedAtMs,
		UpdatedAtMs:     domain.UpdatedAtMs,
	}
}

func DeviceResponseFrom(device storage.Device) DeviceResponse {
	return DeviceResponse{
		DomainID:                device.DomainID,
		DeviceID:                device.DeviceID,
		SigningPublicKeyID:      device.SigningPublicKeyID,
		SigningPublicKey:        cloneBytes(device.SigningPublicKey),
		KeyAgreementPublicKeyID: device.KeyAgreementPublicKeyID,
		KeyAgreementPublicKey:   cloneBytes(device.KeyAgreementPublicKey),
		Status:                  device.Status,
		AuthorizedAtMs:          device.AuthorizedAtMs,
		RevokedAtMs:             device.RevokedAtMs,
		LastSeenAtMs:            device.LastSeenAtMs,
	}
}

func JoinRequestResponseFrom(request storage.JoinRequest) JoinRequestResponse {
	return JoinRequestResponse{
		DomainID:                request.DomainID,
		JoinRequestID:           request.JoinRequestID,
		DeviceID:                request.DeviceID,
		SigningPublicKeyID:      request.SigningPublicKeyID,
		SigningPublicKey:        cloneBytes(request.SigningPublicKey),
		KeyAgreementPublicKeyID: request.KeyAgreementPublicKeyID,
		KeyAgreementPublicKey:   cloneBytes(request.KeyAgreementPublicKey),
		Challenge:               cloneBytes(request.Challenge),
		CreatedAtMs:             request.CreatedAtMs,
		ExpiresAtMs:             request.ExpiresAtMs,
		Status:                  request.Status,
	}
}

func JoinRequestsResponseFrom(requests []storage.JoinRequest) JoinRequestsResponse {
	response := JoinRequestsResponse{
		JoinRequests: make([]JoinRequestResponse, 0, len(requests)),
	}
	for _, request := range requests {
		response.JoinRequests = append(response.JoinRequests, JoinRequestResponseFrom(request))
	}
	return response
}

func RecoveryRecordResponseFrom(record storage.RecoveryRecord, wrappedMaterial []byte) RecoveryRecordResponse {
	return RecoveryRecordResponse{
		DomainID:               record.DomainID,
		RecoveryRecordID:       record.RecoveryRecordID,
		KeyEpoch:               record.KeyEpoch,
		KDFProfile:             record.KDFProfile,
		KDFVersion:             record.KDFVersion,
		MemoryKiB:              record.MemoryKiB,
		Iterations:             record.Iterations,
		Parallelism:            record.Parallelism,
		OutputLen:              record.OutputLen,
		Salt:                   cloneBytes(record.Salt),
		Algorithm:              record.Algorithm,
		Nonce:                  cloneBytes(record.Nonce),
		WrappedMaterialLen:     record.WrappedMaterialLen,
		CiphertextHash:         record.CiphertextHash,
		Status:                 record.Status,
		CreatedAtMs:            record.CreatedAtMs,
		RevokedAtMs:            record.RevokedAtMs,
		SignerDeviceID:         record.SignerDeviceID,
		SignatureSchemaVersion: record.SignatureSchemaVersion,
		SignatureAlgorithm:     record.SignatureAlgorithm,
		SignatureKeyID:         record.SignatureKeyID,
		Signature:              cloneBytes(record.Signature),
		WrappedMaterial:        cloneBytes(wrappedMaterial),
	}
}

func ObjectVersionResponseFrom(version storage.ObjectVersion) ObjectVersionResponse {
	return ObjectVersionResponse{
		DomainID:               version.DomainID,
		ObjectID:               version.ObjectID,
		ObjectType:             version.ObjectType,
		Version:                version.Version,
		BaseVersion:            version.BaseVersion,
		OwnerDeviceID:          version.OwnerDeviceID,
		KeyID:                  version.KeyID,
		KeyEpoch:               version.KeyEpoch,
		Algorithm:              version.Algorithm,
		Nonce:                  cloneBytes(version.Nonce),
		EncryptedPayloadLen:    version.EncryptedPayloadLen,
		CiphertextHash:         version.CiphertextHash,
		SignatureSchemaVersion: version.SignatureSchemaVersion,
		SignatureAlgorithm:     version.SignatureAlgorithm,
		SignatureKeyID:         version.SignatureKeyID,
		Signature:              cloneBytes(version.Signature),
		ServerReceivedAtMs:     version.ServerReceivedAtMs,
		ClientCreatedAtMs:      version.ClientCreatedAtMs,
		ClientUpdatedAtMs:      version.ClientUpdatedAtMs,
	}
}

func cloneBytes(value []byte) []byte {
	if value == nil {
		return nil
	}
	out := make([]byte, len(value))
	copy(out, value)
	return out
}
