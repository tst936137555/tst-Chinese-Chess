# tst 自用象棋 · Rust

离线中国象棋。Rust 2024 编写规则、对局、存档、引擎协议和界面；macroquad 提供 Windows、macOS、Android、iOS 的窗口、绘制、触摸和音效。Pikafish 保持上游 C++ 搜索引擎。

工具链固定 **Rust 1.99.1**，依赖由 `Cargo.lock` 锁定。

## 功能

- 执红/执黑、五档难度、合法落点、将军、悔棋、双建议、认输、按局势结束。
- 将死/困毙判负、三次重复、长将判负、60 回合无吃子判和。
- 后台原子保存、稳定对局 ID、版本继承校验与冲突分支另存、幂等归档；保留 1 局活动对局与最多 50 局旧对局，支持命名、逐局删除和批量清理；已归档棋谱也支持重命名；自动保留旧局备份，未来版本存档拒绝降级覆盖。
- 复盘可边分析边浏览、暂停与继续、按引擎及权重摘要校验的持久缓存；续下保留完整历史并建立独立身份。
- 窄屏纵排、横屏左右分栏、安全区输入映射、触摸棋谱滚动与最新着法跟随；收藏图标直接绘制避免缺字。
- 共用横竖屏界面、中文纵线记谱、内置中文字体和音效。
- 全新宣纸白、朱红、青绿配色；桌面首页双栏、手机纵排，统一卡片、按钮、状态提示与棋盘层次。新局在同一面板选择五档难度与执子。
- 原创五声音阶配乐，木质落子/吃子与柔和将军提示；44.1kHz 双声道、九档落点左右定位、轻混响、关键提示时音乐自动轻降。
- 首页统一「声音」入口，集中控制总静音、背景音乐与棋子音效的开关/音量并保存偏好；修复音频库的循环缓冲区断点并加入混音限幅。
- 细腻宣纸底纹、木纹棋盘与立体棋子；重试入口位于状态卡，不遮挡棋盘。再次点选或 Escape 取消选子，非法落点保留选中；吃子与空位落点分别显示。
- 棋谱收藏筛选、完整滚动裁切与空状态引导；复盘支持首步/末步、左右方向键、Home/End、点击或拖动走势定位。
- 声音滑杆可点选和拖动；横屏声音设置左右分栏，短确认弹窗按内容居中，终局操作立即可用。
- 引擎后台串行搜索、完整历史传递、同迭代 MultiPV 候选、超时和一次重试、旧请求取消；认输不依赖引擎。切后台后恢复走棋、提示、复盘和结算任务；结算保留实际评分类型、深度、耗时和资产摘要。
- Tab/Enter 操作按钮；B 进入棋盘，方向键选格，空格/Enter 落子，Escape 取消，M 静音。启动以新图标与加载条呈现，完成后直接进入主页；优先预缓存常用界面字形，玩法说明、关于与许可证按需准备；长文使用独立留白的可拖动滚动条。提供 Windows UIA / macOS NSAccessibility、Android 虚拟节点和 iOS 语义控件桥接；名称可通过读屏接口设置。移动端名称输入使用原生组合文本状态，缺失字体字符以 Unicode 编号显示并保留原文。读屏与输入法实机体验仍需目标平台验收。

桌面以 1080p 为初始清晰度基准，按 DPI 自动提高；移动端将额外超采样限制在原生像素量的 2.25 倍内。持续帧耗时过高时自动降低内部画布，稳定恢复后再提高，保留设备与显存余量，不增加分辨率选项。棋盘和棋子按密度缓存，静止页面降低绘制频率，长复盘曲线按像素保留极值并缓存；后台任务与输入仍持续处理。

棋谱页提供查找、备份导出和合并恢复。自动快照与手动导出位于数据目录的 `backups/`；自动保留最近八个有变更小时的快照，手动导出独立保留。损坏棋谱库可显式隔离坏局并保留原始文件及恢复报告；无法解析的 JSON 可恢复原备份。完整导出备份也可通过 `--import <备份路径>` 在其他设备合并导入。复盘可打开分页着法列表、跳转失误、独立翻转棋盘及重新分析。

## Windows

