import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../forms/invoice_forms.dart';
import '../forms/project_forms.dart';
import '../notice.dart';
import '../router.dart';
import '../rust/api/rows.dart';
import '../ui/action_sheet.dart';
import '../ui/figures.dart';
import '../ui/record_tile.dart';
import '../ui/section.dart';
import '../ui/status_chip.dart';
import '../ui/theme.dart';

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
              label: const Text('Add milestone'),
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
    final action = await showActionSheet<_Action>(
      context,
      title: milestone.name,
      subtitle: milestone.amount,
      actions: [
        if (pending)
          (
            icon: Icons.receipt_long,
            label: 'Issue invoice',
            value: _Action.issue,
          ),
        if (pending && project.hourly)
          (
            icon: Icons.timer_outlined,
            label: 'Log time',
            value: _Action.logTime,
          ),
        if (invoice != null)
          (
            icon: Icons.open_in_new,
            label: 'Open $invoice',
            value: _Action.openInvoice,
          ),
      ],
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
    final theme = Theme.of(context);
    final credited = project.credited;
    return ListView(
      padding: const EdgeInsets.only(bottom: 96),
      children: [
        ListTile(
          title: Text(project.clientName, style: theme.textTheme.titleMedium),
          subtitle: Text(
            [
              ?project.clientGstin,
              project.billing,
              ?project.clientLocation,
              project.currency,
            ].join(' · '),
          ),
          trailing: StatusChip(project.status),
        ),
        const SizedBox(height: 8),
        FigureGrid([
          ('Total', project.total),
          ('Paid', project.paid),
          if (credited != null) ('Credited', credited),
          ('Outstanding', project.outstanding),
        ]),
        const SectionHeader('Milestones'),
        if (project.milestones.isEmpty)
          const SectionNote('No milestones yet. Add one to invoice it.'),
        for (final m in project.milestones)
          RecordTile(
            title: m.name,
            supporting: [
              'Due ${m.dueDate}',
              ?m.logged,
              ?m.invoiceNumber,
            ].join(' · '),
            amount: m.amount,
            status: m.status,
            onTap: () => onMilestone(m),
          ),
        const SizedBox(height: gutter),
      ],
    );
  }
}
