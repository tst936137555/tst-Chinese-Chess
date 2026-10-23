#!/usr/bin/env python3
"""Resize the approved artwork into native icon formats; requires Pillow."""
from pathlib import Path
import json
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent

def main():
    root = ROOT / "assets/icons"
    source = Image.open(root / "app-source.png").convert("RGB")
    if source.width != source.height:
        raise SystemExit("App icon artwork must be square")
    def png(path, size):
        path.parent.mkdir(parents=True, exist_ok=True)
        source.resize((size,size), Image.Resampling.LANCZOS).save(path)
    png(root / "app.png", 1024)
    for size in [16,32,64]: png(root / f"desktop/icon-{size}.png", size)
    image = source.resize((1024,1024), Image.Resampling.LANCZOS)
    image.save(root / "app.ico", sizes=[(n,n) for n in [16,24,32,48,64,128,256]])
    image.save(root / "app.icns")
    for density, size in [("mdpi",48),("hdpi",72),("xhdpi",96),("xxhdpi",144),("xxxhdpi",192)]:
        png(root / f"android/mipmap-{density}/ic_launcher.png", size)
    png(root / "android/drawable-nodpi/ic_launcher_art.png", 1024)
    values = root / "android/values"; values.mkdir(exist_ok=True)
    (values / "icon_colors.xml").write_text('<resources><color name="ic_launcher_background">#F9F0DB</color></resources>\n',encoding="utf-8")
    drawable = root / "android/drawable"; drawable.mkdir(exist_ok=True)
    (drawable / "ic_launcher_foreground.xml").write_text('<inset xmlns:android="http://schemas.android.com/apk/res/android" android:inset="14%"><bitmap android:src="@drawable/ic_launcher_art" android:gravity="fill"/></inset>\n',encoding="utf-8")
    adaptive = root / "android/mipmap-anydpi-v26"; adaptive.mkdir(exist_ok=True)
    (adaptive / "ic_launcher.xml").write_text('<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android"><background android:drawable="@color/ic_launcher_background"/><foreground android:drawable="@drawable/ic_launcher_foreground"/></adaptive-icon>\n',encoding="utf-8")
    ios = root / "ios/Assets.xcassets/AppIcon.appiconset"
    data = json.loads((ios / "Contents.json").read_text(encoding="utf-8"))
    for item in data["images"]:
        if "filename" in item:
            size = round(float(item["size"].split("x")[0]) * float(item["scale"].rstrip("x")))
            png(ios / item["filename"], size)
    print("Desktop ICO/ICNS, Android legacy/adaptive and iOS icons generated")

if __name__ == "__main__": main()
