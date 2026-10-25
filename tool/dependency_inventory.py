"""Write a deterministic dependency/license and local vendor-patch inventory."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent

def inventory(metadata):
    dependencies = sorted(({
        "name": p["name"], "version": p["version"], "license": p.get("license"),
        "source": p.get("source") or "local", "repository": p.get("repository"),
    } for p in metadata["packages"]), key=lambda p: (p["name"], p["version"]))
    patches = {}
    for component in ["miniquad", "quad-snd"]:
        directory = ROOT / "vendor" / component
        tracked = subprocess.check_output(["git", "ls-files", "-z", "--", str(directory.relative_to(ROOT))], cwd=ROOT).decode().split("\0")
        digest = hashlib.sha256()
        for name in sorted(filter(None, tracked)):
            digest.update(name.encode())
            digest.update(b"\0")
            digest.update(hashlib.sha256((ROOT / name).read_bytes().replace(b"\r\n", b"\n")).digest())
        patches[component] = digest.hexdigest()
    return {"lockSha256": hashlib.sha256((ROOT/"Cargo.lock").read_bytes().replace(b"\r\n", b"\n")).hexdigest(),
            "dependencies": dependencies, "vendorSourceSha256": patches,
            "scope": "declared license metadata and tracked vendor source; not a legal compliance determination"}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output",type=Path)
    args=parser.parse_args()
    metadata=json.loads(subprocess.check_output(["cargo","metadata","--locked","--format-version","1"],cwd=ROOT))
    result=inventory(metadata)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(f"Recorded {len(result['dependencies'])} dependency versions and two vendor digests")

if __name__=="__main__":main()
