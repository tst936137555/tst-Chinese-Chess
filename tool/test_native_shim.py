#!/usr/bin/env python3
"""Exercise restart/EOF behavior of the actual mobile C ABI without an NNUE file."""
from pathlib import Path
import os
import shutil
import subprocess

root = Path(__file__).resolve().parent.parent
compiler = shutil.which("g++") or shutil.which("c++")
if not compiler:
    raise SystemExit("C++ compiler is required for the native adapter regression test")
binary = root / "target" / ("shim-restart-test.exe" if os.name == "nt" else "shim-restart-test")
binary.parent.mkdir(exist_ok=True)
subprocess.run([compiler, "-std=c++17", "-pthread", str(root / "tests/native/shim_restart.cpp"), str(root / "native/pikafish/pikafish_shim.cpp"), "-o", str(binary)], check=True)
subprocess.run([str(binary)], check=True, timeout=15)
