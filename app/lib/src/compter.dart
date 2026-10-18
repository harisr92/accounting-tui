import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';

import 'rust/api/app.dart';
import 'rust/api/rows.dart';

/// `flutter run --dart-define=COMPTER_DEMO=true` opens the sample business in memory, like
/// `compter --demo`; nothing is saved.
const demoMode = bool.fromEnvironment('COMPTER_DEMO');

/// The database in the app's private support folder, opened once for the life of the app
Future<CompterApp> openCompter() async {
  if (demoMode) {
    return CompterApp.demo();
  }
  final dir = await getApplicationSupportDirectory();
  return CompterApp.open(dataDir: dir.path);
}

/// Set in `main` with the opened app
final compterProvider = Provider<CompterApp>(
  (ref) => throw UnimplementedError('compterProvider is overridden in main'),
);

/// The project list, from summaries. Invalidate it after a change.
final projectsProvider = FutureProvider<List<ProjectRow>>(
  (ref) => ref.watch(compterProvider).projects(includeArchived: false),
);

/// One opened project with its milestones
final projectProvider =
    AsyncNotifierProvider.family<ProjectNotifier, ProjectDetail, String>(
      ProjectNotifier.new,
    );

class ProjectNotifier extends AsyncNotifier<ProjectDetail> {
  ProjectNotifier(this.id);

  final String id;

  @override
  Future<ProjectDetail> build() => ref.watch(compterProvider).project(id: id);

  /// Show the project a change returned, which is what was saved, instead of loading it again
  void show(ProjectDetail saved) => state = AsyncData(saved);
}
