import 'package:flutter/material.dart';

import '../models/manager_models.dart';
import 'learning/learning_summary_section.dart';
import 'learning/rank_explain_section.dart';
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
  RankerExplanationKey? selectedExplanationKey;

  @override
  void dispose() {
    explainFilterController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final summary = widget.snapshot.learningSummary;
    final visibleExplanations = filterRankerExplanations(
      explanations: widget.snapshot.explanations,
      query: explainFilterController.text,
    );
    final selectedExplanation = selectedRankerExplanation(
      explanations: visibleExplanations,
      key: selectedExplanationKey,
    );

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
          child: LearningSummaryAudit(summary: summary),
        ),
        const SizedBox(height: 16),
        ManagerSection(
          title: 'rank explain',
          trailing: Text('${widget.snapshot.explanations.length} candidates'),
          child: RankExplainSection(
            controller: explainFilterController,
            allExplanations: widget.snapshot.explanations,
            visibleExplanations: visibleExplanations,
            selectedKey: selectedExplanationKey,
            onFilterChanged: (_) => setState(() {}),
            onSelect: (key) => setState(() {
              selectedExplanationKey = key;
            }),
          ),
        ),
        const SizedBox(height: 16),
        ManagerSection(
          title: '候选解释详情',
          child: RankerExplanationDetail(explanation: selectedExplanation),
        ),
      ],
    );
  }
}
