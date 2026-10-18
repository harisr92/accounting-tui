import 'package:flutter/material.dart';

import '../rust/api/app.dart';
import '../rust/api/rows.dart';
import 'form_dialog.dart';

const _amount = TextInputType.numberWithOptions(decimal: true);

/// New project for an Indian client billed in rupees; a blank rate means fixed price
Future<ProjectDetail?> newProjectForm(BuildContext context, CompterApp app) {
  return FormDialog.show(
    context,
    title: 'New project',
    fields: const [
      FieldSpec('name', 'Name', hint: 'e.g. Website Revamp'),
      FieldSpec('client_name', 'Client', hint: 'e.g. Acme Pvt Ltd'),
      FieldSpec('client_gstin', 'Client GSTIN', hint: 'blank if unregistered'),
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
