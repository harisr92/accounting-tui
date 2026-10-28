import 'dart:io';
import 'dart:ui';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:share_plus/share_plus.dart';
import 'package:url_launcher/url_launcher.dart';

const playStoreUrl =
    'https://play.google.com/store/apps/details?id=app.compter';

/// Null until the iOS app is in the App Store; until then the iPhone hides share and rate
const String? appStoreUrl = null;

const privacyUrl = 'https://docs.hari-in-it.in/privacy.html';
const termsUrl = 'https://docs.hari-in-it.in/terms.html';

/// Where feedback goes: an email needs no account and stays private, unlike an issue
const feedbackEmail = 'feedback@compter.hari-in-it.in';

/// Where compter is listed and where feedback goes, and the platform calls that open them.
/// Tests replace [open] and [share], which have no plugin on the host.
class Storefront {
  const Storefront({
    required this.storeUrl,
    this.open = _openOutside,
    this.share = _share,
  });

  /// The listing for this phone's store, from [playStoreUrl] or [appStoreUrl]
  factory Storefront.forPlatform() =>
      Storefront(storeUrl: Platform.isIOS ? appStoreUrl : playStoreUrl);

  /// Null where compter isn't listed yet, which hides share and rate
  final String? storeUrl;
  final Future<bool> Function(Uri) open;
  final Future<void> Function(String text, Rect? origin) share;

  /// What a friend receives: why, then the link
  String? get shareText => storeUrl == null
      ? null
      : 'I keep my GST invoices and books in compter, made for freelancers '
            'in India: $storeUrl';

  Future<bool> rate() => open(Uri.parse(storeUrl!));

  /// An email with [version] and the phone's system filled in, so a report says what it
  /// ran on without asking
  Future<bool> sendFeedback(String version) => open(
    Uri(
      scheme: 'mailto',
      path: feedbackEmail,
      // mailto wants %20 for spaces, which queryParameters would write as +
      query: _encodeQuery({
        'subject': 'compter feedback',
        'body':
            '\n\n---\ncompter $version on ${Platform.operatingSystem} '
            '${Platform.operatingSystemVersion}',
      }),
    ),
  );
}

String _encodeQuery(Map<String, String> params) => params.entries
    .map((e) => '${e.key}=${Uri.encodeComponent(e.value)}')
    .join('&');

Future<bool> _openOutside(Uri url) =>
    launchUrl(url, mode: LaunchMode.externalApplication);

Future<void> _share(String text, Rect? origin) => SharePlus.instance.share(
  ShareParams(text: text, subject: 'compter', sharePositionOrigin: origin),
);

final storefrontProvider = Provider<Storefront>(
  (ref) => Storefront.forPlatform(),
);

/// This build's version and build number, as `0.2.5 (2)`
final appVersionProvider = FutureProvider<String>((ref) async {
  final info = await PackageInfo.fromPlatform();
  return '${info.version} (${info.buildNumber})';
});
