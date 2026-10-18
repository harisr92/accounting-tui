import 'package:flutter/material.dart';

import 'theme.dart';

/// Figures with their labels in a 2-column grid of equal tiles, like `widgets::stat_tiles` in
/// the terminal app. An odd last figure spans the row.
class FigureGrid extends StatelessWidget {
  const FigureGrid(this.figures, {super.key});

  /// (label, value), both from Rust
  final List<(String, String)> figures;

  @override
  Widget build(BuildContext context) {
    const gap = 8.0;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: gutter),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final half = (constraints.maxWidth - gap) / 2;
          return Wrap(
            spacing: gap,
            runSpacing: gap,
            children: [
              for (final (i, (label, value)) in figures.indexed)
                SizedBox(
                  width: i == figures.length - 1 && figures.length.isOdd
                      ? constraints.maxWidth
                      : half,
                  child: Figure(label, value),
                ),
            ],
          );
        },
      ),
    );
  }
}

/// One figure: a label over its value, on a filled card
class Figure extends StatelessWidget {
  const Figure(this.label, this.value, {super.key});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card.filled(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: gutter, vertical: 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              label,
              style: theme.textTheme.labelMedium?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
            const SizedBox(height: 4),
            FittedBox(
              fit: BoxFit.scaleDown,
              alignment: Alignment.centerLeft,
              child: Text(
                value,
                style: theme.textTheme.titleLarge?.copyWith(
                  fontFeatures: tabular,
                ),
                maxLines: 1,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
