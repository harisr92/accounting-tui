import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    if let registrar = engineBridge.pluginRegistry.registrar(forPlugin: "CompterGoogleSignIn") {
      registerGoogleSignInCheck(messenger: registrar.messenger())
    }
  }

  /// GoogleSignIn raises an uncatchable exception when GIDClientID is empty (no
  /// ios/Flutter/Google.xcconfig), so Dart asks first (lib/src/sync/google_drive.dart)
  private func registerGoogleSignInCheck(messenger: FlutterBinaryMessenger) {
    let channel = FlutterMethodChannel(name: "compter/google_sign_in", binaryMessenger: messenger)
    channel.setMethodCallHandler { call, result in
      guard call.method == "isConfigured" else {
        result(FlutterMethodNotImplemented)
        return
      }
      let clientId = Bundle.main.object(forInfoDictionaryKey: "GIDClientID") as? String ?? ""
      result(!clientId.isEmpty)
    }
  }
}
