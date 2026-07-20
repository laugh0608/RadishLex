use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

use radishlex_ime_sync::{
    DefaultSyncObjectProcessor, HttpSyncRemoteTransport, SyncCycleOutcome, SyncCycleSummary,
    SyncLocalRepository, SyncObjectProcessor, SyncOnceConfig, SyncOrchestrationError,
    SyncOrchestrationErrorCode, SyncOrchestrationService, SyncRemoteClient,
};
use radishlex_ime_userdb::{TermSource, UserDb};

use crate::crypto_provider::{QualificationCryptoProvider, QualificationSigningMaterial};
use crate::remote_setup::{
    authorize_second_device, build_transport, create_domain, QualificationIds,
};
use crate::{
    QualificationError, QualificationErrorCode, QualificationPhase, QualificationRequest,
    QualificationRunSnapshot, QualificationRunState,
};

static ACTIVE_RUN: AtomicBool = AtomicBool::new(false);
static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const WORKSPACE_PREFIX: &str = "radishlex-sync-qualification-";
const WORKSPACE_MARKER: &str = ".radishlex-sync-qualification.v1";
const WORKSPACE_MARKER_CONTENT: &[u8] = b"radishlex-sync-qualification.v1\n";

pub struct QualificationRun {
    shared: Arc<SharedRun>,
    worker: Option<JoinHandle<()>>,
}

impl QualificationRun {
    pub fn start(request: QualificationRequest) -> Result<Self, QualificationError> {
        if ACTIVE_RUN
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(QualificationError::new(
                QualificationErrorCode::AlreadyRunning,
                QualificationPhase::ValidateRequest,
                false,
            ));
        }
        let process_guard = match ProcessRunGuard::acquire() {
            Ok(guard) => guard,
            Err(error) => {
                ACTIVE_RUN.store(false, Ordering::Release);
                return Err(error);
            }
        };
        let shared = Arc::new(SharedRun::new());
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("radishlex-sync-qualification".to_owned())
            .spawn(move || worker_main(request, worker_shared, process_guard));
        match worker {
            Ok(worker) => Ok(Self {
                shared,
                worker: Some(worker),
            }),
            Err(_) => {
                ACTIVE_RUN.store(false, Ordering::Release);
                Err(QualificationError::new(
                    QualificationErrorCode::Internal,
                    QualificationPhase::Created,
                    false,
                ))
            }
        }
    }

    pub fn poll(&self) -> QualificationRunSnapshot {
        self.shared.snapshot()
    }

    pub fn cancel(&self) -> bool {
        let requested = self.shared.request_cancel();
        if !requested {
            return false;
        }
        for target in self.shared.cancel_targets() {
            if target.path.exists() {
                if let Ok(database) = UserDb::open(&target.path) {
                    let _ = database.request_sync_cycle_cancel(&target.domain_id);
                }
            }
        }
        true
    }

    pub fn wait(mut self) -> QualificationRunSnapshot {
        self.join_worker();
        self.shared.snapshot()
    }

    fn join_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for QualificationRun {
    fn drop(&mut self) {
        if !self.shared.snapshot().state.is_terminal() {
            let _ = self.cancel();
        }
        self.join_worker();
    }
}

struct ActiveRunGuard;

impl Drop for ActiveRunGuard {
    fn drop(&mut self) {
        ACTIVE_RUN.store(false, Ordering::Release);
    }
}

#[cfg(unix)]
struct ProcessRunGuard {
    _listener: UnixListener,
    socket_path: PathBuf,
    socket_device: u64,
    socket_inode: u64,
}

#[cfg(unix)]
impl ProcessRunGuard {
    fn acquire() -> Result<Self, QualificationError> {
        let temp_root = std::env::temp_dir();
        let socket_path = temp_root.join("rlx-sync-q-v1.sock");
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                match UnixStream::connect(&socket_path) {
                    Ok(_) => return Err(already_running_error()),
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {}
                    Err(_) => return Err(already_running_error()),
                }
                remove_stale_socket(&temp_root, &socket_path)?;
                UnixListener::bind(&socket_path)
                    .map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?
            }
            Err(_) => return Err(local_error(QualificationPhase::PrepareWorkspace)),
        };
        let metadata = fs::symlink_metadata(&socket_path)
            .map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?;
        cleanup_stale_workspaces(&temp_root)?;
        Ok(Self {
            _listener: listener,
            socket_path,
            socket_device: metadata.dev(),
            socket_inode: metadata.ino(),
        })
    }
}

