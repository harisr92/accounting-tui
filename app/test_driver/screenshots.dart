import 'dart:convert';
import 'dart:io';

import 'package:integration_test/integration_test_driver.dart';

/// Where Play's listing images live, uploaded by fastlane's `metadata` lane
const _images = 'android/fastlane/metadata/android/en-US/images';

/// Writes the images integration_test/screenshots_test.dart drew, as `<name>.png`
Future<void> main() => integrationDriver(
  responseDataCallback: (data) async {
    final images = (data?['images'] as Map<String, dynamic>?) ?? {};
    for (final MapEntry(key: name, value: png) in images.entries) {
      final file = File('$_images/$name.png');
      await file.parent.create(recursive: true);
      await file.writeAsBytes(base64Decode(png as String));
      stdout.writeln('wrote ${file.path}');
    }
  },
);
