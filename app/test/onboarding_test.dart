import 'dart:io';

import 'package:compter/src/onboarding.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The welcome pages on a first start, and the "Get started" card on Projects
void main() {
  setUpAll(initRust);

  Future<CompterApp> emptyApp() async {
    final data = await Directory.systemTemp.createTemp('compter_new');
    return CompterApp.open(dataDir: data.path);
  }

  Future<CompterApp> launchEmpty(
    WidgetTester tester, {
    bool welcome = false,
  }) async {
    final app = (await tester.runAsync(emptyApp))!;
    await launchWith(
      tester,
      app,
      overrides: [showWelcomeProvider.overrideWithValue(welcome)],
    );
    return app;
  }

  /// The card loads with no spinner, so settling doesn't wait for it
  final card = find.text('0 of 3 done');

  Finder step(String title) => find.widgetWithText(ListTile, title);

  test('only a phone with no projects starts on the welcome pages', () async {
    useMemoryPreferences();
    final empty = await emptyApp();
    expect(await needsWelcome(empty), isTrue);

    final scratch = await Directory.systemTemp.createTemp('compter_demo');
    final demo = await CompterApp.demo(scratchDir: scratch.path);
    expect(await needsWelcome(demo), isFalse);
    // Remembered: the phone had data once, so it never welcomes again
    expect(await needsWelcome(empty), isFalse);
  });

  test('read once, the welcome pages stay away', () async {
    useMemoryPreferences();
    final empty = await emptyApp();
    await welcomeSeen();
    expect(await needsWelcome(empty), isFalse);
  });

  testWidgets('the welcome pages lead to the getting started card', (
    tester,
  ) async {
    final app = await launchEmpty(tester, welcome: true);
    expect(find.text('Bill by milestone or by the hour'), findsOneWidget);

    await tester.tap(find.text('Next'));
    await tester.pumpAndSettle();
    expect(find.text('GST invoices, done right'), findsOneWidget);
    await tester.tap(find.text('Next'));
    await tester.pumpAndSettle();
    expect(find.text('Skip').hitTestable(), findsNothing);
    await tester.tap(find.text('Get started'));
    await pumpUntil(tester, card);
    // Still no projects, so only the saved flag keeps it away
    expect(await tester.runAsync(() => needsWelcome(app)), isFalse);
  });

  testWidgets('skipping goes straight to the projects', (tester) async {
    await launchEmpty(tester, welcome: true);

    await tester.tap(find.text('Skip'));
    await pumpUntil(tester, card);
  });

  testWidgets('steps tick from what is saved', (tester) async {
    await launchEmpty(tester);
    await pumpUntil(tester, card);
    expect(find.text('Bill by milestone or by the hour'), findsNothing);
    // An invoice needs business details and a project first
    expect(
      tester.widget<ListTile>(step('Issue your first invoice')).enabled,
      isFalse,
    );

    await tester.tap(step('Add your first project'));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Shop');
    await tester.enterText(find.widgetWithText(TextField, 'Client'), 'Acme');
    await tester.tap(find.text('Save'));
    // The new project opens over the tabs once saved, with no spinner for settle to wait on;
    // back on the list, the card has moved on
    await pumpUntil(tester, find.text('Add milestone'));
    await tester.pageBack();
    await pumpUntil(tester, find.text('1 of 3 done'));
    expect(
      tester.widget<ListTile>(step('Add your first project')).enabled,
      isFalse,
    );
  });

  testWidgets('a hidden card stays hidden', (tester) async {
    await launchEmpty(tester);
    await pumpUntil(tester, card);

    await tester.tap(find.byTooltip('Hide getting started'));
    await settle(tester);

    expect(card, findsNothing);
    expect(
      find.text('No projects yet. Add one to start billing.'),
      findsOneWidget,
    );
  });

  testWidgets('a business already set up sees no card', (tester) async {
    await launchDemo(tester);
    // Give the profile and invoices time to load, as the card waits for them
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 300)),
    );
    await tester.pump();

    expect(find.text('Get started'), findsNothing);
  });
}
