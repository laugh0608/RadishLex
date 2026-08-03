#include "radishlex/linux/application_context.h"
#include "radishlex/linux/application_evidence.h"

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

void testApplicationEvidenceCandidatesAreExactAndOpaque() {
  using radishlex::linux_platform::ApplicationEvidenceCandidate;
  using radishlex::linux_platform::applicationEvidenceCandidateCode;
  using radishlex::linux_platform::matchApplicationEvidenceCandidate;

  const struct {
    const char *program;
    ApplicationEvidenceCandidate candidate;
    const char *code;
  } cases[] = {
      {"firefox-esr", ApplicationEvidenceCandidate::FirefoxEsrExecutable,
       "browser_candidate_01"},
      {"/usr/lib/firefox-esr/firefox-esr",
       ApplicationEvidenceCandidate::FirefoxEsrPath,
       "browser_candidate_02"},
      {"firefox", ApplicationEvidenceCandidate::FirefoxExecutable,
       "browser_candidate_03"},
      {"org.mozilla.firefox",
       ApplicationEvidenceCandidate::MozillaFirefoxIdentity,
       "browser_candidate_04"},
  };

  for (const auto &item : cases) {
    const auto candidate = matchApplicationEvidenceCandidate(item.program);
    const auto code = applicationEvidenceCandidateCode(candidate);
    require(candidate == item.candidate,
            "evidence candidate must use an exact reviewed comparison");
    require(code == item.code,
            "evidence candidate must emit its stable opaque token");
    require(code.find(item.program) == std::string_view::npos,
            "evidence token must not contain the raw program identity");
  }

  const char *unmatched[] = {"", "FIREFOX-ESR", "wrapper:firefox-esr",
                             "firefox-esr.desktop"};
  for (const char *program : unmatched) {
    require(matchApplicationEvidenceCandidate(program) ==
                ApplicationEvidenceCandidate::Unmatched,
            "unreviewed evidence identity variants must remain unmatched");
  }
  require(applicationEvidenceCandidateCode(
              ApplicationEvidenceCandidate::Unmatched) == "unmatched",
          "unmatched evidence must emit only a stable generic token");
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
  testApplicationEvidenceCandidatesAreExactAndOpaque();
  testReviewedRulesAreExactAndCoarse();
  testCapabilityAndPrivacyPriority();
  return 0;
}
