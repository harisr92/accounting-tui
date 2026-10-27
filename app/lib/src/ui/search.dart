import 'package:flutter/material.dart';

import 'empty_state.dart';
import 'glass.dart';
import 'large_title.dart';

/// Words a list is narrowed to, like the terminal's `/` search: a row matches when every word
/// appears in one of its fields, ignoring case. Blank shows everything.
class SearchQuery {
  SearchQuery(String text)
    : text = text.trim(),
      _words = text
          .toLowerCase()
          .split(RegExp(r'\s+'))
          .where((word) => word.isNotEmpty)
          .toList();

  /// What was typed, trimmed, for showing back
  final String text;
  final List<String> _words;

  bool get isEmpty => _words.isEmpty;

  bool matches(List<String> fields) {
    final lower = fields.map((field) => field.toLowerCase()).toList();
    return _words.every((word) => lower.any((field) => field.contains(word)));
  }

  /// The rows that match, in order, [fields] giving each row's searchable text
  List<T> apply<T>(List<T> rows, List<String> Function(T row) fields) =>
      isEmpty ? rows : rows.where((row) => matches(fields(row))).toList();
}

/// A [TitleBar] with a search button that swaps the bar for a search box. Closing the box
/// clears what was typed.
class SearchAppBar extends StatelessWidget implements PreferredSizeWidget {
  const SearchAppBar({
    super.key,
    required this.title,
    required this.hint,
    required this.controller,
    required this.searching,
    required this.onOpen,
    required this.onClose,
    this.actions = const [],
  });

  final String title;

  /// What can be searched, e.g. 'Name or client'
  final String hint;
  final TextEditingController controller;
  final bool searching;
  final VoidCallback onOpen;
  final VoidCallback onClose;

  /// Shown before the search button while the box is closed
  final List<Widget> actions;

  @override
  Size get preferredSize => const Size.fromHeight(kToolbarHeight);

  @override
  Widget build(BuildContext context) {
    if (!searching) {
      return TitleBar(
        title: title,
        actions: [
          ...actions,
          IconButton(
            tooltip: 'Search',
            icon: const Icon(Icons.search),
            onPressed: onOpen,
          ),
        ],
      );
    }
    return AppBar(
      flexibleSpace: const Glass(),
      title: TextField(
        controller: controller,
        autofocus: true,
        textInputAction: TextInputAction.search,
        decoration: InputDecoration(hintText: hint, border: InputBorder.none),
      ),
      actions: [
        IconButton(
          tooltip: 'Close search',
          icon: const Icon(Icons.close),
          onPressed: onClose,
        ),
      ],
    );
  }
}

/// Search state for a list screen: the box's text and whether it is open. The screen rebuilds
/// as the text changes.
mixin Searching<W extends StatefulWidget> on State<W> {
  final searchController = TextEditingController();
  bool searching = false;

  SearchQuery get query => SearchQuery(searchController.text);

  @override
  void initState() {
    super.initState();
    searchController.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    searchController.dispose();
    super.dispose();
  }

  SearchAppBar searchAppBar({
    required String title,
    required String hint,
    List<Widget> actions = const [],
  }) => SearchAppBar(
    title: title,
    hint: hint,
    actions: actions,
    controller: searchController,
    searching: searching,
    onOpen: () => setState(() => searching = true),
    onClose: () => setState(() {
      searching = false;
      searchController.clear();
    }),
  );
}

/// Shown instead of the rows when a search matches none of them
Widget noMatches(SearchQuery query) => EmptyState(
  icon: Icons.search_off,
  message: 'Nothing matches “${query.text}”',
);
