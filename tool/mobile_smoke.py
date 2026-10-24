#!/usr/bin/env python3
"""Emulator-only real Android input, native AI, lifecycle and durable-state regression."""
import json
import subprocess
import time
from pathlib import Path
from build_info import ROOT,version_info
from ui_input_smoke import wait
PACKAGE="com.tst.xiangqi.rust"
AUTOMATION_ROOT=f"/sdcard/Android/data/{PACKAGE}/files"
def adb(*args,check=True,timeout=15):
    return subprocess.run(["adb",*map(str,args)],check=check,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=timeout)

def collect_diagnostics():
    output=ROOT/"target/android-smoke";output.mkdir(parents=True,exist_ok=True)
    commands={
        "logcat.txt": ("logcat","-d"),
        "activity.txt": ("shell","dumpsys","activity","activities"),
        "startup.txt": ("shell","cat",f"{AUTOMATION_ROOT}/smoke/startup.txt"),
        "memory.txt": ("shell","cat",f"{AUTOMATION_ROOT}/smoke/memory.txt"),
    }
    for name,args in commands.items():
        try:
            result=adb(*args,check=False)
            (output/name).write_bytes(result.stdout+result.stderr)
        except (OSError,subprocess.TimeoutExpired) as error:
            (output/name).write_text(str(error),encoding="utf-8")
def launch(*args):
    return adb("shell","am","start","--activity-single-top","-n",f"{PACKAGE}/.MainActivity",*args)
def foreground():
    dump=adb("shell","dumpsys","activity","activities").stdout.decode(errors="replace")
    return any(PACKAGE in line and ("mResumedActivity" in line or "topResumedActivity" in line) for line in dump.splitlines())
def wait_for_smoke(timeout=300, stall_timeout=90):
    started=changed=time.monotonic();last_stage=None
    while time.monotonic()-started < timeout:
        output=adb("shell","cat",f"{AUTOMATION_ROOT}/smoke/startup.txt",check=False)
        stage=output.stdout.decode(errors="replace").strip() if output.returncode==0 else "waiting for native startup"
        if stage!=last_stage:
            print("Android smoke:",stage,flush=True);last_stage=stage;changed=time.monotonic()
        output=adb("shell","cat",f"{AUTOMATION_ROOT}/smoke/result.json",check=False)
        if output.returncode==0:
            try:result=json.loads(output.stdout)
            except json.JSONDecodeError:result=None
            if result is not None:
                if result.get("scenes")!=29 or result.get("realEngine")!="passed":
                    raise AssertionError(f"Android smoke returned an unsuccessful result: {result}")
                return result
        if not adb("shell","pidof",PACKAGE,check=False).stdout.strip():
            raise AssertionError(f"Android app exited before smoke result at {last_stage}")
        if time.monotonic()-changed>=stall_timeout:
            raise AssertionError(f"Android smoke stalled for {stall_timeout}s at {last_stage}")
        time.sleep(.5)
    raise AssertionError(f"Android smoke exceeded {timeout}s at {last_stage}")

