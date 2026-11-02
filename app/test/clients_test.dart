import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The Clients screens against the real Rust core: opened from Projects, read-only
void main() {
  setUpAll(initRust);

  Future<void> openClients(WidgetTester tester) async {
    await launchDemo(tester);
    await tester.tap(find.byTooltip('Clients'));
    await settle(tester);
  }

  testWidgets(
    'lists each client with their place, projects and what they owe',
    (tester) async {
      await openClients(tester);

      expect(find.text('Acme Pvt Ltd'), findsOneWidget);
      expect(find.text('27AAPFU0939F2ZU · 1 project'), findsOneWidget);
      expect(find.text('₹35,400.00'), findsOneWidget);
      expect(find.text('Karnataka (29) · 1 project'), findsOneWidget);
      expect(find.text('Germany · 1 project · nothing owed'), findsOneWidget);
    },
  );

  testWidgets('a client opens with their projects and invoices', (
    tester,
  ) async {
    await openClients(tester);
    await tester.tap(find.text('Acme Pvt Ltd'));
    await settle(tester);

    expect(find.text('Plot 7, MIDC, Pune 411019'), findsOneWidget);
    expect(find.text('Website Revamp'), findsOneWidget);
    expect(find.text('Projects · 1 project'), findsOneWidget);
    expect(find.text('Invoices · 2'), findsOneWidget);
    expect(find.textContaining('Frontend'), findsOneWidget);

    // An invoice opens over the client, and back returns to it
    await tester.tap(find.textContaining('Frontend'));
    await settle(tester);
    expect(find.text('Payments and credit notes'), findsOneWidget);
    await tester.pageBack();
    await settle(tester);
    expect(find.text('Invoices · 2'), findsOneWidget);
  });

  testWidgets('search narrows the clients', (tester) async {
    await openClients(tester);
    await tester.tap(find.byTooltip('Search'));
    await settle(tester);
    await tester.enterText(find.byType(TextField), 'germany');
    await settle(tester);

    expect(find.text('Initrode GmbH'), findsOneWidget);
    expect(find.text('Acme Pvt Ltd'), findsNothing);
  });
}
