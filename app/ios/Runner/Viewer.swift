import Flutter
import QuickLook
import UIKit

/// Shows a PDF Rust just wrote in Quick Look, for lib/src/notice.dart's openPdf. Presented from
/// the foreground scene's own window, so it works with several scenes (no app-wide keyWindow).
final class CompterViewer: NSObject, QLPreviewControllerDataSource {
  private let channel: FlutterMethodChannel
  /// The file on screen; Quick Look asks for it while the preview is open
  private var shown: URL?

  init(messenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(name: "compter/viewer", binaryMessenger: messenger)
    super.init()
    channel.setMethodCallHandler { [weak self] call, result in
      guard call.method == "open", let path = call.arguments as? String, let self else {
        result(FlutterMethodNotImplemented)
        return
      }
      result(self.open(URL(fileURLWithPath: path)))
    }
  }

  private func open(_ url: URL) -> Any? {
    guard FileManager.default.fileExists(atPath: url.path) else {
      return FlutterError(code: "missing", message: "The PDF is no longer there", details: nil)
    }
    guard let presenter = Self.topController() else {
      return FlutterError(code: "noWindow", message: "compter isn't on screen", details: nil)
    }
    shown = url
    let preview = QLPreviewController()
    preview.dataSource = self
    presenter.present(preview, animated: true)
    return nil
  }

  /// The controller on top in the foreground scene's key window
  private static func topController() -> UIViewController? {
    let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
    let scene = scenes.first { $0.activationState == .foregroundActive } ?? scenes.first
    var top = scene?.keyWindow?.rootViewController
    while let presented = top?.presentedViewController {
      top = presented
    }
    return top
  }

  func numberOfPreviewItems(in controller: QLPreviewController) -> Int {
    shown == nil ? 0 : 1
  }

  func previewController(_ controller: QLPreviewController, previewItemAt index: Int)
    -> QLPreviewItem
  {
    (shown ?? URL(fileURLWithPath: "/")) as NSURL
  }
}
