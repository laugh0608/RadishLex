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

	domains             map[string]Domain
	devices             map[domainDeviceKey]Device
	joinRequests        map[joinRequestKey]JoinRequest
	authorizations      map[joinRequestKey]DeviceAuthorization
	wrapping            map[wrappingKey]DeviceWrappingRecord
	revocations         map[revocationKey]DeviceRevocation
	recoveries          map[recoveryKey]RecoveryRecord
	activations         map[recoveryKey]RecoveredDeviceActivation
	recoveryRevocations map[recoveryKey]RecoveryRecordRevocationResult
	latestRecovery      map[string]string
	objects             map[objectKey]SyncObject
	versions            map[objectVersionKey]ObjectVersion
	nextSequence        map[string]uint64
	lifecycle           map[string][]LifecycleEvent
	nextLifecycle       map[string]uint64
	blobs               map[string][]byte
	auditEvents         []AuditEvent
}

func NewMemoryStore() *MemoryStore {
	return &MemoryStore{
		domains:             make(map[string]Domain),
		devices:             make(map[domainDeviceKey]Device),
		joinRequests:        make(map[joinRequestKey]JoinRequest),
		authorizations:      make(map[joinRequestKey]DeviceAuthorization),
		wrapping:            make(map[wrappingKey]DeviceWrappingRecord),
		revocations:         make(map[revocationKey]DeviceRevocation),
		recoveries:          make(map[recoveryKey]RecoveryRecord),
		activations:         make(map[recoveryKey]RecoveredDeviceActivation),
		recoveryRevocations: make(map[recoveryKey]RecoveryRecordRevocationResult),
		latestRecovery:      make(map[string]string),
		objects:             make(map[objectKey]SyncObject),
		versions:            make(map[objectVersionKey]ObjectVersion),
		nextSequence:        make(map[string]uint64),
		lifecycle:           make(map[string][]LifecycleEvent),
		nextLifecycle:       make(map[string]uint64),
		blobs:               make(map[string][]byte),
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
	s.appendLifecycleLocked(LifecycleEvent{
		DomainID:    domain.DomainID,
		EventType:   LifecycleInitialDevice,
		RecordID:    firstDevice.DeviceID,
		KeyEpoch:    domain.CurrentKeyEpoch,
		CreatedAtMs: domain.CreatedAtMs,
		Device:      devicePointer(firstDevice),
	})
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

func (s *MemoryStore) LifecycleSnapshot(ctx context.Context, domainID string) (LifecycleSnapshot, error) {
	if err := checkContext(ctx); err != nil {
		return LifecycleSnapshot{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	domain, ok := s.domains[domainID]
	if !ok {
		return LifecycleSnapshot{}, newError(ErrNotFound, "domain not found")
	}
	return LifecycleSnapshot{Domain: domain, Events: cloneLifecycleEvents(s.lifecycle[domainID])}, nil
}

func (s *MemoryStore) LifecycleEventsAfter(ctx context.Context, domainID string, afterSequence uint64, limit int) ([]LifecycleEvent, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	if !validOpaqueID(domainID) || limit <= 0 || limit > 201 {
		return nil, newError(ErrInvalidRequest, "lifecycle discovery parameters are invalid")
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, ok := s.domains[domainID]; !ok {
		return nil, newError(ErrNotFound, "domain not found")
	}
	events := make([]LifecycleEvent, 0, limit)
	for _, event := range s.lifecycle[domainID] {
		if event.LifecycleSequence > afterSequence {
			events = append(events, cloneLifecycleEvent(event))
			if len(events) == limit {
				break
			}
		}
	}
	return events, nil
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
		SigningAlgorithm:        request.SigningAlgorithm,
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
	wrapping.SignatureRecordType = WrappingSignatureDeviceAuthorization
	wrapping.SignatureSchemaVersion = authorization.SignatureSchemaVersion
	wrapping.SignatureAlgorithm = authorization.SignatureAlgorithm
	wrapping.SignatureKeyID = authorization.SignatureKeyID
	wrapping.Signature = cloneBytes(authorization.Signature)
	upload.Wrapping = wrapping
	if err := validateAuthorization(authorization); err != nil {
		return err
	}
	if err := validateAuthorizationUpload(upload); err != nil {
		return err
	}
	if wrapping.DomainID != authorization.DomainID ||
		wrapping.AuthorizerDeviceID != authorization.AuthorizerDeviceID ||
		wrapping.RecipientDeviceID != authorization.RecipientDeviceID ||
		wrapping.RecipientKeyAgreementKeyID != authorization.RecipientKeyAgreementKeyID ||
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
	device := Device{
		DomainID:                join.DomainID,
		DeviceID:                join.DeviceID,
		SigningAlgorithm:        join.SigningAlgorithm,
		SigningPublicKeyID:      join.SigningPublicKeyID,
		SigningPublicKey:        cloneBytes(join.SigningPublicKey),
		KeyAgreementPublicKeyID: join.KeyAgreementPublicKeyID,
		KeyAgreementPublicKey:   cloneBytes(join.KeyAgreementPublicKey),
		Status:                  DeviceActive,
		AuthorizedAtMs:          authorization.CreatedAtMs,
	}
	s.devices[deviceKey(join.DomainID, join.DeviceID)] = device
	s.appendLifecycleLocked(LifecycleEvent{
		DomainID:      authorization.DomainID,
		EventType:     LifecycleDeviceAuthorized,
		RecordID:      authorization.JoinRequestID,
		KeyEpoch:      authorization.KeyEpoch,
		CreatedAtMs:   authorization.CreatedAtMs,
		Device:        devicePointer(device),
		JoinRequest:   joinRequestPointer(join),
		Authorization: authorizationPointer(authorization),
		Wrapping:      wrappingPointer(wrapping),
	})
	return nil
}

func (s *MemoryStore) PutEpochDistribution(ctx context.Context, upload EpochDistributionUpload) (EpochDistributionResult, error) {
	if err := checkContext(ctx); err != nil {
		return EpochDistributionResult{}, err
	}
	if err := validateEpochDistributionUpload(upload); err != nil {
		return EpochDistributionResult{}, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	domain, ok := s.domains[upload.DomainID]
	if !ok {
		return EpochDistributionResult{}, newError(ErrNotFound, "domain not found")
	}
	if domain.CurrentKeyEpoch != upload.KeyEpoch {
		return EpochDistributionResult{}, newError(ErrInvalidRequest, "epoch distribution must target current domain epoch")
	}
	distributor, err := s.activeDeviceLocked(upload.DomainID, upload.DistributorDeviceID)
	if err != nil {
		return EpochDistributionResult{}, err
	}
	active := make(map[string]Device)
	for key, device := range s.devices {
		if key.domainID == upload.DomainID && device.Status == DeviceActive {
			active[device.DeviceID] = device
		}
	}
	if len(active) != len(upload.Records) {
		return EpochDistributionResult{}, newError(ErrInvalidRequest, "epoch distribution must cover every active device")
	}

	inserted := 0
	for _, item := range upload.Records {
		record := item.Record
		recipient, ok := active[record.RecipientDeviceID]
		if !ok || recipient.KeyAgreementPublicKeyID != record.RecipientKeyAgreementKeyID {
			return EpochDistributionResult{}, newError(ErrForbiddenDevice, "epoch distribution recipient is not active with the signed key")
		}
		if err := verifyEpochDistributionSignature(record, distributor); err != nil {
			return EpochDistributionResult{}, err
		}
		key := wrappingRecordKey(record)
		if existing, exists := s.wrapping[key]; exists {
			existingBytes, blobExists := s.blobs[existing.BlobRef]
			if !blobExists || !sameWrappingRecord(existing, record) || !bytes.Equal(existingBytes, item.WrappedKey) {
				return EpochDistributionResult{}, newError(ErrConflictEpochDistribution, "epoch distribution locator already contains different material")
			}
			continue
		}
		inserted++
	}

	for _, item := range upload.Records {
		record := item.Record
		key := wrappingRecordKey(record)
		if _, exists := s.wrapping[key]; exists {
			continue
		}
		record.BlobRef = wrappingBlobRef(record)
		s.wrapping[key] = cloneWrappingRecord(record)
		s.blobs[record.BlobRef] = cloneBytes(item.WrappedKey)
	}
	return EpochDistributionResult{
		KeyEpoch:        upload.KeyEpoch,
		AcceptedRecords: len(upload.Records),
		InsertedRecords: inserted,
	}, nil
}

func (s *MemoryStore) DeviceWrappedKey(ctx context.Context, domainID string, recipientDeviceID string, keyEpoch uint64, wrappingKeyID string) (DeviceWrappingRecord, []byte, error) {
	if err := checkContext(ctx); err != nil {
		return DeviceWrappingRecord{}, nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, err := s.activeDeviceLocked(domainID, recipientDeviceID); err != nil {
		return DeviceWrappingRecord{}, nil, err
	}
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
	if int64(len(wrappedKey)) != record.WrappedKeyLen || DeviceWrappedKeyCiphertextHash(record, wrappedKey) != record.CiphertextHash {
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
	s.appendLifecycleLocked(LifecycleEvent{
		DomainID:                       revocation.DomainID,
		EventType:                      LifecycleDeviceRevoked,
		RecordID:                       fmt.Sprintf("%s:%d", revocation.RevokedDeviceID, revocation.NewKeyEpoch),
		KeyEpoch:                       revocation.NewKeyEpoch,
		RejectFromObjectChangeSequence: s.nextSequence[revocation.DomainID] + 1,
		CreatedAtMs:                    revocation.CreatedAtMs,
		Device:                         devicePointer(target),
		Revocation:                     revocationPointer(revocation),
	})
	return nil
}

func (s *MemoryStore) appendLifecycleLocked(event LifecycleEvent) {
	s.nextLifecycle[event.DomainID]++
	event.LifecycleSequence = s.nextLifecycle[event.DomainID]
	s.lifecycle[event.DomainID] = append(s.lifecycle[event.DomainID], cloneLifecycleEvent(event))
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

	domain, ok := s.domains[upload.Record.DomainID]
	if !ok {
		return RecoveryRecord{}, newError(ErrNotFound, "domain not found")
	}
	if upload.Record.KeyEpoch != domain.CurrentKeyEpoch {
		return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery record key epoch is not current")
	}
	key := recoveryKey{domainID: upload.Record.DomainID, recoveryRecordID: upload.Record.RecoveryRecordID}
	if existing, ok := s.recoveries[key]; ok {
		wrapped := s.blobs[existing.BlobRef]
		if sameRecoveryRecord(existing, upload.Record) && bytes.Equal(wrapped, upload.WrappedMaterial) {
			return cloneRecoveryRecord(existing), nil
		}
		return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery record id already exists with different content")
	}
	currentID := s.latestRecovery[upload.Record.DomainID]
	if currentID == "" {
		if upload.Record.PreviousRecoveryID != "" {
			return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "first recovery record cannot name a predecessor")
		}
	} else if upload.Record.PreviousRecoveryID != currentID {
		return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery record predecessor is stale")
	} else if upload.Record.CreatedAtMs <= s.recoveries[recoveryKey{domainID: upload.Record.DomainID, recoveryRecordID: currentID}].CreatedAtMs {
		return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery record timestamp does not advance predecessor")
	} else if !recoveryRecordRotatesPublicMaterial(
		s.recoveries[recoveryKey{domainID: upload.Record.DomainID, recoveryRecordID: currentID}],
		upload.Record,
	) {
		return RecoveryRecord{}, newError(ErrConflictRecoveryRecord, "recovery rotation must replace public crypto material")
	}
	signer, err := s.activeDeviceLocked(upload.Record.DomainID, upload.Record.SignerDeviceID)
	if err != nil {
		return RecoveryRecord{}, err
	}
	if err := verifyRecoverySignature(upload.Record, signer); err != nil {
		return RecoveryRecord{}, err
	}
	record := cloneRecoveryRecord(upload.Record)
	record.BlobRef = recoveryBlobRef(record)
	if currentID != "" {
		previousKey := recoveryKey{domainID: record.DomainID, recoveryRecordID: currentID}
		previous := s.recoveries[previousKey]
		if previous.Status == RecoveryRecordActive {
			previous.Status = RecoveryRecordSuperseded
			previous.RevokedAtMs = record.CreatedAtMs
			s.recoveries[previousKey] = previous
		}
	}
	s.recoveries[key] = record
	s.latestRecovery[record.DomainID] = record.RecoveryRecordID
	s.blobs[record.BlobRef] = cloneBytes(upload.WrappedMaterial)
	s.appendLifecycleLocked(LifecycleEvent{
		DomainID:       record.DomainID,
		EventType:      LifecycleRecoveryRecordRotated,
		RecordID:       record.RecoveryRecordID,
		KeyEpoch:       record.KeyEpoch,
		CreatedAtMs:    record.CreatedAtMs,
		Device:         devicePointer(signer),
		RecoveryRecord: recoveryRecordPointer(record),
	})
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
	if record.Status != RecoveryRecordActive {
		return RecoveryRecord{}, newError(ErrNotFound, "active recovery record not found")
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
	if record.Status != RecoveryRecordActive {
		return RecoveryRecord{}, nil, newError(ErrNotFound, "active recovery record not found")
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

func (s *MemoryStore) RecoverDevice(ctx context.Context, upload RecoveredDeviceActivationUpload) (RecoveredDeviceActivationResult, error) {
	if err := checkContext(ctx); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	if err := validateRecoveredDeviceActivationUpload(upload); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	activation := upload.Activation
	domain, ok := s.domains[activation.DomainID]
	if !ok {
		return RecoveredDeviceActivationResult{}, newError(ErrNotFound, "domain not found")
	}
	if domain.CurrentKeyEpoch != activation.KeyEpoch {
		return RecoveredDeviceActivationResult{}, newError(ErrConflictRecoveryRecord, "recovery activation key epoch is not current")
	}
	if s.latestRecovery[activation.DomainID] != activation.RecoveryRecordID {
		return RecoveredDeviceActivationResult{}, newError(ErrConflictRecoveryRecord, "recovery record is not the chain head")
	}
	recoveryKey := recoveryKey{domainID: activation.DomainID, recoveryRecordID: activation.RecoveryRecordID}
	record, ok := s.recoveries[recoveryKey]
	if !ok || record.Status != RecoveryRecordActive {
		return RecoveredDeviceActivationResult{}, newError(ErrConflictRecoveryRecord, "recovery record is not active")
	}
	if record.RecordSchemaVersion != RecoveryRecordSchemaVersionV2 || record.KeyEpoch != activation.KeyEpoch {
		return RecoveredDeviceActivationResult{}, newError(ErrConflictRecoveryRecord, "recovery record cannot activate current epoch")
	}
	if _, exists := s.devices[deviceKey(activation.DomainID, activation.DeviceID)]; exists {
		return RecoveredDeviceActivationResult{}, newError(ErrForbiddenDevice, "recovered device id is already registered")
	}
	if err := verifyRecoveredDeviceActivationSignature(activation, record); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}
	device := recoveredDeviceFromActivation(activation)
	if err := validateDevice(device); err != nil {
		return RecoveredDeviceActivationResult{}, err
	}

	active := make(map[string]Device)
	for key, current := range s.devices {
		if key.domainID == activation.DomainID && current.Status == DeviceActive {
			active[current.DeviceID] = current
		}
	}
	active[device.DeviceID] = device
	if len(active) != len(upload.Distribution.Records) {
		return RecoveredDeviceActivationResult{}, newError(ErrInvalidRequest, "recovery epoch distribution must cover every active device")
	}
	for _, item := range upload.Distribution.Records {
		wrapped := item.Record
		recipient, exists := active[wrapped.RecipientDeviceID]
		if !exists || recipient.KeyAgreementPublicKeyID != wrapped.RecipientKeyAgreementKeyID {
			return RecoveredDeviceActivationResult{}, newError(ErrForbiddenDevice, "recovery epoch recipient is not active with the signed key")
		}
		if err := verifyEpochDistributionSignature(wrapped, device); err != nil {
			return RecoveredDeviceActivationResult{}, err
		}
		key := wrappingRecordKey(wrapped)
		if existing, exists := s.wrapping[key]; exists {
			existingBytes, blobExists := s.blobs[existing.BlobRef]
			if !blobExists || !sameWrappingRecord(existing, wrapped) || !bytes.Equal(existingBytes, item.WrappedKey) {
				return RecoveredDeviceActivationResult{}, newError(ErrConflictEpochDistribution, "recovery epoch distribution locator conflicts")
			}
		}
	}

	s.devices[deviceKey(device.DomainID, device.DeviceID)] = cloneDevice(device)
	s.activations[recoveryKey] = cloneRecoveredDeviceActivation(activation)
	record.Status = RecoveryRecordSuperseded
	record.RevokedAtMs = activation.CreatedAtMs
	s.recoveries[recoveryKey] = record
	for _, item := range upload.Distribution.Records {
		wrapped := item.Record
		key := wrappingRecordKey(wrapped)
		if _, exists := s.wrapping[key]; exists {
			continue
		}
		wrapped.BlobRef = wrappingBlobRef(wrapped)
		s.wrapping[key] = cloneWrappingRecord(wrapped)
		s.blobs[wrapped.BlobRef] = cloneBytes(item.WrappedKey)
	}
	s.appendLifecycleLocked(LifecycleEvent{
		DomainID:            activation.DomainID,
		EventType:           LifecycleDeviceRecovered,
		RecordID:            activation.RecoveryRecordID,
		KeyEpoch:            activation.KeyEpoch,
		CreatedAtMs:         activation.CreatedAtMs,
		Device:              devicePointer(device),
		RecoveredActivation: recoveredActivationPointer(activation),
	})
	return RecoveredDeviceActivationResult{
		Device:             cloneDevice(device),
		LifecycleSequence:  s.nextLifecycle[activation.DomainID],
		DistributedRecords: len(upload.Distribution.Records),
	}, nil
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
	s.nextSequence[version.DomainID]++
	version.ChangeSequence = s.nextSequence[version.DomainID]
	version.BlobRef = objectBlobRef(version)
	object.LatestVersion = version.Version
	object.LatestCiphertextHash = version.CiphertextHash
	object.LatestKeyEpoch = version.KeyEpoch
	object.LatestChangeSequence = version.ChangeSequence
	object.UpdatedAtMs = version.ClientUpdatedAtMs
	s.objects[objKey] = object
	s.versions[versionKey] = version
	s.blobs[version.BlobRef] = cloneBytes(upload.Payload)
	return cloneObjectVersion(version), nil
}

func (s *MemoryStore) ObjectVersionsAfter(ctx context.Context, domainID string, afterSequence uint64, limit int) ([]ObjectVersion, error) {
	if err := checkContext(ctx); err != nil {
		return nil, err
	}
	if !validOpaqueID(domainID) || limit <= 0 || limit > 201 {
		return nil, newError(ErrInvalidRequest, "object discovery parameters are invalid")
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, ok := s.domains[domainID]; !ok {
		return nil, newError(ErrNotFound, "domain not found")
	}
	versions := make([]ObjectVersion, 0, limit)
	for _, version := range s.versions {
		if version.DomainID == domainID && version.ChangeSequence > afterSequence {
			versions = append(versions, cloneObjectVersion(version))
		}
	}
	sort.Slice(versions, func(i int, j int) bool {
		return versions[i].ChangeSequence < versions[j].ChangeSequence
	})
	if len(versions) > limit {
		versions = versions[:limit]
	}
	return versions, nil
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
	if int64(len(payload)) != metadata.EncryptedPayloadLen || ObjectCiphertextHash(metadata, payload) != metadata.CiphertextHash {
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
	if device.SigningAlgorithm == "" || device.SigningPublicKeyID == "" || len(device.SigningPublicKey) == 0 {
		return newError(ErrInvalidRequest, "signing public key is required")
	}
	if err := validateSigningPublicKeyEncoding(device.SigningAlgorithm, device.SigningPublicKey); err != nil {
		return err
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
	if request.SigningAlgorithm == "" || request.SigningPublicKeyID == "" || len(request.SigningPublicKey) == 0 {
		return newError(ErrInvalidRequest, "join request signing key is required")
	}
	if err := validateSigningPublicKeyEncoding(request.SigningAlgorithm, request.SigningPublicKey); err != nil {
		return err
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
	if record.KeyEpoch == 0 || record.RecipientKeyAgreementKeyID == "" || record.WrappingKeyID == "" || record.Algorithm == "" {
		return newError(ErrInvalidRequest, "wrapping record key metadata is required")
	}
	if len(record.Nonce) == 0 || record.WrappedKeyLen <= 0 || record.WrappedKeyLen > MaxDeviceWrappedKeyBytes || record.CiphertextHash == "" {
		return newError(ErrInvalidCiphertextMetadata, "wrapping record ciphertext metadata is required")
	}
	if record.CreatedAtMs <= 0 {
		return newError(ErrInvalidRequest, "wrapping record timestamp is required")
	}
	if record.SignatureRecordType != WrappingSignatureDeviceAuthorization && record.SignatureRecordType != WrappingSignatureEpochDistribution {
		return newError(ErrInvalidSignature, "wrapping record signature type is unsupported")
	}
	if err := validateSignatureFields(record.SignatureSchemaVersion, record.SignatureAlgorithm, record.SignatureKeyID, record.Signature); err != nil {
		return err
	}
	return nil
}

func validateAuthorizationUpload(upload DeviceAuthorizationUpload) error {
	if err := validateWrappingRecord(upload.Wrapping); err != nil {
		return err
	}
	if len(upload.WrappedKey) == 0 || len(upload.WrappedKey) > MaxDeviceWrappedKeyBytes {
		return newError(ErrInvalidCiphertextMetadata, "device wrapped key is required")
	}
	if int64(len(upload.WrappedKey)) != upload.Wrapping.WrappedKeyLen ||
		DeviceWrappedKeyCiphertextHash(upload.Wrapping, upload.WrappedKey) != upload.Wrapping.CiphertextHash {
		return newError(ErrInvalidCiphertextMetadata, "device wrapped key metadata mismatch")
	}
	return nil
}

func validateEpochDistributionUpload(upload EpochDistributionUpload) error {
	if !validOpaqueID(upload.DomainID) || !validOpaqueID(upload.DistributorDeviceID) || upload.KeyEpoch == 0 {
		return newError(ErrInvalidRequest, "epoch distribution identity is invalid")
	}
	if len(upload.Records) == 0 || len(upload.Records) > MaxEpochDistributionRecords {
		return newError(ErrInvalidRequest, "epoch distribution record count is invalid")
	}
	recipients := make(map[string]struct{}, len(upload.Records))
	totalBytes := 0
	for _, item := range upload.Records {
		record := item.Record
		if err := validateWrappingRecord(record); err != nil {
			return err
		}
		if record.SignatureRecordType != WrappingSignatureEpochDistribution ||
			record.DomainID != upload.DomainID ||
			record.AuthorizerDeviceID != upload.DistributorDeviceID ||
			record.KeyEpoch != upload.KeyEpoch {
			return newError(ErrInvalidRequest, "epoch distribution record does not match batch")
		}
		if _, exists := recipients[record.RecipientDeviceID]; exists {
			return newError(ErrInvalidRequest, "epoch distribution recipients must be unique")
		}
		recipients[record.RecipientDeviceID] = struct{}{}
		if len(item.WrappedKey) == 0 || len(item.WrappedKey) > MaxDeviceWrappedKeyBytes {
			return newError(ErrInvalidCiphertextMetadata, "epoch distribution wrapped key is invalid")
		}
		totalBytes += len(item.WrappedKey)
		if totalBytes > MaxEpochDistributionBytes ||
			int64(len(item.WrappedKey)) != record.WrappedKeyLen ||
			DeviceWrappedKeyCiphertextHash(record, item.WrappedKey) != record.CiphertextHash {
			return newError(ErrInvalidCiphertextMetadata, "epoch distribution ciphertext metadata mismatch")
		}
	}
	return nil
}

func sameWrappingRecord(left DeviceWrappingRecord, right DeviceWrappingRecord) bool {
	return left.DomainID == right.DomainID &&
		left.RecipientDeviceID == right.RecipientDeviceID &&
		left.RecipientKeyAgreementKeyID == right.RecipientKeyAgreementKeyID &&
		left.AuthorizerDeviceID == right.AuthorizerDeviceID &&
		left.KeyEpoch == right.KeyEpoch &&
		left.WrappingKeyID == right.WrappingKeyID &&
		left.Algorithm == right.Algorithm &&
		bytes.Equal(left.Nonce, right.Nonce) &&
		left.WrappedKeyLen == right.WrappedKeyLen &&
		left.CiphertextHash == right.CiphertextHash &&
		left.CreatedAtMs == right.CreatedAtMs &&
		left.SignatureRecordType == right.SignatureRecordType &&
		left.SignatureSchemaVersion == right.SignatureSchemaVersion &&
		left.SignatureAlgorithm == right.SignatureAlgorithm &&
		left.SignatureKeyID == right.SignatureKeyID &&
		bytes.Equal(left.Signature, right.Signature)
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
	if record.RecordSchemaVersion != RecoveryRecordSchemaVersionV2 {
		return newError(ErrInvalidRequest, "recovery record schema version is unsupported")
	}
	if record.PreviousRecoveryID != "" && !validOpaqueID(record.PreviousRecoveryID) {
		return newError(ErrInvalidRequest, "recovery record predecessor must be an opaque id")
	}
	if record.PreviousRecoveryID == record.RecoveryRecordID {
		return newError(ErrInvalidRequest, "recovery record cannot name itself as predecessor")
	}
	if record.KeyEpoch == 0 || record.KDFProfile != "argon2id-v1" || record.Algorithm != AlgorithmXChaCha20Poly1305HKDFSHA256 {
		return newError(ErrInvalidRequest, "recovery record key metadata is required")
	}
	if record.KDFVersion != 1 || record.MemoryKiB != 65536 || record.Iterations != 3 ||
		record.Parallelism != 4 || record.OutputLen != 32 {
		return newError(ErrInvalidRequest, "recovery record KDF parameters are required")
	}
	if len(record.Salt) != RecoverySaltBytes || len(record.Nonce) != RecoveryNonceBytes {
		return newError(ErrInvalidCiphertextMetadata, "recovery record public crypto parameters are required")
	}
	if record.ActivationAlgorithm != SignatureAlgorithmEd25519V1 || record.ActivationPublicKeyID == "" || len(record.ActivationPublicKey) != ed25519PublicKeyLen {
		return newError(ErrInvalidRequest, "recovery activation public profile is invalid")
	}
	if record.Status != RecoveryRecordActive || record.RevokedAtMs != 0 {
		return newError(ErrInvalidRequest, "new recovery record must be active")
	}
	if len(upload.WrappedMaterial) != RecoveryWrappedMaterialBytes {
		return newError(ErrInvalidCiphertextMetadata, "recovery wrapped material is required")
	}
	if int64(len(upload.WrappedMaterial)) != record.WrappedMaterialLen || CiphertextHash(upload.WrappedMaterial) != record.CiphertextHash {
		return newError(ErrInvalidCiphertextMetadata, "recovery wrapped material metadata mismatch")
	}
	if record.CreatedAtMs <= 0 || record.UpdatedAtMs < record.CreatedAtMs {
		return newError(ErrInvalidRequest, "recovery record timestamps are invalid")
	}
	if err := validateSignatureFields(record.SignatureSchemaVersion, record.SignatureAlgorithm, record.SignatureKeyID, record.Signature); err != nil {
		return err
	}
	return nil
}

func validateRecoveredDeviceActivationUpload(upload RecoveredDeviceActivationUpload) error {
	activation := upload.Activation
	if !validOpaqueID(activation.DomainID) || !validOpaqueID(activation.RecoveryRecordID) || !validOpaqueID(activation.DeviceID) {
		return newError(ErrInvalidRequest, "recovered device activation ids must be opaque ids")
	}
	if activation.SignatureSchemaVersion != signatureSchemaVersion ||
		activation.ActivationAlgorithm != SignatureAlgorithmEd25519V1 ||
		activation.ActivationPublicKeyID == "" || len(activation.ActivationSignature) != ed25519SignatureLen {
		return newError(ErrInvalidSignature, "recovered device activation signature profile is invalid")
	}
	if !supportedSignatureAlgorithm(activation.SigningAlgorithm) || activation.SigningPublicKeyID == "" {
		return newError(ErrInvalidRequest, "recovered device signing profile is invalid")
	}
	if err := validateSigningPublicKeyEncoding(activation.SigningAlgorithm, activation.SigningPublicKey); err != nil {
		return err
	}
	if activation.KeyAgreementAlgorithm != "p256-ecdh-v1" ||
		activation.KeyAgreementPublicKeyID == "" {
		return newError(ErrInvalidRequest, "recovered device key agreement profile is invalid")
	}
	if err := validateP256KeyAgreementPublicKey(activation.KeyAgreementPublicKey); err != nil {
		return err
	}
	if activation.KeyEpoch == 0 || activation.CreatedAtMs <= 0 {
		return newError(ErrInvalidRequest, "recovered device activation counters are invalid")
	}
	distribution := upload.Distribution
	if distribution.DomainID != activation.DomainID || distribution.DistributorDeviceID != activation.DeviceID || distribution.KeyEpoch != activation.KeyEpoch {
		return newError(ErrInvalidRequest, "recovery epoch distribution does not match activation")
	}
	return validateEpochDistributionUpload(distribution)
}

func recoveredDeviceFromActivation(activation RecoveredDeviceActivation) Device {
	return Device{
		DomainID:                activation.DomainID,
		DeviceID:                activation.DeviceID,
		SigningAlgorithm:        activation.SigningAlgorithm,
		SigningPublicKeyID:      activation.SigningPublicKeyID,
		SigningPublicKey:        cloneBytes(activation.SigningPublicKey),
		KeyAgreementPublicKeyID: activation.KeyAgreementPublicKeyID,
		KeyAgreementPublicKey:   cloneBytes(activation.KeyAgreementPublicKey),
		Status:                  DeviceActive,
		AuthorizedAtMs:          activation.CreatedAtMs,
	}
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
	if int64(len(upload.Payload)) != version.EncryptedPayloadLen || ObjectCiphertextHash(version, upload.Payload) != version.CiphertextHash {
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
		return newSignatureError(signatureDetailAlgorithm, "signature schema version is unsupported")
	}
	if !supportedSignatureAlgorithm(algorithm) {
		return newSignatureError(signatureDetailAlgorithm, "signature algorithm is unsupported")
	}
	if keyID == "" {
		return newSignatureError(signatureDetailVerify, "signature key id is required")
	}
	return validateSignatureEncoding(algorithm, signature)
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

func devicePointer(value Device) *Device {
	cloned := cloneDevice(value)
	return &cloned
}

func authorizationPointer(value DeviceAuthorization) *DeviceAuthorization {
	cloned := cloneAuthorization(value)
	return &cloned
}

func joinRequestPointer(value JoinRequest) *JoinRequest {
	cloned := cloneJoinRequest(value)
	return &cloned
}

func wrappingPointer(value DeviceWrappingRecord) *DeviceWrappingRecord {
	cloned := cloneWrappingRecord(value)
	return &cloned
}

func revocationPointer(value DeviceRevocation) *DeviceRevocation {
	cloned := cloneRevocation(value)
	return &cloned
}

func recoveryRecordPointer(value RecoveryRecord) *RecoveryRecord {
	cloned := cloneRecoveryRecord(value)
	return &cloned
}

func cloneRecoveredDeviceActivation(value RecoveredDeviceActivation) RecoveredDeviceActivation {
	value.SigningPublicKey = cloneBytes(value.SigningPublicKey)
	value.KeyAgreementPublicKey = cloneBytes(value.KeyAgreementPublicKey)
	value.ActivationSignature = cloneBytes(value.ActivationSignature)
	return value
}

func recoveredActivationPointer(value RecoveredDeviceActivation) *RecoveredDeviceActivation {
	cloned := cloneRecoveredDeviceActivation(value)
	return &cloned
}

func cloneLifecycleEvent(value LifecycleEvent) LifecycleEvent {
	if value.Device != nil {
		value.Device = devicePointer(*value.Device)
	}
	if value.Authorization != nil {
		value.Authorization = authorizationPointer(*value.Authorization)
	}
	if value.JoinRequest != nil {
		value.JoinRequest = joinRequestPointer(*value.JoinRequest)
	}
	if value.Wrapping != nil {
		value.Wrapping = wrappingPointer(*value.Wrapping)
	}
	if value.Revocation != nil {
		value.Revocation = revocationPointer(*value.Revocation)
	}
	if value.RecoveryRecord != nil {
		value.RecoveryRecord = recoveryRecordPointer(*value.RecoveryRecord)
	}
	if value.RecoveredActivation != nil {
		value.RecoveredActivation = recoveredActivationPointer(*value.RecoveredActivation)
	}
	if value.RecoveryRevocation != nil {
		value.RecoveryRevocation = recoveryRevocationPointer(*value.RecoveryRevocation)
	}
	return value
}

func cloneLifecycleEvents(values []LifecycleEvent) []LifecycleEvent {
	cloned := make([]LifecycleEvent, 0, len(values))
	for _, value := range values {
		cloned = append(cloned, cloneLifecycleEvent(value))
	}
	return cloned
}

func cloneRecoveryRecord(value RecoveryRecord) RecoveryRecord {
	value.Salt = cloneBytes(value.Salt)
	value.Nonce = cloneBytes(value.Nonce)
	value.ActivationPublicKey = cloneBytes(value.ActivationPublicKey)
	value.Signature = cloneBytes(value.Signature)
	return value
}

func sameRecoveryRecord(left RecoveryRecord, right RecoveryRecord) bool {
	return left.RecordSchemaVersion == right.RecordSchemaVersion &&
		left.DomainID == right.DomainID &&
		left.RecoveryRecordID == right.RecoveryRecordID &&
		left.PreviousRecoveryID == right.PreviousRecoveryID &&
		left.KeyEpoch == right.KeyEpoch &&
		left.KDFProfile == right.KDFProfile &&
		left.KDFVersion == right.KDFVersion &&
		left.MemoryKiB == right.MemoryKiB &&
		left.Iterations == right.Iterations &&
		left.Parallelism == right.Parallelism &&
		left.OutputLen == right.OutputLen &&
		bytes.Equal(left.Salt, right.Salt) &&
		left.Algorithm == right.Algorithm &&
		bytes.Equal(left.Nonce, right.Nonce) &&
		left.WrappedMaterialLen == right.WrappedMaterialLen &&
		left.CiphertextHash == right.CiphertextHash &&
		left.ActivationAlgorithm == right.ActivationAlgorithm &&
		left.ActivationPublicKeyID == right.ActivationPublicKeyID &&
		bytes.Equal(left.ActivationPublicKey, right.ActivationPublicKey) &&
		left.Status == right.Status &&
		left.CreatedAtMs == right.CreatedAtMs &&
		left.UpdatedAtMs == right.UpdatedAtMs &&
		left.RevokedAtMs == right.RevokedAtMs &&
		left.SignerDeviceID == right.SignerDeviceID &&
		left.SignatureSchemaVersion == right.SignatureSchemaVersion &&
		left.SignatureAlgorithm == right.SignatureAlgorithm &&
		left.SignatureKeyID == right.SignatureKeyID &&
		bytes.Equal(left.Signature, right.Signature)
}

func cloneObjectVersion(value ObjectVersion) ObjectVersion {
	value.Nonce = cloneBytes(value.Nonce)
	value.Signature = cloneBytes(value.Signature)
	return value
}
