#!/usr/bin/env bash
# =============================================================================
# setup_ios_engine.sh — iOS 进程内引擎一键构建与集成（仅 macOS 运行）
#
# 做四件事：
#   1. 检出精确 tag 的 Pikafish 源码（与 tool/build_android_engine.sh 同版本）
#   2. 应用 Makefile iOS 补丁（ios/EngineShim/pikafish_ios_makefile.patch）
#   3. 以 -DUNIVERSAL_BINARY 编译 真机(iphoneos) + 模拟器(iphonesimulator) 两个
#      arm64 静态库切片（含 C 适配层 pikafish_shim.cpp），打成 xcframework
#      （两切片同为 arm64，lipo 无法合并，xcframework 由 Xcode 按平台选切片）
#   4. 向 ios/Runner.xcodeproj/project.pbxproj 幂等注入链接配置
#      （libpikafish.xcframework 挂 Frameworks + per-sdk -Wl,-force_load，
#      防止 Dart FFI 符号被链接器裁剪）
#
# 产物：ios/EngineBin/libpikafish.xcframework
# 之后即可 flutter run/build ios；未运行本脚本时 iOS 按现状优雅降级
# （引擎功能如实报 EngineUnavailableException，其余功能不受影响）。
#
# 关键设计（见项目 iOS FFI 可行性评估）：
#   -DUNIVERSAL_BINARY 把 main() 收编为可调用的 Stockfish::main（上游自带
#   机制），C 适配层（ios/EngineShim/）在独立线程驱动它并重定向
#   stdin/stdout 为行队列，UCI 行协议与子进程模式完全一致。
#
# 设备要求：目标 CPU 需 ARMv8.2 dotprod（2018 年 A12 起全部支持）。
# 许可证：Pikafish 为 GPL-3.0，上架需遵守相应源码提供义务。
#
# 用法：
#   ./tool/setup_ios_engine.sh            # 默认 tag + apple-silicon 优化
#   PIKAFISH_TAG=<tag> ./tool/setup_ios_engine.sh
#   ARCH=armv8 ./tool/setup_ios_engine.sh # 退化为纯 NEON（无 dotprod 提速）
# =============================================================================
set -euo pipefail

readonly TAG="${PIKAFISH_TAG:-Pikafish-2026-09-06}"
readonly REPO="https://github.com/official-pikafish/pikafish.git"
readonly ARCH="${ARCH:-apple-silicon}"   # apple-silicon = arm64 + NEON + dotprod
readonly MIN_IOS="${MIN_IOS:-15.0}"      # 与 Runner IPHONEOS_DEPLOYMENT_TARGET 一致
readonly PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly SHIM_DIR="$PROJECT_ROOT/ios/EngineShim"
readonly ENGINE_DIR="$PROJECT_ROOT/ios/EngineBin"
readonly PBXPROJ="$PROJECT_ROOT/ios/Runner.xcodeproj/project.pbxproj"

usage() {
  sed -n '2,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  exit 0
}
case "${1:-}" in
  -h|--help) usage ;;
  "") ;;
  *) echo "未知参数：$1（-h 查看用法）" >&2; exit 1 ;;
esac

log() { printf '[setup_ios_engine] %s\n' "$*"; }
die() { echo "setup_ios_engine: $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# 0. 环境检查（仅 macOS；需 Xcode 命令行工具）
# ---------------------------------------------------------------------------
case "$(uname -s)" in
  Darwin*) ;;
  *) die "本脚本仅支持 macOS（需 Xcode 构建引擎静态库）。当前系统：$(uname -s)" ;;
esac
for cmd in git make xcrun ar ranlib python3; do
  command -v "$cmd" >/dev/null 2>&1 || die "缺少 $cmd（请安装 Xcode Command Line Tools：xcode-select --install）"
done
for f in "$SHIM_DIR/pikafish_shim.cpp" "$SHIM_DIR/pikafish_shim.h" \
         "$SHIM_DIR/pikafish_ios_makefile.patch" "$PBXPROJ"; do
  [ -f "$f" ] || die "缺少项目文件：$f"
done
xcrun -sdk iphoneos --show-sdk-path >/dev/null 2>&1 || die "未找到 iPhoneOS SDK（请安装完整 Xcode）"
xcrun -sdk iphonesimulator --show-sdk-path >/dev/null 2>&1 || die "未找到 iPhoneSimulator SDK（请安装完整 Xcode）"

JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"
log "源码版本：$TAG（$REPO）"
log "架构：ARCH=$ARCH，最低 iOS=$MIN_IOS，并行度=$JOBS"

