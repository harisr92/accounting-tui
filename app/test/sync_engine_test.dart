import 'dart:convert';
import 'dart:io';

import 'package:compter/src/compter.dart';
import 'package:compter/src/handoff.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/api/rows.dart';
import 'package:compter/src/sync/sync_controller.dart';
import 'package:compter/src/sync/sync_settings.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';
import 'support.dart';

/// The engine against a fake Drive folder, with real Rust databases for the phone and the
/// computer: run `flutter test` (the build hook builds the Rust library first).
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(initRust);

  ProjectInput project(String name) => ProjectInput(
    name: name,
    clientName: 'Acme Pvt Ltd',
    clientState: '29',
    clientAddress: const [],
  );

  Future<CompterApp> open(String name) async => CompterApp.open(
    dataDir: (await Directory.systemTemp.createTemp(name)).path,
  );

  Future<List<String>> names(CompterApp app) async =>
      (await app.projects(includeArchived: true)).map((p) => p.name).toList()
        ..sort();

  /// The computer publishing to the folder, as compter does when it quits
  Future<void> publish(CompterApp computer, FakeFolder folder) async {
    final out = await Directory.systemTemp.createTemp('compter_out');
    final sent = await computer.sendSnapshot(
      folder: out.path,
      deviceName: 'laptop',
    );
    folder.files[snapshotFile] = await File(sent.snapshot).readAsBytes();
    folder.files[manifestFile] = await File(sent.manifest).readAsBytes();
  }

  /// The computer taking what the phone sent, as compter does when it starts
  Future<void> takeOnComputer(CompterApp computer, FakeFolder folder) async {
    final inbox = await Directory.systemTemp.createTemp('compter_in');
    await File(
      '${inbox.path}/$snapshotFile',
    ).writeAsBytes(folder.files[snapshotFile]!);
    await File(
      '${inbox.path}/$manifestFile',
    ).writeAsBytes(folder.files[manifestFile]!);
    final received = await computer.receiveSnapshot(folder: inbox.path);
    expect(received.conflict, isFalse, reason: received.message);
  }

  /// The phone's engine, set to sync through [folder]
  Future<(ProviderContainer, SyncController)> phoneSyncing(
    CompterApp phone,
    FakeFolder folder,
  ) async {
    useMemoryPreferences();
    final staging = await Directory.systemTemp.createTemp('compter_handoff');
    final container = ProviderContainer(
      overrides: [
        compterProvider.overrideWithValue(phone),
        cloudAccountProvider.overrideWithValue(FakeAccount(folder)),
        handoffDirProvider.overrideWith((ref) async => staging.path),
      ],
    );
    addTearDown(container.dispose);
    final controller = container.read(syncControllerProvider.notifier);
    await container.read(syncSettingsProvider.notifier).save((
      method: SyncMethod.googleDrive,
    ));
    await controller.start();
    return (container, controller);
  }

  SyncPhase phase(ProviderContainer container) =>
      container.read(syncControllerProvider).phase;

  test('a new phone takes the computer’s data and holds the lease', () async {
    final folder = FakeFolder();
    final computer = await open('computer');
    await computer.createProject(input: project('Site'));
    await publish(computer, folder);
    final phone = await open('phone');

    final (container, _) = await phoneSyncing(phone, folder);

    expect(phase(container), SyncPhase.synced);
    expect(await names(phone), ['Site']);
    expect(folder.json('lease.json')['device_name'], deviceName());
  });

  test('the phone’s changes go up, and the computer takes them', () async {
    final folder = FakeFolder();
    final computer = await open('computer');
    await computer.createProject(input: project('Site'));
    await publish(computer, folder);
    final phone = await open('phone');
    final (container, controller) = await phoneSyncing(phone, folder);

    await phone.createProject(input: project('App'));
    await controller.syncNow();

    expect(phase(container), SyncPhase.synced);
    expect(folder.json(manifestFile)['generation'], 2);
    await takeOnComputer(computer, folder);
    expect(await names(computer), ['App', 'Site']);
  });

  test('the computer’s newer snapshot comes down on the next sync', () async {
    final folder = FakeFolder();
    final computer = await open('computer');
    await computer.createProject(input: project('Site'));
    await publish(computer, folder);
    final phone = await open('phone');
    final (_, controller) = await phoneSyncing(phone, folder);

    await computer.createProject(input: project('Audit'));
    await publish(computer, folder);
    await controller.syncNow();

    expect(await names(phone), ['Audit', 'Site']);
  });

  test(
    'while compter is open on the computer, the phone sends nothing',
    () async {
      final folder = FakeFolder();
      final computer = await open('computer');
      await computer.createProject(input: project('Site'));
      await publish(computer, folder);
      final phone = await open('phone');
      final (container, controller) = await phoneSyncing(phone, folder);
      folder.files['lease.json'] = utf8.encode(
        await computer.leaseJson(deviceName: 'laptop'),
      );

      await phone.createProject(input: project('App'));
      await controller.syncNow();

      expect(phase(container), SyncPhase.blocked);
      expect(
        container.read(syncControllerProvider).message,
        contains('laptop'),
      );
      expect(folder.json(manifestFile)['generation'], 1);
      expect(folder.json('lease.json')['device_name'], 'laptop');
    },
  );

  test('changes on both sides ask, and keeping the phone’s sends it', () async {
    final folder = FakeFolder();
    final computer = await open('computer');
    await computer.createProject(input: project('Site'));
    await publish(computer, folder);
    final phone = await open('phone');
    final (container, controller) = await phoneSyncing(phone, folder);

    await phone.createProject(input: project('App'));
    await computer.createProject(input: project('Audit'));
    await publish(computer, folder);
    await controller.syncNow();
    expect(phase(container), SyncPhase.conflict);
    expect(container.read(syncControllerProvider).needsAttention, isTrue);

    await controller.settle(keepThisPhone: true);

    expect(phase(container), SyncPhase.synced);
    expect(folder.json(manifestFile)['device_name'], deviceName());
    expect(folder.json(manifestFile)['generation'], 3);
    expect(await names(phone), ['App', 'Site']);
  });

  test('offline is reported, and the next sync catches up', () async {
    final folder = FakeFolder();
    final computer = await open('computer');
    await computer.createProject(input: project('Site'));
    await publish(computer, folder);
    final phone = await open('phone');
    final (container, controller) = await phoneSyncing(phone, folder);

    folder.offline = true;
    await phone.createProject(input: project('App'));
    await controller.syncNow();
    expect(phase(container), SyncPhase.offline);
    expect(
      container.read(syncControllerProvider).message,
      'Offline; will retry',
    );

    folder.offline = false;
    await controller.syncNow();
    expect(phase(container), SyncPhase.synced);
    expect(folder.json(manifestFile)['generation'], 2);
  });

  test(
    'going to the background sends what changed and frees the lease',
    () async {
      final folder = FakeFolder();
      final computer = await open('computer');
      await computer.createProject(input: project('Site'));
      await publish(computer, folder);
      final phone = await open('phone');
      final (_, controller) = await phoneSyncing(phone, folder);
      expect(folder.files.containsKey('lease.json'), isTrue);

      await phone.createProject(input: project('App'));
      await controller.leave();

      expect(folder.files.containsKey('lease.json'), isFalse);
      expect(folder.json(manifestFile)['generation'], 2);
    },
  );
}
