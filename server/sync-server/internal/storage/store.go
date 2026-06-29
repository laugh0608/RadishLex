package storage

import "context"

type Store interface {
	CreateDomain(ctx context.Context, domain Domain, firstDevice Device) error
	Domain(ctx context.Context, domainID string) (Domain, error)
	Device(ctx context.Context, domainID string, deviceID string) (Device, error)
	SaveJoinRequest(ctx context.Context, request JoinRequest) error
	AuthorizeJoinRequest(ctx context.Context, upload DeviceAuthorizationUpload) error
	DeviceWrappedKey(ctx context.Context, domainID string, recipientDeviceID string, keyEpoch uint64, wrappingKeyID string) (DeviceWrappingRecord, []byte, error)
	RevokeDevice(ctx context.Context, revocation DeviceRevocation) error
	PutRecoveryRecord(ctx context.Context, upload RecoveryRecordUpload) (RecoveryRecord, error)
	LatestRecoveryRecord(ctx context.Context, domainID string) (RecoveryRecord, error)
	LatestRecoveryWrappedMaterial(ctx context.Context, domainID string) (RecoveryRecord, []byte, error)
	PutObjectVersion(ctx context.Context, upload ObjectVersionUpload) (ObjectVersion, error)
	ObjectVersion(ctx context.Context, domainID string, objectID string, version uint64) (ObjectVersion, error)
	ObjectPayload(ctx context.Context, domainID string, objectID string, version uint64) ([]byte, error)
}
