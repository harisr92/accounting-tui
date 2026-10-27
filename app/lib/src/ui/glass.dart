import 'dart:ui';

import 'package:flutter/material.dart';

/// Frosted glass: the bars' see-through background, so what scrolls under them shows blurred
class Glass extends StatelessWidget {
  const Glass({super.key});

  /// How much of the surface colour the glass keeps
  static const _opacity = 0.72;

  /// The bars' background: the surface, see-through. A [CupertinoTabBar] blurs what is under
  /// a background that isn't opaque by itself.
  static Color tint(ColorScheme scheme) =>
      scheme.surface.withValues(alpha: _opacity);

  @override
  Widget build(BuildContext context) => ClipRect(
    child: BackdropFilter(
      filter: ImageFilter.blur(sigmaX: 20, sigmaY: 20),
      child: ColoredBox(
        color: tint(Theme.of(context).colorScheme),
        child: const SizedBox.expand(),
      ),
    ),
  );
}

/// A screen's list under glass bars. Its [Scaffold] extends the body under the bars and says
/// how tall they are in the padding; the list starts below the header, ends above the tab bar
/// plus [bottom] (room for a floating button), and pull to refresh comes down from under the
/// header.
class ScreenList extends StatelessWidget {
  const ScreenList({
    super.key,
    required this.children,
    this.bottom = 0,
    this.onRefresh,
  });

  final List<Widget> children;
  final double bottom;
  final RefreshCallback? onRefresh;

  @override
  Widget build(BuildContext context) {
    final bars = MediaQuery.paddingOf(context);
    final list = ListView(
      padding: EdgeInsets.only(top: bars.top, bottom: bars.bottom + bottom),
      children: children,
    );
    final refresh = onRefresh;
    return refresh == null
        ? list
        : RefreshIndicator(
            onRefresh: refresh,
            edgeOffset: bars.top,
            child: list,
          );
  }
}
