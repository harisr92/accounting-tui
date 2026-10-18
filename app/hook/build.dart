import 'package:hooks/hooks.dart';
import 'package:native_toolchain_rust/native_toolchain_rust.dart';

/// Builds the Rust core (crates/compter-mobile) for whatever Flutter is building: the phone,
/// the simulator, or the host for `flutter test`. Flutter bundles the library itself, as
/// `compter_mobile.framework` on iOS and `libcompter_mobile.so` on Android, where the
/// generated loader in lib/src/rust/frb_generated.dart finds it; no CocoaPods or Swift
/// Package Manager plugin is involved.
void main(List<String> args) async {
  await build(args, (input, output) async {
    await const RustBuilder(
      // Names the code asset; the loader opens the library by file name, not by this
      assetName: 'src/rust/frb_generated.io.dart',
      cratePath: '../crates/compter-mobile',
    ).run(input: input, output: output);
  });
}
