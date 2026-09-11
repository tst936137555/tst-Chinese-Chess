# tst自用象棋

本地单机中国象棋，内置皮卡鱼（Pikafish）引擎，无网络功能。

此游戏为 tst 自用象棋，自我学习使用（AI 辅助编程，融合个人想法）。

## 功能

- 本地单机对弈：执红先行 / 执黑后手，五档难度（入门 ~ 大师）
- 引擎支持：提示（双建议）、悔棋、按局势判定胜负结束对局
- 复盘分析：全局逐步引擎评估、局势走势折线图、走法质量分级
- 复盘棋谱：对局自动归档（未收藏最多 100 局、收藏最多 50 局，超出自动移除最旧），支持收藏置顶
- 中文记谱（纵线记谱法）、规则判定（将死/困毙、三次重复判和、长将判负、60 回合无吃子自然限着作和）、音效开关

## 支持平台

| 平台 | 引擎形态 | 说明 |
|------|----------|------|
| Android（arm64-v8a / x86_64） | 子进程（`libpikafish.so`） | 提供签名 APK 发布 |
| iOS（A12 及以上） | 进程内 FFI（静态库 `libpikafish.a`） | 需 macOS 构建，引擎依赖 dotprod 指令 |
| macOS | 子进程（随应用打包） | 引擎二进制随仓库提供 |
| Windows | 子进程（`pikafish.exe`） | 发布 zip 已内置引擎；本地构建需自行放置，见「构建」 |

不支持 Linux 与 Web。

## 安装

