#!/usr/bin/env bash
# One-time setup of the generated parts of app/: the iOS and Android folders and the Dart
# bindings. The Rust library itself is built by hook/build.dart whenever Flutter builds.
# Needs Flutter, Xcode and the Android SDK (see app/README.md). Safe to re-run.
set -euo pipefail

FRB_VERSION=2.13.0
APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CRATE_DIR="$APP_DIR/../crates/compter-mobile"
cd "$APP_DIR"

if ! flutter_rust_bridge_codegen --version 2>/dev/null | grep -q "$FRB_VERSION"; then
  cargo install flutter_rust_bridge_codegen --version "$FRB_VERSION" --locked
fi

# 1. Platform folders. `flutter create` skips files that exist, so lib/ and pubspec.yaml stay
if [ ! -d ios ] || [ ! -d android ]; then
  had_test=$([ -e test/widget_test.dart ] && echo yes || echo no)
  flutter create --project-name compter --org dev.compter --platforms ios,android .
  # Its sample test drives the counter app, which this isn't
  [ "$had_test" = no ] && rm -f test/widget_test.dart && rmdir test 2>/dev/null || true
fi

# 2. Dart bindings for crate::api, and the Rust half they call
flutter pub get
flutter_rust_bridge_codegen generate
if ! grep -q '^mod frb_generated;' "$CRATE_DIR/src/lib.rs"; then
  printf '\n/// Written by `flutter_rust_bridge_codegen generate`; never edit it\nmod frb_generated;\n' \
    >>"$CRATE_DIR/src/lib.rs"
fi
# The generator formats in rustfmt's older style; the workspace checks the 2024 one
cargo fmt --manifest-path "$CRATE_DIR/Cargo.toml"

echo "Ready: flutter run (add --dart-define=COMPTER_DEMO=true for the sample business)"
