import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../handoff.dart';
import '../rust/api/error.dart';
import 'cloud.dart';
import 'google_drive.dart';
import 'sync_settings.dart';

const leaseFile = 'lease.json';

/// How often, while the app is open, the lease is refreshed and changes are sent. compter on
/// the computer counts a lease older than 5 minutes as stale.
const syncInterval = Duration(minutes: 1);

/// Where automatic sync stands, for Settings and the badge on its tab
enum SyncPhase {
  /// The files are carried by hand
  manual,
  signedOut,
  syncing,
  synced,
  offline,

  /// compter is open on the computer: this phone takes its changes but sends none
  blocked,

  /// Both sides changed, or a send never arrived: the user chooses which to keep
  conflict,
  failed,
}

@immutable
class SyncReport {
  const SyncReport(this.phase, this.message, {this.account, this.at});

  final SyncPhase phase;

  /// What to show: `compter is open on laptop (last seen 14:05)`, `Offline`, ...
  final String message;

  /// The Google account signed in
  final String? account;

  /// When the last sync finished, for "Synced at"
  final DateTime? at;

  /// Settings has something for the user to do
  bool get needsAttention => const {
    SyncPhase.signedOut,
    SyncPhase.conflict,
    SyncPhase.failed,
  }.contains(phase);
}

/// The cloud account; a provider, so tests use a fake
final cloudAccountProvider = Provider<CloudAccount>(
  (ref) => GoogleDriveAccount(),
);

/// Automatic sync with the computer through compter's hidden app folder in Google Drive. The same steps
/// as compter on a computer, with the phone moving the files itself:
/// - on start and on every return to the app: take the computer's newer snapshot, and hold
///   the lease, so compter on the computer waits;
/// - every [syncInterval] while open, and when the app goes to the background: send this
///   phone's changes, refresh or release the lease.
/// Rust checks every snapshot's generation and identity, so no record is ever overwritten;
/// when both sides changed, the user chooses ([settle]).
final syncControllerProvider = NotifierProvider<SyncController, SyncReport>(
  SyncController.new,
);

class SyncController extends Notifier<SyncReport> {
  Timer? _timer;
  AppLifecycleListener? _lifecycle;
  CloudFolder? _folder;
  String? _account;

  /// Where the conflicting snapshot waits for [settle]
  String? _conflictInbox;
  bool _busy = false;

  /// compter is open on the computer: send nothing until it closes
  bool _blocked = false;

  @override
  SyncReport build() {
    ref.onDispose(_stop);
    return const SyncReport(SyncPhase.manual, 'Sync by hand');
  }

  /// Pick up the saved method and, for Google Drive, sign in silently and sync. Called once
  /// the app is up, and after each change in Settings.
  Future<void> start() async {
    final settings = await ref.read(syncSettingsProvider.future);
    _stop();
    if (settings.method == SyncMethod.manual) {
      state = const SyncReport(SyncPhase.manual, 'Sync by hand');
      return;
    }
    try {
      _account = await ref.read(cloudAccountProvider).restore();
    } on Exception {
      _account = null;
    }
    if (_account == null) {
      state = const SyncReport(
        SyncPhase.signedOut,
        'Sign in to Google to sync',
      );
    } else {
      _folder = ref.read(cloudAccountProvider).folder();
      _lifecycle = AppLifecycleListener(
        onResume: () {
          unawaited(syncNow());
          _tick();
        },
        onPause: () {
          _timer?.cancel();
          unawaited(leave());
        },
      );
      _tick();
      await syncNow();
    }
  }

  void _tick() {
    _timer?.cancel();
    _timer = Timer.periodic(syncInterval, (_) => unawaited(syncNow()));
  }

  void _stop() {
    _timer?.cancel();
    _timer = null;
    _lifecycle?.dispose();
    _lifecycle = null;
    _folder = null;
  }

  /// Take the computer's changes, hold the lease and send this phone's
  Future<void> syncNow() async {
    final folder = _folder;
    if (folder == null || _busy) return;
    _busy = true;
    state = SyncReport(
      SyncPhase.syncing,
      'Syncing…',
      account: _account,
      at: state.at,
    );
    try {
      final holder = await _computerHolding(folder);
      _blocked = holder != null;
      if (!await _pull(folder)) return;
      if (holder != null) {
        state = SyncReport(
          SyncPhase.blocked,
          '$holder. Changes here are sent once it closes.',
          account: _account,
          at: state.at,
        );
        return;
      }
      await _writeLease(folder);
      await _sendIfChanged(folder);
      state = SyncReport(
        SyncPhase.synced,
        'Synced',
        account: _account,
        at: DateTime.now(),
      );
    } on Object catch (error) {
      _report(error);
    } finally {
      _busy = false;
    }
  }

  /// Going to the background: a last send, and the lease released, so the computer can open.
  /// iOS gives only seconds here, which is why changes are also sent while the app is open.
  Future<void> leave() async {
    final folder = _folder;
    if (folder == null || _busy) return;
    _busy = true;
    try {
      if (!_blocked && await _pull(folder)) await _sendIfChanged(folder);
      await _releaseLease(folder);
    } on Object catch (error) {
      _report(error);
    } finally {
      _busy = false;
    }
  }

