import 'package:flutter/material.dart';

import '../rust/api/error.dart';
import 'theme.dart';

const _amountKeyboard = TextInputType.numberWithOptions(decimal: true);

/// One row of a [showCheckList] sheet. With an [amount] the row takes an amount, typed under it
/// and prefilled with this; a row that is not [enabled] is shown but can't be picked.
class CheckItem {
  const CheckItem({
    required this.key,
    required this.title,
    this.subtitle,
    this.checked = false,
    this.amount,
    this.fill,
    this.enabled = true,
  });

  final String key;
  final String title;
  final String? subtitle;
  final bool checked;
  final String? amount;

  /// What an empty amount becomes when the row is picked, e.g. all that is left
  final String? fill;
  final bool enabled;
}

/// Pick any number of [items], with "Select all" above them, like the terminal's `Checklist`.
/// Returns the keys picked, in the order listed, each with the amount typed on it (empty for a
/// row without one); `null` when cancelled. [submitLabel] stays disabled until something is
/// picked. [note] is shown under the title. [check] runs before the sheet closes: an
/// [ApiError] keeps it open, under the row whose key is its field after [fieldPrefix], or at
/// the top for anything else.
Future<List<(String, String)>?> showCheckList(
  BuildContext context, {
  required String title,
  required List<CheckItem> items,
  String submitLabel = 'Next',
  String? amountLabel,
  String? note,
  Future<void> Function(List<(String, String)> picked)? check,
  String fieldPrefix = '',
}) {
  return showModalBottomSheet<List<(String, String)>>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    // Over the navigation bar too, like every form
    useRootNavigator: true,
    builder: (_) => _CheckList(
      title: title,
      items: items,
      submitLabel: submitLabel,
      amountLabel: amountLabel,
      note: note,
      check: check,
      fieldPrefix: fieldPrefix,
    ),
  );
}

class _CheckList extends StatefulWidget {
  const _CheckList({
    required this.title,
    required this.items,
    required this.submitLabel,
    this.amountLabel,
    this.note,
    this.check,
    this.fieldPrefix = '',
  });

  final String title;
  final List<CheckItem> items;
  final String submitLabel;
  final String? amountLabel;
  final String? note;
  final Future<void> Function(List<(String, String)> picked)? check;
  final String fieldPrefix;

  @override
  State<_CheckList> createState() => _CheckListState();
}

class _CheckListState extends State<_CheckList> {
  late final Set<String> _picked = {
    for (final item in widget.items)
      if (item.checked && item.enabled) item.key,
  };
  late final Map<String, TextEditingController> _amounts = {
    for (final item in widget.items)
      if (item.amount != null)
        item.key: TextEditingController(text: item.amount),
  };

  ApiError? _error;
  bool _checking = false;

  Iterable<CheckItem> get _enabled => widget.items.where((i) => i.enabled);

  /// The row [_error] is about, if it names one
  String? get _errorKey {
    final field = _error?.field;
    if (field == null || !field.startsWith(widget.fieldPrefix)) return null;
    final key = field.substring(widget.fieldPrefix.length);
    return widget.items.any((i) => i.key == key) ? key : null;
  }

  @override
  void dispose() {
    for (final controller in _amounts.values) {
      controller.dispose();
    }
    super.dispose();
  }

  /// `true` when every row that can be picked is, `false` when none is, `null` in between
  bool? get _all {
    final picked = _enabled.where((i) => _picked.contains(i.key)).length;
    if (picked == 0) return false;
    return picked == _enabled.length ? true : null;
  }

  /// Pick [item], filling an empty amount with its [CheckItem.fill]
  void _pick(CheckItem item) {
    _picked.add(item.key);
    final amount = _amounts[item.key];
    final fill = item.fill;
    if (amount != null && fill != null && amount.text.isEmpty) {
      amount.text = fill;
    }
  }

  void _toggleAll() {
    setState(() {
      if (_all == true) {
        _picked.clear();
      } else {
        _enabled.forEach(_pick);
      }
    });
  }

  Future<void> _submit() async {
    final picked = [
      for (final item in widget.items)
        if (_picked.contains(item.key))
          (item.key, _amounts[item.key]?.text ?? ''),
    ];
    final check = widget.check;
    if (check != null) {
      setState(() {
        _checking = true;
        _error = null;
      });
      try {
        await check(picked);
      } on ApiError catch (error) {
        if (mounted) setState(() => _error = error);
        return;
      } catch (error) {
        // Anything else still shows, at the top, rather than leaving Next doing nothing
        if (mounted) setState(() => _error = ApiError(message: '$error'));
        return;
      } finally {
        if (mounted) setState(() => _checking = false);
      }
    }
    if (mounted) Navigator.of(context).pop(picked);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      // Above the keyboard while typing an amount
      padding: EdgeInsets.only(bottom: MediaQuery.viewInsetsOf(context).bottom),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(gutter, 0, gutter, 8),
            child: Text(widget.title, style: theme.textTheme.titleLarge),
          ),
          if (widget.note case final note?)
            Padding(
              padding: const EdgeInsets.fromLTRB(gutter, 0, gutter, 8),
              child: Text(note, style: theme.textTheme.bodyMedium),
            ),
          if (_error case final error? when _errorKey == null)
            Padding(
              padding: const EdgeInsets.fromLTRB(gutter, 0, gutter, 8),
              child: Text(
                error.message,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
            ),
          CheckboxListTile(
            title: const Text('Select all'),
            tristate: true,
            value: _all,
            onChanged: _enabled.isEmpty ? null : (_) => _toggleAll(),
            controlAffinity: ListTileControlAffinity.leading,
          ),
          const Divider(height: 1),
          Flexible(
            child: SingleChildScrollView(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [for (final item in widget.items) _row(item)],
              ),
            ),
          ),
          const Divider(),
          Padding(
            padding: const EdgeInsets.fromLTRB(gutter, 12, gutter, 12),
            child: Row(
              children: [
                TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: const Text('Cancel'),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: FilledButton(
                    onPressed: _picked.isEmpty || _checking ? null : _submit,
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

  Widget _row(CheckItem item) {
    final picked = _picked.contains(item.key);
    final amount = _amounts[item.key];
    final subtitle = item.subtitle;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        CheckboxListTile(
          title: Text(item.title),
          subtitle: subtitle == null ? null : Text(subtitle),
          value: picked,
          onChanged: item.enabled
              ? (value) => setState(() {
                  if (value == true) {
                    _pick(item);
                  } else {
                    _picked.remove(item.key);
                  }
                })
              : null,
          controlAffinity: ListTileControlAffinity.leading,
        ),
        if (amount != null && picked)
          Padding(
            padding: const EdgeInsets.fromLTRB(72, 0, gutter, 12),
            child: TextField(
              controller: amount,
              keyboardType: _amountKeyboard,
              decoration: InputDecoration(
                labelText: widget.amountLabel,
                errorText: _errorKey == item.key ? _error?.message : null,
              ),
            ),
          ),
      ],
    );
  }
}
