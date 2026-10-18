import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../forms/return_forms.dart';
import '../notice.dart';
import '../rust/api/reports.dart';
import '../stat_tile.dart';
import '../year_picker.dart';

/// A financial year for income tax and GST, and the GSTR-1 and IFF exports, like the
/// terminal's Reports tab
class ReportsScreen extends ConsumerWidget {
  const ReportsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final year = ref.watch(reportYearProvider);
    final report = ref.watch(reportProvider(year));
    return Scaffold(
      appBar: AppBar(title: const Text('Reports')),
      body: report.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, _) => Center(child: Text('$error')),
        data: (report) => RefreshIndicator(
          onRefresh: () => ref.refresh(reportProvider(year).future),
          child: _ReportBody(
            report,
            gstr1: (button) => _export(button, ref, year, report, iff: false),
            iff: (button) => _export(button, ref, year, report, iff: true),
          ),
        ),
      ),
    );
  }

  /// Write the return and hand its JSON to the share sheet, for upload to the GST portal.
  /// [context] is the button's, so the iPad share popover points at it.
  Future<void> _export(
    BuildContext context,
    WidgetRef ref,
    int year,
    ReportRow report, {
    required bool iff,
  }) async {
    final app = ref.read(compterProvider);
    final exported = iff
        ? await exportIff(context, app, year, report)
        : await exportGstr1(context, app, year, report);
    if (exported == null || !context.mounted) return;
    notify(context, exported.summary);
    await shareFile(context, exported.path, 'application/json');
  }
}

class _ReportBody extends StatelessWidget {
  const _ReportBody(this.report, {required this.gstr1, required this.iff});

  final ReportRow report;

  /// Called with the tapped button's context
  final void Function(BuildContext button) gstr1;
  final void Function(BuildContext button) iff;

  @override
  Widget build(BuildContext context) {
    final text = Theme.of(context).textTheme;
    final scheme = Theme.of(context).colorScheme;
    final noteColor = switch (report.incomeTaxTone) {
      Tone.error => scheme.error,
      Tone.warning => scheme.tertiary,
      Tone.neutral => scheme.onSurfaceVariant,
    };
    Widget heading(String title) => Padding(
      padding: const EdgeInsets.only(top: 24, bottom: 8),
      child: Text(title, style: text.titleMedium),
    );
    Widget tiles(List<FigureRow> figures) => Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [for (final f in figures) StatTile(f.label, f.value)],
    );
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 32),
      children: [
        FiscalYearPicker(reportYearProvider, label: report.year),
        tiles(report.income),
        heading('Income tax'),
        tiles(report.incomeTax),
        const SizedBox(height: 8),
        Text(
          report.incomeTaxNote,
          style: text.bodySmall?.copyWith(color: noteColor),
        ),
        heading('GST'),
        for (final q in report.gst)
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(q.quarter),
            subtitle: Text(
              'CGST ${q.cgst} · SGST ${q.sgst} · IGST ${q.igst} · '
              'Input credit ${q.inputCredit}',
            ),
            trailing: Text(q.net, style: text.titleSmall),
          ),
        Text(
          'Net is output tax less input credit; negative is credit carried '
          'forward.',
          style: text.bodySmall,
        ),
        const SizedBox(height: 16),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            Builder(
              // Their own contexts, so the iPad share popover points at the button
              builder: (context) => FilledButton.icon(
                icon: const Icon(Icons.upload_file),
                label: const Text('Export GSTR-1'),
                onPressed: () => gstr1(context),
              ),
            ),
            Builder(
              builder: (context) => OutlinedButton.icon(
                icon: const Icon(Icons.upload_file),
                label: const Text('Export IFF'),
                onPressed: () => iff(context),
              ),
            ),
          ],
        ),
        heading('Clients · match TDS with Form 26AS'),
        if (report.clients.isEmpty)
          const Text('No invoices or payments this year.'),
        for (final c in report.clients)
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(c.client),
            subtitle: Text(
              'Invoiced ${c.invoiced} · Received ${c.received} · TDS ${c.tds}',
            ),
          ),
      ],
    );
  }
}
