#!/usr/bin/env python3
"""Install and launch the native app on an Apple Silicon iOS simulator."""
import json
import os
from pathlib import Path
import subprocess
import sys
from ui_input_smoke import wait
PACKAGE="com.tst.xiangqi.rust"
def sim(*args,**kw):return subprocess.check_output(["xcrun","simctl",*args],text=True,**kw).strip()
def main():
    app=Path(sys.argv[1]).resolve()
    devices=json.loads(sim("list","devices","available","--json"))["devices"]
    candidates=[d for runtime,ds in devices.items() if "iOS" in runtime for d in ds if d.get("isAvailable") and "iPhone" in d["name"]]
    if not candidates:raise SystemExit("No available iOS simulator")
    device=next((d for d in candidates if d["state"]=="Booted"),candidates[0]);uid=device["udid"]
    if device["state"]!="Booted":sim("boot",uid)
    sim("bootstatus",uid,"-b");sim("install",uid,str(app))
    container=Path(sim("get_app_container",uid,PACKAGE,"data"))
    output=container/"Documents/ui-smoke"
    (output/"result.json").unlink(missing_ok=True)
    sim("launch",uid,PACKAGE,"--smoke-test",str(output))
    result=wait(lambda:json.loads((output/"result.json").read_text()),lambda r:r.get("scenes")==29 and r.get("realEngine")=="passed",120)
    subprocess.run(["xcrun","simctl","terminate",uid,PACKAGE],check=False)
    observation=container/"Documents/interaction.json"
    env={**os.environ,"SIMCTL_CHILD_XIANGQI_OBSERVE_PATH":str(observation),"SIMCTL_CHILD_XIANGQI_DATA_DIR":str(container/"Documents/isolated-input-data")}
    observation.unlink(missing_ok=True)
    sim("launch",uid,PACKAGE,env=env)
    wait(lambda:json.loads(observation.read_text()),lambda s:s.get("screen")=="home" and bool(s.get("controls")))
    sim("io",uid,"screenshot",str(output/"native-launch.png"))
    subprocess.run(["xcrun","simctl","terminate",uid,PACKAGE],check=False);observation.unlink(missing_ok=True);sim("launch",uid,PACKAGE,env=env)
    wait(lambda:json.loads(observation.read_text()),lambda s:s.get("screen")=="home")
    result["simulator"]={"install":"passed","nativeLaunch":"passed","coldLaunch":"passed","rotationAndOsTouch":"requires XCTest/device verification"}
    dest=Path("target/ios-smoke");dest.mkdir(parents=True,exist_ok=True)
    (dest/"result.json").write_text(json.dumps(result,indent=2)+"\n")
    print(json.dumps(result,indent=2))
if __name__=="__main__":main()

