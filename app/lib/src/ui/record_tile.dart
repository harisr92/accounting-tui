import 'package:flutter/material.dart';

import 'status_chip.dart';
import 'theme.dart';

/// The one row layout: what it is, one line of key facts, and its amount with the status
/// under it. Projects, milestones, invoices, expenses, payments and clients all use it.
class RecordTile extends StatelessWidget {
  const RecordTile({
    super.key,
    required this.title,
    this.supporting,
    this.amount,
    this.detail,
    this.status,
    this.onTap,
  });

  final String title;

  /// One line of key facts, joined with ` · `
  final String? supporting;

  final String? amount;

  /// A second line under the amount, when there is no status (e.g. GST credit)
  final String? detail;

  /// A status label from Rust, shown as a chip under the amount
  final String? status;

  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final status = this.status;
    final detail = this.detail;
    return ListTile(
      title: Text(title, maxLines: 1, overflow: TextOverflow.ellipsis),
      subtitle: supporting == null
          ? null
          : Text(supporting!, maxLines: 2, overflow: TextOverflow.ellipsis),
      trailing: amount == null && status == null
          ? null
          : Column(
              // ListTile gives its trailing widget unbounded height
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                if (amount != null)
                  Text(
                    amount!,
                    style: theme.textTheme.titleSmall?.copyWith(
                      fontFeatures: tabular,
                    ),
                  ),
                if (status != null) ...[
                  const SizedBox(height: 4),
                  StatusChip(status),
                ] else if (detail != null)
                  Text(detail, style: theme.textTheme.bodySmall),
              ],
            ),
      onTap: onTap,
    );
  }
}
