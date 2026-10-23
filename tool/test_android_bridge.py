"""Compile the actual generated IME/accessibility bridge against the installed Android SDK."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest
from android_input import with_name_input
from android_accessibility import with_accessibility

ROOT = Path(__file__).resolve().parent.parent

class AndroidBridgeTest(unittest.TestCase):
    def test_template_changes_are_reported(self):
        for rewrite in (with_name_input, with_accessibility):
            with self.assertRaises(ValueError):
                rewrite("changed upstream bridge")

    def test_generated_java_compiles(self):
        sdk = Path(os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT") or Path.home()/"AppData/Local/Android/Sdk")
        jar = sdk / "platforms/android-35/android.jar"
        java_home = os.environ.get("JAVA_HOME")
        javac = str(Path(java_home)/"bin"/("javac.exe" if os.name == "nt" else "javac")) if java_home else shutil.which("javac")
        if not jar.is_file() or not javac:
            self.skipTest("Android SDK 35 and JDK required for Java compilation")
        source = (ROOT/"vendor/miniquad/java/MainActivity.java").read_text(encoding="utf-8")
        source = source.replace("TARGET_PACKAGE_NAME", "com.tst.xiangqi.rust").replace("LIBRARY_NAME", "tst_xiangqi")
        source = with_accessibility(with_name_input(source))
        native = (ROOT/"vendor/miniquad/java/QuadNative.java").read_text(encoding="utf-8").replace("public class QuadNative {", """public class QuadNative {
            public native static void xiangqiAccessibilityActivate(int id);
            public native static void xiangqiNameEdit(String text,int start,int end);
        """)
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            (folder/"MainActivity.java").write_text(source,encoding="utf-8")
            (folder/"QuadNative.java").write_text(native,encoding="utf-8")
            result = subprocess.run([javac,"-encoding","UTF-8","-source","8","-target","8","-classpath",str(jar),"-d",str(folder),str(folder/"MainActivity.java"),str(folder/"QuadNative.java")],capture_output=True,text=True,timeout=60)
            self.assertEqual(result.returncode,0,result.stdout+result.stderr)

if __name__ == "__main__":
    unittest.main()
