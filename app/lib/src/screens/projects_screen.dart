import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../forms/project_forms.dart';
import '../rust/api/rows.dart';
import '../ui/empty_state.dart';
import '../ui/record_tile.dart';
import '../ui/search.dart';

/// Every project that isn't archived, like the terminal's Projects tab
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
    return Scaffold(
      appBar: searchAppBar(
        title: demoMode ? 'Projects (demo)' : 'Projects',
        hint: 'Name or client',
      ),
      body: projects.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, _) => Center(child: Text('$error')),
        data: (rows) => RefreshIndicator(
          onRefresh: () => ref.refresh(projectsProvider.future),
          child: ListView(
            padding: const EdgeInsets.only(bottom: 96),
            children: [
              if (rows.isEmpty)
                const EmptyState(
                  icon: Icons.work_outline,
                  message: 'No projects yet. Add one to start billing.',
                ),
              ..._matching(query, rows),
            ],
          ),
        ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        // Every tab stays mounted in one route, so their buttons would share the default hero tag
        heroTag: null,
        tooltip: 'New project',
        icon: const Icon(Icons.add),
        label: const Text('New project'),
        onPressed: () async {
          final created = await projectForm(context, ref.read(compterProvider));
          if (created == null || !context.mounted) return;
          ref.invalidate(projectsProvider);
          context.go('/projects/${created.id}');
        },
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
