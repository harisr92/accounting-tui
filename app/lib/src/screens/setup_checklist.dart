import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../onboarding.dart';
import '../rust/api/rows.dart';
import '../sync/sync_settings.dart';
import '../ui/theme.dart';
import 'settings_screen.dart';

/// "Get started" on Projects for a new business: business details, a project, an invoice,
/// and optionally sync. Each step ticks itself from what is saved, never from a flag, so
/// data synced in from the computer counts too. Gone once the first three are done, or when
/// hidden.
class SetupChecklist extends ConsumerWidget {
  const SetupChecklist({
    super.key,
    required this.projects,
    required this.onNewProject,
  });

  /// Every project, archived too, as the list loaded them
  final List<ProjectRow> projects;

  /// What the list's "New project" button does
  final VoidCallback onNewProject;

  /// Whether the card shows, from the same providers it reads. False while any is loading,
  /// so it never flashes on a business that is already set up.
  static bool shows(WidgetRef ref, List<ProjectRow> projects) {
    // Hidden first: a hidden card doesn't load the invoices at all
    if (ref.watch(checklistHiddenProvider).value != false) return false;
    final profile = ref.watch(profileProvider);
    final invoices = ref.watch(invoicesProvider).value;
    if (!profile.hasValue || invoices == null) return false;
    return profile.value == null ||
        projects.isEmpty ||
        invoices.invoices.isEmpty;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final hasProfile = ref.watch(profileProvider).value != null;
    final hasProject = projects.isNotEmpty;
    final hasInvoice =
        ref.watch(invoicesProvider).value?.invoices.isNotEmpty ?? false;
    final drive =
        ref.watch(syncSettingsProvider).value?.method == SyncMethod.googleDrive;
    final done = [hasProfile, hasProject, hasInvoice].where((d) => d).length;

    void openProject() {
      final open = projects.where((p) => !p.archived);
      final project = open.isEmpty ? projects.first : open.first;
      context.go('/projects/${project.id}');
    }

    return Card.outlined(
      margin: const EdgeInsets.fromLTRB(gutter, 8, gutter, 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(gutter, 8, 4, 0),
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('Get started', style: theme.textTheme.titleMedium),
                      Text(
                        '$done of 3 done',
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: 'Hide getting started',
                  icon: const Icon(Icons.close),
                  onPressed: () =>
                      ref.read(checklistHiddenProvider.notifier).hide(),
                ),
              ],
            ),
          ),
          _Step(
            done: hasProfile,
            title: 'Add your business details',
            subtitle: 'Your name, address and GSTIN, as invoices print them',
            onTap: () => editProfile(context, ref),
          ),
          _Step(
            done: hasProject,
            title: 'Add your first project',
            subtitle: 'A client, and the milestones you bill them for',
            onTap: onNewProject,
          ),
          _Step(
            done: hasInvoice,
            title: 'Issue your first invoice',
            subtitle: hasProfile && hasProject
                ? 'Open a project and tap a pending milestone'
                : 'Once your details and a project are in',
            onTap: hasProfile && hasProject ? openProject : null,
          ),
          _Step(
            done: drive,
            title: 'Sync with your computer',
            subtitle: 'Optional: through your own Google Drive',
            onTap: () => context.go('/settings'),
          ),
          const SizedBox(height: 8),
        ],
      ),
    );
  }
}

/// One step: ticked when done, and greyed out until it can be taken
class _Step extends StatelessWidget {
  const _Step({
    required this.done,
    required this.title,
    required this.subtitle,
    required this.onTap,
  });

  final bool done;
  final String title;
  final String subtitle;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return ListTile(
      enabled: !done && onTap != null,
      leading: done
          ? Icon(
              Icons.check_circle,
              color: scheme.primary,
              semanticLabel: 'Done',
            )
          : const Icon(Icons.radio_button_unchecked),
      title: Text(
        title,
        style: done
            ? const TextStyle(decoration: TextDecoration.lineThrough)
            : null,
      ),
      subtitle: Text(subtitle),
      trailing: done || onTap == null ? null : const Icon(Icons.chevron_right),
      onTap: onTap,
    );
  }
}
