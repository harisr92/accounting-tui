import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';

/// What this phone's Apple ID has bought. [active] covers the free first month too: Apple
/// starts it as a subscription that costs nothing until it ends.
class Entitlement {
  const Entitlement({
    required this.active,
    this.productId,
    this.expiresAt,
    this.inTrial = false,
    this.willRenew = true,
  });

  /// Where nothing is sold: Android, the demo, and tests on the host
  static const unlimited = Entitlement(active: true);

  static const none = Entitlement(active: false);

  factory Entitlement.fromMap(Map<Object?, Object?> map) {
    final ends = map['expiresAt'] as int?;
    return Entitlement(
      active: map['active'] as bool? ?? false,
      productId: map['productId'] as String?,
      expiresAt: ends == null
          ? null
          : DateTime.fromMillisecondsSinceEpoch(ends).toLocal(),
      inTrial: map['inTrial'] as bool? ?? false,
      willRenew: map['willRenew'] as bool? ?? true,
    );
  }

  final bool active;
  final String? productId;

  /// When the current period ends: the free month's end while [inTrial]
  final DateTime? expiresAt;
  final bool inTrial;

  /// False once cancelled: it stays [active] until [expiresAt], then ends
  final bool willRenew;
}

/// A subscription on sale, its price already in the store's currency (`₹299.00`)
class Plan {
  const Plan({
    required this.id,
    required this.price,
    required this.period,
    this.freePeriod,
  });

  factory Plan.fromMap(Map<Object?, Object?> map) => Plan(
    id: map['id']! as String,
    price: map['price']! as String,
    period: map['period']! as String,
    freePeriod: map['freePeriod'] as String?,
  );

  final String id;
  final String price;

  /// `month` or `year`
  final String period;

  /// `month` while this Apple ID can still have the free first month, otherwise null
  final String? freePeriod;
}

/// What came of tapping a plan
class Purchase {
  const Purchase.bought(Entitlement this.entitlement) : pending = false;

  const Purchase.cancelled() : entitlement = null, pending = false;

  /// Asked a parent's approval (Ask to Buy); the subscription arrives later through
  /// [Store.changes]
  const Purchase.pending() : entitlement = null, pending = true;

  /// What the phone holds after buying; `null` when cancelled or [pending]
  final Entitlement? entitlement;
  final bool pending;
}

/// Where compter is sold. Only the iPhone sells it ([AppStore]); elsewhere it is free
/// ([FreeStore]). Tests replace it.
abstract interface class Store {
  /// Whether writes need a subscription here
  bool get sells;

  Future<Entitlement> entitlement();

  /// Cheapest first; empty when the store can't be reached
  Future<List<Plan>> plans();

  /// Buy [planId], through the App Store's own confirmation sheet
  Future<Purchase> purchase(String planId);

  /// Fetch what this Apple ID bought, for a new phone or a reinstall
  Future<Entitlement> restore();

  /// The system's subscription settings, to change plan or cancel
  Future<void> manage();

  /// Renewals, refunds and purchases made outside the app
  Stream<Entitlement> get changes;
}

class FreeStore implements Store {
  const FreeStore();

  @override
  bool get sells => false;

  @override
  Future<Entitlement> entitlement() async => Entitlement.unlimited;

  @override
  Future<List<Plan>> plans() async => const [];

  @override
  Future<Purchase> purchase(String planId) async =>
      const Purchase.bought(Entitlement.unlimited);

  @override
  Future<Entitlement> restore() async => Entitlement.unlimited;

  @override
  Future<void> manage() async {}

  @override
  Stream<Entitlement> get changes => const Stream.empty();
}

/// StoreKit 2, through ios/Runner/Store.swift
class AppStore implements Store {
  AppStore() {
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'entitlementChanged') {
        _changes.add(Entitlement.fromMap(call.arguments as Map));
      }
    });
  }

  static const _channel = MethodChannel('compter/store');
  final _changes = StreamController<Entitlement>.broadcast();

  @override
  bool get sells => true;

  @override
  Future<Entitlement> entitlement() async => Entitlement.fromMap(
    await _channel.invokeMethod<Map<Object?, Object?>>('entitlement') ?? {},
  );

  @override
  Future<List<Plan>> plans() async {
    final plans = await _channel.invokeListMethod<Map<Object?, Object?>>(
      'plans',
    );
    return [for (final plan in plans ?? const []) Plan.fromMap(plan)];
  }

  @override
  Future<Purchase> purchase(String planId) async {
    final bought = await _channel.invokeMethod<Map<Object?, Object?>>(
      'purchase',
      planId,
    );
    if (bought == null) return const Purchase.cancelled();
    if (bought['pending'] == true) return const Purchase.pending();
    return Purchase.bought(Entitlement.fromMap(bought));
  }

  @override
  Future<Entitlement> restore() async => Entitlement.fromMap(
    await _channel.invokeMethod<Map<Object?, Object?>>('restore') ?? {},
  );

  @override
  Future<void> manage() => _channel.invokeMethod<void>('manage');

  @override
  Stream<Entitlement> get changes => _changes.stream;
}

/// The App Store on an iPhone; free on Android and in the demo, where nothing is the user's
final storeProvider = Provider<Store>(
  (ref) => Platform.isIOS && !demoMode ? AppStore() : const FreeStore(),
);
