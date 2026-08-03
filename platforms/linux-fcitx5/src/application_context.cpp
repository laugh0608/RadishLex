#include "radishlex/linux/application_context.h"

#include <array>
#include <optional>
#include <string_view>

namespace radishlex::linux_platform {
namespace {

struct ApplicationRule {
  std::string_view program;
  ReviewedApplicationKind kind;
};

// Production entries are added only after the corresponding Wayland/X11
// frontend identity and sensitive-field propagation have been reviewed.
constexpr std::array<ApplicationRule, 0> kReviewedApplications{};

std::string_view contextKindName(ReviewedApplicationKind kind) {
  switch (kind) {
    case ReviewedApplicationKind::General:
      return "general";
    case ReviewedApplicationKind::Browser:
      return "browser";
    case ReviewedApplicationKind::Chat:
      return "chat";
    case ReviewedApplicationKind::Code:
      return "code";
    case ReviewedApplicationKind::Editor:
      return "editor";
    case ReviewedApplicationKind::Office:
      return "office";
  }
  return "other";
}

template <typename Rule>
std::optional<ReviewedApplicationKind> classifyReviewedProgram(
    std::string_view program, const Rule *rules, std::size_t rule_count) {
  if (program.empty()) {
    return std::nullopt;
  }
  for (std::size_t index = 0; index < rule_count; ++index) {
    if (!rules[index].program.empty() && rules[index].program == program) {
      return rules[index].kind;
    }
  }
  return std::nullopt;
}

template <typename Rule>
LearningContextProjection projectWithRules(
    const ApplicationContextInput &input,
    const Rule *rules,
    std::size_t rule_count) {
  LearningContextProjection context;
  context.secure_input = input.secure_input;
  context.sensitive_application = input.sensitive_application;
  context.privacy_mode = input.privacy_mode;
  context.context_known = false;
  context.context_kind = "other";

  if (input.secure_input || input.sensitive_application) {
    return context;
  }
  if (input.terminal) {
    context.context_kind = "terminal";
    return context;
  }

  const auto kind =
      classifyReviewedProgram(input.program, rules, rule_count);
  if (!kind.has_value()) {
    return context;
  }
  context.context_known = true;
  context.context_kind = contextKindName(*kind);
  return context;
}

}  // namespace

LearningContextProjection projectApplicationContext(
    const ApplicationContextInput &input) {
  return projectWithRules(input, kReviewedApplications.data(),
                          kReviewedApplications.size());
}

#if defined(RADISHLEX_APPLICATION_CONTEXT_TESTING)
LearningContextProjection projectApplicationContextForTesting(
    const ApplicationContextInput &input,
    const ReviewedApplicationRule *rules,
    std::size_t rule_count) {
  return projectWithRules(input, rules, rule_count);
}
#endif

}  // namespace radishlex::linux_platform
