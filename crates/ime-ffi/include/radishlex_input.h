#ifndef RADISHLEX_INPUT_H
#define RADISHLEX_INPUT_H

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
extern "C" {
#endif

#define RADISHLEX_ABI_CONTRACT_VERSION 4u
#define RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD 1u
#define RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND 1u

#define RADISHLEX_SESSION_OPTIONS_VERSION 1u
#define RADISHLEX_RIME_SESSION_OPTIONS_VERSION 1u
#define RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION 1u
#define RADISHLEX_LEARNING_CONTEXT_VERSION 1u
#define RADISHLEX_ENGINE_KIND_DEMO 1u
#define RADISHLEX_ENGINE_KIND_RIME 2u

#define RADISHLEX_KEY_RESULT_VERSION 2u

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
