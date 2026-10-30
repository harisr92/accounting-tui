import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../notice.dart';
import '../ui/section.dart';
import 'entitlement.dart';
import 'paywall.dart';
import 'store.dart';

/// Where the subscription stands, in Settings, with subscribe, restore and manage. Only where
/// compter is sold.
class SubscriptionSection extends ConsumerWidget {
  const SubscriptionSection({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final store = ref.watch(storeProvider);
    if (!store.sells) return const SizedBox.shrink();
    final status = ref.watch(entitlementProvider);
    final entitlement = status.value;

    Future<void> manage() async {
      try {
        await store.manage();
      } on PlatformException catch (error) {
        if (context.mounted) {
          notify(context, error.message ?? error.code, error: true);
        }
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const SectionHeader('Subscription'),
        ListTile(
          leading: Icon(
            entitlement?.active ?? true
                ? Icons.verified_outlined
                : Icons.lock_outline,
          ),
          title: const Text('compter'),
          subtitle: Text(
            entitlement != null
                ? describeEntitlement(entitlement)
                : status.hasError
                ? 'The App Store can’t be reached'
                : '…',
          ),
        ),
        if (entitlement != null && !entitlement.active)
          ListTile(
            leading: const Icon(Icons.star_outline),
            title: const Text('Subscribe'),
            trailing: const Icon(Icons.chevron_right),
            onTap: () => showPaywall(context),
          )
        else
          ListTile(
            leading: const Icon(Icons.manage_accounts_outlined),
            title: const Text('Manage subscription'),
            subtitle: const Text('Change plan or cancel'),
            trailing: const Icon(Icons.open_in_new),
            onTap: manage,
          ),
        ListTile(
          leading: const Icon(Icons.restore),
          title: const Text('Restore purchases'),
          subtitle: const Text('After a reinstall or on a new iPhone'),
          onTap: () => restorePurchases(context),
        ),
      ],
    );
  }
}

/// Over the project list while read-only, so the reason is plain before anything is tapped
class ReadOnlyBanner extends ConsumerWidget {
  const ReadOnlyBanner({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (ref.watch(canWriteProvider)) return const SizedBox.shrink();
    final scheme = Theme.of(context).colorScheme;
    return ListTile(
      tileColor: scheme.secondaryContainer,
      textColor: scheme.onSecondaryContainer,
      iconColor: scheme.onSecondaryContainer,
      leading: const Icon(Icons.lock_outline),
      title: const Text('Read-only'),
      subtitle: const Text('Subscribe to add or change records'),
      trailing: const Icon(Icons.chevron_right),
      onTap: () => showPaywall(context),
    );
  }
}
