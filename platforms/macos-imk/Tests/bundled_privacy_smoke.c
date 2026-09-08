#include "radishlex_input.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

// Link this test to the assembled InputMethod's dylib, never a rebuilt Rust test library.
// The caller creates a fresh private work directory for each synthetic scenario.
static RadishLexError *error = NULL;

static void require(int condition, const char *message) {
  if (condition) return;
  fprintf(stderr, "%s: %s\n", message,
          error ? radishlex_error_message(error) : "assertion failed");
  radishlex_error_free(error);
  exit(1);
}

static RadishLexLearningContext context(int mode) {
  RadishLexLearningContext value = {
      .version = RADISHLEX_LEARNING_CONTEXT_VERSION,
      .context_known = mode != 2,
      .context_kind = {(const uint8_t *)"editor", 6},
      .privacy_mode = mode == 1,
      .secure_input = mode == 3,
      .sensitive_application = mode == 4,
  };
  return value;
}

static void set_context(RadishLexSession *session, int mode) {
  require(radishlex_session_set_learning_context(session, context(mode), &error) ==
              RADISHLEX_STATUS_OK,
          "set synthetic context");
}

int main(int argc, const char *argv[]) {
  if (argc != 4 || strlen(argv[3]) != 1 || argv[3][0] < '0' || argv[3][0] > '6') {
    fprintf(stderr, "usage: probe <product RimeData> <fresh private work dir> <case 0..6>\n");
    return 2;
  }
  int mode = argv[3][0] - '0';
  struct stat metadata;
  require(lstat(argv[2], &metadata) == 0 && S_ISDIR(metadata.st_mode) &&
              metadata.st_uid == getuid() && (metadata.st_mode & 0777) == 0700,
          "work directory must be owned and private");
  char user[4096], logs[4096], database[4096];
  require(snprintf(user, sizeof(user), "%s/user", argv[2]) < (int)sizeof(user) &&
              snprintf(logs, sizeof(logs), "%s/logs", argv[2]) < (int)sizeof(logs) &&
              snprintf(database, sizeof(database), "%s/userdb.sqlite3", argv[2]) <
                  (int)sizeof(database), "bounded paths");
  require(lstat(database, &metadata) != 0 && errno == ENOENT, "existing or inaccessible database is forbidden");
  require(mkdir(user, 0700) == 0 && mkdir(logs, 0700) == 0, "fresh runtime directories");
  RadishLexPersonalizedRimeSessionOptions options = {
      .version = RADISHLEX_PERSONALIZED_RIME_SESSION_OPTIONS_VERSION,
      .shared_data_dir = argv[1], .user_data_dir = user,
      .schema = "radishlex_pinyin", .log_dir = logs, .deploy_on_start = 1,
      .userdb_path = database, .session_id = "synthetic-bundled-privacy",
  };
  RadishLexSession *session = radishlex_session_new_personalized_rime(&options, &error);
  require(session != NULL && error == NULL, "bundled session initialization");
  set_context(session, mode == 5 ? 1 : (mode == 6 ? 0 : mode));
  const char *input = "shi";
  for (size_t i = 0; input[i]; ++i) {
    RadishLexKeyEvent key = {.key_kind = RADISHLEX_KEY_KIND_CHAR,
                            .codepoint = (uint32_t)input[i], .phase = RADISHLEX_KEY_PHASE_PRESS};
    RadishLexKeyResult *result = NULL;
    require(radishlex_session_handle_key_event(session, key, &result, &error) ==
                RADISHLEX_STATUS_OK && result != NULL, "synthetic input");
    require(!radishlex_key_result_commit_present(result), "composition remains uncommitted");
    radishlex_key_result_free(result);
  }
  if (mode == 6) set_context(session, 1);
  if (mode >= 5) set_context(session, 0);
  RadishLexSnapshot *snapshot = radishlex_session_snapshot_new(session, &error);
  require(snapshot != NULL && error == NULL, "candidate snapshot");
  require(radishlex_snapshot_candidate_count(snapshot) == 5, "product candidate page size");
  RadishLexCandidateView candidate = {0};
  require(radishlex_snapshot_candidate(snapshot, 1, &candidate, &error) == RADISHLEX_STATUS_OK,
          "second synthetic candidate");
  RadishLexKeyResult *selection = NULL;
  require(radishlex_session_select_candidate(session, 1, &selection, &error) ==
              RADISHLEX_STATUS_OK && selection != NULL, "select synthetic candidate");
  RadishLexStringView commit = radishlex_key_result_commit(selection);
  require(radishlex_key_result_commit_present(selection) && commit.len > 0 &&
              commit.len == candidate.text.len &&
              memcmp(commit.data, candidate.text.data, commit.len) == 0, "commit is preserved");
  uint32_t disposition = radishlex_key_result_learning_disposition(selection);
  require(disposition == (mode == 0 ? RADISHLEX_LEARNING_RECORDED :
                                     RADISHLEX_LEARNING_SKIPPED_BY_POLICY), "learning policy");
  printf("case=%d disposition=%u commit_preserved=1\n", mode, disposition);
  radishlex_key_result_free(selection);
  radishlex_snapshot_free(snapshot);
  radishlex_session_free(session);
  require(radishlex_rime_runtime_shutdown(&error) == RADISHLEX_STATUS_OK, "runtime shutdown");
  return 0;
}
