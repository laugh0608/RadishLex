mod assemble;
mod device;
mod http_transport;
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
pub use http_transport::HttpSyncRemoteTransport;
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
    ProductSyncCryptoProvider, SyncCryptoLoadError, SyncDeviceSigningBackend,
    SyncEpochMaterialStore, SyncTrustedDeviceProfile, SyncTrustedDeviceSource,
    SyncTrustedDomainState,
};
pub use remote::{
    LatestObjectConflictMetadata, OpaqueSyncCursor, RemoteObjectDiscoveryPage, RemoteObjectPayload,
    RemoteObjectVersion, SyncRemoteClient, SyncRemoteError, SyncRemoteMethod, SyncRemoteRequest,
    SyncRemoteResponse, SyncRemoteTransport, SyncServerErrorCode,
};
pub use service::{SyncObjectProcessor, SyncOnceConfig, SyncOrchestrationService};
pub use signing::{SignedDeviceAuthorization, SignedDeviceRevocation};
