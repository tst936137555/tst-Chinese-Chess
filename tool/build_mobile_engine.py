#!/usr/bin/env python3
"""Build the pinned Pikafish C ABI for Rust on Android/iOS (no Flutter required)."""
import argparse
import hashlib
import concurrent.futures
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def run(args, log=True, **kw):
    if log:
        print("+", " ".join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), check=True, **kw)


def output(args):
    return subprocess.check_output(list(map(str, args)), text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", choices=["aarch64-linux-android", "x86_64-linux-android", "aarch64-apple-ios", "aarch64-apple-ios-sim"])
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 2, 4))
    args = parser.parse_args()
    target = args.target
    manifest = json.loads((ROOT / "tool/engine_manifest.json").read_text(encoding="utf-8"))
    tag = manifest["pikafish"]["tag"]
    work = ROOT / "mobile-engine" / target
    source = work / "source"
    work.mkdir(parents=True, exist_ok=True)
    if not source.exists():
        run(["git", "clone", "--depth", "1", "--branch", tag, "https://github.com/official-pikafish/Pikafish.git", source])
    if output(["git", "-C", source, "describe", "--tags", "--exact-match"]) != tag:
        raise SystemExit("引擎版本不符，请重新检出清单中的精确 tag")
    commit = output(["git", "-C", source, "rev-parse", "HEAD"])
    if commit != manifest["pikafish"]["commit"]:
        raise SystemExit("引擎提交与清单不符")
    if output(["git", "-C", source, "status", "--porcelain", "--untracked-files=normal"]):
        raise SystemExit("引擎源码存在未记录修改，请先还原或显式更新清单")
    flags = ["-std=c++17", "-O3", "-DNDEBUG", "-DUNIVERSAL_BINARY", "-DIS_64BIT", "-fno-exceptions", "-fPIC", "-pthread"]
    if "android" in target:
        ndk_value = os.environ.get("ANDROID_NDK_HOME") or os.environ.get("ANDROID_NDK") or os.environ.get("NDK_HOME")
        if not ndk_value:
            raise SystemExit("请设置 ANDROID_NDK_HOME（NDK r28c 或更新）")
        host = {"Linux": "linux-x86_64", "Darwin": "darwin-x86_64", "Windows": "windows-x86_64"}[platform.system()]
        tools = Path(ndk_value) / "toolchains/llvm/prebuilt" / host
        compiler = tools / "bin/clang++"
        if platform.system() == "Windows":
            compiler = compiler.with_suffix(".exe")
        triple = "aarch64-linux-android" if target.startswith("aarch64") else "x86_64-linux-android"
        flags += [f"--target={triple}29", f"--sysroot={tools / 'sysroot'}", "-stdlib=libc++"]
        if target.startswith("aarch64"):
            flags += ["-DUSE_NEON", "-march=armv8-a"]
        else:
            flags += ["-DUSE_SSE2", "-msse2"]
        archiver = tools / "bin/llvm-ar"
        # Rust links these archives explicitly, avoiding an unpackaged libc++_shared.so.
        for name in ["libc++_static.a", "libc++abi.a"]:
            shutil.copy2(tools / "sysroot/usr/lib" / triple / name, work / name)
    else:
        if platform.system() != "Darwin":
            raise SystemExit("iOS 引擎构建需要 macOS 和 Xcode")
        sdk = "iphonesimulator" if target.endswith("-sim") else "iphoneos"
        compiler = output(["xcrun", "--sdk", sdk, "--find", "clang++"])
        apple_target = "arm64-apple-ios15.0-simulator" if target.endswith("-sim") else "arm64-apple-ios15.0"
        flags += ["-target", apple_target, "-isysroot", output(["xcrun", "--sdk", sdk, "--show-sdk-path"]), "-DUSE_NEON"]
        archiver = output(["xcrun", "--find", "ar"])
    src = source / "src"
    # Compile only engine translation units. Universal launcher code has its own main.
    files = sorted(p for p in src.rglob("*.cpp") if "universal" not in p.relative_to(src).parts and "temp_builds" not in p.relative_to(src).parts)
    files.extend(sorted(p for p in src.rglob("*.S") if "universal" not in p.relative_to(src).parts and "temp_builds" not in p.relative_to(src).parts))
    files.append(ROOT / "native/pikafish/pikafish_shim.cpp")
    if "apple" in target:
        files.append(ROOT / "native/pikafish/audio_session.mm")
    objects = []
    for i, file in enumerate(files):
        objects.append((file, work / f"{i:03d}-{file.stem}.o"))

    info = work / "BUILD_INFO.txt"
    digest = hashlib.sha256()
    digest.update(output([compiler, "--version"]).encode())
    digest.update(str(compiler).encode())
    digest.update(json.dumps(flags).encode())
    digest.update(commit.encode())
    for file in sorted(set(files) | set(src.rglob("*.h")) | {ROOT / "native/pikafish/pikafish_shim.h"}):
        digest.update(str(file).encode())
        digest.update(file.read_bytes())
    # SDK/NDK upgrades change the cache even when compiler flags remain identical.
    if "android" in target:
        digest.update((Path(ndk_value) / "source.properties").read_bytes())
    else:
        digest.update(output(["xcodebuild", "-version"]).encode())
    fingerprint = digest.hexdigest()
    flags_match = info.exists() and f"fingerprint={fingerprint}\n" in info.read_text(encoding="utf-8")

    def compile_one(pair):
        file, obj = pair
        if flags_match and obj.exists():
            return
        print(f"Compile {target}: {file.name}", flush=True)
        run([compiler, *flags, "-I", src, "-c", file, "-o", obj], cwd=src, log=False)

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        list(pool.map(compile_one, objects))
    archive = work / "libpikafish.a"
    # Rebuild an archive without keeping stale members from previous source versions.
    archive.unlink(missing_ok=True)
    run([archiver, "rcs", archive, *(obj for _, obj in objects)], log=False)
    (work / "BUILD_INFO.txt").write_text(f"fingerprint={fingerprint}\ncompiler={compiler}\ntag={tag}\ncommit={output(['git', '-C', source, 'rev-parse', 'HEAD'])}\ntarget={target}\nflags={' '.join(flags)}\n", encoding="utf-8")
    print(f"\nPIKAFISH_LIB_DIR={work}\n", flush=True)


if __name__ == "__main__":
    main()
