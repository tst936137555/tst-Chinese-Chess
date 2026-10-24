import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from desktop_smoke import wait_for_result


class DesktopSmokeTests(unittest.TestCase):
    def test_result_requires_all_scenes_and_known_engine_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            process = Mock()
            for value in ({"scenes": 28}, {"scenes": 29, "realEngine": "failed"}):
                (root / "result.json").write_text(json.dumps(value))
                with self.assertRaisesRegex(AssertionError, "Invalid smoke result"):
                    wait_for_result(root, process)
            value = {"scenes": 29, "realEngine": "passed"}
            (root / "result.json").write_text(json.dumps(value))
            self.assertEqual(wait_for_result(root, process), value)

    def test_crash_reports_last_startup_stage(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "startup.txt").write_text("font-load")
            with self.assertRaisesRegex(AssertionError, "exited.*font-load"):
                wait_for_result(root, Mock(poll=Mock(return_value=1), returncode=1))

    def test_stalled_process_fails_before_total_timeout(self):
        with tempfile.TemporaryDirectory() as tmp, \
             patch("desktop_smoke.time.monotonic", side_effect=range(100)), \
             patch("desktop_smoke.time.sleep"):
            with self.assertRaisesRegex(AssertionError, "stalled.*native startup"):
                wait_for_result(Path(tmp), Mock(poll=Mock(return_value=None)), stall_timeout=10)


if __name__ == "__main__":
    unittest.main()
