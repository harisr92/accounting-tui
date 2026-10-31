import 'package:flutter/material.dart';

import '../handoff.dart';
import 'storage.dart';
import 'sync_settings.dart';

/// The two files carried by hand with the Files app: Get from computer / Send to computer
class ManualStorage implements SyncStorage {
  const ManualStorage();

  @override
  SyncMethod get method => SyncMethod.manual;

  @override
  String get label => 'By hand, with the files';

  @override
  IconData get icon => Icons.folder_copy_outlined;

  @override
  String get computerNote =>
      'compter on your computer publishes $snapshotFile and $manifestFile to '
      'its sync folder on Google Drive or iCloud Drive.';
}
