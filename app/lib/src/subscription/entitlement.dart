import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'store.dart';

/// What this phone has bought, read at start, kept up to date by the store's [Store.changes],
/// and read again whenever the app comes back to the screen (a cancellation made in Settings)
final entitlementProvider =
    AsyncNotifierProvider<EntitlementNotifier, Entitlement>(
      EntitlementNotifier.new,
    );

class EntitlementNotifier extends AsyncNotifier<Entitlement> {
  @override
  Future<Entitlement> build() async {
    final store = ref.watch(storeProvider);
    final changes = store.changes.listen((e) => state = AsyncData(e));
    final resumed = AppLifecycleListener(onResume: refresh);
    ref.onDispose(() {
      changes.cancel();
      resumed.dispose();
    });
    return store.entitlement();
  }

  /// Read it again, keeping what is shown if the store can't answer
  Future<void> refresh() async {
    try {
      state = AsyncData(await ref.read(storeProvider).entitlement());
    } catch (_) {}
  }

  /// Show what a purchase or restore returned
  void show(Entitlement entitlement) => state = AsyncData(entitlement);
}

/// Whether records can be created or changed. Only an iPhone without a subscription is
/// read-only. While StoreKit answers, and if it fails, writes are allowed: a slow or broken
/// store never locks someone who paid out of their books.
final canWriteProvider = Provider<bool>((ref) {
  if (!ref.watch(storeProvider).sells) return true;
  return ref.watch(entitlementProvider).value?.active ?? true;
});
