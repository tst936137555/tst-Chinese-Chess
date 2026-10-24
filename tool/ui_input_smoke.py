#!/usr/bin/env python3
"""Real X11 mouse/keyboard regression in a disposable data directory (run under Xvfb)."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

def wait(read,predicate,timeout=30):
    end=time.monotonic()+timeout;last=None
    while time.monotonic()<end:
        try:
            last=read()
            if predicate(last):return last
        except (FileNotFoundError,json.JSONDecodeError):pass
        time.sleep(.1)
    raise AssertionError(f"UI condition timed out: {last}")

def main():
    binary=Path(sys.argv[1] if len(sys.argv)>1 else "target/release/tst-xiangqi").resolve()
    with tempfile.TemporaryDirectory(prefix="xiangqi-input-") as tmp:
        root=Path(tmp);state=root/"interaction.json"
        env={**os.environ,"XIANGQI_DATA_DIR":str(root/"data"),"XIANGQI_OBSERVE_PATH":str(state)}
        app=subprocess.Popen([str(binary)],env=env)
        try:
            read=lambda:json.loads(state.read_text(encoding="utf-8"))
            wait(read,lambda s:s["screen"]=="home" and bool(s["controls"]))
            # Miniquad does not publish _NET_WM_PID; this disposable X display
            # contains one game window, identified by its configured title.
            window=subprocess.check_output(["xdotool","search","--sync","--onlyvisible","--name","tst .* Rust"],text=True,timeout=15).splitlines()[0]
            print("X11 game window:",window,flush=True)
            def command(*args):subprocess.run(["xdotool",*map(str,args)],check=True)
            def activate():
                try:
                    result=subprocess.run(["xdotool","windowactivate","--sync",window],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5)
                    return result.returncode==0 and subprocess.check_output(["xdotool","getactivewindow"],text=True,timeout=5).strip()==window
                except (subprocess.CalledProcessError,subprocess.TimeoutExpired):return False
            def pointer_click(x,y,scale):
                wait(activate,bool)
                command("mousemove","--window",window,round(x*scale),round(y*scale))
                wait(read,lambda s:len(s.get("pointer",[]))==2 and abs(s["pointer"][0]-x)<2 and abs(s["pointer"][1]-y)<2)
                command("mousedown",1)
                try:wait(read,lambda s:s.get("pointerDown") is True)
                finally:command("mouseup",1)
                wait(read,lambda s:s.get("pointerDown") is False)
            def click(label):
                wait(activate,bool)
                s=wait(read,lambda s:s.get("sceneStable") and any(c["label"]==label for c in s["controls"]))
                c=next(c for c in s["controls"] if c["label"]==label)
                pointer_click(c["x"]+c["w"]/2,c["y"]+c["h"]/2,s["deviceScale"])
            click("声音");wait(read,lambda s:s["modal"])
            command("key","--window",window,"Escape");wait(read,lambda s:not s["modal"])
            click("开始新对局");wait(read,lambda s:s["modal"]);click("执红开始")
            s=wait(read,lambda s:s["screen"]=="game" and not s["modal"] and s.get("sceneStable") and any(c["label"]=="结束对局" for c in s.get("controls",[])))
            for file,rank in [(7,7),(4,7)]:
                q=next(q for q in s["squares"] if q["file"]==file and q["rank"]==rank)
                pointer_click(q["x"],q["y"],s["deviceScale"])
            wait(read,lambda s:s["plies"]>=1 and s["saved"])
            click("结束对局");wait(read,lambda s:s["modal"]);click("认输并归档")
            wait(read,lambda s:any(c["label"]=="复盘此局" for c in s["controls"]))
            click("复盘此局");wait(read,lambda s:s["screen"]=="review")
            click("末步");wait(read,lambda s:s["reviewPly"]>=1)
            click("首步");wait(read,lambda s:s["reviewPly"]==0)
            command("key","--window",window,"Tab");command("key","--window",window,"Escape")
            print(json.dumps({"osInput":"X11 mouse down/up + keyboard","move":"saved","reviewNavigation":"passed","modalEscape":"passed"}))
            command("key","--window",window,"ctrl+q");time.sleep(.2)
            assert app.poll() is None,"Ctrl+Q must not close the game"
            # Send WM_DELETE_WINDOW through the normal window-manager close action.
            command("windowactivate","--sync",window);command("key","alt+F4");app.wait(timeout=30)
            if app.returncode:raise AssertionError(f"app exit {app.returncode}")
        finally:
            if app.poll() is None:app.terminate();app.wait(timeout=10)
if __name__=="__main__":main()

