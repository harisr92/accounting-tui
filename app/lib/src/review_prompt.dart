import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'about.dart';
import 'compter.dart';

const _winsKey = 'review.wins';
const _askedKey = 'review.asked_at';

/// Asks for a store review once compter has proved useful: after its third win (an invoice
/// issued, or one paid in full), never inside a form, and at most every [interval]. The store
/// decides whether the dialog shows at all, and it can always be dismissed. Kept on the
/// phone, like the theme.
class ReviewPrompt {
  ReviewPrompt({
    required this.ask,
    this.enabled = !demoMode,
    DateTime Function()? now,
  }) : _now = now ?? DateTime.now;

  static const winsBeforeAsking = 3;
  static const interval = Duration(days: 120);

  /// Shows the store's review dialog
  final Future<void> Function() ask;

  /// Off in demo mode: nothing there is the user's own work
  final bool enabled;
  final DateTime Function() _now;

  /// How many wins this phone has counted
  static Future<int> savedWins() async =>
      await SharedPreferencesAsync().getInt(_winsKey) ?? 0;

  /// Count a win, and ask for a review if it is time. Never throws: a review is never worth
  /// failing what the user just did.
  Future<void> win() async {
    if (!enabled) return;
    try {
      final prefs = SharedPreferencesAsync();
      final wins = await savedWins() + 1;
      await prefs.setInt(_winsKey, wins);
      if (wins < winsBeforeAsking) return;
      final asked = DateTime.tryParse(await prefs.getString(_askedKey) ?? '');
      final now = _now();
      // A clock set back before the last ask doesn't hold off asking until it catches up
      final since = asked == null ? null : now.difference(asked);
      if (since != null && !since.isNegative && since < interval) return;
      await prefs.setString(_askedKey, now.toIso8601String());
      await ask();
    } catch (_) {}
  }
}

/// Play's in-app review, asked for by MainActivity.kt. iOS has no handler yet: the prompt is
/// off there until compter is in the App Store (see [reviewPromptProvider]).
const _review = MethodChannel('app.compter/review');

Future<void> _askStore() => _review.invokeMethod<void>('requestReview');

/// Off where compter isn't listed in the store yet, like rate in About
final reviewPromptProvider = Provider<ReviewPrompt>(
  (ref) => ReviewPrompt(
    ask: _askStore,
    enabled: !demoMode && ref.watch(storefrontProvider).storeUrl != null,
  ),
);
