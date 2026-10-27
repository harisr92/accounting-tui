import 'dart:io';

import 'package:compter/src/handoff.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/sync/sync_controller.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';
import 'support.dart';

/// Turning on Google Drive sync in Settings, against a fake Drive: run `flutter test`
void main() {
  setUpAll(initRust);

  testWidgets('choosing Google Drive starts syncing in its app folder', (
    tester,
  ) async {
    final phone = await tester.runAsync(() async {
      final data = await Directory.systemTemp.createTemp('compter_phone');
      return CompterApp.open(dataDir: data.path);
    });
    final staging = await tester.runAsync(
      () => Directory.systemTemp.createTemp('compter_handoff'),
    );
    final folder = FakeFolder();
    await launchWith(
      tester,
      phone!,
      overrides: [
        cloudAccountProvider.overrideWithValue(FakeAccount(folder)),
        handoffDirProvider.overrideWith((ref) async => staging!.path),
      ],
    );
    await tester.tap(find.byIcon(Icons.settings_outlined));
    await settle(tester);
    await tester.scrollUntilVisible(find.text('Sync method'), 200);
    await settle(tester);

    await tester.tap(find.text('Sync method'));
    await settle(tester);
    await tester.tap(find.text('Google Drive, automatic'));
    await pumpUntil(tester, find.textContaining('Synced at'));

    expect(find.text('nila@example.com'), findsOneWidget);
    expect(
      folder.files.keys,
      containsAll(['compter.redb', 'manifest.json', 'lease.json']),
    );
  });
}
