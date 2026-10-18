import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../forms/expense_form.dart';
import '../notice.dart';
import '../rust/api/error.dart';
import '../rust/api/expenses.dart';
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
            ListTile(
              title: Text(
                list.expenses.isEmpty
                    ? 'No expenses this year'
                    : '${list.cost} after GST credit',
                style: Theme.of(context).textTheme.titleMedium,
              ),
            ),
            for (final expense in list.expenses) ...[
              const Divider(height: 1),
              ListTile(
                title: Text(expense.description),
                subtitle: Text(
                  [
                    expense.date,
                    expense.category,
                    if (expense.gstCredit != null)
                      'GST credit ${expense.gstCredit}',
                  ].join(' · '),
                ),
                trailing: Text(
                  expense.amount,
                  style: Theme.of(context).textTheme.titleSmall,
                ),
                onTap: () => _actions(context, ref, expense),
              ),
            ],
          ],
        ),
      ),
      floatingActionButton: FloatingActionButton(
        tooltip: 'Add expense',
        onPressed: () async {
          final added = await expenseForm(context, ref.read(compterProvider));
          if (added == null || !context.mounted) return;
          _changed(ref);
          notify(context, 'Added ${added.description} for ${added.amount}');
        },
        child: const Icon(Icons.add),
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
    final edit = await showModalBottomSheet<bool>(
      context: context,
      showDragHandle: true,
      builder: (context) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              title: Text(expense.description),
              subtitle: Text(expense.amount),
            ),
            ListTile(
              leading: const Icon(Icons.edit_outlined),
              title: const Text('Edit'),
              onTap: () => Navigator.pop(context, true),
            ),
            ListTile(
              leading: const Icon(Icons.delete_outline),
              title: const Text('Delete'),
              onTap: () => Navigator.pop(context, false),
            ),
          ],
        ),
      ),
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
