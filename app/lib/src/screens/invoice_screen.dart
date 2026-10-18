import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../forms/invoice_forms.dart';
import '../notice.dart';
import '../rust/api/error.dart';
import '../rust/api/invoicing.dart';
import '../stat_tile.dart';
import '../status.dart';

/// One invoice: its amounts, payments and credit notes. Its PDF can be opened or shared, and
/// while something is due a payment or credit note can be added.
class InvoiceScreen extends ConsumerWidget {
  const InvoiceScreen({super.key, required this.number});

  final String number;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final invoice = ref.watch(invoiceProvider(number));
    return Scaffold(
      appBar: AppBar(title: Text(number)),
      body: invoice.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, _) => Center(child: Text('$error')),
        data: (detail) => _InvoiceBody(
          detail,
          pdf: (share) => _pdf(context, ref, share: share),
          creditNotePdf: (note, share) =>
              _pdf(context, ref, creditNote: note, share: share),
          pay: () => _pay(context, ref, detail),
          credit: () => _credit(context, ref, detail),
        ),
      ),
    );
  }

  /// A change saved: show the invoice it returned, and refresh the lists it shows up in
  void _show(WidgetRef ref, InvoiceDetail saved) {
    ref.read(invoiceProvider(number).notifier).show(saved);
    ref.invalidate(invoicesProvider);
    ref.invalidate(projectsProvider);
    ref.invalidate(projectProvider);
  }

  /// Open or share the invoice's PDF, or [creditNote]'s; Rust writes it first if it is missing
  Future<void> _pdf(
    BuildContext context,
    WidgetRef ref, {
    String? creditNote,
    required bool share,
  }) async {
    final app = ref.read(compterProvider);
    try {
      final path = creditNote == null
          ? await app.invoicePdf(number: number)
          : await app.creditNotePdf(number: creditNote);
      if (!context.mounted) return;
      await (share ? sharePdf(context, path) : openPdf(context, path));
    } on ApiError catch (error) {
      if (context.mounted) notify(context, error.message, error: true);
    }
  }

  Future<void> _pay(
    BuildContext context,
    WidgetRef ref,
    InvoiceDetail invoice,
  ) async {
    final paid = await recordPayment(
      context,
      ref.read(compterProvider),
      invoice,
    );
    if (paid == null || !context.mounted) return;
    _show(ref, paid);
    notify(
      context,
      paid.settled
          ? '${paid.number} paid in full'
          : 'Payment recorded; ${paid.balance} still due',
    );
  }

  Future<void> _credit(
    BuildContext context,
    WidgetRef ref,
    InvoiceDetail invoice,
  ) async {
    final credited = await issueCreditNote(
      context,
      ref.read(compterProvider),
      invoice,
    );
    if (credited == null || !context.mounted) return;
    _show(ref, credited.invoice);
    final pdfError = credited.pdfError;
    notify(
      context,
      pdfError == null
          ? 'Issued ${credited.noteNumber}'
          : "Issued ${credited.noteNumber}, but its PDF wasn't saved ($pdfError)",
      error: pdfError != null,
    );
  }
}

class _InvoiceBody extends StatelessWidget {
  const _InvoiceBody(
    this.invoice, {
    required this.pdf,
    required this.creditNotePdf,
    required this.pay,
    required this.credit,
  });

  final InvoiceDetail invoice;
  final void Function(bool share) pdf;
  final void Function(String note, bool share) creditNotePdf;
  final VoidCallback pay;
  final VoidCallback credit;

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 32),
      children: [
        Row(
          children: [
            Expanded(child: Text(invoice.clientName, style: text.titleMedium)),
            StatusChip(invoice.status),
          ],
        ),
        Text(
          [
            invoice.milestoneName,
            invoice.date,
            invoice.kind,
            ?invoice.exchangeRate,
          ].join(' · '),
          style: text.bodyMedium,
        ),
        const SizedBox(height: 16),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            StatTile('Total', invoice.total),
            if (invoice.totalInr != null) StatTile('In INR', invoice.totalInr!),
            StatTile('Due', invoice.balance),
          ],
        ),
        const SizedBox(height: 8),
        Text(invoice.breakdown, style: text.bodySmall),
        const SizedBox(height: 16),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            Builder(
              // Its own context, so the iPad share popover points at this button
              builder: (context) => OutlinedButton.icon(
                icon: const Icon(Icons.ios_share),
                label: const Text('Share PDF'),
                onPressed: () => pdf(true),
              ),
            ),
            OutlinedButton.icon(
              icon: const Icon(Icons.picture_as_pdf_outlined),
              label: const Text('Open PDF'),
              onPressed: () => pdf(false),
            ),
            if (!invoice.settled) ...[
              FilledButton.icon(
                icon: const Icon(Icons.payments_outlined),
                label: const Text('Record payment'),
                onPressed: pay,
              ),
              OutlinedButton.icon(
                icon: const Icon(Icons.remove_circle_outline),
                label: const Text('Credit note'),
                onPressed: credit,
              ),
            ],
          ],
        ),
        const SizedBox(height: 24),
        Text('Payments and credit notes', style: text.titleMedium),
        if (invoice.history.isEmpty)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 16),
            child: Text('Nothing received yet'),
          ),
        for (final row in invoice.history)
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: Text('${row.document} · ${row.date}'),
            subtitle: row.creditNote
                ? const Text('Tap to share its PDF')
                : Text(
                    [
                      if (row.received != null) 'Received ${row.received}',
                      if (row.tdsOrBanked != null)
                        invoice.foreign
                            ? 'Banked ${row.tdsOrBanked}'
                            : 'TDS ${row.tdsOrBanked}',
                    ].join(' · '),
                  ),
            trailing: Text(row.settled, style: text.titleSmall),
            onTap: row.creditNote
                ? () => creditNotePdf(row.document, true)
                : null,
          ),
      ],
    );
  }
}
