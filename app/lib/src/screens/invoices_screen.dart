import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../compter.dart';
import '../router.dart';
import '../ui/empty_state.dart';
import '../ui/figures.dart';
import '../ui/large_title.dart';
import '../ui/record_tile.dart';
import '../ui/search.dart';
import '../ui/section.dart';

/// Every issued invoice, newest first, with what is still due across them, like the
/// terminal's Invoices tab
class InvoicesScreen extends ConsumerStatefulWidget {
  const InvoicesScreen({super.key});

  @override
  ConsumerState<InvoicesScreen> createState() => _InvoicesScreenState();
}

class _InvoicesScreenState extends ConsumerState<InvoicesScreen>
    with Searching {
  @override
  Widget build(BuildContext context) {
    final invoices = ref.watch(invoicesProvider);
    return LargeTitleScope(
      child: Scaffold(
        appBar: searchAppBar(
          title: 'Invoices',
          hint: 'Number, client, milestone or status',
        ),
        body: invoices.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Center(child: Text('$error')),
          data: (list) {
            final matches = query.apply(
              list.invoices,
              (row) => [
                row.number,
                row.clientName,
                row.milestoneName,
                row.kind,
                row.status,
              ],
            );
            return RefreshIndicator(
              onRefresh: () => ref.refresh(invoicesProvider.future),
              child: ListView(
                padding: const EdgeInsets.only(bottom: 16),
                children: [
                  const LargeTitle(
                    icon: Icons.receipt_long,
                    title: 'Invoices',
                    description:
                        'Everything issued, newest first, and what is still due',
                  ),
                  if (list.invoices.isEmpty)
                    const EmptyState(
                      icon: Icons.receipt_long_outlined,
                      message:
                          'No invoices yet. Open a project and tap a pending '
                          'milestone to issue one.',
                    )
                  else ...[
                    FigureGrid([('Outstanding', list.outstanding)]),
                    if (matches.isEmpty)
                      noMatches(query)
                    else
                      const SectionHeader('Newest first'),
                  ],
                  for (final row in matches)
                    RecordTile(
                      title: row.clientName,
                      supporting: [row.number, row.milestoneName].join(' · '),
                      amount: row.due ?? row.total,
                      status: row.status,
                      onTap: () => context.go(invoicePath(row.number)),
                    ),
                ],
              ),
            );
          },
        ),
      ),
    );
  }
}
