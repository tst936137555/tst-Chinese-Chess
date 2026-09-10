#!/usr/bin/env bash
# =============================================================================
# build_android_engine.sh — 构建安卓版 Pikafish 引擎（libpikafish.so）
#
# 可复现构建说明（与 README「Android 引擎自编译说明」一致）：
#   - 精确源码版本：official-pikafish/pikafish tag Pikafish-2026-09-06
#     https://github.com/official-pikafish/pikafish/releases/tag/Pikafish-2026-09-06
#   - 工具链：Android NDK r28c（28.2.13676358），COMP=ndk
#     clang 交叉编译，API 29 目标（x86_64-linux-android29-clang++ /
#     aarch64-linux-android29-clang++），静态链接 -static
#
# Pikafish 官方未发布 Android x86_64 预编译版（官方仅有 Android arm64），
# 因此 x86_64 供模拟器使用的 libpikafish.so 由本脚本自行编译。
# arm64-v8a 可直接使用官方预编译版，也可用本脚本（ARCH=armv8）重建。
#
# 用法（在 POSIX 环境：Linux / macOS / WSL / MSYS2-GitBash，需 make + git）：
#   ./tool/build_android_engine.sh x86_64      # 构建 x86_64（默认）
#   ./tool/build_android_engine.sh arm64-v8a   # 构建 arm64-v8a
#   ANDROID_NDK=/path/to/ndk ./tool/build_android_engine.sh x86_64
#
# NDK 定位顺序：$ANDROID_NDK → $ANDROID_SDK_ROOT/ndk/* → $ANDROID_HOME/ndk/*
#   → Windows 常见位置 %LOCALAPPDATA%\Android\sdk\ndk\*（取版本号最大者）。
#
# 产物自动安装到 android/app/src/main/jniLibs/<abi>/libpikafish.so。
# Android 要求 jniLibs 内以 lib*.so 命名；打包后解压至应用 nativeLibraryDir
# （可读可执行），引擎经 Process.start 从该目录启动。
# =============================================================================
set -euo pipefail

readonly TAG="${PIKAFISH_TAG:-Pikafish-2026-09-06}"
readonly REPO="https://github.com/official-pikafish/pikafish.git"
readonly PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly JNI_LIBS="$PROJECT_ROOT/android/app/src/main/jniLibs"

usage() {
  sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  exit 0
}

case "${1:-x86_64}" in
  -h|--help) usage ;;
  x86_64)    ABI="x86_64";    ARCH="x86_64" ;;  # Makefile: x86_64 → x86_64-linux-android29-clang++
  arm64-v8a) ABI="arm64-v8a"; ARCH="armv8"  ;;  # Makefile: armv8  → aarch64-linux-android29-clang++
  *) echo "不支持的 ABI：$1（可选 x86_64 / arm64-v8a）" >&2; exit 1 ;;
esac

log() { printf '[build_android_engine] %s\n' "$*"; }

# ---------------------------------------------------------------------------
# 1. 定位 NDK（需要 r27c+，Makefile 对 ndk 的要求；本项目验证于 r28c）
# ---------------------------------------------------------------------------
find_ndk() {
  local candidates=()
  [ -n "${ANDROID_NDK:-}" ] && candidates+=("$ANDROID_NDK")
  for root in "${ANDROID_SDK_ROOT:-}" "${ANDROID_HOME:-}" \
              "${LOCALAPPDATA:-}/Android/sdk" "$HOME/Android/Sdk" \
              "$HOME/Library/Android/sdk"; do
    [ -n "$root" ] && [ -d "$root/ndk" ] && \
      for d in "$root/ndk"/*; do [ -d "$d" ] && candidates+=("$d"); done
  done
  # 取版本号（目录名）最大的候选
  local best=""
  for c in "${candidates[@]:-}"; do
    [ -d "$c/toolchains/llvm" ] || continue
    [ -z "$best" ] || [ "$(basename "$c")" \> "$(basename "$best")" ] && best="$c"
  done
  [ -n "$best" ] || {
    echo "未找到 Android NDK。请安装 NDK r28c (28.2.13676358) 或设置 ANDROID_NDK。" >&2
    exit 1
  }
  printf '%s' "$best"
}

readonly NDK="$(find_ndk)"
readonly NDK_HOST_CASE="$(uname -s)"
case "$NDK_HOST_CASE" in
  Darwin*) NDK_HOST="darwin-x86_64" ;;
  *_NT*|MINGW*|MSYS*|CYGWIN*) NDK_HOST="windows-x86_64" ;;
  *) NDK_HOST="linux-x86_64" ;;
esac
readonly NDK_BIN="$NDK/toolchains/llvm/prebuilt/$NDK_HOST/bin"

log "NDK：$NDK"
log "NDK 工具链：$NDK_BIN"
log "源码版本：$TAG（$REPO）"
log "目标 ABI：$ABI（ARCH=$ARCH）"

command -v make >/dev/null 2>&1 || { echo "缺少 make（Linux/macOS 自带；Windows 请用 WSL 或 MSYS2）" >&2; exit 1; }
[ -x "$NDK_BIN/clang++" ] || { echo "NDK 工具链不完整：$NDK_BIN" >&2; exit 1; }

# ---------------------------------------------------------------------------
# 2. 检出精确 tag
# ---------------------------------------------------------------------------
readonly WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
log "检出源码到 $WORK/pikafish"
git clone --depth 1 --branch "$TAG" "$REPO" "$WORK/pikafish"
( cd "$WORK/pikafish" && git describe --tags --always )

# ---------------------------------------------------------------------------
# 3. 编译（COMP=ndk，静态链接；与官方 Android CI 一致）
#   产物：$WORK/pikafish/src/pikafish-$ABI
# ---------------------------------------------------------------------------
make -C "$WORK/pikafish/src" -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || nproc 2>/dev/null || echo 4)" \
  build COMP=ndk ARCH="$ARCH" EXE="pikafish-$ABI" \
  LDFLAGS="-static -Wno-unused-command-line-argument"

"$NDK_BIN/llvm-strip" "$WORK/pikafish/src/pikafish-$ABI"

# ---------------------------------------------------------------------------
# 4. 安装到 jniLibs
# ---------------------------------------------------------------------------
mkdir -p "$JNI_LIBS/$ABI"
cp "$WORK/pikafish/src/pikafish-$ABI" "$JNI_LIBS/$ABI/libpikafish.so"
log "已安装：$JNI_LIBS/$ABI/libpikafish.so"
file "$JNI_LIBS/$ABI/libpikafish.so" 2>/dev/null || true
log "完成。重新执行 flutter build apk 即可打包新引擎。"
