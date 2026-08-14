#include "radishlex/linux/application_evidence.h"

#include <array>

namespace radishlex::linux_platform {
namespace {

struct EvidenceCandidateRule {
  std::string_view program;
  ApplicationEvidenceCandidate candidate;
};

constexpr std::array<EvidenceCandidateRule, 4> kEvidenceCandidates{{
    {"firefox-esr", ApplicationEvidenceCandidate::FirefoxEsrExecutable},
    {"/usr/lib/firefox-esr/firefox-esr",
     ApplicationEvidenceCandidate::FirefoxEsrPath},
    {"firefox", ApplicationEvidenceCandidate::FirefoxExecutable},
    {"org.mozilla.firefox",
     ApplicationEvidenceCandidate::MozillaFirefoxIdentity},
}};

}  // namespace

ApplicationEvidenceCandidate matchApplicationEvidenceCandidate(
    std::string_view program) {
  for (const auto &rule : kEvidenceCandidates) {
    if (program == rule.program) {
      return rule.candidate;
    }
  }
  return ApplicationEvidenceCandidate::Unmatched;
}

std::string_view applicationEvidenceCandidateCode(
    ApplicationEvidenceCandidate candidate) {
  switch (candidate) {
    case ApplicationEvidenceCandidate::FirefoxEsrExecutable:
      return "browser_candidate_01";
    case ApplicationEvidenceCandidate::FirefoxEsrPath:
      return "browser_candidate_02";
    case ApplicationEvidenceCandidate::FirefoxExecutable:
      return "browser_candidate_03";
    case ApplicationEvidenceCandidate::MozillaFirefoxIdentity:
      return "browser_candidate_04";
    case ApplicationEvidenceCandidate::Unmatched:
      return "unmatched";
  }
  return "unmatched";
}

}  // namespace radishlex::linux_platform
