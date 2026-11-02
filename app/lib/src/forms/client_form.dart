import 'package:flutter/material.dart';

import '../notice.dart';
import '../rust/api/app.dart';
import '../rust/api/clients.dart';
import '../rust/api/error.dart';
import '../subscription/paywall.dart';
import 'client_picker.dart';
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
        final answer = await _askAboutProjects(
          context,
          projects,
          title: 'Update open projects too?',
          body: 'Their next invoices will use the new details.',
        );
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

/// Call [client] by a new name. The old one stays on them as an alias, since their issued
/// invoices keep it. When open projects carry the old name, asks whether they take the new one.
Future<ClientDetail?> renameClientForm(
  BuildContext context,
  CompterApp app,
  ClientDetail client,
) {
  return FormDialog.show(
    context,
    title: 'Rename ${client.name}',
    fields: [
      FieldSpec(
        'client_name',
        'Name',
        hint: 'Issued invoices keep the old name, which stays as an alias',
        initial: client.name,
      ),
    ],
    submit: (values) async {
      final name = values['client_name']!;
      final projects = await app.clientProjectsToRename(
        id: client.id,
        name: name,
      );
      var updateProjects = false;
      if (projects.isNotEmpty) {
        if (!context.mounted) throw const ApiError(message: 'Not renamed');
        final answer = await _askAboutProjects(
          context,
          projects,
          title: 'Rename on open projects too?',
          body: 'Their next invoices will use the new name.',
        );
        if (answer == null) {
          throw const ApiError(
            message:
                'Not renamed: choose whether their open projects change too',
          );
        }
        updateProjects = answer;
      }
      return app.renameClient(
        id: client.id,
        name: name,
        updateProjects: updateProjects,
      );
    },
  );
}

/// Merge [client], a duplicate, into another client picked from the rest, whose details are
/// kept; [client]'s names become aliases. Always asks first, as a merge can't be undone, and
/// whether [client]'s open projects take the kept details. The client kept, or `null` when
/// cancelled or refused (the refusal is shown).
Future<ClientDetail?> mergeClientForm(
  BuildContext context,
  CompterApp app,
  ClientDetail client,
) async {
  if (!await requireWrite(context) || !context.mounted) return null;
  final others = app.clients().then(
    (all) => [
      for (final c in all)
        if (c.id != client.id) c,
    ],
  );
  final picked = await pickClient(
    context,
    others,
    title: 'Merge ${client.name} into',
    allowNew: false,
  );
  final into = picked?.client;
  if (into == null || !context.mounted) return null;
  try {
    final projects = await app.clientProjectsToMerge(
      from: client.id,
      into: into.id,
    );
    if (!context.mounted) return null;
    final keep =
        "Merge ${client.name} into ${into.name}, keeping ${into.name}'s "
        'details. ${client.name} becomes an alias.';
    bool? updateProjects;
    if (projects.isEmpty) {
      final yes = await confirm(
        context,
        title: 'Merge clients?',
        lines: [keep, 'Issued invoices never change.'],
        yes: 'Merge',
      );
      updateProjects = yes ? false : null;
    } else {
      updateProjects = await _askAboutProjects(
        context,
        projects,
        title: 'Merge into ${into.name}?',
        body:
            '$keep These open projects can take the merged name and details too.',
        only: 'Clients only',
        update: 'Update projects',
      );
    }
    if (updateProjects == null) return null;
    return await app.mergeClient(
      from: client.id,
      into: into.id,
      updateProjects: updateProjects,
    );
  } on ApiError catch (error) {
    if (context.mounted) notify(context, error.message, error: true);
    return null;
  }
}

/// `true` to update [projects] too, `false` for the client only, `null` when dismissed
Future<bool?> _askAboutProjects(
  BuildContext context,
  List<String> projects, {
  required String title,
  required String body,
  String only = 'Client only',
  String update = 'Update projects',
}) {
  return showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(title),
      content: Text(
        '${projects.join(', ')}\n\n$body Issued invoices never change.',
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context, false),
          child: Text(only),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(context, true),
          child: Text(update),
        ),
      ],
    ),
  );
}
