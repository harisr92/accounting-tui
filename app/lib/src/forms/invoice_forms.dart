import 'package:flutter/material.dart';

import '../notice.dart';
import '../rust/api/app.dart';
import '../rust/api/error.dart';
import '../rust/api/invoicing.dart';
import '../rust/api/rows.dart';
import 'form_dialog.dart';

const _amount = TextInputType.numberWithOptions(decimal: true);
const _date = FieldSpec('date', 'Date', date: true);

/// Show the invoice [milestone] would get today and issue it on a yes, like `i` in the
/// terminal. A project billed in a foreign currency first asks for today's exchange rate.
/// `null` when cancelled or refused (the refusal is shown).
Future<IssuedRow?> issueInvoice(
  BuildContext context,
  CompterApp app,
  ProjectDetail project,
  MilestoneRow milestone,
) async {
  String? rate;
  InvoicePreviewRow? preview;
  if (project.currency == 'INR') {
    try {
      preview = await app.previewInvoice(
        projectId: project.id,
        milestoneId: milestone.id,
      );
    } on ApiError catch (error) {
      if (context.mounted) notify(context, error.message, error: true);
      return null;
    }
  } else {
    final asked = await FormDialog.show<(String, InvoicePreviewRow)>(
      context,
      title: 'Exchange rate',
      submitLabel: 'Preview',
      fields: [
        FieldSpec(
          'rate',
          '₹ per ${project.currency}',
          hint: "today's rate, e.g. RBI reference rate",
          keyboardType: _amount,
        ),
      ],
      submit: (values) async => (
        values['rate']!,
        await app.previewInvoice(
          projectId: project.id,
          milestoneId: milestone.id,
          rate: values['rate'],
        ),
      ),
    );
    if (asked == null) return null;
    (rate, preview) = asked;
  }
  if (!context.mounted) return null;
  final yes = await confirm(
    context,
    title: 'Issue ${preview.number}?',
    yes: 'Issue',
    lines: [
      '${preview.clientName} · ${preview.kind}',
      '${preview.total} (${preview.breakdown})',
      if (preview.totalInr != null) '${preview.totalInr} in INR',
      for (final warning in preview.warnings) 'Note: $warning',
    ],
  );
  if (!yes) return null;
  try {
    return await app.issueInvoice(
      projectId: project.id,
      milestoneId: milestone.id,
      rate: rate,
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

/// Ask what to take off the invoice, show the credit note it makes, and issue it on a yes.
/// Prefilled to cancel the whole invoice while nothing has been paid or credited.
Future<CreditedRow?> issueCreditNote(
  BuildContext context,
  CompterApp app,
  InvoiceDetail invoice,
) async {
  final asked = await FormDialog.show<(CreditInput, CreditPreviewRow)>(
    context,
    title: 'Credit note',
    submitLabel: 'Preview',
    fields: [
      _date,
      FieldSpec(
        'amount',
        'Amount',
        hint: 'before GST, in ${invoice.currency}; ${invoice.balance} due',
        keyboardType: _amount,
        initial: invoice.creditValue ?? '',
      ),
      const FieldSpec(
        'reason',
        'Reason',
        hint: 'e.g. Discount agreed, billed in error',
      ),
    ],
    submit: (values) async {
      final input = CreditInput(
        date: values['date']!,
        amount: values['amount']!,
        reason: values['reason']!,
      );
      return (
        input,
        await app.previewCredit(number: invoice.number, input: input),
      );
    },
  );
  if (asked == null || !context.mounted) return null;
  final (input, preview) = asked;
  final yes = await confirm(
    context,
    title: 'Issue ${preview.number}?',
    yes: 'Issue',
    lines: [
      'Against ${preview.invoice} for ${preview.total}'
          '${preview.gst == null ? '' : ' incl. GST ${preview.gst}'}',
      '${preview.balanceAfter} will still be due.',
      for (final warning in preview.warnings) 'Note: $warning',
    ],
  );
  if (!yes) return null;
  try {
    return await app.issueCredit(number: invoice.number, input: input);
  } on ApiError catch (error) {
    if (context.mounted) notify(context, error.message, error: true);
    return null;
  }
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
