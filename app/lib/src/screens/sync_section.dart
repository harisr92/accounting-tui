import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../handoff.dart';
import '../notice.dart';
import '../rust/api/error.dart';
import '../sync/sync_controller.dart';
import '../sync/storage.dart';
import '../sync/storages.dart';
import '../ui/action_sheet.dart';
import '../ui/section.dart';
import 'cloud_rows.dart';

/// Syncing with compter on a computer, through its sync folder: automatically in Google
/// Drive or iCloud (see `SyncController`), or by hand, carrying its two files across with the Files app.
/// Either way Rust checks each snapshot's generation, so no invoice is ever lost.
class SyncSection extends ConsumerWidget {
  const SyncSection({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final status = ref.watch(syncStatusProvider);
    final storage = ref.watch(syncStorageProvider);
    final available = status.value?.available ?? false;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const SectionHeader('Sync with your computer'),
        if (status.isLoading && !status.hasValue)
          const ListTile(
            leading: Icon(Icons.sync),
            title: Align(
              alignment: Alignment.centerLeft,
              child: SizedBox.square(
                dimension: 16,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
            ),
          )
        else if (!available)
          ListTile(
            leading: const Icon(Icons.sync_disabled),
            title: Text(status.value?.summary ?? ''),
          )
        else ...[
          ListTile(
            leading: const Icon(Icons.sync_alt),
            title: const Text('Sync method'),
            subtitle: Text(storage.label),
            onTap: () => _chooseMethod(context, ref),
          ),
          if (storage case CloudStorage cloud)
            CloudRows(cloud)
          else
            ..._manualRows(context, ref, status.value?.summary),
          SectionNote(storage.computerNote),
        ],
      ],
    );
  }

  List<Widget> _manualRows(
    BuildContext context,
    WidgetRef ref,
    String? summary,
  ) => [
    ListTile(
      leading: const Icon(Icons.info_outline),
      title: Text(summary ?? '…'),
    ),
    ListTile(
      leading: const Icon(Icons.download),
      title: const Text('Get from computer'),
      subtitle: const Text('Pick both files from the sync folder'),
      onTap: () => _receive(context, ref),
    ),
    Builder(
      // Its own context, so the iPad share popover points at the row
      builder: (row) => ListTile(
        leading: const Icon(Icons.upload),
        title: const Text('Send to computer'),
        subtitle: const Text('Save both files into the sync folder'),
        onTap: () => _send(row, ref),
      ),
    ),
  ];

  Future<void> _chooseMethod(BuildContext context, WidgetRef ref) async {
    final storage = await showActionSheet<SyncStorage>(
      context,
      title: 'Sync method',
      actions: [
        for (final storage in ref.read(syncStoragesProvider))
          (icon: storage.icon, label: storage.label, value: storage),
      ],
    );
    if (storage == null) return;
    try {
      await ref.read(syncControllerProvider.notifier).use(storage);
    } on Exception catch (error) {
      if (context.mounted) notify(context, '$error', error: true);
    }
  }

  Future<void> _receive(BuildContext context, WidgetRef ref) async {
    final files = await ref.read(pickFilesProvider)();
    if (files.isEmpty || !context.mounted) return;
    final wrong = checkPicked(files);
    if (wrong != null) {
      notify(context, wrong, error: true);
      return;
    }
    final app = ref.read(compterProvider);
    try {
      final inbox = await inboxWith(
        await ref.read(handoffDirProvider.future),
        files,
      );
      final received = await app.receiveSnapshot(folder: inbox);
      reloadEverything(ref);
      if (!context.mounted) return;
      if (!received.conflict) {
        notify(context, received.message);
        return;
      }
      final keepPhone = await askWhichToKeep(context, received.message);
      if (keepPhone == null) return;
      final settled = await app.settleSnapshot(
        folder: inbox,
        keepThisPhone: keepPhone,
        deviceName: deviceName(),
      );
      reloadEverything(ref);
      if (context.mounted) notify(context, settled);
    } on ApiError catch (error) {
      reloadEverything(ref);
      if (context.mounted) notify(context, error.message, error: true);
    }
  }

  Future<void> _send(BuildContext context, WidgetRef ref) async {
    final yes = await confirm(
      context,
      title: 'Send to computer',
      yes: 'Continue',
      lines: const [
        'Save both files into the compter sync folder on Google Drive or '
            'iCloud Drive, replacing the ones there. The Files app asks to '
            'replace them.',
        "If they end up as copies, like 'compter (1).redb', compter on the "
            'computer warns about them, and this phone notices on the next '
            'Get from computer.',
      ],
    );
    if (!yes || !context.mounted) return;
    final app = ref.read(compterProvider);
    try {
      final sent = await app.sendSnapshot(
        folder: await outboxIn(await ref.read(handoffDirProvider.future)),
        deviceName: deviceName(),
      );
      ref.invalidate(syncStatusProvider);
      if (!context.mounted) return;
      notify(context, sent.message);
      await shareFiles(context, [
        (sent.snapshot, 'application/octet-stream'),
        (sent.manifest, 'application/json'),
      ]);
    } on ApiError catch (error) {
      reloadEverything(ref);
      if (context.mounted) notify(context, error.message, error: true);
    }
  }
}
