import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../forms/invoice_forms.dart';
import '../notice.dart';
import '../rust/api/error.dart';
import '../rust/api/invoicing.dart';
import '../ui/collapsing_fab.dart';
import '../ui/figures.dart';
import '../ui/large_title.dart';
import '../ui/record_tile.dart';
import '../ui/section.dart';
import '../ui/theme.dart';

/// One invoice: its amounts, payments and credit notes. Its PDF can be opened or shared. While
/// something is due a payment can be added, and until credit notes take off the whole total, a
/// credit note (on a paid invoice, it refunds or keeps the rest as an advance).
class InvoiceScreen extends ConsumerWidget {
  const InvoiceScreen({super.key, required this.number});

  final String number;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final invoice = ref.watch(invoiceProvider(number));
    final detail = invoice.value;
    final due = detail != null && !detail.settled;
    final creditable = detail != null && detail.creditable;
    return LargeTitleScope(
      child: FabScrollScope(
        child: Scaffold(
          appBar: TitleBar(title: number),
          body: invoice.when(
            loading: () => const Center(child: CircularProgressIndicator()),
            error: (error, _) => Center(child: Text('$error')),
            data: (detail) => _InvoiceBody(
              detail,
              creditNotePdf: (row, note) =>
                  _pdf(row, ref, creditNote: note, share: true),
            ),
          ),
          // Every action within thumb's reach: the documents on the bar, the payment as the one
          // filled button
          bottomNavigationBar: detail == null
              ? null
              : BottomAppBar(
                  child: Row(
                    children: [
                      Builder(
                        // Its own context, so the iPad share popover points at it
                        builder: (button) => IconButton(
                          tooltip: 'Share PDF',
                          icon: const Icon(Icons.ios_share),
                          onPressed: () => _pdf(button, ref, share: true),
                        ),
                      ),
                      IconButton(
                        tooltip: 'Open PDF',
                        icon: const Icon(Icons.picture_as_pdf_outlined),
                        onPressed: () => _pdf(context, ref, share: false),
                      ),
                      if (creditable)
                        IconButton(
                          tooltip: 'Credit note',
                          icon: const Icon(Icons.remove_circle_outline),
                          onPressed: () => _credit(context, ref, detail),
                        ),
                    ],
                  ),
                ),
          floatingActionButton: due
              ? CollapsingFab(
                  icon: Icons.payments_outlined,
                  label: 'Record payment',
                  onPressed: () => _pay(context, ref, detail),
                )
              : null,
          floatingActionButtonLocation:
              FloatingActionButtonLocation.endContained,
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

  /// Open or share the invoice's PDF, or [creditNote]'s; Rust writes it first if it is missing.
  /// [context] is the tapped button's, so the iPad share popover points at it.
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
  const _InvoiceBody(this.invoice, {required this.creditNotePdf});

  final InvoiceDetail invoice;

  /// Called with the tapped row's context
  final void Function(BuildContext row, String note) creditNotePdf;

  @override
  Widget build(BuildContext context) {
    final totalInr = invoice.totalInr;
    final discount = invoice.discount;
    return ListView(
      padding: const EdgeInsets.only(bottom: gutter),
      children: [
        LargeTitle(
          icon: Icons.receipt_long,
          title: invoice.number,
          description: invoice.clientName,
          status: invoice.status,
          details: [
            invoice.milestones,
            invoice.date,
            invoice.kind,
            ?invoice.exchangeRate,
          ].join(' · '),
        ),
        FigureGrid([
          ('Total', invoice.total),
          ('Due', invoice.balance),
          if (totalInr != null) ('In INR', totalInr),
        ]),
        SectionNote(invoice.breakdown),
        if (discount != null)
          SectionNote('After $discount discount, before GST'),
        if (invoice.clientAdvance != null)
          SectionNote(
            '${invoice.clientAdvance} held as an advance for ${invoice.clientName}',
          ),
        const SectionHeader('Payments and credit notes'),
        if (invoice.history.isEmpty) const SectionNote('Nothing received yet'),
        for (final row in invoice.history)
          Builder(
            // Its own context, so the iPad share popover points at the row
            builder: (context) => RecordTile(
              title: row.document,
              supporting: row.creditNote
                  ? [
                      row.date,
                      ?row.returned,
                      'tap to share its PDF',
                    ].join(' · ')
                  : [
                      row.date,
                      if (row.tdsOrBanked != null)
                        invoice.foreign
                            ? 'banked ${row.tdsOrBanked}'
                            : 'TDS ${row.tdsOrBanked}',
                    ].join(' · '),
              amount: row.settled,
              onTap: row.creditNote
                  ? () => creditNotePdf(context, row.document)
                  : null,
            ),
          ),
      ],
    );
  }
}
