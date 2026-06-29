package storage

import (
	"bytes"
	"context"
	"encoding/base64"
	"fmt"
	"regexp"
	"sort"
	"sync"
)

var opaqueIDPattern = regexp.MustCompile(`^[A-Za-z0-9._:-]+$`)

type MemoryStore struct {
	mu sync.Mutex

	domains        map[string]Domain
	devices        map[domainDeviceKey]Device
	joinRequests   map[joinRequestKey]JoinRequest
	authorizations map[joinRequestKey]DeviceAuthorization
	wrapping       map[wrappingKey]DeviceWrappingRecord
	revocations    map[revocationKey]DeviceRevocation
	recoveries     map[recoveryKey]RecoveryRecord
	latestRecovery map[string]string
	objects        map[objectKey]SyncObject
	versions       map[objectVersionKey]ObjectVersion
	blobs          map[string][]byte
	auditEvents    []AuditEvent
}

func NewMemoryStore() *MemoryStore {
	return &MemoryStore{
		domains:        make(map[string]Domain),
		devices:        make(map[domainDeviceKey]Device),
		joinRequests:   make(map[joinRequestKey]JoinRequest),
		authorizations: make(map[joinRequestKey]DeviceAuthorization),
		wrapping:       make(map[wrappingKey]DeviceWrappingRecord),
		revocations:    make(map[revocationKey]DeviceRevocation),
		recoveries:     make(map[recoveryKey]RecoveryRecord),
		latestRecovery: make(map[string]string),
		objects:        make(map[objectKey]SyncObject),
		versions:       make(map[objectVersionKey]ObjectVersion),
		blobs:          make(map[string][]byte),
	}
}

func (s *MemoryStore) RecordAuditEvent(ctx context.Context, event AuditEvent) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	if err := validateAuditEvent(event); err != nil {
		return err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	s.auditEvents = append(s.auditEvents, event)
	return nil
}

func (s *MemoryStore) CreateDomain(ctx context.Context, domain Domain, firstDevice Device) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	if err := validateDomain(domain); err != nil {
		return err
	}
	if firstDevice.DomainID != domain.DomainID {
		return newError(ErrInvalidRequest, "first device domain must match domain")
	}
	if firstDevice.Status != DeviceActive {
		return newError(ErrInvalidRequest, "first device must be active")
	}
	if err := validateDevice(firstDevice); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	if _, exists := s.domains[domain.DomainID]; exists {
		return newError(ErrInvalidRequest, "domain already exists")
	}
	s.domains[domain.DomainID] = domain
	s.devices[deviceKey(firstDevice.DomainID, firstDevice.DeviceID)] = cloneDevice(firstDevice)
	return nil
}

