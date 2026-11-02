import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'screens/client_screen.dart';
import 'screens/clients_screen.dart';
import 'screens/expenses_screen.dart';
import 'screens/home_screen.dart';
import 'screens/invoice_screen.dart';
import 'screens/invoices_screen.dart';
import 'screens/project_screen.dart';
import 'screens/projects_screen.dart';
import 'screens/reports_screen.dart';
import 'screens/settings_screen.dart';
import 'screens/welcome_screen.dart';
import 'sync/sync_controller.dart';
import 'ui/glass.dart';

/// Where invoice [number] opens. Numbers hold slashes (`INV/26-27/004`), so they are encoded.
String invoicePath(String number) => '/invoices/${Uri.encodeComponent(number)}';

/// Settings, opened from the gear on Home over the tabs
const settingsPath = '/settings';

/// The Clients list, opened from Projects over the tabs
const clientsPath = '/clients';

/// Where client [id] opens
String clientPath(String id) => '$clientsPath/$id';

/// A screen pushed over the tabs. Built as a [MaterialPage] here rather than left to
/// go_router, which can take the app for a plain WidgetsApp and show it with no transition,
/// and so no swipe from the left edge to go back on iOS.
Page<void> _detail(GoRouterState state, Widget screen) =>
    MaterialPage<void>(key: state.pageKey, child: screen);

/// A new router at `/projects`, or at the welcome pages on a first start ([welcome]): one per
/// app, so a rebuilt app (or the next test) starts on the list. Each tab keeps its own stack,
/// like the terminal app's tabs keep their selection. A project or invoice opens full screen
/// over the tabs (Material's detail pattern), so its own bottom actions never sit on top of
/// the navigation bar.
GoRouter buildRouter({bool welcome = false}) {
  final root = GlobalKey<NavigatorState>();
  return GoRouter(
    navigatorKey: root,
    initialLocation: welcome ? '/welcome' : '/home',
    routes: [
      GoRoute(path: '/', redirect: (_, _) => '/home'),
      GoRoute(
        path: settingsPath,
        parentNavigatorKey: root,
        pageBuilder: (context, state) => _detail(state, const SettingsScreen()),
      ),
      GoRoute(
        path: clientsPath,
        parentNavigatorKey: root,
        pageBuilder: (context, state) => _detail(state, const ClientsScreen()),
        routes: [
          GoRoute(
            path: ':id',
            parentNavigatorKey: root,
            pageBuilder: (context, state) =>
                _detail(state, ClientScreen(id: state.pathParameters['id']!)),
          ),
        ],
      ),
      GoRoute(
        path: '/welcome',
        builder: (context, state) => const WelcomeScreen(),
      ),
      StatefulShellRoute.indexedStack(
        builder: (context, state, shell) => _Tabs(shell),
        branches: [
          StatefulShellBranch(
            routes: [
              GoRoute(
                path: '/home',
                builder: (context, state) => const HomeScreen(),
              ),
            ],
          ),
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
        ],
      ),
    ],
  );
}

/// The tab bar, iOS style: small labels under the icons, the current tab in the theme's colour,
/// a hairline above, on [Glass] with the screens scrolling under it. It also starts automatic
/// sync once the app is up, and badges Settings when sync needs the user (sign in, choose a
/// folder, settle a conflict).
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
    final scheme = Theme.of(context).colorScheme;
    return Scaffold(
      extendBody: true,
      // The body's padding now counts the tab bar; its view padding too, so a screen's floating
      // button, which keeps clear of the view padding, sits above the bar rather than under it
      body: Builder(
        builder: (context) {
          final media = MediaQuery.of(context);
          return MediaQuery(
            data: media.copyWith(
              viewPadding: media.viewPadding.copyWith(
                bottom: media.padding.bottom,
              ),
            ),
            child: shell,
          );
        },
      ),
      // The bar's height is fixed, so its labels grow no more than 130% however large the
      // phone's text: like the system's own tab bars
      bottomNavigationBar: MediaQuery.withClampedTextScaling(
        maxScaleFactor: 1.3,
        child: CupertinoTabBar(
          currentIndex: shell.currentIndex,
          // A second tap on the current tab goes back to its list. A notice belongs to the tab
          // it was raised on, so it goes with it.
          onTap: (i) {
            ScaffoldMessenger.of(context).clearSnackBars();
            shell.goBranch(i, initialLocation: i == shell.currentIndex);
          },
          backgroundColor: Glass.tint(scheme),
          activeColor: scheme.primary,
          // Its 10 pt labels sit over glass: halfway to the foreground keeps them at 4.5:1
          // whatever scrolls under, in every theme
          inactiveColor: Color.lerp(
            scheme.onSurfaceVariant,
            scheme.onSurface,
            0.5,
          )!,
          border: Border(
            top: BorderSide(color: scheme.outlineVariant, width: 0.5),
          ),
          items: const [
            BottomNavigationBarItem(
              icon: Icon(Icons.home_outlined),
              activeIcon: Icon(Icons.home),
              label: 'Home',
            ),
            BottomNavigationBarItem(
              icon: Icon(Icons.work_outline),
              activeIcon: Icon(Icons.work),
              label: 'Projects',
            ),
            BottomNavigationBarItem(
              icon: Icon(Icons.receipt_long_outlined),
              activeIcon: Icon(Icons.receipt_long),
              label: 'Invoices',
            ),
            BottomNavigationBarItem(
              icon: Icon(Icons.account_balance_wallet_outlined),
              activeIcon: Icon(Icons.account_balance_wallet),
              label: 'Expenses',
            ),
            BottomNavigationBarItem(
              icon: Icon(Icons.insights_outlined),
              activeIcon: Icon(Icons.insights),
              label: 'Reports',
            ),
          ],
        ),
      ),
    );
  }
}
