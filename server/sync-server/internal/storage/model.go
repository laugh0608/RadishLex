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
	RecoveryRecordActive  RecoveryRecordStatus = "active"
	RecoveryRecordRevoked RecoveryRecordStatus = "revoked"
)

const (
	ObjectDictionaryUserTerms    = "dictionary.user_terms"
	ObjectDictionaryDeletedTerms = "dictionary.deleted_terms"
	ObjectRankerWeights          = "ranker.weights"
	ObjectSettingsProfile        = "settings.profile"
	ObjectSettingsSchema         = "settings.schema"
	ObjectBackupSnapshot         = "backup.snapshot"

	AlgorithmXChaCha20Poly1305HKDFSHA256 = "xchacha20poly1305-hkdf-sha256-v1"
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
	DomainID           string
	RecipientDeviceID  string
	AuthorizerDeviceID string
	KeyEpoch           uint64
	WrappingKeyID      string
	Algorithm          string
	Nonce              []byte
	WrappedKeyLen      int64
	CiphertextHash     string
	CreatedAtMs        int64
	Signature          []byte
}

type DeviceAuthorization struct {
	DomainID                    string
	JoinRequestID               string
	AuthorizerDeviceID          string
	RecipientDeviceID           string
	RecipientSigningPublicKeyID string
	RecipientKeyAgreementKeyID  string
	KeyEpoch                    uint64
	CreatedAtMs                 int64
	Signature                   []byte
}

type DeviceRevocation struct {
	DomainID         string
	RevokedDeviceID  string
	RevokerDeviceID  string
	PreviousKeyEpoch uint64
	NewKeyEpoch      uint64
	Reason           string
	CreatedAtMs      int64
	Signature        []byte
}

type RecoveryRecord struct {
	DomainID           string
	RecoveryRecordID   string
	KeyEpoch           uint64
	KDFProfile         string
	Salt               []byte
	Algorithm          string
	Nonce              []byte
	WrappedMaterialLen int64
	CiphertextHash     string
	Status             RecoveryRecordStatus
	CreatedAtMs        int64
	RevokedAtMs        int64
	SignerDeviceID     string
	Signature          []byte
	BlobRef            string
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
	CreatedAtMs          int64
	UpdatedAtMs          int64
}

type ObjectVersion struct {
	DomainID            string
	ObjectID            string
	ObjectType          string
	Version             uint64
	BaseVersion         uint64
	OwnerDeviceID       string
	KeyID               string
	KeyEpoch            uint64
	Algorithm           string
	Nonce               []byte
	EncryptedPayloadLen int64
	CiphertextHash      string
	Signature           []byte
	ServerReceivedAtMs  int64
	ClientCreatedAtMs   int64
	ClientUpdatedAtMs   int64
	BlobRef             string
}

type ObjectVersionUpload struct {
	Version ObjectVersion
	Payload []byte
}
