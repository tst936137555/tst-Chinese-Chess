"""Real macOS pointer/keyboard and accessibility actions; requires Accessibility permission."""
import ctypes
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
from windows_input_smoke import wait
ROOT=Path(__file__).resolve().parent.parent
def main():
    if sys.platform!="darwin":raise SystemExit("macOS required")
    services=ctypes.CDLL("/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices")
    services.AXIsProcessTrusted.restype=ctypes.c_bool
    if not services.AXIsProcessTrusted():raise SystemExit("Accessibility permission is required for this native test runner; no permission settings were changed")
    quartz=ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
    foundation=ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
    class Point(ctypes.Structure):_fields_=[("x",ctypes.c_double),("y",ctypes.c_double)]
    quartz.CGEventCreateMouseEvent.argtypes=[ctypes.c_void_p,ctypes.c_uint,Point,ctypes.c_uint];quartz.CGEventCreateMouseEvent.restype=ctypes.c_void_p
    quartz.CGEventPost.argtypes=[ctypes.c_uint,ctypes.c_void_p];foundation.CFRelease.argtypes=[ctypes.c_void_p]
    binary=Path(sys.argv[1]).resolve();dest=Path(sys.argv[2] if len(sys.argv)>2 else ROOT/"target/macos-input-smoke").resolve();dest.mkdir(parents=True,exist_ok=True)
    observation=dest/"interaction.json";data=dest/("data-"+str(time.time_ns()))
    env={**os.environ,"XIANGQI_OBSERVE_PATH":str(observation),"XIANGQI_DATA_DIR":str(data)}
    clipboard=subprocess.check_output(["pbpaste"]);child=None
    def state():return json.loads(observation.read_text())
    def script(body):
        source=f'tell application "System Events"\n tell (first application process whose unix id is {child.pid})\n set frontmost to true\n {body}\n end tell\nend tell'
        return subprocess.check_output(["osascript","-e",source],text=True,timeout=20).strip()
    def launch():
        nonlocal child
        observation.unlink(missing_ok=True);child=subprocess.Popen([str(binary)],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        wait(state,lambda s:s.get("screen")=="home" and s.get("controls"));script("get name")
    def click(x,y,s):
        frame=[float(v) for v in re.findall(r"-?\d+(?:\.\d+)?",script("return (position of window 1) & (size of window 1)"))]
        ratio=frame[2]/s["screenWidth"];point=Point(frame[0]+x*ratio,frame[1]+frame[3]-s["screenHeight"]*ratio+y*ratio)
        for kind in [1,2]:
            event=quartz.CGEventCreateMouseEvent(None,kind,point,0);quartz.CGEventPost(0,event);foundation.CFRelease(event);time.sleep(.05)
    def tap(label):
        time.sleep(.35)
        s=wait(state,lambda s:s.get("sceneStable",True) and any(c["label"]==label for c in s.get("controls",[])))
        c=next(c for c in s["controls"] if c["label"]==label);click(c["x"]+c["w"]/2,c["y"]+c["h"]/2,s)
    try:
        launch();script('set focused of (first button of window 1 whose name is "开始新对局") to true');time.sleep(.4);assert not state()["modal"]
        script("key code 36");wait(state,lambda s:s["modal"]);script("key code 53");wait(state,lambda s:not s["modal"])
        tap("开始新对局");tap("执红开始");s=wait(state,lambda s:s["screen"]=="game" and not s["modal"])
        for file,rank in [(7,7),(4,7)]:
            q=next(q for q in s["squares"] if (q["file"],q["rank"])==(file,rank));click(q["x"],q["y"],s);time.sleep(.2)
        saved=wait(state,lambda s:s.get("plies",0)>=2 and s.get("saved") and not s.get("pending"),45)
        tap("返回");tap("开始新对局");tap("执红开始");wait(state,lambda s:s["screen"]=="game" and not s["modal"])
        tap("返回");tap("继续未完成对局");tap("命名");subprocess.run(["pbcopy"],input="中文残局".encode(),check=True)
        tap("粘贴");script("key code 36")
        wait(lambda:json.loads((data/"state.json").read_text()),lambda d:any(r and r.get("title")=="中文残局" for r in [d.get("active")]+d.get("drafts",[])))
        # Close the ordinary window and verify durable resume.
        script("click (first button of window 1 whose subrole is \"AXCloseButton\")");child.wait(timeout=20);launch()
        tap("继续未完成对局");tap("执红 · 入门 · 2步");restored=wait(state,lambda s:s["screen"]=="game" and not s["modal"])
        assert restored["fen"]==saved["fen"] and restored["plies"]==saved["plies"]
        (dest/"result.json").write_text(json.dumps({"mouseMoves":"passed","nativeFocusDoesNotInvoke":"passed","keyboard":"passed","unicodePaste":"passed","coldResume":"passed","imeComposition":"requires real IME acceptance"},indent=2))
    finally:
        subprocess.run(["pbcopy"],input=clipboard,check=True)
        if child and child.poll() is None:
            try:script("click (first button of window 1 whose subrole is \"AXCloseButton\")");child.wait(timeout=15)
            except (subprocess.TimeoutExpired,subprocess.CalledProcessError):child.terminate();child.wait(timeout=5)
if __name__=="__main__":main()
