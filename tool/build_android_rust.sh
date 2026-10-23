#!/usr/bin/env bash
# One ABI per invocation: each static engine must match the Rust target.
set -euo pipefail
cd "$(dirname "$0")/.."
target="${1:-aarch64-linux-android}"
case "$target" in aarch64-linux-android|x86_64-linux-android) ;; *) echo "Unsupported target" >&2; exit 1 ;; esac
python3 tool/build_android_rust.py "$target"
