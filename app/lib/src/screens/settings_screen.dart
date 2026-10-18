import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../compter.dart';
import '../forms/profile_form.dart';
import '../notice.dart';
import '../rust/api/profile.dart';

/// The business profile printed on invoices, like the terminal's Settings tab
class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final profile = ref.watch(profileProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: profile.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (error, _) => Center(child: Text('$error')),
        data: (profile) => ListView(
          padding: const EdgeInsets.all(16),
          children: [
            Text(
              'Business profile',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 8),
            if (profile == null)
              const Text(
                'Not set up yet. Add your business details: they are needed '
                'before you can invoice.',
              )
            else
              _Profile(profile),
            const SizedBox(height: 16),
            Align(
              alignment: Alignment.centerLeft,
              child: FilledButton.icon(
                icon: const Icon(Icons.edit_outlined),
                label: Text(
                  profile == null
                      ? 'Add business details'
                      : 'Edit business details',
                ),
                onPressed: () async {
                  final saved = await profileForm(
                    context,
                    ref.read(compterProvider),
                  );
                  if (saved == null || !context.mounted) return;
                  ref.invalidate(profileProvider);
                  notify(context, 'Business details saved');
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Profile extends StatelessWidget {
  const _Profile(this.profile);

  final ProfileRow profile;

  @override
  Widget build(BuildContext context) {
    final rows = <(String, String)>[
      ('Name', profile.name),
      if (profile.address.isNotEmpty) ('Address', profile.address.join('\n')),
      ('GSTIN', profile.gstin ?? 'Not registered: invoices carry no GST'),
      ('SAC', [profile.sacCode, ?profile.sacDescription].join(' · ')),
      ('Prefix', profile.invoicePrefix),
      if (profile.lutArn != null) ('LUT ARN', profile.lutArn!),
      if (profile.terms.isNotEmpty) ('Terms', profile.terms.join('\n')),
      ('Currency', profile.currency),
    ];
    final text = Theme.of(context).textTheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (final (label, value) in rows)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                SizedBox(width: 88, child: Text(label, style: text.labelLarge)),
                Expanded(child: Text(value)),
              ],
            ),
          ),
      ],
    );
  }
}
