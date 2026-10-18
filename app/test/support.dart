import 'dart:io';

import 'package:compter/main.dart';
import 'package:compter/src/compter.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';

/// The host build of compter-mobile (`cargo build -p compter-mobile`), from the workspace's
/// target folder
ExternalLibrary hostLibrary() {
  final file = Platform.isMacOS
      ? 'libcompter_mobile.dylib'
      : Platform.isWindows
      ? 'compter_mobile.dll'
      : 'libcompter_mobile.so';
  return ExternalLibrary.open('../target/debug/$file');
}

Future<void> initRust() => RustLib.init(externalLibrary: hostLibrary());

/// The app on the demo business, in memory
Future<void> launchDemo(WidgetTester tester) async {
  final compter = await tester.runAsync(CompterApp.demo);
  await tester.pumpWidget(
    ProviderScope(
      overrides: [compterProvider.overrideWithValue(compter!)],
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
    if (find.byType(CircularProgressIndicator).evaluate().isEmpty) break;
  }
  await tester.pumpAndSettle();
}
