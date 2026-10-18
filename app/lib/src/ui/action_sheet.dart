import 'package:flutter/material.dart';

/// One choice in an action sheet
typedef SheetAction<T> = ({IconData icon, String label, T value});

/// A bottom sheet of choices for one record, within thumb's reach: [title] and [subtitle] say
/// which record, the rows what can be done with it. `null` when dismissed.
Future<T?> showActionSheet<T>(
  BuildContext context, {
  required String title,
  String? subtitle,
  required List<SheetAction<T>> actions,
}) {
  return showModalBottomSheet<T>(
    context: context,
    // Over the navigation bar too
    useRootNavigator: true,
    builder: (context) => SafeArea(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          ListTile(
            title: Text(title, style: Theme.of(context).textTheme.titleMedium),
            subtitle: subtitle == null ? null : Text(subtitle),
          ),
          const Divider(),
          for (final action in actions)
            ListTile(
              leading: Icon(action.icon),
              title: Text(action.label),
              onTap: () => Navigator.pop(context, action.value),
            ),
          const SizedBox(height: 8),
        ],
      ),
    ),
  );
}
