import 'package:compter/src/ui/collapsing_fab.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// The floating button folds to a round icon while scrolling down and opens again on the way up
void main() {
  Future<void> pumpList(
    WidgetTester tester, {
    VoidCallback? onPressed,
    int rows = 100,
  }) => tester.pumpWidget(
    MaterialApp(
      home: FabScrollScope(
        child: Scaffold(
          body: ListView(
            // Bounces like iOS, so a short list still scrolls and springs back
            physics: const AlwaysScrollableScrollPhysics(
              parent: BouncingScrollPhysics(),
            ),
            children: [
              for (var i = 0; i < rows; i++) ListTile(title: Text('Row $i')),
            ],
          ),
          floatingActionButton: CollapsingFab(
            icon: Icons.add,
            label: 'New project',
            onPressed: onPressed ?? () {},
          ),
        ),
      ),
    ),
  );

  Size fabSize(WidgetTester tester) =>
      tester.getSize(find.byType(FloatingActionButton));

  testWidgets('collapses to a circle on scroll down and extends on scroll up', (
    tester,
  ) async {
    await pumpList(tester);
    expect(fabSize(tester).width, greaterThan(100));

    await tester.drag(find.byType(ListView), const Offset(0, -300));
    await tester.pumpAndSettle();
    expect(fabSize(tester), const Size(56, 56));
    expect(find.byTooltip('New project'), findsOneWidget);

    await tester.drag(find.byType(ListView), const Offset(0, 100));
    await tester.pumpAndSettle();
    expect(fabSize(tester).width, greaterThan(100));
  });

  testWidgets('still taps while collapsed', (tester) async {
    var taps = 0;
    await pumpList(tester, onPressed: () => taps++);
    await tester.drag(find.byType(ListView), const Offset(0, -300));
    await tester.pumpAndSettle();

    await tester.tap(find.byType(FloatingActionButton));
    expect(taps, 1);
  });

  testWidgets(
    'collapses as the drag starts and stays collapsed after a short list bounces back',
    (tester) async {
      await pumpList(tester, rows: 3);
      final gesture = await tester.startGesture(
        tester.getCenter(find.byType(ListView)),
      );
      await gesture.moveBy(const Offset(0, -40));
      await tester.pumpAndSettle();
      expect(fabSize(tester), const Size(56, 56));

      await gesture.up();
      await tester.pumpAndSettle();
      expect(fabSize(tester), const Size(56, 56));
    },
  );
}