#[cfg(unix)]
impl Drop for ProcessRunGuard {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.socket_path) else {
            return;
        };
        if metadata.file_type().is_socket()
            && metadata.dev() == self.socket_device
            && metadata.ino() == self.socket_inode
        {
            let _ = fs::remove_file(&self.socket_path);
        }
    }
}

#[cfg(not(unix))]
struct ProcessRunGuard;

#[cfg(not(unix))]
impl ProcessRunGuard {
    fn acquire() -> Result<Self, QualificationError> {
        Ok(Self)
    }
}

#[derive(Clone)]
struct CancelTarget {
    path: PathBuf,
    domain_id: String,
}

struct SharedRun {
    snapshot: Mutex<QualificationRunSnapshot>,
    cancel_requested: AtomicBool,
    cancel_targets: Mutex<Vec<CancelTarget>>,
}

impl SharedRun {
    fn new() -> Self {
        let snapshot = QualificationRunSnapshot {
            state: QualificationRunState::Running,
            phase: QualificationPhase::ValidateRequest,
            ..QualificationRunSnapshot::default()
        };
        Self {
            snapshot: Mutex::new(snapshot),
            cancel_requested: AtomicBool::new(false),
            cancel_targets: Mutex::new(Vec::new()),
        }
    }

    fn snapshot(&self) -> QualificationRunSnapshot {
        lock(&self.snapshot).clone()
    }

    fn set_phase(&self, phase: QualificationPhase) {
        let mut snapshot = lock(&self.snapshot);
        if !snapshot.state.is_terminal() {
            if snapshot.state != QualificationRunState::Cancelling {
                snapshot.state = QualificationRunState::Running;
            }
            snapshot.phase = phase;
        }
    }

    fn add_summary(&self, summary: &SyncCycleSummary) {
        let mut snapshot = lock(&self.snapshot);
        snapshot.discovered = snapshot.discovered.saturating_add(summary.discovered);
        snapshot.downloaded = snapshot.downloaded.saturating_add(summary.downloaded);
        snapshot.applied = snapshot.applied.saturating_add(summary.applied);
        snapshot.uploaded = snapshot.uploaded.saturating_add(summary.uploaded);
        snapshot.conflicts = snapshot.conflicts.saturating_add(summary.conflicts);
        snapshot.retries = snapshot.retries.saturating_add(summary.retries);
    }

    fn set_convergence_rounds(&self, rounds: usize) {
        lock(&self.snapshot).convergence_rounds = rounds;
    }

    fn request_cancel(&self) -> bool {
        let mut snapshot = lock(&self.snapshot);
        if snapshot.state.is_terminal() {
            return false;
        }
        let first_request = !self.cancel_requested.swap(true, Ordering::AcqRel);
        snapshot.state = QualificationRunState::Cancelling;
        first_request
    }

    fn is_cancel_requested(&self) -> bool {
        self.cancel_requested.load(Ordering::Acquire)
    }

    fn register_cancel_targets(&self, targets: Vec<CancelTarget>) {
        *lock(&self.cancel_targets) = targets;
    }

    fn clear_cancel_targets(&self) {
        lock(&self.cancel_targets).clear();
    }

    fn cancel_targets(&self) -> Vec<CancelTarget> {
        lock(&self.cancel_targets).clone()
    }

    fn finish(&self, result: Result<(), QualificationError>, temporary_files_cleaned: bool) {
        let mut snapshot = lock(&self.snapshot);
        snapshot.temporary_files_cleaned = temporary_files_cleaned;
        snapshot.worker_stopped = true;
        snapshot.transient_inputs_cleared = true;
        match result {
            Ok(()) if !self.is_cancel_requested() => {
                snapshot.state = QualificationRunState::Completed;
                snapshot.phase = QualificationPhase::Complete;
                snapshot.error = None;
            }
            Ok(()) => {
                snapshot.state = QualificationRunState::Cancelled;
                snapshot.error = Some(cancelled_error(snapshot.phase));
            }
            Err(error)
                if error.code == QualificationErrorCode::Cancelled
                    || self.is_cancel_requested() =>
            {
                snapshot.state = QualificationRunState::Cancelled;
                snapshot.phase = error.phase;
                snapshot.error = Some(cancelled_error(error.phase));
            }
            Err(error) => {
                snapshot.state = QualificationRunState::Failed;
                snapshot.phase = error.phase;
                snapshot.error = Some(error);
            }
        }
    }

