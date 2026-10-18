import 'package:flutter/material.dart';

import '../rust/api/app.dart';
import '../rust/api/profile.dart';
import 'form_dialog.dart';
import 'project_forms.dart';

/// The business details printed on every invoice, prefilled from what is saved (or the usual
/// SAC code and prefix the first time)
Future<ProfileRow?> profileForm(BuildContext context, CompterApp app) async {
  final input = await app.profileInput();
  if (!context.mounted) return null;
  String line(List<String> lines, int i) => i < lines.length ? lines[i] : '';
  return FormDialog.show(
    context,
    title: 'Business details',
    fields: [
      FieldSpec(
        'name',
        'Name',
        hint: 'your name or business name',
        initial: input.name,
      ),
      FieldSpec(
        'address_1',
        'Address',
        hint: 'street',
        initial: line(input.address, 0),
      ),
      FieldSpec('address_2', '', hint: 'area', initial: line(input.address, 1)),
      FieldSpec(
        'address_3',
        '',
        hint: 'city, PIN',
        initial: line(input.address, 2),
      ),
      FieldSpec(
        'gstin',
        'GSTIN',
        hint: 'blank if not registered: invoices without GST',
        initial: input.gstin,
      ),
      FieldSpec(
        'sac_code',
        'SAC code',
        hint: '998314 IT design & development',
        keyboardType: TextInputType.number,
        initial: input.sacCode,
      ),
      FieldSpec(
        'invoice_prefix',
        'Prefix',
        hint: 'up to 5 letters, e.g. INV',
        initial: input.invoicePrefix,
      ),
      FieldSpec(
        'lut_arn',
        'LUT ARN',
        hint: 'needed to invoice clients abroad',
        initial: input.lutArn,
      ),
      FieldSpec(
        'terms_1',
        'Terms',
        hint: 'e.g. Bank: HDFC 0001234, IFSC HDFC0000001',
        initial: line(input.terms, 0),
      ),
      FieldSpec(
        'terms_2',
        '',
        hint: 'e.g. Payment due in 15 days',
        initial: line(input.terms, 1),
      ),
      FieldSpec(
        'currency',
        'Currency',
        hint: 'what new projects are billed in',
        initial: input.currency,
        options: currencies,
      ),
    ],
    submit: (values) => app.saveProfile(
      input: ProfileInput(
        name: values['name']!,
        address: [
          values['address_1']!,
          values['address_2']!,
          values['address_3']!,
        ],
        gstin: values['gstin']!,
        sacCode: values['sac_code']!,
        invoicePrefix: values['invoice_prefix']!,
        lutArn: values['lut_arn']!,
        terms: [values['terms_1']!, values['terms_2']!],
        currency: values['currency']!,
      ),
    ),
  );
}
