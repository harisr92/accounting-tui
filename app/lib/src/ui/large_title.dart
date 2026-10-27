import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';

import 'glass.dart';
import 'status_chip.dart';
import 'theme.dart';

/// Wraps a screen so its [TitleBar] shows the title only once the [LargeTitle] at the top of
/// its list has scrolled under the bar, like the iOS Settings app. Until a [LargeTitle] is laid
/// out (while loading, or on an error), the bar shows the title itself.
class LargeTitleScope extends StatefulWidget {
  const LargeTitleScope({super.key, required this.child});

  final Widget child;

  @override
  State<LargeTitleScope> createState() => _LargeTitleScopeState();
}

class _LargeTitleScopeState extends State<LargeTitleScope> {
  final _controller = _TitleController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  bool _onScroll(ScrollNotification notification) {
    // Only the screen's own list: not a horizontal row of chips inside it
    if (notification.depth == 0 && notification.metrics.axis == Axis.vertical) {
      _controller.scrolledTo(notification.metrics.pixels);
    }
    return false;
  }

  @override
  Widget build(BuildContext context) =>
      NotificationListener<ScrollNotification>(
        onNotification: _onScroll,
        child: _Collapsed(notifier: _controller, child: widget.child),
      );
}

/// Whether the large title has gone under the bar: the list's offset against the offset at
/// which the title's last line leaves the top of the list
class _TitleController extends ChangeNotifier {
  double _pixels = 0;

  /// Unknown until a [LargeTitle] is laid out
  double? _threshold;

  bool collapsed = true;

  void scrolledTo(double pixels) {
    _pixels = pixels;
    _update();
  }

  set threshold(double value) {
    _threshold = value;
    _update();
  }

  void _update() {
    final threshold = _threshold;
    final next = threshold == null || _pixels >= threshold;
    if (next == collapsed) return;
    collapsed = next;
    notifyListeners();
  }
}

class _Collapsed extends InheritedNotifier<_TitleController> {
  const _Collapsed({required super.notifier, required super.child});

  /// Collapsed when no [LargeTitleScope] is above, so a bar never hides its title for nothing
  static bool of(BuildContext context) =>
      context
          .dependOnInheritedWidgetOfExactType<_Collapsed>()
          ?.notifier
          ?.collapsed ??
      true;

  /// The controller, without rebuilding the caller when it changes
  static _TitleController? controllerOf(BuildContext context) =>
      context.getInheritedWidgetOfExactType<_Collapsed>()?.notifier;
}

/// The fixed bar of a screen with a [LargeTitle]: empty over the large title, then the title
/// slides in, centred, with a hairline under the bar once the large one has scrolled away. The
/// bar is [Glass], so its [Scaffold] sets `extendBodyBehindAppBar` and lists scroll under it.
class TitleBar extends StatelessWidget implements PreferredSizeWidget {
  const TitleBar({super.key, required this.title, this.actions = const []});

  final String title;
  final List<Widget> actions;

  static const _duration = Duration(milliseconds: 200);

  @override
  Size get preferredSize => const Size.fromHeight(kToolbarHeight);

  @override
  Widget build(BuildContext context) {
    final collapsed = _Collapsed.of(context);
    return AppBar(
      centerTitle: true,
      flexibleSpace: const Glass(),
      shape: collapsed
          ? Border(
              bottom: BorderSide(
                color: Theme.of(context).colorScheme.outlineVariant,
                width: 0.5,
              ),
            )
          : null,
      // Only while collapsed, so the title is on screen once, not twice
      title: AnimatedSwitcher(
        duration: _duration,
        switchInCurve: Curves.easeOut,
        switchOutCurve: Curves.easeIn,
        transitionBuilder: (child, animation) => FadeTransition(
          opacity: animation,
          child: SlideTransition(
            position: Tween(
              begin: const Offset(0, 0.4),
              end: Offset.zero,
            ).animate(animation),
            child: child,
          ),
        ),
        child: collapsed
            ? Text(title, key: ValueKey(title))
            : const SizedBox.shrink(),
      ),
      actions: actions,
    );
  }
}

