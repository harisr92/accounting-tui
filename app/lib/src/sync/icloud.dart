import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'cloud.dart';
import 'storage.dart';
import 'sync_settings.dart';

/// The app's hidden iCloud container (ios/Runner/ICloud.swift). compter on a Mac reads the
/// same folder, under ~/Library/Mobile Documents.
const _channel = MethodChannel('compter/icloud');

/// What iOS shows when iCloud Drive is off for compter
const turnOnICloud =
    'Turn on iCloud Drive for compter in Settings, Apple Account, iCloud';

/// iCloud has no sign-in of its own: the phone's Apple Account is used once iCloud Drive is on
class ICloudStorage implements CloudStorage {
  static const _account = 'iCloud Drive';

  @override
  SyncMethod get method => SyncMethod.iCloud;

  @override
  String get name => 'iCloud';

  @override
  String get label => 'iCloud, automatic';

  @override
  IconData get icon => Icons.cloud_outlined;

  @override
  String get computerNote =>
      'On a Mac signed in to the same Apple Account, run `compter icloud on`. '
      'While this phone is open, compter there waits.';

  @override
  String get signedOut => turnOnICloud;

  @override
  CloudAction get signInAction => (
    icon: Icons.refresh,
    title: 'Try again',
    subtitle: 'After turning on iCloud Drive for compter',
  );

  /// iCloud uses the phone's Apple Account; leaving it is choosing another storage
  @override
  String? get signOutTitle => null;

  @override
  Future<String?> restore() async => await _available() ? _account : null;

  @override
  Future<String> signIn() async {
    if (!await _available()) throw const CloudSignedOut(turnOnICloud);
    return _account;
  }

  /// Nothing to forget: leaving iCloud is choosing another storage
  @override
  Future<void> signOut() async {}

  @override
  CloudFolder folder() => const _ICloudFolder();

  Future<bool> _available() async {
    try {
      return await _channel.invokeMethod<bool>('isAvailable') ?? false;
    } on PlatformException {
      return false;
    } on MissingPluginException {
      return false;
    }
  }
}

class _ICloudFolder implements CloudFolder {
  const _ICloudFolder();

  @override
  Future<Uint8List?> read(String name) =>
      _call(() => _channel.invokeMethod<Uint8List>('read', {'name': name}));

  @override
  Future<void> write(String name, Uint8List bytes) => _call(
    () => _channel.invokeMethod<void>('write', {'name': name, 'bytes': bytes}),
  );

  @override
  Future<void> delete(String name) =>
      _call(() => _channel.invokeMethod<void>('delete', {'name': name}));

  /// iCloud Drive turned off is [CloudSignedOut]; anything else, like a download that didn't
  /// finish, is [CloudOffline] and retried
  Future<T> _call<T>(Future<T> Function() request) async {
    try {
      return await request();
    } on PlatformException catch (error) {
      if (error.code == 'signedOut') {
        throw CloudSignedOut(error.message ?? turnOnICloud);
      }
      throw CloudOffline(error.message ?? 'iCloud is unavailable');
    } on MissingPluginException {
      throw const CloudSignedOut(turnOnICloud);
    }
  }
}
