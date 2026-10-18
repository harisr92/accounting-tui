import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../router.dart';
import '../ui/empty_state.dart';
import '../ui/figures.dart';
import '../ui/record_tile.dart';
import '../ui/section.dart';

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
          child: ListView(
            padding: const EdgeInsets.only(top: 8, bottom: 16),
            children: [
              if (list.invoices.isEmpty)
                const EmptyState(
                  icon: Icons.receipt_long_outlined,
                  message:
                      'No invoices yet. Open a project and tap a pending '
                      'milestone to issue one.',
                )
              else ...[
                FigureGrid([('Outstanding', list.outstanding)]),
                const SectionHeader('Newest first'),
              ],
              for (final row in list.invoices)
                RecordTile(
                  title: row.clientName,
                  supporting: [row.number, row.milestoneName].join(' · '),
                  amount: row.due ?? row.total,
                  status: row.status,
                  onTap: () => context.go(invoicePath(row.number)),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
