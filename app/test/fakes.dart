import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:compter/src/subscription/store.dart';
import 'package:compter/src/sync/cloud.dart';

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

class FakeAccount implements CloudAccount {
  FakeAccount(this.drive);

  final FakeFolder drive;

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
/// month while [eligible]
class FakeStore implements Store {
  FakeStore({this.held = Entitlement.none, this.eligible = true});

  Entitlement held;
  bool eligible;
  final purchased = <String>[];
  final _changes = StreamController<Entitlement>.broadcast();

  @override
  bool get sells => true;

  @override
  Future<Entitlement> entitlement() async => held;

  @override
  Future<List<Plan>> plans() async => [
    Plan(
      id: 'app.compter.pro.monthly',
      price: '₹299.00',
      period: 'month',
      freePeriod: eligible ? 'month' : null,
    ),
    Plan(
      id: 'app.compter.pro.yearly',
      price: '₹2,999.00',
      period: 'year',
      freePeriod: eligible ? 'month' : null,
    ),
  ];

  @override
  Future<Entitlement?> purchase(String planId) async {
    purchased.add(planId);
    held = Entitlement(
      active: true,
      productId: planId,
      expiresAt: DateTime(2026, 11, 30),
      inTrial: eligible,
    );
    eligible = false;
    return held;
  }

  @override
  Future<Entitlement> restore() async => held;

  @override
  Future<void> manage() async {}

  @override
  Stream<Entitlement> get changes => _changes.stream;

  /// A renewal, refund or expiry arriving from StoreKit
  void change(Entitlement entitlement) {
    held = entitlement;
    _changes.add(entitlement);
  }
}
