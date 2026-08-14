#include "radishlex/linux/ffi_projection.h"

namespace radishlex::linux_platform {

FfiApi linkedFfiApi() {
  return FfiApi{
      radishlex_ffi_contract,
      radishlex_session_new_personalized_rime,
      radishlex_session_free,
      radishlex_rime_runtime_shutdown,
      radishlex_session_engine_kind,
      radishlex_session_reset,
      radishlex_session_set_learning_context,
      radishlex_session_handle_key_event,
      radishlex_session_select_candidate,
      radishlex_key_result_version,
      radishlex_key_result_consumed,
      radishlex_key_result_commit,
      radishlex_key_result_commit_present,
      radishlex_key_result_learning_disposition,
      radishlex_key_result_snapshot,
      radishlex_key_result_free,
      radishlex_snapshot_schema,
      radishlex_snapshot_preedit,
      radishlex_snapshot_cursor,
      radishlex_snapshot_candidate_count,
      radishlex_snapshot_personalization_status,
      radishlex_snapshot_candidate,
      radishlex_error_code,
      radishlex_error_message,
      radishlex_error_free,
  };
}

}  // namespace radishlex::linux_platform
