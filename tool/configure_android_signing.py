"""Upload this project's private Android credentials as encrypted GitHub Actions secrets.

Uses the local private signing properties and Git credential helper. Never prints
credential values. Requires PyNaCl, and is run explicitly with --github.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import subprocess
import urllib.error
import urllib.request
from android_signing import release_credentials

REPOSITORY = "tst936137555/tst-Chinese-Chess"

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--github",action="store_true",required=True)
    parser.parse_args()
    from nacl.public import PublicKey, SealedBox
    credentials=release_credentials()
    root=Path(__file__).resolve().parent.parent
    remote=subprocess.check_output(["git","remote","get-url","origin"],cwd=root,text=True).strip().removesuffix(".git")
    if remote!=f"https://github.com/{REPOSITORY}":
        raise SystemExit("Repository identity does not match the signing destination")
    result=subprocess.run(["git","credential","fill"],input="protocol=https\nhost=github.com\n\n",cwd=root,text=True,capture_output=True,timeout=30,env=dict(os.environ,GIT_TERMINAL_PROMPT="0",GCM_INTERACTIVE="Never"))
    values=dict(line.split("=",1) for line in result.stdout.splitlines() if "=" in line)
    token=values.get("password")
    if result.returncode or not token:
        raise SystemExit("No local GitHub credential is available for configuring Actions secrets")
    def request(path,method="GET",body=None):
        req=urllib.request.Request(f"https://api.github.com/repos/{REPOSITORY}/{path}",method=method,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Authorization":f"Bearer {token}","Accept":"application/vnd.github+json","X-GitHub-Api-Version":"2022-11-28","User-Agent":"tst-xiangqi-signing-setup"})
        try:
            with urllib.request.urlopen(req,timeout=30) as response:
                content=response.read()
                return json.loads(content) if content else None
        except urllib.error.HTTPError as error:
            raise SystemExit(f"GitHub signing configuration failed (HTTP {error.code}); no credential values were logged") from None
    public=request("actions/secrets/public-key")
    box=SealedBox(PublicKey(base64.b64decode(public["key"],validate=True)))
    secrets={name:credentials[name] for name in ["ANDROID_KEY_ALIAS","ANDROID_KEYSTORE_PASSWORD","ANDROID_KEY_PASSWORD"]}
    secrets["ANDROID_KEYSTORE_BASE64"]=base64.b64encode(Path(credentials["ANDROID_KEYSTORE"]).read_bytes()).decode()
    for name,value in secrets.items():
        encrypted=base64.b64encode(box.encrypt(value.encode())).decode()
        request(f"actions/secrets/{name}","PUT",{"encrypted_value":encrypted,"key_id":public["key_id"]})
        print(f"Configured encrypted Actions secret: {name}")
    for name in secrets:
        request(f"actions/secrets/{name}")
    print(f"Verified {len(secrets)} signing secrets for {REPOSITORY}")

if __name__=="__main__":main()
