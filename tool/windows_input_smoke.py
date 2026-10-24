"""Real Win32 mouse/keyboard, UIA focus, Unicode commit and durable cold resume."""
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT=Path(__file__).resolve().parent.parent
def wait(read, predicate, timeout=30):
    end=time.monotonic()+timeout;value=None
    while time.monotonic()<end:
        try:
            value=read()
            if predicate(value):return value
        except (OSError,ValueError,KeyError):pass
        time.sleep(.1)
    detail={k:v for k,v in value.items() if k!="squares"} if isinstance(value,dict) else value
    raise AssertionError(f"Windows input smoke timed out: {detail}")

def main():
    if sys.platform!="win32":raise SystemExit("Windows desktop required")
    user=ctypes.WinDLL("user32",use_last_error=True)
    user.SetProcessDPIAware()
    user.ClientToScreen.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.POINT)]
    user.SetForegroundWindow.argtypes=[wintypes.HWND]
    user.GetForegroundWindow.restype=wintypes.HWND
    user.ShowWindow.argtypes=[wintypes.HWND,ctypes.c_int]
    user.GetWindowThreadProcessId.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.DWORD)]
    user.PostMessageW.argtypes=[wintypes.HWND,wintypes.UINT,wintypes.WPARAM,wintypes.LPARAM]
    binary=Path(sys.argv[1]).resolve()
    dest=Path(sys.argv[2] if len(sys.argv)>2 else ROOT/"target/windows-input-smoke").resolve()
    dest.mkdir(parents=True,exist_ok=True)
    (dest/"result.json").unlink(missing_ok=True)
    framework=Path(os.environ["WINDIR"])/"Microsoft.NET/Framework64/v4.0.30319"
    helper=dest/"accessibility-action.exe"
    subprocess.run([str(framework/"csc.exe"),"/nologo",f"/out:{helper}",*[f"/reference:{framework/'WPF'/name}" for name in ["UIAutomationClient.dll","UIAutomationTypes.dll","WindowsBase.dll"]],str(ROOT/"tool/windows_accessibility_action.cs")],check=True,timeout=30)
    observed=dest/"interaction.json";data_dir=dest/("data-"+str(time.time_ns()))
    env={**os.environ,"XIANGQI_OBSERVE_PATH":str(observed),"XIANGQI_DATA_DIR":str(data_dir)}
    class Mouse(ctypes.Structure):_fields_=[("dx",wintypes.LONG),("dy",wintypes.LONG),("mouseData",wintypes.DWORD),("dwFlags",wintypes.DWORD),("time",wintypes.DWORD),("dwExtraInfo",ctypes.c_size_t)]
    class Keyboard(ctypes.Structure):_fields_=[("wVk",wintypes.WORD),("wScan",wintypes.WORD),("dwFlags",wintypes.DWORD),("time",wintypes.DWORD),("dwExtraInfo",ctypes.c_size_t)]
    class Payload(ctypes.Union):_fields_=[("mi",Mouse),("ki",Keyboard)]
    class Input(ctypes.Structure):_fields_=[("type",wintypes.DWORD),("payload",Payload)]
    user.SendInput.argtypes=[wintypes.UINT,ctypes.POINTER(Input),ctypes.c_int]
    def key(vk,up=False,scan=0,unicode=False):
        # Miniquad maps hardware scan codes from LPARAM, not only virtual keys.
        if not unicode:scan=user.MapVirtualKeyW(vk,0)
        event=Input(type=1,payload=Payload(ki=Keyboard(0,scan,(2 if up else 0)|(4 if unicode else 8),0,0)))
        if user.SendInput(1,ctypes.byref(event),ctypes.sizeof(event))!=1:raise ctypes.WinError(ctypes.get_last_error())
    def press(vk):key(vk);time.sleep(.06);key(vk,True)
    def commit(text):
        units=text.encode("utf-16-le")
        for i in range(0,len(units),2):
            scan=int.from_bytes(units[i:i+2],"little");key(0,scan=scan,unicode=True);key(0,True,scan,True)
    def read():return json.loads(observed.read_text(encoding="utf-8"))
    kernel=ctypes.WinDLL("kernel32",use_last_error=True)
    kernel.CreateFileW.argtypes=[wintypes.LPCWSTR,wintypes.DWORD,wintypes.DWORD,ctypes.c_void_p,wintypes.DWORD,wintypes.DWORD,wintypes.HANDLE]
    kernel.CreateFileW.restype=wintypes.HANDLE;kernel.CloseHandle.argtypes=[wintypes.HANDLE]
    def hwnd(pid):
        found=[]
        callback_type=ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.HWND,wintypes.LPARAM)
        def callback(window,_):
            process_id=wintypes.DWORD();user.GetWindowThreadProcessId(window,ctypes.byref(process_id))
            if process_id.value==pid and user.IsWindowVisible(window):found.append(window)
            return True
        user.EnumWindows(callback_type(callback),0)
        return found[0] if found else None
    def launch():
        observed.unlink(missing_ok=True)
        with (dest/"app.log").open("a",encoding="utf-8") as log:
            child=subprocess.Popen([str(binary)],env=env,stdout=log,stderr=subprocess.STDOUT)
        wait(read,lambda s:s.get("screen")=="home" and s.get("controls"))
        window=wait(lambda:hwnd(child.pid),lambda h:h is not None)
        user.ShowWindow(window,9);user.SetForegroundWindow(window)
        return child,window
    child=None;locked=None
    try:
        child,window=launch()
        def click(x,y,state):
            origin=wintypes.POINT(0,0);assert user.ClientToScreen(window,ctypes.byref(origin))
            scale=state.get("deviceScale",1)
            user.SetCursorPos(round(origin.x+x*scale),round(origin.y+y*scale))
            user.mouse_event(2,0,0,0,0)
            try:wait(read,lambda s:s.get("pointerDown") is True)
            finally:user.mouse_event(4,0,0,0,0)
            wait(read,lambda s:s.get("pointerDown") is False)
        def tap(label):
            time.sleep(.35)
            wait(read,lambda s:s.get("sceneStable",True) and any(c["label"]==label for c in s.get("controls",[])))
            subprocess.run([str(helper),str(child.pid),label,"invoke"],check=True,timeout=20,creationflags=0x08000000)
            user.SetForegroundWindow(window)
        def focus(label):
            subprocess.run([str(helper),str(child.pid),label,"focus"],check=True,timeout=20,creationflags=0x08000000)
            user.SetForegroundWindow(window)
        focus("开始新对局");time.sleep(.4);assert not read()["modal"],"UIA Focus invoked new game"
        tap("开始新对局");tap("执红开始")
        state=wait(read,lambda s:s["screen"]=="game" and not s["modal"] and s.get("sceneStable",True))
        time.sleep(.4);state=read()
        for file,rank in [(7,7),(4,7)]:
            square=next(q for q in state["squares"] if (q["file"],q["rank"])==(file,rank));click(square["x"],square["y"],state);time.sleep(.2)
        saved=wait(read,lambda s:s.get("plies",0)>=2 and s.get("saved") and not s.get("pending"),45)
        # Deny atomic replacement of this isolated save, then recover through actual UI.
        locked=kernel.CreateFileW(str(data_dir/"state.json"),0x80000000,1,None,3,0,None)
        assert locked not in (None,ctypes.c_void_p(-1).value)
        for file,rank in [(0,6),(0,5)]:
            square=next(q for q in saved["squares"] if (q["file"],q["rank"])==(file,rank));click(square["x"],square["y"],saved);time.sleep(.2)
        wait(read,lambda s:s.get("plies",0)>=4 and not s.get("pending") and s.get("pendingSaves",1)==0 and s.get("failedSaves",0)>0 and any(c["label"]=="处理保存" for c in s.get("controls",[])),45)
        kernel.CloseHandle(locked);locked=None
        tap("处理保存");tap("重试保存")
        saved=wait(read,lambda s:s.get("saved") and not s.get("modal") and not s.get("pending"))
        focus("结束对局");time.sleep(.4);assert not read()["modal"],"UIA Focus invoked end dialog"
        focused=read();print(json.dumps({"focusedControl":focused.get("focusedControl"),"focusIndex":focused.get("keyboardFocus"),"plies":focused.get("plies")}),flush=True)
        print(json.dumps({"targetWindow":window,"foregroundWindow":user.GetForegroundWindow()}),flush=True)
        assert user.GetForegroundWindow()==window,"Test window is not receiving OS keyboard input"
        press(13);wait(read,lambda s:s["modal"]);press(27);wait(read,lambda s:not s["modal"])
        tap("返回");tap("开始新对局");tap("执红开始");wait(read,lambda s:s["screen"]=="game" and not s["modal"])
        tap("返回");tap("继续未完成对局");tap("命名")
        key(17);press(65);key(17,True);commit("中文残局");press(13)
        wait(lambda:json.loads((data_dir/"state.json").read_text(encoding="utf-8")),lambda d:any(r.get("title")=="中文残局" for r in [d.get("active")]+d.get("drafts",[]) if r))
        user.PostMessageW(window,0x0010,0,0);child.wait(timeout=20)
        child,window=launch();tap("继续未完成对局");tap(f"执红 · 入门 · {saved['plies']}步")
        restored=wait(read,lambda s:s["screen"]=="game" and not s["modal"])
        assert restored["fen"]==saved["fen"] and restored["plies"]==saved["plies"]
        key(17);press(81);key(17,True);time.sleep(.3)
        assert child.poll() is None,"Ctrl+Q must not close the game"
        user.PostMessageW(window,0x0010,0,0);child.wait(timeout=20)
        (dest/"result.json").write_text(json.dumps({"mouseMoves":"passed","uiaFocusDoesNotInvoke":"passed","keyboardInvokeEscape":"passed","windowCloseSave":"passed","ctrlQIgnored":"passed","unicodeCommit":"passed","saveFailureRetry":"passed","coldResume":"passed","imeComposition":"requires real IME/device acceptance"},indent=2),encoding="utf-8")
    finally:
        if locked not in (None,ctypes.c_void_p(-1).value):kernel.CloseHandle(locked)
        if child and child.poll() is None:
            user.PostMessageW(window,0x0010,0,0)
            try:child.wait(timeout=15)
            except subprocess.TimeoutExpired:child.terminate();child.wait(timeout=5)

if __name__=="__main__":main()
