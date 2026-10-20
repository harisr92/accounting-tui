import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../forms/invoice_forms.dart';
import '../forms/project_forms.dart';
import '../notice.dart';
import '../router.dart';
import '../rust/api/error.dart';
import '../rust/api/rows.dart';
import '../ui/action_sheet.dart';
import '../ui/collapsing_fab.dart';
import '../ui/figures.dart';
import '../ui/large_title.dart';
import '../ui/record_tile.dart';
import '../ui/search.dart';
import '../ui/section.dart';
import '../ui/theme.dart';

/// One project: its totals and milestones. The app bar edits or deletes the project, and
/// searches its milestones by name or amount; tapping a milestone offers what can be done with
/// it.
class ProjectScreen extends ConsumerStatefulWidget {
  const ProjectScreen({super.key, required this.id});

  final String id;

  @override
  ConsumerState<ProjectScreen> createState() => _ProjectScreenState();
}

class _ProjectScreenState extends ConsumerState<ProjectScreen> with Searching {
  String get id => widget.id;

  @override
  Widget build(BuildContext context) {
    final project = ref.watch(projectProvider(id));
    return LargeTitleScope(
      child: FabScrollScope(
        child: Scaffold(
          appBar: searchAppBar(
            title: project.value?.name ?? 'Project',
            hint: 'Milestone name or amount',
            actions: [
              if (project.value case final detail?) ...[
                PopupMenuButton<MilestoneOrder>(
                  tooltip: 'Sort milestones',
                  icon: const Icon(Icons.sort),
                  initialValue: ref.watch(milestoneOrderProvider),
                  onSelected: ref.read(milestoneOrderProvider.notifier).choose,
                  itemBuilder: (_) => [
                    for (final order in MilestoneOrder.values)
                      PopupMenuItem(value: order, child: Text(order.label)),
                  ],
                ),
                IconButton(
                  tooltip: 'Edit project',
                  icon: const Icon(Icons.edit_outlined),
                  onPressed: () async {
                    final updated = await projectForm(
                      context,
                      ref.read(compterProvider),
                      existing: detail,
                    );
                    if (updated != null) _show(ref, updated);
                  },
                ),
                if (detail.deletable)
                  IconButton(
                    tooltip: 'Delete project',
                    icon: const Icon(Icons.delete_outline),
                    onPressed: () => _delete(context, ref, detail),
                  ),
              ],
            ],
          ),
          body: project.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (error, _) => Center(child: Text('$error')),
            data: (detail) => _ProjectBody(
              detail,
              order: ref.watch(milestoneOrderProvider),
              query: query,
              onMilestone: (m) => _milestoneActions(context, ref, detail, m),
            ),
          ),
          floatingActionButton: project.value == null
              ? null
              : CollapsingFab(
                  icon: Icons.add,
                  label: 'Add milestone',
                  onPressed: () async {
                    final updated = await milestoneForm(
                      context,
                      ref.read(compterProvider),
                      project.value!,
                    );
                    if (updated != null) _show(ref, updated);
                  },
                ),
        ),
      ),
    );
  }

  /// Show the project a change returned, and refresh the list, whose status may have changed
  void _show(WidgetRef ref, ProjectDetail saved) {
    ref.read(projectProvider(id).notifier).show(saved);
    ref.invalidate(projectsProvider);
  }

  Future<void> _delete(
    BuildContext context,
    WidgetRef ref,
    ProjectDetail project,
  ) async {
    final yes = await confirm(
      context,
      title: 'Delete ${project.name}?',
      lines: [
        'Its milestones and logged time go too. Nothing in it was invoiced.',
      ],
      yes: 'Delete',
    );
    if (!yes || !context.mounted) return;
    try {
      await ref.read(compterProvider).deleteProject(id: project.id);
      if (!context.mounted) return;
      // Leave first: this screen would reload the project it just deleted
      context.go('/projects');
      ref.invalidate(projectsProvider);
      notify(context, 'Deleted ${project.name}');
    } on ApiError catch (error) {
      if (context.mounted) notify(context, error.message, error: true);
    }
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
        if (milestone.lastTime != null)
          (icon: Icons.undo, label: 'Undo last time', value: _Action.undoTime),
        if (pending)
          (icon: Icons.edit_outlined, label: 'Edit', value: _Action.edit),
        if (pending)
          (icon: Icons.delete_outline, label: 'Remove', value: _Action.remove),
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
      case _Action.undoTime:
        final yes = await confirm(
          context,
          title: 'Undo time on ${milestone.name}?',
          lines: ['Removes ${milestone.lastTime}.'],
          yes: 'Undo',
        );
        if (!yes || !context.mounted) return;
        await _change(
          context,
          ref,
          () => app.undoTime(projectId: project.id, milestoneId: milestone.id),
        );
      case _Action.edit:
        final updated = await milestoneForm(
          context,
          app,
          project,
          existing: milestone,
        );
        if (updated != null) _show(ref, updated);
      case _Action.remove:
        final yes = await confirm(
          context,
          title: 'Remove ${milestone.name}?',
          lines: [
            '${milestone.amount}, due ${milestone.dueDate}. It was never invoiced.',
          ],
          yes: 'Remove',
        );
        if (!yes || !context.mounted) return;
        await _change(
          context,
          ref,
          () => app.removeMilestone(
            projectId: project.id,
            milestoneId: milestone.id,
          ),
        );
      case _Action.openInvoice:
        context.go(invoicePath(invoice!));
    }
  }

  /// Run a change outside a form, showing what it saved or its refusal in the footer
  Future<void> _change(
    BuildContext context,
    WidgetRef ref,
    Future<ProjectDetail> Function() change,
  ) async {
    try {
      final saved = await change();
      _show(ref, saved);
    } on ApiError catch (error) {
      if (context.mounted) notify(context, error.message, error: true);
    }
  }
}

enum _Action { issue, logTime, undoTime, edit, remove, openInvoice }

class _ProjectBody extends StatelessWidget {
  const _ProjectBody(
    this.project, {
    required this.order,
    required this.query,
    required this.onMilestone,
  });

  final ProjectDetail project;
  final MilestoneOrder order;
  final SearchQuery query;
  final void Function(MilestoneRow) onMilestone;

  @override
  Widget build(BuildContext context) {
    final credited = project.credited;
    final logged = project.logged;
    final listed = order.of(project).map((i) => project.milestones[i]).toList();
    // Without commas too, so `50000` finds ₹50,000.00
    final matching = query.apply(
      listed,
      (m) => [m.name, m.amount, m.amount.replaceAll(',', '')],
    );
    return ListView(
      padding: const EdgeInsets.only(bottom: 96),
      children: [
        LargeTitle(
          icon: Icons.work,
          title: project.name,
          description: project.clientName,
          status: project.status,
          details: [
            ?project.clientGstin,
            project.billing,
            ?project.clientLocation,
            project.currency,
          ].join(' · '),
        ),
        FigureGrid([
          ('Total', project.total),
          ('Paid', project.paid),
          if (credited != null) ('Credited', credited),
          ('Outstanding', project.outstanding),
          if (logged != null) ('Logged', logged),
        ]),
        SectionHeader('Milestones · by ${order.label.toLowerCase()}'),
        if (project.milestones.isEmpty)
          const SectionNote('No milestones yet. Add one to invoice it.'),
        if (listed.isNotEmpty && matching.isEmpty) noMatches(query),
        // Each tile carries its own milestone, so a tap acts on the one shown
        for (final m in matching)
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
