import 'dart:io';

import 'package:compter/main.dart';
import 'package:compter/src/compter.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';

/// The host build of compter-mobile, which the build hook (hook/build.dart) makes and
/// `flutter test` stages under build/native_assets before the tests run
ExternalLibrary hostLibrary() {
  final (os, file) = Platform.isMacOS
      ? ('macos', 'libcompter_mobile.dylib')
      : Platform.isWindows
      ? ('windows', 'compter_mobile.dll')
      : ('linux', 'libcompter_mobile.so');
  return ExternalLibrary.open('build/native_assets/$os/$file');
}

Future<void> initRust() => RustLib.init(externalLibrary: hostLibrary());

/// The app on the demo business, in memory, writing its PDFs to a scratch folder.
/// [overrides] replace providers, e.g. to pin a tab's year.
Future<void> launchDemo(
  WidgetTester tester, {
  List<Override> overrides = const [],
}) async {
  final compter = await tester.runAsync(() async {
    final scratch = await Directory.systemTemp.createTemp('compter_test');
    return CompterApp.demo(scratchDir: scratch.path);
  });
  await launchWith(tester, compter!, overrides: overrides);
}

/// The app on [compter], an opened database
Future<void> launchWith(
  WidgetTester tester,
  CompterApp compter, {
  List<Override> overrides = const [],
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [compterProvider.overrideWithValue(compter), ...overrides],
      child: const CompterMobile(),
    ),
  );
  await settle(tester);
}

/// Let Rust calls (real async work, outside the test's fake clock) finish, then settle. A
/// spinner animates forever, so frames are pumped one at a time until none is left.
Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 100; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
    await tester.pump();
    // Off screen too: a list builds a little past its edge, and a spinner there still spins
    final spinners = find.byType(
      CircularProgressIndicator,
      skipOffstage: false,
    );
    if (spinners.evaluate().isEmpty) break;
  }
  await tester.pumpAndSettle();
}

/// Pump until [finder] finds something: for Rust work that shows no spinner, like saving after
/// a dialog closes. Fails after [timeout].
Future<void> pumpUntil(
  WidgetTester tester,
  Finder finder, {
  Duration timeout = const Duration(seconds: 10),
}) async {
  final end = DateTime.now().add(timeout);
  while (finder.evaluate().isEmpty) {
    if (DateTime.now().isAfter(end)) {
      final notices = find
          .descendant(of: find.byType(SnackBar), matching: find.byType(Text))
          .evaluate()
          .map((e) => (e.widget as Text).data);
      fail('Timed out waiting for $finder; notices: $notices');
    }
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 20)),
    );
    await tester.pump();
  }
  await settle(tester);
}
