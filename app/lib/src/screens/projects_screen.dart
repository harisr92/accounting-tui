import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../forms/project_forms.dart';
import '../ui/empty_state.dart';
import '../ui/record_tile.dart';

/// Every project that isn't archived, like the terminal's Projects tab
class ProjectsScreen extends ConsumerWidget {
  const ProjectsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final projects = ref.watch(projectsProvider);
    return Scaffold(
      appBar: AppBar(title: Text(demoMode ? 'Projects (demo)' : 'Projects')),
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
              for (final row in rows)
                RecordTile(
                  title: row.name,
                  supporting: row.clientName,
                  status: row.status,
                  onTap: () => context.go('/projects/${row.id}'),
                ),
            ],
          ),
        ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        tooltip: 'New project',
        icon: const Icon(Icons.add),
        label: const Text('New project'),
        onPressed: () async {
          final created = await newProjectForm(
            context,
            ref.read(compterProvider),
          );
          if (created == null || !context.mounted) return;
          ref.invalidate(projectsProvider);
          context.go('/projects/${created.id}');
        },
      ),
    );
  }
}
