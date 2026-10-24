#!/usr/bin/env python3
"""Build a native APK with current Cargo/NDK and miniquad's matching Java bridge."""
import argparse
import hashlib
import re
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import zipfile
from build_info import version_info
from release_manifest import provenance
from android_bridge import activity_source, native_source
from android_signing import release_credentials, verify_release, certificate

ROOT = Path(__file__).resolve().parent.parent
PACKAGE = "com.tst.xiangqi.rust"


def run(args, **kw):
    print("+", " ".join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), check=True, cwd=ROOT, **kw)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", choices=["aarch64-linux-android", "x86_64-linux-android"])
    parser.add_argument("--skip-engine", action="store_true")
    parser.add_argument("--verify-only", action="store_true", help="Build an unsigned internal verification artifact outside dist; never publish")
    args = parser.parse_args()
    signing_env = dict(os.environ) if args.verify_only else release_credentials()
    release = version_info()
    sdk = Path(os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT") or "")
    ndk = Path(os.environ.get("ANDROID_NDK_HOME") or os.environ.get("ANDROID_NDK") or "")
    if not (sdk / "platforms/android-35/android.jar").is_file() or not (ndk / "toolchains/llvm").is_dir():
        raise SystemExit("请设置 ANDROID_HOME（含 SDK 35）及 ANDROID_NDK_HOME")
    host = {"Windows": "windows-x86_64", "Linux": "linux-x86_64", "Darwin": "darwin-x86_64"}[platform.system()]
    tools = ndk / "toolchains/llvm/prebuilt" / host
    win = platform.system() == "Windows"
    binary = lambda name: tools / "bin" / (name + (".exe" if win else ""))
    build_tools = sorted((sdk / "build-tools").iterdir(), key=lambda p: tuple(int(x) for x in p.name.split('.') if x.isdigit()))[-1]
    sdk_tool = lambda name: build_tools / (name + (".bat" if name in ["d8", "apksigner"] else ".exe") if win else name)
    target = args.target
    abi = "arm64-v8a" if target.startswith("aarch64") else "x86_64"
    work = ROOT / "target/android-artifacts" / target
    work.mkdir(parents=True, exist_ok=True)
    run(["rustup", "target", "add", target])
    if not args.skip_engine:
        run([sys.executable, ROOT / "tool/build_mobile_engine.py", target])
    env = signing_env
    env["PIKAFISH_LIB_DIR"] = str(ROOT / "mobile-engine" / target)
    key = target.upper().replace("-", "_")
    env[f"CARGO_TARGET_{key}_LINKER"] = str(binary("clang"))
    env[f"CARGO_TARGET_{key}_RUSTFLAGS"] = f"-C link-arg=--target={target}29 -C link-arg=--sysroot={tools / 'sysroot'} -C link-arg=-Wl,-z,max-page-size=16384"
    # The sysroot argument may contain spaces. Cargo's encoded flag form is unambiguous.
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(["-C", f"link-arg=--target={target}29", "-C", f"link-arg=--sysroot={tools / 'sysroot'}", "-C", "link-arg=-Wl,-z,max-page-size=16384"])
    run(["cargo", "rustc", "--lib", "--crate-type=cdylib", "--release", "--locked", "--target", target], env=env)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target], cwd=ROOT, env=env))
    mq = next(Path(p["manifest_path"]).parent for p in metadata["packages"] if p["name"] == "miniquad")
    sources = work / "java"
    classes = work / "classes"
    dex = work / "dex"
    for path in [sources, classes, dex]:
        if not path.resolve().is_relative_to(work.resolve()):
            raise SystemExit("generated directory escaped Android workspace")
        if path.exists():
            shutil.rmtree(path)
        path.mkdir(exist_ok=True)
    (sources / "MainActivity.java").write_text(activity_source((mq / "java/MainActivity.java").read_text(encoding="utf-8"), PACKAGE), encoding="utf-8")
    (sources / "QuadNative.java").write_text(native_source((mq / "java/QuadNative.java").read_text(encoding="utf-8")), encoding="utf-8")
    java_home = Path(os.environ["JAVA_HOME"]) if os.environ.get("JAVA_HOME") else None
    java_tool = lambda name: str(java_home / "bin" / (name + (".exe" if win else ""))) if java_home else name
    jar = sdk / "platforms/android-35/android.jar"
    run([java_tool("javac"), "-encoding", "UTF-8", "-source", "8", "-target", "8", "-classpath", jar, "-d", classes, sources / "MainActivity.java", sources / "QuadNative.java"])
    run([sdk_tool("d8"), "--min-api", "29", "--lib", jar, "--output", dex, *classes.rglob("*.class")])
    manifest = work / "AndroidManifest.xml"
    manifest.write_text(f'''<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="{PACKAGE}" android:versionCode="{release['build']}" android:versionName="{release['version']}">
<uses-sdk android:minSdkVersion="29" android:targetSdkVersion="35"/>
<uses-feature android:glEsVersion="0x00020000" android:required="true"/>
<application android:label="tst自用象棋" android:icon="@mipmap/ic_launcher" android:hasCode="true" android:debuggable="false" android:extractNativeLibs="true" android:theme="@android:style/Theme.Material.Light.NoActionBar.Fullscreen">
<activity android:name=".MainActivity" android:exported="true" android:configChanges="orientation|keyboardHidden|screenSize" android:screenOrientation="unspecified">
<intent-filter><action android:name="android.intent.action.MAIN"/><category android:name="android.intent.category.LAUNCHER"/></intent-filter>
</activity></application></manifest>''', encoding="utf-8")
    assets = work / "assets"
    if assets.exists():
        if not assets.resolve().is_relative_to(work.resolve()): raise SystemExit("invalid assets directory")
        shutil.rmtree(assets)
    for rel in ["engine/pikafish.nnue", "licenses/COPYING-pikafish.txt", "licenses/NNUE-License.md", "fonts/OFL.txt"]:
        dest = assets / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / "assets" / rel, dest)
    shutil.copy2(ROOT / "LICENSE", assets / "LICENSE")
    for name in ["LICENSE-MIT", "LICENSE-APACHE"]:
        shutil.copy2(ROOT / "vendor/miniquad" / name, assets / "licenses" / f"miniquad-{name}.txt")
    shutil.copy2(ROOT / "vendor/quad-snd/LICENSE", assets / "licenses/quad-snd-MIT.txt")
    unaligned = work / "unaligned.apk"
    resources = work / "res"
    if resources.exists():
        if not resources.resolve().is_relative_to(work.resolve()): raise SystemExit("invalid resources directory")
        shutil.rmtree(resources)
    for source in (ROOT / "assets/icons/android").iterdir():
        shutil.copytree(source, resources / source.name, dirs_exist_ok=True)
    run([sdk_tool("aapt"), "package", "-f", "-M", manifest, "-I", jar, "-A", assets, "-S", resources, "-F", unaligned])
    with zipfile.ZipFile(unaligned, "a", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.write(dex / "classes.dex", "classes.dex")
        archive.write(ROOT / "target" / target / "release/libtst_xiangqi.so", f"lib/{abi}/libtst_xiangqi.so")
    aligned = work / f"tst-xiangqi-{release['version']}-{abi}.apk"
    run([sdk_tool("zipalign"), "-f", "4", unaligned, aligned])
    evidence = {"buildType":"verification", "signed":False}
    if not args.verify_only:
        run([sdk_tool("apksigner"), "sign", "--ks", env["ANDROID_KEYSTORE"], "--ks-key-alias", env["ANDROID_KEY_ALIAS"], "--ks-pass", "env:ANDROID_KEYSTORE_PASSWORD", "--key-pass", "env:ANDROID_KEY_PASSWORD", aligned], env=env)
        evidence = verify_release(aligned, sdk_tool("apksigner"), sdk_tool("aapt"), release["version"], release["build"])
    dist = ROOT / ("target/android-verification" if args.verify_only else "dist")
    dist.mkdir(exist_ok=True)
    final = dist / aligned.name
    shutil.copy2(aligned, final)
    cert=evidence.get("certificateSha256")
    def old_certificate(path):
        try:return certificate(path, sdk_tool("apksigner"))
        except ValueError:return None
    previous={old.name:old_certificate(old) for old in dist.glob(f"*-{abi}.apk") if old!=final}
    compatible=all(value==cert for value in previous.values()) if previous and cert else None
    if compatible is False:
        print("已有旧包的签名与项目正式证书不同；已保留旧包，请勿卸载含用户数据的应用。",flush=True)
    pinned=json.loads((ROOT/"tool/engine_manifest.json").read_text(encoding="utf-8"))["pikafish"]
    with final.open("rb") as stream:digest=hashlib.file_digest(stream,"sha256").hexdigest()
    build_record={**provenance(target=target,flags=env["CARGO_ENCODED_RUSTFLAGS"],build_command=["cargo","rustc","--lib","--crate-type=cdylib","--release","--locked","--target",target]),"effectiveLinker":env[f"CARGO_TARGET_{key}_LINKER"],"sha256":digest,"certificateSha256":cert,"previousCertificates":previous,"compatibleWithPrevious":compatible,"engineCommit":pinned["commit"],"nnueSha256":pinned["artifacts"]["nnue"]["sha256"]}
    build_record.update(evidence)
    final.with_suffix(".build.json").write_text(json.dumps(build_record,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(f"APK: {final}")


if __name__ == "__main__":
    main()
