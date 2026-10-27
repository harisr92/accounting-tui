import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// How this phone syncs with the computer
enum SyncMethod {
  /// The files are carried by hand: Get from computer / Send to computer
  manual,

  /// Automatically, through compter's hidden app folder in Google Drive
  googleDrive,
}

/// What this device remembers about syncing. It belongs to the phone, like `device.json` on a
/// computer, so it lives in the phone's preferences, not the database that travels.
typedef SyncSettings = ({SyncMethod method});

const _method = 'sync.method';

/// The folder older builds synced through; Drive's app folder needs no choice
const _oldFolderKeys = ['sync.drive_folder_id', 'sync.drive_folder_path'];

/// The saved settings; [SyncSettingsNotifier.save] changes them
final syncSettingsProvider =
    AsyncNotifierProvider<SyncSettingsNotifier, SyncSettings>(
      SyncSettingsNotifier.new,
    );

class SyncSettingsNotifier extends AsyncNotifier<SyncSettings> {
  @override
  Future<SyncSettings> build() async {
    final prefs = SharedPreferencesAsync();
    final method = await prefs.getString(_method);
    return (
      method: SyncMethod.values.firstWhere(
        (m) => m.name == method,
        orElse: () => SyncMethod.manual,
      ),
    );
  }

  Future<void> save(SyncSettings settings) async {
    final prefs = SharedPreferencesAsync();
    await prefs.setString(_method, settings.method.name);
    for (final key in _oldFolderKeys) {
      await prefs.remove(key);
    }
    state = AsyncData(settings);
  }
}
