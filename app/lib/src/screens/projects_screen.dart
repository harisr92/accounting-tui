import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../forms/project_forms.dart';
import '../rust/api/rows.dart';
import '../ui/collapsing_fab.dart';
import '../ui/empty_state.dart';
import '../ui/large_title.dart';
import '../ui/record_tile.dart';
import '../ui/search.dart';

/// Every project that isn't archived (or every one, while archived are shown), like the
/// terminal's Projects tab
class ProjectsScreen extends ConsumerStatefulWidget {
  const ProjectsScreen({super.key});

  @override
  ConsumerState<ProjectsScreen> createState() => _ProjectsScreenState();
}

class _ProjectsScreenState extends ConsumerState<ProjectsScreen>
    with Searching {
  @override
  Widget build(BuildContext context) {
    final projects = ref.watch(projectsProvider);
    final showArchived = ref.watch(showArchivedProvider);
    return LargeTitleScope(
      child: FabScrollScope(
        child: Scaffold(
          appBar: searchAppBar(
            title: 'Projects',
            hint: 'Name or client',
            actions: [
              IconButton(
                tooltip: showArchived ? 'Hide archived' : 'Show archived',
                icon: Icon(
                  showArchived ? Icons.inventory_2 : Icons.inventory_2_outlined,
                ),
                onPressed: ref.read(showArchivedProvider.notifier).toggle,
              ),
            ],
          ),
          body: projects.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (error, _) => Center(child: Text('$error')),
            data: (rows) => RefreshIndicator(
              onRefresh: () => ref.refresh(projectsProvider.future),
              child: ListView(
                padding: const EdgeInsets.only(bottom: 96),
                children: [
                  const LargeTitle(
                    icon: Icons.work,
                    title: 'Projects',
                    description: demoMode
                        ? 'The demo business: nothing here is saved'
                        : 'Clients, milestones and the time you bill',
                  ),
                  if (rows.isEmpty)
                    const EmptyState(
                      icon: Icons.work_outline,
                      message: 'No projects yet. Add one to start billing.',
                    )
                  else if (!showArchived && rows.every((row) => row.archived))
                    const EmptyState(
                      icon: Icons.inventory_2_outlined,
                      message:
                          'Every project is archived. Show archived to see them, or add one.',
                    ),
                  ..._matching(
                    query,
                    showArchived
                        ? rows
                        : rows.where((row) => !row.archived).toList(),
                  ),
                ],
              ),
            ),
          ),
          floatingActionButton: CollapsingFab(
            // Every tab stays mounted in one route, so their buttons would share the default hero tag
            heroTag: null,
            icon: Icons.add,
            label: 'New project',
            onPressed: () async {
              final created = await projectForm(
                context,
                ref.read(compterProvider),
              );
              if (created == null || !context.mounted) return;
              ref.invalidate(projectsProvider);
              context.go('/projects/${created.id}');
            },
          ),
        ),
      ),
    );
  }

  /// A tile for each project that matches the search
  List<Widget> _matching(SearchQuery query, List<ProjectRow> rows) {
    final matches = query.apply(rows, (row) => [row.name, row.clientName]);
    if (rows.isNotEmpty && matches.isEmpty) return [noMatches(query)];
    return [
      for (final row in matches)
        RecordTile(
          title: row.name,
          supporting: row.archived
              ? '${row.clientName} · Archived'
              : row.clientName,
          status: row.status,
          onTap: () => context.go('/projects/${row.id}'),
        ),
    ];
  }
}
