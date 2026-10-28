import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../onboarding.dart';
import '../ui/theme.dart';

typedef _Page = ({IconData icon, String title, String body});

const _pages = <_Page>[
  (
    icon: Icons.work_outline,
    title: 'Bill by milestone or by the hour',
    body:
        'Add a project for each client, split it into milestones and log '
        'your hours. compter keeps track of what is pending, invoiced and paid.',
  ),
  (
    icon: Icons.receipt_long_outlined,
    title: 'GST invoices, done right',
    body:
        'Tax, export and plain invoices with the right GST, as PDFs to share. '
        'Record payments with TDS, and export GSTR-1 when the quarter ends.',
  ),
  (
    icon: Icons.lock_outline,
    title: 'Your books stay on your phone',
    body:
        'No account, no sign-up and no ads. If you want, sync with compter on '
        'your computer through your own Google Drive.',
  ),
];

/// What compter does, in three pages, the first time it opens. Skipping counts as read.
class WelcomeScreen extends StatefulWidget {
  const WelcomeScreen({super.key});

  @override
  State<WelcomeScreen> createState() => _WelcomeScreenState();
}

class _WelcomeScreenState extends State<WelcomeScreen> {
  final _controller = PageController();
  var _page = 0;

  bool get _last => _page == _pages.length - 1;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Future<void> _finish() async {
    // Only the next start depends on it, so the app goes on if it isn't saved
    try {
      await welcomeSeen();
    } catch (_) {}
    if (mounted) context.go('/projects');
  }

  void _next() => _controller.nextPage(
    duration: const Duration(milliseconds: 300),
    curve: Curves.easeOut,
  );

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Scaffold(
      body: SafeArea(
        child: Column(
          children: [
            Align(
              alignment: Alignment.centerRight,
              child: Visibility(
                // Keeps its space, so the pages don't jump, but can't be tapped hidden
                maintainSize: true,
                maintainAnimation: true,
                maintainState: true,
                visible: !_last,
                child: TextButton(
                  onPressed: _finish,
                  child: const Text('Skip'),
                ),
              ),
            ),
            Expanded(
              child: PageView(
                controller: _controller,
                onPageChanged: (page) => setState(() => _page = page),
                children: [for (final page in _pages) _WelcomePage(page)],
              ),
            ),
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                for (var i = 0; i < _pages.length; i++)
                  AnimatedContainer(
                    duration: const Duration(milliseconds: 200),
                    margin: const EdgeInsets.all(4),
                    width: i == _page ? 20 : 8,
                    height: 8,
                    decoration: BoxDecoration(
                      color: i == _page
                          ? scheme.primary
                          : scheme.outlineVariant,
                      borderRadius: BorderRadius.circular(4),
                    ),
                  ),
              ],
            ),
            Padding(
              padding: const EdgeInsets.all(gutter),
              child: SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: _last ? _finish : _next,
                  child: Text(_last ? 'Get started' : 'Next'),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _WelcomePage extends StatelessWidget {
  const _WelcomePage(this.page);

  final _Page page;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return SingleChildScrollView(
      padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 24),
      child: Column(
        children: [
          const SizedBox(height: 32),
          CircleAvatar(
            radius: 48,
            backgroundColor: scheme.primaryContainer,
            child: Icon(page.icon, size: 48, color: scheme.onPrimaryContainer),
          ),
          const SizedBox(height: 32),
          Text(
            page.title,
            textAlign: TextAlign.center,
            style: theme.textTheme.headlineSmall,
          ),
          const SizedBox(height: 12),
          Text(
            page.body,
            textAlign: TextAlign.center,
            style: theme.textTheme.bodyLarge?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }
}
