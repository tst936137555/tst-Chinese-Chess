#!/usr/bin/env bash
# Unsigned .app for device or Apple Silicon simulator. Signing is explicit.
set -euo pipefail
cd "$(dirname "$0")/.."
version="$(python3 tool/build_info.py version)"
build_number="$(python3 tool/build_info.py build)"
target="${1:-aarch64-apple-ios-sim}"
case "$target" in
  aarch64-apple-ios-sim) supported=iPhoneSimulator; sdk=iphonesimulator ;;
  aarch64-apple-ios) supported=iPhoneOS; sdk=iphoneos ;;
  *) echo "Unsupported target: $target" >&2; exit 1 ;;
esac
rustup target add "$target"
python3 tool/build_mobile_engine.py "$target"
export PIKAFISH_LIB_DIR="$PWD/mobile-engine/$target"
export IPHONEOS_DEPLOYMENT_TARGET=15.0
export XIANGQI_BUILD_COMMAND="cargo build --locked --release --target $target"
cargo build --locked --release --target "$target"
app="$PWD/dist/ios/$target/tst-xiangqi.app"
python3 - "$app" "$PWD/dist/ios" <<'PY'
import pathlib, shutil, sys
app = pathlib.Path(sys.argv[1]).resolve()
root = pathlib.Path(sys.argv[2]).resolve()
if not app.is_relative_to(root) or app.suffix != '.app':
    raise SystemExit('invalid generated iOS app directory')
if app.exists(): shutil.rmtree(app)
PY
mkdir -p "$app"
cp "target/$target/release/tst-xiangqi" "$app/tst-xiangqi"
for resource in engine/pikafish.nnue licenses/COPYING-pikafish.txt licenses/NNUE-License.md fonts/OFL.txt; do
  mkdir -p "$app/$(dirname "$resource")"
  cp "assets/$resource" "$app/$resource"
done
cp LICENSE "$app/LICENSE"
cp vendor/quad-snd/LICENSE "$app/licenses/quad-snd-MIT.txt"
cp vendor/miniquad/LICENSE-MIT "$app/licenses/miniquad-MIT.txt"
cp vendor/miniquad/LICENSE-APACHE "$app/licenses/miniquad-Apache-2.0.txt"
cat > "$app/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>tst-xiangqi</string>
<key>CFBundleIdentifier</key><string>com.tst.xiangqi.rust</string>
<key>CFBundleName</key><string>tst自用象棋</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>$build_number</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>MinimumOSVersion</key><string>15.0</string>
<key>CFBundleSupportedPlatforms</key><array><string>$supported</string></array>
<key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
<key>UILaunchScreen</key><dict/>
<key>UISupportedInterfaceOrientations</key><array><string>UIInterfaceOrientationPortrait</string><string>UIInterfaceOrientationLandscapeLeft</string><string>UIInterfaceOrientationLandscapeRight</string></array>
</dict></plist>
EOF
xcrun actool assets/icons/ios/Assets.xcassets --compile "$app" --platform "$sdk" \
  --minimum-deployment-target 15.0 --app-icon AppIcon --target-device iphone --target-device ipad \
  --output-partial-info-plist "$app/icon-info.plist"
python3 - "$app" <<'PY'
import pathlib, plistlib, sys
app = pathlib.Path(sys.argv[1])
with (app / 'Info.plist').open('rb') as f:
    data = plistlib.load(f)
with (app / 'icon-info.plist').open('rb') as f:
    data.update(plistlib.load(f))
with (app / 'Info.plist').open('wb') as f:
    plistlib.dump(data, f)
PY
if [[ "$target" == *-sim ]]; then
  codesign --force --sign - "$app"
else
  echo "真机产物未签名。使用自己的描述文件与签名身份签名后安装。"
fi
python3 tool/release_manifest.py "$(dirname "$app")" --target "$target"
echo "$app"
