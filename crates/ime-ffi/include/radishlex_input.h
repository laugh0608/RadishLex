#ifndef RADISHLEX_INPUT_H
#define RADISHLEX_INPUT_H

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
extern "C" {
#endif

#define RADISHLEX_ABI_CONTRACT_VERSION 9u
#define RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD 1u
#define RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND 1u

#define RADISHLEX_SESSION_OPTIONS_VERSION 1u
#define RADISHLEX_RIME_SESSION_OPTIONS_VERSION 1u
#define RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION 1u
#define RADISHLEX_LEARNING_CONTEXT_VERSION 1u
#define RADISHLEX_ENGINE_KIND_DEMO 1u
#define RADISHLEX_ENGINE_KIND_RIME 2u

#define RADISHLEX_KEY_RESULT_VERSION 2u

#define RADISHLEX_APPLE_P256_PRODUCT_STATUS_VERSION 1u
#define RADISHLEX_APPLE_P256_PRODUCT_SMOKE_VERSION 4u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_P256_PRODUCT_STATUS_VERSION 1u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_P256_PRODUCT_SMOKE_VERSION 1u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_PRODUCT_STATUS_VERSION 1u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_PRODUCT_SMOKE_VERSION 1u
#define RADISHLEX_MANAGER_SYNC_PRODUCT_STATUS_VERSION 1u
#define RADISHLEX_MANAGER_SYNC_QUALIFICATION_REQUEST_VERSION 1u
#define RADISHLEX_MANAGER_SYNC_QUALIFICATION_SNAPSHOT_VERSION 1u
#define RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_REQUEST_VERSION 1u
#define RADISHLEX_PRODUCT_UPGRADE_STARTUP_GATE_RESULT_VERSION 1u
#define RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_REQUEST_VERSION 1u
#define RADISHLEX_PRODUCT_INSTALL_STARTUP_GATE_RESULT_VERSION 1u
#define RADISHLEX_LINUX_PRODUCT_STARTUP_REQUEST_VERSION 1u
#define RADISHLEX_LINUX_PRODUCT_STARTUP_RESULT_VERSION 1u
#define RADISHLEX_MANAGER_UPGRADE_VALIDATION_REQUEST_VERSION 1u
#define RADISHLEX_INPUT_METHOD_UPGRADE_VALIDATION_REQUEST_VERSION 1u

#define RADISHLEX_STARTUP_GATE_ALLOWED_FIRST_LAUNCH 1u
#define RADISHLEX_STARTUP_GATE_ALLOWED_NO_UPGRADE_STATE 2u
#define RADISHLEX_STARTUP_GATE_ALLOWED_TERMINAL_RECEIPT 3u
#define RADISHLEX_STARTUP_GATE_BLOCKED_UPGRADE_IN_PROGRESS 4u
#define RADISHLEX_STARTUP_GATE_FAILED_CLOSED 5u

#define RADISHLEX_INSTALL_GATE_ALLOWED_FIRST_LAUNCH 1u
#define RADISHLEX_INSTALL_GATE_ALLOWED_NO_INSTALL_STATE 2u
#define RADISHLEX_INSTALL_GATE_ALLOWED_TERMINAL_RECEIPT 3u
#define RADISHLEX_INSTALL_GATE_BLOCKED_IN_PROGRESS 4u
#define RADISHLEX_INSTALL_GATE_FAILED_CLOSED 5u

#define RADISHLEX_LINUX_STARTUP_BUILD_DEVELOPMENT_STAGED 1u
#define RADISHLEX_LINUX_STARTUP_BUILD_DEBIAN_SYSTEM_PRODUCT 2u

#define RADISHLEX_LINUX_STARTUP_COMPONENT_MANAGER 1u
#define RADISHLEX_LINUX_STARTUP_COMPONENT_FCITX_ADDON 2u

#define RADISHLEX_LINUX_STARTUP_ALLOWED_DEVELOPMENT 1u
#define RADISHLEX_LINUX_STARTUP_ALLOWED_PRODUCT 2u
#define RADISHLEX_LINUX_STARTUP_MAINTENANCE_REQUIRED 3u
#define RADISHLEX_LINUX_STARTUP_FAILED_CLOSED 4u

#define RADISHLEX_LINUX_STARTUP_RECEIPT_NONE 0u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_PREPARED 1u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_ARTIFACTS_STAGED 2u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_QUIESCED 3u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_PACKAGE_MUTATING 4u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_PACKAGE_VERIFIED 5u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_COMPLETED 6u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_ABORTED_PRESERVED 7u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_ROLLBACK_REQUIRED 8u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_SOURCE_RESTORING 9u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_SOURCE_VERIFIED 10u
#define RADISHLEX_LINUX_STARTUP_RECEIPT_ROLLED_BACK 11u

#define RADISHLEX_LINUX_STARTUP_REASON_DEVELOPMENT_STATE_ABSENT 1u
#define RADISHLEX_LINUX_STARTUP_REASON_INSTALLED_RECEIPT_VERIFIED 2u
#define RADISHLEX_LINUX_STARTUP_REASON_ACTIVE_GUARD 10u
#define RADISHLEX_LINUX_STARTUP_REASON_GUARD_INVALID 11u
#define RADISHLEX_LINUX_STARTUP_REASON_INTERRUPTED_RECEIPT 12u
#define RADISHLEX_LINUX_STARTUP_REASON_INTERRUPTED_RECEIPT_INVALID 13u
#define RADISHLEX_LINUX_STARTUP_REASON_OPERATION_IN_PROGRESS 14u
#define RADISHLEX_LINUX_STARTUP_REASON_RECEIPT_MISSING 15u
#define RADISHLEX_LINUX_STARTUP_REASON_RECEIPT_INVALID 16u
#define RADISHLEX_LINUX_STARTUP_REASON_UNEXPECTED_STATE_OBJECT 17u
#define RADISHLEX_LINUX_STARTUP_REASON_ROOT_IDENTITY_CHANGED 18u
#define RADISHLEX_LINUX_STARTUP_REASON_PACKAGE_STATE_UNAVAILABLE 19u
#define RADISHLEX_LINUX_STARTUP_REASON_PACKAGE_STATE_UNKNOWN 20u
#define RADISHLEX_LINUX_STARTUP_REASON_PACKAGE_STATE_INCOMPLETE 21u
#define RADISHLEX_LINUX_STARTUP_REASON_UNMANAGED_PACKAGE_STATE 22u
#define RADISHLEX_LINUX_STARTUP_REASON_PACKAGE_IDENTITY_CHANGED 23u
#define RADISHLEX_LINUX_STARTUP_REASON_REMOVED_PROGRAM 24u
#define RADISHLEX_LINUX_STARTUP_REASON_PRODUCT_NOT_INSTALLED 25u
#define RADISHLEX_LINUX_STARTUP_REASON_COMPONENT_IDENTITY_CHANGED 26u
#define RADISHLEX_LINUX_STARTUP_REASON_DEPENDENCY_UNAVAILABLE 27u
#define RADISHLEX_LINUX_STARTUP_REASON_DEVELOPMENT_ISOLATION_VIOLATION 28u
#define RADISHLEX_LINUX_STARTUP_REASON_PERMISSION_DENIED 29u
#define RADISHLEX_LINUX_STARTUP_REASON_IO 30u

#define RADISHLEX_STARTUP_GATE_ERROR_NONE 0u

#define RADISHLEX_INSTALL_RECEIPT_STATE_COMPLETED 10u
#define RADISHLEX_INSTALL_RECEIPT_STATE_ABORTED_PRESERVED 11u
#define RADISHLEX_INSTALL_RECEIPT_STATE_ROLLED_BACK 14u

#define RADISHLEX_UPGRADE_RECEIPT_STATE_COMPLETED 9u
#define RADISHLEX_UPGRADE_RECEIPT_STATE_ABORTED_PRESERVED 10u
#define RADISHLEX_UPGRADE_RECEIPT_STATE_ROLLED_BACK 12u

#define RADISHLEX_MANAGER_SYNC_QUALIFICATION_STATE_CREATED 1u
#define RADISHLEX_MANAGER_SYNC_QUALIFICATION_STATE_RUNNING 2u
#define RADISHLEX_MANAGER_SYNC_QUALIFICATION_STATE_CANCELLING 3u
#define RADISHLEX_MANAGER_SYNC_QUALIFICATION_STATE_COMPLETED 4u
#define RADISHLEX_MANAGER_SYNC_QUALIFICATION_STATE_FAILED 5u
#define RADISHLEX_MANAGER_SYNC_QUALIFICATION_STATE_CANCELLED 6u

#define RADISHLEX_MANAGER_SIGNING_BACKEND_UNAVAILABLE 0u
#define RADISHLEX_MANAGER_SIGNING_BACKEND_APPLE_SECURE_ENCLAVE_P256_V1 1u
#define RADISHLEX_MANAGER_SIGNING_ALGORITHM_UNAVAILABLE 0u
#define RADISHLEX_MANAGER_SIGNING_ALGORITHM_ECDSA_P256_SHA256_V1 1u
#define RADISHLEX_MANAGER_KEY_AGREEMENT_BACKEND_UNAVAILABLE 0u
#define RADISHLEX_MANAGER_KEY_AGREEMENT_BACKEND_APPLE_SECURE_ENCLAVE_P256_V1 1u

#define RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_NONE 0u
#define RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_NOT_COMPILED 1u
#define RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_UNAVAILABLE 2u
#define RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_SIGNING_QUALIFICATION_REQUIRED 3u
#define RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_KEY_AGREEMENT_NOT_COMPILED 4u
#define RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_KEY_AGREEMENT_RUNTIME_QUALIFICATION_REQUIRED 5u
#define RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_KEY_AGREEMENT_QUALIFICATION_REQUIRED 6u
#define RADISHLEX_MANAGER_SYNC_PRODUCT_BLOCKER_USER_SYNC_CLOSED 7u

#define RADISHLEX_APPLE_P256_SCENARIO_LIFECYCLE 0u
#define RADISHLEX_APPLE_P256_SCENARIO_EXPECT_DENIED_CREATE 1u
#define RADISHLEX_APPLE_P256_SCENARIO_PREPARE_LOCKED_SIGN 2u
#define RADISHLEX_APPLE_P256_SCENARIO_EXPECT_LOCKED_SIGN 3u
#define RADISHLEX_APPLE_P256_SCENARIO_CLEANUP_LOCKED_SIGN 4u

#define RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_LIFECYCLE 0u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_EXPECT_DENIED_CREATE 1u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_PREPARE_LOCKED_SIGN 2u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_EXPECT_LOCKED_SIGN 3u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_CLEANUP_LOCKED_SIGN 4u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_P256_SCENARIO_EXPECT_UNSUPPORTED_CREATE 5u

#define RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_LIFECYCLE 0u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_EXPECT_DENIED_CREATE 1u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_PREPARE_LOCKED_DERIVE 2u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_EXPECT_LOCKED_DERIVE 3u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_CLEANUP_LOCKED_DERIVE 4u
#define RADISHLEX_APPLE_SECURE_ENCLAVE_KEY_AGREEMENT_SCENARIO_EXPECT_UNSUPPORTED_CREATE 5u

#define RADISHLEX_APPLE_P256_ERROR_NONE 0u
#define RADISHLEX_APPLE_P256_ERROR_BACKEND_UNAVAILABLE 1u
#define RADISHLEX_APPLE_P256_ERROR_LOCKED 2u
#define RADISHLEX_APPLE_P256_ERROR_ACCESS_DENIED 3u
#define RADISHLEX_APPLE_P256_ERROR_USER_PRESENCE_REQUIRED 4u
#define RADISHLEX_APPLE_P256_ERROR_MISSING 5u
#define RADISHLEX_APPLE_P256_ERROR_CORRUPTED 6u
#define RADISHLEX_APPLE_P256_ERROR_UNSUPPORTED 7u
#define RADISHLEX_APPLE_P256_ERROR_REVOKED 8u
#define RADISHLEX_APPLE_P256_ERROR_OTHER 255u

#define RADISHLEX_APPLE_P256_ERROR_DETAIL_NONE 0u
#define RADISHLEX_APPLE_P256_ERROR_DETAIL_ACCESS_UNSPECIFIED 1u
#define RADISHLEX_APPLE_P256_ERROR_DETAIL_AUTHENTICATION_FAILED 2u
#define RADISHLEX_APPLE_P256_ERROR_DETAIL_WRITE_PERMISSION 3u
#define RADISHLEX_APPLE_P256_ERROR_DETAIL_READ_ONLY 4u
#define RADISHLEX_APPLE_P256_ERROR_DETAIL_MISSING_ENTITLEMENT 5u
#define RADISHLEX_APPLE_P256_ERROR_DETAIL_RESTRICTED_API 6u
#define RADISHLEX_APPLE_P256_ERROR_DETAIL_UNCLASSIFIED_PLATFORM_STATUS 7u

#define RADISHLEX_APPLE_P256_SMOKE_PASSED 0u
#define RADISHLEX_APPLE_P256_SMOKE_GATE_DISABLED 1u
#define RADISHLEX_APPLE_P256_SMOKE_UNSUPPORTED_BUILD 2u
#define RADISHLEX_APPLE_P256_SMOKE_INVALID_ARGUMENT 3u
#define RADISHLEX_APPLE_P256_SMOKE_CAPABILITY_MISMATCH 4u
#define RADISHLEX_APPLE_P256_SMOKE_CREATE_FAILED 5u
#define RADISHLEX_APPLE_P256_SMOKE_RELOAD_FAILED 6u
#define RADISHLEX_APPLE_P256_SMOKE_RUST_VERIFY_FAILED 7u
#define RADISHLEX_APPLE_P256_SMOKE_GO_VERIFY_FAILED 8u
#define RADISHLEX_APPLE_P256_SMOKE_DELETE_FAILED 9u
#define RADISHLEX_APPLE_P256_SMOKE_MISSING_CHECK_FAILED 10u
#define RADISHLEX_APPLE_P256_SMOKE_EXPECTED_FAILURE_NOT_OBSERVED 11u
#define RADISHLEX_APPLE_P256_SMOKE_UNEXPECTED_ERROR_CATEGORY 12u
#define RADISHLEX_APPLE_P256_SMOKE_INTERNAL_ERROR 255u

#define RADISHLEX_PERSONALIZATION_STATUS_NOT_ENABLED 0u
#define RADISHLEX_PERSONALIZATION_STATUS_READY 1u
#define RADISHLEX_PERSONALIZATION_STATUS_POLICY_BLOCKED 2u
#define RADISHLEX_PERSONALIZATION_STATUS_STORAGE_UNAVAILABLE 3u
#define RADISHLEX_PERSONALIZATION_STATUS_READ_FAILED 4u
#define RADISHLEX_PERSONALIZATION_STATUS_RANK_FAILED 5u

#define RADISHLEX_LEARNING_NOT_APPLICABLE 0u
#define RADISHLEX_LEARNING_RECORDED 1u
#define RADISHLEX_LEARNING_DEFERRED 2u
#define RADISHLEX_LEARNING_SKIPPED_BY_POLICY 3u
#define RADISHLEX_LEARNING_FAILED 4u

#define RADISHLEX_KEY_KIND_CHAR 1u
#define RADISHLEX_KEY_KIND_NAMED 2u

#define RADISHLEX_NAMED_KEY_SPACE 1u
#define RADISHLEX_NAMED_KEY_ENTER 2u
#define RADISHLEX_NAMED_KEY_BACKSPACE 3u
#define RADISHLEX_NAMED_KEY_ESCAPE 4u
#define RADISHLEX_NAMED_KEY_TAB 5u
#define RADISHLEX_NAMED_KEY_ARROW_UP 6u
#define RADISHLEX_NAMED_KEY_ARROW_DOWN 7u
#define RADISHLEX_NAMED_KEY_ARROW_LEFT 8u
#define RADISHLEX_NAMED_KEY_ARROW_RIGHT 9u
#define RADISHLEX_NAMED_KEY_PAGE_UP 10u
#define RADISHLEX_NAMED_KEY_PAGE_DOWN 11u
#define RADISHLEX_NAMED_KEY_SHIFT 12u
#define RADISHLEX_NAMED_KEY_CONTROL 13u
#define RADISHLEX_NAMED_KEY_ALT 14u
#define RADISHLEX_NAMED_KEY_META 15u
#define RADISHLEX_NAMED_KEY_UNKNOWN 255u

#define RADISHLEX_KEY_MOD_SHIFT (1u << 0)
#define RADISHLEX_KEY_MOD_CONTROL (1u << 1)
#define RADISHLEX_KEY_MOD_ALT (1u << 2)
#define RADISHLEX_KEY_MOD_META (1u << 3)

#define RADISHLEX_KEY_PHASE_PRESS 1u
#define RADISHLEX_KEY_PHASE_RELEASE 2u

#define RADISHLEX_CANDIDATE_SOURCE_ENGINE 1u
#define RADISHLEX_CANDIDATE_SOURCE_USER_DICTIONARY 2u
#define RADISHLEX_CANDIDATE_SOURCE_PERSONALIZED 3u
#define RADISHLEX_CANDIDATE_SOURCE_SYSTEM 4u

typedef struct RadishLexSession RadishLexSession;
typedef struct RadishLexKeyResult RadishLexKeyResult;
typedef struct RadishLexSnapshot RadishLexSnapshot;
typedef struct RadishLexBuffer RadishLexBuffer;
typedef struct RadishLexError RadishLexError;
typedef struct RadishLexManagerSyncQualificationRun RadishLexManagerSyncQualificationRun;

typedef struct RadishLexProductUpgradeStartupGateRequest {
  uint32_t version;
  const char *data_root_path;
  uint32_t expected_owner_id;
} RadishLexProductUpgradeStartupGateRequest;

typedef struct RadishLexProductUpgradeStartupGateResult {
  uint32_t version;
  uint32_t decision;
  uint32_t error_code;
  uint32_t receipt_state;
} RadishLexProductUpgradeStartupGateResult;

typedef struct RadishLexProductInstallStartupGateRequest {
  uint32_t version;
  const char *data_root_path;
  uint32_t expected_owner_id;
} RadishLexProductInstallStartupGateRequest;

typedef struct RadishLexProductInstallStartupGateResult {
  uint32_t version;
  uint32_t decision;
  uint32_t error_code;
  uint32_t receipt_state;
} RadishLexProductInstallStartupGateResult;

typedef struct RadishLexLinuxProductStartupRequest {
  uint32_t version;
  uint32_t build_identity;
  uint32_t component;
  const char *component_path;
} RadishLexLinuxProductStartupRequest;

typedef struct RadishLexLinuxProductStartupResult {
  uint32_t version;
  uint32_t decision;
  uint32_t reason;
  uint32_t receipt_state;
} RadishLexLinuxProductStartupResult;

typedef struct RadishLexManagerUpgradeValidationRequest {
  uint32_t version;
  const char *candidate_path;
  const char *settings_path;
} RadishLexManagerUpgradeValidationRequest;

typedef struct RadishLexInputMethodUpgradeValidationRequest {
  uint32_t version;
  const char *candidate_path;
  const char *shared_data_path;
  const char *validation_user_data_path;
  const char *schema;
} RadishLexInputMethodUpgradeValidationRequest;

typedef struct RadishLexUpgradeValidationSummary {
  uint32_t version;
  int64_t schema_version;
  uint32_t management_queries_checked;
  uint32_t settings_checked;
  uint32_t personalized_runtime_checked;
  uint32_t candidate_signals_read;
} RadishLexUpgradeValidationSummary;

typedef struct RadishLexAppleP256ProductStatus {
  uint32_t version;
  uint32_t compiled;
  uint32_t runtime_available;
  uint32_t can_create_signing_keys;
  uint32_t can_sign;
  uint32_t product_qualified;
  uint32_t user_sync_enabled;
  uint32_t exportable;
  uint32_t hardware_backed;
  uint32_t user_presence_required;
  uint32_t backup_migratable;
} RadishLexAppleP256ProductStatus;

typedef struct RadishLexAppleP256ProductSmokeSummary {
  uint32_t version;
  uint32_t result;
  uint32_t scenario;
  uint32_t error_category;
  uint32_t error_detail;
  int32_t platform_status;
  uint32_t compiled;
  uint32_t runtime_available;
  uint32_t can_create_signing_keys;
  uint32_t can_sign;
  uint32_t product_qualified;
  uint32_t user_sync_enabled;
  uint32_t exportable;
  uint32_t hardware_backed;
  uint32_t user_presence_required;
  uint32_t backup_migratable;
  uint32_t created;
  uint32_t reloaded;
  uint32_t rust_verified;
  uint32_t go_verified;
  uint32_t deleted;
  uint32_t missing_confirmed;
  uint32_t fail_closed;
  uint32_t expected_failure_confirmed;
  uint32_t cleanup_required;
  uint32_t cleanup_attempted;
} RadishLexAppleP256ProductSmokeSummary;

typedef struct RadishLexAppleSecureEnclaveKeyAgreementProductStatus {
  uint32_t version;
  uint32_t compiled;
  uint32_t runtime_qualified;
  uint32_t product_qualified;
  uint32_t user_sync_enabled;
  uint32_t exportable;
  uint32_t hardware_backed;
  uint32_t user_presence_required;
  uint32_t backup_migratable;
} RadishLexAppleSecureEnclaveKeyAgreementProductStatus;

typedef struct RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary {
  uint32_t version;
  uint32_t result;
  uint32_t scenario;
  uint32_t error_category;
  uint32_t error_detail;
  int32_t platform_status;
  uint32_t compiled;
  uint32_t runtime_qualified;
  uint32_t product_qualified;
  uint32_t user_sync_enabled;
  uint32_t exportable;
  uint32_t hardware_backed;
  uint32_t user_presence_required;
  uint32_t backup_migratable;
  uint32_t created;
  uint32_t reloaded;
  uint32_t public_key_matched;
  uint32_t shared_secret_derived;
  uint32_t wrapped_epoch_verified;
  uint32_t deleted;
  uint32_t missing_confirmed;
  uint32_t fail_closed;
  uint32_t expected_failure_confirmed;
  uint32_t cleanup_required;
  uint32_t cleanup_attempted;
} RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary;

typedef struct RadishLexManagerSyncProductStatus {
  uint32_t version;
  uint32_t signing_backend;
  uint32_t signing_algorithm;
  uint32_t signing_compiled;
  uint32_t signing_runtime_available;
  uint32_t signing_can_create;
  uint32_t signing_can_sign;
  uint32_t signing_exportable;
  uint32_t signing_hardware_backed;
  uint32_t signing_user_presence_required;
  uint32_t signing_backup_migratable;
  uint32_t signing_product_qualified;
  uint32_t key_agreement_backend;
  uint32_t key_agreement_compiled;
  uint32_t key_agreement_runtime_qualified;
  uint32_t key_agreement_product_qualified;
  uint32_t product_qualified;
  uint32_t user_sync_enabled;
  uint32_t blocker;
} RadishLexManagerSyncProductStatus;

typedef struct RadishLexManagerSyncQualificationRequest {
  uint32_t version;
  const char *endpoint;
  const uint8_t *access_token_data;
  size_t access_token_len;
  const uint8_t *local_ca_der_data;
  size_t local_ca_der_len;
  uint64_t timeout_ms;
} RadishLexManagerSyncQualificationRequest;

typedef struct RadishLexManagerSyncQualificationSnapshot {
  uint32_t version;
  uint32_t state;
  uint32_t phase;
  uint64_t discovered;
  uint64_t downloaded;
  uint64_t applied;
  uint64_t uploaded;
  uint64_t conflicts;
  uint64_t retries;
  uint64_t convergence_rounds;
  uint32_t temporary_files_cleaned;
  uint32_t worker_stopped;
  uint32_t transient_inputs_cleared;
  uint32_t error_code;
  uint32_t error_phase;
  uint32_t error_retryable;
} RadishLexManagerSyncQualificationSnapshot;

typedef enum RadishLexStatusCode {
  RADISHLEX_STATUS_OK = 0,
  RADISHLEX_STATUS_INVALID_ARGUMENT = 1,
  RADISHLEX_STATUS_INVALID_STATE = 2,
  RADISHLEX_STATUS_ENGINE_ERROR = 3,
  RADISHLEX_STATUS_USERDB_ERROR = 4,
  RADISHLEX_STATUS_RANKER_ERROR = 5,
  RADISHLEX_STATUS_SYNC_ERROR = 6,
  RADISHLEX_STATUS_INTERNAL_ERROR = 255
} RadishLexStatusCode;

typedef struct RadishLexFfiContract {
  uint32_t version;
  uint32_t session_thread_policy;
  uint32_t panic_boundary;
} RadishLexFfiContract;

typedef struct RadishLexSessionOptions {
  uint32_t version;
  uint32_t engine_kind;
} RadishLexSessionOptions;

typedef struct RadishLexRimeSessionOptions {
  uint32_t version;
  const char *shared_data_dir;
  const char *user_data_dir;
  const char *schema;
  const char *log_dir;
  uint8_t deploy_on_start;
} RadishLexRimeSessionOptions;

typedef struct RadishLexPersonalizedRimeSessionOptions {
  uint32_t version;
  const char *shared_data_dir;
  const char *user_data_dir;
  const char *schema;
  const char *log_dir;
  uint8_t deploy_on_start;
  const char *userdb_path;
  const char *session_id;
} RadishLexPersonalizedRimeSessionOptions;

typedef struct RadishLexKeyEvent {
  uint32_t key_kind;
  uint32_t codepoint;
  uint32_t named_key;
  uint32_t modifiers;
  uint32_t phase;
} RadishLexKeyEvent;

typedef struct RadishLexStringView {
  const uint8_t *data;
  size_t len;
} RadishLexStringView;

typedef struct RadishLexCandidateView {
  size_t index;
  size_t engine_index;
  RadishLexStringView text;
  RadishLexStringView reading;
  uint8_t reading_present;
  RadishLexStringView annotation;
  uint8_t annotation_present;
  uint32_t source;
} RadishLexCandidateView;

typedef struct RadishLexLearningContext {
  uint32_t version;
  uint8_t secure_input;
  uint8_t sensitive_application;
  uint8_t privacy_mode;
  uint8_t context_known;
  RadishLexStringView context_kind;
} RadishLexLearningContext;

RadishLexStatusCode radishlex_ffi_contract(
    RadishLexFfiContract *contract_out,
    RadishLexError **error_out);

RadishLexStatusCode radishlex_product_upgrade_startup_gate(
    const RadishLexProductUpgradeStartupGateRequest *request,
    RadishLexProductUpgradeStartupGateResult *result_out,
    RadishLexError **error_out);
RadishLexStatusCode radishlex_product_install_startup_gate(
    const RadishLexProductInstallStartupGateRequest *request,
    RadishLexProductInstallStartupGateResult *result_out,
    RadishLexError **error_out);
RadishLexStatusCode radishlex_linux_product_startup_gate(
    const RadishLexLinuxProductStartupRequest *request,
    RadishLexLinuxProductStartupResult *result_out,
    RadishLexError **error_out);
RadishLexStatusCode radishlex_manager_upgrade_validate_candidate(
    const RadishLexManagerUpgradeValidationRequest *request,
    RadishLexUpgradeValidationSummary *summary_out,
    RadishLexError **error_out);
RadishLexStatusCode radishlex_input_method_upgrade_validate_candidate(
    const RadishLexInputMethodUpgradeValidationRequest *request,
    RadishLexUpgradeValidationSummary *summary_out,
    RadishLexError **error_out);

/*
 * Status-only Manager snapshot input. This call does not create, read, use, or
 * delete platform key items and never returns identifiers or secret material.
 */
RadishLexStatusCode radishlex_manager_sync_product_status(
    RadishLexManagerSyncProductStatus *status_out,
    RadishLexError **error_out);

/*
 * Local HTTPS synthetic qualification only. Inputs are copied for one run and
 * never become settings, diagnostics, user sync state, or platform key calls.
 * Calls may move between threads but must be serialized by the caller. free
 * must not overlap poll/cancel; it cancels and joins before releasing.
 */
RadishLexManagerSyncQualificationRun *radishlex_manager_sync_qualification_start(
    const RadishLexManagerSyncQualificationRequest *request,
    RadishLexError **error_out);
RadishLexStatusCode radishlex_manager_sync_qualification_poll(
    const RadishLexManagerSyncQualificationRun *run,
    RadishLexManagerSyncQualificationSnapshot *snapshot_out,
    RadishLexError **error_out);
RadishLexStatusCode radishlex_manager_sync_qualification_cancel(
    const RadishLexManagerSyncQualificationRun *run,
    uint32_t *requested_out,
    RadishLexError **error_out);
void radishlex_manager_sync_qualification_free(
    RadishLexManagerSyncQualificationRun *run);

/*
 * Product-validation ABI only. It returns fixed capability/lifecycle flags and
 * never returns private key, canonical, public-key, or signature bytes.
 */
uint32_t radishlex_apple_p256_product_status(
    RadishLexAppleP256ProductStatus *status_out);
uint32_t radishlex_apple_p256_product_smoke(
    uint32_t scenario,
    const char *go_server_dir,
    RadishLexAppleP256ProductSmokeSummary *summary_out);
uint32_t radishlex_apple_secure_enclave_p256_product_status(
    RadishLexAppleP256ProductStatus *status_out);
uint32_t radishlex_apple_secure_enclave_p256_product_smoke(
    uint32_t scenario,
    const char *go_server_dir,
    RadishLexAppleP256ProductSmokeSummary *summary_out);
uint32_t radishlex_apple_secure_enclave_key_agreement_product_status(
    RadishLexAppleSecureEnclaveKeyAgreementProductStatus *status_out);
uint32_t radishlex_apple_secure_enclave_key_agreement_product_smoke(
    uint32_t scenario,
    RadishLexAppleSecureEnclaveKeyAgreementProductSmokeSummary *summary_out);

RadishLexSession *radishlex_session_new(RadishLexError **error_out);
RadishLexSession *radishlex_session_new_with_options(
    const RadishLexSessionOptions *options,
    RadishLexError **error_out);
RadishLexSession *radishlex_session_new_rime(
    const RadishLexRimeSessionOptions *options,
    RadishLexError **error_out);
RadishLexSession *radishlex_session_new_personalized_rime(
    const RadishLexPersonalizedRimeSessionOptions *options,
    RadishLexError **error_out);
/* Owner-thread only. A non-owner-thread call is ignored. */
void radishlex_session_free(RadishLexSession *session);

/*
 * Call on the Rime runtime owner thread during process teardown after every
 * Rime session is released.
 * The call is idempotent and returns INVALID_STATE while sessions are active.
 */
RadishLexStatusCode radishlex_rime_runtime_shutdown(
    RadishLexError **error_out);

uint32_t radishlex_session_engine_kind(const RadishLexSession *session);
RadishLexStatusCode radishlex_session_reset(
    RadishLexSession *session,
    RadishLexError **error_out);
RadishLexStatusCode radishlex_session_set_schema(
    RadishLexSession *session,
    const char *schema,
    RadishLexError **error_out);
RadishLexStatusCode radishlex_session_set_learning_context(
    RadishLexSession *session,
    RadishLexLearningContext context,
    RadishLexError **error_out);

/*
 * Platform shells must use this entry point. On success, result_out owns one
 * key result and must be released with radishlex_key_result_free. On failure,
 * result_out is NULL and no partial result is returned.
 */
RadishLexStatusCode radishlex_session_handle_key_event(
    RadishLexSession *session,
    RadishLexKeyEvent event,
    RadishLexKeyResult **result_out,
    RadishLexError **error_out);

/* Compatibility entries: these discard KeyOutcome and are not platform APIs. */
RadishLexStatusCode radishlex_session_push_key(
    RadishLexSession *session,
    uint32_t codepoint,
    RadishLexError **error_out);
RadishLexStatusCode radishlex_session_push_key_event(
    RadishLexSession *session,
    RadishLexKeyEvent event,
    RadishLexError **error_out);

uint32_t radishlex_key_result_version(const RadishLexKeyResult *result);
uint8_t radishlex_key_result_consumed(const RadishLexKeyResult *result);
RadishLexStringView radishlex_key_result_commit(
    const RadishLexKeyResult *result);
uint8_t radishlex_key_result_commit_present(
    const RadishLexKeyResult *result);
uint32_t radishlex_key_result_learning_disposition(
    const RadishLexKeyResult *result);

/* Borrowed from result. Do not call radishlex_snapshot_free on this pointer. */
const RadishLexSnapshot *radishlex_key_result_snapshot(
    const RadishLexKeyResult *result);
void radishlex_key_result_free(RadishLexKeyResult *result);

/*
 * Candidate selection can update a segmented composition without committing.
 * Inspect commit_present and the returned snapshot from the same result.
 */
RadishLexStatusCode radishlex_session_select_candidate(
    RadishLexSession *session,
    size_t index,
    RadishLexKeyResult **result_out,
    RadishLexError **error_out);

/* Independent snapshot compatibility API. */
RadishLexSnapshot *radishlex_session_snapshot_new(
    RadishLexSession *session,
    RadishLexError **error_out);
RadishLexStringView radishlex_snapshot_schema(
    const RadishLexSnapshot *snapshot);
RadishLexStringView radishlex_snapshot_preedit(
    const RadishLexSnapshot *snapshot);
size_t radishlex_snapshot_cursor(const RadishLexSnapshot *snapshot);
size_t radishlex_snapshot_candidate_count(
    const RadishLexSnapshot *snapshot);
uint32_t radishlex_snapshot_personalization_status(
    const RadishLexSnapshot *snapshot);
RadishLexStatusCode radishlex_snapshot_candidate(
    const RadishLexSnapshot *snapshot,
    size_t index,
    RadishLexCandidateView *candidate_out,
    RadishLexError **error_out);
void radishlex_snapshot_free(RadishLexSnapshot *snapshot);

const uint8_t *radishlex_buffer_data(const RadishLexBuffer *buffer);
size_t radishlex_buffer_len(const RadishLexBuffer *buffer);
void radishlex_buffer_free(RadishLexBuffer *buffer);

RadishLexStatusCode radishlex_error_code(const RadishLexError *error);
const char *radishlex_error_message(const RadishLexError *error);
void radishlex_error_free(RadishLexError *error);

#if defined(__cplusplus)
} /* extern "C" */
#endif

#endif /* RADISHLEX_INPUT_H */
