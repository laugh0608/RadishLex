import 'package:flutter/material.dart';

class ManagerSection extends StatelessWidget {
  const ManagerSection({
    super.key,
    required this.title,
    required this.child,
    this.trailing,
  });

  final String title;
  final Widget child;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Text(
                    title,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                ?trailing,
              ],
            ),
            const SizedBox(height: 14),
            child,
          ],
        ),
      ),
    );
  }
}

class ManagerMetricTile extends StatelessWidget {
  const ManagerMetricTile({
    super.key,
    required this.icon,
    required this.label,
    required this.value,
  });

  final IconData icon;
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 190,
      child: Card(
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Row(
            children: [
              Icon(icon),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(label, style: Theme.of(context).textTheme.labelMedium),
                    const SizedBox(height: 2),
                    Text(value, style: Theme.of(context).textTheme.titleLarge),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class ManagerKeyValueRow extends StatelessWidget {
  const ManagerKeyValueRow({
    super.key,
    required this.label,
    required this.value,
  });

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 7),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 148,
            child: Text(label, style: Theme.of(context).textTheme.labelLarge),
          ),
          Expanded(child: Text(value)),
        ],
      ),
    );
  }
}

enum ManagerBadgeTone { success, warning, neutral }

class ManagerStatusBadge extends StatelessWidget {
  const ManagerStatusBadge({
    super.key,
    required this.icon,
    required this.label,
    required this.tone,
  });

  final IconData icon;
  final String label;
  final ManagerBadgeTone tone;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final (background, foreground) = switch (tone) {
      ManagerBadgeTone.success => (
        const Color(0xFFE7F3EC),
        const Color(0xFF1F6B45),
      ),
      ManagerBadgeTone.warning => (
        const Color(0xFFFFF1D8),
        const Color(0xFF8A4F00),
      ),
      ManagerBadgeTone.neutral => (
        const Color(0xFFEFF2F5),
        const Color(0xFF45515E),
      ),
    };

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: background,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: colors.outlineVariant),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 16, color: foreground),
          const SizedBox(width: 6),
          Text(label, style: TextStyle(color: foreground)),
        ],
      ),
    );
  }
}
