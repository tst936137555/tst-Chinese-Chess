#!/usr/bin/env python3
"""Test Android diagnostic handling without a connected emulator."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import mobile_smoke


class AndroidSmokeTest(unittest.TestCase):
    def test_adb_has_a_bounded_default_and_install_override(self):
        with patch.object(mobile_smoke.subprocess, "run") as run:
            mobile_smoke.adb("shell", "getprop")
            self.assertEqual(run.call_args.kwargs["timeout"], 15)
            mobile_smoke.adb("install", "-r", "test.apk", timeout=120)
            self.assertEqual(run.call_args.kwargs["timeout"], 120)

    def test_failed_diagnostic_command_does_not_skip_remaining_logs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            result = subprocess.CompletedProcess([], 0, b"startup", b"")
            with patch.object(mobile_smoke, "ROOT", root), \
                 patch.object(mobile_smoke, "adb", side_effect=[
                     subprocess.TimeoutExpired("adb", 15),
                     subprocess.CompletedProcess([], 1, b"", b"device offline"),
                     result,
                 ]):
                mobile_smoke.collect_diagnostics()
            output = root / "target/android-smoke"
            self.assertIn("timed out", (output / "logcat.txt").read_text())
            self.assertEqual((output / "activity.txt").read_bytes(), b"device offline")
            self.assertEqual((output / "startup.txt").read_bytes(), b"startup")


if __name__ == "__main__":
    unittest.main()
