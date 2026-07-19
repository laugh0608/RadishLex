mod assemble;
mod device;
mod epoch_distribution;
mod http_transport;
mod lifecycle;
mod merge;
mod model;
mod orchestration;
mod processor;
mod product_provider;
mod remote;
mod service;
mod signing;

pub use assemble::{
    AssembledSyncObject, PlaintextSyncPayload, SyncEnvelopeAssembler, SyncObjectAssemblySpec,
};
pub use device::{
    DeviceAuthorizationPackage, DeviceJoinRequest, DeviceRevocationReason, DeviceRevocationRecord,
    SyncDevice, SyncDeviceStatus, SyncDomain, SyncObjectVersion,
};
pub use epoch_distribution::{
    validate_epoch_distribution_batch, SignedEpochDistribution, MAX_EPOCH_DISTRIBUTION_RECORDS,
};
pub use http_transport::HttpSyncRemoteTransport;
pub use lifecycle::{
    device_join_profile_challenge, verify_lifecycle_snapshot, SyncLifecycleError,
    VerifiedSyncLifecycle,
};
pub use merge::{
    ClientSyncMergeInput, ClientSyncMergeResult, DictionaryDeletedTermMergeRecord,
    DictionaryUserTermMergeRecord, RankerWeightMergeRecord, SyncMergeDecision,
    SyncMergeDecisionKind, SyncRankerWeightIdentity, SyncTermIdentity, UserTermMergeIntent,
};
pub use model::{
    EncryptedSyncObjectDraft, LocalDataClass, PayloadSource, SyncObjectType, SyncPayloadError,
    SyncPayloadPlan, SyncPlanItem,
};
pub use orchestration::{
    DecryptedSyncObject, LocalSyncSnapshot, PreparedSyncOutbox, SyncApplyPageSummary,
    SyncCycleOutcome, SyncCyclePhase, SyncCycleSummary, SyncLocalRepository,
    SyncOrchestrationError, SyncOrchestrationErrorCode,
};
pub use processor::{
    DefaultSyncObjectProcessor, SyncCryptoCycleSnapshot, SyncCryptoProvider, SyncEpochKeyMaterial,
    SyncRemoteSigningProfile, UnavailableSyncCryptoProvider,
};
pub use product_provider::{
    ProductSyncCryptoProvider, ProductWrappedEpochMaterialStore, SyncCryptoLoadError,
    SyncDeviceKeyAgreementBackend, SyncDeviceSigningBackend, SyncEpochMaterialStore,
    SyncTrustedDeviceProfile, SyncTrustedDeviceSource, SyncTrustedDomainState,
    SyncWrappedEpochMaterialSource,
};
pub use remote::{
    LatestObjectConflictMetadata, OpaqueSyncCursor, RemoteDeviceAuthorization,
    RemoteDeviceRevocation, RemoteEpochDistributionResult, RemoteLifecycleDevice,
    RemoteLifecycleEvent, RemoteLifecycleEventKind, RemoteLifecyclePage, RemoteLifecycleSnapshot,
    RemoteObjectDiscoveryPage, RemoteObjectPayload, RemoteObjectVersion,
    RemoteVerifiedRecoveryRecord, RemoteWrappedEpochLocator, RemoteWrappedEpochMaterialSource,
    SyncRemoteClient, SyncRemoteError, SyncRemoteMethod, SyncRemoteRequest, SyncRemoteResponse,
    SyncRemoteTransport, SyncServerErrorCode,
};
pub use service::{SyncObjectProcessor, SyncOnceConfig, SyncOrchestrationService};
pub use signing::{SignedDeviceAuthorization, SignedDeviceRevocation};
