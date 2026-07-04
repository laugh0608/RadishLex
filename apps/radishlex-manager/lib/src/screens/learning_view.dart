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
          title: 'import batches',
          trailing: Text('${snapshot.importBatches.length} batches'),
          child: Column(
            children: snapshot.importBatches.isEmpty
                ? const [Text('无导入批次')]
                : snapshot.importBatches
                      .map((batch) => _ImportBatchRow(batch))
                      .toList(),
          ),
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

class _ImportBatchRow extends StatelessWidget {
  const _ImportBatchRow(this.batch);

  final DictionaryImportBatchSummary batch;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 96,
            child: Text(
              '#${batch.id}',
              style: Theme.of(context).textTheme.labelLarge,
            ),
          ),
          Expanded(
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Text(batch.sourceName),
                Chip(label: Text('imported ${batch.importedTerms}')),
                Chip(label: Text('inserted ${batch.insertedTerms}')),
                Chip(label: Text('updated ${batch.updatedTerms}')),
                if (batch.skippedDeletedTerms > 0)
                  Chip(label: Text('deleted ${batch.skippedDeletedTerms}')),
                if (batch.skippedDuplicateTerms > 0)
                  Chip(label: Text('duplicate ${batch.skippedDuplicateTerms}')),
              ],
            ),
          ),
          ManagerStatusBadge(
            icon: Icons.event_available_outlined,
            label: batch.createdAt,
            tone: ManagerBadgeTone.neutral,
          ),
        ],
      ),
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
