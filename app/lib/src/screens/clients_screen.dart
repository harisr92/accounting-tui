import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../router.dart';
import '../rust/api/clients.dart';
import '../ui/empty_state.dart';
import '../ui/glass.dart';
import '../ui/large_title.dart';
import '../ui/record_tile.dart';
import '../ui/search.dart';
import '../ui/theme.dart';

/// Every saved client: their GSTIN or place, how many projects they have and what they owe,
/// like the terminal's Clients tab. Opened from Projects; read-only.
class ClientsScreen extends ConsumerStatefulWidget {
  const ClientsScreen({super.key});

  @override
  ConsumerState<ClientsScreen> createState() => _ClientsScreenState();
}

class _ClientsScreenState extends ConsumerState<ClientsScreen> with Searching {
  @override
  Widget build(BuildContext context) {
    final clients = ref.watch(clientsProvider);
    return LargeTitleScope(
      child: Scaffold(
        extendBodyBehindAppBar: true,
        appBar: searchAppBar(title: 'Clients', hint: 'Name, GSTIN or place'),
        body: clients.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Center(child: Text('$error')),
          data: (rows) => ScreenList(
            onRefresh: () => ref.refresh(clientsProvider.future),
            bottom: gutter,
            children: [
              const LargeTitle(
                icon: Icons.people,
                title: 'Clients',
                description: 'Who you bill, and what each still owes',
              ),
              if (rows.isEmpty)
                const EmptyState(
                  icon: Icons.people_outline,
                  message:
                      'No clients yet. A client is saved with their first project.',
                )
              else
                ..._matching(query, rows),
            ],
          ),
        ),
      ),
    );
  }

  /// A tile for each client that matches the search
  List<Widget> _matching(SearchQuery query, List<ClientSummaryRow> rows) {
    final matches = query.apply(rows, (row) => [row.name, row.place ?? '']);
    if (matches.isEmpty) return [noMatches(query)];
    return [
      for (final row in matches)
        RecordTile(
          title: row.name,
          supporting: [
            ?row.place,
            projectCount(row.projects),
            if (row.owed == null) 'nothing owed',
          ].join(' · '),
          amount: row.owed,
          onTap: () => context.push(clientPath(row.id)),
        ),
    ];
  }
}

/// `1 project`, `3 projects`
String projectCount(int count) => '$count project${count == 1 ? '' : 's'}';
