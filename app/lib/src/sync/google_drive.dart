import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:extension_google_sign_in_as_googleapis_auth/extension_google_sign_in_as_googleapis_auth.dart';
import 'package:flutter/services.dart';
import 'package:google_sign_in/google_sign_in.dart';
import 'package:googleapis/drive/v3.dart' as drive;
import 'package:googleapis_auth/googleapis_auth.dart'
    show AccessDeniedException;

import 'cloud.dart';

/// The web client ID of the Google Cloud project, which Android's sign-in needs:
/// `flutter run --dart-define=COMPTER_GOOGLE_SERVER_CLIENT_ID=...`. iOS reads its own client ID
/// from Info.plist (ios/Flutter/Google.xcconfig). See app/README.md, "Google Drive sync".
const _serverClientId = String.fromEnvironment(
  'COMPTER_GOOGLE_SERVER_CLIENT_ID',
);

/// Full Drive access: the phone must read the `compter.redb` the computer's Drive client
/// uploaded, which the narrower `drive.file` scope can't see
const _scopes = [drive.DriveApi.driveScope];

/// Answers whether Info.plist has a `GIDClientID` (ios/Runner/AppDelegate.swift). Without one
/// GoogleSignIn raises a native exception Dart can't catch, so iOS asks before initializing.
const _iosConfig = MethodChannel('compter/google_sign_in');

const _folderMime = 'application/vnd.google-apps.folder';
const _syncFolderName = 'compter-sync';

/// Google Drive through google_sign_in: one account, authorized for [_scopes]
class GoogleDriveAccount implements CloudAccount {
  GoogleSignInAccount? _user;
  drive.DriveApi? _api;
  String? _token;
  Future<void>? _initialized;

  Future<void> _initialize() => _initialized ??= () async {
    if (Platform.isIOS &&
        await _iosConfig.invokeMethod<bool>('isConfigured') != true) {
      throw const CloudNotConfigured();
    }
    await GoogleSignIn.instance.initialize(
      serverClientId: _serverClientId.isEmpty ? null : _serverClientId,
    );
  }();

  @override
  Future<String?> restore() async {
    await _initialize();
    final user = await GoogleSignIn.instance.attemptLightweightAuthentication();
    if (user == null) return null;
    // Only silently: a scope never granted needs signIn, from a tap
    final authorization = await user.authorizationClient.authorizationForScopes(
      _scopes,
    );
    if (authorization == null) return null;
    _user = user;
    _use(authorization);
    return user.email;
  }

  @override
  Future<String> signIn() async {
    await _initialize();
    final user = await GoogleSignIn.instance.authenticate(scopeHint: _scopes);
    final authorization = await user.authorizationClient.authorizeScopes(
      _scopes,
    );
    _user = user;
    _use(authorization);
    return user.email;
  }

  drive.DriveApi _use(GoogleSignInClientAuthorization authorization) {
    _token = authorization.accessToken;
    return _api = drive.DriveApi(authorization.authClient(scopes: _scopes));
  }

  @override
  Future<void> signOut() async {
    await _initialize();
    await GoogleSignIn.instance.signOut();
    _user = null;
    _api = null;
    _token = null;
  }

  /// The API, with a fresh token: access tokens last an hour, and asking again is silent
  /// while the grant stands. The refused token is dropped from the platform's cache first,
  /// or it would be handed back again (as it is after the user removes compter from their
  /// Google account); with the grant gone, asking returns nothing and the user signs in again.
  Future<drive.DriveApi> _fresh() async {
    final user = _user;
    if (user == null) throw const CloudSignedOut();
    final refused = _token;
    if (refused != null) {
      _api = null;
      _token = null;
      await user.authorizationClient.clearAuthorizationToken(
        accessToken: refused,
      );
    }
    final authorization = await user.authorizationClient.authorizationForScopes(
      _scopes,
    );
    if (authorization == null) throw const CloudSignedOut();
    return _use(authorization);
  }

