import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'screens/expenses_screen.dart';
import 'screens/invoice_screen.dart';
import 'screens/invoices_screen.dart';
import 'screens/project_screen.dart';
import 'screens/projects_screen.dart';
import 'screens/reports_screen.dart';
import 'screens/settings_screen.dart';
import 'sync/sync_controller.dart';

/// Where invoice [number] opens. Numbers hold slashes (`INV/26-27/004`), so they are encoded.
String invoicePath(String number) => '/invoices/${Uri.encodeComponent(number)}';

/// A screen pushed over the tabs. Built as a [MaterialPage] here rather than left to
/// go_router, which can take the app for a plain WidgetsApp and show it with no transition,
/// and so no swipe from the left edge to go back on iOS.
Page<void> _detail(GoRouterState state, Widget screen) =>
    MaterialPage<void>(key: state.pageKey, child: screen);

/// A new router at `/projects`: one per app, so a rebuilt app (or the next test) starts on the
/// list. Each tab keeps its own stack, like the terminal app's tabs keep their selection. A
/// project or invoice opens full screen over the tabs (Material's detail pattern), so its own
/// bottom actions never sit on top of the navigation bar.
GoRouter buildRouter() {
  final root = GlobalKey<NavigatorState>();
  return GoRouter(
    navigatorKey: root,
    initialLocation: '/projects',
    routes: [
      GoRoute(path: '/', redirect: (_, _) => '/projects'),
      StatefulShellRoute.indexedStack(
        builder: (context, state, shell) => _Tabs(shell),
        branches: [
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/projects',
                builder: (context, state) => const ProjectsScreen(),
                routes: [
                  GoRoute(
                    path: ':id',
                    parentNavigatorKey: root,
                    pageBuilder: (context, state) => _detail(
                      state,
                      ProjectScreen(id: state.pathParameters['id']!),
                    ),
                  ),
                ],
              ),
            ],
          ),
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/invoices',
                builder: (context, state) => const InvoicesScreen(),
                routes: [
                  GoRoute(
                    path: ':number',
                    parentNavigatorKey: root,
                    pageBuilder: (context, state) => _detail(
                      state,
                      InvoiceScreen(number: state.pathParameters['number']!),
                    ),
                  ),
                ],
              ),
            ],
          ),
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/expenses',
                builder: (context, state) => const ExpensesScreen(),
              ),
            ],
          ),
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/reports',
                builder: (context, state) => const ReportsScreen(),
              ),
            ],
          ),
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/settings',
                builder: (context, state) => const SettingsScreen(),
              ),
            ],
          ),
        ],
      ),
    ],
  );
}

/// The tab bar, iOS style: small labels under the icons, the current tab in the theme's colour,
/// a hairline above. It also starts automatic sync once the app is up, and badges Settings when
/// sync needs the user (sign in, choose a folder, settle a conflict).
class _Tabs extends ConsumerStatefulWidget {
  const _Tabs(this.shell);

  final StatefulNavigationShell shell;

  @override
  ConsumerState<_Tabs> createState() => _TabsState();
}

class _TabsState extends ConsumerState<_Tabs> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => ref.read(syncControllerProvider.notifier).start(),
    );
  }

  @override
  Widget build(BuildContext context) {
    final shell = widget.shell;
    final attention = ref.watch(
      syncControllerProvider.select((report) => report.needsAttention),
    );
    final scheme = Theme.of(context).colorScheme;
    return Scaffold(
      body: shell,
      bottomNavigationBar: CupertinoTabBar(
        currentIndex: shell.currentIndex,
        // A second tap on the current tab goes back to its list
        onTap: (i) =>
            shell.goBranch(i, initialLocation: i == shell.currentIndex),
        backgroundColor: scheme.surface,
        activeColor: scheme.primary,
        inactiveColor: scheme.onSurfaceVariant,
        border: Border(
          top: BorderSide(color: scheme.outlineVariant, width: 0.5),
        ),
        items: [
          const BottomNavigationBarItem(
            icon: Icon(Icons.work_outline),
            activeIcon: Icon(Icons.work),
            label: 'Projects',
          ),
          const BottomNavigationBarItem(
            icon: Icon(Icons.receipt_long_outlined),
            activeIcon: Icon(Icons.receipt_long),
            label: 'Invoices',
          ),
          const BottomNavigationBarItem(
            icon: Icon(Icons.account_balance_wallet_outlined),
            activeIcon: Icon(Icons.account_balance_wallet),
            label: 'Expenses',
          ),
          const BottomNavigationBarItem(
            icon: Icon(Icons.insights_outlined),
            activeIcon: Icon(Icons.insights),
            label: 'Reports',
          ),
          BottomNavigationBarItem(
            icon: Badge(
              isLabelVisible: attention,
              child: const Icon(Icons.settings_outlined),
            ),
            activeIcon: Badge(
              isLabelVisible: attention,
              child: const Icon(Icons.settings),
            ),
            label: 'Settings',
          ),
        ],
      ),
    );
  }
}
