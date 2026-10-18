import 'package:flutter/material.dart';

import '../rust/api/error.dart';
import '../ui/theme.dart';

/// One field of a [FormDialog]. [key] is the name Rust uses in `ApiError.field`, so a refused
/// value is shown under the field that holds it. With [options] the field is a dropdown of
/// those values instead of free text.
class FieldSpec {
  const FieldSpec(
    this.key,
    this.label, {
    this.hint,
    this.keyboardType,
    this.initial = '',
    this.options,
  });

  final String key;
  final String label;
  final String? hint;
  final TextInputType? keyboardType;
  final String initial;
  final List<String>? options;
}

/// Every form in the app, as a bottom sheet: its fields, then Cancel and the one filled action
/// at the foot, within thumb's reach. Values are passed to [submit] as typed; Rust parses and
/// validates them. On a refusal the sheet stays open with the error, like `Form::step` in the
/// terminal app.
class FormDialog<T> extends StatefulWidget {
  const FormDialog({
    super.key,
    required this.title,
    required this.fields,
    required this.submit,
    this.submitLabel = 'Save',
  });

  final String title;
  final List<FieldSpec> fields;
  final Future<T> Function(Map<String, String> values) submit;

  /// `Preview` when a confirmation follows, `Save` otherwise
  final String submitLabel;

  /// The submitted result, or `null` when cancelled
  static Future<T?> show<T>(
    BuildContext context, {
    required String title,
    required List<FieldSpec> fields,
    required Future<T> Function(Map<String, String> values) submit,
    String submitLabel = 'Save',
  }) {
    return showModalBottomSheet<T>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      // Over the navigation bar too: a form is the only thing to act on while open
      useRootNavigator: true,
      builder: (_) => FormDialog<T>(
        title: title,
        fields: fields,
        submit: submit,
        submitLabel: submitLabel,
      ),
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
    final theme = Theme.of(context);
    return Padding(
      // Above the keyboard while typing
      padding: EdgeInsets.only(bottom: MediaQuery.viewInsetsOf(context).bottom),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(gutter, 0, gutter, 16),
            child: Text(widget.title, style: theme.textTheme.titleLarge),
          ),
          Flexible(
            child: SingleChildScrollView(
              padding: const EdgeInsets.symmetric(horizontal: gutter),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (general != null)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 16),
                      child: Text(
                        general,
                        style: theme.textTheme.bodyMedium?.copyWith(
                          color: theme.colorScheme.error,
                        ),
                      ),
                    ),
                  for (final field in widget.fields)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 16),
                      child: _input(
                        field,
                        InputDecoration(
                          labelText: field.label.isEmpty ? null : field.label,
                          // Under the field, so it still shows once a value is in it
                          helperText: field.hint,
                          errorText: error?.field == field.key
                              ? error!.message
                              : null,
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ),
          const Divider(),
          Padding(
            padding: const EdgeInsets.fromLTRB(gutter, 12, gutter, 12),
            child: Row(
              children: [
                TextButton(
                  onPressed: _saving ? null : () => Navigator.of(context).pop(),
                  child: const Text('Cancel'),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: FilledButton(
                    onPressed: _saving ? null : _save,
                    child: Text(widget.submitLabel),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _input(FieldSpec field, InputDecoration decoration) {
    final controller = _controllers[field.key]!;
    final options = field.options;
    if (options == null) {
      return TextField(
        controller: controller,
        keyboardType: field.keyboardType,
        decoration: decoration,
      );
    }
    return DropdownButtonFormField<String>(
      initialValue: options.contains(controller.text) ? controller.text : null,
      items: [
        for (final option in options)
          DropdownMenuItem(value: option, child: Text(option)),
      ],
      onChanged: (value) => controller.text = value ?? '',
      decoration: decoration,
    );
  }
}
