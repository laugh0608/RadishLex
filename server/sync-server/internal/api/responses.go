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
	SigningAlgorithm        string               `json:"signing_algorithm"`
	SigningPublicKeyID      string               `json:"signing_public_key_id"`
	SigningPublicKey        []byte               `json:"signing_public_key"`
	KeyAgreementPublicKeyID string               `json:"key_agreement_public_key_id"`
	KeyAgreementPublicKey   []byte               `json:"key_agreement_public_key"`
	Status                  storage.DeviceStatus `json:"status"`
	AuthorizedAtMs          int64                `json:"authorized_at_ms,omitempty"`
	RevokedAtMs             int64                `json:"revoked_at_ms,omitempty"`
	LastSeenAtMs            int64                `json:"last_seen_at_ms,omitempty"`
}

type DeviceWrappedEpochResponse struct {
	SchemaVersion              uint16 `json:"schema_version"`
	Algorithm                  string `json:"algorithm"`
	DomainID                   string `json:"domain_id"`
	RecipientDeviceID          string `json:"recipient_device_id"`
	RecipientKeyAgreementKeyID string `json:"recipient_key_agreement_key_id"`
	DistributorDeviceID        string `json:"distributor_device_id"`
	WrappingKeyID              string `json:"wrapping_key_id"`
	KeyEpoch                   uint64 `json:"key_epoch"`
	Nonce                      []byte `json:"nonce"`
	WrappedKey                 []byte `json:"wrapped_key"`
	CiphertextHash             string `json:"ciphertext_hash"`
	CreatedAtMs                int64  `json:"created_at_ms"`
	SignatureRecordType        string `json:"signature_record_type"`
	SignatureSchemaVersion     uint16 `json:"signature_schema_version"`
	SignatureAlgorithm         string `json:"signature_algorithm"`
	SignatureKeyID             string `json:"signature_key_id"`
	Signature                  []byte `json:"signature"`
}

type EpochDistributionResponse struct {
	KeyEpoch        uint64 `json:"key_epoch"`
	AcceptedRecords int    `json:"accepted_records"`
	InsertedRecords int    `json:"inserted_records"`
}

func EpochDistributionResponseFrom(result storage.EpochDistributionResult) EpochDistributionResponse {
	return EpochDistributionResponse{
		KeyEpoch:        result.KeyEpoch,
		AcceptedRecords: result.AcceptedRecords,
		InsertedRecords: result.InsertedRecords,
	}
}

