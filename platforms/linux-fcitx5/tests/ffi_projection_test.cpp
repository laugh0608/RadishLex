#include "radishlex/linux/ffi_projection.h"
#include "radishlex/linux/key_projection.h"

#include <cstdlib>
#include <iostream>
#include <string>
#include <thread>
#include <vector>

namespace {

using radishlex::linux_platform::ConsumedPressTracker;
using radishlex::linux_platform::FfiApi;
using radishlex::linux_platform::LearningContextProjection;
using radishlex::linux_platform::PersonalizedSessionConfig;
using radishlex::linux_platform::PlatformKeyInput;
using radishlex::linux_platform::PlatformNamedKey;
using radishlex::linux_platform::ProjectionError;
using radishlex::linux_platform::SessionProjection;

struct FakeCandidate {
  std::size_t display_index;
  std::size_t engine_index;
  std::string text;
  std::string reading;
  std::string annotation;
  std::uint32_t source;
};

struct FakeSnapshot {
  std::string schema = "radishlex_pinyin";
  std::string preedit;
  std::size_t cursor = 0;
  std::vector<FakeCandidate> candidates;
  std::uint32_t personalization_status =
      RADISHLEX_PERSONALIZATION_STATUS_READY;
};

struct FakeResult {
  std::uint32_t version = RADISHLEX_KEY_RESULT_VERSION;
  std::uint8_t consumed = 0;
  std::optional<std::string> commit;
  std::uint32_t learning = RADISHLEX_LEARNING_NOT_APPLICABLE;
  FakeSnapshot snapshot;
};

struct FakeSession {
  FakeSnapshot snapshot;
};

struct FakeError {
  RadishLexStatusCode status;
  std::string message;
};

struct FakeRuntime {
  int active_sessions = 0;
  int freed_results = 0;
  std::size_t selected_display_index = 999;
  RadishLexKeyEvent last_event{};
  LearningContextProjection last_context;
  std::vector<std::string> lifecycle;
  std::uint32_t result_version = RADISHLEX_KEY_RESULT_VERSION;
};

FakeRuntime runtime;

template <typename Public, typename Fake>
Public *publicPointer(Fake *value) {
  return reinterpret_cast<Public *>(value);
}

template <typename Fake, typename Public>
Fake *fakePointer(Public *value) {
  return reinterpret_cast<Fake *>(value);
}

template <typename Fake, typename Public>
const Fake *fakePointer(const Public *value) {
  return reinterpret_cast<const Fake *>(value);
}

RadishLexStringView view(const std::string &value) {
  return RadishLexStringView{
      reinterpret_cast<const std::uint8_t *>(value.data()),
      value.size(),
  };
}

RadishLexStatusCode fakeContract(RadishLexFfiContract *output,
                                 RadishLexError **) {
  *output = RadishLexFfiContract{
      RADISHLEX_ABI_CONTRACT_VERSION,
      RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD,
      RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND,
  };
  return RADISHLEX_STATUS_OK;
}

RadishLexSession *fakeNewSession(
    const RadishLexPersonalizedRimeSessionOptions *options,
    RadishLexError **) {
  if (options == nullptr ||
      options->version !=
          RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION ||
      std::string(options->schema) != "radishlex_pinyin") {
    return nullptr;
  }
  ++runtime.active_sessions;
  runtime.lifecycle.push_back("session_new");
  return publicPointer<RadishLexSession>(new FakeSession);
}

void fakeSessionFree(RadishLexSession *session) {
  delete fakePointer<FakeSession>(session);
  --runtime.active_sessions;
  runtime.lifecycle.push_back("session_free");
}

RadishLexStatusCode fakeShutdown(RadishLexError **) {
  if (runtime.active_sessions != 0) {
    return RADISHLEX_STATUS_INVALID_STATE;
  }
  runtime.lifecycle.push_back("runtime_shutdown");
  return RADISHLEX_STATUS_OK;
}

std::uint32_t fakeEngineKind(const RadishLexSession *) {
  return RADISHLEX_ENGINE_KIND_RIME;
}

RadishLexStatusCode fakeReset(RadishLexSession *session, RadishLexError **) {
  FakeSession *fake = fakePointer<FakeSession>(session);
  fake->snapshot.preedit.clear();
  fake->snapshot.cursor = 0;
  fake->snapshot.candidates.clear();
  runtime.lifecycle.push_back("session_reset");
  return RADISHLEX_STATUS_OK;
}

RadishLexStatusCode fakeSetLearningContext(
    RadishLexSession *, RadishLexLearningContext context, RadishLexError **) {
  runtime.last_context.secure_input = context.secure_input == 1;
  runtime.last_context.sensitive_application =
      context.sensitive_application == 1;
  runtime.last_context.privacy_mode = context.privacy_mode == 1;
  runtime.last_context.context_known = context.context_known == 1;
  runtime.last_context.context_kind =
      std::string(reinterpret_cast<const char *>(context.context_kind.data),
                  context.context_kind.len);
  return RADISHLEX_STATUS_OK;
}

FakeResult *makeResult(const FakeSession &session, bool consumed) {
  auto *result = new FakeResult;
  result->version = runtime.result_version;
  result->consumed = static_cast<std::uint8_t>(consumed);
  result->snapshot = session.snapshot;
  return result;
}

RadishLexStatusCode fakeHandleKey(RadishLexSession *session,
                                  RadishLexKeyEvent event,
                                  RadishLexKeyResult **output,
                                  RadishLexError **) {
  runtime.last_event = event;
  FakeSession *fake = fakePointer<FakeSession>(session);
  if (event.phase == RADISHLEX_KEY_PHASE_RELEASE) {
    *output = publicPointer<RadishLexKeyResult>(makeResult(*fake, false));
    return RADISHLEX_STATUS_OK;
  }
  if (event.key_kind == RADISHLEX_KEY_KIND_CHAR) {
    fake->snapshot.preedit.push_back(static_cast<char>(event.codepoint));
    fake->snapshot.cursor = fake->snapshot.preedit.size();
    fake->snapshot.candidates = {
        FakeCandidate{0, 7, "萝卜", "luó bo", "engine", 1},
        FakeCandidate{1, 3, "词核", "cí hé", "personalized", 3},
    };
    *output = publicPointer<RadishLexKeyResult>(makeResult(*fake, true));
    return RADISHLEX_STATUS_OK;
  }
  *output = publicPointer<RadishLexKeyResult>(makeResult(*fake, false));
  return RADISHLEX_STATUS_OK;
}

RadishLexStatusCode fakeSelectCandidate(RadishLexSession *session,
                                        std::size_t display_index,
                                        RadishLexKeyResult **output,
                                        RadishLexError **) {
  runtime.selected_display_index = display_index;
  FakeSession *fake = fakePointer<FakeSession>(session);
  if (display_index >= fake->snapshot.candidates.size()) {
    return RADISHLEX_STATUS_INVALID_ARGUMENT;
  }
  auto *result = makeResult(*fake, true);
  result->commit = fake->snapshot.candidates[display_index].text;
  result->learning = RADISHLEX_LEARNING_RECORDED;
  result->snapshot.preedit.clear();
  result->snapshot.cursor = 0;
  result->snapshot.candidates.clear();
  fake->snapshot = result->snapshot;
  *output = publicPointer<RadishLexKeyResult>(result);
  return RADISHLEX_STATUS_OK;
}

std::uint32_t fakeResultVersion(const RadishLexKeyResult *result) {
  return fakePointer<FakeResult>(result)->version;
}

std::uint8_t fakeResultConsumed(const RadishLexKeyResult *result) {
  return fakePointer<FakeResult>(result)->consumed;
}

RadishLexStringView fakeResultCommit(const RadishLexKeyResult *result) {
  const FakeResult *fake = fakePointer<FakeResult>(result);
  return fake->commit.has_value() ? view(*fake->commit)
                                  : RadishLexStringView{nullptr, 0};
}

std::uint8_t fakeResultCommitPresent(const RadishLexKeyResult *result) {
  return static_cast<std::uint8_t>(
      fakePointer<FakeResult>(result)->commit.has_value());
}

std::uint32_t fakeResultLearning(const RadishLexKeyResult *result) {
  return fakePointer<FakeResult>(result)->learning;
}

const RadishLexSnapshot *fakeResultSnapshot(
    const RadishLexKeyResult *result) {
  return publicPointer<const RadishLexSnapshot>(
      &fakePointer<FakeResult>(result)->snapshot);
}

void fakeResultFree(RadishLexKeyResult *result) {
  delete fakePointer<FakeResult>(result);
  ++runtime.freed_results;
}

RadishLexStringView fakeSnapshotSchema(const RadishLexSnapshot *snapshot) {
  return view(fakePointer<FakeSnapshot>(snapshot)->schema);
}

RadishLexStringView fakeSnapshotPreedit(const RadishLexSnapshot *snapshot) {
  return view(fakePointer<FakeSnapshot>(snapshot)->preedit);
}

std::size_t fakeSnapshotCursor(const RadishLexSnapshot *snapshot) {
  return fakePointer<FakeSnapshot>(snapshot)->cursor;
}

std::size_t fakeSnapshotCount(const RadishLexSnapshot *snapshot) {
  return fakePointer<FakeSnapshot>(snapshot)->candidates.size();
}

std::uint32_t fakeSnapshotPersonalization(
    const RadishLexSnapshot *snapshot) {
  return fakePointer<FakeSnapshot>(snapshot)->personalization_status;
}

RadishLexStatusCode fakeSnapshotCandidate(const RadishLexSnapshot *snapshot,
                                          std::size_t index,
                                          RadishLexCandidateView *output,
                                          RadishLexError **) {
  const auto &candidates = fakePointer<FakeSnapshot>(snapshot)->candidates;
  if (index >= candidates.size()) {
    return RADISHLEX_STATUS_INVALID_ARGUMENT;
  }
  const FakeCandidate &candidate = candidates[index];
  *output = RadishLexCandidateView{
      candidate.display_index,
      candidate.engine_index,
      view(candidate.text),
      view(candidate.reading),
      1,
      view(candidate.annotation),
      1,
      candidate.source,
  };
  return RADISHLEX_STATUS_OK;
}

RadishLexStatusCode fakeErrorCode(const RadishLexError *error) {
  return fakePointer<FakeError>(error)->status;
}

const char *fakeErrorMessage(const RadishLexError *error) {
  return fakePointer<FakeError>(error)->message.c_str();
}

void fakeErrorFree(RadishLexError *error) {
  delete fakePointer<FakeError>(error);
}

FfiApi fakeApi() {
  return FfiApi{
      fakeContract,
      fakeNewSession,
      fakeSessionFree,
      fakeShutdown,
      fakeEngineKind,
      fakeReset,
      fakeSetLearningContext,
      fakeHandleKey,
      fakeSelectCandidate,
      fakeResultVersion,
      fakeResultConsumed,
      fakeResultCommit,
      fakeResultCommitPresent,
      fakeResultLearning,
      fakeResultSnapshot,
      fakeResultFree,
      fakeSnapshotSchema,
      fakeSnapshotPreedit,
      fakeSnapshotCursor,
      fakeSnapshotCount,
      fakeSnapshotPersonalization,
      fakeSnapshotCandidate,
      fakeErrorCode,
      fakeErrorMessage,
      fakeErrorFree,
  };
}

void require(bool condition, const char *message) {
  if (!condition) {
    std::cerr << message << '\n';
    std::exit(1);
  }
}

PersonalizedSessionConfig config() {
  return PersonalizedSessionConfig{
      "/product/rime",
      "/user/data/radishlex/rime",
      "radishlex_pinyin",
      std::nullopt,
      true,
      "/user/data/radishlex/userdb.sqlite3",
      "linux-contract-1",
  };
}

void testKeyProjection() {
  PlatformKeyInput character;
  character.codepoint = 0x83dcU;
  character.modifiers = RADISHLEX_KEY_MOD_SHIFT;
  const auto projected =
      radishlex::linux_platform::projectKeyEvent(character);
  require(projected.has_value(), "Unicode key must project");
  require(projected->key_kind == RADISHLEX_KEY_KIND_CHAR,
          "Unicode key kind must be char");
  require(projected->codepoint == 0x83dcU,
          "Unicode scalar must be preserved");

  PlatformKeyInput page_down;
  page_down.named_key = PlatformNamedKey::PageDown;
  page_down.release = true;
  const auto named = radishlex::linux_platform::projectKeyEvent(page_down);
  require(named.has_value(), "named key must project");
  require(named->named_key == RADISHLEX_NAMED_KEY_PAGE_DOWN,
          "named key code must match ABI");
  require(named->phase == RADISHLEX_KEY_PHASE_RELEASE,
          "release phase must be preserved");

  PlatformKeyInput surrogate;
  surrogate.codepoint = 0xd800U;
  require(!radishlex::linux_platform::projectKeyEvent(surrogate).has_value(),
          "surrogate must be rejected");
  PlatformKeyInput reserved = character;
  reserved.platform_reserved = true;
  require(!radishlex::linux_platform::projectKeyEvent(reserved).has_value(),
          "platform-reserved key must pass through");
}

void testConsumedPressLifecycle() {
  constexpr std::uint32_t kArrowDown = 0xff54U;
  constexpr std::uint32_t kArrowUp = 0xff52U;
  constexpr std::uint32_t kDigitTwo = 0x32U;
  constexpr std::uint32_t kSpace = 0x20U;
  ConsumedPressTracker tracker;

  tracker.recordPress(kArrowDown);
  require(!tracker.consumeRelease(kArrowUp),
          "Up release must not match a consumed Down press");
  require(tracker.consumeRelease(kArrowDown),
          "Down release must match its consumed navigation press");
  require(!tracker.consumeRelease(kArrowDown),
          "a matched Down release must be removed");

  tracker.recordPress(kArrowUp);
  require(tracker.consumeRelease(kArrowUp),
          "Up release must match its consumed navigation press");

  tracker.recordPress(kDigitTwo);
  require(tracker.consumeRelease(kDigitTwo),
          "digit selection release must match its consumed press");

  tracker.recordPress(kSpace);
  require(tracker.consumeRelease(kSpace),
          "Space commit release must match its consumed press");

  tracker.recordPress(kArrowDown);
  tracker.recordPress(kDigitTwo);
  tracker.clear();
  require(!tracker.consumeRelease(kArrowDown) &&
              !tracker.consumeRelease(kDigitTwo),
          "reset or deactivate must clear consumed press state");
}

void testClientPreeditProjection() {
  radishlex::linux_platform::SnapshotProjection snapshot{
      "radishlex_pinyin",
      "ceshi",
      5,
      {},
      RADISHLEX_PERSONALIZATION_STATUS_READY,
  };
  const auto projected =
      radishlex::linux_platform::projectClientPreedit(snapshot);
  require(projected.text == "ceshi",
          "client preedit text must match the Rust snapshot");
  require(projected.cursor == 5,
          "client preedit cursor must preserve the UTF-8 byte offset");
  require(projected.prevent_commit_on_unfocus,
          "raw composition must not commit when the context loses focus");

  snapshot.preedit = "测a";
  snapshot.cursor = snapshot.preedit.size();
  require(radishlex::linux_platform::projectClientPreedit(snapshot).cursor == 4,
          "multibyte client preedit cursor must remain a byte offset");

  snapshot.cursor = 1;
  bool rejected_cursor = false;
  try {
    (void)radishlex::linux_platform::projectClientPreedit(snapshot);
  } catch (const ProjectionError &error) {
    rejected_cursor = error.status() == RADISHLEX_STATUS_INVALID_STATE;
  }
  require(rejected_cursor,
          "client preedit must reject a cursor inside a UTF-8 codepoint");
}

void testSessionProjection() {
  runtime = FakeRuntime{};
  const FfiApi api = fakeApi();
  radishlex::linux_platform::validateFfiContract(api);
  {
    SessionProjection session(api, config());
    LearningContextProjection context;
    context.secure_input = true;
    context.sensitive_application = true;
    context.privacy_mode = true;
    context.context_known = false;
    context.context_kind = "general";

    RadishLexKeyEvent key{
        RADISHLEX_KEY_KIND_CHAR,
        static_cast<std::uint32_t>('l'),
        0,
        0,
        RADISHLEX_KEY_PHASE_PRESS,
    };
    const auto result = session.handleKeyEvent(key, context);
    require(result.consumed, "character key must be consumed");
    require(result.snapshot.preedit == "l",
            "snapshot must come from the same key result");
    require(result.snapshot.cursor == 1,
            "snapshot cursor must be copied");
    require(result.snapshot.candidates.size() == 2,
            "candidate page must be copied");
    require(result.snapshot.candidates[1].display_index == 1 &&
                result.snapshot.candidates[1].engine_index == 3,
            "display and engine index must remain distinct");
    require(runtime.last_context.secure_input &&
                runtime.last_context.sensitive_application &&
                runtime.last_context.privacy_mode &&
                !runtime.last_context.context_known,
            "learning context must be set before key handling");
    require(runtime.freed_results == 1,
            "owned key result must be released after copying");
    require(session.hasComposition(),
            "session must retain composition projection");

    {
      SessionProjection second(api, PersonalizedSessionConfig{
                                        "/product/rime",
                                        "/user/data/radishlex/rime",
                                        "radishlex_pinyin",
                                        std::nullopt,
                                        true,
                                        "/user/data/radishlex/userdb.sqlite3",
                                        "linux-contract-2",
                                    });
      RadishLexKeyEvent other_key = key;
      other_key.codepoint = static_cast<std::uint32_t>('x');
      const auto other_result = second.handleKeyEvent(other_key, context);
      require(other_result.snapshot.preedit == "x",
              "second context must have an independent session");
      require(session.hasComposition(),
              "second context must not reset the first session");
    }

    const auto selection = session.selectCandidate(1, context);
    require(runtime.selected_display_index == 1,
            "selection must pass display index unchanged");
    require(selection.commit == std::optional<std::string>("词核"),
            "selection commit must be copied");
    require(selection.learning_disposition == RADISHLEX_LEARNING_RECORDED,
            "learning disposition must be preserved");
    require(!session.hasComposition(),
            "commit snapshot must clear composition");

    session.reset();
    require(!session.hasComposition(), "reset must clear projected state");

    runtime.result_version = RADISHLEX_KEY_RESULT_VERSION + 1;
    bool rejected_version = false;
    try {
      session.handleKeyEvent(key, context);
    } catch (const ProjectionError &error) {
      rejected_version = error.status() == RADISHLEX_STATUS_INVALID_STATE;
    }
    require(rejected_version, "unknown key result version must fail closed");
    require(runtime.freed_results == 4,
            "invalid owned result must still be released");
    runtime.result_version = RADISHLEX_KEY_RESULT_VERSION;

    bool rejected = false;
    std::thread other([&] {
      try {
        session.reset();
      } catch (const ProjectionError &error) {
        rejected = error.status() == RADISHLEX_STATUS_INVALID_STATE;
      }
    });
    other.join();
    require(rejected, "non-owner thread must be rejected before FFI");
  }
  radishlex::linux_platform::shutdownRimeRuntime(api);
  require(runtime.lifecycle.size() >= 4,
          "lifecycle events must be recorded");
  require(runtime.lifecycle[runtime.lifecycle.size() - 2] == "session_free" &&
              runtime.lifecycle.back() == "runtime_shutdown",
          "session must free before runtime shutdown");
}

}  // namespace

int main() {
  testKeyProjection();
  testConsumedPressLifecycle();
  testClientPreeditProjection();
  testSessionProjection();
  std::cout << "Linux FFI projection contract passed.\n";
  return 0;
}
