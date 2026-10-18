import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../forms/return_forms.dart';
import '../notice.dart';
import '../rust/api/reports.dart';
import '../ui/action_sheet.dart';
import '../ui/figures.dart';
import '../ui/record_tile.dart';
import '../ui/section.dart';
import '../ui/theme.dart';
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
          child: _ReportBody(report),
        ),
      ),
      floatingActionButton: report.value == null
          ? null
          : Builder(
              // Its own context, so the iPad share popover points at it
              builder: (button) => FloatingActionButton.extended(
                icon: const Icon(Icons.upload_file),
                label: const Text('Export return'),
                onPressed: () => _export(button, ref, year, report.value!),
              ),
            ),
    );
  }

  /// Which return, then its form; the written JSON goes to the share sheet, for the GST portal
  Future<void> _export(
    BuildContext context,
    WidgetRef ref,
    int year,
    ReportRow report,
  ) async {
    final iff = await showActionSheet<bool>(
      context,
      title: 'Export return',
      subtitle: report.year,
      actions: const [
        (
          icon: Icons.calendar_view_month,
          label: 'GSTR-1 for a quarter',
          value: false,
        ),
        (
          icon: Icons.calendar_today,
          label: 'IFF for a month (B2B)',
          value: true,
        ),
      ],
    );
    if (iff == null || !context.mounted) return;
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
  const _ReportBody(this.report);

  final ReportRow report;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final noteColor = switch (report.incomeTaxTone) {
      Tone.error => scheme.error,
      Tone.warning => scheme.tertiary,
      Tone.neutral => null,
    };
    List<(String, String)> figures(List<FigureRow> rows) => [
      for (final f in rows) (f.label, f.value),
    ];
    return ListView(
      padding: const EdgeInsets.only(bottom: 96),
      children: [
        FiscalYearPicker(reportYearProvider, label: report.year),
        const SectionHeader('Income'),
        FigureGrid(figures(report.income)),
        const SectionHeader('Income tax'),
        FigureGrid(figures(report.incomeTax)),
        SectionNote(report.incomeTaxNote, color: noteColor),
        const SectionHeader('GST by quarter'),
        for (final q in report.gst)
          RecordTile(
            title: q.quarter,
            supporting:
                'CGST ${q.cgst} · SGST ${q.sgst}\n'
                'IGST ${q.igst} · Input credit ${q.inputCredit}',
            amount: q.net,
          ),
        const SectionNote(
          'Net is output tax less input credit; negative is credit carried '
          'forward.',
        ),
        const SectionHeader('Clients · match TDS with Form 26AS'),
        if (report.clients.isEmpty)
          const SectionNote('No invoices or payments this year.'),
        for (final c in report.clients)
          RecordTile(
            title: c.client,
            supporting: 'Received ${c.received} · TDS ${c.tds}',
            amount: c.invoiced,
          ),
        const SizedBox(height: gutter),
      ],
    );
  }
}
