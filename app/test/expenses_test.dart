import 'package:compter/src/compter.dart';
import 'package:compter/src/year_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The demo's expenses are in FY 2026-27, whatever today is
class _Year2026 extends YearNotifier {
  @override
  int build() => 2026;
}

/// The Expenses tab on the demo business against the real Rust core: run
/// `cargo build -p compter-mobile` first, then `flutter test`.
void main() {
  setUpAll(initRust);

  Future<void> openExpenses(WidgetTester tester) async {
    await launchDemo(
      tester,
      overrides: [expenseYearProvider.overrideWith(_Year2026.new)],
    );
    await tester.tap(find.byIcon(Icons.account_balance_wallet_outlined));
    await settle(tester);
  }

  Future<void> fill(WidgetTester tester, String label, String text) =>
      tester.enterText(find.widgetWithText(TextField, label), text);

  /// Tap a row once the snack bar that may cover it is gone
  Future<void> tapRow(WidgetTester tester, String text) async {
    tester
        .state<ScaffoldMessengerState>(find.byType(ScaffoldMessenger))
        .removeCurrentSnackBar();
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text(text));
    await tester.tap(find.text(text));
    await settle(tester);
  }

  testWidgets('a year lists its expenses and steps to the one before', (
    tester,
  ) async {
    await openExpenses(tester);
    expect(find.text('FY 2026-27'), findsOneWidget);
    expect(find.text('Laptop'), findsOneWidget);
    expect(find.text('₹1,77,000.00'), findsOneWidget);

    await tester.tap(find.byTooltip('Previous year'));
    await settle(tester);

    expect(find.text('FY 2025-26'), findsOneWidget);
    expect(find.text('No expenses this year'), findsOneWidget);
  });

  testWidgets('an expense is added, edited and deleted', (tester) async {
    await openExpenses(tester);

    await tester.tap(find.byTooltip('Add expense'));
    await settle(tester);
    await fill(tester, 'Date', '2026-10-02');
    await fill(tester, 'Description', 'GitHub');
    await fill(tester, 'Amount', '21,240');
    await fill(tester, 'GST credit', '3,240');
    await tester.tap(find.text('Save'));
    await pumpUntil(tester, find.text('Added GitHub for ₹21,240.00'));
    expect(find.text('₹21,240.00'), findsOneWidget);

    await tapRow(tester, 'GitHub');
    await tester.tap(find.text('Edit'));
    await settle(tester);
    await fill(tester, 'Amount', '10,000');
    await tester.tap(find.text('Save'));
    await pumpUntil(tester, find.text('₹10,000.00'));

    await tapRow(tester, 'GitHub');
    await tester.tap(find.text('Delete'));
    await settle(tester);
    await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
    await pumpUntil(tester, find.text('Deleted GitHub'));
    expect(find.text('₹10,000.00'), findsNothing);
  });

  testWidgets('a refused amount stays in the form under its field', (
    tester,
  ) async {
    await openExpenses(tester);
    await tester.tap(find.byTooltip('Add expense'));
    await settle(tester);
    await fill(tester, 'Description', 'GitHub');
    await fill(tester, 'Amount', 'lots');
    await tester.tap(find.text('Save'));
    await settle(tester);

    expect(find.text('Add expense'), findsOneWidget);
    expect(find.textContaining('amount must be a number'), findsOneWidget);
  });
}
