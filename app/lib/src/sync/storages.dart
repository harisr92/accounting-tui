import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'google_drive.dart';
import 'icloud.dart';
import 'manual.dart';
import 'storage.dart';
import 'sync_settings.dart';

/// The storages this phone offers, in Settings' order: the one place that names each, and
/// where a new one is added. Built once, since google_sign_in may be initialized only once.
/// iCloud's container syncs only between Apple devices. Tests override it with fakes.
final syncStoragesProvider = Provider<List<SyncStorage>>(
  (ref) => [
    GoogleDriveStorage(),
    if (Platform.isIOS) ICloudStorage(),
    const ManualStorage(),
  ],
);

/// The storage of the saved method: by hand until the settings load, or when the saved one
/// isn't offered on this phone
final syncStorageProvider = Provider<SyncStorage>((ref) {
  final method = ref.watch(syncSettingsProvider).value?.method;
  return ref
          .watch(syncStoragesProvider)
          .where((storage) => storage.method == method)
          .firstOrNull ??
      const ManualStorage();
});
