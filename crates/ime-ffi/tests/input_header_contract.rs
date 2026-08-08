use std::mem::size_of;

use radishlex_ime_ffi::{
    radishlex_apple_p256_product_smoke, radishlex_apple_p256_product_status,
    radishlex_apple_secure_enclave_key_agreement_product_smoke,
    radishlex_apple_secure_enclave_key_agreement_product_status,
    radishlex_apple_secure_enclave_p256_product_smoke,
    radishlex_apple_secure_enclave_p256_product_status, radishlex_error_free,
    radishlex_key_result_commit, radishlex_key_result_commit_present,
    radishlex_key_result_consumed, radishlex_key_result_free,
    radishlex_key_result_learning_disposition, radishlex_key_result_snapshot,
    radishlex_key_result_version, radishlex_linux_product_startup_gate,
    radishlex_manager_sync_product_status, radishlex_manager_sync_qualification_cancel,
    radishlex_manager_sync_qualification_free, radishlex_manager_sync_qualification_poll,
    radishlex_manager_sync_qualification_start, radishlex_product_install_startup_gate,
    radishlex_rime_runtime_shutdown, radishlex_session_handle_key_event,
    RadishLexAppleP256ProductSmokeSummary, RadishLexAppleP256ProductStatus,
    RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
    RadishLexAppleSecureEnclaveKeyAgreementProductStatus, RadishLexError, RadishLexFfiContract,
    RadishLexKeyEvent, RadishLexKeyResult, RadishLexLinuxProductStartupRequest,
    RadishLexLinuxProductStartupResult, RadishLexManagerSyncProductStatus,
    RadishLexManagerSyncQualificationRequest, RadishLexManagerSyncQualificationRun,
    RadishLexManagerSyncQualificationSnapshot, RadishLexProductInstallStartupGateRequest,
    RadishLexProductInstallStartupGateResult, RadishLexSession, RadishLexSessionOptions,
    RadishLexSnapshot, RadishLexStatusCode, RadishLexStringView, RADISHLEX_ABI_CONTRACT_VERSION,
    RADISHLEX_INSTALL_RECEIPT_STATE_ABORTED_PRESERVED, RADISHLEX_INSTALL_RECEIPT_STATE_COMPLETED,
    RADISHLEX_INSTALL_RECEIPT_STATE_ROLLED_BACK, RADISHLEX_KEY_RESULT_VERSION,
    RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_NONE, RADISHLEX_STARTUP_GATE_ERROR_NONE,
    RADISHLEX_UPGRADE_RECEIPT_STATE_ABORTED_PRESERVED, RADISHLEX_UPGRADE_RECEIPT_STATE_COMPLETED,
    RADISHLEX_UPGRADE_RECEIPT_STATE_ROLLED_BACK,
};

