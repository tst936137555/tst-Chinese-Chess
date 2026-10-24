import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from android_signing import release_credentials, signing_identity, verify_release

class AndroidSigningTests(unittest.TestCase):
    def test_missing_credentials_fail_instead_of_creating_a_debug_key(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError,"签名配置"):
                release_credentials(environment={},home=folder)
            self.assertEqual(list(Path(folder).iterdir()),[])

    def test_private_properties_and_environment_override(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);config=root/"key.properties";key=root/"release.jks";key.write_bytes(b"fixture")
            config.write_text("storeFile=release.jks\nkeyAlias=fixture\nstorePassword=fixture-password\nkeyPassword=fixture-key-password\n")
            env=release_credentials({"ANDROID_SIGNING_PROPERTIES":str(config),"ANDROID_KEY_ALIAS":"override"},home=root)
            self.assertEqual(env["ANDROID_KEYSTORE"],str(key.resolve()))
            self.assertEqual(env["ANDROID_KEY_ALIAS"],"override")

    def test_apk_certificate_and_manifest_are_actually_checked(self):
        identity=signing_identity()
        signed=subprocess.CompletedProcess([],0,f"Signer #1 certificate SHA-256 digest: {identity['certificateSha256']}\n","")
        manifest=f"package: name='{identity['package']}' versionCode='20003' versionName='2.0.3'"
        with patch("android_signing.subprocess.run",side_effect=[signed,subprocess.CompletedProcess([],0,manifest,"")]):
            self.assertEqual(verify_release(Path("fixture.apk"),"apksigner","aapt","2.0.3",20003)["debuggable"],False)
        for output in [manifest+"\napplication-debuggable",manifest.replace("20003","20002"),manifest.replace(identity["package"],"other.app")]:
            with patch("android_signing.subprocess.run",side_effect=[signed,subprocess.CompletedProcess([],0,output,"")]):
                with self.assertRaisesRegex(ValueError,"验证失败"):verify_release(Path("fixture.apk"),"apksigner","aapt","2.0.3",20003)
        with patch("android_signing.subprocess.run",return_value=subprocess.CompletedProcess([],0,"certificate SHA-256 digest: "+"0"*64,"")):
            with self.assertRaisesRegex(ValueError,"正式签名"):verify_release(Path("fixture.apk"),"apksigner","aapt","2.0.3",20003)

if __name__=="__main__":unittest.main()
