import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'compter.dart';
import 'rust/api/app.dart';

const _welcomeKey = 'onboarding.welcome_seen';
const _checklistKey = 'onboarding.checklist_hidden';

/// Whether this phone starts on the welcome pages: only the first time, and never for a
/// phone that already holds projects (an update, or data synced in), which is remembered so
/// they don't appear later either. Read before the first frame, like the theme. Never throws:
/// a welcome is never worth the app not starting, so on a failure it opens on the tabs.
Future<bool> needsWelcome(CompterApp app) async {
  if (demoMode) return false;
  try {
    final prefs = SharedPreferencesAsync();
    if (await prefs.getBool(_welcomeKey) ?? false) return false;
    final projects = await app.projects(includeArchived: true);
    if (projects.isEmpty) return true;
    await prefs.setBool(_welcomeKey, true);
  } catch (_) {}
  return false;
}

/// The welcome pages were read or skipped
Future<void> welcomeSeen() =>
    SharedPreferencesAsync().setBool(_welcomeKey, true);

/// Set in `main` from [needsWelcome]; tests start on the tabs
final showWelcomeProvider = Provider<bool>((ref) => false);

/// Whether the "Get started" card on Projects was hidden on this phone. While it loads the
/// card stays hidden, so it never flashes.
final checklistHiddenProvider =
    AsyncNotifierProvider<ChecklistHiddenNotifier, bool>(
      ChecklistHiddenNotifier.new,
    );

class ChecklistHiddenNotifier extends AsyncNotifier<bool> {
  @override
  Future<bool> build() async =>
      await SharedPreferencesAsync().getBool(_checklistKey) ?? false;

  Future<void> hide() async {
    state = const AsyncData(true);
    await SharedPreferencesAsync().setBool(_checklistKey, true);
  }
}