安装 [Rust](https://rustup.rs/) 和 Visual Studio C++ Build Tools（MSVC），首次获取引擎需要网络与 7-Zip：

```powershell
./tool/fetch_engine.ps1 -Target windows
cargo run --release --locked
```

只有 MinGW GCC 的机器可使用 GNU 工具链：

```powershell
rustup toolchain install 1.99.1-x86_64-pc-windows-gnu --component rustfmt --component clippy
cargo +1.99.1-x86_64-pc-windows-gnu run --release --locked
./tool/package_windows_rust.ps1 -Toolchain 1.99.1-x86_64-pc-windows-gnu
```

完整便携包输出 `dist/tst-xiangqi-2.1.1-windows.zip`，解压后是同名文件夹，根目录只放游戏运行程序。引擎、权重、许可证、说明、源码与构建清单各归对应目录，运行无需 Flutter。请完整解压后双击游戏 EXE，不要只移动 EXE。

```text
tst-xiangqi-2.1.1-windows/
├── tst-xiangqi.exe
├── assets/
│   ├── engine/       # 皮卡鱼引擎与 NNUE 权重
│   ├── fonts/        # 字体许可
│   └── licenses/     # 项目及第三方许可
├── readme/           # README 与更新记录
├── source/           # 对应版本源码与内置资源
└── metadata/         # 构建版本及文件 SHA256 清单
```

## macOS

```sh
pwsh ./tool/fetch_engine.ps1 -Target macos
cargo build --release --locked
./target/release/tst-xiangqi
```

分发时将引擎放在程序旁，将权重放在 `assets/engine/pikafish.nnue`，保留 `assets/licenses/`、`assets/fonts/OFL.txt` 和 `LICENSE`。也可用 `PIKAFISH_PATH`、`PIKAFISH_NNUE` 设置绝对路径。

## Android

需要 JDK 17、Android SDK 35、NDK r28c 或更新、Python 3.11+ 与 Rust。引擎通过 C ABI 静态链接，使用当前 Cargo 和 SDK 工具打包，无需 Flutter 或引擎子进程。

```sh
export ANDROID_HOME=/path/to/android/sdk
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
export NDK_HOME="$ANDROID_NDK_HOME"
pwsh ./tool/fetch_engine.ps1 -Target core
bash tool/build_android_rust.sh aarch64-linux-android
# 模拟器另行构建：
bash tool/build_android_rust.sh x86_64-linux-android
```

每次生成单 ABI APK，输出到 `dist/`，中间文件位于 `target/android-artifacts/`。包名 `com.tst.xiangqi.rust`，可与旧版并存。首次启动将包内 NNUE 安装到私有目录。所有发布 APK 均为项目正式证书签名的非调试包，缺少签名配置时直接失败。私有配置默认读取 ~/.tst-xiangqi/signing/key.properties，或设置 ANDROID_KEYSTORE、ANDROID_KEY_ALIAS、ANDROID_KEYSTORE_PASSWORD、ANDROID_KEY_PASSWORD；密钥及密码保留在仓库外。正式证书指纹、加密 Actions Secrets 配置、构建校验与升级要求见 [正式发布说明](docs/RELEASE.md)。签名不同不能覆盖升级，切勿卸载含数据的旧应用。

## iOS

需要 macOS、完整 Xcode、Python 3.11+、Rust。脚本直接构建引擎静态库和 Rust 应用，无需旧 Runner 工程。

```sh
pwsh ./tool/fetch_engine.ps1 -Target core
bash tool/build_ios_rust.sh aarch64-apple-ios-sim
xcrun simctl install booted dist/ios/aarch64-apple-ios-sim/tst-xiangqi.app
xcrun simctl launch booted com.tst.xiangqi.rust
# 真机未签名 .app：
bash tool/build_ios_rust.sh aarch64-apple-ios
```

真机需开发者自己的描述文件和代码签名。最低 iOS 15；引擎使用基础 NEON，不强制要求 dotprod。进程内引擎真挂死时需重启应用。[macroquad iOS 文档](https://macroquad.rs/articles/ios/)。

首页仅保留“继续未完成对局”和对局选择；已取消存档管理、完整错误查看、诊断导出/分享入口；存档读取失败时提供重试和备份恢复。对局继续自动保存；保存失败时保留重试/退出确认。

## 数据与旧版迁移

Rust 数据与旧版分开保存：Windows `%APPDATA%/tst-xiangqi-rust/state.json`；macOS/iOS `$HOME/Library/Application Support/tst-xiangqi-rust/state.json`；Android 私有 files 目录。`XIANGQI_DATA_DIR` 可覆盖目录。当前存档 schema 为 7，旧数据读取时迁移；显式删除身份会阻止旧窗口重新写回。设置独立保存到同目录的 `state.settings.json`，采用原子写入与 `state.settings.bak` 备份，不再因调音量重写整库；损坏设置先读取其备份或主存档内的迁移值，更新时另存损坏文件，未来版本设置拒绝覆盖。恢复主存档不会回退独立设置。棋谱日期统一显示北京时间（UTC+8）。

```powershell
cargo run --release -- --import "C:/path/to/game_archive.json"
```

导入旧 JSON 棋谱时逐步重放验证走法，重建局面和记谱，去重并保留原文件。旧 SharedPreferences 的偏好和未结束对局尚不自动迁移；不会跨移动端沙盒读取旧应用数据。

项目当前版本为 Rust 2.1.1。应用源码位于 `src/`，测试位于 `tests/`，移动端 C ABI 适配层位于 `native/pikafish/`，各平台图标集中在 `assets/icons/`，构建脚本位于 `tool/`。旧版源码可通过 Git 历史查阅。第三方组件与字体说明合并在本文末尾。

## 验证

Windows 可以先执行依赖检查，脚本会在缺少 MSVC 链接器且 GCC 可用时选择 GNU 工具链：

```powershell
./tool/check.ps1 -DoctorOnly -Python /path/to/python.exe
./tool/check.ps1 -WithEngine -Python /path/to/python.exe
```

新局面板明确提示简化裁决：长将判负、重复判和，未包含长捉判罚；该策略保持兼容旧棋谱。保存失败状态持续显示在对局卡片，可立即处理保存；重新读取不会覆盖待恢复着法，分歧恢复保留双方历史。设置、棋盘和复盘缓存各自确认，界面不会因后续失败而丢掉先前成功的收藏等操作。


```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets --locked
```

覆盖初始局面 perft（44 / 1920 / 79666）、规则、非法局面、长将、悔棋、UCI 历史、取消、MultiPV、终局依据、并发存档、幂等归档、对局、版本迁移、备份恢复与分析缓存。设置 `XIANGQI_REQUIRE_ENGINE=1` 时，缺少真实引擎会令测试失败。

便携包独立检查与隔离界面回归：

```sh
./target/release/tst-xiangqi --self-test
./target/release/tst-xiangqi --smoke-test target/ui-smoke
```

Windows 使用 `.exe` 文件。`--self-test` 验证真实引擎、历史、续局及归档；`--smoke-test` 使用指定输出目录保存测试数据，自动生成 29 个场景截图并验证横屏落子、新局设置、音量调节、选子取消、收藏筛选和复盘跳转。不会修改正常用户存档。Android 正式包可在模拟器运行 `python tool/mobile_smoke.py`。本轮通过 29 场景（含独立 2K/4K 内部画布）与原有视觉基准检查，字体首帧与稳定帧对比 RMS 为 0。

内部冒烟直接调用命中处理；另外提供真实输入回归，不能把截图通过解释为真机触摸已验收：Linux 在 Xvfb 下运行 `python3 tool/ui_input_smoke.py target/release/tst-xiangqi`（需要 xdotool）；Android 模拟器运行 `python3 tool/mobile_smoke.py`；macOS 构建模拟器 app 后运行 `python3 tool/ios_smoke.py dist/ios/aarch64-apple-ios-sim/tst-xiangqi.app`。iOS 基础脚本验证安装/启动/冷启动，CI 另外运行 XCTest 系统触摸、旋转、后台恢复和续局；物理设备体验仍需验收。

主项目新增保存失败/重读/分支/重启、异步响应、删除屏障、独立设置、评分类型转换、阻塞管道、渲染预算、Unicode 输入与规则局面回归，覆盖分歧历史、冲突确认、四类任务恢复、缓存空闲定时落盘/合并/失败退出、旧记录碰撞、对局容量、评分证据和固定随机种子，以及非法元数据导入、棋谱改名保留、备份恢复任务、设置失败确认、名称编辑和渲染尺寸边界。以下示例输出独立文件，使用临时数据，不改用户存档：

```sh
cargo run --release --locked --example check_ui -- target/ui-smoke
cargo run --release --locked --example font_regression -- target/font-regression
cargo run --release --locked --example benchmark -- target/performance-baseline.json
cargo run --release --locked --example calibrate -- target/difficulty-calibration.json --matches --max-plies 512 --pairs 8
cargo run --release --locked --example profile_ui -- target/ui-profile
cargo run --release --locked --example fuzz_inputs -- 10000
```

`check_ui` 比较固定逻辑视口的整体及局部区域，兼容 DPI；冒烟另检查语义节点身份、边界、禁用状态。`font_regression --density-cycle` 检查 1/1.25/1.5/2/1 倍字体缓存切换的首帧与预热画面；这模拟缓存密度，不代表系统缩放验收。`benchmark` 测量 150 局档案、独立设置与整库事务的分布和原始样本，以及五档搜索，可用 `XIANGQI_BENCH_DEVICE` 标记设备。`profile_ui` 依次启动五个独立进程，测量空库首页、150 局首页、普通对局、长复盘和 4K；记录启动阶段、实际画面重绘次数与呈现提交耗时，不并行跑性能样本。`calibrate` 使用 16 个固定局面；`--matches` 将相邻档位交换执子，运行到规则终局或 `--max-plies` 上限，`--pairs` 控制每组重复次数。未结束样本单列；胜率区间只统计分出胜负的棋局，和棋另列，不推断 ELO。移动设备显存、耗电和热稳定性仍需实机测量。

音频与布局检查包括左右声道定位、无削波、循环接点连续、混音缓冲跨界和棋盘适配。可用 `cargo test --manifest-path vendor/quad-snd/Cargo.toml --lib` 单独验证音频库修复；`cargo run --release --example render_audio` 输出离线 WAV 试听文件到 `dist/audio-preview/`。音乐与音效由 `src/audio.rs` 原创合成，无外部录音素材或额外运行时依赖。

Windows 可执行真实鼠标/键盘、UIA 焦点、中文提交与冷启动续局测试：`python tool/windows_input_smoke.py target/release/tst-xiangqi.exe`。macOS 使用 `python3 tool/macos_input_smoke.py target/release/tst-xiangqi`，测试进程需已有“辅助功能”权限，脚本不会更改权限。中文提交/粘贴回归不等于拼音组合输入和读屏实机验收。规则差分：先构建 `rules_probe` example，再执行 `python tool/rules_differential.py target/release/examples/rules_probe.exe`；macOS/Linux 去掉 `.exe` 并提供实际引擎路径。

CI 结构、复现方法及平台验收边界见 [CI 说明](docs/CI.md)。完整 Apple 链接、签名、目标设备安装、读屏、输入法组合、生命周期和功耗仍须在对应环境验收，不能视为已验证发行版。

自动保存正常时保持安静，仅失败时显示提示和处理按钮。关闭系统窗口会等待保存，失败时可重试或另存分支。

普通走棋不再保存整份偏好；同一窗口仅把实际修改的字段合并到磁盘最新设置。显式删除及容量淘汰都会阻止旧窗口重新写回同一身份。存档使用紧凑 JSON，已有旧格式仍可读取；相同事务跳过重复写入，首次恢复点仍会建立。

推送 `v*` tag 后，CI 先完成所有平台检查和构建，再验证 tag 与 Cargo 版本、提取该版本更新日志、为安装包生成 SHA256，最后复验实际 APK 签名与非调试属性，生成 SHA256 并直接发布 **正式 Release**。iOS 未签名及模拟器产物保留为 CI 验证附件，不进入正式发布资产；本地打包脚本不会发布。Actions 已固定提交，更新提交时需重新核验来源。每周 CI 执行受限变异输入检查；所有性能数据应以独立、串行采样为准。

## 第三方组件与字体说明

本项目源码：[tst936137555/tst-Chinese-Chess](https://github.com/tst936137555/tst-Chinese-Chess)，Copyright © 2026 tst-936137555，许可证全文见 [LICENSE](LICENSE)。

### Pikafish

- 固定版本：`Pikafish-2026-09-06`，Copyright © Pikafish contributors。
- [对应源码](https://github.com/official-pikafish/Pikafish/tree/Pikafish-2026-09-06)，GNU GPL v3.0。
- Windows/macOS 引擎由 `tool/fetch_engine.ps1` 按 `tool/engine_manifest.json` 下载并校验 SHA256；Android/iOS 由 `tool/build_mobile_engine.py` 从相同 tag 构建，适配代码位于 `native/pikafish/`。
- 构建版本、提交、目标和编译参数记录在 `mobile-engine/<target>/BUILD_INFO.txt`。分发引擎时附带许可证与对应源码。
- NNUE 权重的使用条件以随包附带的 `assets/licenses/NNUE-License.md` 为准；权重仅授权个人非商业用途，商业用途需向 Pikafish 团队另行申请许可。
- 引擎许可证全文由下载脚本取得，位于 `assets/licenses/COPYING-pikafish.txt`。

### 霞鹜文楷（LXGW WenKai）

- 子集基线版本：v1.522，SIL Open Font License 1.1。
- Copyright 2021-2026 LXGW，基于 Klee One 衍生（Copyright 2020 The Klee Project Authors）。[字体源码](https://github.com/lxgw/LxgwWenKai)。
- `assets/fonts/XqKai-Medium-subset.ttf` 由 `tool/subset_font.py` 生成；内部家族名改为 `XqKai`，避免使用 OFL 保留字体名标识修改版。
- 字符集包含 ASCII、`src/` 和 `tests/` 中的 Rust 源码字符、随包许可证文本、GB2312 一级常用字及棋子与标点补充集。
- 许可证全文见 [OFL.txt](assets/fonts/OFL.txt)，分发时随字体保留。

重新生成字体（需自行准备完整字体）：

```sh
python -m pip install fonttools
python tool/subset_font.py --input assets/fonts/LXGWWenKai-Medium.ttf
```

### Rust 界面与音频

- macroquad：MIT / Apache-2.0，依赖版本由 `Cargo.lock` 固定。
- quad-snd 0.2.8：MIT / Apache-2.0，本地依赖位于 `vendor/quad-snd/`；保留上游许可证、说明及 [本地修改记录](vendor/quad-snd/LOCAL_CHANGES.md)。
- 音乐与音效由 `src/audio.rs` 原创合成。
- 其他依赖使用各自上游许可证。

本地 `vendor/miniquad` 固定 0.4.11，修改 OpenGL 纹理删除后的绑定缓存失效处理；保留 MIT 与 Apache-2.0 原许可证。

### 系统无障碍适配

桌面语义树使用 [AccessKit](https://github.com/AccessKit/accesskit)（MIT / Apache-2.0），Windows/macOS 适配版本由 Cargo.lock 固定。Android 使用虚拟可访问节点，iOS 使用 UIKit 语义元素和原生名称输入框。编译通过或内部截图通过不等于读屏、输入法、旋转和真机生命周期已验收。


版本 2.0.3 增加逐局语义验证、导入容量事务保护、复盘缓存单调合并、收藏目标状态修订、独立设置恢复及退出状态机。schema 6 记录 simplified-v1 裁决版本；长期删除身份使用有界明细和时间屏障压缩，未知旧窗口身份需另存新分支，显式导入仍保留容量与重复检查。正式签名与发布流程见 [发布说明](docs/RELEASE.md)，第三方本地补丁见 [维护清单](docs/VENDOR_PATCHES.md)。


v2.1.1 优化复盘红黑优势走势与失误标记，陷入杀棋显示错招角标；改善按钮、长文字与短横屏布局，支持音量条滚轮调整。分析和提示统一使用最高难度参数，对弈难度与步时保持不变。

v2.1.0 修复缺失主存档与设置备份恢复、过期命名重试、Android JNI 引用存活期及 Windows 小数 DPI 尺寸损失。schema 7 增加独立名称修订号。续局与打开复盘采用后台重放，语义缓存独立校验元数据；复盘支持「参考变化」与实际分析依据。旧收藏兼容接口最多新增 4096 个去重凭证，既有重试保留，当前界面使用的带修订号操作不受影响。首次故障在数据目录的 diagnostics/ 内保留脱敏摘要并轮换一份旧报告。Android 只在对弈与复盘分析时保持亮屏。

依赖声明与本地补丁摘要可用 `python tool/dependency_inventory.py target/dependencies.json` 生成；每周维护工作流执行固定版本的 RustSec 扫描并保留结果。仓库只保留 `main` 分支，依赖更新直接在 `main` 上检查和维护。
