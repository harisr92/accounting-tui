import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';

import 'rust/api/app.dart';
import 'rust/api/invoicing.dart';
import 'rust/api/profile.dart';
import 'rust/api/rows.dart';

/// `flutter run --dart-define=COMPTER_DEMO=true` opens the sample business in memory, like
/// `compter --demo`; nothing is saved.
const demoMode = bool.fromEnvironment('COMPTER_DEMO');

/// The database in the app's private support folder, opened once for the life of the app.
/// PDFs are written next to it; the demo writes them to the cache folder.
Future<CompterApp> openCompter() async {
  if (demoMode) {
    final scratch = await getTemporaryDirectory();
    return CompterApp.demo(scratchDir: scratch.path);
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

/// The business profile, or `null` before it is set up. Invalidate it after saving.
final profileProvider = FutureProvider<ProfileRow?>(
  (ref) => ref.watch(compterProvider).profile(),
);

/// Every issued invoice, newest first. Invalidate it after issuing, a payment or a credit note.
final invoicesProvider = FutureProvider<InvoiceList>(
  (ref) => ref.watch(compterProvider).invoices(),
);

/// One opened invoice with its payments and credit notes
final invoiceProvider =
    AsyncNotifierProvider.family<InvoiceNotifier, InvoiceDetail, String>(
      InvoiceNotifier.new,
    );

class InvoiceNotifier extends AsyncNotifier<InvoiceDetail> {
  InvoiceNotifier(this.number);

  final String number;

  @override
  Future<InvoiceDetail> build() =>
      ref.watch(compterProvider).invoice(number: number);

  /// Show the invoice a payment or credit note returned instead of loading it again
  void show(InvoiceDetail saved) => state = AsyncData(saved);
}
