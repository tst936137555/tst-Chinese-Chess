import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    // 从 Bundle 中提取皮卡鱼引擎到应用支持目录
    if let enginePath = AppDelegate.extractEngine() {
      setenv("XIANGQI_ENGINE_PATH", enginePath, 1)
    }
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
  }

  // 复制束内皮卡鱼引擎到应用支持目录，并赋予可执行权限
  static func extractEngine() -> String? {
    let bundle = Bundle.main
    guard let src = bundle.path(forResource: "pikafish", ofType: nil) else { return nil }
    let fm = FileManager.default
    guard let dir = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else { return nil }
    let appDir = dir.appendingPathComponent("xiangqi", isDirectory: true)
    let dst = appDir.appendingPathComponent("pikafish")
    do {
      if !fm.fileExists(atPath: appDir.path) {
        try fm.createDirectory(at: appDir, withIntermediateDirectories: true)
      }
      // 每次启动重新拷贝，确保 bundle 内引擎升级后同步更新
      if fm.fileExists(atPath: dst.path) {
        try fm.removeItem(at: dst)
      }
      try fm.copyItem(at: URL(fileURLWithPath: src), to: dst)
      try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: dst.path)
      return dst.path
    } catch {
      NSLog("extractEngine error: \(error)")
      return nil
    }
  }
}
