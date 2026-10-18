# compter mobile

The iOS and Android app. All the accounting (projects, invoices, GST, the ledger, storage) is
the Rust core in `../crates/compter-core`, reached through `../crates/compter-mobile` with
[flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/). This folder is only the UI.

```
lib/main.dart              opens the database, starts the app
lib/src/compter.dart       the opened CompterApp and the Riverpod providers over it
lib/src/router.dart        go_router: Projects, Invoices and Settings tabs, each with its own stack
lib/src/screens/           one per tab (projects, invoices, expenses, reports, settings), plus a
                           project and an invoice
lib/src/forms/             FormDialog (every form) and the forms and flows built on it
lib/src/notice.dart        snack bars, yes/no questions, opening and sharing files
lib/src/year_picker.dart   the financial year a tab shows, stepped like [ and ] in the terminal
lib/src/rust/              generated bindings: never edit, re-run codegen
rust_builder/              cargokit plugin that builds the Rust crate in Xcode and Gradle
test/                      widget tests on the host, loading ../target/debug's library
integration_test/          smoke test on a simulator or emulator, on the demo business
```

Money and dates cross the bridge as text: Rust formats amounts (`₹1,50,000.00`) and parses
what was typed, so Dart never does either. A refused input comes back as `ApiError` with the
`field` it belongs to, and `FormDialog` shows it under that field.

## Set up (once)

1. **Xcode** from the App Store, then
   `sudo xcode-select -s /Applications/Xcode.app && xcodebuild -runFirstLaunch`,
   and an iOS simulator runtime (Xcode → Settings → Components). `brew install cocoapods`.
2. **Android Studio** (`brew install --cask android-studio`), then in its SDK Manager: an
   Android SDK, **NDK (Side by side)** and an emulator image. Accept the licences with
   `flutter doctor --android-licenses`.
3. **Flutter** (`brew install --cask flutter`); `flutter doctor` should be clean for iOS and
   Android.
4. `tool/bootstrap.sh`: creates `ios/` and `android/`, adds `rust_builder/`, installs
   `flutter_rust_bridge_codegen` 2.13.0 and generates the bindings.

## Run and test

```bash
flutter run                                    # your data, in the app's support folder
flutter run --dart-define=COMPTER_DEMO=true    # the sample business, in memory
cargo build -p compter-mobile && flutter test  # screens on this machine, against the real Rust core
flutter test integration_test                  # smoke test on the running simulator or emulator
cargo test -p compter-mobile                   # the Rust API, from the repo root
```

After changing anything under `crates/compter-mobile/src/api`, run
`flutter_rust_bridge_codegen generate && cargo fmt --all` here. The generator formats its Rust
in rustfmt's older style, which `cargo fmt --check` refuses.

## Upgrading flutter_rust_bridge

`rust_builder/cargokit` is patched: it looks for `lib<package>.a`, but cargo names the library
of `compter-mobile` `libcompter_mobile.a` (`CrateInfo.libraryName` turns the hyphen into an
underscore). Without the patch iOS builds fail with "Unable to find bundle for dynamic
library". To take a new template, delete `rust_builder/` and run `tool/bootstrap.sh`, which
re-applies the patch and stops if it no longer matches.
