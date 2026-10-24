"""Compile the actual generated IME/accessibility bridge against the installed Android SDK."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest
from android_bridge import activity_source, native_source
from android_input import with_name_input
from android_accessibility import with_accessibility

ROOT = Path(__file__).resolve().parent.parent

class AndroidBridgeTest(unittest.TestCase):
    def test_template_changes_are_reported(self):
        for rewrite in (with_name_input, with_accessibility):
            with self.assertRaises(ValueError):
                rewrite("changed upstream bridge")

    def test_startup_hooks_precede_native_launch(self):
        source = activity_source((ROOT/"vendor/miniquad/java/MainActivity.java").read_text(encoding="utf-8"))
        for hook in ("QuadNative.xiangqiInit();", "QuadNative.xiangqiSmoke(", "QuadNative.xiangqiObserve("):
            self.assertLess(source.index(hook), source.index("QuadNative.activityOnCreate(this);"))
        self.assertIn("registerOnBackInvokedCallback", source)
        self.assertNotIn("super.onBackPressed();", source)
        native = native_source((ROOT/"vendor/miniquad/java/QuadNative.java").read_text(encoding="utf-8"))
        for method in ("xiangqiInit", "xiangqiSmoke", "xiangqiObserve", "xiangqiPaused", "xiangqiNameEdit", "xiangqiAccessibilityActivate"):
            self.assertIn("void " + method + "(", native)

    def test_generated_java_compiles(self):
        sdk = Path(os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT") or Path.home()/"AppData/Local/Android/Sdk")
        jar = sdk / "platforms/android-35/android.jar"
        java_home = os.environ.get("JAVA_HOME")
        javac = str(Path(java_home)/"bin"/("javac.exe" if os.name == "nt" else "javac")) if java_home else shutil.which("javac")
        if not jar.is_file() or not javac:
            self.skipTest("Android SDK 35 and JDK required for Java compilation")
        source = activity_source((ROOT/"vendor/miniquad/java/MainActivity.java").read_text(encoding="utf-8"))
        native = native_source((ROOT/"vendor/miniquad/java/QuadNative.java").read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            (folder/"MainActivity.java").write_text(source,encoding="utf-8")
            (folder/"QuadNative.java").write_text(native,encoding="utf-8")
            result = subprocess.run([javac,"-encoding","UTF-8","-source","8","-target","8","-classpath",str(jar),"-d",str(folder),str(folder/"MainActivity.java"),str(folder/"QuadNative.java")],capture_output=True,text=True,errors="replace",timeout=60)
            self.assertEqual(result.returncode,0,result.stdout+result.stderr)

if __name__ == "__main__":
    unittest.main()