    fn finish_after_panic(&self) {
        let mut snapshot = lock(&self.snapshot);
        snapshot.state = QualificationRunState::Failed;
        snapshot.worker_stopped = true;
        snapshot.transient_inputs_cleared = true;
        snapshot.error = Some(QualificationError::new(
            QualificationErrorCode::Internal,
            snapshot.phase,
            false,
        ));
    }
}

fn worker_main(
    request: QualificationRequest,
    shared: Arc<SharedRun>,
    _process_guard: ProcessRunGuard,
) {
    let _active_guard = ActiveRunGuard;
    let result = catch_unwind(AssertUnwindSafe(|| run_worker(request, &shared)));
    if result.is_err() {
        shared.clear_cancel_targets();
        shared.finish_after_panic();
    }
}

fn run_worker(request: QualificationRequest, shared: &SharedRun) {
    let started = Instant::now();
    let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    shared.set_phase(QualificationPhase::PrepareWorkspace);
    let workspace = match TempWorkspace::new(sequence) {
        Ok(workspace) => workspace,
        Err(error) => {
            drop(request);
            shared.finish(Err(error), true);
            return;
        }
    };
    let result = execute(&request, shared, &workspace, sequence, started);
    shared.clear_cancel_targets();
    shared.set_phase(QualificationPhase::Cleanup);
    let temporary_files_cleaned = workspace.cleanup();
    drop(request);
    shared.finish(result, temporary_files_cleaned);
}

