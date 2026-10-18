import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The screens against the real Rust core on this machine: run
/// `cargo build -p compter-mobile` first, then `flutter test`.
void main() {
  setUpAll(initRust);

  testWidgets('lists the demo projects and opens one', (tester) async {
    await launchDemo(tester);
    expect(find.text('Data Pipeline'), findsOneWidget);

    await tester.tap(find.text('Data Pipeline'));
    await settle(tester);

    expect(find.text('Milestones'), findsOneWidget);
    expect(find.text('Outstanding'), findsOneWidget);
    expect(find.textContaining('₹'), findsWidgets);
  });

  testWidgets('a refused rate stays in the form under its field', (
    tester,
  ) async {
    await launchDemo(tester);
    await tester.tap(find.byTooltip('New project'));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Support');
    await tester.enterText(find.widgetWithText(TextField, 'Client'), 'Acme');
    await tester.enterText(
      find.widgetWithText(TextField, 'Rate / hour'),
      'lots',
    );
    await tester.tap(find.text('Save'));
    await settle(tester);

    // Still open: the sheet's title over the button that opened it
    expect(find.text('New project'), findsNWidgets(2));
    expect(find.textContaining('amount must be a number'), findsOneWidget);
  });

  testWidgets('a new fixed project takes a milestone', (tester) async {
    await launchDemo(tester);
    await tester.tap(find.byTooltip('New project'));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Website');
    await tester.enterText(find.widgetWithText(TextField, 'Client'), 'Acme');
    await tester.tap(find.text('Save'));
    await settle(tester);

    expect(find.textContaining('No milestones yet'), findsOneWidget);
    await tester.tap(find.byType(FloatingActionButton));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Design');
    await tester.enterText(
      find.widgetWithText(TextField, 'Due date'),
      '2026-11-01',
    );
    await tester.enterText(find.widgetWithText(TextField, 'Amount'), '50,000');
    await tester.tap(find.text('Save'));
    await settle(tester);

    expect(find.text('Design'), findsOneWidget);
    expect(find.text('Due 2026-11-01'), findsOneWidget);
    expect(find.text('₹50,000.00'), findsWidgets);
  });
}
