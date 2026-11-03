import 'package:flutter/material.dart';

import '../rust/api/error.dart';
import '../ui/error_toast.dart';
import '../subscription/paywall.dart';
import '../ui/theme.dart';

/// Fills in some of a form's fields at once, by their keys; `null` when nothing was picked
typedef FieldPicker =
    Future<Map<InputField, String>?> Function(BuildContext context);

/// One field of a [FormDialog]. [key] is the [InputField] Rust names in `ApiError.field`, so a
/// refused value is shown under the field that holds it, and a key renamed on one side only
/// doesn't build. With [options] the field is a dropdown of
/// those values instead of free text, each shown as its entry in [optionLabels] if it has one.
///
/// [pick] adds a button to a text field that fills in other fields too, e.g. a client's
/// details from an earlier client. [derive] works the field out from the others whenever one
/// named in [deriveFrom] changes: while it gives a value, the field shows it, is locked with
/// [derivedHint] under it, and is submitted as shown, as the terminal does, so a client is
/// saved the same way from either.
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

  final InputField key;
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
  final String? Function(Map<InputField, String> values)? derive;
  final List<InputField> deriveFrom;
  final String? derivedHint;
}

/// Every form in the app, as a bottom sheet: its fields, then Cancel and the one filled action
/// at the foot, within thumb's reach. Values are passed to [submit] as typed; Rust parses and
/// validates them. On a refusal the sheet stays open, like `Form::step` in the terminal app: an
/// error about a field shows under it until that field is edited, and any other shows at the
/// foot of the form as an [ErrorToast] the user closes.
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
  final Future<T> Function(Map<InputField, String> values) submit;

  /// `Preview` when a confirmation follows, `Save` otherwise
  final String submitLabel;

  /// The submitted result, or `null` when cancelled. A form that [writes] a record asks for a
  /// subscription first ([requireWrite]); one that only exports doesn't.
  static Future<T?> show<T>(
    BuildContext context, {
    required String title,
    required List<FieldSpec> fields,
    required Future<T> Function(Map<InputField, String> values) submit,
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
  late final Map<InputField, TextEditingController> _controllers = {
    for (final field in widget.fields)
      field.key: TextEditingController(text: field.initial),
  };

  /// The refusal about one of the fields, with what that field held when it was refused
  (ApiError, String)? _fieldError;

  /// A refusal about none of the fields, until closed
  String? _toast;
  bool _saving = false;

  /// Fields [FieldSpec.derive] currently works out, so they are locked, each with what it
  /// held before, put back once it no longer is
  final Map<InputField, String> _derived = {};

  Map<InputField, String> get _values => {
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
    for (final entry in _controllers.entries) {
      entry.value.addListener(() => _edited(entry.key));
    }
  }

  /// A field's error goes once its text changes; moving the cursor doesn't count
  void _edited(InputField key) {
    final refused = _fieldError;
    if (refused == null || refused.$1.field != key) return;
    if (_controllers[key]?.text == refused.$2) return;
    setState(() => _fieldError = null);
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
      _fieldError = null;
      _toast = null;
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
      _fieldError = null;
      _toast = null;
    });
    try {
      final result = await widget.submit(_values);
      if (mounted) Navigator.of(context).pop(result);
    } on ApiError catch (error) {
      if (mounted) setState(() => _refused(error));
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  /// Under its field when [error] names one of this form's, else as the toast
  void _refused(ApiError error) {
    final field = _controllers[error.field];
    if (field == null) {
      _toast = error.message;
    } else {
      _fieldError = (error, field.text);
    }
  }

  @override
  Widget build(BuildContext context) {
    final error = _fieldError?.$1;
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
            child: WithErrorToast(
              message: _toast,
              onClose: () => setState(() => _toast = null),
              child: SingleChildScrollView(
                padding: const EdgeInsets.symmetric(horizontal: gutter),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
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
