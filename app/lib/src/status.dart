import 'package:flutter/material.dart';

/// A status label from Rust (`PENDING`, `INVOICED`, `PAID`, ...) as a chip coloured by the
/// theme. The labels themselves are never written in Dart; only their tone is chosen here.
class StatusChip extends StatelessWidget {
  const StatusChip(this.label, {super.key});

  final String label;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final (background, foreground) = switch (label) {
      'PAID' => (scheme.primaryContainer, scheme.onPrimaryContainer),
      'INVOICED' ||
      'COMPLETED' => (scheme.tertiaryContainer, scheme.onTertiaryContainer),
      'CREDITED' => (scheme.surfaceContainerHighest, scheme.onSurfaceVariant),
      _ => (scheme.secondaryContainer, scheme.onSecondaryContainer),
    };
    return Chip(
      label: Text(label),
      labelStyle: Theme.of(
        context,
      ).textTheme.labelSmall?.copyWith(color: foreground),
      backgroundColor: background,
      side: BorderSide.none,
      visualDensity: VisualDensity.compact,
    );
  }
}
