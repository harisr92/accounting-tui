import 'package:flutter/material.dart';

/// compter's green, the seed of every colour in the light and dark themes
const _seed = Color(0xFF2E6B5E);

/// Purple, for what credit notes took back: settled, but not by payment
const _creditedSeed = Color(0xFF7B4FA0);

/// Side gutter of every screen, and the gap between sections
const gutter = 16.0;
const sectionGap = 24.0;

/// Digits of equal width, so amounts in a column line up
const tabular = [FontFeature.tabularFigures()];

/// Background and text of a CREDITED status chip, unlike any other status in its theme
@immutable
class CreditedColors extends ThemeExtension<CreditedColors> {
  const CreditedColors({required this.background, required this.foreground});

  /// From its own seed, so it is unlike the seeded scheme's colours
  factory CreditedColors.seeded(Brightness brightness) {
    final scheme = ColorScheme.fromSeed(
      seedColor: _creditedSeed,
      brightness: brightness,
    );
    return CreditedColors(
      background: scheme.primaryContainer,
      foreground: scheme.onPrimaryContainer,
    );
  }

  final Color background;
  final Color foreground;

  /// The theme's, or the light one's when a theme has none
  static CreditedColors of(BuildContext context) =>
      Theme.of(context).extension<CreditedColors>() ??
      CreditedColors.seeded(Brightness.light);

  @override
  CreditedColors copyWith({Color? background, Color? foreground}) =>
      CreditedColors(
        background: background ?? this.background,
        foreground: foreground ?? this.foreground,
      );

  @override
  CreditedColors lerp(CreditedColors? other, double t) => other == null
      ? this
      : CreditedColors(
          background: Color.lerp(background, other.background, t)!,
          foreground: Color.lerp(foreground, other.foreground, t)!,
        );
}

/// Material 3 from compter's green, light or dark
ThemeData seededTheme(Brightness brightness) => _build(
  ColorScheme.fromSeed(seedColor: _seed, brightness: brightness),
  CreditedColors.seeded(brightness),
);

/// The gruvbox dark palette, like the terminal's gruvbox theme: cream text on warm greys,
/// orange for what can be tapped. Each container is its colour faded into the background,
/// under that colour's bright shade.
ThemeData gruvboxDarkTheme() {
  const bg0 = Color(0xFF282828);
  const fg = Color(0xFFEBDBB2);
  const orange = Color(0xFFFE8019);
  const yellow = Color(0xFFFABD2F);
  const aqua = Color(0xFF8EC07C);
  const red = Color(0xFFFB4934);
  const purple = Color(0xFFD3869B);
  Color faded(Color color) =>
      Color.alphaBlend(color.withValues(alpha: 0.22), bg0);
  // [color] mixed toward the foreground by [toFg]: each the least that reaches WCAG AA's
  // 4.5:1 for small text (test/accessibility_test.dart). Yellow needs none.
  Color lighter(Color color, double toFg) => Color.lerp(color, fg, toFg)!;
  final scheme =
      ColorScheme.fromSeed(
        seedColor: orange,
        brightness: Brightness.dark,
      ).copyWith(
        // Small orange labels (the selected tab) over the glass need a touch more light
        primary: lighter(orange, 0.15),
        onPrimary: bg0,
        primaryContainer: faded(orange),
        onPrimaryContainer: lighter(orange, 0.25),
        secondary: yellow,
        onSecondary: bg0,
        secondaryContainer: faded(yellow),
        onSecondaryContainer: yellow,
        tertiary: aqua,
        onTertiary: bg0,
        tertiaryContainer: faded(aqua),
        onTertiaryContainer: lighter(aqua, 0.1),
        error: red,
        onError: bg0,
        errorContainer: faded(red),
        onErrorContainer: lighter(red, 0.45),
        surface: bg0,
        onSurface: fg,
        // gruvbox fg3: fg4 falls below 4.5:1 on the raised tiles, and fg2 is too close to
        // the foreground to tell secondary text from primary
        onSurfaceVariant: const Color(0xFFBDAE93),
        surfaceDim: const Color(0xFF1D2021),
        surfaceBright: const Color(0xFF504945),
        surfaceContainerLowest: const Color(0xFF1D2021),
        surfaceContainerLow: const Color(0xFF32302F),
        surfaceContainer: const Color(0xFF3C3836),
        // Between bg1 and bg2: bg2 leaves fg3 short of 4.5:1
        surfaceContainerHigh: const Color(0xFF45403D),
        surfaceContainerHighest: const Color(0xFF665C54),
        outline: const Color(0xFF928374),
        outlineVariant: const Color(0xFF504945),
        inverseSurface: fg,
        onInverseSurface: bg0,
        inversePrimary: const Color(0xFFAF3A03),
        surfaceTint: orange,
      );
  return _build(
    scheme,
    CreditedColors(background: faded(purple), foreground: lighter(purple, 0.3)),
  );
}

/// Every component's size and shape, set here rather than screen by screen. Bars are glass
/// (`glass.dart`): see-through, so what scrolls under them shows blurred.
ThemeData _build(ColorScheme scheme, CreditedColors credited) {
  final base = ThemeData(colorScheme: scheme, useMaterial3: true);
  final text = base.textTheme;
  const buttonSize = Size(64, 48);
  return base.copyWith(
    extensions: [credited],
    appBarTheme: const AppBarTheme(
      centerTitle: false,
      backgroundColor: Colors.transparent,
      surfaceTintColor: Colors.transparent,
      scrolledUnderElevation: 0,
    ),
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
