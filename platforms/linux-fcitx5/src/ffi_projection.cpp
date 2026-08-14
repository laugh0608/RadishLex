#include "radishlex/linux/ffi_projection.h"

#include <array>
#include <limits>
#include <utility>

namespace radishlex::linux_platform {
namespace {

class OwnedError final {
 public:
  explicit OwnedError(const FfiApi &api) : api_(api), value_(nullptr) {}
  ~OwnedError() {
    if (value_ != nullptr) {
      api_.error_free(value_);
    }
  }

  RadishLexError **out() { return &value_; }

  std::string message(const char *fallback) const {
    if (value_ == nullptr) {
      return fallback;
    }
    const char *message = api_.error_message(value_);
    return message == nullptr ? fallback : message;
  }

 private:
  const FfiApi &api_;
  RadishLexError *value_;
};

class OwnedResult final {
 public:
  OwnedResult(const FfiApi &api, RadishLexKeyResult *value)
      : api_(api), value_(value) {}
  ~OwnedResult() {
    if (value_ != nullptr) {
      api_.key_result_free(value_);
    }
  }

 private:
  const FfiApi &api_;
  RadishLexKeyResult *value_;
};

void requireStatus(RadishLexStatusCode status, OwnedError &error,
                   const char *fallback) {
  if (status != RADISHLEX_STATUS_OK) {
    throw ProjectionError(status, error.message(fallback));
  }
}

bool isContinuationByte(std::uint8_t byte) {
  return (byte & 0xc0U) == 0x80U;
}

bool isValidUtf8(const std::uint8_t *data, std::size_t len) {
  if (len == 0) {
    return true;
  }
  if (data == nullptr) {
    return false;
  }

  std::size_t index = 0;
  while (index < len) {
    const std::uint8_t first = data[index];
    std::size_t width = 0;
    std::uint32_t codepoint = 0;
    if (first <= 0x7fU) {
      width = 1;
      codepoint = first;
    } else if (first >= 0xc2U && first <= 0xdfU) {
      width = 2;
      codepoint = first & 0x1fU;
    } else if (first >= 0xe0U && first <= 0xefU) {
      width = 3;
      codepoint = first & 0x0fU;
    } else if (first >= 0xf0U && first <= 0xf4U) {
      width = 4;
      codepoint = first & 0x07U;
    } else {
      return false;
    }
    if (index + width > len) {
      return false;
    }
    for (std::size_t offset = 1; offset < width; ++offset) {
      const std::uint8_t continuation = data[index + offset];
      if (!isContinuationByte(continuation)) {
        return false;
      }
      codepoint = (codepoint << 6U) | (continuation & 0x3fU);
    }
    if ((width == 2 && codepoint < 0x80U) ||
        (width == 3 && codepoint < 0x800U) ||
        (width == 4 && codepoint < 0x10000U) ||
        (codepoint >= 0xd800U && codepoint <= 0xdfffU) ||
        codepoint > 0x10ffffU) {
      return false;
    }
    index += width;
  }
  return true;
}

std::string copyView(RadishLexStringView view, const char *field) {
  if (!isValidUtf8(view.data, view.len)) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          std::string(field) + " is not valid UTF-8");
  }
  if (view.len == 0) {
    return {};
  }
  return std::string(reinterpret_cast<const char *>(view.data), view.len);
}

std::optional<std::string> copyOptionalView(RadishLexStringView view,
                                            std::uint8_t present,
                                            const char *field) {
  if (present > 1) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          std::string(field) + " presence is not boolean");
  }
  if (present == 0) {
    if (view.len != 0) {
      throw ProjectionError(
          RADISHLEX_STATUS_INVALID_STATE,
          std::string(field) + " has bytes while marked absent");
    }
    return std::nullopt;
  }
  return copyView(view, field);
}

bool isUtf8Boundary(const std::string &text, std::size_t offset) {
  return offset <= text.size() &&
         (offset == text.size() ||
          !isContinuationByte(static_cast<std::uint8_t>(text[offset])));
}

