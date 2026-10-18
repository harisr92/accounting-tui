import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../handoff.dart';
import '../notice.dart';
import '../rust/api/error.dart';

/// Hands the data between this phone and compter on a computer. The phone can't see the
/// computer's sync folder (Google Drive, iCloud Drive), so the user carries its two files
/// across with the Files app; Rust checks generations so no invoice is ever lost.
class SyncSection extends ConsumerWidget {
  const SyncSection({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final status = ref.watch(syncStatusProvider);
    final text = Theme.of(context).textTheme;
    final available = status.value?.available ?? false;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text('Sync with your computer', style: text.titleMedium),
        const SizedBox(height: 8),
        status.when(
          loading: () => const SizedBox.square(
            dimension: 16,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
          error: (error, _) => Text('$error'),
          data: (status) => Text(status.summary),
        ),
        const SizedBox(height: 4),
        Text(
          'Uses the two files in the sync folder compter on your computer '
          'publishes to: $snapshotFile and $manifestFile.',
          style: text.bodySmall,
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            OutlinedButton.icon(
              icon: const Icon(Icons.download),
              label: const Text('Get from computer'),
              onPressed: available ? () => _receive(context, ref) : null,
            ),
            Builder(
              // Its own context, so the iPad share popover points at the button
              builder: (context) => OutlinedButton.icon(
                icon: const Icon(Icons.upload),
                label: const Text('Send to computer'),
                onPressed: available ? () => _send(context, ref) : null,
              ),
            ),
          ],
        ),
      ],
    );
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
      final keepPhone = await _askWhichToKeep(context, received.message);
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

  /// `true` to keep this phone's data, `false` to take the computer's, `null` to decide later
  Future<bool?> _askWhichToKeep(BuildContext context, String message) {
    return showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Which data to keep?'),
        content: Text(
          '$message\n\nInvoice numbers can’t be merged, so one side wins. '
          "Taking the computer's keeps this phone's database as a backup.",
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Later'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text("Take computer's"),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text("Keep this phone's"),
          ),
        ],
      ),
    );
  }
}
