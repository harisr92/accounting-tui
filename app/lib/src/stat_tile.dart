import 'package:flutter/material.dart';

/// A figure with its label, like `widgets::stat_tiles` in the terminal app
class StatTile extends StatelessWidget {
  const StatTile(this.label, this.value, {super.key});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    return Card.filled(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(label, style: text.labelMedium),
            Text(value, style: text.titleMedium),
          ],
        ),
      ),
    );
  }
}
