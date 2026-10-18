import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../forms/project_forms.dart';
import '../status.dart';

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
        data: (rows) => rows.isEmpty
            ? const Center(child: Text('No projects yet: tap + to add one'))
            : RefreshIndicator(
                onRefresh: () => ref.refresh(projectsProvider.future),
                child: ListView.separated(
                  itemCount: rows.length,
                  separatorBuilder: (_, _) => const Divider(height: 1),
                  itemBuilder: (context, i) {
                    final row = rows[i];
                    return ListTile(
                      title: Text(row.name),
                      subtitle: Text(row.clientName),
                      trailing: StatusChip(row.status),
                      onTap: () => context.go('/projects/${row.id}'),
                    );
                  },
                ),
              ),
      ),
      floatingActionButton: FloatingActionButton(
        tooltip: 'New project',
        onPressed: () async {
          final created = await newProjectForm(
            context,
            ref.read(compterProvider),
          );
          if (created == null || !context.mounted) return;
          ref.invalidate(projectsProvider);
          context.go('/projects/${created.id}');
        },
        child: const Icon(Icons.add),
      ),
    );
  }
}
