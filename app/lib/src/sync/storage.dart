import 'package:flutter/material.dart';

import 'cloud.dart';
import 'sync_settings.dart';

/// Where this phone's data meets the computer's: carried by hand, or through a cloud drive's
/// hidden app folder. Settings offers the storages `syncStoragesProvider` lists and saves the
/// chosen one's [method]; the rest of the app works through this interface only.
abstract interface class SyncStorage {
  /// What Settings saves when this storage is chosen
  SyncMethod get method;

  /// The choice, as Settings shows it: `Google Drive, automatic`
  String get label;

  IconData get icon;

  /// What to do on the computer so it syncs with this phone
  String get computerNote;
}

/// A row Settings shows for a cloud storage: its icon, title and subtitle
typedef CloudAction = ({IconData icon, String title, String subtitle});

/// Syncs on its own, through the app's hidden folder in a cloud drive
abstract interface class CloudStorage implements SyncStorage {
  /// The cloud, for messages: `Google Drive`, `iCloud`
  String get name;

  /// Who is signed in, or `null`. Tries the last session without showing anything.
  Future<String?> restore();

  /// Sign in, showing the platform's account picker if it has one; who signed in
  Future<String> signIn();

  Future<void> signOut();

  /// The app's hidden folder, which compter on the computer reads as well
  CloudFolder folder();

  /// Why it doesn't sync until [restore] or [signIn] succeeds
  String get signedOut;

  /// The row that runs [signIn]
  CloudAction get signInAction;

  /// The row that runs [signOut], or `null` when the app has no sign-in of its own to leave
  String? get signOutTitle;
}
