import 'package:flutter/material.dart';

import '../rust/api/app.dart';
import '../rust/api/expenses.dart';
import 'form_dialog.dart';

const _amount = TextInputType.numberWithOptions(decimal: true);

/// Add an expense, or edit [existing] with the form filled from it
Future<ExpenseRow?> expenseForm(
  BuildContext context,
  CompterApp app, {
  ExpenseRow? existing,
}) {
  final categories = expenseCategories();
  final values = existing?.edit;
  return FormDialog.show(
    context,
    title: existing == null ? 'Add expense' : 'Edit expense',
    fields: [
      FieldSpec('date', 'Date', date: true, initial: values?.date ?? ''),
      FieldSpec(
        'category',
        'Category',
        initial: values?.category ?? categories.first,
        options: categories,
      ),
      FieldSpec(
        'description',
        'Description',
        hint: 'e.g. Figma, annual plan',
        initial: values?.description ?? '',
      ),
      FieldSpec(
        'amount',
        'Amount',
        hint: 'total paid, GST included',
        keyboardType: _amount,
        initial: values?.amount ?? '',
      ),
      FieldSpec(
        'gst_credit',
        'GST credit',
        hint: 'GST in the amount to claim back; blank for none',
        keyboardType: _amount,
        initial: values?.gstCredit ?? '',
      ),
    ],
    submit: (values) {
      final input = ExpenseInput(
        date: values['date']!,
        category: values['category']!,
        description: values['description']!,
        amount: values['amount']!,
        gstCredit: values['gst_credit']!,
      );
      return existing == null
          ? app.addExpense(input: input)
          : app.editExpense(id: existing.id, input: input);
    },
  );
}
