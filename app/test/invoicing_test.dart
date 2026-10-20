import 'dart:io';

import 'package:compter/src/compter.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/api/rows.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The amount field of a line picked for a credit note on a rupee invoice
const _creditAmount = 'Credit, before GST, in INR';

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

  testWidgets(
    'several milestones go on one invoice and are credited together',
    (tester) async {
      final compter = await tester.runAsync(() async {
        final scratch = await Directory.systemTemp.createTemp('compter_test');
        final app = await CompterApp.demo(scratchDir: scratch.path);
        final website = (await app.projects(
          includeArchived: false,
        )).firstWhere((p) => p.name == 'Website Revamp');
        await app.addMilestone(
          projectId: website.id,
          input: const MilestoneInput(
            name: 'Support',
            dueDate: '2026-12-01',
            amount: '10,000',
          ),
        );
        return app;
      });
      await launchWith(tester, compter!);
      await tester.tap(find.text('Website Revamp'));
      await settle(tester);
      await tester.ensureVisible(find.text('Launch'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Launch'));
      await settle(tester);
      await tester.tap(find.text('Issue invoice'));
      await settle(tester);

      // Launch is picked; Select all adds Support
      expect(find.text('Milestones to invoice'), findsOneWidget);
      await tester.tap(find.text('Select all'));
      await tester.pump();
      await tester.tap(find.text('Next'));
      await settle(tester);
      expect(find.text('2 milestones'), findsOneWidget);
      // ₹50,000 + 18% GST
      expect(find.textContaining('₹59,000.00'), findsOneWidget);
      await tester.tap(find.text('Issue'));
      await pumpUntil(tester, find.byTooltip('Share PDF'));
      expect(find.textContaining('Launch, Support'), findsWidgets);

      // Nothing is picked on an invoice of several lines until asked
      await tester.tap(find.byTooltip('Credit note'));
      await settle(tester);
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Next'))
            .onPressed,
        isNull,
      );
      await tester.tap(find.text('Select all'));
      await tester.pump();
      await tester.tap(find.text('Next'));
      await settle(tester);
      await tester.enterText(
        find.widgetWithText(TextField, 'Reason'),
        'Project cancelled',
      );
      await tester.tap(find.text('Preview'));
      await settle(tester);
      expect(find.text('₹0.00 will still be due.'), findsOneWidget);
      await tester.tap(find.text('Issue'));
      await pumpUntil(tester, find.text('CREDITED'));
    },
  );

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

    // The one line is picked, for all of it
    await tester.tap(find.byTooltip('Credit note'));
    await settle(tester);
    expect(find.text('35000.00'), findsOneWidget);
    await tester.tap(find.text('Next'));
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

    // Paid: nothing is picked or prefilled, and the guide says what is due
    await tester.tap(find.byTooltip('Credit note'));
    await settle(tester);
    expect(
      find.textContaining('₹0.00 due · up to ₹59,000.00 incl. GST'),
      findsOneWidget,
    );
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Design'));
    await tester.pump();
    await tester.enterText(
      find.widgetWithText(TextField, _creditAmount),
      '10000',
    );
    await tester.tap(find.text('Next'));
    await settle(tester);
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
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Design'));
    await tester.pump();
    // Picking the line fills in all that is left on it
    expect(find.text('50000.00'), findsOneWidget);
    // More than the line has left is refused under it, before the sheet closes
    await tester.enterText(
      find.widgetWithText(TextField, _creditAmount),
      '50001',
    );
    await tester.tap(find.text('Next'));
    await settle(tester);
    expect(find.textContaining('left to credit on this line'), findsOneWidget);
    await tester.enterText(
      find.widgetWithText(TextField, _creditAmount),
      '49999',
    );
    await tester.tap(find.text('Next'));
    await settle(tester);
    // Cancelling the next step comes back to the lines, with the amount still typed
    await tester.tap(find.text('Cancel'));
    await settle(tester);
    expect(find.text('Lines to credit'), findsOneWidget);
    expect(find.text('49999'), findsOneWidget);
    await tester.enterText(
      find.widgetWithText(TextField, _creditAmount),
      '50000',
    );
    await tester.tap(find.text('Next'));
    await settle(tester);
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
    // Not the cash split: the closing confirm dialog shows it too, before the note is saved
    await pumpUntil(tester, find.textContaining('Issued CN/'));

    expect(find.textContaining('₹54,000.00 cash'), findsOneWidget);
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
