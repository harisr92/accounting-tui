import 'package:flutter/material.dart';

import 'theme.dart';

/// A status label from Rust (`PENDING`, `INVOICED`, `PAID`, ...) as a chip coloured by the
/// theme, the same height for every label. The labels themselves are never written in Dart;
/// only their tone is chosen here.
class StatusChip extends StatelessWidget {
  const StatusChip(this.label, {super.key});

  final String label;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final (background, foreground) = switch (label) {
      'PAID' => (scheme.primaryContainer, scheme.onPrimaryContainer),
      'INVOICED' ||
      'ISSUED' ||
      'COMPLETED' => (scheme.tertiaryContainer, scheme.onTertiaryContainer),
      'CREDITED' => creditedColors(theme.brightness),
      _ => (scheme.secondaryContainer, scheme.onSecondaryContainer),
    };
    return Container(
      height: 24,
      padding: const EdgeInsets.symmetric(horizontal: 8),
      decoration: BoxDecoration(
        color: background,
        borderRadius: BorderRadius.circular(8),
      ),
      // As wide as the label, centred in the fixed height
      child: Center(
        widthFactor: 1,
        child: Text(
          label,
          style: theme.textTheme.labelSmall?.copyWith(color: foreground),
        ),
      ),
    );
  }
}
