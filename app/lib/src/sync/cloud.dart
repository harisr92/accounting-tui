import 'dart:typed_data';

/// One folder in a cloud drive, holding the files compter's sync folder holds
/// (`compter.redb`, `manifest.json`, `lease.json`). The sync engine only reads and writes
/// whole files by name, so tests use an in-memory folder.
abstract interface class CloudFolder {
  /// The file's contents, or `null` when the folder has no file by that name
  Future<Uint8List?> read(String name);

  /// Replace the file's contents in place, creating it if missing; never a second copy
  Future<void> write(String name, Uint8List bytes);

  /// Remove the file, if there is one
  Future<void> delete(String name);
}

/// A folder the user can choose as the sync folder
typedef CloudFolderChoice = ({String id, String path});

/// The account behind the cloud drive: signing in and finding folders. Google Drive is the
/// one there is; tests use a fake.
abstract interface class CloudAccount {
  /// Who is signed in, or `null`. Tries the last session without showing anything.
  Future<String?> restore();

  /// Sign in, showing the platform's account picker; who signed in
  Future<String> signIn();

  Future<void> signOut();

  /// Folders that already hold a `manifest.json`: compter's sync folders
  Future<List<CloudFolderChoice>> findSyncFolders();

  /// A new `compter-sync` folder at the top of the drive
  Future<CloudFolderChoice> createSyncFolder();

  /// The folder with [id], to sync through
  CloudFolder folder(String id);
}

/// The cloud can't be reached: offline, or Google is down. Nothing is lost; sync retries.
class CloudOffline implements Exception {
  const CloudOffline(this.message);

  final String message;

  @override
  String toString() => message;
}

/// The cloud refused the request because sign-in lapsed (Google's testing mode ends it after
/// 7 days); signing in again fixes it
class CloudSignedOut implements Exception {
  const CloudSignedOut();

  @override
  String toString() => 'Signed out of Google; sign in again to sync';
}