func (s *MemoryStore) Domain(ctx context.Context, domainID string) (Domain, error) {
	if err := checkContext(ctx); err != nil {
		return Domain{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	domain, ok := s.domains[domainID]
	if !ok {
		return Domain{}, newError(ErrNotFound, "domain not found")
	}
	return domain, nil
}

func (s *MemoryStore) Device(ctx context.Context, domainID string, deviceID string) (Device, error) {
	if err := checkContext(ctx); err != nil {
		return Device{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	device, ok := s.devices[deviceKey(domainID, deviceID)]
	if !ok {
		return Device{}, newError(ErrNotFound, "device not found")
	}
	return cloneDevice(device), nil
}

func (s *MemoryStore) SaveJoinRequest(ctx context.Context, request JoinRequest) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	if request.Status == "" {
		request.Status = DevicePending
	}
	if err := validateJoinRequest(request); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	if _, ok := s.domains[request.DomainID]; !ok {
		return newError(ErrNotFound, "domain not found")
	}
	key := joinKey(request.DomainID, request.JoinRequestID)
	if _, exists := s.joinRequests[key]; exists {
		return newError(ErrInvalidRequest, "join request already exists")
	}
	s.joinRequests[key] = cloneJoinRequest(request)
	s.devices[deviceKey(request.DomainID, request.DeviceID)] = Device{
		DomainID:                request.DomainID,
		DeviceID:                request.DeviceID,
		SigningPublicKeyID:      request.SigningPublicKeyID,
		SigningPublicKey:        cloneBytes(request.SigningPublicKey),
		KeyAgreementPublicKeyID: request.KeyAgreementPublicKeyID,
		KeyAgreementPublicKey:   cloneBytes(request.KeyAgreementPublicKey),
		Status:                  DevicePending,
	}
	return nil
}

func (s *MemoryStore) PendingJoinRequests(ctx context.Context, domainID string) ([]JoinRequest, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()

	if _, ok := s.domains[domainID]; !ok {
		return nil, newError(ErrNotFound, "domain not found")
	}
	var requests []JoinRequest
	for key, request := range s.joinRequests {
		if key.domainID == domainID && request.Status == DevicePending {
			requests = append(requests, cloneJoinRequest(request))
		}
	}
	sort.Slice(requests, func(i int, j int) bool {
		if requests[i].CreatedAtMs == requests[j].CreatedAtMs {
			return requests[i].JoinRequestID < requests[j].JoinRequestID
		}
		return requests[i].CreatedAtMs < requests[j].CreatedAtMs
	})
	return requests, nil
}

func (s *MemoryStore) AuthorizeJoinRequest(ctx context.Context, upload DeviceAuthorizationUpload) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	authorization := upload.Authorization
	wrapping := upload.Wrapping
	if err := validateAuthorization(authorization); err != nil {
		return err
	}
	if err := validateAuthorizationUpload(upload); err != nil {
		return err
	}
	if wrapping.DomainID != authorization.DomainID ||
		wrapping.AuthorizerDeviceID != authorization.AuthorizerDeviceID ||
		wrapping.RecipientDeviceID != authorization.RecipientDeviceID ||
		wrapping.KeyEpoch != authorization.KeyEpoch {
		return newError(ErrInvalidRequest, "wrapping record must match authorization")
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	authorizer, err := s.activeDeviceLocked(authorization.DomainID, authorization.AuthorizerDeviceID)
	if err != nil {
		return err
	}
	join, ok := s.joinRequests[joinKey(authorization.DomainID, authorization.JoinRequestID)]
	if !ok {
		return newError(ErrNotFound, "join request not found")
	}
	if join.Status != DevicePending {
		return newError(ErrForbiddenDevice, "join request is not pending")
	}
	if authorization.CreatedAtMs > join.ExpiresAtMs {
		return newError(ErrForbiddenDevice, "join request expired")
	}
	if authorization.RecipientDeviceID != join.DeviceID ||
		authorization.RecipientSigningPublicKeyID != join.SigningPublicKeyID ||
		authorization.RecipientKeyAgreementKeyID != join.KeyAgreementPublicKeyID {
		return newError(ErrForbiddenDevice, "recipient public key does not match join request")
	}
	if err := verifyAuthorizationSignature(authorization, wrapping, join, authorizer); err != nil {
		return err
	}

	wrapping.BlobRef = wrappingBlobRef(wrapping)
	join.Status = DeviceActive
	s.joinRequests[joinKey(authorization.DomainID, authorization.JoinRequestID)] = join
	s.authorizations[joinKey(authorization.DomainID, authorization.JoinRequestID)] = cloneAuthorization(authorization)
	s.wrapping[wrappingRecordKey(wrapping)] = cloneWrappingRecord(wrapping)
	s.blobs[wrapping.BlobRef] = cloneBytes(upload.WrappedKey)
	s.devices[deviceKey(join.DomainID, join.DeviceID)] = Device{
		DomainID:                join.DomainID,
		DeviceID:                join.DeviceID,
		SigningPublicKeyID:      join.SigningPublicKeyID,
		SigningPublicKey:        cloneBytes(join.SigningPublicKey),
		KeyAgreementPublicKeyID: join.KeyAgreementPublicKeyID,
		KeyAgreementPublicKey:   cloneBytes(join.KeyAgreementPublicKey),
		Status:                  DeviceActive,
		AuthorizedAtMs:          authorization.CreatedAtMs,
	}
	return nil
}

func (s *MemoryStore) DeviceWrappedKey(ctx context.Context, domainID string, recipientDeviceID string, keyEpoch uint64, wrappingKeyID string) (DeviceWrappingRecord, []byte, error) {
	if err := checkContext(ctx); err != nil {
		return DeviceWrappingRecord{}, nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	record, ok := s.wrapping[wrappingKey{
		domainID:          domainID,
		recipientDeviceID: recipientDeviceID,
		keyEpoch:          keyEpoch,
		wrappingKeyID:     wrappingKeyID,
	}]
	if !ok {
		return DeviceWrappingRecord{}, nil, newError(ErrNotFound, "device wrapping record not found")
	}
	wrappedKey, ok := s.blobs[record.BlobRef]
	if !ok {
		return DeviceWrappingRecord{}, nil, newError(ErrStorageUnavailable, "device wrapped key is missing")
	}
	if int64(len(wrappedKey)) != record.WrappedKeyLen || CiphertextHash(wrappedKey) != record.CiphertextHash {
		return DeviceWrappingRecord{}, nil, newError(ErrStorageUnavailable, "device wrapped key metadata mismatch")
	}
	return cloneWrappingRecord(record), cloneBytes(wrappedKey), nil
}

func (s *MemoryStore) RevokeDevice(ctx context.Context, revocation DeviceRevocation) error {
	if err := checkContext(ctx); err != nil {
		return err
	}
	if err := validateRevocation(revocation); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	domain, ok := s.domains[revocation.DomainID]
	if !ok {
		return newError(ErrNotFound, "domain not found")
	}
	revoker, err := s.activeDeviceLocked(revocation.DomainID, revocation.RevokerDeviceID)
	if err != nil {
		return err
	}
	targetKey := deviceKey(revocation.DomainID, revocation.RevokedDeviceID)
	target, ok := s.devices[targetKey]
	if !ok {
		return newError(ErrNotFound, "revoked device not found")
	}
	if target.Status != DeviceActive {
		return newError(ErrForbiddenDevice, "revoked device is not active")
	}
	if revocation.PreviousKeyEpoch != domain.CurrentKeyEpoch {
		return newError(ErrInvalidRequest, "previous key epoch must match domain")
	}
	if revocation.NewKeyEpoch <= domain.CurrentKeyEpoch {
		return newError(ErrInvalidRequest, "new key epoch must advance domain")
	}
	if err := verifyRevocationSignature(revocation, revoker); err != nil {
		return err
	}

	target.Status = DeviceRevoked
	target.RevokedAtMs = revocation.CreatedAtMs
	s.devices[targetKey] = target
	domain.CurrentKeyEpoch = revocation.NewKeyEpoch
	domain.UpdatedAtMs = revocation.CreatedAtMs
	s.domains[domain.DomainID] = domain
	s.revocations[revocationKeyFor(revocation)] = cloneRevocation(revocation)
	return nil
}

func (s *MemoryStore) PutRecoveryRecord(ctx context.Context, upload RecoveryRecordUpload) (RecoveryRecord, error) {
	if err := checkContext(ctx); err != nil {
		return RecoveryRecord{}, err
	}
	if err := validateRecoveryRecordUpload(upload); err != nil {
		return RecoveryRecord{}, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	signer, err := s.activeDeviceLocked(upload.Record.DomainID, upload.Record.SignerDeviceID)
	if err != nil {
		return RecoveryRecord{}, err
	}
	if err := verifyRecoverySignature(upload.Record, signer); err != nil {
		return RecoveryRecord{}, err
	}
	record := cloneRecoveryRecord(upload.Record)
	record.BlobRef = recoveryBlobRef(record)
	s.recoveries[recoveryKey{domainID: record.DomainID, recoveryRecordID: record.RecoveryRecordID}] = record
	if record.Status == RecoveryRecordActive {
		s.latestRecovery[record.DomainID] = record.RecoveryRecordID
	}
	s.blobs[record.BlobRef] = cloneBytes(upload.WrappedMaterial)
	return record, nil
}

func (s *MemoryStore) LatestRecoveryRecord(ctx context.Context, domainID string) (RecoveryRecord, error) {
	if err := checkContext(ctx); err != nil {
		return RecoveryRecord{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	recoveryID, ok := s.latestRecovery[domainID]
	if !ok {
		return RecoveryRecord{}, newError(ErrNotFound, "active recovery record not found")
	}
	record, ok := s.recoveries[recoveryKey{domainID: domainID, recoveryRecordID: recoveryID}]
	if !ok {
		return RecoveryRecord{}, newError(ErrStorageUnavailable, "latest recovery record metadata missing")
	}
	return cloneRecoveryRecord(record), nil
}

func (s *MemoryStore) LatestRecoveryWrappedMaterial(ctx context.Context, domainID string) (RecoveryRecord, []byte, error) {
	if err := checkContext(ctx); err != nil {
		return RecoveryRecord{}, nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	recoveryID, ok := s.latestRecovery[domainID]
	if !ok {
		return RecoveryRecord{}, nil, newError(ErrNotFound, "active recovery record not found")
	}
	record, ok := s.recoveries[recoveryKey{domainID: domainID, recoveryRecordID: recoveryID}]
	if !ok {
		return RecoveryRecord{}, nil, newError(ErrStorageUnavailable, "latest recovery record metadata missing")
	}
	wrappedMaterial, ok := s.blobs[record.BlobRef]
	if !ok {
		return RecoveryRecord{}, nil, newError(ErrStorageUnavailable, "recovery wrapped material is missing")
	}
	if int64(len(wrappedMaterial)) != record.WrappedMaterialLen || CiphertextHash(wrappedMaterial) != record.CiphertextHash {
		return RecoveryRecord{}, nil, newError(ErrStorageUnavailable, "recovery wrapped material metadata mismatch")
	}
	return cloneRecoveryRecord(record), cloneBytes(wrappedMaterial), nil
}

func (s *MemoryStore) PutObjectVersion(ctx context.Context, upload ObjectVersionUpload) (ObjectVersion, error) {
	if err := checkContext(ctx); err != nil {
		return ObjectVersion{}, err
	}
	if err := validateObjectUpload(upload); err != nil {
		return ObjectVersion{}, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	domain, ok := s.domains[upload.Version.DomainID]
	if !ok {
		return ObjectVersion{}, newError(ErrNotFound, "domain not found")
	}
	signer, err := s.activeDeviceLocked(upload.Version.DomainID, upload.Version.OwnerDeviceID)
	if err != nil {
		return ObjectVersion{}, err
	}
	if upload.Version.KeyEpoch < domain.CurrentKeyEpoch {
		return ObjectVersion{}, newError(ErrForbiddenDevice, "object key epoch is older than domain")
	}
	if err := verifyObjectSignature(upload.Version, signer); err != nil {
		return ObjectVersion{}, err
	}

	versionKey := objectVersionKeyFor(upload.Version.DomainID, upload.Version.ObjectID, upload.Version.Version)
	if existing, exists := s.versions[versionKey]; exists {
		if existing.CiphertextHash == upload.Version.CiphertextHash {
			return cloneObjectVersion(existing), nil
		}
		return ObjectVersion{}, newError(ErrConflictObjectVersion, "object version exists with different ciphertext hash")
	}

	objKey := objectKey{domainID: upload.Version.DomainID, objectID: upload.Version.ObjectID}
	object, exists := s.objects[objKey]
	if !exists {
		if upload.Version.Version != 1 || upload.Version.BaseVersion != 0 {
			return ObjectVersion{}, newError(ErrInvalidRequest, "new object must start at version 1 with base version 0")
		}
		object = SyncObject{
			DomainID:    upload.Version.DomainID,
			ObjectID:    upload.Version.ObjectID,
			ObjectType:  upload.Version.ObjectType,
			CreatedAtMs: upload.Version.ClientCreatedAtMs,
		}
	} else {
		if upload.Version.ObjectType != object.ObjectType {
			return ObjectVersion{}, newError(ErrInvalidRequest, "object type cannot change")
		}
		if upload.Version.BaseVersion < object.LatestVersion {
			return ObjectVersion{}, conflictStaleBaseVersion(object.LatestVersion, object.LatestCiphertextHash)
		}
		if upload.Version.BaseVersion > object.LatestVersion {
			return ObjectVersion{}, newError(ErrInvalidRequest, "base version cannot exceed latest version")
		}
		if upload.Version.Version != object.LatestVersion+1 {
			return ObjectVersion{}, newError(ErrInvalidRequest, "object version must advance by one")
		}
	}

	version := cloneObjectVersion(upload.Version)
	version.BlobRef = objectBlobRef(version)
	object.LatestVersion = version.Version
	object.LatestCiphertextHash = version.CiphertextHash
	object.LatestKeyEpoch = version.KeyEpoch
	object.UpdatedAtMs = version.ClientUpdatedAtMs
	s.objects[objKey] = object
	s.versions[versionKey] = version
	s.blobs[version.BlobRef] = cloneBytes(upload.Payload)
	return cloneObjectVersion(version), nil
}

func (s *MemoryStore) ObjectVersion(ctx context.Context, domainID string, objectID string, version uint64) (ObjectVersion, error) {
	if err := checkContext(ctx); err != nil {
		return ObjectVersion{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	metadata, ok := s.versions[objectVersionKeyFor(domainID, objectID, version)]
	if !ok {
		return ObjectVersion{}, newError(ErrNotFound, "object version not found")
	}
	return cloneObjectVersion(metadata), nil
}

func (s *MemoryStore) ObjectPayload(ctx context.Context, domainID string, objectID string, version uint64) ([]byte, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	metadata, ok := s.versions[objectVersionKeyFor(domainID, objectID, version)]
	if !ok {
		return nil, newError(ErrNotFound, "object version not found")
	}
	payload, ok := s.blobs[metadata.BlobRef]
	if !ok {
		return nil, newError(ErrStorageUnavailable, "object payload is missing")
	}
	if int64(len(payload)) != metadata.EncryptedPayloadLen || CiphertextHash(payload) != metadata.CiphertextHash {
		return nil, newError(ErrStorageUnavailable, "object payload metadata mismatch")
	}
	return cloneBytes(payload), nil
}

func (s *MemoryStore) activeDeviceLocked(domainID string, deviceID string) (Device, error) {
	device, ok := s.devices[deviceKey(domainID, deviceID)]
	if !ok {
		return Device{}, newError(ErrForbiddenDevice, "device is not registered")
	}
	if device.Status != DeviceActive {
		return Device{}, newError(ErrForbiddenDevice, "device is not active")
	}
	return device, nil
}

func checkContext(ctx context.Context) error {
	if ctx == nil {
		return nil
	}
	if err := ctx.Err(); err != nil {
		return fmt.Errorf("storage context: %w", err)
	}
	return nil
}

func validateDomain(domain Domain) error {
	if !validOpaqueID(domain.DomainID) {
		return newError(ErrInvalidRequest, "domain id must be an opaque id")
	}
	if domain.CurrentKeyEpoch == 0 {
		return newError(ErrInvalidRequest, "current key epoch must be positive")
	}
	if domain.ActiveKeyID == "" {
		return newError(ErrInvalidRequest, "active key id is required")
	}
	if domain.CreatedAtMs <= 0 || domain.UpdatedAtMs < domain.CreatedAtMs {
		return newError(ErrInvalidRequest, "domain timestamps are invalid")
	}
	return nil
}

func validateDevice(device Device) error {
	if !validOpaqueID(device.DomainID) || !validOpaqueID(device.DeviceID) {
		return newError(ErrInvalidRequest, "device ids must be opaque ids")
	}
	if device.SigningPublicKeyID == "" || len(device.SigningPublicKey) == 0 {
		return newError(ErrInvalidRequest, "signing public key is required")
	}
	if device.KeyAgreementPublicKeyID == "" || len(device.KeyAgreementPublicKey) == 0 {
		return newError(ErrInvalidRequest, "key agreement public key is required")
	}
	if !validDeviceStatus(device.Status) {
		return newError(ErrInvalidRequest, "device status is invalid")
	}
	return nil
}

func validateJoinRequest(request JoinRequest) error {
	if !validOpaqueID(request.DomainID) || !validOpaqueID(request.JoinRequestID) || !validOpaqueID(request.DeviceID) {
		return newError(ErrInvalidRequest, "join request ids must be opaque ids")
	}
	if request.Status != DevicePending {
		return newError(ErrInvalidRequest, "join request must start pending")
	}
	if request.SigningPublicKeyID == "" || len(request.SigningPublicKey) == 0 {
		return newError(ErrInvalidRequest, "join request signing key is required")
	}
	if request.KeyAgreementPublicKeyID == "" || len(request.KeyAgreementPublicKey) == 0 {
		return newError(ErrInvalidRequest, "join request key agreement key is required")
	}
	if len(request.Challenge) == 0 {
		return newError(ErrInvalidRequest, "join request challenge is required")
	}
	if request.CreatedAtMs <= 0 || request.ExpiresAtMs <= request.CreatedAtMs {
		return newError(ErrInvalidRequest, "join request timestamps are invalid")
	}
	return nil
}

func validateAuthorization(authorization DeviceAuthorization) error {
	if !validOpaqueID(authorization.DomainID) ||
		!validOpaqueID(authorization.JoinRequestID) ||
		!validOpaqueID(authorization.AuthorizerDeviceID) ||
		!validOpaqueID(authorization.RecipientDeviceID) {
		return newError(ErrInvalidRequest, "authorization ids must be opaque ids")
	}
	if authorization.RecipientSigningPublicKeyID == "" || authorization.RecipientKeyAgreementKeyID == "" {
		return newError(ErrInvalidRequest, "authorization recipient key ids are required")
	}
	if authorization.JoinShortCode == "" {
		return newError(ErrInvalidRequest, "authorization join short code is required")
	}
	if authorization.KeyEpoch == 0 {
		return newError(ErrInvalidRequest, "authorization key epoch must be positive")
	}
	if err := validateSignatureFields(authorization.SignatureSchemaVersion, authorization.SignatureAlgorithm, authorization.SignatureKeyID, authorization.Signature); err != nil {
		return err
	}
	if authorization.CreatedAtMs <= 0 {
		return newError(ErrInvalidRequest, "authorization timestamp is required")
	}
	return nil
}

func validateWrappingRecord(record DeviceWrappingRecord) error {
	if !validOpaqueID(record.DomainID) || !validOpaqueID(record.RecipientDeviceID) || !validOpaqueID(record.AuthorizerDeviceID) {
		return newError(ErrInvalidRequest, "wrapping record ids must be opaque ids")
	}
	if record.KeyEpoch == 0 || record.WrappingKeyID == "" || record.Algorithm == "" {
		return newError(ErrInvalidRequest, "wrapping record key metadata is required")
	}
	if len(record.Nonce) == 0 || record.WrappedKeyLen <= 0 || record.CiphertextHash == "" {
		return newError(ErrInvalidCiphertextMetadata, "wrapping record ciphertext metadata is required")
	}
	if record.CreatedAtMs <= 0 {
		return newError(ErrInvalidRequest, "wrapping record timestamp is required")
	}
	if len(record.Signature) == 0 {
		return newError(ErrInvalidSignature, "wrapping record signature is required")
	}
	return nil
}

func validateAuthorizationUpload(upload DeviceAuthorizationUpload) error {
	if err := validateWrappingRecord(upload.Wrapping); err != nil {
		return err
	}
	if len(upload.WrappedKey) == 0 {
		return newError(ErrInvalidCiphertextMetadata, "device wrapped key is required")
	}
	if int64(len(upload.WrappedKey)) != upload.Wrapping.WrappedKeyLen ||
		CiphertextHash(upload.WrappedKey) != upload.Wrapping.CiphertextHash {
		return newError(ErrInvalidCiphertextMetadata, "device wrapped key metadata mismatch")
	}
	return nil
}

func validateRevocation(revocation DeviceRevocation) error {
	if !validOpaqueID(revocation.DomainID) || !validOpaqueID(revocation.RevokedDeviceID) || !validOpaqueID(revocation.RevokerDeviceID) {
		return newError(ErrInvalidRequest, "revocation ids must be opaque ids")
	}
	if revocation.PreviousKeyEpoch == 0 || revocation.NewKeyEpoch <= revocation.PreviousKeyEpoch {
		return newError(ErrInvalidRequest, "revocation must advance key epoch")
	}
	if revocation.CreatedAtMs <= 0 {
		return newError(ErrInvalidRequest, "revocation timestamp is required")
	}
	if err := validateSignatureFields(revocation.SignatureSchemaVersion, revocation.SignatureAlgorithm, revocation.SignatureKeyID, revocation.Signature); err != nil {
		return err
	}
	return nil
}

func validateRecoveryRecordUpload(upload RecoveryRecordUpload) error {
	record := upload.Record
	if !validOpaqueID(record.DomainID) || !validOpaqueID(record.RecoveryRecordID) || !validOpaqueID(record.SignerDeviceID) {
		return newError(ErrInvalidRequest, "recovery record ids must be opaque ids")
	}
	if record.KeyEpoch == 0 || record.KDFProfile == "" || record.Algorithm == "" {
		return newError(ErrInvalidRequest, "recovery record key metadata is required")
	}
	if record.KDFVersion == 0 || record.MemoryKiB == 0 || record.Iterations == 0 ||
		record.Parallelism == 0 || record.OutputLen <= 0 {
		return newError(ErrInvalidRequest, "recovery record KDF parameters are required")
	}
	if len(record.Salt) == 0 || len(record.Nonce) == 0 {
		return newError(ErrInvalidCiphertextMetadata, "recovery record public crypto parameters are required")
	}
	if record.Status != RecoveryRecordActive && record.Status != RecoveryRecordRevoked {
		return newError(ErrInvalidRequest, "recovery record status is invalid")
	}
	if len(upload.WrappedMaterial) == 0 {
		return newError(ErrInvalidCiphertextMetadata, "recovery wrapped material is required")
	}
	if int64(len(upload.WrappedMaterial)) != record.WrappedMaterialLen || CiphertextHash(upload.WrappedMaterial) != record.CiphertextHash {
		return newError(ErrInvalidCiphertextMetadata, "recovery wrapped material metadata mismatch")
	}
	if record.CreatedAtMs <= 0 || record.RevokedAtMs < 0 {
		return newError(ErrInvalidRequest, "recovery record timestamps are invalid")
	}
	if err := validateSignatureFields(record.SignatureSchemaVersion, record.SignatureAlgorithm, record.SignatureKeyID, record.Signature); err != nil {
		return err
	}
	return nil
}

func validateObjectUpload(upload ObjectVersionUpload) error {
	version := upload.Version
	if !validOpaqueID(version.DomainID) || !validOpaqueID(version.ObjectID) || !validOpaqueID(version.OwnerDeviceID) {
		return newError(ErrInvalidRequest, "object ids must be opaque ids")
	}
	if !validObjectType(version.ObjectType) {
		return newError(ErrInvalidRequest, "object type is invalid")
	}
	if version.Version == 0 {
		return newError(ErrInvalidRequest, "object version must be positive")
	}
	if version.KeyID == "" || version.KeyEpoch == 0 || version.Algorithm == "" || len(version.Nonce) == 0 {
		return newError(ErrInvalidCiphertextMetadata, "object crypto metadata is required")
	}
	if len(upload.Payload) == 0 {
		return newError(ErrInvalidCiphertextMetadata, "encrypted payload is required")
	}
	if int64(len(upload.Payload)) != version.EncryptedPayloadLen || CiphertextHash(upload.Payload) != version.CiphertextHash {
		return newError(ErrInvalidCiphertextMetadata, "encrypted payload metadata mismatch")
	}
	if version.ClientCreatedAtMs <= 0 || version.ClientUpdatedAtMs < version.ClientCreatedAtMs {
		return newError(ErrInvalidRequest, "object timestamps are invalid")
	}
	if err := validateSignatureFields(version.SignatureSchemaVersion, version.SignatureAlgorithm, version.SignatureKeyID, version.Signature); err != nil {
		return err
	}
	return nil
}

func validateAuditEvent(event AuditEvent) error {
	if event.EventType == "" || event.ResultCode == "" {
		return newError(ErrInvalidRequest, "audit event type and result code are required")
	}
	if event.Version > 0 && event.ObjectID == "" {
		return newError(ErrInvalidRequest, "audit event object id is required for object version")
	}
	if event.Bytes < 0 || event.ServerTimeMs <= 0 {
		return newError(ErrInvalidRequest, "audit event counters are invalid")
	}
	return nil
}

func validateSignatureFields(schemaVersion uint16, algorithm string, keyID string, signature []byte) error {
	if schemaVersion != signatureSchemaVersion {
		return newError(ErrInvalidSignature, "signature schema version is unsupported")
	}
	if algorithm != signatureAlgorithm {
		return newError(ErrInvalidSignature, "signature algorithm is unsupported")
	}
	if keyID == "" {
		return newError(ErrInvalidSignature, "signature key id is required")
	}
	if len(signature) != ed25519SignatureLen {
		return newError(ErrInvalidSignature, "signature length is invalid")
	}
	return nil
}

func validOpaqueID(value string) bool {
	return value != "" && opaqueIDPattern.MatchString(value)
}

func validDeviceStatus(status DeviceStatus) bool {
	switch status {
	case DevicePending, DeviceActive, DeviceRevoked, DeviceLost:
		return true
	default:
		return false
	}
}

func validObjectType(objectType string) bool {
	switch objectType {
	case ObjectDictionaryUserTerms,
		ObjectDictionaryDeletedTerms,
		ObjectRankerWeights,
		ObjectSettingsProfile,
		ObjectSettingsSchema,
		ObjectBackupSnapshot:
		return true
	default:
		return false
	}
}

type domainDeviceKey struct {
	domainID string
	deviceID string
}

type joinRequestKey struct {
	domainID      string
	joinRequestID string
}

type wrappingKey struct {
	domainID          string
	recipientDeviceID string
	keyEpoch          uint64
	wrappingKeyID     string
}

type revocationKey struct {
	domainID        string
	revokedDeviceID string
	newKeyEpoch     uint64
}

type recoveryKey struct {
	domainID         string
	recoveryRecordID string
}

type objectKey struct {
	domainID string
	objectID string
}

type objectVersionKey struct {
	domainID string
	objectID string
	version  uint64
}

func deviceKey(domainID string, deviceID string) domainDeviceKey {
	return domainDeviceKey{domainID: domainID, deviceID: deviceID}
}

func joinKey(domainID string, joinRequestID string) joinRequestKey {
	return joinRequestKey{domainID: domainID, joinRequestID: joinRequestID}
}

func wrappingRecordKey(record DeviceWrappingRecord) wrappingKey {
	return wrappingKey{
		domainID:          record.DomainID,
		recipientDeviceID: record.RecipientDeviceID,
		keyEpoch:          record.KeyEpoch,
		wrappingKeyID:     record.WrappingKeyID,
	}
}

func revocationKeyFor(revocation DeviceRevocation) revocationKey {
	return revocationKey{
		domainID:        revocation.DomainID,
		revokedDeviceID: revocation.RevokedDeviceID,
		newKeyEpoch:     revocation.NewKeyEpoch,
	}
}

func objectVersionKeyFor(domainID string, objectID string, version uint64) objectVersionKey {
	return objectVersionKey{domainID: domainID, objectID: objectID, version: version}
}

func objectBlobRef(version ObjectVersion) string {
	return fmt.Sprintf(
		"objects/%s/%s/%d/%s",
		blobPathComponent(version.DomainID),
		blobPathComponent(version.ObjectID),
		version.Version,
		blobPathComponent(version.CiphertextHash),
	)
}

func recoveryBlobRef(record RecoveryRecord) string {
	return fmt.Sprintf(
		"recovery/%s/%s/%s",
		blobPathComponent(record.DomainID),
		blobPathComponent(record.RecoveryRecordID),
		blobPathComponent(record.CiphertextHash),
	)
}

func wrappingBlobRef(record DeviceWrappingRecord) string {
	return fmt.Sprintf(
		"wrapping/%s/%s/%d/%s/%s",
		blobPathComponent(record.DomainID),
		blobPathComponent(record.RecipientDeviceID),
		record.KeyEpoch,
		blobPathComponent(record.WrappingKeyID),
		blobPathComponent(record.CiphertextHash),
	)
}

func blobPathComponent(value string) string {
	return base64.RawURLEncoding.EncodeToString([]byte(value))
}

func cloneBytes(value []byte) []byte {
	if value == nil {
		return nil
	}
	return bytes.Clone(value)
}

func cloneDevice(value Device) Device {
	value.SigningPublicKey = cloneBytes(value.SigningPublicKey)
	value.KeyAgreementPublicKey = cloneBytes(value.KeyAgreementPublicKey)
	return value
}

func cloneJoinRequest(value JoinRequest) JoinRequest {
	value.SigningPublicKey = cloneBytes(value.SigningPublicKey)
	value.KeyAgreementPublicKey = cloneBytes(value.KeyAgreementPublicKey)
	value.Challenge = cloneBytes(value.Challenge)
	return value
}

func cloneAuthorization(value DeviceAuthorization) DeviceAuthorization {
	value.Signature = cloneBytes(value.Signature)
	return value
}

func cloneWrappingRecord(value DeviceWrappingRecord) DeviceWrappingRecord {
	value.Nonce = cloneBytes(value.Nonce)
	value.Signature = cloneBytes(value.Signature)
	return value
}

func cloneRevocation(value DeviceRevocation) DeviceRevocation {
	value.Signature = cloneBytes(value.Signature)
	return value
}

func cloneRecoveryRecord(value RecoveryRecord) RecoveryRecord {
	value.Salt = cloneBytes(value.Salt)
	value.Nonce = cloneBytes(value.Nonce)
	value.Signature = cloneBytes(value.Signature)
	return value
}

func cloneObjectVersion(value ObjectVersion) ObjectVersion {
	value.Nonce = cloneBytes(value.Nonce)
	value.Signature = cloneBytes(value.Signature)
	return value
}
