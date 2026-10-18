import 'package:flutter/material.dart';

import '../rust/api/error.dart';

/// One text field of a [FormDialog]. [key] is the name Rust uses in `ApiError.field`, so a
/// refused value is shown under the field that holds it.
class FieldSpec {
  const FieldSpec(
    this.key,
    this.label, {
    this.hint,
    this.keyboardType,
    this.initial = '',
  });

  final String key;
  final String label;
  final String? hint;
  final TextInputType? keyboardType;
  final String initial;
}

/// Every form in the app: text fields, Cancel and Save. Values are passed to [submit] as typed;
/// Rust parses and validates them. On a refusal the dialog stays open with the error, like
/// `Form::step` in the terminal app.
class FormDialog<T> extends StatefulWidget {
  const FormDialog({
    super.key,
    required this.title,
    required this.fields,
    required this.submit,
  });

  final String title;
  final List<FieldSpec> fields;
  final Future<T> Function(Map<String, String> values) submit;

  /// The submitted result, or `null` when cancelled
  static Future<T?> show<T>(
    BuildContext context, {
    required String title,
    required List<FieldSpec> fields,
    required Future<T> Function(Map<String, String> values) submit,
  }) {
    return showDialog<T>(
      context: context,
      builder: (_) =>
          FormDialog<T>(title: title, fields: fields, submit: submit),
    );
  }

  @override
  State<FormDialog<T>> createState() => _FormDialogState<T>();
}

class _FormDialogState<T> extends State<FormDialog<T>> {
  late final Map<String, TextEditingController> _controllers = {
    for (final field in widget.fields)
      field.key: TextEditingController(text: field.initial),
  };
  ApiError? _error;
  bool _saving = false;

  @override
  void dispose() {
    for (final controller in _controllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  Future<void> _save() async {
    setState(() {
      _saving = true;
      _error = null;
    });
    final values = {
      for (final entry in _controllers.entries) entry.key: entry.value.text,
    };
    try {
      final result = await widget.submit(values);
      if (mounted) Navigator.of(context).pop(result);
    } on ApiError catch (error) {
      if (mounted) setState(() => _error = error);
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final error = _error;
    final fieldKeys = widget.fields.map((f) => f.key).toSet();
    final general = error != null && !fieldKeys.contains(error.field)
        ? error.message
        : null;
    return AlertDialog(
      title: Text(widget.title),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (general != null)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Text(
                  general,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ),
            for (final field in widget.fields)
              Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: TextField(
                  controller: _controllers[field.key],
                  keyboardType: field.keyboardType,
                  decoration: InputDecoration(
                    labelText: field.label,
                    hintText: field.hint,
                    errorText: error?.field == field.key
                        ? error!.message
                        : null,
                  ),
                ),
              ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _saving ? null : () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: _saving ? null : _save,
          child: const Text('Save'),
        ),
      ],
    );
  }
}