fn execute(
    request: &QualificationRequest,
    shared: &SharedRun,
    workspace: &TempWorkspace,
    sequence: u64,
    started: Instant,
) -> Result<(), QualificationError> {
    check_control(
        shared,
        started,
        request.timeout(),
        QualificationPhase::PrepareWorkspace,
    )?;
    let created_at_ms = current_time_ms()?;
    let ids = QualificationIds::new(sequence, created_at_ms);
    let signing = QualificationSigningMaterial::new(
        &ids.device_a_id,
        &ids.signing_key_a_id,
        &ids.device_b_id,
        &ids.signing_key_b_id,
        ids.created_at_ms,
    )?;
    let transport = build_transport(request)?;

    shared.set_phase(QualificationPhase::CreateDomain);
    check_control(
        shared,
        started,
        request.timeout(),
        QualificationPhase::CreateDomain,
    )?;
    create_domain(&transport, &ids, &signing.public_key_a)?;

    shared.set_phase(QualificationPhase::AuthorizeSecondDevice);
    check_control(
        shared,
        started,
        request.timeout(),
        QualificationPhase::AuthorizeSecondDevice,
    )?;
    authorize_second_device(&transport, &ids, &signing.store, &signing.public_key_b)?;

    let device_a_path = workspace.path.join("client-a.sqlite");
    let device_b_path = workspace.path.join("client-b.sqlite");
    let mut device_a = open_userdb(&device_a_path, QualificationPhase::PrepareWorkspace)?;
    let mut device_b = open_userdb(&device_b_path, QualificationPhase::PrepareWorkspace)?;
    shared.register_cancel_targets(vec![
        CancelTarget {
            path: device_a_path,
            domain_id: ids.domain_id.clone(),
        },
        CancelTarget {
            path: device_b_path,
            domain_id: ids.domain_id.clone(),
        },
    ]);
    seed_client_a(&mut device_a)?;
    seed_client_b(&mut device_b)?;

    let mut service_a = sync_service(request, &ids, true)?;
    let mut service_b = sync_service(request, &ids, false)?;

    shared.set_phase(QualificationPhase::ClientAFirstSync);
    check_control(
        shared,
        started,
        request.timeout(),
        QualificationPhase::ClientAFirstSync,
    )?;
    let first_a = service_a.sync_once(&mut device_a, &ids.domain_id, ids.created_at_ms + 1_000);
    accept_summary(shared, &first_a, QualificationPhase::ClientAFirstSync)?;
    if first_a.uploaded == 0 {
        return Err(convergence_error(QualificationPhase::ClientAFirstSync));
    }

    shared.set_phase(QualificationPhase::PrepareConflict);
    device_a
        .add_term(
            "qualificationconflict",
            "合成资格冲突词",
            Some("qualification conflict"),
            TermSource::ManualAdd,
        )
        .map_err(|_| local_error(QualificationPhase::PrepareConflict))?;
    prepare_stale_outboxes(&mut device_a, &ids, ids.created_at_ms + 2_000)?;

    shared.set_phase(QualificationPhase::ClientBMerge);
    check_control(
        shared,
        started,
        request.timeout(),
        QualificationPhase::ClientBMerge,
    )?;
    let first_b = service_b.sync_once(&mut device_b, &ids.domain_id, ids.created_at_ms + 3_000);
    accept_summary(shared, &first_b, QualificationPhase::ClientBMerge)?;
    if first_b.downloaded == 0 || first_b.uploaded == 0 {
        return Err(convergence_error(QualificationPhase::ClientBMerge));
    }

    shared.set_phase(QualificationPhase::ClientAConflictRecovery);
    check_control(
        shared,
        started,
        request.timeout(),
        QualificationPhase::ClientAConflictRecovery,
    )?;
    let conflict_a = service_a.sync_once(&mut device_a, &ids.domain_id, ids.created_at_ms + 4_000);
    accept_summary(
        shared,
        &conflict_a,
        QualificationPhase::ClientAConflictRecovery,
    )?;
    if conflict_a.conflicts == 0 || conflict_a.uploaded == 0 {
        return Err(QualificationError::new(
            QualificationErrorCode::ConflictNotObserved,
            QualificationPhase::ClientAConflictRecovery,
            false,
        ));
    }

    shared.set_phase(QualificationPhase::VerifyConvergence);
    check_control(
        shared,
        started,
        request.timeout(),
        QualificationPhase::VerifyConvergence,
    )?;
    let mut final_uploads = (usize::MAX, usize::MAX);
    for round in 0_i64..2 {
        check_control(
            shared,
            started,
            request.timeout(),
            QualificationPhase::VerifyConvergence,
        )?;
        let round_offset = round * 2_000;
        let converge_b = service_b.sync_once(
            &mut device_b,
            &ids.domain_id,
            ids.created_at_ms + 5_000 + round_offset,
        );
        accept_summary(shared, &converge_b, QualificationPhase::VerifyConvergence)?;
        let converge_a = service_a.sync_once(
            &mut device_a,
            &ids.domain_id,
            ids.created_at_ms + 6_000 + round_offset,
        );
        accept_summary(shared, &converge_a, QualificationPhase::VerifyConvergence)?;
        final_uploads = (converge_a.uploaded, converge_b.uploaded);
    }
    shared.set_convergence_rounds(2);
    if final_uploads != (0, 0) {
        return Err(convergence_error(QualificationPhase::VerifyConvergence));
    }
    assert_converged(&device_a)?;
    assert_converged(&device_b)?;
    Ok(())
}

fn sync_service(
    request: &QualificationRequest,
    ids: &QualificationIds,
    client_a: bool,
) -> Result<
    SyncOrchestrationService<
        HttpSyncRemoteTransport,
        DefaultSyncObjectProcessor<QualificationCryptoProvider>,
    >,
    QualificationError,
> {
    let provider = QualificationCryptoProvider::new(ids, client_a)?;
    SyncOrchestrationService::new(
        SyncRemoteClient::new(build_transport(request)?),
        DefaultSyncObjectProcessor::qualification(provider),
        SyncOnceConfig::default(),
    )
    .map_err(|_| protocol_error(QualificationPhase::PrepareWorkspace))
}

