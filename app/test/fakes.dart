import 'dart:convert';
import 'dart:typed_data';

import 'package:compter/src/sync/cloud.dart';

/// A Drive folder in memory; [offline] makes every call fail as with no network
class FakeFolder implements CloudFolder {
  final files = <String, Uint8List>{};
  bool offline = false;

  void _check() {
    if (offline) throw const CloudOffline('Offline');
  }

  @override
  Future<Uint8List?> read(String name) async {
    _check();
    return files[name];
  }

  @override
  Future<void> write(String name, Uint8List bytes) async {
    _check();
    files[name] = bytes;
  }

  @override
  Future<void> delete(String name) async {
    _check();
    files.remove(name);
  }

  Map<String, Object?> json(String name) =>
      jsonDecode(utf8.decode(files[name]!)) as Map<String, Object?>;
}

class FakeAccount implements CloudAccount {
  FakeAccount(this.drive);

  final FakeFolder drive;

  @override
  Future<String?> restore() async => 'nila@example.com';

  @override
  Future<String> signIn() async => 'nila@example.com';

  @override
  Future<void> signOut() async {}

  @override
  Future<List<CloudFolderChoice>> findSyncFolders() async => [
    (id: 'sync', path: 'My Drive/compter-sync'),
  ];

  @override
  Future<CloudFolderChoice> createSyncFolder() async =>
      (id: 'sync', path: 'My Drive/compter-sync');

  @override
  CloudFolder folder(String id) => drive;
}
