import 'package:compter/src/compter.dart';
import 'package:compter/src/year_picker.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The demo's figures are in FY 2026-27, whatever today is
class _Year2026 extends YearNotifier {
  @override
  int build() => 2026;
}

/// Every screen on a small phone with large text: nothing overflows (an overflow fails the
/// test), and each screen's one primary action sits in the bottom third, within thumb's reach,
/// above the glass tab bar that the list scrolls under.
void main() {
  setUpAll(initRust);

  Future<void> onSmallPhone(WidgetTester tester) async {
    // 360 x 780 points, a small Android phone; text 30% larger than standard
    tester.view.physicalSize = const Size(1080, 2340);
    tester.view.devicePixelRatio = 3;
    tester.platformDispatcher.textScaleFactorTestValue = 1.3;
    addTearDown(tester.view.reset);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    await launchDemo(
      tester,
      overrides: [
        expenseYearProvider.overrideWith(_Year2026.new),
        reportYearProvider.overrideWith(_Year2026.new),
      ],
    );
  }

  /// The screen's primary action is the extended FAB labelled [label], in the bottom third
  void expectPrimaryInReach(WidgetTester tester, String label) {
    final fab = find.widgetWithText(FloatingActionButton, label);
    expect(fab, findsOneWidget);
    final height =
        tester.view.physicalSize.height / tester.view.devicePixelRatio;
    expect(tester.getCenter(fab).dy, greaterThan(height * 2 / 3));
    final tabs = find.byType(CupertinoTabBar);
    if (tabs.evaluate().isNotEmpty) {
      expect(
        tester.getBottomLeft(fab).dy,
        lessThanOrEqualTo(tester.getTopLeft(tabs).dy),
      );
    }
  }

  Future<void> openTab(WidgetTester tester, IconData icon) async {
    await tester.tap(find.byIcon(icon));
    await settle(tester);
  }

  testWidgets('projects and a project', (tester) async {
    await onSmallPhone(tester);
    expectPrimaryInReach(tester, 'New project');
    // The list scrolls under the glass bars but starts below the header and ends above the tabs
    final list = find.byType(ListView).last;
    final header = tester.getBottomLeft(find.byType(AppBar).last).dy;
    expect(
      tester.getTopLeft(find.byIcon(Icons.work).first).dy,
      greaterThan(header),
    );
    expect(tester.getTopLeft(list).dy, lessThan(header));

    await tester.tap(find.text('Website Revamp'));
    await settle(tester);
    expectPrimaryInReach(tester, 'Add milestone');
    await tester.drag(find.byType(ListView).last, const Offset(0, -600));
    await settle(tester);
  });

  testWidgets('invoices and an invoice', (tester) async {
    await onSmallPhone(tester);
    await openTab(tester, Icons.receipt_long_outlined);
    await tester.drag(find.byType(ListView).last, const Offset(0, -600));
    await settle(tester);

    await tester.tap(find.textContaining('Frontend'));
    await settle(tester);
    expectPrimaryInReach(tester, 'Record payment');
    expect(find.byTooltip('Share PDF'), findsOneWidget);
  });

  testWidgets('expenses, reports and settings', (tester) async {
    await onSmallPhone(tester);
    await openTab(tester, Icons.account_balance_wallet_outlined);
    expectPrimaryInReach(tester, 'Add expense');

    await openTab(tester, Icons.insights_outlined);
    expectPrimaryInReach(tester, 'Export return');
    await tester.drag(find.byType(ListView).last, const Offset(0, -1500));
    await settle(tester);

    await openTab(tester, Icons.settings_outlined);
    await tester.drag(find.byType(ListView).last, const Offset(0, -1500));
    await settle(tester);
  });

  testWidgets('forms open as sheets with their action at the foot', (
    tester,
  ) async {
    await onSmallPhone(tester);
    await tester.tap(find.text('New project'));
    await settle(tester);

    final save = find.widgetWithText(FilledButton, 'Save');
    final height =
        tester.view.physicalSize.height / tester.view.devicePixelRatio;
    expect(tester.getCenter(save).dy, greaterThan(height * 2 / 3));
    expect(find.widgetWithText(TextButton, 'Cancel'), findsOneWidget);
  });
}
