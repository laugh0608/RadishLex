package api

import "github.com/laugh0608/RadishLex/server/sync-server/internal/storage"

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

func cloneBytes(value []byte) []byte {
	if value == nil {
		return nil
	}
	out := make([]byte, len(value))
	copy(out, value)
	return out
}
