# compter mobile

The iOS and Android app. All the accounting (projects, invoices, GST, the ledger, storage) is
the Rust core in `../crates/compter-core`, reached through `../crates/compter-mobile` with
[flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/). This folder is only the UI.

```
lib/main.dart              opens the database, starts the app
lib/src/compter.dart       the opened CompterApp and the Riverpod providers over it
lib/src/router.dart        go_router: the tabs, each with its own stack
lib/src/screens/           one per tab (projects, invoices, expenses, reports, settings), plus a
                           project and an invoice
lib/src/forms/             FormDialog (every form) and the forms and flows built on it
lib/src/notice.dart        snack bars, yes/no questions, opening and sharing files
lib/src/handoff.dart       picking and staging the snapshot and manifest carried to and from the
                           computer's sync folder (see "Phones" in the main README)
lib/src/year_picker.dart   the financial year a tab shows, stepped like [ and ] in the terminal
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

## Releasing

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

1. Create the app `dev.compter.compter` in Play Console.
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

1. Register the bundle ID `dev.compter.compter` and create the app in App Store Connect.
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

Play uploads stay drafts until the app has been published once; then set the repository
variable `COMPTER_PLAY_RELEASE_STATUS` to `completed`.

To run a lane yourself, export the same `COMPTER_*` variables, then from `ios/` or `android/`:
`BUNDLE_GEMFILE=../Gemfile bundle exec fastlane beta` (or `internal`).

Before the first store submission the app still needs its own launcher icon and store
listing (screenshots, description, privacy policy); the icon is Flutter's default.
