//! compter for iOS and Android: the services of `compter_core`, exposed to the Flutter app in
//! `app/` through flutter_rust_bridge. Everything under `api` crosses the bridge; the Dart side
//! is generated from it with `flutter_rust_bridge_codegen generate` (see `app/README.md`).
//!
//! Like the terminal views, this layer only calls services: it never touches storage or domain
//! mutators, and it hands Dart plain rows, never domain types.

pub mod api;

/// Written by `flutter_rust_bridge_codegen generate`; never edit it
mod frb_generated;
