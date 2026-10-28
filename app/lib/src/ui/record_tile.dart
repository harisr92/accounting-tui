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

  /// Text this much larger than standard doesn't fit the column at the right, so the amount
  /// and status move under the facts instead
  static const _stackAbove = 1.5;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final status = this.status;
    final detail = this.detail;
    final amount = this.amount;
    final supporting = this.supporting;
    final hasFigures = amount != null || status != null;
    final stacked =
        hasFigures && MediaQuery.textScalerOf(context).scale(1) > _stackAbove;
    final amountText = amount == null
        ? null
        : Text(
            amount,
            style: theme.textTheme.titleSmall?.copyWith(fontFeatures: tabular),
          );
    final second = status != null
        ? StatusChip(status)
        : detail != null
        ? Text(detail, style: theme.textTheme.bodySmall)
        : null;
    return ListTile(
      title: Text(title, maxLines: 1, overflow: TextOverflow.ellipsis),
      subtitle: stacked
          ? Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (supporting != null) ...[
                  Text(supporting),
                  const SizedBox(height: 4),
                ],
                Wrap(
                  spacing: 8,
                  runSpacing: 4,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [?amountText, ?second],
                ),
              ],
            )
          : supporting == null
          ? null
          : Text(supporting, maxLines: 2, overflow: TextOverflow.ellipsis),
      trailing: stacked || !hasFigures
          ? null
          : Column(
              // ListTile gives its trailing widget unbounded height
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                ?amountText,
                if (status != null) const SizedBox(height: 4),
                ?second,
              ],
            ),
      onTap: onTap,
    );
  }
}
