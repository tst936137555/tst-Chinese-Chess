#!/usr/bin/env python3
"""Single release version source shared by native packaging scripts."""
from pathlib import Path
import json
import sys
import tomllib
ROOT = Path(__file__).resolve().parent.parent

def version_info():
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8-sig"))["package"]["version"]
    major, minor, patch = map(int, version.split("."))
    if not (0 <= minor < 100 and 0 <= patch < 100):
        raise ValueError("minor and patch must be below 100 for native build numbers")
    return {"version": version, "build": major * 10000 + minor * 100 + patch}

if __name__ == "__main__":
    info = version_info()
    print(info[sys.argv[1]] if len(sys.argv) > 1 else json.dumps(info))