- **Android**：从 GitHub [Releases](https://github.com/tst936137555/tst-Chinese-Chess/releases) 下载 `tst_xiangqi-<版本>.apk` 直接安装（由 CI 自动构建签名，历史版本附件已清理，仅提供最新版本）。
- **Windows**：从 Releases 下载 `tst_xiangqi-<版本>-windows.zip`，解压后运行 `tst_xiangqi.exe`（已内置 Pikafish 引擎与许可证文件，由 CI 自动构建）。
- **iOS / macOS**：无预编译分发，请按下一节自行构建。

## 构建

前置要求：Flutter SDK（stable 渠道）、Android SDK（构建 Android）或 Xcode（构建 iOS/macOS）。

```bash
flutter pub get
flutter run                # 调试运行到已连接设备
```

各平台发布构建：

- **Android**：`flutter build apk --release`；或推送 `v*` 标签触发 CI 自动构建签名 APK（见 `.github/workflows/ci.yml`）。
- **iOS**：先在 macOS 上运行 `tool/setup_ios_engine.sh`（构建 `libpikafish.xcframework` 静态库并注入 Xcode 工程），之后 `flutter build ios`。
- **Windows**：`flutter build windows`，并将 [Pikafish 官方发布版](https://github.com/official-pikafish/pikafish/releases) 的 Windows 可执行文件重命名为 `pikafish.exe` 放入产物目录（开发调试时放项目根目录即可）。
- **macOS**：`flutter build macos`（引擎二进制已在 `macos/EngineBin/`）。

NNUE 权重由 fetch 脚本按 manifest 下载；字体（霞鹜文楷子集）与许可证文本已入库随 assets 打包，无需额外下载。引擎等大文件的下载源与版本钉住在 [tool/engine_manifest.json](tool/engine_manifest.json)：优先从上游官方 release 下载，失败时回退本仓库 [镜像 release](https://github.com/tst936137555/tst-Chinese-Chess/releases/tag/engine-mirror)（由 [Engine Mirror 工作流](.github/workflows/engine-mirror.yml)自动转存维护），防上游下架导致 CI 与本地构建断供。

## 发版流程

1. （可选）在 [CHANGELOG.md](CHANGELOG.md) 顶部新增 `## v<版本号>` 段落手写更新内容；不写则 CI 自动从提交历史按类型分组生成发行说明（`feat` → 新增、`fix` → 修复、`perf` → 优化、`refactor` → 重构、`docs/test/chore/ci/style/build` → 其他变更），支持 `feat(模块):` 作用域格式与全角冒号，最后统一附上 GitHub 自动生成的提交对比
2. 更新 `pubspec.yaml` 版本号与 `lib/ui/home_screen.dart` 首页版本标注，保持一致
3. 提交并推送到 main
4. 打 tag 并推送：`git tag v<版本号> && git push <remote> v<版本号>`
5. CI 自动构建 Android 签名 APK 与 Windows 引擎内置 zip，并创建带附件的 Release

## 测试

```bash
flutter analyze
flutter test
```

全部单元测试无需真实引擎二进制与平台通道：引擎可靠性测试使用内存伪造 UCI 引擎驱动真实会话协议；真实引擎端到端测试（`review_engine_integration_test.dart`）在无引擎环境自动跳过。

BoardView 渲染有 Golden 基线测试（`golden_test.dart`）：基线统一在 Windows 生成，其他平台自动跳过；CI 的 ubuntu job 跑全量逻辑测试，windows-test job 以同平台比对基线。更新基线：

```bash
flutter test --update-goldens test/golden_test.dart
```

## 已知限制

- 仅单机对弈：无网络对战、无账号与云同步功能。
- 随应用分发的 Pikafish NNUE 权重仅授权个人非商业用途，本应用及其衍生分发不得商用（见下文开源声明第 3 节）。
- iOS 需 A12 芯片（2018 年机型）及以上，旧设备不支持。
- 棋谱存档有数量上限（未收藏 100 局 / 收藏 50 局），超出自动移除最旧；存档保存在应用文档目录，卸载应用即清除。
- 引擎崩溃或挂死时会自动重启并重试一次，仍失败则如实提示「引擎不可用」，不会伪造走法或评分。
- Windows 发行版未做代码签名：首次运行时 SmartScreen 可能提示「Windows 已保护你的电脑」，点「更多信息 → 仍要运行」即可；请仅从 GitHub Releases 获取安装包。

## 作者声明

此游戏为 tst 自用象棋，自我学习使用。

## 开源软件声明 / Open Source Notices

本应用基于 GNU GPL v3.0 发布，包含以下开源组件：

### 1. 本应用程序

- 许可证：GNU GPL v3.0
- 版权：Copyright © 2026 tst-936137555
- 源码：https://github.com/tst936137555/tst-Chinese-Chess

### 2. Pikafish 引擎（皮卡鱼）

- 版本：v2026-09-06
- 许可证：GNU GPL v3.0
- 版权：Copyright © Pikafish contributors
- 源码：https://github.com/official-pikafish/pikafish
- 本应用包含 Pikafish 引擎代码，依 GPLv3 条款使用，本 App 源码已公开，满足 GPLv3 对应源码要求。
- Android 预编译来源：
  - arm64-v8a：官方发布版 `Pikafish-Android-arm64-universal`（随 [v2026-09-06 release](https://github.com/official-pikafish/pikafish/releases/tag/Pikafish-2026-09-06) 分发）；
  - x86_64：官方**未提供** Android x86_64 预编译版，由本项目依据精确源码版本与工具链自行编译，见下文「4. Android x86_64 引擎自编译说明」。
- Windows 预编译来源：官方发布版 `Pikafish-Windows-x86-64-universal`（随 [v2026-09-06 release](https://github.com/official-pikafish/pikafish/releases/tag/Pikafish-2026-09-06) 分发，由 CI 下载并经 SHA256 校验后随应用 zip 打包，GPLv3 与 NNUE 许可证文本随包附带）。

### 3. Pikafish NNUE 权重文件许可证

随 Pikafish 发布的权重文件（pikafish.nnue）：

- 仅限合法使用，超出合法范围使用的后果由用户自行承担；
- 仅授权个人非商业用途免费使用，任何商业用途须另向 Pikafish 团队申请商业许可。

本 App 为非商用项目，严格遵守上述限制。

### 4. Android x86_64 引擎自编译说明

`android/app/src/main/jniLibs/x86_64/libpikafish.so` 为自行编译产物，构建要素如下（可复现）：

- **精确源码版本**：official-pikafish/pikafish tag [`Pikafish-2026-09-06`](https://github.com/official-pikafish/pikafish/releases/tag/Pikafish-2026-09-06)
- **工具链**：Android NDK r28c（28.2.13676358），`COMP=ndk`（clang 交叉编译，API 29 目标 `x86_64-linux-android29-clang++`），静态链接 `-static`
- **构建命令**（在 POSIX 环境：Linux / macOS / WSL / MSYS2，需 make + git）：

  ```bash
  git clone --depth 1 --branch Pikafish-2026-09-06 https://github.com/official-pikafish/pikafish.git
  cd pikafish/src
  export PATH="$ANDROID_NDK/toolchains/llvm/prebuilt/<host>/bin:$PATH"
  make -j build COMP=ndk ARCH=x86_64 EXE=pikafish-x86_64 \
       LDFLAGS="-static -Wno-unused-command-line-argument"
  llvm-strip pikafish-x86_64
  cp pikafish-x86_64 <项目>/android/app/src/main/jniLibs/x86_64/libpikafish.so
  ```

- **一键脚本**：`tool/build_android_engine.sh`（自动定位 NDK、检出 tag、编译、strip 并安装到 jniLibs；`./tool/build_android_engine.sh x86_64`，也支持 `arm64-v8a` 重建）
- 说明：Pikafish `src/Makefile` 原生支持 `COMP=ndk`（`ARCH=x86_64` / `armv8`）；静态链接与官方 Android CI（arm64 universal）一致，避免依赖 `libc++_shared.so`；jniLibs 内必须以 `lib*.so` 命名，打包后安装至应用 `nativeLibraryDir` 供 `Process.start` 启动。

### 5. 霞鹜文楷字体（LXGW WenKai）

应用全局字体使用霞鹜文楷（Medium）的子集版本：

- 版本：v1.522（子集基线）
- 许可证：SIL Open Font License 1.1（OFL-1.1）
- 版权：Copyright 2021-2026 LXGW（保留字体名「霞鹜」「落霞孤鹜」「LXGW」等）；基于 Fontworks 开源的 Klee One 衍生（Copyright 2020 The Klee Project Authors）
- 源码：https://github.com/lxgw/LxgwWenKai
- 入库资产 `assets/fonts/XqKai-Medium-subset.ttf`（约 1.7MB）由 [tool/subset_font.py](tool/subset_font.py) 从完整字体（约 24MB，不入库）子集化生成：字符集 = ASCII + lib/test 源码扫描 + GB2312 一级常用字 + 补充集；内部家族名已改写为 `XqKai`（OFL 规定保留字体名不得用于标识修改版，pubspec 与代码引用本就使用 XqKai，行为不变）；
- 完整版可从[上游 release](https://github.com/lxgw/LxgwWenKai/releases) 或本仓库 [镜像 release](https://github.com/tst936137555/tst-Chinese-Chess/releases/tag/engine-mirror) 下载后重新生成子集：

  ```bash
  python -m pip install fonttools
  python tool/subset_font.py --input assets/fonts/LXGWWenKai-Medium.ttf
  ```

- 修改 UI 文案后若个别字显示为系统字体（缺字回退），重跑上述脚本并重新提交子集资产；
- 许可证全文随应用分发：`assets/fonts/OFL.txt`（入库并打包进 APK，满足 OFL「分发字体须附带许可证副本」要求）。

## 许可证

完整 GPLv3 许可证全文见：

- 本仓库 [LICENSE](LICENSE)
- https://www.gnu.org/licenses/gpl-3.0.txt

GPLv3（Pikafish `Copying.txt`）与 NNUE 许可证全文由 `tool/fetch_engine.ps1` 从官方发布包取得（SHA256 校验），作为 Flutter 资产（`assets/licenses/`）随 Android / iOS / macOS / Windows 各平台安装包分发，App 内「作者声明」对话框可离线查看。

霞鹜文楷字体许可证全文见 [assets/fonts/OFL.txt](assets/fonts/OFL.txt)（SIL OFL 1.1）。
