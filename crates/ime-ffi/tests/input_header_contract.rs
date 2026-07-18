use std::mem::size_of;

use radishlex_ime_ffi::{
    radishlex_apple_p256_product_smoke, radishlex_apple_p256_product_status,
    radishlex_apple_secure_enclave_p256_product_smoke,
    radishlex_apple_secure_enclave_p256_product_status, radishlex_key_result_commit,
    radishlex_key_result_commit_present, radishlex_key_result_consumed, radishlex_key_result_free,
    radishlex_key_result_learning_disposition, radishlex_key_result_snapshot,
    radishlex_key_result_version, radishlex_rime_runtime_shutdown,
    radishlex_session_handle_key_event, RadishLexAppleP256ProductSmokeSummary,
    RadishLexAppleP256ProductStatus, RadishLexError, RadishLexFfiContract, RadishLexKeyEvent,
    RadishLexKeyResult, RadishLexSession, RadishLexSessionOptions, RadishLexSnapshot,
    RadishLexStatusCode, RadishLexStringView, RADISHLEX_ABI_CONTRACT_VERSION,
    RADISHLEX_KEY_RESULT_VERSION,
};

#[test]
fn rust_input_abi_layout_matches_the_checked_header_contract() {
    assert_eq!(RADISHLEX_ABI_CONTRACT_VERSION, 5);
    assert_eq!(RADISHLEX_KEY_RESULT_VERSION, 2);
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

_Static_assert(RADISHLEX_ABI_CONTRACT_VERSION == 5u, "ABI version mismatch");
_Static_assert(RADISHLEX_KEY_RESULT_VERSION == 2u, "key result version mismatch");
_Static_assert(sizeof(RadishLexFfiContract) == 3u * sizeof(uint32_t), "contract layout mismatch");
_Static_assert(sizeof(RadishLexSessionOptions) == 2u * sizeof(uint32_t), "session options layout mismatch");
_Static_assert(sizeof(RadishLexKeyEvent) == 5u * sizeof(uint32_t), "key event layout mismatch");
_Static_assert(sizeof(RadishLexAppleP256ProductStatus) == 11u * sizeof(uint32_t), "Apple P-256 status layout mismatch");
_Static_assert(sizeof(RadishLexAppleP256ProductSmokeSummary) == 26u * sizeof(uint32_t), "Apple P-256 smoke layout mismatch");

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
  return status;
}
"#;
