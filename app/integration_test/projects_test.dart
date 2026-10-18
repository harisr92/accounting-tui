import 'package:compter/main.dart';
import 'package:compter/src/compter.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:path_provider/path_provider.dart';

/// A smoke test on a simulator or emulator, with the Rust library cargokit built for it:
/// `flutter test integration_test`. Runs on the demo business, in memory, so nothing on the
/// device is touched. The screens are tested in detail on the host, in `test/`.
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => RustLib.init());

  Future<void> launch(WidgetTester tester) async {
    final scratch = await getTemporaryDirectory();
    final compter = await CompterApp.demo(scratchDir: scratch.path);
    await tester.pumpWidget(
      ProviderScope(
        overrides: [compterProvider.overrideWithValue(compter)],
        child: const CompterMobile(),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('lists the demo projects and opens one', (tester) async {
    await launch(tester);
    expect(find.text('Data Pipeline'), findsOneWidget);

    await tester.tap(find.text('Data Pipeline'));
    await tester.pumpAndSettle();

    expect(find.text('Milestones'), findsOneWidget);
    expect(find.text('Outstanding'), findsOneWidget);
  });

  testWidgets('opens an invoice and writes its PDF for the viewer', (
    tester,
  ) async {
    await launch(tester);
    await tester.tap(find.byIcon(Icons.receipt_long_outlined));
    await tester.pumpAndSettle();
    await tester.tap(find.textContaining('Frontend'));
    await tester.pumpAndSettle();
    expect(find.text('PART PAID'), findsOneWidget);

    await tester.tap(find.text('Open PDF'));
    await tester.pump(const Duration(seconds: 2));

    // An error from Rust or the viewer would show in a snack bar
    expect(find.byType(SnackBar), findsNothing);
  });
}
