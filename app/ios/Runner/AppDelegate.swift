import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  /// Kept for the life of the app: it listens for StoreKit's transaction updates
  private var store: CompterStore?

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
    if let registrar = engineBridge.pluginRegistry.registrar(forPlugin: "CompterBackground") {
      registerBackgroundTime(messenger: registrar.messenger())
    }
    if let registrar = engineBridge.pluginRegistry.registrar(forPlugin: "CompterStore") {
      store = CompterStore(messenger: registrar.messenger())
    }
  }

  /// Background time for Google Drive sync's last send and the lease's release when the app
  /// leaves the screen (lib/src/sync/background_time.dart). Without it iOS suspends the app
  /// within seconds, and the computer waits 5 minutes for the lease to go stale.
  private func registerBackgroundTime(messenger: FlutterBinaryMessenger) {
    let channel = FlutterMethodChannel(name: "compter/background", binaryMessenger: messenger)
    channel.setMethodCallHandler { call, result in
      switch call.method {
      case "begin":
        var task = UIBackgroundTaskIdentifier.invalid
        task = UIApplication.shared.beginBackgroundTask(withName: "compter sync") {
          // Out of time: iOS suspends the app, and the lease goes stale on its own
          UIApplication.shared.endBackgroundTask(task)
          task = .invalid
        }
        result(task.rawValue)
      case "end":
        if let raw = call.arguments as? Int, raw != UIBackgroundTaskIdentifier.invalid.rawValue {
          UIApplication.shared.endBackgroundTask(UIBackgroundTaskIdentifier(rawValue: raw))
        }
        result(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
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
