import 'dart:typed_data';

/// The app's own hidden folder in a cloud drive, holding the files compter's sync folder holds
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

/// The account behind the cloud drive: signing in, and its app folder. Google Drive is the
/// one there is; tests use a fake.
abstract interface class CloudAccount {
  /// Who is signed in, or `null`. Tries the last session without showing anything.
  Future<String?> restore();

  /// Sign in, showing the platform's account picker; who signed in
  Future<String> signIn();

  Future<void> signOut();

  /// The app's hidden folder, which compter on the computer signs in to as well
  CloudFolder folder();
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

/// This build has no OAuth client for Google sign-in (iOS: ios/Flutter/Google.xcconfig).
/// See app/README.md, "Google Drive sync".
class CloudNotConfigured implements Exception {
  const CloudNotConfigured();

  @override
  String toString() => "Google sign-in isn't set up in this build";
}
