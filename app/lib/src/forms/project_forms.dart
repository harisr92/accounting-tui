import 'package:flutter/material.dart';

import '../rust/api/error.dart';
import '../rust/api/app.dart';
import '../rust/api/clients.dart';
import '../rust/api/rows.dart';
import 'client_picker.dart';
import 'form_dialog.dart';

const _amount = TextInputType.numberWithOptions(decimal: true);

/// What projects and the profile can be billed in, as Rust names them
const currencies = ['INR', 'USD', 'GBP', 'EUR'];

/// New project, or [existing] edited with the form filled from it. The client can be picked
/// from those billed before, which fills in their details. A client with a country is abroad:
/// an export, billed in any currency. One in India without a GSTIN needs a state, the place of
/// supply; with a GSTIN the state is the GSTIN's. A blank rate is fixed price.
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
        InputField.name,
        'Name',
        hint: 'e.g. Website Revamp',
        initial: values?.name ?? '',
      ),
      FieldSpec(
        InputField.clientName,
        'Client',
        hint: 'e.g. Acme Pvt Ltd',
        initial: values?.clientName ?? '',
        pick: (context) => _pickClient(context, app),
        pickTooltip: 'Earlier clients',
      ),
      ...clientFields(
        gstin: values?.clientGstin,
        state: values?.clientState,
        country: values?.clientCountry,
        address: values?.clientAddress,
        currency: values?.currency,
      ),
      FieldSpec(
        InputField.hourlyRate,
        'Rate / hour',
        hint: 'blank for a fixed price',
        keyboardType: _amount,
        initial: values?.hourlyRate ?? '',
      ),
    ],
    submit: (values) {
      final input = ProjectInput(
        name: values[InputField.name]!,
        clientName: values[InputField.clientName]!,
        clientGstin: values[InputField.clientGstin],
        clientState: values[InputField.clientState],
        clientCountry: values[InputField.clientCountry],
        clientAddress: values[InputField.address]!.split('\n'),
        currency: values[InputField.currency],
        hourlyRate: values[InputField.hourlyRate],
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
        InputField.name,
        'Name',
        hint: 'e.g. Design',
        initial: values?.name ?? '',
      ),
      FieldSpec(
        InputField.dueDate,
        'Due date',
        date: true,
        initial: values?.dueDate ?? '',
      ),
      if (!project.hourly)
        FieldSpec(
          InputField.amount,
          'Amount',
          hint: 'e.g. 50,000',
          keyboardType: _amount,
          initial: values?.amount ?? '',
        ),
    ],
    submit: (values) {
      final input = MilestoneInput(
        name: values[InputField.name]!,
        dueDate: values[InputField.dueDate]!,
        amount: values[InputField.amount],
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

/// A client's details after their name, keyed as [projectForm] and the edit client form both
/// read them, so an [ApiError] names the same field in either
List<FieldSpec> clientFields({
  String? gstin,
  String? state,
  String? country,
  List<String>? address,
  String? currency,
}) {
  final states = gstStates();
  return [
    FieldSpec(
      InputField.clientGstin,
      'Client GSTIN',
      hint: 'blank if unregistered or abroad',
      initial: gstin ?? '',
    ),
    FieldSpec(
      InputField.clientState,
      'State',
      hint: 'India, no GSTIN: the place of supply',
      initial: state ?? '',
      options: ['', for (final state in states) state.code],
      optionLabels: {
        '': 'Not known',
        for (final state in states) state.code: state.label,
      },
      derive: (values) =>
          gstinState(gstin: values[InputField.clientGstin] ?? ''),
      deriveFrom: const [InputField.clientGstin],
      derivedHint: "From the client's GSTIN",
    ),
    FieldSpec(
      InputField.clientCountry,
      'Country',
      hint: 'abroad only, e.g. Germany',
      initial: country ?? '',
    ),
    FieldSpec(
      InputField.address,
      'Address',
      hint: 'one line each: street, city and PIN / postcode',
      initial: address?.join('\n') ?? '',
      multiline: true,
    ),
    FieldSpec(
      InputField.currency,
      'Currency',
      hint: 'anything but INR needs a client abroad',
      initial: currency ?? 'INR',
      options: currencies,
    ),
  ];
}

/// The client fields of [projectForm] filled from an earlier client, or cleared for a new one
Future<Map<InputField, String>?> _pickClient(
  BuildContext context,
  CompterApp app,
) async {
  final pick = await pickClient(context, app.clients());
  if (pick == null) return null;
  final client = pick.client;
  return {
    InputField.clientName: client?.name ?? '',
    InputField.clientGstin: client?.gstin ?? '',
    InputField.clientState: client?.state ?? '',
    InputField.clientCountry: client?.country ?? '',
    InputField.address: client?.address.join('\n') ?? '',
    InputField.currency: client?.currency ?? 'INR',
  };
}
