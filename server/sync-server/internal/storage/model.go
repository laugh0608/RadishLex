package storage

type DeviceStatus string

const (
	DevicePending DeviceStatus = "pending"
	DeviceActive  DeviceStatus = "active"
	DeviceRevoked DeviceStatus = "revoked"
	DeviceLost    DeviceStatus = "lost"
)

type RecoveryRecordStatus string

const (
	RecoveryRecordActive     RecoveryRecordStatus = "active"
	RecoveryRecordSuperseded RecoveryRecordStatus = "superseded"
	RecoveryRecordRevoked    RecoveryRecordStatus = "revoked"
)

const (
	RecoveryRecordSchemaVersionV2 = 2
	RecoverySaltBytes             = 16
	RecoveryNonceBytes            = 24
	RecoveryWrappedMaterialBytes  = 48

	ObjectDictionaryUserTerms    = "dictionary.user_terms"
	ObjectDictionaryDeletedTerms = "dictionary.deleted_terms"
	ObjectRankerWeights          = "ranker.weights"
	ObjectSettingsProfile        = "settings.profile"
	ObjectSettingsSchema         = "settings.schema"
	ObjectBackupSnapshot         = "backup.snapshot"

	AlgorithmXChaCha20Poly1305HKDFSHA256 = "xchacha20poly1305-hkdf-sha256-v1"
	AlgorithmWrappedEpochP256ECDHV1      = "p256-ecdh-hkdf-sha256-xchacha20poly1305-v1"
	SignatureAlgorithmEd25519V1          = "ed25519-v1"
	SignatureAlgorithmECDSAP256SHA256V1  = "ecdsa-p256-sha256-v1"
	MaxDeviceWrappedKeyBytes             = 64 * 1024
	MaxEpochDistributionRecords          = 64
	MaxEpochDistributionBytes            = MaxDeviceWrappedKeyBytes * MaxEpochDistributionRecords
	WrappingSignatureDeviceAuthorization = "device_authorization"
	WrappingSignatureEpochDistribution   = "epoch_distribution"
	RecoveredDeviceActivationRecordType  = "recovered_device_activation"
)

type Domain struct {
	DomainID        string
	CurrentKeyEpoch uint64
	ActiveKeyID     string
	CreatedAtMs     int64
	UpdatedAtMs     int64
}

type Device struct {
	DomainID                string
	DeviceID                string
	SigningAlgorithm        string
	SigningPublicKeyID      string
	SigningPublicKey        []byte
	KeyAgreementPublicKeyID string
	KeyAgreementPublicKey   []byte
	Status                  DeviceStatus
	AuthorizedAtMs          int64
	RevokedAtMs             int64
	LastSeenAtMs            int64
}

type JoinRequest struct {
	DomainID                string
	JoinRequestID           string
	DeviceID                string
	SigningAlgorithm        string
	SigningPublicKeyID      string
	SigningPublicKey        []byte
	KeyAgreementPublicKeyID string
	KeyAgreementPublicKey   []byte
	Challenge               []byte
	CreatedAtMs             int64
	ExpiresAtMs             int64
	Status                  DeviceStatus
}

type DeviceWrappingRecord struct {
	DomainID                   string
	RecipientDeviceID          string
	RecipientKeyAgreementKeyID string
	AuthorizerDeviceID         string
	KeyEpoch                   uint64
	WrappingKeyID              string
	Algorithm                  string
	Nonce                      []byte
	WrappedKeyLen              int64
	CiphertextHash             string
	CreatedAtMs                int64
	SignatureRecordType        string
	SignatureSchemaVersion     uint16
	SignatureAlgorithm         string
	SignatureKeyID             string
	Signature                  []byte
	BlobRef                    string
}

type DeviceWrappingUpload struct {
	Record     DeviceWrappingRecord
	WrappedKey []byte
}

type EpochDistributionUpload struct {
	DomainID            string
	DistributorDeviceID string
	KeyEpoch            uint64
	Records             []DeviceWrappingUpload
}

type EpochDistributionResult struct {
	KeyEpoch        uint64
	AcceptedRecords int
	InsertedRecords int
}

type DeviceAuthorizationUpload struct {
	Authorization DeviceAuthorization
	Wrapping      DeviceWrappingRecord
	WrappedKey    []byte
}

type DeviceAuthorization struct {
	DomainID                    string
	JoinRequestID               string
	AuthorizerDeviceID          string
	RecipientDeviceID           string
	RecipientSigningPublicKeyID string
	RecipientKeyAgreementKeyID  string
	JoinShortCode               string
	KeyEpoch                    uint64
	CreatedAtMs                 int64
	SignatureSchemaVersion      uint16
	SignatureAlgorithm          string
	SignatureKeyID              string
	Signature                   []byte
}

