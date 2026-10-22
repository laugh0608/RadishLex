//! Untrusted evidence syntax. Trusted catalog records are deliberately defined elsewhere.

use radishlex_ime_product_install::{ProductArtifactIdentity, ProgramComponent};
use serde::{Deserialize, Serialize};

pub(super) const FORMAT: &str = "radishlex-cancellation-qualification-evidence-v1";
pub(super) const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
pub(super) const MAX_ROWS: usize = 4096;
pub(super) const MAX_FILES: usize = 4096;
pub(super) const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
pub(super) const MAX_DEPTH: usize = 16;

macro_rules! record {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub(super) struct $name { $(pub(super) $field: $ty),* }
    };
}
macro_rules! enumeration {
    ($name:ident { $($variant:ident),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub(super) enum $name { $($variant),* }
    };
}
record!(Manifest {
    format: String,
    run_id: String,
    profile: String,
    source: ExactSource,
    environment: Environment,
    schema: SchemaMeasurement,
    collector: Collector,
    samples: Vec<Sample>,
    rows: Vec<CoverageRow>,
    files: Vec<FileEntry>,
});
record!(ExactSource {
    product: ProductArtifactIdentity,
    programs: Vec<ProgramIdentity>,
});
record!(ProgramIdentity {
    component: ProgramComponent,
    executable: ContentIdentity,
    dylibs: Vec<ContentIdentity>,
    resources: Vec<ContentIdentity>,
    signatures: Vec<ArchitectureSignature>,
});
record!(ContentIdentity {
    path: String,
    sha256: String
});
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    Arm64,
    X86_64,
}
enumeration!(SignatureKind { AdHoc, DeveloperId });
record!(ArchitectureSignature {
    architecture: Architecture,
    code_directory_sha256: String,
    designated_requirement_sha256: String,
    entitlements_sha256: String,
    signing_identifier: String,
    signature_kind: SignatureKind,
    team_identifier: Option<String>,
});
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Native,
    Translated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseSemantics {
    Sensitive,
    Insensitive,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationMethod {
    DedicatedGuest,
    DedicatedAccount,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub os_build: String,
    pub host_architecture: Architecture,
    pub process_architecture: Architecture,
    pub execution_mode: ExecutionMode,
    pub filesystem: String,
    pub case_semantics: CaseSemantics,
    pub isolation: IsolationMethod,
    pub isolation_record: String,
    pub admission_record: String,
}
record!(SourceSchema {
    version: i64,
    semantic_sha256: String
});
record!(SchemaMeasurement {
    source: SourceSchema,
    source_manifest_sha256: String,
    recipe_sha256: String,
    method: SchemaMethod,
    output: String,
    independent_observation: String,
});
enumeration!(SchemaMethod {
    IsolatedSourceSyntheticDatabase
});
record!(Collector {
    source_commit: String,
    executable_sha256: String,
    protocol: String,
    command_list_sha256: String,
    command_list: String,
});
record!(FileEntry {
    path: String,
    bytes: u64,
    sha256: String,
    role: FileRole
});
enumeration!(FileRole {
    RawObservation,
    StructuredObservation,
    SyntheticRecipe,
    SyntheticData,
    SchemaMeasurement,
    EnvironmentAdmission,
    CollectorCommands,
});
record!(Sample {
    sample_id: String,
    run_id: String,
    recipe: String,
    recipe_sha256: String,
    initial_state: LogicalState,
    initial_files: Vec<CapturedFile>,
});
record!(CapturedFile {
    path: String,
    evidence: String,
    device: u64,
    inode: u64,
    bytes: u64,
    sha256: String,
});
record!(LogicalState {
    schema: i64,
    words_sha256: String,
    tombstones_sha256: String,
    settings_sha256: String,
    offline_projection_sha256: String,
    wal_sha256: String,
});
enumeration!(CoverageGroup {
    CompatiblePostcondition,
    HistoricalCombination,
    NormalPrograms,
    DataSemantics,
    BlockingState,
    ReaderRejection,
    SubsequentRecovery,
});
enumeration!(Maintenance {
    Before,
    AfterEquivalent
});
enumeration!(OuterState {
    Completed,
    AbortedPreserved,
    RolledBack
});
enumeration!(DataPredecessor {
    Absent,
    SameOperation,
    DistinctOperation
});
enumeration!(Lifecycle { NoIndex, V1, V2 });
enumeration!(SourceFamily {
    Original,
    MaintenanceEquivalent
});
enumeration!(BlockingObject {
    None,
    PreparationMarker,
    CancellationMarker,
    IntermediateArchive,
    UnknownTemporary
});
enumeration!(RejectionFault {
    None,
    MissingFile,
    WrongDigest,
    WrongInode,
    WrongSource,
    WrongSchema,
    WrongProof,
    WrongRoot,
    WrongAncestor,
    WrongIndex,
    UnreviewedPack,
});
record!(Scenario {
    maintenance: Maintenance,
    outer_state: OuterState,
    data_predecessor: DataPredecessor,
    lifecycle: Lifecycle,
    family: SourceFamily,
    repeated_cancellation: bool,
    blocking_object: BlockingObject,
    rejection_fault: RejectionFault,
});
enumeration!(Outcome { Allowed, Rejected });
enumeration!(EndState {
    Preserved,
    LearnedAndReopened,
    BlockedWithoutWrite,
    DataTerminalReleased,
    InterruptedAndReauthorized
});
record!(CoverageRow {
    case_id: String,
    group: CoverageGroup,
    scenario: Scenario,
    run_id: String,
    sample_id: String,
    attempt: u32,
    retry_of: Option<String>,
    attempt_id: String,
    preconditions: Vec<Assertion>,
    expected: Outcome,
    actual: Outcome,
    end_state: EndState,
    observations: Vec<Observation>,
    assertions: Vec<Assertion>,
    raw_evidence: Vec<String>,
    independent_evidence: Vec<String>,
});
// This is an internal trusted policy requirement, never deserialized from a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CaseDefinition {
    pub(super) case_id: String,
    pub(super) group: CoverageGroup,
    pub(super) scenario: Scenario,
    pub(super) expected: Outcome,
    pub(super) end_state: EndState,
    pub(super) preconditions: Vec<AssertionExpectation>,
    pub(super) assertions: Vec<AssertionExpectation>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AssertionExpectation {
    pub(super) kind: AssertionKind,
    pub(super) expected: AssertionValue,
}
enumeration!(AssertionKind {
    InitialState,
    MaintenanceEquivalent,
    NoNewDataReceipt,
    OuterTerminal,
    HistoryPreserved,
    RepeatedCancellation,
    ManagerTwoGates,
    InputMethodTwoGates,
    ManagerRuntimeOpened,
    InputMethodRuntimeOpened,
    ClientSelection,
    LearningPersisted,
    ReopenedSemantics,
    WalPreserved,
    WordLearned,
    TombstonePreserved,
    SettingsPreserved,
    OfflineProjectionPreserved,
    GateDeniedBeforeWrite,
    ReaderRejected,
    NextPreparation,
    NewV1,
    DataTerminalRelease,
    ProtectionSnapshotsPreserved,
    Reauthorization,
    CheckpointRevalidation,
});
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(super) enum AssertionValue {
    State(LogicalState),
    Outcome(Outcome),
    OuterState(OuterState),
    Count(u64),
    Digest(String),
    EndState(EndState),
}
record!(Assertion {
    kind: AssertionKind,
    expected: AssertionValue,
    actual: AssertionValue,
    observation_sequences: Vec<u64>,
    evidence: Vec<String>,
});
record!(Observation {
    run_id: String,
    sample_id: String,
    sequence: u64,
    process: Option<ProcessInstance>,
    event: Event,
    evidence: String,
});
record!(ProcessInstance {
    component: ProgramComponent,
    pid: u64,
    start_id: String,
    start_sequence: u64,
    executable_sha256: String,
});
enumeration!(Gate {
    OuterInstall,
    DataUpgrade
});
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Event {
    Started {
        launch_path: String,
    },
    Gate {
        gate: Gate,
        outcome: Outcome,
    },
    RuntimeOpened {
        database: CapturedFile,
    },
    ClientInput {
        input_sha256: String,
    },
    Selection {
        selection_sha256: String,
    },
    Learned {
        state: LogicalState,
    },
    Exited {
        exit_code: i32,
    },
    Reopened {
        state: LogicalState,
    },
    Storage {
        before: Vec<CapturedFile>,
        after: Vec<CapturedFile>,
        state: LogicalState,
    },
    ReceiptState {
        new_data_receipts: u64,
        outer: OuterState,
        cancellation_count: u64,
        history_sha256: String,
        protection_snapshots_sha256: String,
    },
    ReaderResult {
        fault: RejectionFault,
        outcome: Outcome,
    },
    Transaction {
        checkpoint: TransactionCheckpoint,
        state: EndState,
    },
}
enumeration!(TransactionCheckpoint {
    Initial,
    Postcondition,
    NextPreparation,
    NewV1,
    DataTerminalRelease,
    BeforeMarkerMove,
    AfterMarkerMove,
    AfterMarkerSync,
    Reauthorized,
    Released,
});
