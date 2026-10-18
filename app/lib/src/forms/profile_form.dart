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
        'address',
        'Address',
        hint: 'one line each: street, area, city and PIN',
        initial: input.address.join('\n'),
        multiline: true,
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
        'terms',
        'Terms',
        hint: 'one per line, e.g. Bank: HDFC 0001234, IFSC HDFC0000001',
        initial: input.terms.join('\n'),
        multiline: true,
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
        address: values['address']!.split('\n'),
        gstin: values['gstin']!,
        sacCode: values['sac_code']!,
        invoicePrefix: values['invoice_prefix']!,
        lutArn: values['lut_arn']!,
        terms: values['terms']!.split('\n'),
        currency: values['currency']!,
      ),
    ),
  );
}
