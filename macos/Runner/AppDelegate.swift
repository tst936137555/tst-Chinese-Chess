import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }

  // 复制束内皮卡鱼引擎到应用支持目录，并赋予可执行权限
  static func prepareEngine() -> String? {
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
      NSLog("prepareEngine error: \(error)")
      return nil
    }
  }

  override func applicationDidFinishLaunching(_ notification: Notification) {
    if let enginePath = AppDelegate.prepareEngine() {
      setenv("XIANGQI_ENGINE_PATH", enginePath, 1)
    }
    super.applicationDidFinishLaunching(notification)
  }
}
