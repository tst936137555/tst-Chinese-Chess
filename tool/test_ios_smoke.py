#!/usr/bin/env python3
"""Check simulator orchestration and failure evidence without requiring macOS."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import ios_smoke


class SimulatorSmokeTest(unittest.TestCase):
    def exercise(self, fail, cold_fail=False):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            container = root / "container"
            output = container / "Documents/ui-smoke"
            output.mkdir(parents=True)
            (output / "startup.txt").write_text("audio-start")
            launches = []

            def sim(*args, **kwargs):
                if args[0] == "list":
                    return json.dumps({"devices": {"iOS": [{
                        "name": "iPhone", "isAvailable": True,
                        "state": "Booted", "udid": "test-device",
                    }]}})
                if args[0] == "get_app_container":
                    return str(container)
                if args[0] == "launch":
                    self.assertEqual(kwargs["timeout"], 180)
                    launches.append((args, kwargs["env"]))
                    (output / "startup.txt").write_text("audio-start")
                if args[0] == "io":
                    Path(args[-1]).write_bytes(b"screenshot")
                return ""

            def wait(read, predicate, timeout=20):
                if cold_fail and len(launches)==3:
                    raise AssertionError("cold launch timed out")
                return {"screen": "home", "controls": [1]}

            def smoke_result(path):
                if fail:
                    raise AssertionError("UI condition timed out")
                return {"scenes": 29, "realEngine": "passed"}

            with patch.object(ios_smoke, "sim", side_effect=sim), \
                 patch.object(ios_smoke, "wait", side_effect=wait), \
                 patch.object(ios_smoke, "wait_for_smoke", side_effect=smoke_result), \
                 patch.object(ios_smoke.subprocess, "run") as run, \
                 patch.object(ios_smoke.sys, "argv", ["ios_smoke.py", "test.app"]):
                previous = Path.cwd()
                try:
                    os.chdir(root)
                    if fail or cold_fail:
                        with self.assertRaisesRegex(AssertionError, "timed out"):
                            ios_smoke.main()
                    else:
                        ios_smoke.main()
                finally:
                    os.chdir(previous)
            dest = root / "target/ios-smoke"
            self.assertEqual((dest / "app/startup.txt").read_text(), "audio-start")
            self.assertTrue((dest / "simulator.log").exists())
            self.assertEqual(launches[0][1]["SIMCTL_CHILD_XIANGQI_SMOKE_OUTPUT"], str(output))
            self.assertTrue(any(call.args[0][2] == "terminate" for call in run.call_args_list))
            self.assertEqual((dest / "result.json").exists(), not (fail or cold_fail))

    def test_success(self):
        self.exercise(False)

    def test_failure_preserves_evidence(self):
        self.exercise(True)

    def test_cold_launch_failure_preserves_evidence(self):
        self.exercise(False, cold_fail=True)

    def test_stalled_app_fails_early(self):
        with tempfile.TemporaryDirectory() as tmp, \
             patch.object(ios_smoke.time, "monotonic", side_effect=range(100)), \
             patch.object(ios_smoke.time, "sleep"):
            with self.assertRaisesRegex(AssertionError, "stalled.*waiting for UI startup"):
                ios_smoke.wait_for_smoke(Path(tmp), timeout=300, stall_timeout=10)

    def test_failed_engine_is_not_a_success(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "result.json").write_text(json.dumps({"scenes": 29, "realEngine": "failed"}))
            with self.assertRaisesRegex(AssertionError, "unsuccessful result"):
                ios_smoke.wait_for_smoke(root)

    def test_slow_but_progressing_scenes_finish_after_five_minutes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);(root/"startup.txt").touch()
            clock=[0];scenes=[0]
            expected={"scenes":29,"realEngine":"passed"}
            def now():
                clock[0]+=15
                return clock[0]
            def read(path,*args,**kwargs):
                if path.name=="startup.txt":
                    scenes[0]+=1
                    return f"scene-{scenes[0]}/29"
                if scenes[0]>=10:return json.dumps(expected)
                raise FileNotFoundError(path)
            with patch.object(ios_smoke.time,"monotonic",side_effect=now), \
                 patch.object(ios_smoke.time,"sleep"), \
                 patch.object(Path,"read_text",read):
                self.assertEqual(ios_smoke.wait_for_smoke(root),expected)
            self.assertGreater(clock[0],300)

    def test_completed_smoke_result(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            expected = {"scenes": 29, "realEngine": "passed"}
            (root / "result.json").write_text(json.dumps(expected))
            self.assertEqual(ios_smoke.wait_for_smoke(root), expected)


if __name__ == "__main__":
    unittest.main()