bool knownCandidateSource(std::uint32_t source) {
  return source >= RADISHLEX_CANDIDATE_SOURCE_ENGINE &&
         source <= RADISHLEX_CANDIDATE_SOURCE_SYSTEM;
}

bool knownPersonalizationStatus(std::uint32_t status) {
  return status <= RADISHLEX_PERSONALIZATION_STATUS_RANK_FAILED;
}

bool knownLearningDisposition(std::uint32_t disposition) {
  return disposition <= RADISHLEX_LEARNING_FAILED;
}

SnapshotProjection copySnapshot(const FfiApi &api,
                                const RadishLexSnapshot *snapshot) {
  if (snapshot == nullptr) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "key result snapshot is null");
  }

  SnapshotProjection output{
      copyView(api.snapshot_schema(snapshot), "snapshot schema"),
      copyView(api.snapshot_preedit(snapshot), "snapshot preedit"),
      api.snapshot_cursor(snapshot),
      {},
      api.snapshot_personalization_status(snapshot),
  };
  if (!isUtf8Boundary(output.preedit, output.cursor)) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "snapshot cursor is not a UTF-8 boundary");
  }
  if (!knownPersonalizationStatus(output.personalization_status)) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "snapshot personalization status is unknown");
  }

  const std::size_t count = api.snapshot_candidate_count(snapshot);
  if (count > 1024) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "snapshot candidate count exceeds platform limit");
  }
  output.candidates.reserve(count);
  for (std::size_t index = 0; index < count; ++index) {
    RadishLexCandidateView view{};
    OwnedError error(api);
    requireStatus(api.snapshot_candidate(snapshot, index, &view, error.out()),
                  error, "unable to copy snapshot candidate");
    if (view.index != index) {
      throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                            "candidate display index is inconsistent");
    }
    if (!knownCandidateSource(view.source)) {
      throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                            "candidate source is unknown");
    }
    output.candidates.push_back(CandidateProjection{
        view.index,
        view.engine_index,
        copyView(view.text, "candidate text"),
        copyOptionalView(view.reading, view.reading_present,
                         "candidate reading"),
        copyOptionalView(view.annotation, view.annotation_present,
                         "candidate annotation"),
        view.source,
    });
  }
  return output;
}

}  // namespace

ProjectionError::ProjectionError(RadishLexStatusCode status,
                                 const std::string &message)
    : std::runtime_error(message), status_(status) {}

RadishLexStatusCode ProjectionError::status() const noexcept { return status_; }

ClientPreeditProjection projectClientPreedit(
    const SnapshotProjection &snapshot) {
  if (!isUtf8Boundary(snapshot.preedit, snapshot.cursor) ||
      snapshot.cursor >
          static_cast<std::size_t>(std::numeric_limits<int>::max())) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "client preedit cursor is invalid");
  }
  return ClientPreeditProjection{
      snapshot.preedit,
      static_cast<int>(snapshot.cursor),
      true,
  };
}

void validateFfiContract(const FfiApi &api) {
  RadishLexFfiContract contract{};
  OwnedError error(api);
  requireStatus(api.ffi_contract(&contract, error.out()), error,
                "unable to read FFI contract");
  if (contract.version != RADISHLEX_ABI_CONTRACT_VERSION ||
      contract.session_thread_policy !=
          RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD ||
      contract.panic_boundary != RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "FFI contract does not match Linux addon");
  }
}

void shutdownRimeRuntime(const FfiApi &api) {
  OwnedError error(api);
  requireStatus(api.rime_runtime_shutdown(error.out()), error,
                "Rime runtime shutdown failed");
}

SessionProjection::SessionProjection(const FfiApi &api,
                                     const PersonalizedSessionConfig &config)
    : api_(api),
      session_(nullptr),
      owner_thread_(std::this_thread::get_id()),
      has_composition_(false) {
  validateFfiContract(api_);
  const char *log_dir =
      config.log_dir.has_value() ? config.log_dir->c_str() : nullptr;
  RadishLexPersonalizedRimeSessionOptions options{
      RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION,
      config.shared_data_dir.c_str(),
      config.user_data_dir.c_str(),
      config.schema.c_str(),
      log_dir,
      static_cast<std::uint8_t>(config.deploy_on_start),
      config.userdb_path.c_str(),
      config.session_id.c_str(),
  };
  OwnedError error(api_);
  session_ = api_.session_new_personalized_rime(&options, error.out());
  if (session_ == nullptr) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          error.message("personalized Rime session failed"));
  }
  if (api_.session_engine_kind(session_) != RADISHLEX_ENGINE_KIND_RIME) {
    api_.session_free(session_);
    session_ = nullptr;
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "personalized session is not a Rime session");
  }
}

