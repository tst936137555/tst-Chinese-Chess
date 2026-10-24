import unittest
import hashlib
import json
from pathlib import Path
import tempfile
from release_notes import notes, release_assets
from android_signing import signing_identity

def official_inventory(root):
    names=["tst-xiangqi-2.0.2-windows.zip","tst-xiangqi-2.0.2-arm64-v8a.apk","tst-xiangqi-2.0.2-x86_64.apk"]
    for name in names:
        path=root/name;path.write_bytes(name.encode())
        if path.suffix==".apk":
            evidence={**signing_identity(),"buildType":"release","signed":True,"debuggable":False,"version":"2.0.2","versionCode":20002,"sha256":hashlib.sha256(path.read_bytes()).hexdigest()}
            path.with_suffix(".build.json").write_text(json.dumps(evidence))
    return names

class ReleaseNotesTests(unittest.TestCase):
    def test_complete_release_inventory_and_checksums(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            names=official_inventory(root)
            self.assertEqual(len(release_assets(root,"2.0.2")),3)
            for line in (root/"SHA256SUMS").read_text().splitlines():
                digest,name=line.split("  ")
                self.assertEqual(digest,hashlib.sha256((root/name).read_bytes()).hexdigest())
            (root/names[0]).unlink()
            with self.assertRaisesRegex(ValueError,"missing=.*windows"):
                release_assets(root,"2.0.2")

    def test_debug_wrong_certificate_tampered_and_missing_metadata_are_rejected(self):
        for field,value in [("debuggable",True),("signed",False),("certificateSha256","0"*64),("sha256","0"*64),("versionCode",1)]:
            with self.subTest(field=field),tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp);names=official_inventory(root)
                metadata=(root/names[1]).with_suffix(".build.json")
                evidence=json.loads(metadata.read_text());evidence[field]=value;metadata.write_text(json.dumps(evidence))
                with self.assertRaisesRegex(ValueError,"mismatch"):release_assets(root,"2.0.2")
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);names=official_inventory(root);(root/names[1]).with_suffix(".build.json").unlink()
            with self.assertRaisesRegex(ValueError,"missing"):release_assets(root,"2.0.2")
            (root/names[0]).write_bytes(b"package")
            (root/"tst-xiangqi-2.0.1-windows.zip").write_bytes(b"stale")
            with self.assertRaisesRegex(ValueError,"unexpected=.*2.0.1"):
                release_assets(root,"2.0.2")

    def test_duplicate_package_names_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            for folder in [root/"a",root/"b"]:
                folder.mkdir();(folder/"same.apk").write_bytes(b"duplicate")
            with self.assertRaisesRegex(ValueError,"duplicate"):
                release_assets(root,"2.0.2")

    def test_exact_section_ignores_unreleased_and_next_version(self):
        text="## 未发布\n- pending\n## v2.0.1（修复）\n\n- correct\n## v2.0.0\n- old"
        self.assertEqual(notes("v2.0.1",text,"2.0.1"),"- correct\n")
    def test_mismatched_missing_or_empty_notes_are_rejected(self):
        for tag,text in [("v2.0.0","## v2.0.1\n- x"),("v2.0.1","## v2.0.10\n- x"),("v2.0.1","## v2.0.1\n## v2.0.0\n- x")]:
            with self.assertRaises(ValueError):notes(tag,text,"2.0.1")
if __name__=="__main__":unittest.main()
