import 'package:flutter/material.dart';

import '../../models/manager_models.dart';
import '../manager_widgets.dart';
import 'learning_empty_state.dart';

class RankerExplanationKey {
  const RankerExplanationKey({
    required this.inputCode,
    required this.candidate,
    required this.contextKind,
  });

  factory RankerExplanationKey.from(RankerExplanation explanation) {
    return RankerExplanationKey(
      inputCode: explanation.inputCode,
      candidate: explanation.candidate,
      contextKind: explanation.contextKind,
    );
  }

  final String inputCode;
  final String candidate;
  final String contextKind;

  @override
  bool operator ==(Object other) {
    return other is RankerExplanationKey &&
        other.inputCode == inputCode &&
        other.candidate == candidate &&
        other.contextKind == contextKind;
  }

  @override
  int get hashCode => Object.hash(inputCode, candidate, contextKind);
}

List<RankerExplanation> filterRankerExplanations({
  required List<RankerExplanation> explanations,
  required String query,
}) {
  final normalizedQuery = query.trim().toLowerCase();
  if (normalizedQuery.isEmpty) {
    return explanations;
  }
  return explanations
      .where((explanation) {
        return explanation.inputCode.toLowerCase().contains(normalizedQuery) ||
            explanation.candidate.toLowerCase().contains(normalizedQuery) ||
            explanation.contextKind.toLowerCase().contains(normalizedQuery) ||
            explanation.signals.any(
              (signal) => signal.toLowerCase().contains(normalizedQuery),
            );
      })
      .toList(growable: false);
}

RankerExplanation? selectedRankerExplanation({
  required List<RankerExplanation> explanations,
  required RankerExplanationKey? key,
}) {
  if (key == null) {
    return null;
  }
  for (final explanation in explanations) {
    if (RankerExplanationKey.from(explanation) == key) {
      return explanation;
    }
  }
  return null;
}

class RankExplainSection extends StatelessWidget {
  const RankExplainSection({
    super.key,
    required this.controller,
    required this.allExplanations,
    required this.visibleExplanations,
    required this.selectedKey,
    required this.onFilterChanged,
    required this.onSelect,
  });

  final TextEditingController controller;
  final List<RankerExplanation> allExplanations;
  final List<RankerExplanation> visibleExplanations;
  final RankerExplanationKey? selectedKey;
  final ValueChanged<String> onFilterChanged;
  final ValueChanged<RankerExplanationKey> onSelect;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextField(
          key: const Key('rank-explain-filter'),
          controller: controller,
          decoration: const InputDecoration(
            prefixIcon: Icon(Icons.search),
            labelText: '筛选 input code / candidate / context / signal',
          ),
          onChanged: onFilterChanged,
        ),
        const SizedBox(height: 14),
        if (allExplanations.isEmpty)
          const LearningEmptyState(
            icon: Icons.manage_search_outlined,
            message: '暂无 rank explain 摘要',
          )
        else if (visibleExplanations.isEmpty)
          const LearningEmptyState(
            icon: Icons.search_off_outlined,
            message: '没有匹配当前筛选条件的 explain 摘要',
          )
        else
          Column(
            children: visibleExplanations
                .map(
                  (explanation) => _RankerExplanationRow(
                    explanation,
                    selected:
                        RankerExplanationKey.from(explanation) == selectedKey,
                    onSelect: () =>
                        onSelect(RankerExplanationKey.from(explanation)),
                  ),
                )
                .toList(),
          ),
      ],
    );
  }
}

class RankerExplanationDetail extends StatelessWidget {
  const RankerExplanationDetail({super.key, required this.explanation});

  final RankerExplanation? explanation;

  @override
  Widget build(BuildContext context) {
    final selectedExplanation = explanation;
    if (selectedExplanation == null) {
      return const LearningEmptyState(
        icon: Icons.info_outline,
        message: '选择一个候选查看 rank explain 贡献项',
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            ManagerStatusBadge(
              icon: Icons.trending_up,
              label: selectedExplanation.score.toStringAsFixed(2),
              tone: ManagerBadgeTone.success,
            ),
            const ManagerStatusBadge(
              icon: Icons.privacy_tip_outlined,
              label: 'summary only',
              tone: ManagerBadgeTone.neutral,
            ),
          ],
        ),
        const SizedBox(height: 10),
        ManagerKeyValueRow(
          label: 'input code',
          value: selectedExplanation.inputCode,
        ),
        ManagerKeyValueRow(
          label: 'candidate',
          value: selectedExplanation.candidate,
        ),
        ManagerKeyValueRow(
          label: 'context',
          value: selectedExplanation.contextKind,
        ),
        ManagerKeyValueRow(
          label: 'score',
          value: selectedExplanation.score.toStringAsFixed(2),
        ),
        ManagerKeyValueRow(
          label: 'signals',
          value: selectedExplanation.signals.join(', '),
        ),
        const ManagerKeyValueRow(label: 'privacy', value: '不展示 P1 原始事件明细'),
      ],
    );
  }
}

class _RankerExplanationRow extends StatelessWidget {
  const _RankerExplanationRow(
    this.explanation, {
    required this.selected,
    required this.onSelect,
  });

  final RankerExplanation explanation;
  final bool selected;
  final VoidCallback onSelect;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Material(
      color: selected
          ? colors.secondaryContainer.withValues(alpha: 0.38)
          : null,
      child: InkWell(
        onTap: onSelect,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 10),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                width: 104,
                child: Text(
                  explanation.inputCode,
                  style: Theme.of(context).textTheme.labelLarge,
                ),
              ),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(explanation.candidate),
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: [
                        Chip(label: Text('context=${explanation.contextKind}')),
                        ...explanation.signals.map(
                          (signal) => Chip(label: Text(signal)),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
              ManagerStatusBadge(
                icon: Icons.trending_up,
                label: explanation.score.toStringAsFixed(2),
                tone: ManagerBadgeTone.success,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