fn prepare_stale_outboxes(
    database: &mut UserDb,
    ids: &QualificationIds,
    prepared_at_ms: i64,
) -> Result<(), QualificationError> {
    let provider = QualificationCryptoProvider::new(ids, true)?;
    let mut processor = DefaultSyncObjectProcessor::qualification(provider);
    processor
        .preflight(&ids.domain_id)
        .map_err(map_orchestration_error)?;
    let snapshots = database
        .outbound_snapshots(&ids.domain_id)
        .map_err(map_orchestration_error)?;
    if snapshots.is_empty() {
        return Err(convergence_error(QualificationPhase::PrepareConflict));
    }
    for snapshot in snapshots {
        // Reserve the next version for client B. Client A keeps base version 1
        // but prepares version 3, so B's version 2 upload produces the exact
        // stale-base conflict that sync_once is specified to recover.
        let version = snapshot.remote_base_version.unwrap_or(0).saturating_add(2);
        let outbox = processor
            .prepare_outbox(snapshot, version, prepared_at_ms)
            .map_err(map_orchestration_error)?;
        database
            .store_prepared_outbox(&outbox)
            .map_err(map_orchestration_error)?;
    }
    Ok(())
}

fn accept_summary(
    shared: &SharedRun,
    summary: &SyncCycleSummary,
    phase: QualificationPhase,
) -> Result<(), QualificationError> {
    shared.add_summary(summary);
    match summary.outcome {
        SyncCycleOutcome::Completed => Ok(()),
        SyncCycleOutcome::Cancelled => Err(cancelled_error(phase)),
        SyncCycleOutcome::Blocked | SyncCycleOutcome::Failed => Err(summary
            .error
            .as_ref()
            .map(map_orchestration_error_ref)
            .unwrap_or_else(|| protocol_error(phase))),
    }
}

fn seed_client_a(database: &mut UserDb) -> Result<(), QualificationError> {
    database
        .add_term(
            "qualificationa",
            "合成资格甲词",
            Some("qualification a"),
            TermSource::ManualAdd,
        )
        .map(|_| ())
        .map_err(|_| local_error(QualificationPhase::PrepareWorkspace))
}

fn seed_client_b(database: &mut UserDb) -> Result<(), QualificationError> {
    database
        .add_term(
            "qualificationb",
            "合成资格乙词",
            Some("qualification b"),
            TermSource::ManualAdd,
        )
        .map(|_| ())
        .map_err(|_| local_error(QualificationPhase::PrepareWorkspace))
}

fn assert_converged(database: &UserDb) -> Result<(), QualificationError> {
    let terms = database
        .list_active_terms()
        .map_err(|_| local_error(QualificationPhase::VerifyConvergence))?;
    for expected in ["合成资格甲词", "合成资格乙词", "合成资格冲突词"] {
        if !terms.iter().any(|term| term.text == expected) {
            return Err(convergence_error(QualificationPhase::VerifyConvergence));
        }
    }
    Ok(())
}

fn check_control(
    shared: &SharedRun,
    started: Instant,
    timeout: Duration,
    phase: QualificationPhase,
) -> Result<(), QualificationError> {
    if shared.is_cancel_requested() {
        return Err(cancelled_error(phase));
    }
    if started.elapsed() >= timeout {
        return Err(QualificationError::new(
            QualificationErrorCode::TransportTimeout,
            phase,
            true,
        ));
    }
    Ok(())
}

fn current_time_ms() -> Result<i64, QualificationError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| internal_error(QualificationPhase::PrepareWorkspace))?
        .as_millis();
    i64::try_from(millis).map_err(|_| internal_error(QualificationPhase::PrepareWorkspace))
}

fn open_userdb(path: &Path, phase: QualificationPhase) -> Result<UserDb, QualificationError> {
    UserDb::open(path).map_err(|_| local_error(phase))
}

fn map_orchestration_error(error: SyncOrchestrationError) -> QualificationError {
    map_orchestration_error_ref(&error)
}

