import 'dart:async';

import 'package:flutter/material.dart';

import 'theme.dart';

/// How long an error shows before it goes by itself: long enough to read a sentence or two.
/// It can always be closed sooner.
const errorShownFor = Duration(seconds: 6);

/// A refusal that isn't about any one field of a sheet, as a card that goes after
/// [errorShownFor] or when closed. Errors about a field show under that field instead.
class ErrorToast extends StatefulWidget {
  const ErrorToast({super.key, required this.message, required this.onClose});

  final String message;

  /// Called on the close button, or once the card has shown for [errorShownFor]
  final VoidCallback onClose;

  @override
  State<ErrorToast> createState() => _ErrorToastState();
}

class _ErrorToastState extends State<ErrorToast> {
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _start();
  }

  @override
  void didUpdateWidget(ErrorToast old) {
    super.didUpdateWidget(old);
    // Another refusal gets its own full time
    if (old.message != widget.message) _start();
  }

  void _start() {
    _timer?.cancel();
    _timer = Timer(errorShownFor, () => widget.onClose());
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Semantics(
      liveRegion: true,
      child: Material(
        color: scheme.errorContainer,
        elevation: 3,
        borderRadius: BorderRadius.circular(12),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 4, 4, 4),
          child: Row(
            children: [
              Icon(Icons.error_outline, color: scheme.onErrorContainer),
              const SizedBox(width: 12),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 10),
                  child: Text(
                    widget.message,
                    style: TextStyle(color: scheme.onErrorContainer),
                  ),
                ),
              ),
              IconButton(
                tooltip: 'Dismiss',
                icon: const Icon(Icons.close),
                color: scheme.onErrorContainer,
                onPressed: widget.onClose,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// [child], a sheet's scrolling body, with [message] as an [ErrorToast] at its foot while
/// there is one. The body shrinks to make room, so the card never covers a field, even on a
/// form of one field.
class WithErrorToast extends StatelessWidget {
  const WithErrorToast({
    super.key,
    required this.child,
    required this.message,
    required this.onClose,
  });

  final Widget child;
  final String? message;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final message = this.message;
    // Always the same tree, so the fields keep their focus and scroll when the card comes
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Flexible(child: child),
        if (message != null)
          Padding(
            padding: const EdgeInsets.fromLTRB(gutter, 0, gutter, 8),
            child: ErrorToast(message: message, onClose: onClose),
          ),
      ],
    );
  }
}
