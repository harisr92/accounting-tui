import 'dart:async';

import 'package:compter/src/subscription/paywall.dart';
import 'package:compter/src/subscription/store.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';
import 'support.dart';

/// The iPhone's subscription on the demo business against the real Rust core: without one the
/// app is read-only, and the paywall stands in for every form that would write
void main() {
  setUpAll(initRust);

  Future<FakeStore> launchSelling(WidgetTester tester, FakeStore store) async {
    await launchDemo(
      tester,
      overrides: <Override>[storeProvider.overrideWithValue(store)],
    );
    return store;
  }

  Finder field(String label) => find.widgetWithText(TextField, label);

  testWidgets('without a subscription a new project shows the paywall', (
    tester,
  ) async {
    await launchSelling(tester, FakeStore());

    expect(find.text('Read-only'), findsOneWidget);
    await tester.tap(find.byTooltip('New project'));
    await settle(tester);

    expect(find.text('Start your free month'), findsOneWidget);
    expect(find.text('Free month, then ₹299.00 a month'), findsOneWidget);
    expect(field('Name'), findsNothing);

    await tester.tap(find.text('Not now'));
    await settle(tester);
    expect(field('Name'), findsNothing);
  });

  testWidgets('subscribing in the paywall opens the form that was asked for', (
    tester,
  ) async {
    final store = await launchSelling(tester, FakeStore());

    await tester.tap(find.byTooltip('New project'));
    await settle(tester);
    await tester.tap(find.text('Free month, then ₹2,999.00 a year'));
    await settle(tester);

    expect(store.purchased, ['app.compter.pro.yearly']);
    expect(field('Name'), findsWidgets);
    await tester.tap(find.text('Cancel'));
    await settle(tester);
    expect(find.text('Read-only'), findsNothing);
  });

  testWidgets('read-only still opens invoices and exports returns', (
    tester,
  ) async {
    await launchSelling(tester, FakeStore());

    await tester.tap(find.byIcon(Icons.receipt_long_outlined));
    await settle(tester);
    expect(find.textContaining('NDC/'), findsWidgets);

    await tester.tap(find.byIcon(Icons.insights_outlined));
    await settle(tester);
    await tester.tap(find.text('Export return'));
    await settle(tester);
    await tester.tap(find.text('GSTR-1 for a quarter'));
    await settle(tester);

    expect(find.text('Start your free month'), findsNothing);
    expect(find.text('Export'), findsOneWidget);
  });

  testWidgets('an expiry from the store makes the app read-only', (
    tester,
  ) async {
    final store = await launchSelling(
      tester,
      FakeStore(held: const Entitlement(active: true)),
    );
    expect(find.text('Read-only'), findsNothing);

    store.change(Entitlement.none);
    await settle(tester);

    expect(find.text('Read-only'), findsOneWidget);
  });

  testWidgets('settings shows when the free month ends', (tester) async {
    await launchSelling(
      tester,
      FakeStore(
        held: Entitlement(
          active: true,
          expiresAt: DateTime(2026, 11, 30),
          inTrial: true,
          freePeriod: 'month',
        ),
      ),
    );
    await tester.tap(find.byIcon(Icons.settings_outlined));
    await settle(tester);
    await scrollToMiddle(tester, find.text('Manage subscription'));

    expect(find.text('Free month, then billed on 30 Nov 2026'), findsOneWidget);
  });

  testWidgets('where compter is free nothing asks for a subscription', (
    tester,
  ) async {
    await launchDemo(tester);

    expect(find.text('Read-only'), findsNothing);
    await tester.tap(find.byTooltip('New project'));
    await settle(tester);
    expect(field('Name'), findsWidgets);
  });

  testWidgets(
    'a purchase waiting for a parent says so, and unlocks on approval',
    (tester) async {
      final store = await launchSelling(tester, FakeStore(askToBuy: true));

      await tester.tap(find.byTooltip('New project'));
      await settle(tester);
      await tester.tap(find.text('Free month, then ₹299.00 a month'));
      await settle(tester);

      expect(
        find.text('Asked for approval. compter unlocks once it’s approved.'),
        findsOneWidget,
      );
      expect(find.text('Start your free month'), findsNothing);
      expect(field('Name'), findsNothing);
      expect(find.text('Read-only'), findsOneWidget);

      store.change(const Entitlement(active: true));
      await settle(tester);
      expect(find.text('Read-only'), findsNothing);
    },
  );

  testWidgets('tapping restore again joins the restore already running', (
    tester,
  ) async {
    final store = FakeStore()..holdRestore = Completer<void>();
    await launchSelling(tester, store);
    await tester.tap(find.byIcon(Icons.settings_outlined));
    await settle(tester);
    await scrollToMiddle(tester, find.text('Restore purchases'));

    await tester.tap(find.text('Restore purchases'));
    await tester.pump();
    await tester.tap(find.text('Restore purchases'));
    await tester.pump();
    store.holdRestore!.complete();
    await settle(tester);

    expect(store.restores, 1);
    expect(
      find.text('No subscription found for this Apple ID'),
      findsOneWidget,
    );
  });

  testWidgets('the paywall names the offer the store has', (tester) async {
    final store = FakeStore();
    await launchSelling(tester, store);
    store.freePeriod = '2 weeks';

    await tester.tap(find.byTooltip('New project'));
    await settle(tester);

    expect(find.text('Start your 2 free weeks'), findsOneWidget);
    expect(find.text('2 free weeks, then ₹299.00 a month'), findsOneWidget);
  });

  test('settings words each state of the subscription', () {
    final ends = DateTime(2026, 12, 1);
    expect(
      describeEntitlement(Entitlement.none),
      'Read-only: subscribe to make changes',
    );
    expect(
      describeEntitlement(
        Entitlement(
          active: true,
          expiresAt: ends,
          inTrial: true,
          freePeriod: 'month',
          willRenew: false,
        ),
      ),
      'Free month, ends 1 Dec 2026',
    );
    expect(
      describeEntitlement(
        Entitlement(
          active: true,
          expiresAt: ends,
          inTrial: true,
          freePeriod: '2 weeks',
        ),
      ),
      '2 free weeks, then billed on 1 Dec 2026',
    );
    expect(
      describeEntitlement(
        Entitlement(active: true, expiresAt: ends, inTrial: true),
      ),
      'Free trial, then billed on 1 Dec 2026',
    );
    expect(
      describeEntitlement(Entitlement(active: true, expiresAt: ends)),
      'Renews 1 Dec 2026',
    );
    expect(
      describeEntitlement(
        Entitlement(active: true, expiresAt: ends, willRenew: false),
      ),
      'Ends 1 Dec 2026',
    );
  });
}
