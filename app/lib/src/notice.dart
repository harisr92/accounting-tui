import 'package:flutter/material.dart';
import 'package:open_filex/open_filex.dart';
import 'package:share_plus/share_plus.dart';

/// A message at the bottom of the screen, like `Notice` in the terminal's footer
void notify(BuildContext context, String message, {bool error = false}) {
  final scheme = Theme.of(context).colorScheme;
  ScaffoldMessenger.of(context)
    ..hideCurrentSnackBar()
    ..showSnackBar(
      SnackBar(
        content: Text(
          message,
          style: error ? TextStyle(color: scheme.onErrorContainer) : null,
        ),
        backgroundColor: error ? scheme.errorContainer : null,
      ),
    );
}

/// Ask a yes/no question; `true` only on [yes]
Future<bool> confirm(
  BuildContext context, {
  required String title,
  required List<String> lines,
  required String yes,
}) async {
  final answer = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(title),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          for (final line in lines)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: Text(line),
            ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(true),
          child: Text(yes),
        ),
      ],
    ),
  );
  return answer ?? false;
}

/// Open a PDF Rust just wrote in the phone's viewer
Future<void> openPdf(BuildContext context, String path) async {
  final result = await OpenFilex.open(path, type: 'application/pdf');
  if (result.type != ResultType.done && context.mounted) {
    notify(context, result.message, error: true);
  }
}

/// Send a PDF Rust just wrote to another app (mail, WhatsApp, Files)
Future<void> sharePdf(BuildContext context, String path) =>
    shareFile(context, path, 'application/pdf');

/// Send a file Rust just wrote (a PDF, a return's JSON) to another app
Future<void> shareFile(BuildContext context, String path, String mimeType) =>
    shareFiles(context, [(path, mimeType)]);

/// Send files Rust just wrote, as (path, MIME type), to another app in one go
Future<void> shareFiles(
  BuildContext context,
  List<(String, String)> files,
) async {
  final box = context.findRenderObject() as RenderBox?;
  await SharePlus.instance.share(
    ShareParams(
      files: [for (final (path, type) in files) XFile(path, mimeType: type)],
      // iPad shows the share sheet as a popover pointing at what was tapped
      sharePositionOrigin: box == null
          ? null
          : box.localToGlobal(Offset.zero) & box.size,
    ),
  );
}
