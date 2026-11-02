import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../forms/client_form.dart';
import '../notice.dart';
import '../router.dart';
import '../rust/api/clients.dart';
import '../rust/api/error.dart';
import '../subscription/paywall.dart';
import '../ui/figures.dart';
import '../ui/glass.dart';
import '../ui/large_title.dart';
import '../ui/record_tile.dart';
import '../ui/section.dart';
import '../ui/theme.dart';
import 'clients_screen.dart';

/// One client: what they owe, their address, their projects and every invoice issued to them.
/// Tapping a project or an invoice opens it. The menu corrects the client's details, renames
/// them, or merges them into another client, and their open projects too if asked; issued
/// invoices never change, so old names stay on the client as aliases. A client with no projects
/// or invoices left can be removed from the menu too.
class ClientScreen extends ConsumerWidget {
  const ClientScreen({super.key, required this.id});

  final String id;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final client = ref.watch(clientProvider(id));
    return LargeTitleScope(
      child: Scaffold(
        extendBodyBehindAppBar: true,
        appBar: TitleBar(
          title: client.value?.name ?? 'Client',
          actions: [
            if (client.value case final detail?)
              PopupMenuButton<Future<void> Function()>(
                tooltip: 'Change client',
                icon: const Icon(Icons.edit_outlined),
                onSelected: (action) => action(),
                itemBuilder: (_) => [
                  for (final (change, label) in _changes)
                    PopupMenuItem(
                      value: () => _change(context, ref, detail, change),
                      child: Text(label),
                    ),
                  // Only a client with nothing filed under them can go
                  if (detail.projects.isEmpty && detail.invoices.isEmpty)
                    PopupMenuItem(
                      value: () => _remove(context, ref, detail),
                      child: const Text('Remove client'),
                    ),
                ],
              ),
          ],
        ),
        body: client.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Center(child: Text('$error')),
          data: (detail) => _ClientBody(detail),
        ),
      ),
    );
  }
}

enum _Change { edit, rename, merge }

/// The menu's changes, in order, with their labels
const _changes = [
  (_Change.edit, 'Edit details'),
  (_Change.rename, 'Rename'),
  (_Change.merge, 'Merge into…'),
];

Future<void> _change(
  BuildContext context,
  WidgetRef ref,
  ClientDetail detail,
  _Change change,
) async {
  final app = ref.read(compterProvider);
  final saved = await switch (change) {
    _Change.edit => clientForm(context, app, detail),
    _Change.rename => renameClientForm(context, app, detail),
    _Change.merge => mergeClientForm(context, app, detail),
  };
  if (saved == null) return;
  // Reloads this client and the list; projects may have taken the change, and reports group
  // a renamed or merged client under their name now
  ref.invalidate(projectsProvider);
  ref.invalidate(projectProvider);
  ref.invalidate(reportProvider);
  if (change == _Change.merge && context.mounted) {
    // This client is gone; show the one kept
    context.pushReplacement(clientPath(saved.id));
  }
}

/// Ask, then take [client] out of the address book and go back to the list
Future<void> _remove(
  BuildContext context,
  WidgetRef ref,
  ClientDetail client,
) async {
  if (!await requireWrite(context) || !context.mounted) return;
  final yes = await confirm(
    context,
    title: 'Remove ${client.name}?',
    lines: [
      'They have no projects or invoices. A new project under their name adds them again.',
    ],
    yes: 'Remove',
  );
  if (!yes || !context.mounted) return;
  try {
    await ref.read(compterProvider).removeClient(id: client.id);
    // Before leaving, while this screen's ref is still live; the list reloads without this
    // client, and this screen's own client provider isn't touched
    ref.invalidate(clientsProvider);
    if (!context.mounted) return;
    if (context.canPop()) {
      context.pop();
    } else {
      context.go(clientsPath);
    }
    notify(context, 'Removed ${client.name}');
  } on ApiError catch (error) {
    if (context.mounted) notify(context, error.message, error: true);
  }
}

class _ClientBody extends StatelessWidget {
  const _ClientBody(this.client);

  final ClientDetail client;

  @override
  Widget build(BuildContext context) {
    return ScreenList(
      bottom: gutter,
      children: [
        LargeTitle(
          icon: Icons.person,
          title: client.name,
          description: client.place ?? 'No GSTIN or place recorded',
          details: _details(client),
        ),
        FigureGrid([
          ('Owes', client.owed ?? 'Nothing'),
          ('Projects', '${client.projects.length}'),
        ]),
        SectionHeader('Projects · ${projectCount(client.projects.length)}'),
        for (final project in client.projects)
          RecordTile(
            title: project.name,
            supporting: project.archived ? 'Archived' : null,
            status: project.status,
            onTap: () => context.push('/projects/${project.id}'),
          ),
        SectionHeader('Invoices · ${client.invoices.length}'),
        if (client.invoices.isEmpty) const SectionNote('No invoices yet'),
        for (final invoice in client.invoices)
          RecordTile(
            title: invoice.number,
            supporting: [invoice.date, invoice.milestones].join(' · '),
            amount: invoice.due ?? invoice.total,
            status: invoice.status,
            onTap: () => context.push(invoicePath(invoice.number)),
          ),
      ],
    );
  }
}

/// The address, and the names the client went by before; `null` when there are neither
String? _details(ClientDetail client) {
  final lines = [
    if (client.address.isNotEmpty) client.address.join(', '),
    if (client.aliases.isNotEmpty) 'Also known as ${client.aliases.join(', ')}',
  ];
  return lines.isEmpty ? null : lines.join('\n');
}
