#!/usr/bin/env python3
"""Record the exact compiler, target and current source tree used for a release."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess
import tomllib
from build_info import ROOT, version_info

def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

def provenance(toolchain=None, target=None, flags=None, build_command=None):
    command=["rustc", *([f"+{toolchain}"] if toolchain else []), "-vV"]
    compiler=subprocess.check_output(command,text=True).strip()
    host=next(line.split(": ",1)[1] for line in compiler.splitlines() if line.startswith("host: "))
    files=subprocess.check_output(["git","ls-files","--cached","--others","--exclude-standard","-z"],cwd=ROOT).decode("utf-8").split("\0")
    digest=hashlib.sha256()
    for name in sorted(set(files)):
        path=ROOT/name
        if not name or not path.is_file():continue
        digest.update(name.replace("\\","/").encode());digest.update(b"\0");digest.update(bytes.fromhex(sha(path)))
    return {
        **version_info(),"rustc":compiler,"toolchain":toolchain or os.environ.get("RUSTUP_TOOLCHAIN","rust-toolchain.toml"),
        "target":target or host,"flags":flags if flags is not None else os.environ.get("CARGO_ENCODED_RUSTFLAGS",os.environ.get("RUSTFLAGS","")),
        "flagEnvironment":{name:value for name,value in os.environ.items() if name in ["RUSTFLAGS","CARGO_ENCODED_RUSTFLAGS","IPHONEOS_DEPLOYMENT_TARGET"] or (name.startswith("CARGO_TARGET_") and name.endswith(("_RUSTFLAGS","_LINKER")))},
        "releaseProfile":tomllib.loads((ROOT/"Cargo.toml").read_text(encoding="utf-8-sig")).get("profile",{}).get("release",{}),
        "buildCommand":build_command or os.environ.get("XIANGQI_BUILD_COMMAND"),
        "sourceCommit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
        "sourceDirty":bool(subprocess.check_output(["git","status","--porcelain"],cwd=ROOT)),
        "sourceSha256":digest.hexdigest(),"lockSha256":sha(ROOT/"Cargo.lock"),
    }

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder",type=Path);parser.add_argument("--toolchain");parser.add_argument("--target");parser.add_argument("--flags")
    parser.add_argument("--output",type=Path,default=Path("BUILD_INFO.json"))
    parser.add_argument("--build-command")
    args=parser.parse_args();folder=args.folder.resolve()
    if not folder.is_dir():raise SystemExit("release directory does not exist")
    output=(folder/args.output).resolve()
    if not output.is_relative_to(folder):raise SystemExit("manifest output escaped release directory")
    manifest=json.loads((ROOT/"tool/engine_manifest.json").read_text(encoding="utf-8"))
    result={**provenance(args.toolchain,args.target,args.flags,args.build_command),"engineCommit":manifest["pikafish"]["commit"],"nnueSha256":manifest["pikafish"]["artifacts"]["nnue"]["sha256"],
        "files":{p.relative_to(folder).as_posix():sha(p) for p in folder.rglob("*") if p.is_file() and p.resolve()!=output}}
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
if __name__=="__main__":main()