fn map_orchestration_error_ref(error: &SyncOrchestrationError) -> QualificationError {
    let code = match error.code {
        SyncOrchestrationErrorCode::Unauthenticated => QualificationErrorCode::Unauthenticated,
        SyncOrchestrationErrorCode::TransportTimeout => QualificationErrorCode::TransportTimeout,
        SyncOrchestrationErrorCode::ServerUnavailable => QualificationErrorCode::ServerUnavailable,
        SyncOrchestrationErrorCode::Cancelled => QualificationErrorCode::Cancelled,
        SyncOrchestrationErrorCode::LocalTransactionFailed
        | SyncOrchestrationErrorCode::CursorInvalid => QualificationErrorCode::LocalStorageFailed,
        SyncOrchestrationErrorCode::BackendUnavailable
        | SyncOrchestrationErrorCode::KeyEpochRejected
        | SyncOrchestrationErrorCode::SignatureMismatch
        | SyncOrchestrationErrorCode::CiphertextHashMismatch
        | SyncOrchestrationErrorCode::AadMismatch
        | SyncOrchestrationErrorCode::DecryptFailed => QualificationErrorCode::CryptoRejected,
        _ => QualificationErrorCode::ProtocolRejected,
    };
    QualificationError::new(code, map_sync_phase(error.phase), error.retryable)
}

fn map_sync_phase(phase: radishlex_ime_sync::SyncCyclePhase) -> QualificationPhase {
    use radishlex_ime_sync::SyncCyclePhase;
    match phase {
        SyncCyclePhase::Preflight => QualificationPhase::PrepareWorkspace,
        SyncCyclePhase::Discover
        | SyncCyclePhase::Download
        | SyncCyclePhase::Verify
        | SyncCyclePhase::DecryptAndDecode
        | SyncCyclePhase::ApplyAndAdvanceCursor
        | SyncCyclePhase::PlanUpload
        | SyncCyclePhase::PrepareSignedOutbox
        | SyncCyclePhase::Upload
        | SyncCyclePhase::AcknowledgeOutbox
        | SyncCyclePhase::Complete => QualificationPhase::VerifyConvergence,
    }
}

fn cancelled_error(phase: QualificationPhase) -> QualificationError {
    QualificationError::new(QualificationErrorCode::Cancelled, phase, false)
}

fn local_error(phase: QualificationPhase) -> QualificationError {
    QualificationError::new(QualificationErrorCode::LocalStorageFailed, phase, false)
}

fn protocol_error(phase: QualificationPhase) -> QualificationError {
    QualificationError::new(QualificationErrorCode::ProtocolRejected, phase, false)
}

fn convergence_error(phase: QualificationPhase) -> QualificationError {
    QualificationError::new(QualificationErrorCode::ConvergenceFailed, phase, false)
}

fn internal_error(phase: QualificationPhase) -> QualificationError {
    QualificationError::new(QualificationErrorCode::Internal, phase, false)
}

struct TempWorkspace {
    path: PathBuf,
    cleaned: bool,
}

impl TempWorkspace {
    fn new(sequence: u64) -> Result<Self, QualificationError> {
        Self::new_in(&std::env::temp_dir(), sequence)
    }

    fn new_in(root: &Path, sequence: u64) -> Result<Self, QualificationError> {
        let path = root.join(format!(
            "{WORKSPACE_PREFIX}{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).is_err() {
                let _ = fs::remove_dir_all(&path);
                return Err(local_error(QualificationPhase::PrepareWorkspace));
            }
        }
        if fs::write(path.join(WORKSPACE_MARKER), WORKSPACE_MARKER_CONTENT).is_err() {
            let _ = fs::remove_dir_all(&path);
            return Err(local_error(QualificationPhase::PrepareWorkspace));
        }
        Ok(Self {
            path,
            cleaned: false,
        })
    }

    fn cleanup(mut self) -> bool {
        let removed = fs::remove_dir_all(&self.path).is_ok() && !self.path.exists();
        self.cleaned = removed;
        removed
    }
}

#[cfg(unix)]
fn remove_stale_socket(root: &Path, socket_path: &Path) -> Result<(), QualificationError> {
    let root_metadata =
        fs::metadata(root).map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?;
    let socket_metadata = fs::symlink_metadata(socket_path)
        .map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?;
    if !socket_metadata.file_type().is_socket() || socket_metadata.uid() != root_metadata.uid() {
        return Err(already_running_error());
    }
    fs::remove_file(socket_path).map_err(|_| local_error(QualificationPhase::PrepareWorkspace))
}

