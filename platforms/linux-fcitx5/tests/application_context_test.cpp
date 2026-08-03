#include "radishlex/linux/application_context.h"

#include <cstdlib>
#include <iostream>
#include <string>

namespace {

using radishlex::linux_platform::ApplicationContextInput;
using radishlex::linux_platform::LearningContextProjection;
using radishlex::linux_platform::ReviewedApplicationKind;
using radishlex::linux_platform::ReviewedApplicationRule;

constexpr ReviewedApplicationRule kSyntheticRules[] = {
    {"org.radishlex.synthetic.general", ReviewedApplicationKind::General},
    {"org.radishlex.synthetic.browser", ReviewedApplicationKind::Browser},
    {"org.radishlex.synthetic.chat", ReviewedApplicationKind::Chat},
    {"org.radishlex.synthetic.code", ReviewedApplicationKind::Code},
    {"org.radishlex.synthetic.editor", ReviewedApplicationKind::Editor},
    {"org.radishlex.synthetic.office", ReviewedApplicationKind::Office},
    {"", ReviewedApplicationKind::Editor},
};

void require(bool condition, const char *message) {
  if (!condition) {
    std::cerr << message << '\n';
    std::exit(1);
  }
}

LearningContextProjection project(ApplicationContextInput input) {
  return radishlex::linux_platform::projectApplicationContextForTesting(
      input, kSyntheticRules,
      sizeof(kSyntheticRules) / sizeof(kSyntheticRules[0]));
}

void requireUnknown(const LearningContextProjection &context,
                    const char *message) {
  require(!context.context_known && context.context_kind == "other", message);
}

void testProductionStartsFailClosed() {
  const auto context =
      radishlex::linux_platform::projectApplicationContext(
          ApplicationContextInput{false, false, false, false,
                                  "org.radishlex.synthetic.editor"});
  requireUnknown(context,
                 "unreviewed production identities must remain unknown");
}

void testReviewedRulesAreExactAndCoarse() {
  const struct {
    const char *program;
    const char *kind;
  } cases[] = {
      {"org.radishlex.synthetic.general", "general"},
      {"org.radishlex.synthetic.browser", "browser"},
      {"org.radishlex.synthetic.chat", "chat"},
      {"org.radishlex.synthetic.code", "code"},
      {"org.radishlex.synthetic.editor", "editor"},
      {"org.radishlex.synthetic.office", "office"},
  };

  for (const auto &item : cases) {
    const auto context = project(ApplicationContextInput{
        false, false, false, false, item.program});
    require(context.context_known,
            "reviewed synthetic identity must become known");
    require(context.context_kind == item.kind,
            "reviewed identity must emit only its coarse category");
    require(context.context_kind.find(item.program) == std::string::npos,
            "raw identity must not appear in the projected category");
  }

  requireUnknown(project(ApplicationContextInput{}),
                 "empty program identity must remain unknown");
  requireUnknown(project(ApplicationContextInput{false, false, false, false,
                                                 "ORG.RADISHLEX.SYNTHETIC.EDITOR"}),
                 "case variants must not bypass the exact allowlist");
  requireUnknown(project(ApplicationContextInput{false, false, false, false,
                                                 "wrapper:org.radishlex.synthetic.editor"}),
                 "wrapper variants must not bypass the exact allowlist");
  requireUnknown(project(ApplicationContextInput{false, false, false, false,
                                                 "org.radishlex.unknown"}),
                 "unknown identity must remain fail-closed");
}

void testCapabilityAndPrivacyPriority() {
  const auto privacy = project(ApplicationContextInput{
      false, false, false, true, "org.radishlex.synthetic.editor"});
  require(privacy.privacy_mode && privacy.context_known &&
              privacy.context_kind == "editor",
          "privacy must preserve a reviewed coarse category for read-only use");

  const auto password = project(ApplicationContextInput{
      true, false, false, false, "org.radishlex.synthetic.editor"});
  require(password.secure_input && !password.sensitive_application,
          "password capability must project only its safety bit");
  requireUnknown(password,
                 "password capability must bypass application classification");

  const auto sensitive = project(ApplicationContextInput{
      false, true, false, false, "org.radishlex.synthetic.editor"});
  require(!sensitive.secure_input && sensitive.sensitive_application,
          "sensitive capability must project only its safety bit");
  requireUnknown(
      sensitive,
      "sensitive capability must bypass application classification");

  const auto terminal = project(ApplicationContextInput{
      false, false, true, true, "org.radishlex.synthetic.editor"});
  require(terminal.privacy_mode && !terminal.context_known &&
              terminal.context_kind == "terminal",
          "terminal capability must remain non-learning ahead of identity");
}

}  // namespace

int main() {
  testProductionStartsFailClosed();
  testReviewedRulesAreExactAndCoarse();
  testCapabilityAndPrivacyPriority();
  return 0;
}