type DeviceRevocation struct {
	DomainID               string
	RevokedDeviceID        string
	RevokerDeviceID        string
	PreviousKeyEpoch       uint64
	NewKeyEpoch            uint64
	Reason                 string
	CreatedAtMs            int64
	SignatureSchemaVersion uint16
	SignatureAlgorithm     string
	SignatureKeyID         string
	Signature              []byte
}

type RecoveredDeviceActivation struct {
	RecoveryRecordID        string
	DomainID                string
	DeviceID                string
	SigningAlgorithm        string
	SigningPublicKeyID      string
	SigningPublicKey        []byte
	KeyAgreementAlgorithm   string
	KeyAgreementPublicKeyID string
	KeyAgreementPublicKey   []byte
	KeyEpoch                uint64
	CreatedAtMs             int64
	SignatureSchemaVersion  uint16
	ActivationAlgorithm     string
	ActivationPublicKeyID   string
	ActivationSignature     []byte
}

type RecoveredDeviceActivationUpload struct {
	Activation   RecoveredDeviceActivation
	Distribution EpochDistributionUpload
}

type RecoveredDeviceActivationResult struct {
	Device             Device
	LifecycleSequence  uint64
	DistributedRecords int
}

type LifecycleEventType string

const (
	LifecycleInitialDevice         LifecycleEventType = "initial_device"
	LifecycleDeviceAuthorized      LifecycleEventType = "device_authorized"
	LifecycleDeviceRevoked         LifecycleEventType = "device_revoked"
	LifecycleRecoveryRecordRotated LifecycleEventType = "recovery_record_rotated"
	LifecycleDeviceRecovered       LifecycleEventType = "device_recovered"
)

type LifecycleEvent struct {
	DomainID                       string
	LifecycleSequence              uint64
	EventType                      LifecycleEventType
	RecordID                       string
	KeyEpoch                       uint64
	RejectFromObjectChangeSequence uint64
	CreatedAtMs                    int64
	Device                         *Device
	JoinRequest                    *JoinRequest
	Authorization                  *DeviceAuthorization
	Wrapping                       *DeviceWrappingRecord
	Revocation                     *DeviceRevocation
	RecoveryRecord                 *RecoveryRecord
	RecoveredActivation            *RecoveredDeviceActivation
}

type LifecycleSnapshot struct {
	Domain Domain
	Events []LifecycleEvent
}

type RecoveryRecord struct {
	RecordSchemaVersion    uint16
	DomainID               string
	RecoveryRecordID       string
	PreviousRecoveryID     string
	KeyEpoch               uint64
	KDFProfile             string
	KDFVersion             uint16
	MemoryKiB              uint32
	Iterations             uint32
	Parallelism            uint32
	OutputLen              int64
	Salt                   []byte
	Algorithm              string
	Nonce                  []byte
	WrappedMaterialLen     int64
	CiphertextHash         string
	ActivationAlgorithm    string
	ActivationPublicKeyID  string
	ActivationPublicKey    []byte
	Status                 RecoveryRecordStatus
	CreatedAtMs            int64
	UpdatedAtMs            int64
	RevokedAtMs            int64
	SignerDeviceID         string
	SignatureSchemaVersion uint16
	SignatureAlgorithm     string
	SignatureKeyID         string
	Signature              []byte
	BlobRef                string
}

type RecoveryRecordUpload struct {
	Record          RecoveryRecord
	WrappedMaterial []byte
}

type SyncObject struct {
	DomainID             string
	ObjectID             string
	ObjectType           string
	LatestVersion        uint64
	LatestCiphertextHash string
	LatestKeyEpoch       uint64
	LatestChangeSequence uint64
	CreatedAtMs          int64
	UpdatedAtMs          int64
}

type ObjectVersion struct {
	DomainID               string
	ObjectID               string
	ObjectType             string
	Version                uint64
	BaseVersion            uint64
	ChangeSequence         uint64
	OwnerDeviceID          string
	KeyID                  string
	KeyEpoch               uint64
	Algorithm              string
	Nonce                  []byte
	EncryptedPayloadLen    int64
	CiphertextHash         string
	SignatureSchemaVersion uint16
	SignatureAlgorithm     string
	SignatureKeyID         string
	Signature              []byte
	ServerReceivedAtMs     int64
	ClientCreatedAtMs      int64
	ClientUpdatedAtMs      int64
	BlobRef                string
}

type ObjectVersionUpload struct {
	Version ObjectVersion
	Payload []byte
}

type AuditEvent struct {
	DomainID     string
	EventType    string
	DeviceID     string
	ObjectID     string
	Version      uint64
	ResultCode   string
	Bytes        int64
	ServerTimeMs int64
}
