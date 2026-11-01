import 'dart:io';

import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/api/invoicing.dart';
import 'package:compter/src/sync/sync_controller.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// Sync that needs the user to sign in again, and never runs
class _SignedOut extends SyncController {
  @override
  SyncReport build() => const SyncReport(SyncPhase.signedOut, 'Sign in again');

  @override
  Future<void> start() async {}
}

/// Home on the demo business against the real Rust core: what is owed today, and Settings
/// behind its gear
void main() {
  setUpAll(initRust);

  Badge gearBadge(WidgetTester tester) => tester.widget<Badge>(
    find.ancestor(
      of: find.byIcon(Icons.settings_outlined),
      matching: find.byType(Badge),
    ),
  );

  testWidgets('the app opens on Home, with five tabs and no Settings tab', (
    tester,
  ) async {
    await launchDemo(tester, tab: null);

    // The demo's invoices are all from 2026, so these hold on any later day
    expect(find.text('Owed to you'), findsOneWidget);
    expect(find.text('₹95,285.00'), findsOneWidget);
    expect(find.text('Umbrella Analytics'), findsOneWidget);
    expect(find.text('OVERDUE'), findsWidgets);

    final tabs = tester.widget<CupertinoTabBar>(find.byType(CupertinoTabBar));
    expect(tabs.items.map((item) => item.label), [
      'Home',
      'Projects',
      'Invoices',
      'Expenses',
      'Reports',
    ]);
  });

  testWidgets('an unpaid invoice opens over Home and goes back to it', (
    tester,
  ) async {
    await launchDemo(tester, tab: null);

    await tester.tap(find.text('Umbrella Analytics'));
    await settle(tester);
    expect(find.text('Record payment'), findsOneWidget);

    await tester.pageBack();
    await settle(tester);
    expect(find.text('Owed to you'), findsOneWidget);
  });

  testWidgets('the gear opens Settings, from any tab', (tester) async {
    await launchDemo(tester);
    expect(find.byTooltip('Settings'), findsNothing);

    await openSettings(tester);
    expect(find.text('Business profile'), findsOneWidget);
    expect(find.byType(CupertinoTabBar).hitTestable(), findsNothing);

    await tester.pageBack();
    await settle(tester);
    expect(find.text('Owed to you'), findsOneWidget);
  });

  testWidgets('the gear has no badge while sync is fine', (tester) async {
    await launchDemo(tester, tab: null);
    expect(gearBadge(tester).isLabelVisible, isFalse);
  });

  testWidgets('a signed-out sync badges the gear', (tester) async {
    await launchDemo(
      tester,
      tab: null,
      overrides: [syncControllerProvider.overrideWith(_SignedOut.new)],
    );
    expect(gearBadge(tester).isLabelVisible, isTrue);
    expect(find.byTooltip('Settings: sync needs you'), findsOneWidget);
  });

  testWidgets('coming back to the app works Home out again', (tester) async {
    final compter = (await tester.runAsync(() async {
      final scratch = await Directory.systemTemp.createTemp('compter_home');
      return CompterApp.demo(scratchDir: scratch.path);
    }))!;
    await launchWith(tester, compter, tab: null);
    expect(find.text('₹95,285.00'), findsOneWidget);

    // Prototype's ₹18,585 is paid while the app is in the background, as a sync would bring it
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    await tester.runAsync(
      () => compter.recordPayment(
        number: 'NDC/26-27/005',
        input: const PaymentInput(
          date: '',
          received: '18585',
          tdsOrBanked: '0',
        ),
      ),
    );
    expect(find.text('₹95,285.00'), findsOneWidget);

    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await pumpUntil(tester, find.text('₹76,700.00'));
    expect(find.text('Globex Labs'), findsNothing);
  });
}
