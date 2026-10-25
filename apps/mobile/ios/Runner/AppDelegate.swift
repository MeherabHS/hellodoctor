import Flutter
import UIKit

// iOS has no FLAG_SECURE equivalent to block screenshots/screen-recording
// outright; the standard mitigation is to cover the app's own content with a
// blur the instant it stops being the active app, since that's the exact
// frame iOS freezes into the app-switcher snapshot and into a screen
// recording of the switcher. Screen content here is clinical (consultation
// video, prescriptions, Health Vault documents) — this closes the same
// PHI-exposure side door the Android build's FLAG_SECURE closes natively.
private let privacyOverlayTag = 987654321

private func showPrivacyOverlay(in window: UIWindow?) {
  guard let window = window, window.viewWithTag(privacyOverlayTag) == nil else { return }
  let blur = UIVisualEffectView(effect: UIBlurEffect(style: .dark))
  blur.tag = privacyOverlayTag
  blur.frame = window.bounds
  blur.autoresizingMask = [.flexibleWidth, .flexibleHeight]
  window.addSubview(blur)
}

private func hidePrivacyOverlay(in window: UIWindow?) {
  window?.viewWithTag(privacyOverlayTag)?.removeFromSuperview()
}

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  override func applicationWillResignActive(_ application: UIApplication) {
    showPrivacyOverlay(in: window)
    super.applicationWillResignActive(application)
  }

  override func applicationDidBecomeActive(_ application: UIApplication) {
    hidePrivacyOverlay(in: window)
    super.applicationDidBecomeActive(application)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    let channel = FlutterMethodChannel(name: "hellodoctor/build_metadata",
      binaryMessenger: engineBridge.applicationRegistrar.messenger())
    channel.setMethodCallHandler { call, result in
      guard call.method == "get" else { result(FlutterMethodNotImplemented); return }
      result([
        "app_version": Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "",
        "build_number": Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? ""
      ])
    }
  }
}
