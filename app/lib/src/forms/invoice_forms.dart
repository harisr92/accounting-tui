import 'package:flutter/material.dart';

import '../notice.dart';
import '../rust/api/app.dart';
import '../rust/api/error.dart';
import '../rust/api/invoicing.dart';
import '../rust/api/rows.dart';
import '../ui/check_list.dart';
import '../subscription/paywall.dart';
import 'form_dialog.dart';

const _amount = TextInputType.numberWithOptions(decimal: true);
const _date = FieldSpec('date', 'Date', date: true);
const _refund = 'Refund';
const _advance = 'Keep as advance';

/// How Rust names the amount of one credit note line in `ApiError.field`: `line:<milestone id>`
const _line = 'line:';

/// Pick which pending milestones of [project] to invoice together, starting from [milestone],
/// then show the invoice they would get today and issue it on a yes, like `i` in the terminal.
/// With only [milestone] pending there is nothing to pick. Asks for any discount first, and
/// today's exchange rate for a project billed in a foreign currency. `null` when cancelled or
/// refused (the refusal is shown).
Future<IssuedRow?> issueInvoice(
  BuildContext context,
  CompterApp app,
  ProjectDetail project,
  MilestoneRow milestone,
) async {
  if (!await requireWrite(context) || !context.mounted) return null;
  final pending = [
    for (final i in project.byDue)
      if (project.milestones[i].status == 'PENDING') project.milestones[i],
  ];
  var milestoneIds = [milestone.id];
  if (pending.length > 1) {
    final picked = await showCheckList(
      context,
      title: 'Milestones to invoice',
      items: [
        for (final m in pending)
          CheckItem(
            key: m.id,
            title: m.name,
            subtitle: '${m.amount} · due ${m.dueDate}',
            checked: m.id == milestone.id,
          ),
      ],
    );
    if (picked == null || !context.mounted) return null;
    milestoneIds = [for (final (id, _) in picked) id];
  }
  final foreign = project.currency != 'INR';
  final asked = await FormDialog.show<(String?, String?, InvoicePreviewRow)>(
    context,
    title: 'Invoice',
    submitLabel: 'Preview',
    fields: [
      const FieldSpec(
        'discount',
        'Discount',
        hint: 'e.g. 5000 or 10%, before GST; blank for none',
      ),
      if (foreign)
        FieldSpec(
          'rate',
          '₹ per ${project.currency}',
          hint: "today's rate, e.g. RBI reference rate",
          keyboardType: _amount,
        ),
    ],
    submit: (values) async => (
      values['rate'],
      values['discount'],
      await app.previewInvoice(
        projectId: project.id,
        milestoneIds: milestoneIds,
        rate: values['rate'],
        discount: values['discount'],
      ),
    ),
  );
  if (asked == null) return null;
  final (rate, discount, preview) = asked;
  if (!context.mounted) return null;
  final yes = await confirm(
    context,
    title: 'Issue ${preview.number}?',
    yes: 'Issue',
    lines: [
      '${preview.clientName} · ${preview.kind}',
      if (milestoneIds.length > 1) '${milestoneIds.length} milestones',
      '${preview.total} (${preview.breakdown})',
      if (preview.discount != null) 'After ${preview.discount} discount',
      if (preview.totalInr != null) '${preview.totalInr} in INR',
      for (final warning in preview.warnings) 'Note: $warning',
    ],
  );
  if (!yes) return null;
  try {
    return await app.issueInvoice(
      projectId: project.id,
      milestoneIds: milestoneIds,
      rate: rate,
      discount: discount,
    );
  } on ApiError catch (error) {
    if (context.mounted) notify(context, error.message, error: true);
    return null;
  }
}

/// Record all or part of a payment, prefilled with the whole balance and no TDS. A foreign
/// invoice asks for the rupees banked instead of TDS.
Future<InvoiceDetail?> recordPayment(
  BuildContext context,
  CompterApp app,
  InvoiceDetail invoice,
) {
  return FormDialog.show(
    context,
    title: 'Record payment',
    submitLabel: 'Record',
    fields: [
      _date,
      FieldSpec(
        'received',
        'Received (${invoice.currency})',
        hint: '${invoice.balance} due; part payments are fine',
        keyboardType: _amount,
        initial: invoice.balanceValue,
      ),
      if (invoice.foreign)
        FieldSpec(
          'tds_or_banked',
          'Banked ₹',
          hint:
              'rupees credited to your bank; invoiced at '
              '${invoice.exchangeRate ?? ''}',
          keyboardType: _amount,
        )
      else
        FieldSpec(
          'tds_or_banked',
          'TDS',
          hint: 'withheld by the client; 10% is ${invoice.usualTds ?? ''}',
          keyboardType: _amount,
          initial: '0',
        ),
    ],
    submit: (values) => app.recordPayment(
      number: invoice.number,
      input: PaymentInput(
        date: values['date']!,
        received: values['received']!,
        tdsOrBanked: values['tds_or_banked']!,
      ),
    ),
  );
}

