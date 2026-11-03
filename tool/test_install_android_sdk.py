from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from install_android_sdk import install_packages


class AndroidSdkInstallTests(unittest.TestCase):
    def test_success_installs_exact_packages_without_retrying_or_clearing_staging(self):
        with tempfile.TemporaryDirectory(prefix="sdk install ") as folder:
            root = Path(folder).resolve()
            staging = root / ".temp"
            staging.mkdir()
            with patch("install_android_sdk.subprocess.run", return_value=subprocess.CompletedProcess([], 0)) as run, \
                 patch("install_android_sdk.time.sleep") as sleep:
                packages = ["platforms;android-35", "build-tools;35.0.0", "ndk;28.2.13676358"]
                self.assertEqual(install_packages(root, packages), 0)
                run.assert_called_once_with(["sdkmanager", f"--sdk_root={root}", *packages], check=False)
                sleep.assert_not_called()
            self.assertTrue(staging.is_dir())

    def test_bad_archive_is_removed_before_retry_and_installed_packages_survive(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            staging = root / ".temp/PackageOperation01"
            installed = root / "ndk/already-installed/source.properties"
            installed.parent.mkdir(parents=True)
            installed.write_text("installed")
            licenses = root / "licenses/android-sdk-license"
            licenses.parent.mkdir()
            licenses.write_text("accepted")

            def failed_download(*args, **kwargs):
                staging.mkdir(parents=True)
                (staging / "android-ndk-r28c-linux.zip").write_bytes(b"truncated ZIP")
                return subprocess.CompletedProcess(args, 1)

            def second_download(*args, **kwargs):
                self.assertFalse(staging.exists())
                self.assertEqual(installed.read_text(), "installed")
                self.assertEqual(licenses.read_text(), "accepted")
                return subprocess.CompletedProcess(args, 0)

            downloads = iter([failed_download, second_download])
            with patch("install_android_sdk.subprocess.run", side_effect=lambda *a, **kw: next(downloads)(*a, **kw)) as run, \
                 patch("install_android_sdk.time.sleep") as sleep:
                self.assertEqual(install_packages(root, ["ndk;28.2.13676358"]), 0)
                self.assertEqual(run.call_count, 2)
                sleep.assert_called_once_with(10)

    def test_persistent_failure_stays_failed_after_bounded_backoff(self):
        with tempfile.TemporaryDirectory() as folder, \
             patch("install_android_sdk.subprocess.run", return_value=subprocess.CompletedProcess([], 7)) as run, \
             patch("install_android_sdk.time.sleep") as sleep:
            self.assertEqual(install_packages(folder, ["ndk;28.2.13676358"]), 7)
            self.assertEqual(run.call_count, 3)
            self.assertEqual([call.args[0] for call in sleep.call_args_list], [10, 20])

    def test_redirected_staging_cannot_delete_another_directory(self):
        with tempfile.TemporaryDirectory() as folder, tempfile.TemporaryDirectory() as outside:
            root = Path(folder).resolve()
            valuable = Path(outside) / "keep.txt"
            valuable.write_text("keep")
            try:
                (root / ".temp").symlink_to(outside, target_is_directory=True)
            except OSError:
                self.skipTest("directory symlinks are unavailable")
            with patch("install_android_sdk.subprocess.run", return_value=subprocess.CompletedProcess([], 1)), \
                 patch("install_android_sdk.time.sleep") as sleep:
                with self.assertRaisesRegex(ValueError, "staging directory"):
                    install_packages(root, ["ndk;28.2.13676358"])
                sleep.assert_not_called()
            self.assertEqual(valuable.read_text(), "keep")


if __name__ == "__main__":
    unittest.main()
