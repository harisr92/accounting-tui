import 'package:compter/src/about.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// Settings' About section: sharing the store link, rating, and feedback that says what it
/// ran on
void main() {
  setUpAll(initRust);

  late List<Uri> opened;
  late List<String> shared;
  var opens = true;

  setUp(() {
    opened = [];
    shared = [];
    opens = true;
  });

  Storefront store(String? url) => Storefront(
    storeUrl: url,
    open: (uri) async {
      opened.add(uri);
      return opens;
    },
    share: (text, _) async => shared.add(text),
  );

  Future<void> openAbout(WidgetTester tester, String? url) async {
    await launchDemo(
      tester,
      overrides: [
        storefrontProvider.overrideWithValue(store(url)),
        appVersionProvider.overrideWith((ref) async => '0.3.0 (7)'),
      ],
    );
    await tester.tap(find.byIcon(Icons.settings_outlined));
    await settle(tester);
  }

  Future<void> tapRow(WidgetTester tester, String text) async {
    await scrollToMiddle(tester, find.text(text));
    await tester.tap(find.text(text));
    await settle(tester);
  }

  testWidgets('share sends the store link and rate opens the listing', (
    tester,
  ) async {
    await openAbout(tester, playStoreUrl);

    await tapRow(tester, 'Share compter');
    await tapRow(tester, 'Rate compter');

    expect(shared.single, contains(playStoreUrl));
    expect(opened.single, Uri.parse(playStoreUrl));
    await scrollToMiddle(tester, find.text('Version'));
    expect(find.text('0.3.0 (7)'), findsOneWidget);
  });

  testWidgets('feedback opens an email carrying the version', (tester) async {
    await openAbout(tester, playStoreUrl);

    await tapRow(tester, 'Send feedback');

    final email = opened.single;
    expect(email.scheme, 'mailto');
    expect(email.path, feedbackEmail);
    expect(email.queryParameters['subject'], 'compter feedback');
    expect(email.queryParameters['body'], contains('compter 0.3.0 (7)'));
  });

  testWidgets('a store not listed yet hides share and rate', (tester) async {
    await openAbout(tester, null);
    await scrollToMiddle(tester, find.text('Send feedback'));

    expect(find.text('Share compter'), findsNothing);
    expect(find.text('Rate compter'), findsNothing);
    expect(find.text('Send feedback'), findsOneWidget);
  });

  testWidgets('a link that will not open says so', (tester) async {
    opens = false;
    await openAbout(tester, playStoreUrl);

    await tapRow(tester, 'Privacy policy');

    expect(opened.single, Uri.parse(privacyUrl));
    expect(find.text('Couldn’t open the link'), findsOneWidget);
  });
}
