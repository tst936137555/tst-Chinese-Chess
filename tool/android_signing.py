"""Resolve private release credentials and verify public APK signing evidence."""
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent

def signing_identity():
    return json.loads((ROOT / "tool/android_signing.json").read_text(encoding="utf-8"))

def release_credentials(environment=None, home=None):
    env = dict(os.environ if environment is None else environment)
    config = Path(env.get("ANDROID_SIGNING_PROPERTIES") or (Path.home() if home is None else Path(home)) / ".tst-xiangqi/signing/key.properties")
    properties = {}
    if config.is_file():
        for line in config.read_text(encoding="utf-8-sig").splitlines():
            if "=" in line and not line.lstrip().startswith(("#", "!")):
                key, value = line.split("=", 1)
                properties[key.strip()] = value.strip()
    mapping = {"ANDROID_KEYSTORE":"storeFile", "ANDROID_KEY_ALIAS":"keyAlias", "ANDROID_KEYSTORE_PASSWORD":"storePassword", "ANDROID_KEY_PASSWORD":"keyPassword"}
    for variable, key in mapping.items():
        if not env.get(variable) and properties.get(key):
            value = properties[key]
            if key == "storeFile":
                value = str((config.parent / value).resolve())
            env[variable] = value
    if any(not env.get(key) for key in mapping):
        raise ValueError("正式构建缺少 Android 签名配置；请配置私有 key.properties 或 ANDROID_KEYSTORE/ANDROID_KEY_ALIAS 及两个密码环境变量")
    if not Path(env["ANDROID_KEYSTORE"]).is_file():
        raise ValueError("正式签名密钥文件不存在")
    return env

def certificate(apk, apksigner):
    result = subprocess.run([str(apksigner), "verify", "--verbose", "--print-certs", str(apk)], capture_output=True, text=True, errors="replace", timeout=30)
    fingerprints = re.findall(r"certificate SHA-256 digest: ([a-fA-F0-9]{64})", result.stdout)
    if result.returncode or len(set(fingerprints)) != 1:
        raise ValueError("APK 签名验证失败或签名者数量不符合正式发布要求")
    return fingerprints[0].lower()

def verify_release(apk, apksigner, aapt, version, build):
    identity = signing_identity()
    digest = certificate(apk, apksigner)
    if digest != identity["certificateSha256"]:
        raise ValueError("APK 证书与项目正式签名不一致，已拒绝发布")
    result = subprocess.run([str(aapt), "dump", "badging", str(apk)], capture_output=True, text=True, errors="replace", timeout=30)
    expected = f"package: name='{identity['package']}' versionCode='{build}' versionName='{version}'"
    if result.returncode or expected not in result.stdout or "application-debuggable" in result.stdout:
        raise ValueError("APK 包名、版本或非调试属性验证失败")
    return {"certificateSha256": digest, "package":identity["package"], "debuggable":False, "signed":True, "buildType":"release", "version":version, "versionCode":build}
