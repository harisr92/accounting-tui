#!/usr/bin/env bash
# One-time setup of the generated parts of app/: the iOS and Android folders, the cargokit
# plugin that builds crates/compter-mobile inside Xcode and Gradle, and the Dart bindings.
# Needs Flutter, Xcode and the Android SDK + NDK (see app/README.md). Safe to re-run.
set -euo pipefail

FRB_VERSION=2.13.0
APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CRATE_DIR="$APP_DIR/../crates/compter-mobile"
cd "$APP_DIR"

if ! flutter_rust_bridge_codegen --version 2>/dev/null | grep -q "$FRB_VERSION"; then
  cargo install flutter_rust_bridge_codegen --version "$FRB_VERSION" --locked
fi

# 1. Platform folders. `flutter create` skips files that exist, so lib/ and pubspec.yaml stay;
# --no-pub because rust_builder/, which pubspec.yaml needs, comes in step 2.
if [ ! -d ios ] || [ ! -d android ]; then
  had_test=$([ -e test/widget_test.dart ] && echo yes || echo no)
  flutter create --no-pub --project-name compter --org dev.compter --platforms ios,android .
  # Its sample test drives the counter app, which this isn't
  [ "$had_test" = no ] && rm -f test/widget_test.dart && rmdir test 2>/dev/null || true
fi

# 2. The cargokit plugin, taken from flutter_rust_bridge's template and pointed at our crate
if [ ! -d rust_builder ]; then
  scratch="$(mktemp -d)"
  trap 'rm -rf "$scratch"' EXIT
  (cd "$scratch" && flutter create --platforms ios,android compter >/dev/null &&
    cd compter && flutter_rust_bridge_codegen integrate)
  cp -R "$scratch/compter/rust_builder" rust_builder
  # The template's crate is app/rust as `rust_lib_compter`; ours is compter-mobile
  grep -rl rust_lib_compter rust_builder | xargs perl -pi -e 's/rust_lib_compter/compter_mobile/g'
  find rust_builder -name '*rust_lib_compter*' | while read -r f; do
    mv "$f" "${f//rust_lib_compter/compter_mobile}"
  done
  grep -rl '\.\./\.\./rust' rust_builder | xargs perl -pi -e 's#\.\./\.\./rust(?![_\w])#../../../crates/compter-mobile#g'
  # cargokit looks for lib<package>.a, but cargo writes libcompter_mobile.a for compter-mobile
  src=rust_builder/cargokit/build_tool/lib/src
  perl -0pi -e "s/(  final String packageName;\n)/\$1\n  \/\/ Cargo names library files after the package with hyphens turned into underscores.\n  String get libraryName => packageName.replaceAll('-', '_');\n/" "$src/cargo.dart"
  perl -pi -e 's/libraryName: (environment\.)?crateInfo\.packageName/libraryName: $1crateInfo.libraryName/' \
    "$src/artifacts_provider.dart" "$src/precompile_binaries.dart" "$src/verify_binaries.dart"
  perl -pi -e 's/libName = environment\.crateInfo\.packageName/libName = environment.crateInfo.libraryName/' "$src/build_pod.dart"
  # A newer cargokit the patterns don't match would only fail later, inside Xcode
  if ! grep -q 'get libraryName' "$src/cargo.dart" ||
    grep -qE '(libraryName: |libName = )(environment\.)?crateInfo\.packageName' "$src"/*.dart; then
    echo "bootstrap: the cargokit library-name patch did not apply; see app/README.md" >&2
    exit 1
  fi
fi

# 3. Dart bindings for crate::api, and the Rust half they call
flutter pub get
flutter_rust_bridge_codegen generate
if ! grep -q '^mod frb_generated;' "$CRATE_DIR/src/lib.rs"; then
  printf '\n/// Written by `flutter_rust_bridge_codegen generate`; never edit it\nmod frb_generated;\n' \
    >>"$CRATE_DIR/src/lib.rs"
fi

echo "Ready: flutter run (add --dart-define=COMPTER_DEMO=true for the sample business)"
