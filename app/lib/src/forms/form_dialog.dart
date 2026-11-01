import 'package:flutter/material.dart';

import '../rust/api/error.dart';
import '../subscription/paywall.dart';
import '../ui/theme.dart';

/// Fills in some of a form's fields at once, by their keys; `null` when nothing was picked
typedef FieldPicker =
    Future<Map<String, String>?> Function(BuildContext context);

/// One field of a [FormDialog]. [key] is the name Rust uses in `ApiError.field`, so a refused
/// value is shown under the field that holds it. With [options] the field is a dropdown of
/// those values instead of free text, each shown as its entry in [optionLabels] if it has one.
///
/// [pick] adds a button to a text field that fills in other fields too, e.g. a client's
/// details from an earlier client. [derive] works the field out from the others whenever one
/// named in [deriveFrom] changes: while it gives a value, the field shows it, is locked with [derivedHint] under it,
/// and is submitted blank, since Rust works it out the same way.
class FieldSpec {
  const FieldSpec(
    this.key,
    this.label, {
    this.hint,
    this.keyboardType,
    this.initial = '',
    this.options,
    this.optionLabels,
    this.date = false,
    this.multiline = false,
    this.pick,
    this.pickTooltip,
    this.derive,
    this.deriveFrom = const [],
    this.derivedHint,
  });

  final String key;
  final String label;
  final String? hint;
  final TextInputType? keyboardType;
  final String initial;
  final List<String>? options;
  final Map<String, String>? optionLabels;
  final bool date;
  final bool multiline;
  final FieldPicker? pick;
  final String? pickTooltip;
  final String? Function(Map<String, String> values)? derive;
  final List<String> deriveFrom;
  final String? derivedHint;
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

  /// The submitted result, or `null` when cancelled. A form that [writes] a record asks for a
  /// subscription first ([requireWrite]); one that only exports doesn't.
  static Future<T?> show<T>(
    BuildContext context, {
    required String title,
    required List<FieldSpec> fields,
    required Future<T> Function(Map<String, String> values) submit,
    String submitLabel = 'Save',
    bool writes = true,
  }) async {
    if (writes && !await requireWrite(context)) return null;
    if (!context.mounted) return null;
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

  /// Fields [FieldSpec.derive] currently works out, so they are locked, each with what it
  /// held before, put back once it no longer is
  final Map<String, String> _derived = {};

  Map<String, String> get _values => {
    for (final entry in _controllers.entries) entry.key: entry.value.text,
  };

  @override
  void initState() {
    super.initState();
    final sources = {
      for (final field in widget.fields)
        if (field.derive != null) ...field.deriveFrom,
    };
    for (final key in sources) {
      _controllers[key]?.addListener(_rederive);
    }
    if (widget.fields.any((f) => f.derive != null)) _rederive();
  }

  /// Work out every derived field again from the fields it is derived from
  void _rederive() {
    final values = _values;
    var changed = false;
    for (final field in widget.fields) {
      final derive = field.derive;
      if (derive == null) continue;
      final value = derive(values);
      final controller = _controllers[field.key]!;
      if (value == null) {
        final before = _derived.remove(field.key);
        if (before != null) {
          changed = true;
          controller.text = before;
        }
      } else {
        if (!_derived.containsKey(field.key)) {
          _derived[field.key] = controller.text;
          changed = true;
        }
        if (controller.text != value) controller.text = value;
      }
    }
    if (changed && mounted) setState(() {});
  }

  /// Fill in what [pick] returns
  Future<void> _pick(FieldPicker pick) async {
    final picked = await pick(context);
    if (picked == null || !mounted) return;
    setState(() {
      for (final entry in picked.entries) {
        // A field derived from another keeps showing what it's derived from; the picked
        // value is what it goes back to once the derivation stops
        if (_derived.containsKey(entry.key)) {
          _derived[entry.key] = entry.value;
        } else {
          _controllers[entry.key]?.text = entry.value;
        }
      }
      _error = null;
    });
    _rederive();
  }

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
      for (final entry in _values.entries)
        entry.key: _derived.containsKey(entry.key) ? '' : entry.value,
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
                          helperText: _derived.containsKey(field.key)
                              ? field.derivedHint ?? field.hint
                              : field.hint,
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
    if (field.date) {
      return TextField(
        controller: controller,
        readOnly: true,
        decoration: decoration.copyWith(
          hintText: 'Today',
          suffixIcon: const Icon(Icons.calendar_today_outlined),
        ),
        onTap: () => _pickDate(controller),
      );
    }
    final locked = _derived.containsKey(field.key);
    if (options == null) {
      final pick = field.pick;
      return TextField(
        controller: controller,
        readOnly: locked,
        keyboardType: field.multiline
            ? TextInputType.multiline
            : field.keyboardType,
        minLines: field.multiline ? 3 : 1,
        maxLines: field.multiline ? null : 1,
        decoration: pick == null
            ? decoration
            : decoration.copyWith(
                suffixIcon: IconButton(
                  icon: const Icon(Icons.contacts_outlined),
                  tooltip: field.pickTooltip,
                  onPressed: () => _pick(pick),
                ),
              ),
      );
    }
    final labels = field.optionLabels ?? const {};
    return DropdownButtonFormField<String>(
      // Built again when a pick or a derivation sets the value
      key: ValueKey('${field.key}:${controller.text}'),
      initialValue: options.contains(controller.text) ? controller.text : null,
      isExpanded: true,
      items: [
        for (final option in options)
          DropdownMenuItem(
            value: option,
            child: Text(
              labels[option] ?? option,
              overflow: TextOverflow.ellipsis,
            ),
          ),
      ],
      onChanged: locked ? null : (value) => controller.text = value ?? '',
      decoration: decoration,
    );
  }

  Future<void> _pickDate(TextEditingController controller) async {
    final today = DateUtils.dateOnly(DateTime.now());
    final picked = await showDatePicker(
      context: context,
      initialDate: DateTime.tryParse(controller.text) ?? today,
      firstDate: DateTime(2000),
      lastDate: DateTime(today.year + 10),
    );
    if (picked != null) controller.text = _isoDate(picked);
  }
}

/// `YYYY-MM-DD`, as Rust parses dates
String _isoDate(DateTime date) =>
    '${date.year.toString().padLeft(4, '0')}-'
    '${date.month.toString().padLeft(2, '0')}-'
    '${date.day.toString().padLeft(2, '0')}';
