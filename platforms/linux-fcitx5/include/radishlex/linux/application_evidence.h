#ifndef RADISHLEX_LINUX_APPLICATION_EVIDENCE_H
#define RADISHLEX_LINUX_APPLICATION_EVIDENCE_H

#include <string_view>

namespace radishlex::linux_platform {

enum class ApplicationEvidenceCandidate {
  Unmatched,
  FirefoxEsrExecutable,
  FirefoxEsrPath,
  FirefoxExecutable,
  MozillaFirefoxIdentity,
};

ApplicationEvidenceCandidate matchApplicationEvidenceCandidate(
    std::string_view program);

std::string_view applicationEvidenceCandidateCode(
    ApplicationEvidenceCandidate candidate);

}  // namespace radishlex::linux_platform

#endif
