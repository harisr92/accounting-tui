import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../about.dart';
import '../notice.dart';
import '../ui/theme.dart';
import 'entitlement.dart';
import 'store.dart';

/// The one gate before anything that creates or changes a record: `true` when it may go
/// ahead. On an iPhone without a subscription it shows [showPaywall] instead, and goes ahead
/// only if they subscribe there. Exports, PDFs and sync never ask: the records are theirs.
/// Waits briefly for StoreKit's first answer, so the first ask after launch (the welcome
/// pages' offer) sees the real entitlement; a slow or failing store still lets them write.
Future<bool> requireWrite(BuildContext context) async {
  final container = ProviderScope.containerOf(context, listen: false);
  if (!container.read(storeProvider).sells) return true;
  final Entitlement entitlement;
  try {
    entitlement = await container
        .read(entitlementProvider.future)
        .timeout(const Duration(seconds: 3));
  } catch (_) {
    return true;
  }
  if (entitlement.active) return true;
  if (!context.mounted) return false;
  return showPaywall(context);
}

/// The restore running now, which a second tap joins rather than asking for the Apple ID again
Future<Entitlement>? _restoring;

/// Fetch what this Apple ID bought (a reinstall, a new iPhone) and say what was found. `true`
/// when it holds a subscription.
Future<bool> restorePurchases(BuildContext context) async {
  final container = ProviderScope.containerOf(context, listen: false);
  final running = _restoring ??= container.read(storeProvider).restore();
  try {
    final restored = await running;
    container.read(entitlementProvider.notifier).show(restored);
    if (context.mounted) {
      notify(
        context,
        restored.active
            ? 'Subscription restored'
            : 'No subscription found for this Apple ID',
      );
    }
    return restored.active;
  } on PlatformException catch (error) {
    if (context.mounted) {
      notify(context, error.message ?? error.code, error: true);
    }
    return false;
  } finally {
    if (identical(_restoring, running)) _restoring = null;
  }
}

/// The plans, each with its price and any free period, plus restore. `true` when it closes
/// with a subscription.
Future<bool> showPaywall(BuildContext context) async {
  final subscribed = await showModalBottomSheet<bool>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    useRootNavigator: true,
    builder: (_) => const Paywall(),
  );
  return subscribed ?? false;
}

class Paywall extends ConsumerStatefulWidget {
  const Paywall({super.key});

  @override
  ConsumerState<Paywall> createState() => _PaywallState();
}

class _PaywallState extends ConsumerState<Paywall> {
  late final Future<List<Plan>> _plans = ref.read(storeProvider).plans();

  /// While a purchase or restore is with the App Store, which shows its own sheet
  var _busy = false;

  Future<void> _buy(Plan plan) async {
    setState(() => _busy = true);
    try {
      final purchase = await ref.read(storeProvider).purchase(plan.id);
      if (!mounted) return;
      final bought = purchase.entitlement;
      if (bought != null) {
        ref.read(entitlementProvider.notifier).show(bought);
        if (bought.active) Navigator.pop(context, true);
      } else if (purchase.pending) {
        // The approval comes through the store's changes, whenever the parent answers
        notify(
          context,
          'Asked for approval. compter unlocks once it’s approved.',
        );
        Navigator.pop(context, false);
      }
    } on PlatformException catch (error) {
      if (mounted) notify(context, error.message ?? error.code, error: true);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _restore() async {
    setState(() => _busy = true);
    final restored = await restorePurchases(context);
    if (!mounted) return;
    setState(() => _busy = false);
    if (restored) Navigator.pop(context, true);
  }

  Future<void> _open(String url) async {
    try {
      await ref.read(storefrontProvider).open(Uri.parse(url));
    } catch (_) {}
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SafeArea(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(gutter),
        child: FutureBuilder<List<Plan>>(
          future: _plans,
          builder: (context, snapshot) {
            final plans = snapshot.data ?? const <Plan>[];
            final free = plans.map((p) => p.freePeriod).nonNulls.firstOrNull;
            return Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  free == null
                      ? 'Subscribe to compter'
                      : 'Start your ${freePeriodLabel(free)}',
                  style: theme.textTheme.headlineSmall,
                ),
                const SizedBox(height: 8),
                Text(
                  'Issue invoices, record payments and expenses, and file your returns. '
                  'Without a subscription you can still open, share and export everything '
                  'you already have.',
                  style: theme.textTheme.bodyMedium,
                ),
                const SizedBox(height: sectionGap),
                if (snapshot.connectionState != ConnectionState.done)
                  const Center(child: CircularProgressIndicator())
                else if (plans.isEmpty)
                  const Text(
                    'The App Store can’t be reached. Try again when online.',
                  )
                else
                  for (final plan in plans)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 12),
                      child: FilledButton(
                        onPressed: _busy ? null : () => _buy(plan),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(vertical: 8),
                          child: Text(
                            _offer(plan),
                            textAlign: TextAlign.center,
                          ),
                        ),
                      ),
                    ),
                Text(
                  'Payment is charged to your Apple ID when the free period ends, or at '
                  'purchase without one. The subscription renews automatically unless '
                  'cancelled at least 24 hours before the end of the period, in your '
                  'Apple ID’s subscription settings.',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(height: 8),
                Wrap(
                  alignment: WrapAlignment.center,
                  children: [
                    TextButton(
                      onPressed: _busy ? null : _restore,
                      child: const Text('Restore purchases'),
                    ),
                    TextButton(
                      onPressed: () => _open(termsUrl),
                      child: const Text('Terms'),
                    ),
                    TextButton(
                      onPressed: () => _open(privacyUrl),
                      child: const Text('Privacy'),
                    ),
                    TextButton(
                      onPressed: () => Navigator.pop(context, false),
                      child: const Text('Not now'),
                    ),
                  ],
                ),
              ],
            );
          },
        ),
      ),
    );
  }
}

/// `Free month, then ₹299.00 a month`, or the price alone without a free period
String _offer(Plan plan) {
  final price = '${plan.price} a ${plan.period}';
  final free = plan.freePeriod;
  if (free == null) return price;
  return '${_capitalised(freePeriodLabel(free))}, then $price';
}

/// A free [period] as StoreKit's bridge words it: `month` is a `free month`, `2 weeks` are
/// `2 free weeks`
String freePeriodLabel(String period) {
  final words = period.split(' ');
  return words.length == 2 ? '${words[0]} free ${words[1]}' : 'free $period';
}

String _capitalised(String text) =>
    text.isEmpty ? text : text[0].toUpperCase() + text.substring(1);

/// How Settings says where the subscription stands
String describeEntitlement(Entitlement entitlement) {
  if (!entitlement.active) return 'Read-only: subscribe to make changes';
  final ends = entitlement.expiresAt;
  if (ends == null) return 'Active';
  final date = shortDate(ends);
  if (entitlement.inTrial) {
    final free = entitlement.freePeriod;
    final trial = free == null
        ? 'Free trial'
        : _capitalised(freePeriodLabel(free));
    return entitlement.willRenew
        ? '$trial, then billed on $date'
        : '$trial, ends $date';
  }
  return entitlement.willRenew ? 'Renews $date' : 'Ends $date';
}

const _months = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
];

/// `30 Nov 2026`
String shortDate(DateTime date) =>
    '${date.day} ${_months[date.month - 1]} ${date.year}';
