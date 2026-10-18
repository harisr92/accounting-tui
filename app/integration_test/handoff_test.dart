import 'dart:io';

import 'package:compter/src/handoff.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/api/rows.dart';
import 'package:compter/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:path_provider/path_provider.dart';

/// The hand-off on the device itself: closing and reopening the database, and the snapshot
/// files, inside the app's sandbox. The system file picker and the share sheet are left out;
/// they are platform UI.
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => RustLib.init());

  ProjectInput project(String name) => ProjectInput(
    name: name,
    clientName: 'Acme Pvt Ltd',
    clientState: '29',
    clientAddress: const [],
  );

  Future<Directory> fresh(String name) async {
    final base = await getTemporaryDirectory();
    final dir = Directory('${base.path}/handoff_test/$name');
    if (await dir.exists()) await dir.delete(recursive: true);
    return dir.create(recursive: true);
  }

  Future<List<PickedFile>> read(String snapshot, String manifest) async => [
    (name: snapshotFile, bytes: await File(snapshot).readAsBytes()),
    (name: manifestFile, bytes: await File(manifest).readAsBytes()),
  ];

  Future<List<String>> names(CompterApp app) async =>
      (await app.projects(includeArchived: true)).map((p) => p.name).toList()
        ..sort();

  testWidgets('a round trip with the computer', (tester) async {
    final computer = await CompterApp.open(
      dataDir: (await fresh('computer')).path,
    );
    final phone = await CompterApp.open(dataDir: (await fresh('phone')).path);
    final staging = (await fresh('staging')).path;
    await computer.createProject(input: project('Site'));

    // The computer's snapshot, carried to the phone
    final published = await computer.sendSnapshot(
      folder: (await fresh('folder')).path,
      deviceName: 'laptop',
    );
    final picked = await read(published.snapshot, published.manifest);
    expect(checkPicked(picked), isNull);
    final received = await phone.receiveSnapshot(
      folder: await inboxWith(staging, picked),
    );
    expect(received.conflict, isFalse);
    expect(received.message, 'Took snapshot 1 from the computer');
    expect(await names(phone), ['Site']);

    // The phone's work, carried back
    await phone.createProject(input: project('App'));
    expect((await phone.syncStatus()).changed, isTrue);
    final sent = await phone.sendSnapshot(
      folder: await outboxIn(staging),
      deviceName: deviceName(),
    );
    expect(sent.message, startsWith('Snapshot 2 is ready'));
    expect(await names(phone), ['App', 'Site'], reason: 'reopened after');
    final back = await computer.receiveSnapshot(
      folder: await inboxWith(
        staging,
        await read(sent.snapshot, sent.manifest),
      ),
    );
    expect(back.message, 'Took snapshot 2 from the computer');
    expect(await names(computer), ['App', 'Site']);

    // A send that never arrived: the folder still holds snapshot 1
    await phone.createProject(input: project('Audit'));
    await phone.sendSnapshot(
      folder: await outboxIn(staging),
      deviceName: deviceName(),
    );
    final stale = await phone.receiveSnapshot(
      folder: await inboxWith(staging, picked),
    );
    expect(stale.conflict, isTrue);
    expect(stale.message, contains('saved as a copy'));
  });
}
