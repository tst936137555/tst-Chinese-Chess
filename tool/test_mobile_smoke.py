#!/usr/bin/env python3
"""Test Android diagnostic handling without a connected emulator."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import mobile_smoke


class AndroidSmokeTest(unittest.TestCase):
    def test_process_can_be_spawned_after_activity_launch_returns(self):
        completed=lambda code,data:subprocess.CompletedProcess([],code,data,b"")
        with patch.object(mobile_smoke,"adb",side_effect=[
            completed(1,b""),completed(1,b""),completed(1,b""),
            completed(0,b"font-load"),completed(0,b'{"scenes":29,"realEngine":"passed"}'),
        ]),patch.object(mobile_smoke.time,"sleep"):
            self.assertEqual(mobile_smoke.wait_for_smoke()["scenes"],29)

    def test_stalled_startup_reports_native_stage(self):
        def command(*args, **kwargs):
            value = b"native-start" if args[-1].endswith("/smoke/startup.txt") else b"123"
            code = 1 if args[-1].endswith("/smoke/result.json") else 0
            return subprocess.CompletedProcess([], code, value, b"")
        with patch.object(mobile_smoke, "adb", side_effect=command), \
             patch.object(mobile_smoke.time, "monotonic", side_effect=range(100)), \
             patch.object(mobile_smoke.time, "sleep"):
            with self.assertRaisesRegex(AssertionError, "stalled.*native-start"):
                mobile_smoke.wait_for_smoke(stall_timeout=10)

    def test_crash_fails_immediately(self):
        with patch.object(mobile_smoke, "adb", side_effect=[
            subprocess.CompletedProcess([], 0, b"font-load", b""),
            subprocess.CompletedProcess([], 1, b"", b"missing"),
            subprocess.CompletedProcess([], 1, b"", b""),
        ]):
            with self.assertRaisesRegex(AssertionError, "exited.*font-load"):
                mobile_smoke.wait_for_smoke()

    def test_mobile_result_requires_real_engine(self):
        with patch.object(mobile_smoke, "adb", side_effect=[
            subprocess.CompletedProcess([], 0, b"scene-28/29", b""),
            subprocess.CompletedProcess([], 0, b'{"scenes":29,"realEngine":"assets unavailable"}', b""),
        ]):
            with self.assertRaisesRegex(AssertionError, "unsuccessful result"):
                mobile_smoke.wait_for_smoke()

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
                     result,
                     result,
                     result,
                     result,
                 ]):
                mobile_smoke.collect_diagnostics()
            output = root / "target/android-smoke"
            self.assertIn("timed out", (output / "logcat.txt").read_text())
            self.assertEqual((output / "activity.txt").read_bytes(), b"device offline")
            self.assertEqual((output / "window.txt").read_bytes(), b"startup")
            self.assertEqual((output / "input.txt").read_bytes(), b"startup")
            self.assertEqual((output / "startup.txt").read_bytes(), b"startup")
            self.assertEqual((output / "memory.txt").read_bytes(), b"startup")
            self.assertEqual((output / "interaction.txt").read_bytes(), b"startup")


if __name__ == "__main__":
    unittest.main()
