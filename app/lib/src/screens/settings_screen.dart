import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../about.dart';
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
                    'Your business as invoices print it, the theme, syncing, and support',
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
              const _AboutSection(),
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

/// Telling others about compter, rating it, and reporting what could be better
class _AboutSection extends ConsumerWidget {
  const _AboutSection();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final store = ref.watch(storefrontProvider);
    final version = ref.watch(appVersionProvider).value;

    Future<void> go(
      Future<bool> Function() open, {
      String failed = 'Couldn’t open the link',
    }) async {
      var opened = false;
      try {
        opened = await open();
      } catch (_) {}
      if (!opened && context.mounted) {
        notify(context, failed, error: true);
      }
    }

    final shareText = store.shareText;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const SectionHeader('About'),
        if (shareText != null)
          Builder(
            // Its own context, so the iPad's share popover points at this row
            builder: (row) => ListTile(
              leading: const Icon(Icons.share_outlined),
              title: const Text('Share compter'),
              subtitle: const Text('Send the link to a fellow freelancer'),
              onTap: () {
                final box = row.findRenderObject() as RenderBox?;
                store.share(
                  shareText,
                  box == null
                      ? null
                      : box.localToGlobal(Offset.zero) & box.size,
                );
              },
            ),
          ),
        if (store.storeUrl != null)
          ListTile(
            leading: const Icon(Icons.star_outline),
            title: const Text('Rate compter'),
            subtitle: const Text('A review helps others find it'),
            trailing: const Icon(Icons.open_in_new),
            onTap: () => go(store.rate),
          ),
        ListTile(
          leading: const Icon(Icons.feedback_outlined),
          title: const Text('Send feedback'),
          subtitle: const Text('Report a problem or suggest an idea'),
          trailing: const Icon(Icons.open_in_new),
          onTap: () => go(
            () => store.sendFeedback(version ?? 'unknown'),
            failed: 'No email app: write to $feedbackEmail',
          ),
        ),
        ListTile(
          leading: const Icon(Icons.privacy_tip_outlined),
          title: const Text('Privacy policy'),
          trailing: const Icon(Icons.open_in_new),
          onTap: () => go(() => store.open(Uri.parse(privacyUrl))),
        ),
        ListTile(
          leading: const Icon(Icons.description_outlined),
          title: const Text('Terms'),
          trailing: const Icon(Icons.open_in_new),
          onTap: () => go(() => store.open(Uri.parse(termsUrl))),
        ),
        if (version != null) _Field('Version', version),
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
