import 'package:flutter/material.dart';

/// compter's green, the seed of every colour in the app
const _seed = Color(0xFF2E6B5E);

/// Purple, for what credit notes took back: settled, but not by payment
const _creditedSeed = Color(0xFF7B4FA0);

/// Background and text of a CREDITED status chip, from its own seed so it is unlike any other
/// status
(Color, Color) creditedColors(Brightness brightness) {
  final scheme = ColorScheme.fromSeed(
    seedColor: _creditedSeed,
    brightness: brightness,
  );
  return (scheme.primaryContainer, scheme.onPrimaryContainer);
}

/// Side gutter of every screen, and the gap between sections
const gutter = 16.0;
const sectionGap = 24.0;

/// Digits of equal width, so amounts in a column line up
const tabular = [FontFeature.tabularFigures()];

/// The one theme: Material 3 from the seed, with every component's size and shape set here
/// rather than screen by screen
ThemeData buildTheme(Brightness brightness) {
  final scheme = ColorScheme.fromSeed(seedColor: _seed, brightness: brightness);
  final base = ThemeData(colorScheme: scheme, useMaterial3: true);
  final text = base.textTheme;
  const buttonSize = Size(64, 48);
  return base.copyWith(
    appBarTheme: const AppBarTheme(centerTitle: false),
    listTileTheme: ListTileThemeData(
      contentPadding: const EdgeInsets.symmetric(horizontal: gutter),
      minVerticalPadding: 12,
      titleTextStyle: text.bodyLarge?.copyWith(color: scheme.onSurface),
      subtitleTextStyle: text.bodyMedium?.copyWith(
        color: scheme.onSurfaceVariant,
      ),
      leadingAndTrailingTextStyle: text.titleSmall?.copyWith(
        color: scheme.onSurface,
        fontFeatures: tabular,
      ),
    ),
    cardTheme: const CardThemeData(margin: EdgeInsets.zero),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(minimumSize: buttonSize),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(minimumSize: buttonSize),
    ),
    textButtonTheme: TextButtonThemeData(
      style: TextButton.styleFrom(minimumSize: buttonSize),
    ),
    bottomSheetTheme: const BottomSheetThemeData(showDragHandle: true),
    inputDecorationTheme: const InputDecorationTheme(
      border: OutlineInputBorder(),
      helperMaxLines: 2,
    ),
    dividerTheme: const DividerThemeData(space: 1),
  );
}
