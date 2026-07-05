import 'package:flutter/material.dart';

import '../../models/manager_models.dart';
import 'learning_empty_state.dart';

class LearningSummaryAudit extends StatelessWidget {
  const LearningSummaryAudit({super.key, required this.summary});

  final LearningSummary summary;

  @override
  Widget build(BuildContext context) {
    final hasLearningData =
        summary.userTerms > 0 ||
        summary.selectionEvents > 0 ||
        summary.suppressedTerms > 0 ||
        summary.deletedTerms > 0;
    if (!hasLearningData) {
      return const LearningEmptyState(
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