#[cfg(unix)]
fn cleanup_stale_workspaces(root: &Path) -> Result<(), QualificationError> {
    let root_metadata =
        fs::metadata(root).map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?;
    let entries =
        fs::read_dir(root).map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?;
    for entry in entries {
        let entry = entry.map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !valid_workspace_name(name) {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?;
        if !metadata.is_dir() || metadata.uid() != root_metadata.uid() {
            continue;
        }
        let marker_path = path.join(WORKSPACE_MARKER);
        let Ok(marker_metadata) = fs::symlink_metadata(&marker_path) else {
            continue;
        };
        if !marker_metadata.is_file() || marker_metadata.uid() != root_metadata.uid() {
            continue;
        }
        let Ok(marker) = fs::read(&marker_path) else {
            continue;
        };
        if marker == WORKSPACE_MARKER_CONTENT {
            fs::remove_dir_all(&path)
                .map_err(|_| local_error(QualificationPhase::PrepareWorkspace))?;
        }
    }
    Ok(())
}

fn valid_workspace_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix(WORKSPACE_PREFIX) else {
        return false;
    };
    let Some((pid, sequence)) = suffix.split_once('-') else {
        return false;
    };
    !pid.is_empty()
        && !sequence.is_empty()
        && pid.bytes().all(|byte| byte.is_ascii_digit())
        && sequence.bytes().all(|byte| byte.is_ascii_digit())
}

fn already_running_error() -> QualificationError {
    QualificationError::new(
        QualificationErrorCode::AlreadyRunning,
        QualificationPhase::ValidateRequest,
        false,
    )
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    use super::*;

    #[test]
    fn concurrent_start_is_rejected_and_cancel_cleans_the_active_run() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stalled TLS peer");
        let port = listener.local_addr().expect("listener address").port();
        let (accepted_tx, accepted_rx) = mpsc::channel();
        let peer = thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept qualification client");
            accepted_tx.send(()).expect("report accepted client");
            thread::sleep(Duration::from_secs(1));
        });
        let endpoint = format!("https://127.0.0.1:{port}");
        let request =
            QualificationRequest::new(&endpoint, "z".repeat(48), None, 500).expect("request");
        let run = QualificationRun::start(request).expect("run");
        let duplicate = QualificationRequest::new(&endpoint, "y".repeat(48), None, 500)
            .expect("duplicate request");
        let duplicate_error = QualificationRun::start(duplicate)
            .err()
            .expect("concurrent run is rejected");
        assert_eq!(duplicate_error.code, QualificationErrorCode::AlreadyRunning);
        accepted_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("qualification client reaches stalled TLS peer");
        assert!(run.cancel());
        assert!(!run.cancel());
        let deadline = Instant::now() + Duration::from_secs(5);
        let snapshot = loop {
            let snapshot = run.poll();
            if snapshot.state.is_terminal() {
                break snapshot;
            }
            assert!(Instant::now() < deadline, "qualification run timed out");
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(snapshot.state, QualificationRunState::Cancelled);
        assert!(snapshot.temporary_files_cleaned);
        assert!(snapshot.worker_stopped);
        assert!(snapshot.transient_inputs_cleared);
        assert_eq!(
            snapshot.error.map(|error| error.code),
            Some(QualificationErrorCode::Cancelled)
        );
        peer.join().expect("stalled TLS peer exits");
    }

    #[cfg(unix)]
    #[test]
    fn restart_cleanup_removes_only_exact_marked_workspaces() {
        let root = std::env::temp_dir().join(format!(
            "radishlex-sync-qualification-cleanup-test-{}-{}",
            std::process::id(),
            RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("cleanup test root");
        let stale = TempWorkspace::new_in(&root, 77).expect("stale workspace");
        let stale_path = stale.path.clone();
        std::mem::forget(stale);
        let unmarked = root.join(format!("{WORKSPACE_PREFIX}999-78"));
        fs::create_dir(&unmarked).expect("unmarked directory");
        let unrelated = root.join("unrelated");
        fs::create_dir(&unrelated).expect("unrelated directory");

        cleanup_stale_workspaces(&root).expect("restart cleanup");

        assert!(!stale_path.exists());
        assert!(unmarked.exists());
        assert!(unrelated.exists());
        fs::remove_dir_all(&root).expect("cleanup test root removed");
    }
}
