import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../forms/invoice_forms.dart';
import '../forms/project_forms.dart';
import '../notice.dart';
import '../router.dart';
import '../rust/api/rows.dart';
import '../stat_tile.dart';
import '../status.dart';

/// One project: its totals and milestones. Tapping a milestone offers what can be done with it.
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
        data: (detail) => _ProjectBody(
          detail,
          onMilestone: (m) => _milestoneActions(context, ref, detail, m),
        ),
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
                if (updated != null) _show(ref, updated);
              },
            ),
    );
  }

  /// Show the project a change returned, and refresh the list, whose status may have changed
  void _show(WidgetRef ref, ProjectDetail saved) {
    ref.read(projectProvider(id).notifier).show(saved);
    ref.invalidate(projectsProvider);
  }

  Future<void> _milestoneActions(
    BuildContext context,
    WidgetRef ref,
    ProjectDetail project,
    MilestoneRow milestone,
  ) async {
    final pending = milestone.status == 'PENDING';
    final invoice = milestone.invoiceNumber;
    final action = await showModalBottomSheet<_Action>(
      context: context,
      showDragHandle: true,
      builder: (context) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              title: Text(milestone.name),
              subtitle: Text(milestone.amount),
            ),
            if (pending)
              ListTile(
                leading: const Icon(Icons.receipt_long),
                title: const Text('Issue invoice'),
                onTap: () => Navigator.pop(context, _Action.issue),
              ),
            if (pending && project.hourly)
              ListTile(
                leading: const Icon(Icons.timer_outlined),
                title: const Text('Log time'),
                onTap: () => Navigator.pop(context, _Action.logTime),
              ),
            if (invoice != null)
              ListTile(
                leading: const Icon(Icons.open_in_new),
                title: Text('Open $invoice'),
                onTap: () => Navigator.pop(context, _Action.openInvoice),
              ),
          ],
        ),
      ),
    );
    if (action == null || !context.mounted) return;
    final app = ref.read(compterProvider);
    switch (action) {
      case _Action.issue:
        final issued = await issueInvoice(context, app, project, milestone);
        if (issued == null || !context.mounted) return;
        _show(ref, issued.project);
        ref.invalidate(invoicesProvider);
        final number = issued.invoice.number;
        final pdfError = issued.pdfError;
        notify(
          context,
          pdfError == null
              ? 'Issued $number'
              : "Issued $number, but its PDF wasn't saved ($pdfError)",
          error: pdfError != null,
        );
        context.go(invoicePath(number));
      case _Action.logTime:
        final updated = await logTime(context, app, project, milestone);
        if (updated != null) _show(ref, updated);
      case _Action.openInvoice:
        context.go(invoicePath(invoice!));
    }
  }
}

enum _Action { issue, logTime, openInvoice }

class _ProjectBody extends StatelessWidget {
  const _ProjectBody(this.project, {required this.onMilestone});

  final ProjectDetail project;
  final void Function(MilestoneRow) onMilestone;

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    final credited = project.credited;
    final gstin = project.clientGstin;
    final location = project.clientLocation;
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
        Text(
          [project.billing, ?location, project.currency].join(' · '),
          style: text.bodyMedium,
        ),
        const SizedBox(height: 16),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            StatTile('Total', project.total),
            StatTile('Paid', project.paid),
            if (credited != null) StatTile('Credited', credited),
            StatTile('Outstanding', project.outstanding),
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
              ['Due ${m.dueDate}', ?m.logged, ?m.invoiceNumber].join(' · '),
            ),
            trailing: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text(m.amount, style: text.titleSmall),
                Text(m.status, style: text.labelSmall),
              ],
            ),
            onTap: () => onMilestone(m),
          ),
      ],
    );
  }
}
