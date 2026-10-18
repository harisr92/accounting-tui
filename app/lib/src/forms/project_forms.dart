import 'package:flutter/material.dart';

import '../rust/api/app.dart';
import '../rust/api/rows.dart';
import 'form_dialog.dart';

const _amount = TextInputType.numberWithOptions(decimal: true);

/// What projects and the profile can be billed in, as Rust names them
const currencies = ['INR', 'USD', 'GBP', 'EUR'];

/// New project. A client with a country is abroad: an export, billed in any currency. One in
/// India without a GSTIN needs a state code, the place of supply. A blank rate is fixed price.
Future<ProjectDetail?> newProjectForm(BuildContext context, CompterApp app) {
  return FormDialog.show(
    context,
    title: 'New project',
    fields: const [
      FieldSpec('name', 'Name', hint: 'e.g. Website Revamp'),
      FieldSpec('client_name', 'Client', hint: 'e.g. Acme Pvt Ltd'),
      FieldSpec(
        'client_gstin',
        'Client GSTIN',
        hint: 'blank if unregistered or abroad',
      ),
      FieldSpec(
        'client_state',
        'State code',
        hint: 'India, no GSTIN: e.g. 29',
        keyboardType: TextInputType.number,
      ),
      FieldSpec('client_country', 'Country', hint: 'abroad only, e.g. Germany'),
      FieldSpec('address_1', 'Address', hint: 'street, building'),
      FieldSpec('address_2', '', hint: 'city, PIN / postcode'),
      FieldSpec(
        'currency',
        'Currency',
        hint: 'anything but INR needs a client abroad',
        initial: 'INR',
        options: currencies,
      ),
      FieldSpec(
        'hourly_rate',
        'Rate / hour',
        hint: 'blank for a fixed price',
        keyboardType: _amount,
      ),
    ],
    submit: (values) => app.createProject(
      input: ProjectInput(
        name: values['name']!,
        clientName: values['client_name']!,
        clientGstin: values['client_gstin'],
        clientState: values['client_state'],
        clientCountry: values['client_country'],
        clientAddress: [values['address_1']!, values['address_2']!],
        currency: values['currency'],
        hourlyRate: values['hourly_rate'],
      ),
    ),
  );
}

/// New milestone; hourly projects take no amount, it comes from logged time
Future<ProjectDetail?> addMilestoneForm(
  BuildContext context,
  CompterApp app,
  ProjectDetail project,
) {
  return FormDialog.show(
    context,
    title: 'Add milestone',
    fields: [
      const FieldSpec('name', 'Name', hint: 'e.g. Design'),
      const FieldSpec(
        'due_date',
        'Due date',
        hint: 'YYYY-MM-DD, blank for today',
        keyboardType: TextInputType.datetime,
      ),
      if (!project.hourly)
        const FieldSpec(
          'amount',
          'Amount',
          hint: 'e.g. 50,000',
          keyboardType: _amount,
        ),
    ],
    submit: (values) => app.addMilestone(
      projectId: project.id,
      input: MilestoneInput(
        name: values['name']!,
        dueDate: values['due_date']!,
        amount: values['amount'],
      ),
    ),
  );
}
