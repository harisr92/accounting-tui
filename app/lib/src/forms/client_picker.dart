import 'package:flutter/material.dart';

import '../rust/api/clients.dart';
import '../ui/record_tile.dart';
import '../ui/search.dart';
import '../ui/theme.dart';

/// What was picked from [pickClient]: an earlier client, or `null` for a new one
typedef ClientPick = ({ClientChoiceRow? client});

/// Pick one of [clients], those billed before, or (with [allowNew]) a new client; `null` when
/// dismissed. A search box narrows a long list by name, GSTIN or place.
Future<ClientPick?> pickClient(
  BuildContext context,
  Future<List<ClientChoiceRow>> clients, {
  String title = 'Client',
  bool allowNew = true,
}) {
  return showModalBottomSheet<ClientPick>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    builder: (_) =>
        _ClientPicker(clients: clients, title: title, allowNew: allowNew),
  );
}

class _ClientPicker extends StatefulWidget {
  const _ClientPicker({
    required this.clients,
    required this.title,
    required this.allowNew,
  });

  final Future<List<ClientChoiceRow>> clients;
  final String title;
  final bool allowNew;

  @override
  State<_ClientPicker> createState() => _ClientPickerState();
}

class _ClientPickerState extends State<_ClientPicker> {
  final _search = TextEditingController();

  @override
  void dispose() {
    _search.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.viewInsetsOf(context).bottom),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(gutter, 0, gutter, 8),
            child: Text(widget.title, style: theme.textTheme.titleLarge),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: gutter),
            child: TextField(
              controller: _search,
              decoration: const InputDecoration(
                hintText: 'Name, GSTIN or place',
                prefixIcon: Icon(Icons.search),
              ),
              onChanged: (_) => setState(() {}),
            ),
          ),
          if (widget.allowNew) ...[
            ListTile(
              leading: const Icon(Icons.person_add_alt_outlined),
              title: const Text('New client'),
              onTap: () => Navigator.of(context).pop((client: null)),
            ),
            const Divider(height: 1),
          ],
          Flexible(
            child: FutureBuilder(
              future: widget.clients,
              builder: (context, snapshot) {
                if (snapshot.hasError) {
                  return _note(theme, "Earlier clients couldn't be read");
                }
                final clients = snapshot.data;
                if (clients == null) {
                  return const Padding(
                    padding: EdgeInsets.all(24),
                    child: Center(child: CircularProgressIndicator()),
                  );
                }
                if (clients.isEmpty) {
                  return _note(theme, 'No earlier clients yet');
                }
                final shown = SearchQuery(
                  _search.text,
                ).apply(clients, (c) => [c.name, c.summary]);
                return ListView(
                  shrinkWrap: true,
                  children: [
                    for (final client in shown)
                      RecordTile(
                        title: client.name,
                        supporting: client.summary.isEmpty
                            ? null
                            : client.summary,
                        onTap: () =>
                            Navigator.of(context).pop((client: client)),
                      ),
                  ],
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}

Widget _note(ThemeData theme, String text) => Padding(
  padding: const EdgeInsets.all(gutter),
  child: Text(text, style: theme.textTheme.bodyMedium),
);
