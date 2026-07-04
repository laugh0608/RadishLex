import 'package:flutter/material.dart';

import '../models/manager_models.dart';
import 'manager_widgets.dart';

class LearningView extends StatelessWidget {
  const LearningView({super.key, required this.snapshot});

  final ManagerSnapshot snapshot;

  @override
  Widget build(BuildContext context) {
    final summary = snapshot.learningSummary;

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
          title: 'rank explain',
          trailing: Text('updated ${summary.lastUpdated}'),
          child: Column(
            children: snapshot.explanations
                .map((explanation) => _RankerExplanationRow(explanation))
                .toList(),
          ),
        ),
      ],
    );
  }
}

class _RankerExplanationRow extends StatelessWidget {
  const _RankerExplanationRow(this.explanation);

  final RankerExplanation explanation;

  @override
  Widget build(BuildContext context) {
    return Padding(
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
    );
  }
}
