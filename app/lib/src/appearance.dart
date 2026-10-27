import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'ui/theme.dart';

/// Which colours the app is drawn in. It belongs to the phone, like the sync method, so it
/// lives in the phone's preferences, not the database that travels.
enum AppTheme {
  system('Like the phone'),
  light('Light'),
  dark('Dark'),
  gruvboxDark('Gruvbox dark');

  const AppTheme(this.label);

  final String label;

  /// Light, dark and the [ThemeMode] choosing between them. Built each time, not kept:
  /// [ThemeData] fixes the platform it was made on, and with it page transitions.
  (ThemeData, ThemeData, ThemeMode) get themes {
    final light = seededTheme(Brightness.light);
    return switch (this) {
      system => (light, seededTheme(Brightness.dark), ThemeMode.system),
      AppTheme.light => (light, seededTheme(Brightness.dark), ThemeMode.light),
      dark => (light, seededTheme(Brightness.dark), ThemeMode.dark),
      gruvboxDark => (light, gruvboxDarkTheme(), ThemeMode.dark),
    };
  }
}

const _key = 'ui.theme';

/// The theme saved on this phone, read before the first frame so the app doesn't flash
/// another one
Future<AppTheme> savedAppTheme() async {
  final name = await SharedPreferencesAsync().getString(_key);
  return AppTheme.values.firstWhere(
    (theme) => theme.name == name,
    orElse: () => AppTheme.system,
  );
}

/// The chosen theme; [AppThemeNotifier.choose] changes and saves it
final appThemeProvider = NotifierProvider<AppThemeNotifier, AppTheme>(
  AppThemeNotifier.new,
);

class AppThemeNotifier extends Notifier<AppTheme> {
  AppThemeNotifier([this._saved = AppTheme.system]);

  final AppTheme _saved;

  @override
  AppTheme build() => _saved;

  Future<void> choose(AppTheme theme) async {
    state = theme;
    await SharedPreferencesAsync().setString(_key, theme.name);
  }
}