def main():
    if adb("shell","getprop","ro.kernel.qemu").stdout.strip()!=b"1":raise SystemExit("This isolated release install test requires an Android emulator")
    apk=ROOT/"dist"/f"tst-xiangqi-{version_info()['version']}-x86_64.apk"
    adb("install","-r",apk,timeout=120);adb("shell","am","force-stop",PACKAGE)
    adb("logcat","-c")
    adb("shell","rm","-f",f"{AUTOMATION_ROOT}/smoke/result.json",f"{AUTOMATION_ROOT}/smoke/startup.txt",check=False)
    launch("--ez","xiangqiSmoke","true")
    result=wait_for_smoke()
    adb("shell","am","force-stop",PACKAGE);adb("shell","rm","-f",f"{AUTOMATION_ROOT}/interaction/interaction.json",check=False);launch("--ez","xiangqiObserve","true")
    def state():
        output=adb("shell","cat",f"{AUTOMATION_ROOT}/interaction/interaction.json",check=False)
        return json.loads(output.stdout) if output.returncode==0 else {}
    def tap(label):
        s=wait(state,lambda s:any(c["label"]==label for c in s.get("controls",[])))
        c=next(c for c in s["controls"] if c["label"]==label);dpi=s["deviceScale"];ox,oy=s["viewOrigin"]
        adb("shell","input","tap",round((c["x"]+c["w"]/2)*dpi+ox),round((c["y"]+c["h"]/2)*dpi+oy));time.sleep(.25)
    wait(state,lambda s:s.get("screen")=="home")
    tap("开始新对局");wait(state,lambda s:s.get("modal"))
    adb("shell","input","keyevent",4);wait(state,lambda s:s.get("screen")=="home" and not s.get("modal"));assert foreground()
    tap("开始新对局");tap("执红开始")
    s=wait(state,lambda s:s.get("screen")=="game" and not s.get("modal") and s.get("sceneStable") and any(c["label"]=="结束对局" for c in s.get("controls",[])))
    for file,rank in [(7,7),(4,7)]:
        q=next(q for q in s["squares"] if q["file"]==file and q["rank"]==rank);dpi=s["deviceScale"];ox,oy=s["viewOrigin"]
        adb("shell","input","tap",round(q["x"]*dpi+ox),round(q["y"]*dpi+oy));time.sleep(.25)
    s=wait(state,lambda s:s.get("plies",0)>=2 and s.get("saved") and not s.get("pending"),45)
    before=s["pauseSerial"];tap("结束对局")
    adb("shell","input","keyevent",3);time.sleep(.5);launch()
    wait(state,lambda s:s.get("pauseSerial",0)>before and any(c["label"]=="按当前局势结算" for c in s.get("controls",[])),45)
    adb("shell","input","keyevent",4);wait(state,lambda s:s.get("screen")=="game" and not s.get("modal"));assert foreground()
    old_rotation=adb("shell","settings","get","system","user_rotation").stdout.decode().strip()
    old_auto=adb("shell","settings","get","system","accelerometer_rotation").stdout.decode().strip()
    try:
        adb("shell","settings","put","system","accelerometer_rotation",0);adb("shell","settings","put","system","user_rotation",1)
        rotated=wait(state,lambda s:s.get("screenWidth",0)>s.get("screenHeight",1))
        assert all(c["h"]>=43.9 for c in rotated["controls"])
    finally:
        adb("shell","settings","put","system","user_rotation",old_rotation)
        adb("shell","settings","put","system","accelerometer_rotation",old_auto)
    saved=wait(state,lambda s:s.get("saved"))
    adb("shell","am","force-stop",PACKAGE);adb("shell","rm","-f",f"{AUTOMATION_ROOT}/interaction/interaction.json",check=False);launch("--ez","xiangqiObserve","true")
    wait(state,lambda s:s.get("screen")=="home");tap("继续未完成对局")
    restored=wait(state,lambda s:s.get("screen")=="game")
    assert restored["fen"]==saved["fen"] and restored["plies"]==saved["plies"]
    tap("返回");wait(state,lambda s:s.get("screen")=="home" and s.get("sceneStable"))
    adb("shell","input","keyevent",4);wait(foreground,lambda active:not active)
    launch();wait(state,lambda s:s.get("screen")=="home");assert foreground()
    tap("继续未完成对局");resumed=wait(state,lambda s:s.get("screen")=="game")
    assert resumed["fen"]==saved["fen"] and resumed["plies"]==saved["plies"]
    result["osInput"]={"backModal":"passed","foregroundActivity":"passed","nativeAiReply":"passed","endAnalysisPauseResume":"passed","rotation":"passed","coldResume":"passed","homeBackResume":"passed"}
    output=ROOT/"target/android-smoke";output.mkdir(parents=True,exist_ok=True)
    (output/"result.json").write_text(json.dumps(result,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
    print(json.dumps(result,indent=2,ensure_ascii=False))
if __name__=="__main__":
    try:main()
    finally:collect_diagnostics()