#[test]
fn rust_input_abi_layout_matches_the_checked_header_contract() {
    assert_eq!(RADISHLEX_ABI_CONTRACT_VERSION, 9);
    assert_eq!(RADISHLEX_KEY_RESULT_VERSION, 2);
    assert_eq!(RADISHLEX_STARTUP_GATE_ERROR_NONE, 0);
    assert_eq!(
        (
            RADISHLEX_INSTALL_RECEIPT_STATE_COMPLETED,
            RADISHLEX_INSTALL_RECEIPT_STATE_ABORTED_PRESERVED,
            RADISHLEX_INSTALL_RECEIPT_STATE_ROLLED_BACK,
        ),
        (10, 11, 14)
    );
    assert_eq!(
        (
            RADISHLEX_UPGRADE_RECEIPT_STATE_COMPLETED,
            RADISHLEX_UPGRADE_RECEIPT_STATE_ABORTED_PRESERVED,
            RADISHLEX_UPGRADE_RECEIPT_STATE_ROLLED_BACK,
        ),
        (9, 10, 12)
    );
    assert_eq!(size_of::<RadishLexFfiContract>(), 3 * size_of::<u32>());
    assert_eq!(size_of::<RadishLexSessionOptions>(), 2 * size_of::<u32>());
    assert_eq!(size_of::<RadishLexKeyEvent>(), 5 * size_of::<u32>());
    assert_eq!(
        size_of::<RadishLexAppleP256ProductStatus>(),
        11 * size_of::<u32>()
    );
    assert_eq!(
        size_of::<RadishLexAppleP256ProductSmokeSummary>(),
        26 * size_of::<u32>()
    );
    assert_eq!(
        size_of::<RadishLexAppleSecureEnclaveKeyAgreementProductStatus>(),
        9 * size_of::<u32>()
    );
    assert_eq!(
        size_of::<RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary>(),
        25 * size_of::<u32>()
    );
    assert_eq!(
        size_of::<RadishLexManagerSyncProductStatus>(),
        19 * size_of::<u32>()
    );
    assert_eq!(size_of::<RadishLexManagerSyncQualificationRequest>(), 56);
    assert_eq!(size_of::<RadishLexManagerSyncQualificationSnapshot>(), 96);
    assert_eq!(
        size_of::<RadishLexProductInstallStartupGateRequest>(),
        if cfg!(target_pointer_width = "64") {
            24
        } else {
            12
        }
    );
    assert_eq!(
        size_of::<RadishLexProductInstallStartupGateResult>(),
        4 * size_of::<u32>()
    );
    assert_eq!(
        size_of::<RadishLexLinuxProductStartupRequest>(),
        if cfg!(target_pointer_width = "64") {
            24
        } else {
            16
        }
    );
    assert_eq!(
        size_of::<RadishLexLinuxProductStartupResult>(),
        4 * size_of::<u32>()
    );

    let _: unsafe extern "C" fn(
        *mut RadishLexSession,
        RadishLexKeyEvent,
        *mut *mut RadishLexKeyResult,
        *mut *mut RadishLexError,
    ) -> RadishLexStatusCode = radishlex_session_handle_key_event;
    let _: unsafe extern "C" fn(*const RadishLexKeyResult) -> u32 = radishlex_key_result_version;
    let _: unsafe extern "C" fn(*const RadishLexKeyResult) -> u8 = radishlex_key_result_consumed;
    let _: unsafe extern "C" fn(*const RadishLexKeyResult) -> RadishLexStringView =
        radishlex_key_result_commit;
    let _: unsafe extern "C" fn(*const RadishLexKeyResult) -> u8 =
        radishlex_key_result_commit_present;
    let _: unsafe extern "C" fn(*const RadishLexKeyResult) -> u32 =
        radishlex_key_result_learning_disposition;
    let _: unsafe extern "C" fn(*const RadishLexKeyResult) -> *const RadishLexSnapshot =
        radishlex_key_result_snapshot;
    let _: unsafe extern "C" fn(*mut RadishLexKeyResult) = radishlex_key_result_free;
    let _: unsafe extern "C" fn(*mut *mut RadishLexError) -> RadishLexStatusCode =
        radishlex_rime_runtime_shutdown;
    let _: unsafe extern "C" fn(*mut RadishLexAppleP256ProductStatus) -> u32 =
        radishlex_apple_p256_product_status;
    let _: unsafe extern "C" fn(
        u32,
        *const std::os::raw::c_char,
        *mut RadishLexAppleP256ProductSmokeSummary,
    ) -> u32 = radishlex_apple_p256_product_smoke;
    let _: unsafe extern "C" fn(*mut RadishLexAppleP256ProductStatus) -> u32 =
        radishlex_apple_secure_enclave_p256_product_status;
    let _: unsafe extern "C" fn(
        u32,
        *const std::os::raw::c_char,
        *mut RadishLexAppleP256ProductSmokeSummary,
    ) -> u32 = radishlex_apple_secure_enclave_p256_product_smoke;
    let _: unsafe extern "C" fn(*mut RadishLexAppleSecureEnclaveKeyAgreementProductStatus) -> u32 =
        radishlex_apple_secure_enclave_key_agreement_product_status;
    let _: unsafe extern "C" fn(
        u32,
        *mut RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary,
    ) -> u32 = radishlex_apple_secure_enclave_key_agreement_product_smoke;
    let _: unsafe extern "C" fn(
        *mut RadishLexManagerSyncProductStatus,
        *mut *mut RadishLexError,
    ) -> RadishLexStatusCode = radishlex_manager_sync_product_status;
    let _: unsafe extern "C" fn(
        *const RadishLexManagerSyncQualificationRequest,
        *mut *mut RadishLexError,
    ) -> *mut RadishLexManagerSyncQualificationRun = radishlex_manager_sync_qualification_start;
    let _: unsafe extern "C" fn(
        *const RadishLexManagerSyncQualificationRun,
        *mut RadishLexManagerSyncQualificationSnapshot,
        *mut *mut RadishLexError,
    ) -> RadishLexStatusCode = radishlex_manager_sync_qualification_poll;
    let _: unsafe extern "C" fn(
        *const RadishLexManagerSyncQualificationRun,
        *mut u32,
        *mut *mut RadishLexError,
    ) -> RadishLexStatusCode = radishlex_manager_sync_qualification_cancel;
    let _: unsafe extern "C" fn(*mut RadishLexManagerSyncQualificationRun) =
        radishlex_manager_sync_qualification_free;
    let _: unsafe extern "C" fn(
        *const RadishLexProductInstallStartupGateRequest,
        *mut RadishLexProductInstallStartupGateResult,
        *mut *mut RadishLexError,
    ) -> RadishLexStatusCode = radishlex_product_install_startup_gate;
    let _: unsafe extern "C" fn(
        *const RadishLexLinuxProductStartupRequest,
        *mut RadishLexLinuxProductStartupResult,
        *mut *mut RadishLexError,
    ) -> RadishLexStatusCode = radishlex_linux_product_startup_gate;
}