# ---------------------------------------------------------------------------
# 1-2. 检出源码 + 应用 iOS 补丁
# ---------------------------------------------------------------------------
readonly WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
log "检出源码到 $WORK/pikafish"
git clone --depth 1 --branch "$TAG" "$REPO" "$WORK/pikafish"
( cd "$WORK/pikafish" && git describe --tags --always )

if grep -q "IOS_PLATFORM" "$WORK/pikafish/src/Makefile"; then
  log "Makefile 已含 iOS 支持，跳过补丁"
else
  patch -d "$WORK/pikafish" -p1 --forward \
    < "$SHIM_DIR/pikafish_ios_makefile.patch" \
    || die "Makefile 补丁应用失败（tag 源码可能已漂移，需重新生成补丁）"
  grep -q "IOS_PLATFORM" "$WORK/pikafish/src/Makefile" || die "补丁校验失败"
  log "已应用 Makefile iOS 补丁"
fi

cp "$SHIM_DIR/pikafish_shim.cpp" "$SHIM_DIR/pikafish_shim.h" "$WORK/pikafish/src/"
log "已复制 C 适配层到 src/（随引擎一同编译进静态库）"

# ---------------------------------------------------------------------------
# 3. 编译两个 arm64 切片并打包为 xcframework
#    注意：两次构建共享 src/ 对象目录，第二轮前必须 clean（make 不感知旗标变化）
# ---------------------------------------------------------------------------
build_slice() {
  local platform="$1" out="$2"
  log "构建 $platform 切片 ..."
  make -C "$WORK/pikafish/src" clean >/dev/null
  make -C "$WORK/pikafish/src" -j"$JOBS" build \
    COMP=clang ARCH="$ARCH" \
    IOS_PLATFORM="$platform" IOS_MIN_VER="$MIN_IOS" \
    EXTRACXXFLAGS="-DUNIVERSAL_BINARY"
  ( cd "$WORK/pikafish/src" && ar rcs "$out" *.o )
  log "已产出 $out"
}

DEVICE_A="$WORK/libpikafish-iphoneos.a"
SIM_A="$WORK/libpikafish-iphonesim.a"
build_slice iphoneos "$DEVICE_A"
build_slice iphonesimulator "$SIM_A"

# 真机/模拟器切片同为 arm64，lipo 无法合并同架构切片
# （fatal error: ... have the same specified architectures (arm64)），
# 必须用 xcframework 让 Xcode 按目标平台自动选择切片。
mkdir -p "$ENGINE_DIR"
readonly ENGINE_XCFRAMEWORK="$ENGINE_DIR/libpikafish.xcframework"
rm -rf "$ENGINE_XCFRAMEWORK"   # -create-xcframework 要求输出目录不存在
xcrun xcodebuild -create-xcframework \
  -library "$DEVICE_A" \
  -library "$SIM_A" \
  -output "$ENGINE_XCFRAMEWORK"
ls "$ENGINE_XCFRAMEWORK" | sed 's/^/[setup_ios_engine]   /'

cat > "$ENGINE_DIR/ENGINE_BUILD_INFO.txt" <<EOF
Pikafish iOS 进程内引擎静态库
tag:        $TAG
arch:       $ARCH（真机+模拟器 arm64 双切片，xcframework 分发）
min_ios:    $MIN_IOS
extras:     -DUNIVERSAL_BINARY（main 收编为 Stockfish::main）
构建日期:   $(date '+%Y-%m-%d %H:%M:%S %z')
脚本:       tool/setup_ios_engine.sh
EOF
log "已写入 $ENGINE_DIR/ENGINE_BUILD_INFO.txt"

# ---------------------------------------------------------------------------
# 4. pbxproj 幂等注入：libpikafish.xcframework 引用 + per-sdk -force_load
# ---------------------------------------------------------------------------
# xcframework 挂进 Frameworks 构建阶段后由 Xcode 按目标平台自动选切片，
# 但 Dart FFI 符号仅经 dlsym 查找、不产生未解析引用，静态库成员默认
# 不会被链接器拉入，仍需 -force_load 强制整体链接。force_load 不识别
# xcframework 容器，须按 sdk 指到内部切片目录（ios-arm64 / ios-arm64-simulator）。
python3 - "$PBXPROJ" <<'PY'
import re
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as f:  # 保留 BOM（若存在）
    src = f.read()

if "libpikafish.xcframework" in src:
    print("[setup_ios_engine] pbxproj 已注入过，跳过")
    sys.exit(0)

# 稳定 ID（24 位十六进制，避开常规生成段）
FR = "9E2100002DC9AE000000000001"  # PBXFileReference
BF = "9E2100002DC9AE000000000002"  # PBXBuildFile
GR = "9E2100002DC9AE000000000003"  # EngineBin group


def insert_before(section_end: str, block: str) -> None:
    global src
    assert section_end in src, f"锚点缺失: {section_end}"
    src = src.replace(section_end, block + section_end, 1)