/// Pick the invoice lines to take back and how much of each (all that is left, unless lowered),
/// then when and why; show the credit note it makes, and issue it on a yes. A note beyond what
/// is due gives the rest back to the client: refunded (in rupees, on a foreign invoice) or kept
/// as an advance.
Future<CreditedRow?> issueCreditNote(
  BuildContext context,
  CompterApp app,
  InvoiceDetail invoice,
) async {
  if (!await requireWrite(context)) return null;
  final names = {for (final line in invoice.lines) line.milestoneId: line.name};
  // What was picked and typed, kept when the next form is cancelled to come back to the lines
  Map<String, String>? kept;
  while (true) {
    if (!context.mounted) return null;
    final picked = await _pickCreditLines(context, app, invoice, kept);
    if (picked == null || !context.mounted) return null;
    final asked = await _creditTerms(context, app, invoice, picked, names);
    if (!context.mounted) return null;
    if (asked == null) {
      kept = {for (final (id, amount) in picked) id: amount};
      continue;
    }
    final (input, preview) = asked;
    final yes = await confirm(
      context,
      title: 'Issue ${preview.number}?',
      yes: 'Issue',
      lines: [
        'Against ${preview.invoice} for ${preview.total}'
            '${preview.gst == null ? '' : ' incl. GST ${preview.gst}'}',
        '${preview.balanceAfter} will still be due.',
        if (preview.returned != null) '${preview.returned}.',
        for (final warning in preview.warnings) 'Note: $warning',
      ],
    );
    if (!yes) return null;
    try {
      return await app.issueCredit(number: invoice.number, input: input);
    } on ApiError catch (error) {
      if (context.mounted) {
        notify(context, _naming(error, names).message, error: true);
      }
      return null;
    }
  }
}

/// The lines of [invoice] a credit note takes back and how much of each, checked by Rust before
/// the sheet closes; [kept] restores what was picked before, when coming back from the next step
Future<List<(String, String)>?> _pickCreditLines(
  BuildContext context,
  CompterApp app,
  InvoiceDetail invoice,
  Map<String, String>? kept,
) {
  final single = invoice.lines.length == 1;
  return showCheckList(
    context,
    title: 'Lines to credit',
    note: invoice.creditGuide,
    amountLabel: 'Credit, before GST, in ${invoice.currency}',
    // Prefilled (and the one line picked) only while nothing is paid or credited, so nothing
    // is refunded by default
    items: [
      for (final line in invoice.lines)
        CheckItem(
          key: line.milestoneId,
          title: line.name,
          subtitle: 'Billed ${line.amount} · ${line.creditable} left',
          amount: line.canCredit
              ? (kept == null
                    ? (line.creditValue ?? '')
                    : (kept[line.milestoneId] ?? ''))
              : null,
          // Picking a line fills in all that is left, to lower if need be
          fill: line.leftValue,
          enabled: line.canCredit,
          checked: kept == null
              ? single && line.creditValue != null
              : kept.containsKey(line.milestoneId),
        ),
    ],
    fieldPrefix: _line,
    check: (picked) => app.checkCreditLines(
      number: invoice.number,
      lines: _creditLines(picked),
    ),
  );
}

List<CreditLineInput> _creditLines(List<(String, String)> picked) => [
  for (final (id, amount) in picked)
    CreditLineInput(milestoneId: id, amount: amount),
];

/// [error] with the line it is about named, when it is about one: `Build: …`
ApiError _naming(ApiError error, Map<String, String> names) {
  final field = error.field;
  if (field == null || !field.startsWith(_line)) return error;
  final name = names[field.substring(_line.length)] ?? 'A line';
  // A line Rust checked names itself already
  if (error.message.startsWith('$name: ')) return error;
  return ApiError(message: '$name: ${error.message}');
}

/// When and why the credit note takes back [picked], previewed; `null` when cancelled
Future<(CreditInput, CreditPreviewRow)?> _creditTerms(
  BuildContext context,
  CompterApp app,
  InvoiceDetail invoice,
  List<(String, String)> picked,
  Map<String, String> names,
) {
  final lines = _creditLines(picked);
  return FormDialog.show<(CreditInput, CreditPreviewRow)>(
    context,
    title: 'Credit note',
    submitLabel: 'Preview',
    fields: [
      _date,
      const FieldSpec(
        'reason',
        'Reason',
        hint: 'e.g. Discount agreed, billed in error',
      ),
      const FieldSpec(
        'returned',
        'If already paid',
        hint: 'what happens to the part the client already paid',
        initial: _refund,
        options: [_refund, _advance],
      ),
      if (invoice.foreign)
        const FieldSpec(
          'refunded_inr',
          'Refunded ₹',
          hint: 'rupees refunded from your bank, for a refund',
          keyboardType: _amount,
        )
      else if (invoice.tdsWithheld)
        const FieldSpec(
          'tds_reversed',
          'TDS reversed',
          hint:
              'TDS the client cancels in its return; blank keeps it as your credit',
          keyboardType: _amount,
        ),
    ],
    submit: (values) async {
      final input = CreditInput(
        date: values['date']!,
        lines: lines,
        reason: values['reason']!,
        keepAsAdvance: values['returned'] == _advance,
        refundedInr: values['refunded_inr'] ?? '',
        tdsReversed: values['tds_reversed'] ?? '',
      );
      try {
        return (
          input,
          await app.previewCredit(number: invoice.number, input: input),
        );
      } on ApiError catch (error) {
        // An amount picked on the previous sheet: say which line it was
        throw _naming(error, names);
      }
    },
  );
}

/// Time worked on a pending milestone of an hourly project
Future<ProjectDetail?> logTime(
  BuildContext context,
  CompterApp app,
  ProjectDetail project,
  MilestoneRow milestone,
) {
  return FormDialog.show(
    context,
    title: 'Log time on ${milestone.name}',
    fields: const [
      _date,
      FieldSpec('duration', 'Time', hint: '1h30m, 45m, 1:30 or 1.5 (hours)'),
      FieldSpec('note', 'Note', hint: 'what was done'),
    ],
    submit: (values) => app.logTime(
      projectId: project.id,
      milestoneId: milestone.id,
      input: TimeInput(
        date: values['date']!,
        duration: values['duration']!,
        note: values['note']!,
      ),
    ),
  );
}
