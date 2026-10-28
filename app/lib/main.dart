import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'src/appearance.dart';
import 'src/compter.dart';
import 'src/onboarding.dart';
import 'src/router.dart';
import 'src/rust/frb_generated.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  final compter = await openCompter();
  final theme = await savedAppTheme();
  final welcome = await needsWelcome(compter);
  runApp(
    ProviderScope(
      overrides: [
        compterProvider.overrideWithValue(compter),
        appThemeProvider.overrideWith(() => AppThemeNotifier(theme)),
        showWelcomeProvider.overrideWithValue(welcome),
      ],
      child: const CompterMobile(),
    ),
  );
}

class CompterMobile extends ConsumerStatefulWidget {
  const CompterMobile({super.key});

  @override
  ConsumerState<CompterMobile> createState() => _CompterMobileState();
}

class _CompterMobileState extends ConsumerState<CompterMobile> {
  late final _router = buildRouter(welcome: ref.read(showWelcomeProvider));

  @override
  void dispose() {
    _router.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final (light, dark, mode) = ref.watch(appThemeProvider).themes;
    return MaterialApp.router(
      title: 'compter',
      theme: light,
      darkTheme: dark,
      themeMode: mode,
      routerConfig: _router,
    );
  }
}
