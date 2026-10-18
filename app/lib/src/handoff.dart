import 'dart:io';
import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';

/// The two files compter's sync folder holds; the phone carries exactly these, by these names
const snapshotFile = 'compter.redb';
const manifestFile = 'manifest.json';

/// A file the user picked, read whole: picked files on Android have no path, only contents
typedef PickedFile = ({String name, Uint8List bytes});

/// Opens the system file browser (iCloud Drive, Google Drive, Files) for the snapshot and
/// manifest. A provider, so tests can hand over files without the platform picker.
final pickFilesProvider = Provider<Future<List<PickedFile>> Function()>(
  (ref) => () async {
    final files = await FilePicker.pickFiles(
      dialogTitle: 'Pick $snapshotFile and $manifestFile from your sync folder',
    );
    return [
      for (final file in files)
        (name: file.name, bytes: await file.readAsBytes()),
    ];
  },
);

/// What this phone calls itself in the manifest, and in the computer's messages
String deviceName() => Platform.isIOS ? 'iPhone' : 'Android phone';

/// Why the picked files aren't a snapshot to take, or `null` when they are
String? checkPicked(List<PickedFile> files) {
  final names = files.map((f) => f.name).toSet();
  if (names.contains(snapshotFile) && names.contains(manifestFile)) return null;
  final copies = names.where(
    (name) => name != snapshotFile && name != manifestFile,
  );
  final hint = copies.isEmpty ? '' : ' (not ${copies.join(', ')})';
  return 'Pick both $snapshotFile and $manifestFile from your compter sync '
      'folder$hint';
}

/// Where hand-offs are staged: the cache folder. A provider, so tests can use their own.
final handoffDirProvider = FutureProvider<String>(
  (ref) async => (await getTemporaryDirectory()).path,
);

/// A fresh folder in [base] holding the picked snapshot and manifest, for Rust to take
Future<String> inboxWith(String base, List<PickedFile> files) async {
  final inbox = await _fresh(base, 'sync-inbox');
  for (final file in files) {
    if (file.name == snapshotFile || file.name == manifestFile) {
      await File('${inbox.path}/${file.name}').writeAsBytes(file.bytes);
    }
  }
  return inbox.path;
}

/// A fresh folder in [base] for Rust to write the snapshot and manifest to, before they are
/// shared
Future<String> outboxIn(String base) async =>
    (await _fresh(base, 'sync-outbox')).path;

/// `name` in [base], emptied: nothing from an earlier hand-off is carried again
Future<Directory> _fresh(String base, String name) async {
  final dir = Directory('$base/$name');
  if (await dir.exists()) await dir.delete(recursive: true);
  return dir.create(recursive: true);
}
