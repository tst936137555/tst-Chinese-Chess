# CI 与本地复现

版本取自 `Cargo.toml`，Rust 工具链取自 `rust-toolchain.toml`；公共 setup action 安装同一 Python 3.13。

`ci.yml` 先执行格式、严格 Clippy、脚本回归、actionlint 和发行说明校验，再调用三个平台工作流：

| 工作流 | 检查 |
| --- | --- |
| `desktop.yml` | Windows/macOS/Linux 全目标测试、混音回归、真实引擎差分；Windows 打包与安装目录自检；独立 Linux 渲染、视觉基线、字体和真实 X11 输入 |
| `android.yml` | 两种 ABI 的 APK 构建、实际生成的 Java 桥接编译；独立模拟器安装本次 x86_64 APK，验证真实引擎、输入和续局 |
| `ios.yml` | UIKit 桥接提前语法检查、真机及模拟器完整链接；独立模拟器安装本次 `.app`，验证启动、引擎和冷启动 |

固定 runner 系列以减少镜像漂移；`macos-15` 使用 Apple Silicon，匹配 arm64 模拟器目标（[GitHub runner 说明](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)）。第三方 Actions 固定提交。构建与模拟器检查分开，后者下载前者产物，避免重建另一个包。失败时仍上传诊断。`CI passed` 必须看到所有依赖成功，失败、取消和跳过均不会放行。tag 构建不被新推送自动取消；只有通过完整 CI 的 tag 才发布正式 Release。

桌面、Android、iOS 冒烟均报告启动阶段并检测停止推进；桌面同时捕获进程退出和日志，Android 保留完整 logcat，避免最后几千行系统噪声覆盖启动错误。结果文件要求全部 29 个场景完成；移动端还必须通过真实引擎校验。Linux 不携带桌面引擎，真实引擎检查由 Windows/macOS 和移动端承担。

```powershell
./tool/check.ps1 -WithEngine -Python <Python 3.11+ 路径>
cargo build --release --locked --examples --bin tst-xiangqi
python tool/desktop_smoke.py target/release/tst-xiangqi.exe target/ui-smoke
python tool/windows_input_smoke.py target/release/tst-xiangqi.exe
```

Linux 运行图形测试需要 Xvfb、Openbox 和 ALSA null sink，具体命令见 `desktop.yml`。`tool/check.ps1` 的 `-Toolchain` 可选择已安装的本机 Rust host，例如 Windows GNU 工具链。工作流语法本地使用 actionlint 1.7.7 校验；CI 从固定 Go module 版本执行相同工具。

本地 Windows 检查不能替代 GitHub 的 Linux、macOS、Android、iOS 运行结果。签名、目标设备安装、读屏、输入法组合和功耗仍需对应设备验收。macOS 原生输入测试要求 runner 已有辅助功能权限，因此仅在手动工作流勾选时运行；其余平台检查始终执行。


v2.0.3 起 Android 受信任构建必须使用正式签名，正式包 debuggable=false，缺少密钥立即失败；外部 fork PR 仅执行未签名编译验证。正式包模拟器回归使用隔离外部目录，不再要求 run-as。发布任务再次验证真实 APK 证书、包名、版本和 SHA256 后直接生成正式 Release。iOS 构建以 verification-ios-* 保留，不进入正式发布资产。细节见 [正式发布说明](RELEASE.md)。


v2.1.0：Windows 增加真实截图与视觉基线检查，逐场景保存实际 DPI/视口；Android 开启 CheckJNI 并连续三次重建 Activity；iOS 在已有安装/引擎/冷启动检查后运行 `python tool/ios_ui_test.py`，XCTest 以隔离数据完成系统触摸、横屏、前后台和冷启动续局，结果保留为 xcresult。维护工作流定期检查依赖，清单记录所有目标平台的声明及 vendor 摘要。仓库仅保留 `main` 分支，不自动创建依赖更新分支。
