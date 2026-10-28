import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'rust/api/expenses.dart';

/// The financial year a tab shows, by its starting year; starts on the current one. Each tab
/// that shows a year has its own, like `YearPicker` in the terminal app.
class YearNotifier extends Notifier<int> {
  @override
  int build() => currentYear();

  void previous() => state--;
  void next() => state++;
}

/// `‹ FY 2026-27 ›`: steps [year] back and forward, like `[` and `]` in the terminal
class FiscalYearPicker extends ConsumerWidget {
  const FiscalYearPicker(this.year, {super.key, required this.label});

  final NotifierProvider<YearNotifier, int> year;

  /// The year as Rust names it, e.g. `FY 2026-27`
  final String label;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        IconButton(
          tooltip: 'Previous year',
          icon: const Icon(Icons.chevron_left),
          onPressed: () => ref.read(year.notifier).previous(),
        ),
        // Wraps at very large text rather than pushing the arrows off the screen
        Flexible(
          child: Text(
            label,
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.titleMedium,
          ),
        ),
        IconButton(
          tooltip: 'Next year',
          icon: const Icon(Icons.chevron_right),
          onPressed: () => ref.read(year.notifier).next(),
        ),
      ],
    );
  }
}
