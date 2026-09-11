#!/usr/bin/env python3
# =============================================================================
# subset_font.py — 霞鹜文楷字体子集化：为应用生成小体积字体资产
#
# 背景：完整 LXGW WenKai Medium 约 24MB（收录 3 万+ 字形），应用实际只用到
# 常用汉字。子集化后字体降至 MB 级，作为入库资产 assets/fonts/
# XqKai-Medium-subset.ttf 直接打包，APK / 各平台安装包体积显著缩小；
# CI 与 fetch_engine.ps1 不再需要下载字体。
#
# 字符集构成（宁多勿缺：缺字会回退系统字体，且可能破坏 Golden 基线）：
#   1. ASCII 可打印字符（0x20-0x7E）
#   2. 源码扫描：lib/ 与 test/ 全部 .dart 中的非 ASCII 字符（覆盖全部 UI
#      文案与记谱字符；文案改动后重跑本脚本即可）
#   3. GB2312 一级常用字（约 3755 字）：为未来新增文案留出的安全余量
#   4. 补充集：全角标点/数字、繁体棋子字等零散字符
#
# OFL-1.1 合规：子集属「修改版」，保留字体名（「霞鹜」「落霞孤鹜」「LXGW」）
# 不得用于标识修改版，故输出字体的内部家族名改写为 XqKai（pubspec 声明与
# 代码引用本就使用 XqKai，行为不变）；来源与许可证见 README 与 OFL.txt。
#
# 用法（需 Python 3.9+ 与 fonttools，pip install fonttools）：
#   python tool/subset_font.py --input assets/fonts/LXGWWenKai-Medium.ttf
# 输出：assets/fonts/XqKai-Medium-subset.ttf（缺省与输入同目录）
# =============================================================================

import argparse
import sys
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

REPO_ROOT = Path(__file__).resolve().parent.parent
SCAN_DIRS = ("lib", "test")
DEFAULT_OUTPUT = "XqKai-Medium-subset.ttf"

# 内部名改写目标（nameID → 值）。全部避开 OFL 保留字体名；
# nameID 6 PostScript 名必须为 ASCII。
NAME_OVERRIDES = {
    1: "XqKai",                # Family
    3: "XqKai-Medium-subset",  # Unique ID
    4: "XqKai Medium",         # Full name
    6: "XqKai-Medium",         # PostScript name
    16: "XqKai",               # Typographic Family
}

# 补充字符：全角标点/数字、繁体棋子与棋盘用字、零散符号
# （繁体字不在 GB2312 内，须显式列出；简化棋子字已在 GB2312-1 内）
EXTRA_CHARS = (
    "，。、；：？！「」『』（）《》〈〉【】〔〕·—…％￥＋－×÷〇"
    "０１２３４５６７８９"
    "帥將士象馬車炮兵卒仕相進退平前中後楚河漢界紅黑"
)


def gb2312_level1() -> set[str]:
    """GB2312 一级常用字（区码 0xB0-0xD7），为未来文案提供安全余量。"""
    chars: set[str] = set()
    for hi in range(0xB0, 0xD8):
        for lo in range(0xA1, 0xFF):
            try:
                chars.add(bytes([hi, lo]).decode("gb2312"))
            except UnicodeDecodeError:
                pass  # 少量空位
    return chars


def scan_source_chars() -> set[str]:
    """扫描 lib/ 与 test/ 全部 Dart 源码中的非 ASCII 字符。"""
    chars: set[str] = set()
    for d in SCAN_DIRS:
        for path in sorted((REPO_ROOT / d).rglob("*.dart")):
            text = path.read_text(encoding="utf-8", errors="ignore")
            chars.update(ch for ch in text if ord(ch) > 0x7E and ch != "\ufeff")
    return chars


def rename_family(path: Path) -> None:
    """改写输出字体内部名，避免 OFL 保留字体名标识修改版。"""
    font = TTFont(str(path))
    name_table = font["name"]
    kept = [r for r in name_table.names if r.nameID not in NAME_OVERRIDES]
    name_table.names = kept
    for name_id, value in NAME_OVERRIDES.items():
        # Windows（Unicode BMP, en-US）+ Mac（Roman, English）双平台记录
        name_table.setName(value, name_id, 3, 1, 0x409)
        name_table.setName(value, name_id, 1, 0, 0)
    font.save(str(path))


def main() -> int:
    parser = argparse.ArgumentParser(description="霞鹜文楷字体子集化")
    parser.add_argument(
        "--input",
        type=Path,
        required=True,
        help="完整字体路径（LXGWWenKai-Medium.ttf）",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help=f"输出路径（缺省：输入同目录/{DEFAULT_OUTPUT}）",
    )
    args = parser.parse_args()
    input_path: Path = args.input
    if not input_path.is_file():
        print(f"错误：输入字体不存在：{input_path}", file=sys.stderr)
        return 1
    output_path = args.output or input_path.with_name(DEFAULT_OUTPUT)

    ui_chars = scan_source_chars()
    gb_chars = gb2312_level1()
    extra_chars = set(EXTRA_CHARS)
    ascii_chars = {chr(c) for c in range(0x20, 0x7F)}
    charset = ui_chars | gb_chars | extra_chars | ascii_chars
    print(
        f"字符集：源码 {len(ui_chars)} + GB2312-1 {len(gb_chars)} + "
        f"补充 {len(extra_chars)} + ASCII {len(ascii_chars)}"
        f"（去重后 {len(charset)}）"
    )

    options = subset.Options()
    options.layout_features = ["*"]  # 保留全部 OpenType 特性（kern 等）
    options.name_IDs = ["*"]  # 保留 name 表，落盘前改写保留名
    options.notdef_outline = True

    font = subset.load_font(str(input_path), options)
    subsetter = subset.Subsetter(options)
    subsetter.populate(text="".join(sorted(charset)))
    subsetter.subset(font)
    subset.save_font(font, str(output_path), options)
    rename_family(output_path)

    # 覆盖校验：源码用到的每个字符必须存在于输出字体（缺字会回退系统字体，
    # 且 Golden 基线以该字体渲染，缺字直接导致基线比对失败）
    cmap = TTFont(str(output_path)).getBestCmap()
    missing_ui = sorted(ch for ch in ui_chars if ord(ch) not in cmap)
    missing_gb = sorted(ch for ch in gb_chars if ord(ch) not in cmap)
    if missing_ui:
        print(
            f"错误：源码字符未覆盖（{len(missing_ui)} 个）："
            f"{''.join(missing_ui[:50])}",
            file=sys.stderr,
        )
        return 1
    if missing_gb:
        print(f"警告：GB2312-1 缺字（{len(missing_gb)} 个）：{''.join(missing_gb)}")

    in_size = input_path.stat().st_size
    out_size = output_path.stat().st_size
    print(f"输出：{output_path}")
    print(
        f"体积：{in_size / 1048576:.1f}MB → {out_size / 1048576:.1f}MB"
        f"（{out_size * 100 // in_size}%）"
    )
    print("覆盖校验通过（源码字符全覆盖）。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
