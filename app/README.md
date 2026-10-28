# compter mobile

The iOS and Android app. All the accounting (projects, invoices, GST, the ledger, storage) is
the Rust core in `../crates/compter-core`, reached through `../crates/compter-mobile` with
[flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/). This folder is only the UI.

```
lib/main.dart              opens the database, starts the app
lib/src/compter.dart       the opened CompterApp and the Riverpod providers over it
lib/src/router.dart        go_router: the tabs, each with its own stack
lib/src/screens/           one per tab (projects, invoices, expenses, reports, settings), plus a
                           project, an invoice, the welcome pages and the Get started card
lib/src/forms/             FormDialog (every form) and the forms and flows built on it
lib/src/notice.dart        snack bars, yes/no questions, opening and sharing files
lib/src/handoff.dart       picking and staging the snapshot and manifest carried to and from the
                           computer's sync folder (see "Phones" in the main README)
lib/src/sync/              automatic sync through Google Drive (see "Google Drive sync")
lib/src/year_picker.dart   the financial year a tab shows, stepped like [ and ] in the terminal
lib/src/about.dart         the store links, sharing the app, rating it and emailing feedback
lib/src/review_prompt.dart asking for a store review after the third invoice issued or paid
lib/src/onboarding.dart    whether to show the welcome pages and the Get started card
lib/src/appearance.dart    the theme this phone draws in (like the phone, light, dark, gruvbox
                           dark), kept in its preferences
lib/src/ui/glass.dart      the frosted header and tab bar, and ScreenList, which scrolls under them
lib/src/rust/              generated bindings: never edit, re-run codegen
hook/build.dart            builds ../crates/compter-mobile whenever Flutter builds (see below)
ios/fastlane, android/fastlane   store uploads, run by the release workflow (see Releasing)
test/                      widget tests on the host, against the library the hook built for it
integration_test/          tests on a simulator or emulator: smoke test, hand-off round trip
```

Money and dates cross the bridge as text: Rust formats amounts (`₹1,50,000.00`) and parses
what was typed, so Dart never does either. A refused input comes back as `ApiError` with the
`field` it belongs to, and `FormDialog` shows it under that field.

## Set up (once)

1. **Xcode** from the App Store, then
   `sudo xcode-select -s /Applications/Xcode.app && xcodebuild -runFirstLaunch`,
   and an iOS simulator runtime (Xcode → Settings → Components). No CocoaPods: every iOS
   plugin is a Swift package.
2. **Android Studio** (`brew install --cask android-studio`), then in its SDK Manager: an
   Android SDK and an emulator image (Gradle fetches the NDK itself). Accept the licences
   with `flutter doctor --android-licenses`.
3. **rustup**. The first build installs the Rust version and targets pinned in
   `../crates/compter-mobile/rust-toolchain.toml`.
4. **Flutter** (`brew install --cask flutter`); `flutter doctor` should be clean for iOS and
   Android.
5. `tool/bootstrap.sh`: creates `ios/` and `android/` if missing, installs
   `flutter_rust_bridge_codegen` 2.13.0 and generates the bindings.

## Run and test

```bash
flutter run                                    # your data, in the app's support folder
flutter run --dart-define=COMPTER_DEMO=true    # the sample business, in memory
flutter test                                   # screens on this machine, against the real Rust core
flutter test integration_test                  # on the running simulator or emulator
cargo test -p compter-mobile                   # the Rust API, from the repo root
```

After changing anything under `crates/compter-mobile/src/api`, run
`flutter_rust_bridge_codegen generate && cargo fmt --all` here. The generator formats its Rust
in rustfmt's older style, which `cargo fmt --check` refuses.

## How the Rust library is built

`hook/build.dart` is a Dart build hook: whenever Flutter builds (`flutter run`, `flutter test`,
`flutter build`), it runs `native_toolchain_rust`, which builds `../crates/compter-mobile` in
release mode for the target being built, with the Rust pinned in that crate's
`rust-toolchain.toml`. Flutter bundles the library as `compter_mobile.framework` on iOS and
`libcompter_mobile.so` on Android, the names flutter_rust_bridge's generated loader opens. No
CocoaPods or Swift Package Manager plugin is involved. The build goes to
`.dart_tool/hooks_runner`, apart from the workspace's `target/`.

To move to a newer Rust, change `channel` in `rust-toolchain.toml`; it must be an exact
version, and `targets` must list every platform built for, the host included.

## Google Drive sync

Settings → Sync method → **Google Drive, automatic** keeps the phone and compter on a computer
in step through compter's hidden app folder in Google Drive (`lib/src/sync/`). The computer
signs in to the same Cloud project with `compter drive sign-in`, so both see that one folder:

- On opening and on every return to the app, the phone takes the computer's newer snapshot and
  writes its own `lease.json`, so compter on the computer waits until the phone is put away.
- Every minute while open, and when sent to the background, it sends what changed (snapshot,
  then manifest, updating the same Drive files) and refreshes or releases the lease.
- If compter is open on the computer, the phone takes nothing over it and sends nothing until
  it closes. If both sides changed, Settings (badged) asks which to keep.

It needs only `drive.appdata`, the app's own hidden folder, which Google lists as non-sensitive:
publishing needs basic verification, with no security assessment. The files never show in the
Drive UI, so nobody moves or renames them by accident. In **Testing**, sign-in lapses after
7 days and Settings asks again.

### One-time setup

1. In Google Cloud: a project, with the **Google Drive API** enabled.
2. OAuth consent screen: External, your account as a test user while in **Testing**, scope
   `https://www.googleapis.com/auth/drive.appdata` (and `openid`, `email`). Not the full
   `.../auth/drive`: it is restricted and needs a yearly CASA assessment.