  /// Who holds a fresh lease on the computer's side, or `null`
  Future<String?> _computerHolding(CloudFolder folder) async {
    final lease = await folder.read(leaseFile);
    if (lease == null) return null;
    return ref.read(compterProvider).leaseHolder(leaseJson: utf8.decode(lease));
  }

  /// Take the folder's snapshot when it isn't the one this phone last synced. `false` when
  /// it couldn't be taken (a conflict, or a snapshot still uploading): then nothing may be
  /// sent over it.
  Future<bool> _pull(CloudFolder folder) async {
    final app = ref.read(compterProvider);
    final manifest = await folder.read(manifestFile);
    if (manifest == null) return true;
    final status = await app.syncStatus();
    final remote = (jsonDecode(utf8.decode(manifest)) as Map)['sha256'];
    if (remote != null && remote == status.sha256) return true;
    final snapshot = await folder.read(snapshotFile);
    if (snapshot == null) {
      state = SyncReport(
        SyncPhase.offline,
        "The computer's snapshot is still uploading",
        account: _account,
        at: state.at,
      );
      return false;
    }
    final inbox = await inboxWith(await ref.read(handoffDirProvider.future), [
      (name: snapshotFile, bytes: snapshot),
      (name: manifestFile, bytes: manifest),
    ]);
    final received = await app.receiveSnapshot(folder: inbox);
    reloadEverythingWith(ref.invalidate);
    if (received.conflict) {
      _conflictInbox = inbox;
      state = SyncReport(
        SyncPhase.conflict,
        received.message,
        account: _account,
      );
      return false;
    }
    return true;
  }

  Future<void> _sendIfChanged(CloudFolder folder) async {
    if ((await ref.read(compterProvider).syncStatus()).changed) {
      await _send(folder);
    }
  }

  /// Snapshot first, manifest second, as compter on a computer publishes: a device that sees
  /// the new manifest before the snapshot finds the hashes differ, and waits
  Future<void> _send(CloudFolder folder) async {
    final sent = await ref
        .read(compterProvider)
        .sendSnapshot(
          folder: await outboxIn(await ref.read(handoffDirProvider.future)),
          deviceName: deviceName(),
        );
    ref.invalidate(syncStatusProvider);
    await folder.write(snapshotFile, await File(sent.snapshot).readAsBytes());
    await folder.write(manifestFile, await File(sent.manifest).readAsBytes());
  }

  Future<void> _writeLease(CloudFolder folder) async {
    final lease = await ref
        .read(compterProvider)
        .leaseJson(deviceName: deviceName());
    await folder.write(leaseFile, utf8.encode(lease));
  }

  /// Remove the lease, unless the computer took it meanwhile
  Future<void> _releaseLease(CloudFolder folder) async {
    if (await _computerHolding(folder) == null) await folder.delete(leaseFile);
  }

  /// Settle a conflict: keep this phone's data (and send it now), or take the computer's
  Future<void> settle({required bool keepThisPhone}) async {
    final folder = _folder;
    final inbox = _conflictInbox;
    if (folder == null || inbox == null) return;
    _busy = true;
    try {
      await ref
          .read(compterProvider)
          .settleSnapshot(
            folder: inbox,
            keepThisPhone: keepThisPhone,
            deviceName: deviceName(),
          );
      _conflictInbox = null;
      reloadEverythingWith(ref.invalidate);
      if (keepThisPhone) await _send(folder);
    } on Object catch (error) {
      _report(error);
      return;
    } finally {
      _busy = false;
    }
    await syncNow();
  }

  /// Sign in from a tap, then carry on where the settings say
  Future<void> signIn() async {
    _account = await ref.read(cloudAccountProvider).signIn();
    await start();
  }

  Future<void> signOut() async {
    final folder = _folder;
    if (folder != null) {
      try {
        await _releaseLease(folder);
      } on Object {
        // Offline: the lease goes stale on its own
      }
    }
    _stop();
    await ref.read(cloudAccountProvider).signOut();
    _account = null;
    state = const SyncReport(SyncPhase.signedOut, 'Sign in to Google to sync');
  }

  /// Sync through Google Drive, or by hand
  Future<void> useMethod(SyncMethod method) async {
    if (method == SyncMethod.manual && _folder != null) {
      try {
        await _releaseLease(_folder!);
      } on Object {
        // Offline: the lease goes stale on its own
      }
    }
    await ref.read(syncSettingsProvider.notifier).save((method: method));
    await start();
  }

  void _report(Object error) {
    state = switch (error) {
      CloudOffline(:final message) => SyncReport(
        SyncPhase.offline,
        '$message; will retry',
        account: _account,
        at: state.at,
      ),
      CloudSignedOut() => SyncReport(
        SyncPhase.signedOut,
        error.toString(),
        account: _account,
      ),
      ApiError(:final message) => SyncReport(
        SyncPhase.failed,
        message,
        account: _account,
        at: state.at,
      ),
      _ => SyncReport(
        SyncPhase.failed,
        '$error',
        account: _account,
        at: state.at,
      ),
    };
  }
}
