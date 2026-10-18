import 'package:compter/main.dart';
import 'package:compter/src/compter.dart';
import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/frb_generated.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

/// A smoke test on a simulator or emulator, with the Rust library cargokit built for it:
/// `flutter test integration_test`. Runs on the demo business, in memory, so nothing on the
/// device is touched. The screens are tested in detail on the host, in `test/`.
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => RustLib.init());

  Future<void> launch(WidgetTester tester) async {
    final compter = await CompterApp.demo();
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
}
