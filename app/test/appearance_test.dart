import 'package:compter/src/appearance.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// Choosing a theme in Settings redraws the app in it and keeps it for the next start
void main() {
  setUpAll(initRust);

  Color surface(WidgetTester tester) =>
      Theme.of(tester.element(find.byType(Scaffold).last)).colorScheme.surface;

  testWidgets('gruvbox dark is chosen in settings and saved', (tester) async {
    await launchDemo(tester);
    await tester.tap(find.byIcon(Icons.settings_outlined));
    await settle(tester);
    await scrollToMiddle(tester, find.text(AppTheme.system.label));

    await tester.tap(find.text('Theme'));
    await settle(tester);
    await tester.tap(find.text('Gruvbox dark'));
    await settle(tester);

    expect(surface(tester), const Color(0xFF282828));
    expect(find.text('Gruvbox dark'), findsOneWidget);
    expect(await tester.runAsync(savedAppTheme), AppTheme.gruvboxDark);
  });
}
