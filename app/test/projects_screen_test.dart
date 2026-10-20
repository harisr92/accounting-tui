import 'dart:io';

import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/api/rows.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The screens against the real Rust core on this machine: run
/// `cargo build -p compter-mobile` first, then `flutter test`.
void main() {
  setUpAll(initRust);

  testWidgets('lists the demo projects and opens one', (tester) async {
    await launchDemo(tester);
    expect(find.text('Data Pipeline'), findsOneWidget);

    await tester.tap(find.text('Data Pipeline'));
    await settle(tester);

    expect(find.text('Milestones · by due date'), findsOneWidget);
    expect(find.text('Outstanding'), findsOneWidget);
    expect(find.textContaining('₹'), findsWidgets);
  });

  testWidgets('on iOS a swipe from the left edge goes back to the list', (
    tester,
  ) async {
    await launchDemo(tester);
    await tester.tap(find.text('Data Pipeline'));
    await settle(tester);
    expect(find.text('Milestones · by due date'), findsOneWidget);

    final size = tester.getSize(find.byType(Scaffold).last);
    await tester.timedDragFrom(
      Offset(5, size.height / 2),
      Offset(size.width * 0.8, 0),
      const Duration(milliseconds: 300),
    );
    await tester.pumpAndSettle();

    expect(find.text('Milestones · by due date'), findsNothing);
    expect(find.byTooltip('New project'), findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.iOS));

  testWidgets('milestones sort by amount and a tap opens the one shown', (
    tester,
  ) async {
    await launchDemo(tester);
    await tester.tap(find.text('Website Revamp'));
    await settle(tester);
    double top(String name) => tester.getTopLeft(find.text(name).last).dy;
    // By due date: Design (1 Sep), Frontend (20 Oct), Launch (15 Nov)
    expect(top('Design'), lessThan(top('Frontend')));

    await tester.tap(find.byTooltip('Sort milestones'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Amount'));
    await tester.pumpAndSettle();
    expect(find.text('Milestones · by amount'), findsOneWidget);
    // Frontend ₹60,000, Design ₹50,000, Launch ₹40,000
    expect(top('Frontend'), lessThan(top('Design')));
    expect(top('Design'), lessThan(top('Launch')));

    await tester.ensureVisible(find.text('Launch'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Launch'));
    await settle(tester);
    expect(find.text('Issue invoice'), findsOneWidget);
    expect(find.text('₹40,000.00'), findsWidgets);
  });

  testWidgets('an archived project is listed on its own until restored', (
    tester,
  ) async {
    await launchDemo(tester);
    // The demo has two API Audits; this one is paid in full
    await tester.tap(find.text('Initrode GmbH'));
    await settle(tester);

    await tester.tap(find.byTooltip('Archive project'));
    await settle(tester);
    expect(find.byTooltip('New project'), findsOneWidget);
    // The list refetches behind its old rows, showing no spinner to wait for
    final audit = find.widgetWithText(ListTile, 'Initrode GmbH');
    for (var i = 0; i < 100 && audit.evaluate().isNotEmpty; i++) {
      await settle(tester);
    }
    expect(audit, findsNothing);

    await tester.tap(find.byTooltip('Show archived'));
    await settle(tester);
    expect(audit, findsOneWidget);
    expect(find.text('Data Pipeline'), findsNothing);

    await tester.tap(audit);
    await settle(tester);
    expect(find.byType(FloatingActionButton), findsNothing);
    await tester.tap(find.byTooltip('Restore project'));
    await settle(tester);
    expect(find.byTooltip('Archive project'), findsOneWidget);

    await tester.pageBack();
    await settle(tester);
    for (var i = 0; i < 100 && audit.evaluate().isNotEmpty; i++) {
      await settle(tester);
    }
    expect(find.text('No archived projects.'), findsOneWidget);
    await tester.tap(find.byTooltip('Show projects'));
    await settle(tester);
    expect(audit, findsOneWidget);
    expect(find.text('Data Pipeline'), findsOneWidget);
  });

  testWidgets('a project with work pending offers no archive', (tester) async {
    await launchDemo(tester);
    await tester.tap(find.text('Website Revamp'));
    await settle(tester);

    expect(find.byTooltip('Archive project'), findsNothing);
  });

  testWidgets('a refused rate stays in the form under its field', (
    tester,
  ) async {
    await launchDemo(tester);
    await tester.tap(find.byTooltip('New project'));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Support');
    await tester.enterText(find.widgetWithText(TextField, 'Client'), 'Acme');
    await tester.enterText(
      find.widgetWithText(TextField, 'Rate / hour'),
      'lots',
    );
    await tester.tap(find.text('Save'));
    await settle(tester);

    // Still open: the sheet's title over the button that opened it
    expect(find.text('New project'), findsNWidgets(2));
    expect(find.textContaining('amount must be a number'), findsOneWidget);
  });

  testWidgets('a new fixed project takes a milestone', (tester) async {
    await launchDemo(tester);
    await tester.tap(find.byTooltip('New project'));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Website');
    await tester.enterText(find.widgetWithText(TextField, 'Client'), 'Acme');
    await tester.tap(find.text('Save'));
    await settle(tester);

    expect(find.textContaining('No milestones yet'), findsOneWidget);
    await tester.tap(find.byType(FloatingActionButton));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Design');
    await pickDate(tester, 'Due date', DateTime(2026, 11, 1));
    await tester.enterText(find.widgetWithText(TextField, 'Amount'), '50,000');
    await tester.tap(find.text('Save'));
    await settle(tester);

    expect(find.text('Design'), findsOneWidget);
    expect(find.text('Due 2026-11-01'), findsOneWidget);
    expect(find.text('₹50,000.00'), findsWidgets);
  });

  testWidgets('a project is renamed from its filled form', (tester) async {
    await newProject(tester, 'Website');

    await tester.tap(find.byTooltip('Edit project'));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(TextField, 'Website'), findsOneWidget);
    await tester.enterText(
      find.widgetWithText(TextField, 'Name'),
      'Website revamp',
    );
    await tester.tap(find.text('Save'));
    await settle(tester);

    expect(find.text('Website revamp'), findsOneWidget);
  });

  testWidgets('an edit keeps every address line', (tester) async {
    final compter = await tester.runAsync(() async {
      final scratch = await Directory.systemTemp.createTemp('compter_test');
      final app = await CompterApp.demo(scratchDir: scratch.path);
      await app.createProject(
        input: const ProjectInput(
          name: 'Website',
          clientName: 'Acme',
          clientAddress: ['Floor 3', 'MG Road', 'Bengaluru 560001'],
        ),
      );
      return app;
    });
    await launchWith(tester, compter!);
    await tester.tap(find.text('Website'));
    await settle(tester);

    await tester.tap(find.byTooltip('Edit project'));
    await tester.pumpAndSettle();
    expect(
      find.widgetWithText(TextField, 'Floor 3\nMG Road\nBengaluru 560001'),
      findsOneWidget,
    );
    await tester.enterText(find.widgetWithText(TextField, 'Name'), 'Web');
    await tester.tap(find.text('Save'));
    await settle(tester);

    final saved = await tester.runAsync(
      () => compter
          .projects(includeArchived: false)
          .then(
            (rows) =>
                compter.project(id: rows.firstWhere((p) => p.name == 'Web').id),
          ),
    );
    expect(saved!.edit.clientAddress, [
      'Floor 3',
      'MG Road',
      'Bengaluru 560001',
    ]);
  });

  testWidgets('a pending milestone is edited and removed', (tester) async {
    await newProject(tester, 'Website');
    await addMilestone(tester, 'Design', amount: '50,000');

    await tester.tap(find.text('Design'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Edit'));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Amount'), '60,000');
    await tester.tap(find.text('Save'));
    await settle(tester);
    expect(find.text('₹60,000.00'), findsWidgets);

    await tester.tap(find.text('Design'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Remove'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Remove'));
    await settle(tester);
    expect(find.textContaining('No milestones yet'), findsOneWidget);
  });

  testWidgets('logged time is undone', (tester) async {
    await newProject(tester, 'Support', rate: '1,500');
    await addMilestone(tester, 'October');

    await tester.tap(find.text('October'));
    await tester.pumpAndSettle();
    expect(find.text('Undo last time'), findsNothing);
    await tester.tap(find.text('Log time'));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextField, 'Time'), '2h');
    await tester.tap(find.text('Save'));
    await settle(tester);
    // On the milestone and in the project's Logged figure
    expect(find.text('Logged'), findsOneWidget);
    expect(find.textContaining('2h 00m'), findsNWidgets(2));

    await tester.tap(find.text('October'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Undo last time'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Undo'));
    await settle(tester);
    expect(find.textContaining('0h 00m'), findsNWidgets(2));
  });

  testWidgets('a project with nothing invoiced is deleted', (tester) async {
    await newProject(tester, 'Website');

    await tester.tap(find.byTooltip('Delete project'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
    await settle(tester);

    expect(find.text('Deleted Website'), findsOneWidget);
    // The list refetches behind its old rows, showing no spinner to wait for
    final website = find.widgetWithText(ListTile, 'Website');
    for (var i = 0; i < 100 && website.evaluate().isNotEmpty; i++) {
      await settle(tester);
    }
    expect(find.text('Data Pipeline'), findsOneWidget);
    expect(website, findsNothing);
  });

  testWidgets('a project with invoices offers no delete', (tester) async {
    await launchDemo(tester);
    await tester.tap(find.text('Data Pipeline'));
    await settle(tester);

    expect(find.byTooltip('Edit project'), findsOneWidget);
    expect(find.byTooltip('Delete project'), findsNothing);
  });

  testWidgets('searching narrows the projects by name or client', (
    tester,
  ) async {
    await launchDemo(tester);
    await tester.tap(find.byTooltip('Search'));
    await tester.pumpAndSettle();

    await tester.enterText(find.byType(TextField), 'globex');
    await tester.pumpAndSettle();
    expect(find.text('Mobile App'), findsOneWidget);
    expect(find.text('Data Pipeline'), findsNothing);

    await tester.enterText(find.byType(TextField), 'nothing like this');
    await tester.pumpAndSettle();
    expect(find.text('Nothing matches “nothing like this”'), findsOneWidget);

    await tester.tap(find.byTooltip('Close search'));
    await tester.pumpAndSettle();
    expect(find.text('Mobile App'), findsOneWidget);
    expect(find.text('Data Pipeline'), findsOneWidget);
  });

  testWidgets('searching narrows the milestones by name or amount', (
    tester,
  ) async {
    await launchDemo(tester);
    await tester.tap(find.text('Website Revamp'));
    await settle(tester);
    await tester.tap(find.byTooltip('Search'));
    await tester.pumpAndSettle();

    await tester.enterText(find.byType(TextField), 'front');
    await tester.pumpAndSettle();
    expect(find.text('Frontend'), findsOneWidget);
    expect(find.text('Launch'), findsNothing);

    // An amount matches as shown or typed without commas
    for (final amount in ['40,000', '40000']) {
      await tester.enterText(find.byType(TextField), amount);
      await tester.pumpAndSettle();
      expect(find.text('Launch'), findsOneWidget, reason: amount);
      expect(find.text('Frontend'), findsNothing, reason: amount);
    }

    await tester.enterText(find.byType(TextField), 'nothing like this');
    await tester.pumpAndSettle();
    expect(find.text('Nothing matches “nothing like this”'), findsOneWidget);

    await tester.tap(find.byTooltip('Close search'));
    await tester.pumpAndSettle();
    expect(find.text('Frontend'), findsOneWidget);
    expect(find.text('Launch'), findsOneWidget);
    expect(find.byTooltip('Sort milestones'), findsOneWidget);
  });
}

/// Launch the demo and create a project called [name], landing on it
Future<void> newProject(
  WidgetTester tester,
  String name, {
  String? rate,
}) async {
  await launchDemo(tester);
  await tester.tap(find.byTooltip('New project'));
  await tester.pumpAndSettle();
  await tester.enterText(find.widgetWithText(TextField, 'Name'), name);
  await tester.enterText(find.widgetWithText(TextField, 'Client'), 'Acme');
  if (rate != null) {
    await tester.enterText(find.widgetWithText(TextField, 'Rate / hour'), rate);
  }
  await tester.tap(find.text('Save'));
  await settle(tester);
}

Future<void> addMilestone(
  WidgetTester tester,
  String name, {
  String? amount,
}) async {
  await tester.tap(find.byType(FloatingActionButton));
  await tester.pumpAndSettle();
  await tester.enterText(find.widgetWithText(TextField, 'Name'), name);
  if (amount != null) {
    await tester.enterText(find.widgetWithText(TextField, 'Amount'), amount);
  }
  await tester.tap(find.text('Save'));
  await settle(tester);
}
