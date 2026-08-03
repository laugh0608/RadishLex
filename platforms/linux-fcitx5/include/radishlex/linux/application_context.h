#ifndef RADISHLEX_LINUX_APPLICATION_CONTEXT_H
#define RADISHLEX_LINUX_APPLICATION_CONTEXT_H

#include <cstddef>
#include <string_view>

#include "radishlex/linux/ffi_projection.h"

namespace radishlex::linux_platform {

enum class ReviewedApplicationKind {
  General,
  Browser,
  Chat,
  Code,
  Editor,
  Office,
};

struct ApplicationContextInput {
  bool secure_input = false;
  bool sensitive_application = false;
  bool terminal = false;
  bool privacy_mode = false;
  std::string_view program;
};

LearningContextProjection projectApplicationContext(
    const ApplicationContextInput &input);

#if defined(RADISHLEX_APPLICATION_CONTEXT_TESTING)
struct ReviewedApplicationRule {
  std::string_view program;
  ReviewedApplicationKind kind;
};

LearningContextProjection projectApplicationContextForTesting(
    const ApplicationContextInput &input,
    const ReviewedApplicationRule *rules,
    std::size_t rule_count);
#endif

}  // namespace radishlex::linux_platform

#endif
