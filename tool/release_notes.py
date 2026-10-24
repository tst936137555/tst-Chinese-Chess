"""Validate a version tag and extract its exact changelog section."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
from build_info import ROOT,version_info
from android_signing import signing_identity, verify_release
def notes(tag,text,version):
    if tag!="v"+version:raise ValueError("tag does not match Cargo.toml version")
    lines=text.splitlines();start=None
    for index,line in enumerate(lines):
        match=re.match(r"^## v([^\s（(]+)",line)
        if match and match[1]==version:start=index;break
    if start is None:raise ValueError("version section missing from CHANGELOG.md")
    end=next((i for i in range(start+1,len(lines)) if lines[i].startswith("## ")),len(lines))
    body="\n".join(lines[start+1:end]).strip()
    if not body:raise ValueError("release notes are empty")
    return body+"\n"
def release_assets(root,version,android_build_tools=None):
    artifacts=sorted(p for p in root.rglob("*") if p.is_file() and p.suffix in [".zip",".apk"])
    names=[p.name for p in artifacts]
    expected={
        f"tst-xiangqi-{version}-windows.zip",
        f"tst-xiangqi-{version}-arm64-v8a.apk",
        f"tst-xiangqi-{version}-x86_64.apk",
    }
    if len(set(names))!=len(names):raise ValueError("duplicate release artifact basenames")
    if set(names)!=expected:
        raise ValueError(f"release artifacts mismatch: missing={sorted(expected-set(names))}, unexpected={sorted(set(names)-expected)}")
    lines=[]
    for path in artifacts:
        with path.open("rb") as stream:digest=hashlib.file_digest(stream,"sha256").hexdigest()
        if path.suffix==".apk":
            metadata=path.with_suffix(".build.json")
            if not metadata.is_file():raise ValueError("Android release signing evidence is missing")
            evidence=json.loads(metadata.read_text(encoding="utf-8"))
            major,minor,patch=map(int,version.split("."));build=major*10000+minor*100+patch
            expected={"buildType":"release","signed":True,"debuggable":False,"package":signing_identity()["package"],"certificateSha256":signing_identity()["certificateSha256"],"version":version,"versionCode":build,"sha256":digest}
            if any(evidence.get(k)!=v or type(evidence.get(k)) is not type(v) for k,v in expected.items()):
                raise ValueError("Android release metadata, certificate or APK checksum mismatch")
            if android_build_tools is not None:
                signer=android_build_tools/("apksigner.bat" if os.name=="nt" else "apksigner")
                aapt=android_build_tools/("aapt.exe" if os.name=="nt" else "aapt")
                verify_release(path,signer,aapt,version,build)
        lines.append(f"{digest}  {path.name}")
    (root/"SHA256SUMS").write_text("\n".join(lines)+"\n",encoding="utf-8")
    return artifacts

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag",required=True);parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--assets",type=Path)
    parser.add_argument("--android-build-tools",type=Path)
    args=parser.parse_args();body=notes(args.tag,(ROOT/"CHANGELOG.md").read_text(encoding="utf-8-sig"),version_info()["version"])
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(body,encoding="utf-8")
    if args.assets:
        if not args.android_build_tools:raise ValueError("Release inventory requires actual Android signature verification")
        release_assets(args.assets,version_info()["version"],args.android_build_tools)
if __name__=="__main__":main()
