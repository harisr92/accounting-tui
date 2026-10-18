import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import 'screens/invoice_screen.dart';
import 'screens/invoices_screen.dart';
import 'screens/project_screen.dart';
import 'screens/projects_screen.dart';
import 'screens/settings_screen.dart';

/// Where invoice [number] opens. Numbers hold slashes (`INV/26-27/004`), so they are encoded.
String invoicePath(String number) => '/invoices/${Uri.encodeComponent(number)}';

/// A new router at `/projects`: one per app, so a rebuilt app (or the next test) starts on the
/// list. Each tab keeps its own stack, like the terminal app's tabs keep their selection.
GoRouter buildRouter() => GoRouter(
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
                  builder: (context, state) =>
                      ProjectScreen(id: state.pathParameters['id']!),
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
                  builder: (context, state) =>
                      InvoiceScreen(number: state.pathParameters['number']!),
                ),
              ],
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

class _Tabs extends StatelessWidget {
  const _Tabs(this.shell);

  final StatefulNavigationShell shell;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: shell,
      bottomNavigationBar: NavigationBar(
        selectedIndex: shell.currentIndex,
        // A second tap on the current tab goes back to its list
        onDestinationSelected: (i) =>
            shell.goBranch(i, initialLocation: i == shell.currentIndex),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.work_outline),
            selectedIcon: Icon(Icons.work),
            label: 'Projects',
          ),
          NavigationDestination(
            icon: Icon(Icons.receipt_long_outlined),
            selectedIcon: Icon(Icons.receipt_long),
            label: 'Invoices',
          ),
          NavigationDestination(
            icon: Icon(Icons.settings_outlined),
            selectedIcon: Icon(Icons.settings),
            label: 'Settings',
          ),
        ],
      ),
    );
  }
}
