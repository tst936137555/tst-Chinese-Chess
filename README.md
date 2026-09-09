# tst自用象棋

本地单机中国象棋，内置皮卡鱼（Pikafish）引擎，无网络功能。

此游戏为 tst 自用象棋，自我学习使用（AI 辅助编程，融合个人想法）。

## 功能

- 本地单机对弈：执红先行 / 执黑后手，五档难度（入门 ~ 大师）
- 引擎支持：提示（双建议）、悔棋、按局势判定胜负结束对局
- 复盘分析：全局逐步引擎评估、局势走势折线图、走法质量分级
- 复盘棋谱：对局自动归档（最多保留 100 局），支持收藏（置顶且不被自动移除）
- 中文记谱（纵线记谱法）、规则判定（将死/困毙、三次重复判和、长将判负）、音效开关

## 作者声明

此游戏为 tst 自用象棋，自我学习使用。

## 开源软件声明 / Open Source Notices

本应用基于 GNU GPL v3.0 发布，包含以下开源组件：

### 1. 本应用程序

- 许可证：GNU GPL v3.0
- 版权：Copyright © 2026 tst-936137555
- 源码：https://github.com/tst936137555/tst-Chinese-Chess（tag: v1.3.2）

### 2. Pikafish 引擎（皮卡鱼）

- 版本：v2026-09-06
- 许可证：GNU GPL v3.0
- 版权：Copyright © Pikafish contributors
- 源码：https://github.com/official-pikafish/pikafish
- 本应用包含 Pikafish 引擎代码，依 GPLv3 条款使用，本 App 源码已公开，满足 GPLv3 对应源码要求。
- Android 预编译来源：
  - arm64-v8a：官方发布版 `Pikafish-Android-arm64-universal`（随 [v2026-09-06 release](https://github.com/official-pikafish/pikafish/releases/tag/Pikafish-2026-09-06) 分发）；
  - x86_64：官方**未提供** Android x86_64 预编译版，由本项目依据精确源码版本与工具链自行编译，见下文「4. Android x86_64 引擎自编译说明」。

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

应用全局字体使用霞鹜文楷（Medium）：

- 版本：v1.522
- 许可证：SIL Open Font License 1.1（OFL-1.1）
- 版权：Copyright 2021-2026 LXGW（保留字体名「霞鹜」「落霞孤鹜」「LXGW」等）；基于 Fontworks 开源的 Klee One 衍生（Copyright 2020 The Klee Project Authors）
- 源码：https://github.com/lxgw/LxgwWenKai
- 许可证全文随应用分发：`assets/fonts/OFL.txt`（同时打包进 APK，满足 OFL「分发字体须附带许可证副本」要求）。

## 许可证

完整 GPLv3 许可证全文见：

- 本仓库 [LICENSE](LICENSE)
- https://www.gnu.org/licenses/gpl-3.0.txt

霞鹜文楷字体许可证全文见 [assets/fonts/OFL.txt](assets/fonts/OFL.txt)（SIL OFL 1.1）。