SessionProjection::~SessionProjection() {
  if (session_ != nullptr && owner_thread_ == std::this_thread::get_id()) {
    api_.session_free(session_);
  }
}

KeyResultProjection SessionProjection::handleKeyEvent(
    const RadishLexKeyEvent &event,
    const LearningContextProjection &learning_context) {
  requireOwnerThread();
  setLearningContext(learning_context);
  RadishLexKeyResult *result = nullptr;
  OwnedError error(api_);
  requireStatus(api_.session_handle_key_event(session_, event, &result,
                                              error.out()),
                error, "key event failed");
  return copyResult(result);
}

KeyResultProjection SessionProjection::selectCandidate(
    std::size_t display_index,
    const LearningContextProjection &learning_context) {
  requireOwnerThread();
  setLearningContext(learning_context);
  RadishLexKeyResult *result = nullptr;
  OwnedError error(api_);
  requireStatus(api_.session_select_candidate(session_, display_index, &result,
                                              error.out()),
                error, "candidate selection failed");
  return copyResult(result);
}

void SessionProjection::updateLearningContext(
    const LearningContextProjection &context) {
  requireOwnerThread();
  setLearningContext(context);
}

void SessionProjection::reset() {
  requireOwnerThread();
  OwnedError error(api_);
  requireStatus(api_.session_reset(session_, error.out()), error,
                "session reset failed");
  has_composition_ = false;
}

bool SessionProjection::hasComposition() const noexcept {
  return has_composition_;
}

void SessionProjection::requireOwnerThread() const {
  if (owner_thread_ != std::this_thread::get_id()) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "session call is not on its owner thread");
  }
}

void SessionProjection::setLearningContext(
    const LearningContextProjection &context) {
  const RadishLexStringView context_kind{
      reinterpret_cast<const std::uint8_t *>(context.context_kind.data()),
      context.context_kind.size(),
  };
  const RadishLexLearningContext ffi_context{
      RADISHLEX_LEARNING_CONTEXT_VERSION,
      static_cast<std::uint8_t>(context.secure_input),
      static_cast<std::uint8_t>(context.sensitive_application),
      static_cast<std::uint8_t>(context.privacy_mode),
      static_cast<std::uint8_t>(context.context_known),
      context_kind,
  };
  OwnedError error(api_);
  requireStatus(api_.session_set_learning_context(session_, ffi_context,
                                                  error.out()),
                error, "learning context failed");
}

KeyResultProjection SessionProjection::copyResult(
    RadishLexKeyResult *result) {
  if (result == nullptr) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "successful FFI call returned a null key result");
  }
  OwnedResult owned(api_, result);
  if (api_.key_result_version(result) != RADISHLEX_KEY_RESULT_VERSION) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "key result version is unsupported");
  }
  const std::uint8_t consumed = api_.key_result_consumed(result);
  const std::uint8_t commit_present = api_.key_result_commit_present(result);
  if (consumed > 1 || commit_present > 1) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "key result contains a non-boolean field");
  }
  const std::uint32_t learning =
      api_.key_result_learning_disposition(result);
  if (!knownLearningDisposition(learning)) {
    throw ProjectionError(RADISHLEX_STATUS_INVALID_STATE,
                          "key result learning disposition is unknown");
  }

  KeyResultProjection output{
      consumed == 1,
      copyOptionalView(api_.key_result_commit(result), commit_present,
                       "key result commit"),
      learning,
      copySnapshot(api_, api_.key_result_snapshot(result)),
  };
  has_composition_ = !output.snapshot.preedit.empty();
  return output;
}

}  // namespace radishlex::linux_platform
