import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../notice.dart';
import '../sync/sync_controller.dart';

/// Automatic sync through Google Drive: where it stands, the account, and what the user may
/// need to do (sign in, settle a conflict)
class DriveRows extends ConsumerWidget {
  const DriveRows({super.key});

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
            subtitle: const Text('compter keeps its files in a hidden folder'),
            onTap: () => _signIn(context, ref),
          )
        else ...[
          ListTile(
            enabled: phase != SyncPhase.syncing,
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
}
