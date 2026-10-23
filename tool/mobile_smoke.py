#!/usr/bin/env python3
"""Emulator-only real Android input, native AI, lifecycle and durable-state regression."""
import json
import subprocess
import time
from pathlib import Path
from build_info import ROOT,version_info
from ui_input_smoke import wait
PACKAGE="com.tst.xiangqi.rust"
def adb(*args,check=True):
    return subprocess.run(["adb",*map(str,args)],check=check,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
def launch(*args):
    return adb("shell","am","start","--activity-single-top","-n",f"{PACKAGE}/.MainActivity",*args)
def foreground():
    dump=adb("shell","dumpsys","activity","activities").stdout.decode(errors="replace")
    return any(PACKAGE in line and ("mResumedActivity" in line or "topResumedActivity" in line) for line in dump.splitlines())
def main():
    if adb("shell","getprop","ro.kernel.qemu").stdout.strip()!=b"1":raise SystemExit("This destructive debug install test requires an Android emulator")
    apk=ROOT/"dist"/f"tst-xiangqi-{version_info()['version']}-x86_64.apk"
    adb("install","-r",apk);adb("shell","am","force-stop",PACKAGE)
    adb("shell","run-as",PACKAGE,"rm","-f","files/smoke/result.json",check=False)
    launch("--ez","xiangqiSmoke","true")
    def smoke_result():
        output=adb("shell","run-as",PACKAGE,"cat","files/smoke/result.json",check=False)
        return json.loads(output.stdout) if output.returncode==0 else {}
    result=wait(smoke_result,lambda r:r.get("scenes")==29 and r.get("realEngine")=="passed",120)
    adb("shell","am","force-stop",PACKAGE);adb("shell","run-as",PACKAGE,"rm","-f","files/interaction/interaction.json",check=False);launch("--ez","xiangqiObserve","true")
    def state():
        output=adb("shell","run-as",PACKAGE,"cat","files/interaction/interaction.json",check=False)
        return json.loads(output.stdout) if output.returncode==0 else {}
    def tap(label):
        s=wait(state,lambda s:any(c["label"]==label for c in s.get("controls",[])))
        c=next(c for c in s["controls"] if c["label"]==label);dpi=s["deviceScale"];ox,oy=s["viewOrigin"]
        adb("shell","input","tap",round((c["x"]+c["w"]/2)*dpi+ox),round((c["y"]+c["h"]/2)*dpi+oy));time.sleep(.25)
    wait(state,lambda s:s.get("screen")=="home")
    tap("开始新对局");wait(state,lambda s:s.get("modal"))
    adb("shell","input","keyevent",4);wait(state,lambda s:s.get("screen")=="home" and not s.get("modal"));assert foreground()
    tap("开始新对局");tap("执红开始")
    s=wait(state,lambda s:s.get("screen")=="game" and not s.get("modal"))
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
    adb("shell","am","force-stop",PACKAGE);adb("shell","run-as",PACKAGE,"rm","-f","files/interaction/interaction.json",check=False);launch("--ez","xiangqiObserve","true")
    wait(state,lambda s:s.get("screen")=="home");tap("继续未完成对局")
    restored=wait(state,lambda s:s.get("screen")=="game")
    assert restored["fen"]==saved["fen"] and restored["plies"]==saved["plies"]
    result["osInput"]={"backModal":"passed","foregroundActivity":"passed","nativeAiReply":"passed","endAnalysisPauseResume":"passed","rotation":"passed","coldResume":"passed"}
    output=ROOT/"target/android-smoke";output.mkdir(parents=True,exist_ok=True)
    (output/"result.json").write_text(json.dumps(result,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
    print(json.dumps(result,indent=2,ensure_ascii=False))
if __name__=="__main__":main()

