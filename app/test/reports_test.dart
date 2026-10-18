import 'package:compter/src/compter.dart';
import 'package:compter/src/year_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The demo business is in FY 2026-27, whatever today is
class _Year2026 extends YearNotifier {
  @override
  int build() => 2026;
}

/// The Reports tab on the demo business against the real Rust core: run
/// `cargo build -p compter-mobile` first, then `flutter test`.
void main() {
  setUpAll(initRust);

  /// What the share sheet was handed, instead of showing it
  final shared = <List<Object?>>[];
  setUp(() {
    shared.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('dev.fluttercommunity.plus/share'),
          (call) async {
            final arguments = call.arguments as Map<Object?, Object?>;
            shared.add(arguments['paths']! as List<Object?>);
            return 'dev.fluttercommunity.plus/share/unavailable';
          },
        );
  });

  Future<void> openReports(WidgetTester tester) async {
    await launchDemo(
      tester,
      overrides: [reportYearProvider.overrideWith(_Year2026.new)],
    );
    await tester.tap(find.byIcon(Icons.insights_outlined));
    await settle(tester);
  }

  testWidgets('a year shows income, the 44ADA check, GST and clients', (
    tester,
  ) async {
    await openReports(tester);

    expect(find.text('FY 2026-27'), findsOneWidget);
    expect(find.text('₹2,50,750.00'), findsOneWidget);
    await tester.scrollUntilVisible(
      find.textContaining('tax audit (44AB)'),
      200,
    );
    expect(find.text('₹1,25,375.00'), findsOneWidget);
    await tester.scrollUntilVisible(find.text('Q2 (Jul-Sep)'), 200);
    expect(find.text('-₹13,680.00'), findsOneWidget);
    await tester.scrollUntilVisible(find.text('Acme Pvt Ltd'), 200);
    expect(find.text('Acme Pvt Ltd'), findsOneWidget);
  });

  testWidgets('a GSTR-1 export is handed to the share sheet as JSON', (
    tester,
  ) async {
    await openReports(tester);
    await tester.tap(find.text('Export return'));
    await settle(tester);
    await tester.tap(find.text('GSTR-1 for a quarter'));
    await settle(tester);
    await tester.tap(find.byType(DropdownButtonFormField<String>).first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Q2 (Jul-Sep)').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Export'));
    await pumpUntil(
      tester,
      find.textContaining('GSTR-1 Q2 (Jul-Sep) 2026-27: '),
    );

    expect(shared, hasLength(1));
    expect(shared.single.single, endsWith('.json'));
  });

  testWidgets('the previous year has its own figures', (tester) async {
    await openReports(tester);
    await tester.tap(find.byTooltip('Previous year'));
    await settle(tester);

    expect(find.text('FY 2025-26'), findsOneWidget);
    await tester.scrollUntilVisible(
      find.textContaining('Within the 44ADA limit'),
      200,
    );
  });
}
