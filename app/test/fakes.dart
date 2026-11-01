import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:compter/src/subscription/store.dart';
import 'package:compter/src/sync/cloud.dart';
import 'package:compter/src/sync/manual.dart';
import 'package:compter/src/sync/storage.dart';
import 'package:compter/src/sync/sync_settings.dart';
import 'package:flutter/material.dart';

/// Drive's app folder in memory; [offline] makes every call fail as with no network
class FakeFolder implements CloudFolder {
  final files = <String, Uint8List>{};
  bool offline = false;

  void _check() {
    if (offline) throw const CloudOffline('Offline');
  }

  @override
  Future<Uint8List?> read(String name) async {
    _check();
    return files[name];
  }

  @override
  Future<void> write(String name, Uint8List bytes) async {
    _check();
    files[name] = bytes;
  }

  @override
  Future<void> delete(String name) async {
    _check();
    files.remove(name);
  }

  Map<String, Object?> json(String name) =>
      jsonDecode(utf8.decode(files[name]!)) as Map<String, Object?>;
}

/// Google Drive and by hand, with Drive in memory
List<SyncStorage> fakeStorages(FakeFolder drive) => [
  FakeAccount(drive),
  const ManualStorage(),
];

class FakeAccount implements CloudStorage {
  FakeAccount(this.drive);

  final FakeFolder drive;

  @override
  SyncMethod get method => SyncMethod.googleDrive;

  @override
  String get name => 'Google Drive';

  @override
  String get label => 'Google Drive, automatic';

  @override
  IconData get icon => Icons.cloud_sync_outlined;

  @override
  String get computerNote => 'Run `compter drive sign-in`';

  @override
  String get signedOut => 'Sign in to Google to sync';

  @override
  CloudAction get signInAction =>
      (icon: Icons.login, title: 'Sign in to Google', subtitle: '');

  @override
  String? get signOutTitle => 'Sign out of Google';

  @override
  Future<String?> restore() async => 'nila@example.com';

  @override
  Future<String> signIn() async => 'nila@example.com';

  @override
  Future<void> signOut() async {}

  @override
  CloudFolder folder() => drive;
}

/// The App Store in memory: [held] is what this Apple ID has, and a purchase grants a free
/// month while [eligible], or waits for a parent with [askToBuy]. A restore waits for
/// [holdRestore] when set.
class FakeStore implements Store {
  FakeStore({
    this.held = Entitlement.none,
    this.eligible = true,
    this.askToBuy = false,
  });

  Entitlement held;
  bool eligible;
  bool askToBuy;

  /// The introductory offer's length, as StoreKit's bridge words it
  String freePeriod = 'month';
  Completer<void>? holdRestore;
  var restores = 0;
  final purchased = <String>[];
  final _changes = StreamController<Entitlement>.broadcast();

  @override
  bool get sells => true;

  @override
  Future<Entitlement> entitlement() async => held;

  /// Plans fail as with no network, or StoreKit knowing none of the products
  bool offline = false;

  @override
  Future<List<Plan>> plans() async {
    if (offline) throw Exception('The App Store has none of the plans');
    return _plans();
  }

  List<Plan> _plans() => [
    Plan(
      id: 'app.compter.pro.monthly',
      price: '₹299.00',
      period: 'month',
      freePeriod: eligible ? freePeriod : null,
    ),
    Plan(
      id: 'app.compter.pro.yearly',
      price: '₹2,999.00',
      period: 'year',
      freePeriod: eligible ? freePeriod : null,
    ),
  ];

  @override
  Future<Purchase> purchase(String planId) async {
    purchased.add(planId);
    if (askToBuy) return const Purchase.pending();
    held = Entitlement(
      active: true,
      productId: planId,
      expiresAt: DateTime(2026, 11, 30),
      inTrial: eligible,
      freePeriod: eligible ? freePeriod : null,
    );
    eligible = false;
    return Purchase.bought(held);
  }

  @override
  Future<Entitlement> restore() async {
    restores++;
    await holdRestore?.future;
    return held;
  }

  @override
  Future<void> manage() async {}

  /// What redeeming a code gives; `null` when the sheet is closed without one
  Entitlement? redeems;
  var redeemed = 0;

  @override
  Future<void> redeemCode() async {
    redeemed++;
    if (redeems case final given?) held = given;
  }

  @override
  Stream<Entitlement> get changes => _changes.stream;

  /// A renewal, refund or expiry arriving from StoreKit
  void change(Entitlement entitlement) {
    held = entitlement;
    _changes.add(entitlement);
  }
}
