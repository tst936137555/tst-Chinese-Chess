import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    // 打开时在屏幕可视区域（排除菜单栏与 Dock）居中，保持 XIB 设定的尺寸；
    // 屏幕过小时贴左下角兜底。macOS 坐标系原点在左下角，y 轴向上。
    if let visible = NSScreen.main?.visibleFrame {
      var frame = self.frame
      frame.origin.x = max(visible.minX, visible.midX - frame.width / 2)
      frame.origin.y = max(visible.minY, visible.midY - frame.height / 2)
      self.setFrameOrigin(frame.origin)
    }
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)

    super.awakeFromNib()
  }
}
