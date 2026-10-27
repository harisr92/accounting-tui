import 'dart:io';

import 'package:flutter/services.dart';

/// Asks iOS for background time (ios/Runner/AppDelegate.swift): without it the app is
/// suspended seconds after going to the background, often before its last send and the lease's
/// release reach Drive
const _background = MethodChannel('compter/background');

/// Run [work], asking iOS to keep the app running until it finishes (about 30 seconds at most).
/// Elsewhere it just runs: Android lets the work finish, and tests run on the host.
Future<void> withBackgroundTime(Future<void> Function() work) async {
  int? task;
  if (Platform.isIOS) {
    try {
      task = await _background.invokeMethod<int>('begin');
    } on PlatformException {
      task = null;
    } on MissingPluginException {
      task = null;
    }
  }
  try {
    await work();
  } finally {
    if (task != null) {
      try {
        await _background.invokeMethod<void>('end', task);
      } on PlatformException {
        // iOS already ended it when time ran out
      }
    }
  }
}
