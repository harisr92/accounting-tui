import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../forms/expense_form.dart';
import '../notice.dart';
import '../rust/api/error.dart';
import '../rust/api/expenses.dart';
import '../ui/action_sheet.dart';
import '../ui/empty_state.dart';
import '../ui/figures.dart';
import '../ui/record_tile.dart';
import '../ui/section.dart';
import '../year_picker.dart';

/// A financial year's business expenses, like the terminal's Expenses tab. Tapping one offers
/// to edit or delete it.
class ExpensesScreen extends ConsumerWidget {
  const ExpensesScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final year = ref.watch(expenseYearProvider);
    final expenses = ref.watch(expensesProvider(year));
    return Scaffold(
      appBar: AppBar(title: const Text('Expenses')),
      body: expenses.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, _) => Center(child: Text('$error')),
        data: (list) => ListView(
          padding: const EdgeInsets.only(bottom: 96),
          children: [
            FiscalYearPicker(expenseYearProvider, label: list.year),
            if (list.expenses.isEmpty)
              const EmptyState(
                icon: Icons.account_balance_wallet_outlined,
                message: 'No expenses this year',
              )
            else ...[
              FigureGrid([('After GST credit', list.cost)]),
              const SectionHeader('By date'),
            ],
            for (final expense in list.expenses)
              RecordTile(
                title: expense.description,
                supporting: [expense.date, expense.category].join(' · '),
                amount: expense.amount,
                detail: expense.gstCredit == null
                    ? null
                    : 'GST ${expense.gstCredit}',
                onTap: () => _actions(context, ref, expense),
              ),
          ],
        ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        // Every tab stays mounted in one route, so their buttons would share the default hero tag
        heroTag: null,
        tooltip: 'Add expense',
        icon: const Icon(Icons.add),
        label: const Text('Add expense'),
        onPressed: () async {
          final added = await expenseForm(context, ref.read(compterProvider));
          if (added == null || !context.mounted) return;
          _changed(ref);
          notify(context, 'Added ${added.description} for ${added.amount}');
        },
      ),
    );
  }

  /// Every year's list may have changed: an edit can move an expense to another year
  void _changed(WidgetRef ref) => ref.invalidate(expensesProvider);

  Future<void> _actions(
    BuildContext context,
    WidgetRef ref,
    ExpenseRow expense,
  ) async {
    final edit = await showActionSheet<bool>(
      context,
      title: expense.description,
      subtitle: expense.amount,
      actions: const [
        (icon: Icons.edit_outlined, label: 'Edit', value: true),
        (icon: Icons.delete_outline, label: 'Delete', value: false),
      ],
    );
    if (edit == null || !context.mounted) return;
    final app = ref.read(compterProvider);
    if (edit) {
      final saved = await expenseForm(context, app, existing: expense);
      if (saved == null || !context.mounted) return;
      _changed(ref);
      notify(context, 'Saved ${saved.description} for ${saved.amount}');
      return;
    }
    final yes = await confirm(
      context,
      title: 'Delete ${expense.description}?',
      lines: [
        '${expense.amount} on ${expense.date}. Its ledger posting goes too.',
      ],
      yes: 'Delete',
    );
    if (!yes) return;
    try {
      await app.deleteExpense(id: expense.id);
      if (!context.mounted) return;
      _changed(ref);
      notify(context, 'Deleted ${expense.description}');
    } on ApiError catch (error) {
      if (context.mounted) notify(context, error.message, error: true);
    }
  }
}
