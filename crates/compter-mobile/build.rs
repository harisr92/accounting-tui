//! Links the Android library for 16 KB memory pages, which Google Play requires of every
//! 64-bit native library (https://developer.android.com/guide/practices/page-sizes).
//! NDK r28 does this by default, but Flutter links with whichever NDK `ANDROID_NDK_HOME`
//! names first, and CI runners point it at r27, which still aligns to 4 KB.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-z,max-page-size=16384");
    }
}
