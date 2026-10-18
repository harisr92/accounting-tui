import 'package:flutter/material.dart';

import '../rust/api/app.dart';
import '../rust/api/reports.dart';
import 'form_dialog.dart';

/// Which of the quarter's first two months had their B2B invoices filed in an IFF
const _filed = ['neither', '1st month', '2nd month', 'both'];

/// Write a quarter's GSTR-1, offering the last quarter that has ended
Future<ExportRow?> exportGstr1(
  BuildContext context,
  CompterApp app,
  int year,
  ReportRow report,
) {
  return FormDialog.show(
    context,
    title: 'Export GSTR-1',
    submitLabel: 'Export',
    fields: [
      FieldSpec(
        'quarter',
        'Quarter',
        initial: report.quarters[report.quarterToFile - 1],
        options: report.quarters,
      ),
      const FieldSpec(
        'filed',
        'IFF filed',
        hint: 'months whose B2B invoices went in an IFF are left out',
        initial: 'neither',
        options: _filed,
      ),
    ],
    submit: (values) {
      final filed = _filed.indexOf(values['filed']!);
      return app.exportGstr1(
        year: year,
        quarter: report.quarters.indexOf(values['quarter']!) + 1,
        firstMonthFiled: filed == 1 || filed == 3,
        secondMonthFiled: filed == 2 || filed == 3,
      );
    },
  );
}

/// Write a month's IFF (its B2B invoices), offering the latest month that has ended
Future<ExportRow?> exportIff(
  BuildContext context,
  CompterApp app,
  int year,
  ReportRow report,
) {
  return FormDialog.show(
    context,
    title: 'Export IFF (B2B)',
    submitLabel: 'Export',
    fields: [
      FieldSpec(
        'month',
        'Month',
        initial: report.iffMonths[report.iffMonthToFile],
        options: report.iffMonths,
      ),
    ],
    submit: (values) => app.exportIff(
      year: year,
      month: report.iffMonths.indexOf(values['month']!),
    ),
  );
}
