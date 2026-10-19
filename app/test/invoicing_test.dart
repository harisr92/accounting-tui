import 'package:compter/src/compter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// Issuing, payments, credit notes and the profile, on the demo business against the real Rust
/// core: run `cargo build -p compter-mobile` first, then `flutter test`.
void main() {
  setUpAll(initRust);

  Future<void> openInvoice(WidgetTester tester, String milestone) async {
    await tester.tap(find.byIcon(Icons.receipt_long_outlined));
    await settle(tester);
    await tester.tap(find.textContaining(milestone));
    await settle(tester);
  }

  testWidgets('issuing confirms the amounts, then opens the invoice', (
    tester,
  ) async {
    await launchDemo(tester);
    await tester.tap(find.text('Website Revamp'));
    await settle(tester);
    // Below the project's large title on the test's short screen
    await tester.ensureVisible(find.text('Launch'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Launch'));
    await settle(tester);
    await tester.tap(find.text('Issue invoice'));
    await settle(tester);

    expect(
      find.text('₹47,200.00 (40,000.00 + CGST 3,600.00 + SGST 3,600.00)'),
      findsOneWidget,
    );
    await tester.tap(find.text('Issue'));
    await pumpUntil(tester, find.byTooltip('Share PDF'));

    expect(find.text('Record payment'), findsOneWidget);
    expect(find.byTooltip('Open PDF'), findsOneWidget);
    expect(find.text('Nothing received yet'), findsOneWidget);
    expect(find.textContaining('Issued NDC/'), findsOneWidget);
  });

  testWidgets('a part payment leaves the rest due', (tester) async {
    await launchDemo(tester);
    await openInvoice(tester, 'Frontend');
    expect(find.text('PART PAID'), findsOneWidget);

    await tester.tap(find.text('Record payment'));
    await settle(tester);
    await tester.enterText(
      find.widgetWithText(TextField, 'Received (INR)'),
      '10,000',
    );
    await tester.tap(find.text('Record'));
    await settle(tester);

    expect(find.text('Payment recorded; ₹25,400.00 still due'), findsOneWidget);
    expect(find.text('Payment'), findsNWidgets(2));
  });

  testWidgets('a refused payment stays in the form under its field', (
    tester,
  ) async {
    await launchDemo(tester);
    await openInvoice(tester, 'Frontend');
    await tester.tap(find.text('Record payment'));
    await settle(tester);
    await tester.enterText(find.widgetWithText(TextField, 'TDS'), 'some');
    await tester.tap(find.text('Record'));
    await settle(tester);

    expect(find.textContaining('amount must be a number'), findsOneWidget);
    expect(find.text('Record'), findsOneWidget);
  });

  testWidgets('a credit note for the whole invoice cancels it', (tester) async {
    await launchDemo(tester);
    await openInvoice(tester, 'Dashboards');

    await tester.tap(find.byTooltip('Credit note'));
    await settle(tester);
    await tester.enterText(
      find.widgetWithText(TextField, 'Reason'),
      'Billed in error',
    );
    await tester.tap(find.text('Preview'));
    await settle(tester);
    expect(find.text('₹0.00 will still be due.'), findsOneWidget);
    await tester.tap(find.text('Issue'));
    await pumpUntil(tester, find.text('CREDITED'));

    expect(find.text('Record payment'), findsNothing);
    expect(find.textContaining('CN/'), findsWidgets);
  });

  testWidgets('a credit note on a paid invoice refunds what was paid', (
    tester,
  ) async {
    await launchDemo(tester);
    await openInvoice(tester, 'Design');
    expect(find.text('PAID'), findsOneWidget);

    await tester.tap(find.byTooltip('Credit note'));
    await settle(tester);
    await tester.enterText(find.widgetWithText(TextField, 'Amount'), '10000');
    await tester.enterText(
      find.widgetWithText(TextField, 'Reason'),
      'Discount',
    );
    await tester.tap(find.text('Preview'));
    await settle(tester);
    expect(
      find.text('₹11,800.00 already paid will be refunded.'),
      findsOneWidget,
    );
    await tester.tap(find.text('Issue'));
    await pumpUntil(tester, find.textContaining('₹11,800.00 refunded'));

    expect(find.text('PAID'), findsOneWidget);
    expect(find.byTooltip('Credit note'), findsOneWidget);
  });

  testWidgets('cancelling after a TDS payment asks for the TDS reversed', (
    tester,
  ) async {
    await launchDemo(tester);
    await openInvoice(tester, 'Design');
    await tester.tap(find.byTooltip('Credit note'));
    await settle(tester);
    await tester.enterText(find.widgetWithText(TextField, 'Amount'), '50000');
    await tester.enterText(
      find.widgetWithText(TextField, 'Reason'),
      'Cancelled',
    );
    await tester.enterText(
      find.widgetWithText(TextField, 'TDS reversed'),
      '5000',
    );
    await tester.tap(find.text('Preview'));
    await settle(tester);
    expect(find.textContaining('₹5,000.00 TDS reversed'), findsOneWidget);
    await tester.tap(find.text('Issue'));
    await pumpUntil(tester, find.textContaining('₹54,000.00 cash'));

    expect(find.byTooltip('Credit note'), findsNothing);
  });

  testWidgets('settings shows the business profile and saves edits', (
    tester,
  ) async {
    await launchDemo(tester);
    await tester.tap(find.byIcon(Icons.settings_outlined));
    await settle(tester);
    expect(find.text('Nila Design & Code'), findsOneWidget);

    await tester.tap(find.byTooltip('Edit business details'));
    await settle(tester);
    await tester.enterText(
      find.widgetWithText(TextField, 'Name'),
      'Nila Studio',
    );
    final terms = find.widgetWithText(TextField, 'Terms');
    await tester.ensureVisible(terms);
    await tester.enterText(
      terms,
      'Bank: HDFC 0001234\n\nPayment due in 15 days\nThank you',
    );
    await tester.tap(find.text('Save'));
    await pumpUntil(tester, find.text('Nila Studio'));

    expect(find.text('Business details saved'), findsOneWidget);
    final saved = await tester.runAsync(
      () => ProviderScope.containerOf(
        tester.element(find.byType(Scaffold).first),
      ).read(compterProvider).profile(),
    );
    // Blank lines are dropped
    expect(saved!.terms, [
      'Bank: HDFC 0001234',
      'Payment due in 15 days',
      'Thank you',
    ]);
  });

  testWidgets('searching narrows the invoices by client, kind or status', (
    tester,
  ) async {
    await launchDemo(tester);
    await tester.tap(find.byIcon(Icons.receipt_long_outlined));
    await settle(tester);
    await tester.scrollUntilVisible(find.text('Initrode GmbH'), 200);
    await tester.tap(find.byTooltip('Search'));
    await tester.pumpAndSettle();

    await tester.enterText(find.byType(TextField), 'acme');
    await tester.pumpAndSettle();
    expect(find.text('Acme Pvt Ltd'), findsNWidgets(2));
    expect(find.text('Initrode GmbH'), findsNothing);

    await tester.enterText(find.byType(TextField), 'export');
    await tester.pumpAndSettle();
    expect(find.text('Initrode GmbH'), findsOneWidget);
    expect(find.text('Acme Pvt Ltd'), findsNothing);
    expect(find.text('Outstanding'), findsOneWidget);
  });
}
