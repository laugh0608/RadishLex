#ifndef RADISHLEX_LINUX_FFI_PROJECTION_H
#define RADISHLEX_LINUX_FFI_PROJECTION_H

#include <cstddef>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

#include "radishlex_input.h"

namespace radishlex::linux_platform {

struct CandidateProjection {
  std::size_t display_index;
  std::size_t engine_index;
  std::string text;
  std::optional<std::string> reading;
  std::optional<std::string> annotation;
  std::uint32_t source;
};

struct SnapshotProjection {
  std::string schema;
  std::string preedit;
  std::size_t cursor;
  std::vector<CandidateProjection> candidates;
  std::uint32_t personalization_status;
};

struct ClientPreeditProjection {
  std::string text;
  int cursor;
  bool prevent_commit_on_unfocus;
};

struct KeyResultProjection {
  bool consumed;
  std::optional<std::string> commit;
  std::uint32_t learning_disposition;
  SnapshotProjection snapshot;
};

struct LearningContextProjection {
  bool secure_input = false;
  bool sensitive_application = false;
  bool privacy_mode = false;
  bool context_known = false;
  std::string context_kind = "general";
};

struct PersonalizedSessionConfig {
  std::string shared_data_dir;
  std::string user_data_dir;
  std::string schema;
  std::optional<std::string> log_dir;
  bool deploy_on_start = true;
  std::string userdb_path;
  std::string session_id;
};

class ProjectionError final : public std::runtime_error {
 public:
  ProjectionError(RadishLexStatusCode status, const std::string &message);

  RadishLexStatusCode status() const noexcept;

 private:
  RadishLexStatusCode status_;
};

ClientPreeditProjection projectClientPreedit(
    const SnapshotProjection &snapshot);

struct FfiApi {
  RadishLexStatusCode (*ffi_contract)(RadishLexFfiContract *,
                                      RadishLexError **);
  RadishLexSession *(*session_new_personalized_rime)(
      const RadishLexPersonalizedRimeSessionOptions *, RadishLexError **);
  void (*session_free)(RadishLexSession *);
  RadishLexStatusCode (*rime_runtime_shutdown)(RadishLexError **);
  std::uint32_t (*session_engine_kind)(const RadishLexSession *);
  RadishLexStatusCode (*session_reset)(RadishLexSession *, RadishLexError **);
  RadishLexStatusCode (*session_set_learning_context)(
      RadishLexSession *, RadishLexLearningContext, RadishLexError **);
  RadishLexStatusCode (*session_handle_key_event)(
      RadishLexSession *, RadishLexKeyEvent, RadishLexKeyResult **,
      RadishLexError **);
  RadishLexStatusCode (*session_select_candidate)(
      RadishLexSession *, std::size_t, RadishLexKeyResult **,
      RadishLexError **);
  std::uint32_t (*key_result_version)(const RadishLexKeyResult *);
  std::uint8_t (*key_result_consumed)(const RadishLexKeyResult *);
  RadishLexStringView (*key_result_commit)(const RadishLexKeyResult *);
  std::uint8_t (*key_result_commit_present)(const RadishLexKeyResult *);
  std::uint32_t (*key_result_learning_disposition)(
      const RadishLexKeyResult *);
  const RadishLexSnapshot *(*key_result_snapshot)(
      const RadishLexKeyResult *);
  void (*key_result_free)(RadishLexKeyResult *);
  RadishLexStringView (*snapshot_schema)(const RadishLexSnapshot *);
  RadishLexStringView (*snapshot_preedit)(const RadishLexSnapshot *);
  std::size_t (*snapshot_cursor)(const RadishLexSnapshot *);
  std::size_t (*snapshot_candidate_count)(const RadishLexSnapshot *);
  std::uint32_t (*snapshot_personalization_status)(
      const RadishLexSnapshot *);
  RadishLexStatusCode (*snapshot_candidate)(
      const RadishLexSnapshot *, std::size_t, RadishLexCandidateView *,
      RadishLexError **);
  RadishLexStatusCode (*error_code)(const RadishLexError *);
  const char *(*error_message)(const RadishLexError *);
  void (*error_free)(RadishLexError *);
};

FfiApi linkedFfiApi();
void validateFfiContract(const FfiApi &api);
void shutdownRimeRuntime(const FfiApi &api);

class SessionProjection final {
 public:
  SessionProjection(const FfiApi &api,
                    const PersonalizedSessionConfig &config);
  ~SessionProjection();

  SessionProjection(const SessionProjection &) = delete;
  SessionProjection &operator=(const SessionProjection &) = delete;
  SessionProjection(SessionProjection &&) = delete;
  SessionProjection &operator=(SessionProjection &&) = delete;

  KeyResultProjection handleKeyEvent(
      const RadishLexKeyEvent &event,
      const LearningContextProjection &learning_context);
  KeyResultProjection selectCandidate(
      std::size_t display_index,
      const LearningContextProjection &learning_context);
  void reset();
  bool hasComposition() const noexcept;

 private:
  void requireOwnerThread() const;
  void setLearningContext(const LearningContextProjection &context);
  KeyResultProjection copyResult(RadishLexKeyResult *result);

  const FfiApi &api_;
  RadishLexSession *session_;
  std::thread::id owner_thread_;
  bool has_composition_;
};

}  // namespace radishlex::linux_platform

#endif
