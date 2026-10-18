import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../notice.dart';
import '../sync/cloud.dart';
import '../sync/sync_controller.dart';
import '../ui/action_sheet.dart';

/// Automatic sync through Google Drive: where it stands, the account, the folder, and what
/// the user may need to do (sign in, choose a folder, settle a conflict)
class DriveRows extends ConsumerWidget {
  const DriveRows({super.key, required this.folderPath});

  /// The chosen folder, as `My Drive/compter-sync`
  final String? folderPath;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final report = ref.watch(syncControllerProvider);
    final controller = ref.read(syncControllerProvider.notifier);
    final phase = report.phase;
    final signedIn = report.account != null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ListTile(
          leading: Icon(_icon(phase)),
          title: Text(_status(context, report)),
          subtitle: report.account == null ? null : Text(report.account!),
        ),
        if (phase == SyncPhase.conflict)
          ListTile(
            leading: const Icon(Icons.call_split),
            title: const Text('Choose which data to keep'),
            onTap: () async {
              final keep = await askWhichToKeep(context, report.message);
              if (keep != null) await controller.settle(keepThisPhone: keep);
            },
          ),
        if (!signedIn)
          ListTile(
            leading: const Icon(Icons.login),
            title: const Text('Sign in to Google'),
            subtitle: const Text('To reach the sync folder in your Drive'),
            onTap: () => _signIn(context, ref),
          )
        else ...[
          ListTile(
            leading: const Icon(Icons.folder_outlined),
            title: const Text('Sync folder'),
            subtitle: Text(folderPath ?? 'Not chosen yet'),
            onTap: () => _chooseFolder(context, ref),
          ),
          ListTile(
            enabled: folderPath != null && phase != SyncPhase.syncing,
            leading: const Icon(Icons.sync),
            title: const Text('Sync now'),
            onTap: controller.syncNow,
          ),
          ListTile(
            leading: const Icon(Icons.logout),
            title: const Text('Sign out of Google'),
            onTap: controller.signOut,
          ),
        ],
      ],
    );
  }

  static IconData _icon(SyncPhase phase) => switch (phase) {
    SyncPhase.synced => Icons.cloud_done_outlined,
    SyncPhase.syncing => Icons.cloud_sync_outlined,
    SyncPhase.offline => Icons.cloud_off_outlined,
    SyncPhase.blocked => Icons.computer,
    SyncPhase.conflict || SyncPhase.failed => Icons.error_outline,
    _ => Icons.cloud_outlined,
  };

  static String _status(BuildContext context, SyncReport report) {
    final at = report.at;
    if (report.phase == SyncPhase.synced && at != null) {
      return 'Synced at ${TimeOfDay.fromDateTime(at).format(context)}';
    }
    return report.message;
  }

  Future<void> _signIn(BuildContext context, WidgetRef ref) async {
    try {
      await ref.read(syncControllerProvider.notifier).signIn();
    } on Exception catch (error) {
      if (context.mounted) {
        notify(context, 'Sign-in failed: $error', error: true);
      }
    }
  }

  /// Folders that already hold compter's files, or a new one
  Future<void> _chooseFolder(BuildContext context, WidgetRef ref) async {
    final account = ref.read(cloudAccountProvider);
    final List<CloudFolderChoice> found;
    try {
      found = await account.findSyncFolders();
    } on Exception catch (error) {
      if (context.mounted) notify(context, '$error', error: true);
      return;
    }
    if (!context.mounted) return;
    const create = (id: '', path: '');
    final choice = await showActionSheet<CloudFolderChoice>(
      context,
      title: 'Sync folder',
      subtitle: found.isEmpty
          ? 'No folder in your Drive holds compter’s files yet'
          : 'Folders holding compter’s files',
      actions: [
        for (final folder in found)
          (icon: Icons.folder_outlined, label: folder.path, value: folder),
        (
          icon: Icons.create_new_folder_outlined,
          label: 'New folder: My Drive/compter-sync',
          value: create,
        ),
      ],
    );
    if (choice == null || !context.mounted) return;
    final controller = ref.read(syncControllerProvider.notifier);
    try {
      if (choice == create) {
        final made = await account.createSyncFolder();
        await controller.useFolder(made);
        if (context.mounted) {
          notify(
            context,
            'Made ${made.path}. Point compter on your computer at it.',
          );
        }
      } else {
        await controller.useFolder(choice);
      }
    } on Exception catch (error) {
      if (context.mounted) notify(context, '$error', error: true);
    }
  }
}
