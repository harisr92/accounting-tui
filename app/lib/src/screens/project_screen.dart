import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../forms/project_forms.dart';
import '../rust/api/rows.dart';
import '../status.dart';

/// One project: its totals and milestones
class ProjectScreen extends ConsumerWidget {
  const ProjectScreen({super.key, required this.id});

  final String id;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final project = ref.watch(projectProvider(id));
    return Scaffold(
      appBar: AppBar(title: Text(project.value?.name ?? 'Project')),
      body: project.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, _) => Center(child: Text('$error')),
        data: (detail) => _ProjectBody(detail),
      ),
      floatingActionButton: project.value == null
          ? null
          : FloatingActionButton.extended(
              icon: const Icon(Icons.add),
              label: const Text('Milestone'),
              onPressed: () async {
                final updated = await addMilestoneForm(
                  context,
                  ref.read(compterProvider),
                  project.value!,
                );
                if (updated == null) return;
                ref.read(projectProvider(id).notifier).show(updated);
                // The list's status may have changed
                ref.invalidate(projectsProvider);
              },
            ),
    );
  }
}

class _ProjectBody extends StatelessWidget {
  const _ProjectBody(this.project);

  final ProjectDetail project;

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    final credited = project.credited;
    final gstin = project.clientGstin;
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 96),
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                gstin == null
                    ? project.clientName
                    : '${project.clientName} · $gstin',
                style: text.titleMedium,
              ),
            ),
            StatusChip(project.status),
          ],
        ),
        Text(project.billing, style: text.bodyMedium),
        const SizedBox(height: 16),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            _Tile('Total', project.total),
            _Tile('Paid', project.paid),
            if (credited != null) _Tile('Credited', credited),
            _Tile('Outstanding', project.outstanding),
          ],
        ),
        const SizedBox(height: 24),
        Text('Milestones', style: text.titleMedium),
        if (project.milestones.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 16),
            child: Text('No milestones yet'),
          ),
        for (final m in project.milestones)
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(m.name),
            subtitle: Text(
              [
                'Due ${m.dueDate}',
                if (m.logged != null) m.logged!,
                if (m.invoiceNumber != null) m.invoiceNumber!,
              ].join(' · '),
            ),
            trailing: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text(m.amount, style: text.titleSmall),
                Text(m.status, style: text.labelSmall),
              ],
            ),
          ),
      ],
    );
  }
}

/// A figure with its label, like `widgets::stat_tiles` in the terminal app
class _Tile extends StatelessWidget {
  const _Tile(this.label, this.value);

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    return Card.filled(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(label, style: text.labelMedium),
            Text(value, style: text.titleMedium),
          ],
        ),
      ),
    );
  }
}
