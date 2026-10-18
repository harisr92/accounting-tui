import 'package:go_router/go_router.dart';

import 'screens/project_screen.dart';
import 'screens/projects_screen.dart';

/// A new router at `/`: one per app, so a rebuilt app (or the next test) starts on the list
GoRouter buildRouter() => GoRouter(
  routes: [
    GoRoute(
      path: '/',
      builder: (context, state) => const ProjectsScreen(),
      routes: [
        GoRoute(
          path: 'projects/:id',
          builder: (context, state) =>
              ProjectScreen(id: state.pathParameters['id']!),
        ),
      ],
    ),
  ],
);
