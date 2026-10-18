import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'src/compter.dart';
import 'src/router.dart';
import 'src/rust/frb_generated.dart';
import 'src/ui/theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  final compter = await openCompter();
  runApp(
    ProviderScope(
      overrides: [compterProvider.overrideWithValue(compter)],
      child: const CompterMobile(),
    ),
  );
}

class CompterMobile extends StatefulWidget {
  const CompterMobile({super.key});

  @override
  State<CompterMobile> createState() => _CompterMobileState();
}

class _CompterMobileState extends State<CompterMobile> {
  final _router = buildRouter();

  @override
  void dispose() {
    _router.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: 'compter',
      theme: buildTheme(Brightness.light),
      darkTheme: buildTheme(Brightness.dark),
      routerConfig: _router,
    );
  }
}
