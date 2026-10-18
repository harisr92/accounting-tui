import 'dart:io';

import 'package:compter/src/handoff.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/api/rows.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// Handing the data between the phone and a computer, against the real Rust core: run
/// `cargo build -p compter-mobile` first, then `flutter test`. A second database plays the
/// computer and writes a real snapshot.
void main() {
  setUpAll(initRust);

  /// What the share sheet was handed, instead of showing it
  final shared = <List<Object?>>[];
  setUp(() {
    shared.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('dev.fluttercommunity.plus/share'),
          (call) async {
            final arguments = call.arguments as Map<Object?, Object?>;
            shared.add(arguments['paths']! as List<Object?>);
            return 'dev.fluttercommunity.plus/share/unavailable';
          },
        );
  });

  ProjectInput project(String name) => ProjectInput(
    name: name,
    clientName: 'Acme Pvt Ltd',
    clientState: '29',
    clientAddress: const [],
  );

  /// A computer with projects called [names], and the snapshot it published of them
  Future<List<PickedFile>> computerWith(
    WidgetTester tester,
    List<String> names,
  ) async {
    final files = await tester.runAsync(() async {
      final data = await Directory.systemTemp.createTemp('compter_computer');
      final folder = await Directory.systemTemp.createTemp('compter_folder');
      final computer = await CompterApp.open(dataDir: data.path);
      for (final name in names) {
        await computer.createProject(input: project(name));
      }
      final sent = await computer.sendSnapshot(
        folder: folder.path,
        deviceName: 'laptop',
      );
      return [
        (name: snapshotFile, bytes: await File(sent.snapshot).readAsBytes()),
        (name: manifestFile, bytes: await File(sent.manifest).readAsBytes()),
      ];
    });
    return files!;
  }

  /// Settings, scrolled down to the sync section
  Future<void> openSync(WidgetTester tester) async {
    await tester.tap(find.byIcon(Icons.settings_outlined));
    await settle(tester);
    await tester.scrollUntilVisible(find.text('Send to computer'), 200);
    // The section's status loads once it scrolls into view
    await settle(tester);
  }

  /// This phone, on a new install's empty database, picking [picked] when asked for files
  /// (or what [pick] returns then)
  Future<CompterApp> launchPhone(
    WidgetTester tester,
    List<PickedFile> picked, {
    List<PickedFile> Function()? pick,
  }) async {
    final phone = await tester.runAsync(() async {
      final data = await Directory.systemTemp.createTemp('compter_phone');
      return CompterApp.open(dataDir: data.path);
    });
    final staging = await tester.runAsync(
      () => Directory.systemTemp.createTemp('compter_handoff'),
    );
    await launchWith(
      tester,
      phone!,
      overrides: [
        pickFilesProvider.overrideWithValue(
          () async => pick == null ? picked : pick(),
        ),
        handoffDirProvider.overrideWith((ref) async => staging!.path),
      ],
    );
    await openSync(tester);
    return phone;
  }

  /// Tap [label] once the snack bar that may cover it is gone
  Future<void> tapButton(WidgetTester tester, String label) async {
    tester
        .state<ScaffoldMessengerState>(find.byType(ScaffoldMessenger))
        .removeCurrentSnackBar();
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(find.text(label), 200);
    await tester.ensureVisible(find.text(label));
    await tester.pumpAndSettle();
    await tester.tap(find.text(label));
  }

  testWidgets('a new phone gets the computer’s data', (tester) async {
    final snapshot = await computerWith(tester, ['Site']);
    await launchPhone(tester, snapshot);
    expect(find.text('Not synced with a computer yet'), findsOneWidget);

    await tapButton(tester, 'Get from computer');
    await pumpUntil(tester, find.text('Took snapshot 1 from the computer'));
    expect(
      find.text('Synced with snapshot 1; no changes since'),
      findsOneWidget,
    );

    // The list reloads without a spinner, showing the old rows meanwhile
    await tester.tap(find.byIcon(Icons.work_outline));
    await pumpUntil(tester, find.text('Site'));
  });

  testWidgets('copies and stray files are refused before anything is taken', (
    tester,
  ) async {
    final snapshot = await computerWith(tester, ['Site']);
    await launchPhone(tester, [
      (name: 'compter (1).redb', bytes: snapshot.first.bytes),
      snapshot.last,
    ]);

    await tapButton(tester, 'Get from computer');
    await settle(tester);

    expect(find.textContaining('(not compter (1).redb)'), findsOneWidget);
    expect(find.text('Not synced with a computer yet'), findsOneWidget);
  });

  testWidgets('sending hands both files to the share sheet', (tester) async {
    final snapshot = await computerWith(tester, ['Site']);
    await launchPhone(tester, snapshot);
    await tapButton(tester, 'Get from computer');
    await pumpUntil(tester, find.text('Took snapshot 1 from the computer'));

    await tapButton(tester, 'Send to computer');
    await settle(tester);
    await tester.tap(find.text('Continue'));
    await pumpUntil(tester, find.textContaining('Snapshot 1 is ready'));

    expect(shared, hasLength(1));
    expect(shared.single.map((path) => (path! as String).split('/').last), [
      snapshotFile,
      manifestFile,
    ]);
  });

  testWidgets('changes on both sides ask which to keep', (tester) async {
    final first = await computerWith(tester, ['Site']);
    var picked = first;
    final phone = await launchPhone(tester, first, pick: () => picked);
    await tapButton(tester, 'Get from computer');
    await pumpUntil(tester, find.text('Took snapshot 1 from the computer'));
    // The phone adds a project while another computer's snapshot, unrelated, arrives
    await tester.runAsync(() => phone.createProject(input: project('App')));
    picked = await computerWith(tester, ['Audit']);

    await tapButton(tester, 'Get from computer');
    await pumpUntil(tester, find.text('Which data to keep?'));
    await tester.tap(find.text("Take computer's"));
    await pumpUntil(tester, find.textContaining("Took the computer's data"));

    await tester.tap(find.byIcon(Icons.work_outline));
    await pumpUntil(tester, find.text('Audit'));
    expect(find.text('App'), findsNothing);
  });

  testWidgets('the demo has nothing to sync', (tester) async {
    await launchDemo(tester);
    await openSync(tester);

    expect(find.text('sync is off in demo mode'), findsOneWidget);
    final button = tester.widget<OutlinedButton>(
      find.ancestor(
        of: find.text('Get from computer'),
        matching: find.byWidgetPredicate((w) => w is OutlinedButton),
      ),
    );
    expect(button.onPressed, isNull);
  });
}
