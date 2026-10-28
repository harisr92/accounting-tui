import 'package:compter/src/review_prompt.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The store review is asked for after the third win, then not again for 120 days
void main() {
  late int asked;
  late DateTime today;

  setUp(() {
    useMemoryPreferences();
    asked = 0;
    today = DateTime(2026, 11, 1);
  });

  ReviewPrompt prompt({bool enabled = true}) => ReviewPrompt(
    ask: () async => asked++,
    enabled: enabled,
    now: () => today,
  );

  Future<void> wins(int n) async {
    for (var i = 0; i < n; i++) {
      await prompt().win();
    }
  }

  test('asks only once the third win is in', () async {
    await wins(2);
    expect(asked, 0);

    await wins(1);
    expect(asked, 1);
    expect(await ReviewPrompt.savedWins(), 3);
  });

  test('waits 120 days before asking again', () async {
    await wins(3);
    today = today.add(const Duration(days: 119));
    await wins(1);
    expect(asked, 1);

    today = today.add(const Duration(days: 1));
    await wins(1);
    expect(asked, 2);
  });

  test('the demo never counts or asks', () async {
    for (var i = 0; i < 5; i++) {
      await prompt(enabled: false).win();
    }

    expect(asked, 0);
    expect(await ReviewPrompt.savedWins(), 0);
  });

  test('a store that fails to ask is not the user’s problem', () async {
    final failing = ReviewPrompt(
      ask: () => Future.error(StateError('no store')),
      now: () => today,
    );
    for (var i = 0; i < 3; i++) {
      await failing.win();
    }

    expect(await ReviewPrompt.savedWins(), 3);
  });
}