  /// Run [call], once more with a fresh token if Google refused the old one, mapping network
  /// failures to [CloudOffline] and lapsed sign-in to [CloudSignedOut]. A refused token
  /// comes back as [AccessDeniedException] (a 401 with `WWW-Authenticate: invalid_token`),
  /// not as an API error.
  Future<T> _call<T>(Future<T> Function(drive.DriveApi api) call) async {
    try {
      final api = _api ?? await _fresh();
      try {
        return await call(api);
      } on AccessDeniedException {
        return await call(await _fresh());
      } on drive.DetailedApiRequestError catch (error) {
        if (error.status != 401) rethrow;
        return await call(await _fresh());
      }
    } on AccessDeniedException {
      throw const CloudSignedOut();
    } on drive.DetailedApiRequestError catch (error) {
      if (error.status == 401 || error.status == 403) {
        throw const CloudSignedOut();
      }
      throw CloudOffline('Google Drive refused: ${error.message}');
    } on SocketException {
      throw const CloudOffline('Offline');
    } on TimeoutException {
      throw const CloudOffline('Google Drive did not answer');
    }
  }

  @override
  Future<List<CloudFolderChoice>> findSyncFolders() => _call((api) async {
    final manifests = await api.files.list(
      q: "name = 'manifest.json' and trashed = false",
      $fields: 'files(parents)',
      spaces: 'drive',
      pageSize: 50,
    );
    final ids = {
      for (final file in manifests.files ?? <drive.File>[]) ...?file.parents,
    };
    return [for (final id in ids) (id: id, path: await _path(api, id))];
  });

  /// `My Drive/Work/compter-sync`, walking up the folder's parents
  Future<String> _path(drive.DriveApi api, String id) async {
    final names = <String>[];
    String? next = id;
    // A folder deeper than this is shown by its last parts
    for (var depth = 0; next != null && depth < 8; depth++) {
      final folder =
          await api.files.get(next, $fields: 'name,parents') as drive.File;
      final parents = folder.parents;
      names.insert(0, parents == null ? 'My Drive' : folder.name ?? '?');
      next = parents?.first;
    }
    return names.join('/');
  }

  @override
  Future<CloudFolderChoice> createSyncFolder() => _call((api) async {
    final folder = await api.files.create(
      drive.File()
        ..name = _syncFolderName
        ..mimeType = _folderMime,
      $fields: 'id',
    );
    return (id: folder.id!, path: 'My Drive/$_syncFolderName');
  });

  @override
  CloudFolder folder(String id) => _DriveFolder(this, id);
}

/// One Drive folder. Files are found by name and their IDs remembered, so a write updates
/// the same file rather than adding another of the same name.
class _DriveFolder implements CloudFolder {
  _DriveFolder(this._account, this._id);

  final GoogleDriveAccount _account;
  final String _id;
  final _ids = <String, String>{};

  Future<String?> _fileId(drive.DriveApi api, String name) async {
    final known = _ids[name];
    if (known != null) return known;
    final found = await api.files.list(
      q: "name = '$name' and '$_id' in parents and trashed = false",
      $fields: 'files(id)',
      spaces: 'drive',
      orderBy: 'modifiedTime desc',
      pageSize: 1,
    );
    final id = found.files?.firstOrNull?.id;
    if (id != null) _ids[name] = id;
    return id;
  }

  @override
  Future<Uint8List?> read(String name) => _account._call((api) async {
    final id = await _fileId(api, name);
    if (id == null) return null;
    final media =
        await api.files.get(
              id,
              downloadOptions: drive.DownloadOptions.fullMedia,
            )
            as drive.Media;
    final bytes = BytesBuilder(copy: false);
    await media.stream.forEach(bytes.add);
    return bytes.takeBytes();
  });

  @override
  Future<void> write(String name, Uint8List bytes) =>
      _account._call((api) async {
        final media = drive.Media(Stream.value(bytes), bytes.length);
        final id = await _fileId(api, name);
        if (id != null) {
          await api.files.update(drive.File(), id, uploadMedia: media);
          return;
        }
        final created = await api.files.create(
          drive.File()
            ..name = name
            ..parents = [_id],
          uploadMedia: media,
          $fields: 'id',
        );
        _ids[name] = created.id!;
      });

  @override
  Future<void> delete(String name) => _account._call((api) async {
    final id = await _fileId(api, name);
    if (id == null) return;
    await api.files.delete(id);
    _ids.remove(name);
  });
}