insert_before("/* End PBXBuildFile section */",
              f"\t\t{BF} /* libpikafish.xcframework in Frameworks */ = "
              f"{{isa = PBXBuildFile; fileRef = {FR} /* libpikafish.xcframework */; }};\n")
insert_before("/* End PBXFileReference section */",
              f"\t\t{FR} /* libpikafish.xcframework */ = {{isa = PBXFileReference; "
              f"lastKnownFileType = wrapper.xcframework; path = libpikafish.xcframework; "
              f"sourceTree = \"<group>\"; }};\n")
insert_before("/* End PBXGroup section */",
              f"\t\t{GR} /* EngineBin */ = {{\n"
              f"\t\t\tisa = PBXGroup;\n"
              f"\t\t\tchildren = (\n"
              f"\t\t\t\t{FR} /* libpikafish.xcframework */,\n"
              f"\t\t\t);\n"
              f"\t\t\tpath = EngineBin;\n"
              f"\t\t\tsourceTree = \"<group>\";\n"
              f"\t\t}};\n")

# 挂入 mainGroup children
m = re.search(r"mainGroup = ([0-9A-F]{24});", src)
assert m, "未找到 mainGroup"
main_id = m.group(1)
mm = re.search(r"\n\t\t" + main_id +
               r"(?: /\* [^*]+ \*/)? = \{\n\t+isa = PBXGroup;\n\t+children = \(\n",
               src)
assert mm, "未找到 mainGroup children"
src = src[:mm.end()] + f"\t\t\t\t{GR} /* EngineBin */,\n" + src[mm.end():]

# 加入 Frameworks 构建阶段（Flutter iOS 模板仅 Runner 一个 Frameworks 阶段）
phases = re.findall(
    r"\n\t\t([0-9A-F]{24}) /\* Frameworks \*/ = \{\n\t+isa = PBXFrameworksBuildPhase;",
    src)
assert len(phases) == 1, f"Frameworks 阶段数量异常: {len(phases)}"
fm = re.search(
    r"\n\t\t" + phases[0] +
    r" /\* Frameworks \*/ = \{\n\t+isa = PBXFrameworksBuildPhase;\n"
    r"\t+buildActionMask = [^;]+;\n\t+files = \(\n", src)
assert fm, "未找到 Frameworks files 列表"
src = src[:fm.end()] + f"\t\t\t\t{BF} /* libpikafish.xcframework in Frameworks */,\n" + src[fm.end():]

# Runner 目标三个配置（Debug/Release/Profile）按 sdk 注入 force_load：
#   $(SRCROOT) = 含 Runner.xcodeproj 的 ios/ 目录（勿再拼一层 ios/）
#   xcframework 内部切片目录名由 -create-xcframework 按平台约定生成
LDFLAGS_DEVICE = ('\t\t\t\tOTHER_LDFLAGS[sdk=iphoneos*] = '
                  '"-Wl,-force_load,$(SRCROOT)/EngineBin/libpikafish.xcframework'
                  '/ios-arm64/libpikafish.a";\n')
LDFLAGS_SIM = ('\t\t\t\tOTHER_LDFLAGS[sdk=iphonesimulator*] = '
               '"-Wl,-force_load,$(SRCROOT)/EngineBin/libpikafish.xcframework'
               '/ios-arm64-simulator/libpikafish.a";\n')
configs = re.findall(
    r"([0-9A-F]{24}) /\* (?:Debug|Release|Profile) \*/ = \{", src)
assert len(configs) >= 3, "未找到足够 XCBuildConfiguration"
injected = 0
for cid in configs:
    cm = re.search(
        r"\n\t\t" + cid +
        r" /\* (?:Debug|Release|Profile) \*/ = \{\n"
        r"\t+isa = XCBuildConfiguration;\n"
        r"\t+baseConfigurationReference = [^;]+;\n\t+buildSettings = \{\n",
        src)
    if cm is None:
        continue  # 项目级配置（无 baseConfigurationReference）不注入
    src = src[:cm.end()] + LDFLAGS_DEVICE + LDFLAGS_SIM + src[cm.end():]
    injected += 1
assert injected >= 3, f"OTHER_LDFLAGS 注入数量异常: {injected}"

with open(path, "w", encoding="utf-8", newline="") as f:
    f.write(src)
print(f"[setup_ios_engine] pbxproj 注入完成（OTHER_LDFLAGS x{injected} 组）")
PY

grep -q "libpikafish.xcframework" "$PBXPROJ" || die "pbxproj 注入校验失败"

log "全部完成。下一步：flutter run -d <iOS 设备/模拟器>"
log "提示：模拟器需 Apple Silicon Mac（arm64 切片）；引擎功能要求 2018 年（A12）及以后的机型。"