/// The first row of a screen's list: its title as a bold heading over a short line on what it
/// holds, after the screen's icon on a tinted tile as tall as the two. A record's [status] and
/// [details] go in a line under them.
class LargeTitle extends StatefulWidget {
  const LargeTitle({
    super.key,
    required this.icon,
    required this.title,
    required this.description,
    this.status,
    this.details,
  });

  final IconData icon;
  final String title;
  final String description;

  /// A status label from Rust, shown as a [StatusChip]
  final String? status;

  /// What doesn't fit in the description, such as a client's GSTIN and billing
  final String? details;

  @override
  State<LargeTitle> createState() => _LargeTitleState();
}

class _LargeTitleState extends State<LargeTitle> {
  /// The title's line height, tight so the icon beside it isn't taller than it needs to be
  static const _titleHeight = 1.15;

  final _titleKey = GlobalKey();

  /// Where the title's last line leaves the top of the list, measured after each layout so a
  /// larger text size or a longer name moves it
  void _measure(Duration _) {
    if (!mounted) return;
    final controller = _Collapsed.controllerOf(context);
    final box = _titleKey.currentContext?.findRenderObject();
    if (controller == null || box is! RenderBox || !box.hasSize) return;
    final viewport = RenderAbstractViewport.maybeOf(box);
    if (viewport == null) return;
    final top = viewport.getOffsetToReveal(box, 0).offset;
    // The list runs under the glass bar, which covers the top of it as far as its padding
    final covered = MediaQuery.paddingOf(context).top;
    controller.threshold = top + box.size.height - covered;
  }

  /// The icon's side: the height of the title's line and the description's one or two lines,
  /// at the text size in use. The description is measured beside the larger icon, so when it
  /// fits on one line there, it still does beside the smaller one.
  double _iconSide(
    BuildContext context,
    double width,
    TextStyle? titleStyle,
    TextStyle? descriptionStyle,
  ) {
    TextPainter laidOut(
      String text,
      TextStyle? style,
      int lines,
      double width,
    ) => TextPainter(
      text: TextSpan(text: text, style: style),
      maxLines: lines,
      ellipsis: '…',
      textDirection: Directionality.of(context),
      textScaler: MediaQuery.textScalerOf(context),
    )..layout(maxWidth: width);
    final title = laidOut(widget.title, titleStyle, 1, width);
    final line = laidOut('', descriptionStyle, 1, width);
    final largest = title.height + 2 * line.height;
    final description = laidOut(
      widget.description,
      descriptionStyle,
      2,
      (width - gutter - largest).clamp(0, width),
    );
    final side = title.height + description.height;
    title.dispose();
    line.dispose();
    description.dispose();
    return side;
  }

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback(_measure);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final status = widget.status;
    final details = widget.details;
    final titleStyle = theme.textTheme.headlineSmall?.copyWith(
      fontWeight: FontWeight.bold,
      height: _titleHeight,
    );
    final descriptionStyle = theme.textTheme.bodyMedium?.copyWith(
      color: scheme.onSurfaceVariant,
    );
    return Padding(
      padding: const EdgeInsets.fromLTRB(gutter, 8, gutter, 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          LayoutBuilder(
            builder: (context, constraints) {
              final side = _iconSide(
                context,
                constraints.maxWidth,
                titleStyle,
                descriptionStyle,
              );
              return Row(
                children: [
                  DecoratedBox(
                    decoration: BoxDecoration(
                      color: scheme.primary,
                      borderRadius: BorderRadius.circular(side / 4),
                    ),
                    child: SizedBox.square(
                      dimension: side,
                      child: Icon(
                        widget.icon,
                        size: side / 2,
                        color: scheme.onPrimary,
                      ),
                    ),
                  ),
                  const SizedBox(width: gutter),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          widget.title,
                          key: _titleKey,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: titleStyle,
                        ),
                        Text(
                          widget.description,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: descriptionStyle,
                        ),
                      ],
                    ),
                  ),
                ],
              );
            },
          ),
          if (status != null || details != null) ...[
            const SizedBox(height: 12),
            Row(
              children: [
                if (status != null) StatusChip(status),
                if (status != null && details != null) const SizedBox(width: 8),
                if (details != null)
                  Expanded(
                    child: Text(
                      details,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}
