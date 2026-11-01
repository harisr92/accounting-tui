import 'package:compter/src/sync/cloud.dart';
import 'package:compter/src/sync/icloud.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

/// The iCloud container through its channel (ios/Runner/ICloud.swift), answered here
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('compter/icloud');

  void answer(Object? Function(MethodCall call) handler) {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async => handler(call));
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null),
    );
  }

  test('iCloud Drive turned off is signed out, not an error', () async {
    answer((call) => false);
    final account = ICloudStorage();
    expect(await account.restore(), isNull);
    expect(account.signIn, throwsA(isA<CloudSignedOut>()));
  });

  test('files are read and written by name in the container', () async {
    final files = <String, Uint8List>{};
    answer((call) {
      final args = call.arguments as Map;
      return switch (call.method) {
        'read' => files[args['name']],
        'write' => files[args['name'] as String] = args['bytes'] as Uint8List,
        _ => files.remove(args['name']),
      };
    });
    final folder = ICloudStorage().folder();
    await folder.write('lease.json', Uint8List.fromList([1, 2]));
    expect(await folder.read('lease.json'), [1, 2]);
    await folder.delete('lease.json');
    expect(await folder.read('lease.json'), isNull);
  });

  test('a download that does not finish is offline and retried', () async {
    answer(
      (call) => throw PlatformException(
        code: call.method == 'write' ? 'signedOut' : 'offline',
        message: 'compter.redb is still downloading from iCloud',
      ),
    );
    final folder = ICloudStorage().folder();
    expect(folder.read('compter.redb'), throwsA(isA<CloudOffline>()));
    expect(
      folder.write('lease.json', Uint8List(0)),
      throwsA(isA<CloudSignedOut>()),
    );
  });
}
