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

/// Every project that isn't archived, or only the archived ones, like the terminal's Projects
/// tab
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
    final archivedOnly = ref.watch(archivedOnlyProvider);
    return LargeTitleScope(
      child: FabScrollScope(
        child: Scaffold(
          appBar: searchAppBar(
            title: archivedOnly ? 'Archived' : 'Projects',
            hint: 'Name or client',
            actions: [
              IconButton(
                tooltip: archivedOnly ? 'Show projects' : 'Show archived',
                icon: Icon(
                  archivedOnly ? Icons.inventory_2 : Icons.inventory_2_outlined,
                ),
                onPressed: ref.read(archivedOnlyProvider.notifier).toggle,
              ),
            ],
          ),
          body: projects.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (error, _) => Center(child: Text('$error')),
            data: (rows) {
              final listed = rows
                  .where((row) => row.archived == archivedOnly)
                  .toList();
              return RefreshIndicator(
                onRefresh: () => ref.refresh(projectsProvider.future),
                child: ListView(
                  padding: const EdgeInsets.only(bottom: 96),
                  children: [
                    if (archivedOnly)
                      const LargeTitle(
                        icon: Icons.inventory_2,
                        title: 'Archived',
                        description:
                            'Finished projects put away; restore one to add work',
                      )
                    else
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
                    else if (listed.isEmpty && archivedOnly)
                      const EmptyState(
                        icon: Icons.inventory_2_outlined,
                        message: 'No archived projects.',
                      )
                    else if (listed.isEmpty)
                      const EmptyState(
                        icon: Icons.inventory_2_outlined,
                        message:
                            'Every project is archived. Show archived to see them, or add one.',
                      ),
                    ..._matching(query, listed),
                  ],
                ),
              );
            },
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
              // A new project isn't archived: back on the list, show it
              ref.read(archivedOnlyProvider.notifier).choose(false);
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
          supporting: row.clientName,
          status: row.status,
          onTap: () => context.go('/projects/${row.id}'),
        ),
    ];
  }
}
