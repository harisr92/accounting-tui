import 'package:flutter/material.dart';

import '../rust/api/app.dart';
import '../rust/api/clients.dart';
import '../rust/api/error.dart';
import 'form_dialog.dart';
import 'project_forms.dart';

/// Correct [client]'s GSTIN, place, address or currency. Their name stays: their projects and
/// invoices are filed under it. When open projects would change, asks whether they take the
/// new details too; issued invoices never change.
Future<ClientDetail?> clientForm(
  BuildContext context,
  CompterApp app,
  ClientDetail client,
) {
  final values = client.edit;
  return FormDialog.show(
    context,
    title: 'Edit ${client.name}',
    fields: clientFields(
      gstin: values.gstin,
      state: values.state,
      country: values.country,
      address: values.address,
      currency: values.currency,
    ),
    submit: (values) async {
      final input = ClientInput(
        gstin: values['client_gstin'],
        state: values['client_state'],
        country: values['client_country'],
        address: values['address']!.split('\n'),
        currency: values['currency'],
      );
      final projects = await app.clientProjectsToUpdate(
        id: client.id,
        input: input,
      );
      var updateProjects = false;
      if (projects.isNotEmpty) {
        if (!context.mounted) throw const ApiError(message: 'Not saved');
        final answer = await _askAboutProjects(context, projects);
        // Dismissed: back to the form with what was typed, as Esc does in the terminal
        if (answer == null) {
          throw const ApiError(
            message: 'Not saved: choose whether their open projects change too',
          );
        }
        updateProjects = answer;
      }
      return app.editClient(
        id: client.id,
        input: input,
        updateProjects: updateProjects,
      );
    },
  );
}

/// `true` to update [projects] too, `false` for the client only, `null` when dismissed
Future<bool?> _askAboutProjects(BuildContext context, List<String> projects) {
  return showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      title: const Text('Update open projects too?'),
      content: Text(
        '${projects.join(', ')}\n\nTheir next invoices will use the new details. '
        'Issued invoices never change.',
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context, false),
          child: const Text('Client only'),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, true),
          child: const Text('Update projects'),
        ),
      ],
    ),
  );
}
