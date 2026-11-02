import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../forms/client_form.dart';
import '../router.dart';
import '../rust/api/clients.dart';
import '../ui/figures.dart';
import '../ui/glass.dart';
import '../ui/large_title.dart';
import '../ui/record_tile.dart';
import '../ui/section.dart';
import '../ui/theme.dart';
import 'clients_screen.dart';

/// One client: what they owe, their address, their projects and every invoice issued to them.
/// Tapping a project or an invoice opens it. The edit button corrects the client's details,
/// and their open projects' copies if asked; issued invoices never change.
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
              IconButton(
                tooltip: 'Edit client',
                icon: const Icon(Icons.edit_outlined),
                onPressed: () async {
                  final saved = await clientForm(
                    context,
                    ref.read(compterProvider),
                    detail,
                  );
                  if (saved == null) return;
                  // Reloads this client and the list; projects may have taken the details
                  ref.invalidate(projectsProvider);
                  ref.invalidate(projectProvider);
                },
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
          details: client.address.isEmpty ? null : client.address.join(', '),
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