#[test]
fn manager_sync_product_status_is_read_only_and_fails_closed() {
    let mut status = RadishLexManagerSyncProductStatus::empty();
    let mut error = std::ptr::null_mut();

    assert_eq!(
        unsafe { radishlex_manager_sync_product_status(&mut status, &mut error) },
        RadishLexStatusCode::Ok
    );
    assert!(error.is_null());
    assert_eq!(
        status.product_qualified,
        cfg!(all(feature = "apple-keychain", target_os = "macos")) as u32
    );
    assert_eq!(status.user_sync_enabled, 0);
    assert_ne!(status.blocker, RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_NONE);

    assert_eq!(
        unsafe { radishlex_manager_sync_product_status(std::ptr::null_mut(), &mut error) },
        RadishLexStatusCode::InvalidArgument
    );
    assert!(!error.is_null());
    unsafe {
        radishlex_error_free(error);
    }
}

#[cfg(unix)]
#[test]
fn input_header_compiles_as_c11() {
    compile_header("c");
}

#[cfg(target_os = "macos")]
#[test]
fn input_header_compiles_for_objective_c() {
    compile_header("objective-c");
}

#[cfg(unix)]
fn compile_header(language: &str) {
    use std::fs;
    use std::path::PathBuf;
    use std::process::{self, Command};
    use std::time::{SystemTime, UNIX_EPOCH};

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let include_dir = manifest_dir.join("include");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is valid")
        .as_nanos();
    let source_path = std::env::temp_dir().join(format!(
        "radishlex-input-header-{}-{nanos}.c",
        process::id()
    ));

    fs::write(&source_path, HEADER_SMOKE_SOURCE).expect("header smoke source is written");

    let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let output = Command::new(&compiler)
        .arg("-x")
        .arg(language)
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-pedantic")
        .arg("-fsyntax-only")
        .arg("-I")
        .arg(&include_dir)
        .arg(&source_path)
        .output()
        .unwrap_or_else(|error| panic!("failed to run C compiler {compiler:?}: {error}"));

    let _ = fs::remove_file(&source_path);

    assert!(
        output.status.success(),
        "{language} header contract failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
const HEADER_SMOKE_SOURCE: &str = r#"
#include "radishlex_input.h"

_Static_assert(RADISHLEX_ABI_CONTRACT_VERSION == 9u, "ABI version mismatch");
_Static_assert(RADISHLEX_KEY_RESULT_VERSION == 2u, "key result version mismatch");
_Static_assert(RADISHLEX_STARTUP_GATE_ERROR_NONE == 0u, "startup error mismatch");
_Static_assert(RADISHLEX_INSTALL_RECEIPT_STATE_COMPLETED == 10u, "install completed state mismatch");
_Static_assert(RADISHLEX_INSTALL_RECEIPT_STATE_ABORTED_PRESERVED == 11u, "install aborted state mismatch");
_Static_assert(RADISHLEX_INSTALL_RECEIPT_STATE_ROLLED_BACK == 14u, "install rollback state mismatch");
_Static_assert(RADISHLEX_UPGRADE_RECEIPT_STATE_COMPLETED == 9u, "upgrade completed state mismatch");
_Static_assert(RADISHLEX_UPGRADE_RECEIPT_STATE_ABORTED_PRESERVED == 10u, "upgrade aborted state mismatch");
_Static_assert(RADISHLEX_UPGRADE_RECEIPT_STATE_ROLLED_BACK == 12u, "upgrade rollback state mismatch");
_Static_assert(RADISHLEX_LINUX_STARTUP_ALLOWED_PRODUCT == 2u, "Linux product startup decision mismatch");
_Static_assert(RADISHLEX_LINUX_STARTUP_FAILED_CLOSED == 4u, "Linux failed-closed decision mismatch");
_Static_assert(RADISHLEX_LINUX_STARTUP_RECEIPT_COMPLETED == 6u, "Linux completed receipt state mismatch");
_Static_assert(RADISHLEX_LINUX_STARTUP_RECEIPT_ABORTED_PRESERVED == 7u, "Linux aborted receipt state mismatch");
_Static_assert(RADISHLEX_LINUX_STARTUP_RECEIPT_ROLLED_BACK == 11u, "Linux rolled-back receipt state mismatch");
_Static_assert(sizeof(RadishLexFfiContract) == 3u * sizeof(uint32_t), "contract layout mismatch");
_Static_assert(sizeof(RadishLexSessionOptions) == 2u * sizeof(uint32_t), "session options layout mismatch");
_Static_assert(sizeof(RadishLexKeyEvent) == 5u * sizeof(uint32_t), "key event layout mismatch");
_Static_assert(sizeof(RadishLexAppleP256ProductStatus) == 11u * sizeof(uint32_t), "Apple P-256 status layout mismatch");
_Static_assert(sizeof(RadishLexAppleP256ProductSmokeSummary) == 26u * sizeof(uint32_t), "Apple P-256 smoke layout mismatch");
_Static_assert(sizeof(RadishLexAppleSecureEnclaveKeyAgreementProductStatus) == 9u * sizeof(uint32_t), "Apple key-agreement status layout mismatch");
_Static_assert(sizeof(RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary) == 25u * sizeof(uint32_t), "Apple key-agreement smoke layout mismatch");
_Static_assert(sizeof(RadishLexManagerSyncProductStatus) == 19u * sizeof(uint32_t), "Manager sync product status layout mismatch");
_Static_assert(sizeof(RadishLexManagerSyncQualificationRequest) == 56u, "Manager sync qualification request layout mismatch");
_Static_assert(sizeof(RadishLexManagerSyncQualificationSnapshot) == 96u, "Manager sync qualification snapshot layout mismatch");
_Static_assert(sizeof(RadishLexLinuxProductStartupResult) == 4u * sizeof(uint32_t), "Linux startup result layout mismatch");

RadishLexStatusCode radishlex_compile_input_contract(
    RadishLexSession *session,
    RadishLexKeyEvent event,
    RadishLexError **error_out) {
  RadishLexStatusCode (*runtime_shutdown)(RadishLexError **) =
      radishlex_rime_runtime_shutdown;
  uint32_t (*apple_status)(RadishLexAppleP256ProductStatus *) =
      radishlex_apple_p256_product_status;
  uint32_t (*apple_smoke)(uint32_t, const char *, RadishLexAppleP256ProductSmokeSummary *) =
      radishlex_apple_p256_product_smoke;
  uint32_t (*secure_enclave_status)(RadishLexAppleP256ProductStatus *) =
      radishlex_apple_secure_enclave_p256_product_status;
  uint32_t (*secure_enclave_smoke)(uint32_t, const char *, RadishLexAppleP256ProductSmokeSummary *) =
      radishlex_apple_secure_enclave_p256_product_smoke;
  uint32_t (*key_agreement_status)(RadishLexAppleSecureEnclaveKeyAgreementProductStatus *) =
      radishlex_apple_secure_enclave_key_agreement_product_status;
  uint32_t (*key_agreement_smoke)(uint32_t, RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary *) =
      radishlex_apple_secure_enclave_key_agreement_product_smoke;
  RadishLexStatusCode (*manager_sync_status)(RadishLexManagerSyncProductStatus *, RadishLexError **) =
      radishlex_manager_sync_product_status;
  RadishLexManagerSyncQualificationRun *(*qualification_start)(const RadishLexManagerSyncQualificationRequest *, RadishLexError **) =
      radishlex_manager_sync_qualification_start;
  RadishLexStatusCode (*qualification_poll)(const RadishLexManagerSyncQualificationRun *, RadishLexManagerSyncQualificationSnapshot *, RadishLexError **) =
      radishlex_manager_sync_qualification_poll;
  RadishLexStatusCode (*qualification_cancel)(const RadishLexManagerSyncQualificationRun *, uint32_t *, RadishLexError **) =
      radishlex_manager_sync_qualification_cancel;
  void (*qualification_free)(RadishLexManagerSyncQualificationRun *) =
      radishlex_manager_sync_qualification_free;
  RadishLexStatusCode (*linux_startup)(const RadishLexLinuxProductStartupRequest *, RadishLexLinuxProductStartupResult *, RadishLexError **) =
      radishlex_linux_product_startup_gate;
  RadishLexKeyResult *result = NULL;
  RadishLexStatusCode status =
      radishlex_session_handle_key_event(session, event, &result, error_out);
  if (status == RADISHLEX_STATUS_OK && result != NULL) {
    uint32_t version = radishlex_key_result_version(result);
    uint8_t consumed = radishlex_key_result_consumed(result);
    uint8_t commit_present = radishlex_key_result_commit_present(result);
    uint32_t learning_disposition =
        radishlex_key_result_learning_disposition(result);
    RadishLexStringView commit = radishlex_key_result_commit(result);
    const RadishLexSnapshot *snapshot = radishlex_key_result_snapshot(result);
    RadishLexStringView preedit = radishlex_snapshot_preedit(snapshot);
    RadishLexCandidateView candidate = {0};
    uint32_t personalization_status =
        radishlex_snapshot_personalization_status(snapshot);
    (void)version;
    (void)consumed;
    (void)commit_present;
    (void)learning_disposition;
    (void)commit;
    (void)preedit;
    (void)candidate;
    (void)personalization_status;
    radishlex_key_result_free(result);
  }
  (void)runtime_shutdown;
  (void)apple_status;
  (void)apple_smoke;
  (void)secure_enclave_status;
  (void)secure_enclave_smoke;
  (void)key_agreement_status;
  (void)key_agreement_smoke;
  (void)manager_sync_status;
  (void)qualification_start;
  (void)qualification_poll;
  (void)qualification_cancel;
  (void)qualification_free;
  (void)linux_startup;
  return status;
}
"#;
