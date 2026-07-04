import 'package:flutter/material.dart';

import '../models/manager_models.dart';
import 'manager_widgets.dart';

class LearningView extends StatelessWidget {
  const LearningView({super.key, required this.snapshot});

  final ManagerSnapshot snapshot;

  @override
  Widget build(BuildContext context) {
    return _LearningViewBody(snapshot: snapshot);
  }
}

class _LearningViewBody extends StatefulWidget {
  const _LearningViewBody({required this.snapshot});

  final ManagerSnapshot snapshot;

  @override
  State<_LearningViewBody> createState() => _LearningViewBodyState();
}

class _LearningViewBodyState extends State<_LearningViewBody> {
  final explainFilterController = TextEditingController();
  _RankerExplanationKey? selectedExplanationKey;

  @override
  void dispose() {
    explainFilterController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final summary = widget.snapshot.learningSummary;
    final visibleExplanations = _filterExplanations(
      widget.snapshot.explanations,
    );
    final selectedExplanation = _selectedExplanation(visibleExplanations);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            ManagerMetricTile(
              icon: Icons.library_add_check_outlined,
              label: 'user terms',
              value: summary.userTerms.toString(),
            ),
            ManagerMetricTile(
              icon: Icons.history_outlined,
              label: 'selection events',
              value: summary.selectionEvents.toString(),
            ),
            ManagerMetricTile(
              icon: Icons.block_outlined,
              label: 'suppressed',
              value: summary.suppressedTerms.toString(),
            ),
            ManagerMetricTile(
              icon: Icons.delete_sweep_outlined,
              label: 'deleted',
              value: summary.deletedTerms.toString(),
            ),
          ],
        ),
        const SizedBox(height: 16),
        ManagerSection(
          title: '学习摘要',
          trailing: Text('updated ${summary.lastUpdated}'),
          child: _LearningSummaryAudit(summary: summary),
        ),
        const SizedBox(height: 16),
        ManagerSection(
          title: 'rank explain',
          trailing: Text('${widget.snapshot.explanations.length} candidates'),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              TextField(
                key: const Key('rank-explain-filter'),
                controller: explainFilterController,
                decoration: const InputDecoration(
                  prefixIcon: Icon(Icons.search),
                  labelText: '筛选 input code / candidate / signal',
                ),
                onChanged: (_) => setState(() {}),
              ),
              const SizedBox(height: 14),
              if (widget.snapshot.explanations.isEmpty)
                const _LearningEmptyState(
                  icon: Icons.manage_search_outlined,
                  message: '暂无 rank explain 摘要',
                )
              else if (visibleExplanations.isEmpty)
                const _LearningEmptyState(
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
                              _RankerExplanationKey.from(explanation) ==
                              selectedExplanationKey,
                          onSelect: () => setState(() {
                            selectedExplanationKey = _RankerExplanationKey.from(
                              explanation,
                            );
                          }),
                        ),
                      )
                      .toList(),
                ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        ManagerSection(
          title: '候选解释详情',
          child: _RankerExplanationDetail(explanation: selectedExplanation),
        ),
      ],
    );
  }

  List<RankerExplanation> _filterExplanations(
    List<RankerExplanation> explanations,
  ) {
    final query = explainFilterController.text.trim().toLowerCase();
    if (query.isEmpty) {
      return explanations;
    }
    return explanations
        .where((explanation) {
          return explanation.inputCode.toLowerCase().contains(query) ||
              explanation.candidate.toLowerCase().contains(query) ||
              explanation.signals.any(
                (signal) => signal.toLowerCase().contains(query),
              );
        })
        .toList(growable: false);
  }

  RankerExplanation? _selectedExplanation(
    List<RankerExplanation> explanations,
  ) {
    final key = selectedExplanationKey;
    if (key == null) {
      return null;
    }
    for (final explanation in explanations) {
      if (_RankerExplanationKey.from(explanation) == key) {
        return explanation;
      }
    }
    return null;
  }
}

class _LearningSummaryAudit extends StatelessWidget {
  const _LearningSummaryAudit({required this.summary});

  final LearningSummary summary;

  @override
  Widget build(BuildContext context) {
    final hasLearningData =
        summary.userTerms > 0 ||
        summary.selectionEvents > 0 ||
        summary.suppressedTerms > 0 ||
        summary.deletedTerms > 0;
    if (!hasLearningData) {
      return const _LearningEmptyState(
        icon: Icons.psychology_alt_outlined,
        message: '暂无学习摘要',
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text('仅展示聚合学习摘要，不展示 P1 原始选择事件、原始输入历史或应用窗口信息。'),
        const SizedBox(height: 10),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            Chip(label: Text('user terms ${summary.userTerms}')),
            Chip(label: Text('selection events ${summary.selectionEvents}')),
            Chip(label: Text('suppressed ${summary.suppressedTerms}')),
            Chip(label: Text('deleted ${summary.deletedTerms}')),
          ],
        ),
      ],
    );
  }
}

class _RankerExplanationDetail extends StatelessWidget {
  const _RankerExplanationDetail({required this.explanation});

  final RankerExplanation? explanation;

  @override
  Widget build(BuildContext context) {
    final selectedExplanation = explanation;
    if (selectedExplanation == null) {
      return const _LearningEmptyState(
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

class _LearningEmptyState extends StatelessWidget {
  const _LearningEmptyState({required this.icon, required this.message});

  final IconData icon;
  final String message;

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.onSurfaceVariant;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 20),
      child: Row(
        children: [
          Icon(icon, color: color),
          const SizedBox(width: 10),
          Flexible(
            child: Text(
              message,
              style: Theme.of(
                context,
              ).textTheme.bodyMedium?.copyWith(color: color),
            ),
          ),
        ],
      ),
    );
  }
}

class _RankerExplanationKey {
  const _RankerExplanationKey({
    required this.inputCode,
    required this.candidate,
  });

  factory _RankerExplanationKey.from(RankerExplanation explanation) {
    return _RankerExplanationKey(
      inputCode: explanation.inputCode,
      candidate: explanation.candidate,
    );
  }

  final String inputCode;
  final String candidate;

  @override
  bool operator ==(Object other) {
    return other is _RankerExplanationKey &&
        other.inputCode == inputCode &&
        other.candidate == candidate;
  }

  @override
  int get hashCode => Object.hash(inputCode, candidate);
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
                      children: explanation.signals
                          .map((signal) => Chip(label: Text(signal)))
                          .toList(),
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
