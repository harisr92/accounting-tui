import 'dart:io';

import 'package:compter/src/rust/api/app.dart';
import 'package:compter/src/rust/api/rows.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support.dart';

/// The Clients screens against the real Rust core: opened from Projects; a client's details
/// are corrected from their screen
void main() {
  setUpAll(initRust);

  Future<void> openClients(WidgetTester tester) async {
    await launchDemo(tester);
    await tester.tap(find.byTooltip('Clients'));
    await settle(tester);
  }

  /// Pick [item] from the opened client's change menu
  Future<void> change(WidgetTester tester, String item) async {
    await tester.tap(find.byTooltip('Change client'));
    await settle(tester);
    await tester.tap(find.text(item));
    await settle(tester);
  }

  Future<void> openAcme(WidgetTester tester) async {
    await openClients(tester);
    await tester.tap(find.text('Acme Pvt Ltd'));
    await settle(tester);
  }

  testWidgets(
    'lists each client with their place, projects and what they owe',
    (tester) async {
      await openClients(tester);

      expect(find.text('Acme Pvt Ltd'), findsOneWidget);
      expect(find.text('27AAPFU0939F2ZU · 1 project'), findsOneWidget);
      expect(find.text('₹35,400.00'), findsOneWidget);
      expect(find.text('Karnataka (29) · 1 project'), findsOneWidget);
      expect(find.text('Germany · 1 project · nothing owed'), findsOneWidget);
    },
  );

  testWidgets('a client opens with their projects and invoices', (
    tester,
  ) async {
    await openClients(tester);
    await tester.tap(find.text('Acme Pvt Ltd'));
    await settle(tester);

    expect(find.text('Plot 7, MIDC, Pune 411019'), findsOneWidget);
    expect(find.text('Website Revamp'), findsOneWidget);
    expect(find.text('Projects · 1 project'), findsOneWidget);
    expect(find.text('Invoices · 2'), findsOneWidget);
    expect(find.textContaining('Frontend'), findsOneWidget);

    // An invoice opens over the client, and back returns to it
    await tester.tap(find.textContaining('Frontend'));
    await settle(tester);
    expect(find.text('Payments and credit notes'), findsOneWidget);
    await tester.pageBack();
    await settle(tester);
    expect(find.text('Invoices · 2'), findsOneWidget);
  });

  testWidgets('search narrows the clients', (tester) async {
    await openClients(tester);
    await tester.tap(find.byTooltip('Search'));
    await settle(tester);
    await tester.enterText(find.byType(TextField), 'germany');
    await settle(tester);

    expect(find.text('Initrode GmbH'), findsOneWidget);
    expect(find.text('Acme Pvt Ltd'), findsNothing);
  });

  testWidgets('a client is corrected, with their open projects if asked', (
    tester,
  ) async {
    await openClients(tester);
    await tester.tap(find.text('Acme Pvt Ltd'));
    await settle(tester);
    await change(tester, 'Edit details');
    expect(find.text('Edit Acme Pvt Ltd'), findsOneWidget);

    await tester.enterText(
      find.widgetWithText(TextField, 'Address'),
      'Plot 9, MIDC\nPune 411019',
    );
    await tester.tap(find.text('Save'));
    await pumpUntil(tester, find.text('Update open projects too?'));
    expect(find.text('Website Revamp'), findsWidgets);

    await tester.tap(find.text('Update projects'));
    await pumpUntil(tester, find.text('Plot 9, MIDC, Pune 411019'));
    expect(find.text('Invoices · 2'), findsOneWidget);
  });

  testWidgets('a renamed client keeps the old name as an alias', (
    tester,
  ) async {
    await openAcme(tester);
    await change(tester, 'Rename');
    await tester.enterText(
      find.widgetWithText(TextField, 'Name'),
      'Acme Private Limited',
    );
    await tester.tap(find.text('Save'));
    await pumpUntil(tester, find.text('Rename on open projects too?'));

    await tester.tap(find.text('Update projects'));
    await pumpUntil(tester, find.textContaining('Also known as Acme Pvt Ltd'));
    expect(find.text('Acme Private Limited'), findsWidgets);
    expect(find.text('Invoices · 2'), findsOneWidget);
  });

  testWidgets('a duplicate merges into the client kept, which opens', (
    tester,
  ) async {
    await openClients(tester);
    await tester.tap(find.text('Globex Labs'));
    await settle(tester);
    await change(tester, 'Merge into…');
    expect(find.text('Merge Globex Labs into'), findsOneWidget);
    expect(find.text('New client'), findsNothing);

    await tester.tap(find.text('Acme Pvt Ltd'));
    await pumpUntil(tester, find.text('Merge into Acme Pvt Ltd?'));
    expect(find.text('Clients only'), findsOneWidget);
    await tester.tap(find.text('Update projects'));
    await pumpUntil(tester, find.textContaining('Also known as Globex Labs'));
    expect(find.text('Projects · 2 projects'), findsOneWidget);
  });

  testWidgets('clients with different GSTINs are not merged', (tester) async {
    await openAcme(tester);
    await change(tester, 'Merge into…');
    await tester.tap(find.text('Umbrella Analytics'));
    await pumpUntil(tester, find.textContaining('different GSTIN'));
    expect(find.text('Merge into Umbrella Analytics?'), findsNothing);
  });

  testWidgets('a client with projects offers no removal', (tester) async {
    await openAcme(tester);
    await tester.tap(find.byTooltip('Change client'));
    await settle(tester);
    expect(find.text('Merge into…'), findsOneWidget);
    expect(find.text('Remove client'), findsNothing);
  });

  testWidgets('a client whose last project was deleted is removed', (
    tester,
  ) async {
    final compter = await tester.runAsync(() async {
      final scratch = await Directory.systemTemp.createTemp('compter_test');
      final app = await CompterApp.demo(scratchDir: scratch.path);
      final project = await app.createProject(
        input: const ProjectInput(
          name: 'Brochure',
          clientName: 'Hooli',
          clientAddress: [],
        ),
      );
      await app.deleteProject(id: project.id);
      return app;
    });
    await launchWith(tester, compter!);
    await tester.tap(find.byTooltip('Clients'));
    await settle(tester);
    await tester.tap(find.text('Hooli'));
    await settle(tester);

    await change(tester, 'Remove client');
    expect(find.text('Remove Hooli?'), findsOneWidget);
    await tester.tap(find.text('Remove'));
    await pumpUntil(tester, find.text('Removed Hooli'));
    expect(find.text('Hooli'), findsNothing);
    expect(find.text('Acme Pvt Ltd'), findsOneWidget);
  });
}
