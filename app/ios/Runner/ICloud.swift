import Flutter
import Foundation

/// The app's hidden iCloud container, for lib/src/sync/icloud.dart: whole files by name in its
/// Documents folder, like Google Drive's app folder. compter on a Mac reads the same folder
/// under ~/Library/Mobile Documents. Every access is coordinated with iCloud's daemon, and a
/// file iCloud evicted to save space is downloaded before it is read.
final class CompterICloud {
  static let containerId = "iCloud.app.compter"

  /// How long a read waits for iCloud to download the latest version of a file
  private static let downloadWait: TimeInterval = 60

  private let channel: FlutterMethodChannel
  /// One file operation at a time, off the main thread: finding the container and downloading
  /// can take a while
  private let queue = DispatchQueue(label: "app.compter.icloud")

  init(messenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(name: "compter/icloud", binaryMessenger: messenger)
    channel.setMethodCallHandler { [queue] call, result in
      queue.async {
        let answer: Any?
        do {
          answer = try Self.handle(call)
        } catch let error as ICloudError {
          answer = FlutterError(code: error.code, message: error.message, details: nil)
        } catch {
          answer = FlutterError(code: "offline", message: error.localizedDescription, details: nil)
        }
        DispatchQueue.main.async { result(answer) }
      }
    }
  }

  private static func handle(_ call: FlutterMethodCall) throws -> Any? {
    if call.method == "isAvailable" {
      return FileManager.default.ubiquityIdentityToken != nil && documents() != nil
    }
    let arguments = call.arguments as? [String: Any]
    guard let name = arguments?["name"] as? String, !name.contains("/") else {
      return FlutterMethodNotImplemented
    }
    let url = try file(name)
    switch call.method {
    case "read":
      return try read(url).map { FlutterStandardTypedData(bytes: $0) }
    case "write":
      guard let bytes = arguments?["bytes"] as? FlutterStandardTypedData else {
        return FlutterMethodNotImplemented
      }
      try write(bytes.data, to: url)
      return nil
    case "delete":
      try delete(url)
      return nil
    default:
      return FlutterMethodNotImplemented
    }
  }

  /// The container's Documents folder; nil when iCloud Drive is off or the user isn't signed in
  private static func documents() -> URL? {
    guard
      let container = FileManager.default.url(forUbiquityContainerIdentifier: containerId)
    else { return nil }
    let documents = container.appendingPathComponent("Documents", isDirectory: true)
    try? FileManager.default.createDirectory(at: documents, withIntermediateDirectories: true)
    return documents
  }

  private static func file(_ name: String) throws -> URL {
    guard FileManager.default.ubiquityIdentityToken != nil, let documents = documents() else {
      throw ICloudError.signedOut
    }
    return documents.appendingPathComponent(name, isDirectory: false)
  }

  private static func read(_ url: URL) throws -> Data? {
    try downloadLatest(url)
    // Our manifest's generation decides between two devices' writes, not iCloud: keep the
    // version iCloud chose, so no conflict version lingers
    if let conflicts = NSFileVersion.unresolvedConflictVersionsOfItem(at: url), !conflicts.isEmpty {
      for version in conflicts { version.isResolved = true }
      try? NSFileVersion.removeOtherVersionsOfItem(at: url)
    }
    var data: Data?
    try coordinate { coordinator, error in
      coordinator.coordinate(readingItemAt: url, options: [], error: error) { url in
        data = FileManager.default.fileExists(atPath: url.path) ? try? Data(contentsOf: url) : nil
      }
    }
    return data
  }

  private static func write(_ data: Data, to url: URL) throws {
    var failure: Error?
    try coordinate { coordinator, error in
      coordinator.coordinate(writingItemAt: url, options: .forReplacing, error: error) { url in
        do { try data.write(to: url, options: .atomic) } catch { failure = error }
      }
    }
    if let failure { throw failure }
  }

  private static func delete(_ url: URL) throws {
    try coordinate { coordinator, error in
      coordinator.coordinate(writingItemAt: url, options: .forDeleting, error: error) { url in
        // Already gone is fine
        try? FileManager.default.removeItem(at: url)
      }
    }
  }

  private static func coordinate(_ body: (NSFileCoordinator, NSErrorPointer) -> Void) throws {
    var error: NSError?
    body(NSFileCoordinator(filePresenter: nil), &error)
    if let error { throw error }
  }

  /// Wait until the file on this phone is iCloud's latest version. A file iCloud doesn't know
  /// is simply missing.
  private static func downloadLatest(_ url: URL) throws {
    let keys: Set<URLResourceKey> = [.isUbiquitousItemKey, .ubiquitousItemDownloadingStatusKey]
    var url = url
    guard let values = try? url.resourceValues(forKeys: keys), values.isUbiquitousItem == true,
      values.ubiquitousItemDownloadingStatus != .current
    else { return }
    try FileManager.default.startDownloadingUbiquitousItem(at: url)
    let deadline = Date().addingTimeInterval(downloadWait)
    while Date() < deadline {
      Thread.sleep(forTimeInterval: 0.25)
      url.removeAllCachedResourceValues()
      if (try? url.resourceValues(forKeys: keys))?.ubiquitousItemDownloadingStatus == .current {
        return
      }
    }
    throw ICloudError.stillDownloading(url.lastPathComponent)
  }
}

private struct ICloudError: Error {
  let code: String
  let message: String

  static let signedOut = ICloudError(
    code: "signedOut",
    message: "Turn on iCloud Drive for compter in Settings, Apple Account, iCloud")

  static func stillDownloading(_ name: String) -> ICloudError {
    ICloudError(code: "offline", message: "\(name) is still downloading from iCloud")
  }
}
