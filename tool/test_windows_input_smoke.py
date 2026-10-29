import os
from pathlib import Path
import subprocess
import sys
import unittest


class WindowsInputSmokeTests(unittest.TestCase):
    def test_redirected_logs_preserve_chinese_on_legacy_windows_encoding(self):
        # Reproduce the hosted Windows runner's pipe encoding, even on Linux.
        # Stop at the first native API so this regression needs no desktop.
        result = subprocess.run(
            [sys.executable, "-c", """
import sys
from unittest.mock import patch
import windows_input_smoke

assert sys.stdout.encoding == sys.stderr.encoding == "cp1252"

def native_entry(*args, **kwargs):
    print("UIA invoke:", "开始新对局", flush=True)
    print("诊断：中文残局", file=sys.stderr, flush=True)
    raise SystemExit(0)

with patch.object(sys, "platform", "win32"), \
     patch.object(windows_input_smoke.ctypes, "WinDLL", native_entry, create=True):
    windows_input_smoke.main()
"""],
            cwd=Path(__file__).resolve().parent,
            env={**os.environ, "PYTHONIOENCODING": "cp1252", "PYTHONUTF8": "0"},
            capture_output=True,
            timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode("ascii", errors="replace"))
        self.assertEqual(result.stdout.decode("utf-8").strip(), "UIA invoke: 开始新对局")
        self.assertEqual(result.stderr.decode("utf-8").strip(), "诊断：中文残局")


if __name__ == "__main__":
    unittest.main()
