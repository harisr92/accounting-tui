import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';

/// Wraps a screen so its [CollapsingFab] shrinks to a round icon as soon as the user starts
/// scrolling down, and gets its label back when they scroll up. A short list that bounces back
/// to the top stays collapsed: only the user's own upward scroll extends it.
class FabScrollScope extends StatefulWidget {
  const FabScrollScope({super.key, required this.child});

  final Widget child;

  @override
  State<FabScrollScope> createState() => _FabScrollScopeState();
}

class _FabScrollScopeState extends State<FabScrollScope> {
  final _extended = ValueNotifier(true);

  @override
  void dispose() {
    _extended.dispose();
    super.dispose();
  }

  bool _onScroll(ScrollNotification notification) {
    // Only the screen's own list: not a horizontal row of chips inside it
    if (notification.depth != 0 || notification.metrics.axis != Axis.vertical) {
      return false;
    }
    if (notification is UserScrollNotification) {
      switch (notification.direction) {
        case ScrollDirection.reverse:
          _extended.value = false;
        case ScrollDirection.forward:
          _extended.value = true;
        case ScrollDirection.idle:
          break;
      }
    }
    return false;
  }

  @override
  Widget build(BuildContext context) =>
      NotificationListener<ScrollNotification>(
        onNotification: _onScroll,
        child: _Extended(notifier: _extended, child: widget.child),
      );
}

class _Extended extends InheritedNotifier<ValueNotifier<bool>> {
  const _Extended({required super.notifier, required super.child});

  /// Extended when no [FabScrollScope] is above, so a button never hides its label for nothing
  static bool of(BuildContext context) =>
      context
          .dependOnInheritedWidgetOfExactType<_Extended>()
          ?.notifier
          ?.value ??
      true;
}

/// Stands in for [FloatingActionButton.extended]'s default hero tag, which is private
class _DefaultHeroTag {
  const _DefaultHeroTag();
}

/// An extended floating button that morphs into a round icon button inside a [FabScrollScope]
class CollapsingFab extends StatelessWidget {
  const CollapsingFab({
    super.key,
    required this.icon,
    required this.label,
    required this.onPressed,
    this.tooltip,
    this.heroTag = const _DefaultHeroTag(),
  });

  final IconData icon;
  final String label;
  final VoidCallback? onPressed;

  /// Defaults to [label], which is all a collapsed button has to explain itself
  final String? tooltip;
  final Object? heroTag;

  static const _duration = Duration(milliseconds: 200);

  @override
  Widget build(BuildContext context) {
    final extended = _Extended.of(context);
    return TweenAnimationBuilder<double>(
      tween: Tween(end: extended ? 1 : 0),
      duration: _duration,
      curve: Curves.easeInOut,
      builder: (context, t, _) => FloatingActionButton.extended(
        heroTag: heroTag,
        tooltip: tooltip ?? label,
        onPressed: onPressed,
        // Corners grow to half the 56 height, so the collapsed button is a circle
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(28 - 12 * t),
        ),
        // 16 + 24 icon + 16 = the 56 of a round button; extended keeps Material's 20 at the end
        extendedPadding: EdgeInsetsDirectional.only(start: 16, end: 16 + 4 * t),
        extendedIconLabelSpacing: 0,
        icon: Icon(icon),
        label: ClipRect(
          child: Align(
            alignment: AlignmentDirectional.centerStart,
            widthFactor: t,
            child: Opacity(
              opacity: t,
              child: Padding(
                padding: const EdgeInsetsDirectional.only(start: 8),
                child: Text(label),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