type JoinRequestResponse struct {
	DomainID                string               `json:"domain_id"`
	JoinRequestID           string               `json:"join_request_id"`
	DeviceID                string               `json:"device_id"`
	SigningAlgorithm        string               `json:"signing_algorithm"`
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
	ChangeSequence         uint64 `json:"change_sequence"`
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

type ObjectDiscoveryResponse struct {
	Entries    []ObjectVersionResponse `json:"entries"`
	NextCursor string                  `json:"next_cursor"`
	HasMore    bool                    `json:"has_more"`
}

type DeviceAuthorizationResponse struct {
	DomainID                    string `json:"domain_id"`
	JoinRequestID               string `json:"join_request_id"`
	AuthorizerDeviceID          string `json:"authorizer_device_id"`
	RecipientDeviceID           string `json:"recipient_device_id"`
	RecipientSigningPublicKeyID string `json:"recipient_signing_public_key_id"`
	RecipientKeyAgreementKeyID  string `json:"recipient_key_agreement_key_id"`
	JoinShortCode               string `json:"join_short_code"`
	JoinChallenge               []byte `json:"join_challenge"`
	JoinCreatedAtMs             int64  `json:"join_created_at_ms"`
	JoinExpiresAtMs             int64  `json:"join_expires_at_ms"`
	KeyEpoch                    uint64 `json:"key_epoch"`
	WrappingKeyID               string `json:"wrapping_key_id"`
	EncryptedKeyLen             int64  `json:"encrypted_key_len"`
	CreatedAtMs                 int64  `json:"created_at_ms"`
	SignatureSchemaVersion      uint16 `json:"signature_schema_version"`
	SignatureAlgorithm          string `json:"signature_algorithm"`
	SignatureKeyID              string `json:"signature_key_id"`
	Signature                   []byte `json:"signature"`
}

type DeviceRevocationResponse struct {
	DomainID               string `json:"domain_id"`
	RevokedDeviceID        string `json:"revoked_device_id"`
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

type LifecycleEventResponse struct {
	DomainID                       string                       `json:"domain_id"`
	LifecycleSequence              uint64                       `json:"lifecycle_sequence"`
	EventType                      storage.LifecycleEventType   `json:"event_type"`
	RecordID                       string                       `json:"record_id"`
	KeyEpoch                       uint64                       `json:"key_epoch"`
	RejectFromObjectChangeSequence uint64                       `json:"reject_from_object_change_sequence,omitempty"`
	CreatedAtMs                    int64                        `json:"created_at_ms"`
	Device                         *DeviceResponse              `json:"device,omitempty"`
	Authorization                  *DeviceAuthorizationResponse `json:"authorization,omitempty"`
	Revocation                     *DeviceRevocationResponse    `json:"revocation,omitempty"`
}

type LifecycleSnapshotResponse struct {
	Domain     DomainResponse           `json:"domain"`
	Entries    []LifecycleEventResponse `json:"entries"`
	NextCursor string                   `json:"next_cursor"`
}

type LifecycleDiscoveryResponse struct {
	Entries    []LifecycleEventResponse `json:"entries"`
	NextCursor string                   `json:"next_cursor"`
	HasMore    bool                     `json:"has_more"`
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
		SigningAlgorithm:        device.SigningAlgorithm,
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

func DeviceWrappedEpochResponseFrom(record storage.DeviceWrappingRecord, wrappedKey []byte) DeviceWrappedEpochResponse {
	return DeviceWrappedEpochResponse{
		SchemaVersion:              1,
		Algorithm:                  record.Algorithm,
		DomainID:                   record.DomainID,
		RecipientDeviceID:          record.RecipientDeviceID,
		RecipientKeyAgreementKeyID: record.RecipientKeyAgreementKeyID,
		DistributorDeviceID:        record.AuthorizerDeviceID,
		WrappingKeyID:              record.WrappingKeyID,
		KeyEpoch:                   record.KeyEpoch,
		Nonce:                      cloneBytes(record.Nonce),
		WrappedKey:                 cloneBytes(wrappedKey),
		CiphertextHash:             record.CiphertextHash,
		CreatedAtMs:                record.CreatedAtMs,
		SignatureRecordType:        record.SignatureRecordType,
		SignatureSchemaVersion:     record.SignatureSchemaVersion,
		SignatureAlgorithm:         record.SignatureAlgorithm,
		SignatureKeyID:             record.SignatureKeyID,
		Signature:                  cloneBytes(record.Signature),
	}
}

func LifecycleEventResponseFrom(event storage.LifecycleEvent) LifecycleEventResponse {
	response := LifecycleEventResponse{
		DomainID:                       event.DomainID,
		LifecycleSequence:              event.LifecycleSequence,
		EventType:                      event.EventType,
		RecordID:                       event.RecordID,
		KeyEpoch:                       event.KeyEpoch,
		RejectFromObjectChangeSequence: event.RejectFromObjectChangeSequence,
		CreatedAtMs:                    event.CreatedAtMs,
	}
	if event.Device != nil {
		device := DeviceResponseFrom(*event.Device)
		response.Device = &device
	}
	if event.Authorization != nil && event.JoinRequest != nil && event.Wrapping != nil {
		authorization := event.Authorization
		response.Authorization = &DeviceAuthorizationResponse{
			DomainID: authorization.DomainID, JoinRequestID: authorization.JoinRequestID,
			AuthorizerDeviceID: authorization.AuthorizerDeviceID, RecipientDeviceID: authorization.RecipientDeviceID,
			RecipientSigningPublicKeyID: authorization.RecipientSigningPublicKeyID,
			RecipientKeyAgreementKeyID:  authorization.RecipientKeyAgreementKeyID,
			JoinShortCode:               authorization.JoinShortCode,
			JoinChallenge:               cloneBytes(event.JoinRequest.Challenge),
			JoinCreatedAtMs:             event.JoinRequest.CreatedAtMs,
			JoinExpiresAtMs:             event.JoinRequest.ExpiresAtMs,
			KeyEpoch:                    authorization.KeyEpoch,
			WrappingKeyID:               event.Wrapping.WrappingKeyID,
			EncryptedKeyLen:             event.Wrapping.WrappedKeyLen,
			CreatedAtMs:                 authorization.CreatedAtMs, SignatureSchemaVersion: authorization.SignatureSchemaVersion,
			SignatureAlgorithm: authorization.SignatureAlgorithm, SignatureKeyID: authorization.SignatureKeyID,
			Signature: cloneBytes(authorization.Signature),
		}
	}
	if event.Revocation != nil {
		revocation := event.Revocation
		response.Revocation = &DeviceRevocationResponse{
			DomainID: revocation.DomainID, RevokedDeviceID: revocation.RevokedDeviceID,
			RevokerDeviceID: revocation.RevokerDeviceID, PreviousKeyEpoch: revocation.PreviousKeyEpoch,
			NewKeyEpoch: revocation.NewKeyEpoch, Reason: revocation.Reason,
			CreatedAtMs: revocation.CreatedAtMs, SignatureSchemaVersion: revocation.SignatureSchemaVersion,
			SignatureAlgorithm: revocation.SignatureAlgorithm, SignatureKeyID: revocation.SignatureKeyID,
			Signature: cloneBytes(revocation.Signature),
		}
	}
	return response
}

func LifecycleEventResponsesFrom(events []storage.LifecycleEvent) []LifecycleEventResponse {
	responses := make([]LifecycleEventResponse, 0, len(events))
	for _, event := range events {
		responses = append(responses, LifecycleEventResponseFrom(event))
	}
	return responses
}

func JoinRequestResponseFrom(request storage.JoinRequest) JoinRequestResponse {
	return JoinRequestResponse{
		DomainID:                request.DomainID,
		JoinRequestID:           request.JoinRequestID,
		DeviceID:                request.DeviceID,
		SigningAlgorithm:        request.SigningAlgorithm,
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
		ChangeSequence:         version.ChangeSequence,
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