3. OAuth clients (Credentials → Create credentials → OAuth client ID):
   - **iOS**, bundle ID `app.compter`. Copy `ios/Flutter/Google.xcconfig.example` to
     `ios/Flutter/Google.xcconfig` (gitignored) and fill in its client ID and reversed ID.
   - **Android**, package `app.compter`, with the SHA-1 of each key that signs the
     app: the debug key (`keytool -list -v -keystore ~/.android/debug.keystore -storepass
     android`), the upload key, and **Google Play's app signing key** (Play Console → Test and
     release → App integrity → App signing). Play re-signs what it installs with that last one,
     so without it sign-in fails only in store builds, with "Developer console is not set up
     correctly".
     Its ID goes nowhere: Google finds it by package and SHA-1.
   - **Web application**, in the same project, with no origins or redirect URIs (it has no
     SHA-1). Android's sign-in needs its client ID, passed when building:
     `flutter run --dart-define=COMPTER_GOOGLE_SERVER_CLIENT_ID=<web client ID>`. Passing the
     Android client's ID instead fails with "Developer console is not set up correctly".
   - **Desktop app**, in the same project, for compter on the computer. Build it with
     `COMPTER_GOOGLE_DESKTOP_CLIENT_ID` and `COMPTER_GOOGLE_DESKTOP_CLIENT_SECRET` set (an
     installed app's secret isn't secret; Google still asks for it). Release builds read
     them from the repository secrets of the same names; a `v*` tag fails without them.
4. On the computer, run `compter drive sign-in` with the same Google account. It keeps the
   grant in the OS keychain and, if the Drive has no snapshot yet, publishes the computer's.



CI (`.github/workflows/mobile.yml`) checks the bindings are current, runs `flutter analyze` and
the widget tests, and builds both apps on every push to `trunk` that touches `app/` or
`crates/`.

A tag `mobile-v<version>` (matching `version:` in `pubspec.yaml`) runs
`.github/workflows/mobile-release.yml`: fastlane builds a signed app bundle for Google Play's
internal track and a signed IPA for TestFlight. The build number is the workflow's run number.
Desktop releases keep their own `v<version>` tags.

```bash
# bump version: in pubspec.yaml first
git tag mobile-v0.3.0 && git push origin mobile-v0.3.0
```

### One-time setup

**Google Play** (developer account, $25 once)

1. Create the app `app.compter` in Play Console.
2. Make an upload key and keep it safe; losing it means asking Google to reset it:
   `keytool -genkeypair -keystore upload.jks -alias upload -keyalg RSA -keysize 2048 -validity 10000`
3. Upload the first app bundle by hand (Play requires it): put the key in
   `android/key.properties` (never committed), then `flutter build appbundle`.
   ```properties
   storeFile=/absolute/path/to/upload.jks
   storePassword=...
   keyAlias=upload
   keyPassword=...
   ```
4. In Google Cloud, make a service account with a JSON key, and in Play Console under
   Users and permissions give it release rights for the app.

**App Store** (Apple Developer Program, $99 a year)

1. Register the bundle ID `app.compter` and create the app in App Store Connect.
2. Create an Apple Distribution certificate (export it as a `.p12` with a password) and an
   App Store provisioning profile for the bundle ID.
3. In App Store Connect, under Users and Access, Integrations, create an API key with the
   App Manager role.

### Repository secrets

| Secret | What |
|---|---|
| `COMPTER_UPLOAD_KEYSTORE_BASE64` | `base64 -i upload.jks` |
| `COMPTER_UPLOAD_STORE_PASSWORD`, `COMPTER_UPLOAD_KEY_ALIAS`, `COMPTER_UPLOAD_KEY_PASSWORD` | the upload key's passwords and alias |
| `COMPTER_PLAY_SERVICE_ACCOUNT_JSON` | the service account's JSON key |
| `COMPTER_IOS_TEAM_ID` | the 10-character team ID |
| `COMPTER_IOS_CERTIFICATE_BASE64`, `COMPTER_IOS_CERTIFICATE_PASSWORD` | `base64 -i distribution.p12`, and its password |
| `COMPTER_IOS_PROFILE_BASE64`, `COMPTER_IOS_PROFILE_NAME` | `base64 -i compter.mobileprovision`, and the profile's name |
| `COMPTER_ASC_KEY_ID`, `COMPTER_ASC_ISSUER_ID`, `COMPTER_ASC_KEY` | the API key's ID, issuer ID and `.p8` contents |
| `COMPTER_GOOGLE_SERVER_CLIENT_ID` | the **Web** OAuth client's ID, for Android's Google sign-in |
| `COMPTER_GOOGLE_IOS_CLIENT_ID`, `COMPTER_GOOGLE_REVERSED_CLIENT_ID` | the iOS OAuth client's ID and reversed ID, written to `ios/Flutter/Google.xcconfig` |

Play uploads stay drafts until the app has been published once; then set the repository
variable `COMPTER_PLAY_RELEASE_STATUS` to `completed`.

To run a lane yourself, export the same `COMPTER_*` variables, then from `ios/` or `android/`:
`BUNDLE_GEMFILE=../Gemfile bundle exec fastlane beta` (or `internal`).

The Play listing's text (title, short and full description) lives in
`android/fastlane/metadata/android/en-US/`, so its search keywords are reviewed like code.
Play allows 30, 80 and 4000 characters. After changing it, run the `metadata` lane from
`android/` with `COMPTER_PLAY_SERVICE_ACCOUNT_JSON` set; it uploads the text only, no build.
Screenshots and the feature graphic are still added in Play Console.

Before the first store submission the app still needs its own launcher icon and store
listing (screenshots, description, privacy policy); the icon is Flutter's default.
