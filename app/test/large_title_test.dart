import 'package:compter/src/ui/large_title.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// The bar's title stays hidden over the large title and slides in once it scrolls under
void main() {
  Future<void> pumpScreen(WidgetTester tester) => tester.pumpWidget(
    MaterialApp(
      home: LargeTitleScope(
        child: Scaffold(
          appBar: const TitleBar(title: 'Projects'),
          body: ListView(
            children: [
              const LargeTitle(
                icon: Icons.work,
                title: 'Projects',
                description: 'Clients and milestones',
              ),
              for (var i = 0; i < 50; i++) ListTile(title: Text('Row $i')),
            ],
          ),
        ),
      ),
    ),
  );

  Finder barTitle() =>
      find.descendant(of: find.byType(AppBar), matching: find.text('Projects'));

  testWidgets('the bar shows the title only once the large one is gone', (
    tester,
  ) async {
    await pumpScreen(tester);
    await tester.pumpAndSettle();
    expect(barTitle(), findsNothing);
    expect(find.text('Clients and milestones'), findsOneWidget);

    // A little: the large title is still below the bar
    await tester.drag(find.byType(ListView), const Offset(0, -20));
    await tester.pumpAndSettle();
    expect(barTitle(), findsNothing);

    await tester.drag(find.byType(ListView), const Offset(0, -300));
    await tester.pumpAndSettle();
    expect(barTitle(), findsOneWidget);

    await tester.drag(find.byType(ListView), const Offset(0, 400));
    await tester.pumpAndSettle();
    expect(barTitle(), findsNothing);
  });

  testWidgets('without a scope the bar always shows its title', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(appBar: TitleBar(title: 'Projects')),
      ),
    );
    expect(barTitle(), findsOneWidget);
  });

  for (final (lines, description) in [
    (1, 'Short'),
    (2, 'A description long enough to wrap onto a second line beside the icon'),
  ]) {
    testWidgets(
      'the icon is as tall as the title and $lines-line description',
      (tester) async {
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: ListView(
                children: [
                  LargeTitle(
                    icon: Icons.work,
                    title: 'Projects',
                    description: description,
                  ),
                ],
              ),
            ),
          ),
        );
        final text = find.ancestor(
          of: find.text('Projects'),
          matching: find.byType(Column),
        );
        final icon = find.ancestor(
          of: find.byIcon(Icons.work),
          matching: find.byType(DecoratedBox),
        );
        final textHeight = tester.getSize(text.first).height;
        expect(tester.getSize(icon.first).height, closeTo(textHeight, 0.5));
        expect(
          tester.getTopLeft(icon.first).dy,
          closeTo(tester.getTopLeft(text.first).dy, 0.5),
        );
      },
    );
  }
}
