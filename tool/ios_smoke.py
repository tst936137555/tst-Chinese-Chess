#!/usr/bin/env python3
"""Install and launch the native app on an Apple Silicon iOS simulator."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
from ui_input_smoke import wait
PACKAGE="com.tst.xiangqi.rust"
def sim(*args,**kw):return subprocess.check_output(["xcrun","simctl",*args],text=True,timeout=kw.pop("timeout",60),**kw).strip()
def wait_for_smoke(output, timeout=300, stall_timeout=60):
    started=changed=time.monotonic();last_stage=None
    while time.monotonic()-started < timeout:
        stage_path=output/"startup.txt"
        stage=stage_path.read_text() if stage_path.exists() else "waiting for UI startup"
        if stage!=last_stage:
            print("iOS smoke:",stage,flush=True)
            last_stage=stage;changed=time.monotonic()
        try:
            value=json.loads((output/"result.json").read_text())
            if value.get("scenes")==29 and value.get("realEngine")=="passed":return value
            raise AssertionError(f"iOS smoke returned an unsuccessful result: {value}")
        except (FileNotFoundError,json.JSONDecodeError):pass
        if time.monotonic()-changed >= stall_timeout:
            raise AssertionError(f"iOS smoke stalled for {stall_timeout}s at {last_stage}")
        time.sleep(.2)
    raise AssertionError(f"iOS smoke exceeded {timeout}s at {last_stage}")

def run_checks(uid,container,dest):
    output=container/"Documents/ui-smoke"
    (output/"result.json").unlink(missing_ok=True)
    (output/"startup.txt").unlink(missing_ok=True)
    # Pass the test directory through the child environment as well as argv:
    # This also supports launchers that do not preserve the command-line arguments.
    smoke_env={**os.environ,"SIMCTL_CHILD_XIANGQI_SMOKE_OUTPUT":str(output)}
    def launch(phase,env,*args):
        print("iOS launch:",phase,flush=True)
        print(sim("launch","--stdout="+str(dest/f"{phase}-stdout.log"),"--stderr="+str(dest/f"{phase}-stderr.log"),uid,PACKAGE,*args,env=env),flush=True)
    # The smoke app may already have exited successfully after writing its result.
    def terminate():subprocess.run(["xcrun","simctl","terminate",uid,PACKAGE],check=False,timeout=15)
    launch("smoke",smoke_env,"--smoke-test",str(output))
    result=wait_for_smoke(output)
    terminate()
    observation=container/"Documents/interaction.json"
    env={**os.environ,"SIMCTL_CHILD_XIANGQI_OBSERVE_PATH":str(observation),"SIMCTL_CHILD_XIANGQI_DATA_DIR":str(container/"Documents/isolated-input-data")}
    observation.unlink(missing_ok=True)
    launch("native",env)
    wait(lambda:json.loads(observation.read_text()),lambda s:s.get("screen")=="home" and bool(s.get("controls")))
    sim("io",uid,"screenshot",str(output/"native-launch.png"))
    shutil.copy2(output/"native-launch.png",dest/"native-launch.png")
    terminate();observation.unlink(missing_ok=True);launch("cold",env)
    wait(lambda:json.loads(observation.read_text()),lambda s:s.get("screen")=="home")
    result["simulator"]={"install":"passed","nativeLaunch":"passed","coldLaunch":"passed","rotationAndOsTouch":"requires XCTest/device verification"}
    (dest/"result.json").write_text(json.dumps(result,indent=2)+"\n")
    print(json.dumps(result,indent=2))

def collect_diagnostics(uid,container,dest):
    if container is not None:
        output=container/"Documents/ui-smoke"
        # Keep the application's rendering result separate from the full harness result.
        if output.exists():shutil.copytree(output,dest/"app",dirs_exist_ok=True)
        observation=container/"Documents/interaction.json"
        if observation.exists():shutil.copy2(observation,dest/"interaction.json")
        stage=output/"startup.txt"
        print("Last startup stage:",stage.read_text() if stage.exists() else "app did not enter UI startup",flush=True)
    try:
        with (dest/"simulator.log").open("w") as log:
            subprocess.run(["xcrun","simctl","spawn",uid,"log","show","--last","10m","--style","compact","--predicate",'process == "tst-xiangqi"'],stdout=log,stderr=subprocess.STDOUT,check=False,timeout=30)
    except (OSError,subprocess.TimeoutExpired) as error:
        print("Simulator log collection failed:",error,flush=True)
    finally:
        try:subprocess.run(["xcrun","simctl","terminate",uid,PACKAGE],check=False,timeout=15)
        except (OSError,subprocess.TimeoutExpired) as error:print("Simulator cleanup failed:",error,flush=True)

def main():
    app=Path(sys.argv[1]).resolve()
    dest=Path("target/ios-smoke").resolve();dest.mkdir(parents=True,exist_ok=True)
    devices=json.loads(sim("list","devices","available","--json"))["devices"]
    candidates=[d for runtime,ds in devices.items() if "iOS" in runtime for d in ds if d.get("isAvailable") and "iPhone" in d["name"]]
    if not candidates:raise SystemExit("No available iOS simulator")
    device=next((d for d in candidates if d["state"]=="Booted"),candidates[0]);uid=device["udid"]
    container=None
    try:
        if device["state"]!="Booted":sim("boot",uid)
        sim("bootstatus",uid,"-b",timeout=180);sim("install",uid,str(app))
        container=Path(sim("get_app_container",uid,PACKAGE,"data"))
        run_checks(uid,container,dest)
    finally:collect_diagnostics(uid,container,dest)
if __name__=="__main__":main()

