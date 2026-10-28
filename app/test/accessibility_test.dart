import 'dart:io';

import 'package:compter/src/appearance.dart';
import 'package:compter/src/onboarding.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// Flutter's accessibility guidelines on every main screen, in every theme: tap targets big
/// enough (48 dp on Android, 44 on iOS), every one of them labelled for a screen reader, and
/// text that contrasts with what is behind it (WCAG AA)
void main() {
  setUpAll(initRust);

  Future<void> meetsAll(WidgetTester tester) async {
    await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
    await expectLater(tester, meetsGuideline(iOSTapTargetGuideline));
    await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
    await expectLater(tester, meetsGuideline(textContrastGuideline));
  }

  /// A new, empty business, opened on the welcome pages
  Future<void> launchWelcome(
    WidgetTester tester, {
    List<Override> overrides = const [],
  }) async {
    final app = await tester.runAsync(() async {
      final data = await Directory.systemTemp.createTemp('compter_a11y');
      return CompterApp.open(dataDir: data.path);
    });
    await launchWith(
      tester,
      app!,
      overrides: [...overrides, showWelcomeProvider.overrideWithValue(true)],
    );
  }

  Future<void> openTab(WidgetTester tester, IconData icon) async {
    await tester.tap(find.byIcon(icon));
    await settle(tester);
  }

  group('twice the text size, on a small phone', () {
    /// 360 x 780 points with text at 200%, Android's largest setting: an overflow anywhere
    /// fails the test
    void largeText(WidgetTester tester) {
      tester.view.physicalSize = const Size(1080, 2340);
      tester.view.devicePixelRatio = 3;
      tester.platformDispatcher.textScaleFactorTestValue = 2;
      addTearDown(tester.view.reset);
      addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    }

    testWidgets('every tab, a project and an invoice', (tester) async {
      largeText(tester);
      await launchDemo(tester);
      await tester.tap(find.text('Website Revamp'));
      await settle(tester);
      await tester.pageBack();
      await settle(tester);
      for (final icon in [
        Icons.receipt_long_outlined,
        Icons.account_balance_wallet_outlined,
        Icons.insights_outlined,
        Icons.settings_outlined,
      ]) {
        await openTab(tester, icon);
      }
      await openTab(tester, Icons.receipt_long_outlined);
      await tester.tap(find.textContaining('Frontend'));
      await settle(tester);
    });

    testWidgets('the welcome pages and getting started', (tester) async {
      largeText(tester);
      await launchWelcome(tester);
      for (var page = 0; page < 2; page++) {
        await tester.tap(find.text('Next'));
        await tester.pumpAndSettle();
      }
      await tester.tap(find.text('Get started'));
      await pumpUntil(tester, find.text('0 of 3 done'));
    });
  });

  for (final theme in AppTheme.values) {
    group(theme.label, () {
      final themed = appThemeProvider.overrideWith(
        () => AppThemeNotifier(theme),
      );

      testWidgets('projects and a project', (tester) async {
        final semantics = tester.ensureSemantics();
        await launchDemo(tester, overrides: [themed]);
        await meetsAll(tester);

        await tester.tap(find.text('Website Revamp'));
        await settle(tester);
        await meetsAll(tester);
        semantics.dispose();
      });

      testWidgets('invoices and an invoice', (tester) async {
        final semantics = tester.ensureSemantics();
        await launchDemo(tester, overrides: [themed]);
        await openTab(tester, Icons.receipt_long_outlined);
        await meetsAll(tester);

        await tester.tap(find.textContaining('Frontend'));
        await settle(tester);
        await meetsAll(tester);
        semantics.dispose();
      });

      testWidgets('expenses, reports and settings', (tester) async {
        final semantics = tester.ensureSemantics();
        await launchDemo(tester, overrides: [themed]);
        for (final icon in [
          Icons.account_balance_wallet_outlined,
          Icons.insights_outlined,
          Icons.settings_outlined,
        ]) {
          await openTab(tester, icon);
          await meetsAll(tester);
        }
        semantics.dispose();
      });

      testWidgets('the welcome pages and getting started', (tester) async {
        final semantics = tester.ensureSemantics();
        await launchWelcome(tester, overrides: [themed]);
        await meetsAll(tester);

        await tester.tap(find.text('Skip'));
        await pumpUntil(tester, find.text('0 of 3 done'));
        await meetsAll(tester);
        semantics.dispose();
      });
    });
  }
}
