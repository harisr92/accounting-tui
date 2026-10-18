import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// How this phone syncs with the computer
enum SyncMethod {
  /// The files are carried by hand: Get from computer / Send to computer
  manual,

  /// Automatically, through the computer's sync folder in Google Drive
  googleDrive,
}

/// What this device remembers about syncing. It belongs to the phone, like `device.json` on a
/// computer, so it lives in the phone's preferences, not the database that travels.
typedef SyncSettings = ({
  SyncMethod method,
  String? folderId,
  String? folderPath,
});

const _method = 'sync.method';
const _folderId = 'sync.drive_folder_id';
const _folderPath = 'sync.drive_folder_path';

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
      folderId: await prefs.getString(_folderId),
      folderPath: await prefs.getString(_folderPath),
    );
  }

  Future<void> save(SyncSettings settings) async {
    final prefs = SharedPreferencesAsync();
    await prefs.setString(_method, settings.method.name);
    final (:folderId, :folderPath, method: _) = settings;
    if (folderId == null || folderPath == null) {
      await prefs.remove(_folderId);
      await prefs.remove(_folderPath);
    } else {
      await prefs.setString(_folderId, folderId);
      await prefs.setString(_folderPath, folderPath);
    }
    state = AsyncData(settings);
  }
}
