import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../appearance.dart';
import '../compter.dart';
import '../forms/profile_form.dart';
import '../notice.dart';
import '../rust/api/profile.dart';
import '../ui/action_sheet.dart';
import '../ui/glass.dart';
import '../ui/large_title.dart';
import '../ui/section.dart';
import '../ui/theme.dart';
import 'sync_section.dart';

/// The business profile printed on invoices, and syncing with a computer, like the
/// terminal's Settings tab
class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final profile = ref.watch(profileProvider);
    Future<void> edit() async {
      final saved = await profileForm(context, ref.read(compterProvider));
      if (saved == null || !context.mounted) return;
      ref.invalidate(profileProvider);
      notify(context, 'Business details saved');
    }

    return LargeTitleScope(
      child: Scaffold(
        extendBodyBehindAppBar: true,
        appBar: const TitleBar(title: 'Settings'),
        body: profile.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (error, _) => Center(child: Text('$error')),
          data: (profile) => ScreenList(
            bottom: gutter,
            children: [
              const LargeTitle(
                icon: Icons.settings,
                title: 'Settings',
                description:
                    'Your business as invoices print it, the theme, and syncing',
              ),
              SectionHeader(
                'Business profile',
                trailing: profile == null
                    ? null
                    : IconButton(
                        tooltip: 'Edit business details',
                        icon: const Icon(Icons.edit_outlined),
                        onPressed: edit,
                      ),
              ),
              if (profile == null)
                ListTile(
                  leading: const Icon(Icons.add_business_outlined),
                  title: const Text('Add business details'),
                  subtitle: const Text('Needed before you can invoice'),
                  onTap: edit,
                )
              else
                ..._profileRows(profile),
              const _AppearanceSection(),
              const SyncSection(),
            ],
          ),
        ),
      ),
    );
  }

  List<Widget> _profileRows(ProfileRow profile) => [
    _Field('Name', profile.name),
    if (profile.address.isNotEmpty)
      _Field('Address', profile.address.join('\n')),
    _Field('GSTIN', profile.gstin ?? 'Not registered: invoices carry no GST'),
    _Field('SAC', [profile.sacCode, ?profile.sacDescription].join(' · ')),
    _Field('Invoice prefix', profile.invoicePrefix),
    if (profile.lutArn != null) _Field('LUT ARN', profile.lutArn!),
    if (profile.terms.isNotEmpty) _Field('Terms', profile.terms.join('\n')),
    _Field('Currency', profile.currency),
    _Field('44ADA basis', profile.incomeBasis),
  ];
}

/// The theme this phone draws the app in
class _AppearanceSection extends ConsumerWidget {
  const _AppearanceSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final current = ref.watch(appThemeProvider);
    Future<void> choose() async {
      final picked = await showActionSheet(
        context,
        title: 'Theme',
        subtitle: 'For this phone only',
        actions: [
          for (final theme in AppTheme.values)
            (
              icon: theme == current
                  ? Icons.radio_button_checked
                  : Icons.radio_button_unchecked,
              label: theme.label,
              value: theme,
            ),
        ],
      );
      if (picked == null) return;
      try {
        await ref.read(appThemeProvider.notifier).choose(picked);
      } catch (error) {
        // Drawn in it now, but the next start won't remember it
        if (context.mounted) {
          notify(context, 'Theme not saved: $error', error: true);
        }
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const SectionHeader('Appearance'),
        ListTile(
          leading: const Icon(Icons.palette_outlined),
          title: const Text('Theme'),
          subtitle: Text(current.label),
          trailing: const Icon(Icons.chevron_right),
          onTap: choose,
        ),
      ],
    );
  }
}

/// A setting's label over its value
class _Field extends StatelessWidget {
  const _Field(this.label, this.value);

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: gutter, vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            label,
            style: theme.textTheme.labelMedium?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 2),
          Text(value, style: theme.textTheme.bodyLarge),
        ],
      ),
    );
  }
}
