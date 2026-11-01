import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../router.dart';
import '../rust/api/dashboard.dart';
import '../sync/sync_controller.dart';
import '../ui/empty_state.dart';
import '../ui/figures.dart';
import '../ui/glass.dart';
import '../ui/large_title.dart';
import '../ui/record_tile.dart';
import '../ui/section.dart';
import '../ui/theme.dart';

/// What needs the user today: money owed, invoices overdue, milestones ready to bill. It has no
/// year, unlike Reports, which is for looking back and filing. Settings opens from its gear.
class HomeScreen extends ConsumerWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final dashboard = ref.watch(dashboardProvider);
    return LargeTitleScope(
      child: Scaffold(
        extendBodyBehindAppBar: true,
        appBar: const TitleBar(title: 'Home', actions: [SettingsButton()]),
        body: dashboard.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Center(child: Text('$error')),
          data: (dashboard) => _HomeBody(
            dashboard,
            onRefresh: () => ref.refresh(dashboardProvider.future),
          ),
        ),
      ),
    );
  }
}

/// The gear that opens Settings, badged while sync needs the user (sign in, choose a folder,
/// settle a conflict), as the Settings tab was
class SettingsButton extends ConsumerWidget {
  const SettingsButton({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final attention = ref.watch(
      syncControllerProvider.select((report) => report.needsAttention),
    );
    return IconButton(
      tooltip: attention ? 'Settings: sync needs you' : 'Settings',
      icon: Badge(
        isLabelVisible: attention,
        child: const Icon(Icons.settings_outlined),
      ),
      onPressed: () => context.push(settingsPath),
    );
  }
}

class _HomeBody extends StatelessWidget {
  const _HomeBody(this.dashboard, {required this.onRefresh});

  final DashboardRow dashboard;
  final RefreshCallback onRefresh;

  @override
  Widget build(BuildContext context) {
    final nothing = dashboard.unpaid.isEmpty && dashboard.ready.isEmpty;
    return ScreenList(
      onRefresh: onRefresh,
      bottom: gutter,
      children: [
        const LargeTitle(
          icon: Icons.home,
          title: 'Home',
          description: 'What clients owe you, and what is ready to bill',
        ),
        FigureGrid([for (final f in dashboard.figures) (f.label, f.value)]),
        if (nothing)
          const EmptyState(
            icon: Icons.task_alt,
            message:
                'Nothing waiting. Every invoice is paid, and no milestone is '
                'due to bill yet.',
          ),
        if (dashboard.unpaid.isNotEmpty) ...[
          const SectionHeader('Clients owe you · oldest first'),
          for (final row in dashboard.unpaid)
            RecordTile(
              title: row.clientName,
              supporting: [
                row.number,
                row.milestones,
                _waited(row.days),
              ].join(' · '),
              amount: row.due,
              status: row.status,
              onTap: () => context.push(invoicePath(row.number)),
            ),
        ],
        if (dashboard.ready.isNotEmpty) ...[
          const SectionHeader('Ready to invoice'),
          for (final row in dashboard.ready)
            RecordTile(
              title: row.projectName,
              supporting: [
                row.milestoneName,
                row.clientName,
                'due ${row.dueDate}',
              ].join(' · '),
              amount: row.amount,
              onTap: () => context.push('/projects/${row.projectId}'),
            ),
        ],
      ],
    );
  }
}

String _waited(int days) => switch (days) {
  0 => 'today',
  1 => '1 day',
  _ => '$days days',
};
