import Flutter
import StoreKit
import UIKit

/// The compter subscription through StoreKit 2, for lib/src/subscription/store.dart. StoreKit
/// verifies every transaction on the phone and keeps the last known ones offline, so there is
/// no receipt to check and no server. The free first month is the products' introductory
/// offer, set in App Store Connect (and Compter.storekit for local runs).
final class CompterStore {
  /// Monthly and yearly, in one subscription group
  static let productIds = ["app.compter.pro.monthly", "app.compter.pro.yearly"]

  private let channel: FlutterMethodChannel
  private var updates: Task<Void, Never>?

  init(messenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(name: "compter/store", binaryMessenger: messenger)
    channel.setMethodCallHandler { call, result in
      Task { @MainActor in
        do {
          result(try await Self.handle(call))
        } catch {
          result(FlutterError(code: "store", message: error.localizedDescription, details: nil))
        }
      }
    }
    // Renewals, refunds, Ask to Buy approvals and purchases made outside the app. Started at
    // launch, so StoreKit has somewhere to deliver them.
    updates = Task.detached { [channel] in
      for await update in Transaction.updates {
        guard case .verified(let transaction) = update else { continue }
        await transaction.finish()
        let entitlement = await Self.entitlement()
        await MainActor.run { channel.invokeMethod("entitlementChanged", arguments: entitlement) }
      }
    }
  }

  deinit {
    updates?.cancel()
  }

  @MainActor
  private static func handle(_ call: FlutterMethodCall) async throws -> Any? {
    switch call.method {
    case "entitlement":
      return await entitlement()
    case "plans":
      return await plans()
    case "purchase":
      guard let id = call.arguments as? String,
        let product = try await Product.products(for: [id]).first
      else { throw StoreError.unknownProduct }
      return try await purchase(product)
    case "restore":
      try await AppStore.sync()
      return await entitlement()
    case "manage":
      let scene = UIApplication.shared.connectedScenes
        .compactMap { $0 as? UIWindowScene }
        .first { $0.activationState == .foregroundActive }
      if let scene { try await AppStore.showManageSubscriptions(in: scene) }
      return nil
    default:
      return FlutterMethodNotImplemented
    }
  }

  /// `nil` when cancelled, or waiting for a parent's approval (Ask to Buy), which arrives
  /// later through [updates]
  @MainActor
  private static func purchase(_ product: Product) async throws -> [String: Any]? {
    switch try await product.purchase() {
    case .success(.verified(let transaction)):
      await transaction.finish()
      return await entitlement()
    case .success(.unverified(_, let error)):
      throw error
    case .userCancelled, .pending:
      return nil
    @unknown default:
      return nil
    }
  }

  /// The subscription the phone holds now, the one lasting longest when there are two (a
  /// switch from monthly to yearly). `active` is false with nothing else when there is none.
  static func entitlement() async -> [String: Any] {
    var current: Transaction?
    for await result in Transaction.currentEntitlements {
      guard case .verified(let transaction) = result,
        productIds.contains(transaction.productID),
        transaction.revocationDate == nil
      else { continue }
      let ends = transaction.expirationDate ?? .distantFuture
      if ends > (current?.expirationDate ?? .distantPast) { current = transaction }
    }
    guard let current else { return ["active": false] }
    var entitlement: [String: Any] = [
      "active": true,
      "productId": current.productID,
      "inTrial": current.offerType == .introductory,
      "willRenew": await willRenew(current),
    ]
    if let ends = current.expirationDate {
      entitlement["expiresAt"] = Int(ends.timeIntervalSince1970 * 1000)
    }
    return entitlement
  }

  /// Whether the subscription renews at its end; true when StoreKit can't say, so the app
  /// never tells someone their subscription ends when it doesn't
  private static func willRenew(_ transaction: Transaction) async -> Bool {
    guard let product = try? await Product.products(for: [transaction.productID]).first,
      let statuses = try? await product.subscription?.status
    else { return true }
    for status in statuses {
      guard case .verified(let renewal) = status.renewalInfo,
        renewal.originalTransactionID == transaction.originalID
      else { continue }
      return renewal.willAutoRenew
    }
    return true
  }

  /// Each plan with its price in the store's currency, and its free period while this Apple
  /// ID can still have one
  static func plans() async -> [[String: Any]] {
    guard let products = try? await Product.products(for: productIds) else { return [] }
    var plans: [[String: Any]] = []
    for product in products.sorted(by: { $0.price < $1.price }) {
      guard let subscription = product.subscription else { continue }
      var plan: [String: Any] = [
        "id": product.id,
        "price": product.displayPrice,
        "period": describe(subscription.subscriptionPeriod),
      ]
      if let offer = subscription.introductoryOffer, offer.paymentMode == .freeTrial,
        await subscription.isEligibleForIntroOffer
      {
        plan["freePeriod"] = describe(offer.period)
      }
      plans.append(plan)
    }
    return plans
  }

  /// `month`, `year`, or with a count, `2 weeks`
  private static func describe(_ period: Product.SubscriptionPeriod) -> String {
    let unit: String
    switch period.unit {
    case .day: unit = "day"
    case .week: unit = "week"
    case .month: unit = "month"
    case .year: unit = "year"
    @unknown default: unit = "period"
    }
    return period.value == 1 ? unit : "\(period.value) \(unit)s"
  }

  enum StoreError: LocalizedError {
    case unknownProduct

    var errorDescription: String? { "That plan isn’t in the App Store" }
  }
}
