import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../forms/expense_form.dart';
import '../notice.dart';
import '../rust/api/error.dart';
import '../rust/api/expenses.dart';
import '../subscription/paywall.dart';
import '../ui/action_sheet.dart';
import '../ui/collapsing_fab.dart';
import '../ui/empty_state.dart';
import '../ui/figures.dart';
import '../ui/glass.dart';
import '../ui/large_title.dart';
import '../ui/record_tile.dart';
import '../ui/search.dart';
import '../ui/section.dart';
import '../year_picker.dart';

/// A financial year's business expenses, like the terminal's Expenses tab. Tapping one offers
/// to edit or delete it.
class ExpensesScreen extends ConsumerStatefulWidget {
  const ExpensesScreen({super.key});

  @override
  ConsumerState<ExpensesScreen> createState() => _ExpensesScreenState();
}

class _ExpensesScreenState extends ConsumerState<ExpensesScreen>
    with Searching {
  @override
  Widget build(BuildContext context) {
    final year = ref.watch(expenseYearProvider);
    final expenses = ref.watch(expensesProvider(year));
    return LargeTitleScope(
      child: FabScrollScope(
        child: Scaffold(
          extendBodyBehindAppBar: true,
          appBar: searchAppBar(
            title: 'Expenses',
            hint: 'Description or category',
          ),
          body: expenses.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (error, _) => Center(child: Text('$error')),
            data: (list) {
              final matches = query.apply(
                list.expenses,
                (expense) => [expense.description, expense.category],
              );
              return ScreenList(
                bottom: 96,
                children: [
                  const LargeTitle(
                    icon: Icons.account_balance_wallet,
                    title: 'Expenses',
                    description:
                        'What the business spent, and the GST it can claim back',
                  ),
                  FiscalYearPicker(expenseYearProvider, label: list.year),
                  if (list.expenses.isEmpty)
                    const EmptyState(
                      icon: Icons.account_balance_wallet_outlined,
                      message: 'No expenses this year',
                    )
                  else ...[
                    FigureGrid([('After GST credit', list.cost)]),
                    if (matches.isEmpty)
                      noMatches(query)
                    else
                      const SectionHeader('By date'),
                  ],
                  for (final expense in matches)
                    RecordTile(
                      title: expense.description,
                      supporting: [expense.date, expense.category].join(' · '),
                      amount: expense.amount,
                      detail: expense.gstCredit == null
                          ? null
                          : 'GST ${expense.gstCredit}',
                      onTap: () => _actions(expense),
                    ),
                ],
              );
            },
          ),
          floatingActionButton: CollapsingFab(
            // Every tab stays mounted in one route, so their buttons would share the default hero tag
            heroTag: null,
            icon: Icons.add,
            label: 'Add expense',
            onPressed: () async {
              final added = await expenseForm(
                context,
                ref.read(compterProvider),
              );
              if (added == null || !context.mounted) return;
              _changed();
              notify(context, 'Added ${added.description} for ${added.amount}');
            },
          ),
        ),
      ),
    );
  }

  /// Every year's list may have changed: an edit can move an expense to another year
  void _changed() => ref.invalidate(expensesProvider);

  Future<void> _actions(ExpenseRow expense) async {
    final edit = await showActionSheet<bool>(
      context,
      title: expense.description,
      subtitle: expense.amount,
      actions: const [
        (icon: Icons.edit_outlined, label: 'Edit', value: true),
        (icon: Icons.delete_outline, label: 'Delete', value: false),
      ],
    );
    if (edit == null || !mounted) return;
    final app = ref.read(compterProvider);
    if (edit) {
      final saved = await expenseForm(context, app, existing: expense);
      if (saved == null || !mounted) return;
      _changed();
      notify(context, 'Saved ${saved.description} for ${saved.amount}');
      return;
    }
    if (!await requireWrite(context) || !mounted) return;
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
      if (!mounted) return;
      _changed();
      notify(context, 'Deleted ${expense.description}');
    } on ApiError catch (error) {
      if (mounted) notify(context, error.message, error: true);
    }
  }
}
