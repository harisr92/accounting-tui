import 'package:flutter/material.dart';

import '../rust/api/app.dart';
import '../rust/api/rows.dart';
import 'form_dialog.dart';

const _amount = TextInputType.numberWithOptions(decimal: true);

/// What projects and the profile can be billed in, as Rust names them
const currencies = ['INR', 'USD', 'GBP', 'EUR'];

/// New project, or [existing] edited with the form filled from it. A client with a country is
/// abroad: an export, billed in any currency. One in India without a GSTIN needs a state code,
/// the place of supply. A blank rate is fixed price.
Future<ProjectDetail?> projectForm(
  BuildContext context,
  CompterApp app, {
  ProjectDetail? existing,
}) {
  final values = existing?.edit;
  return FormDialog.show(
    context,
    title: existing == null ? 'New project' : 'Edit project',
    fields: [
      FieldSpec(
        'name',
        'Name',
        hint: 'e.g. Website Revamp',
        initial: values?.name ?? '',
      ),
      FieldSpec(
        'client_name',
        'Client',
        hint: 'e.g. Acme Pvt Ltd',
        initial: values?.clientName ?? '',
      ),
      FieldSpec(
        'client_gstin',
        'Client GSTIN',
        hint: 'blank if unregistered or abroad',
        initial: values?.clientGstin ?? '',
      ),
      FieldSpec(
        'client_state',
        'State code',
        hint: 'India, no GSTIN: e.g. 29',
        keyboardType: TextInputType.number,
        initial: values?.clientState ?? '',
      ),
      FieldSpec(
        'client_country',
        'Country',
        hint: 'abroad only, e.g. Germany',
        initial: values?.clientCountry ?? '',
      ),
      FieldSpec(
        'address',
        'Address',
        hint: 'one line each: street, city and PIN / postcode',
        initial: values?.clientAddress.join('\n') ?? '',
        multiline: true,
      ),
      FieldSpec(
        'currency',
        'Currency',
        hint: 'anything but INR needs a client abroad',
        initial: values?.currency ?? 'INR',
        options: currencies,
      ),
      FieldSpec(
        'hourly_rate',
        'Rate / hour',
        hint: 'blank for a fixed price',
        keyboardType: _amount,
        initial: values?.hourlyRate ?? '',
      ),
    ],
    submit: (values) {
      final input = ProjectInput(
        name: values['name']!,
        clientName: values['client_name']!,
        clientGstin: values['client_gstin'],
        clientState: values['client_state'],
        clientCountry: values['client_country'],
        clientAddress: values['address']!.split('\n'),
        currency: values['currency'],
        hourlyRate: values['hourly_rate'],
      );
      return existing == null
          ? app.createProject(input: input)
          : app.editProject(id: existing.id, input: input);
    },
  );
}

/// New milestone, or [existing] edited; hourly projects take no amount, it comes from logged
/// time
Future<ProjectDetail?> milestoneForm(
  BuildContext context,
  CompterApp app,
  ProjectDetail project, {
  MilestoneRow? existing,
}) {
  final values = existing?.edit;
  return FormDialog.show(
    context,
    title: existing == null ? 'Add milestone' : 'Edit milestone',
    fields: [
      FieldSpec(
        'name',
        'Name',
        hint: 'e.g. Design',
        initial: values?.name ?? '',
      ),
      FieldSpec(
        'due_date',
        'Due date',
        date: true,
        initial: values?.dueDate ?? '',
      ),
      if (!project.hourly)
        FieldSpec(
          'amount',
          'Amount',
          hint: 'e.g. 50,000',
          keyboardType: _amount,
          initial: values?.amount ?? '',
        ),
    ],
    submit: (values) {
      final input = MilestoneInput(
        name: values['name']!,
        dueDate: values['due_date']!,
        amount: values['amount'],
      );
      return existing == null
          ? app.addMilestone(projectId: project.id, input: input)
          : app.editMilestone(
              projectId: project.id,
              milestoneId: existing.id,
              input: input,
            );
    },
  );
}
