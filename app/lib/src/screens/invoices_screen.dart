import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../router.dart';
import '../status.dart';

/// Every issued invoice, newest first, with what is still due across them, like the
/// terminal's Invoices tab
class InvoicesScreen extends ConsumerWidget {
  const InvoicesScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final invoices = ref.watch(invoicesProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('Invoices')),
      body: invoices.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, _) => Center(child: Text('$error')),
        data: (list) => RefreshIndicator(
          onRefresh: () => ref.refresh(invoicesProvider.future),
          child: ListView.separated(
            itemCount: list.invoices.length + 1,
            separatorBuilder: (_, _) => const Divider(height: 1),
            itemBuilder: (context, i) {
              if (i == 0) {
                return ListTile(
                  title: Text(
                    list.invoices.isEmpty
                        ? 'No invoices yet. Open a project and tap a pending '
                              'milestone to issue one.'
                        : 'Outstanding ${list.outstanding}',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                );
              }
              final row = list.invoices[i - 1];
              return ListTile(
                title: Text('${row.number} · ${row.clientName}'),
                subtitle: Text(
                  [
                    row.milestoneName,
                    row.date,
                    row.total,
                    if (row.due != null) '${row.due} due',
                  ].join(' · '),
                ),
                trailing: StatusChip(row.status),
                onTap: () => context.go(invoicePath(row.number)),
              );
            },
          ),
        ),
      ),
    );
  }
}
