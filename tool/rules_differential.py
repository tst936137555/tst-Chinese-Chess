"""Compare complete root move sets and depth-two counts against the bundled engine."""
import argparse
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import threading
import time
ROOT=Path(__file__).resolve().parent.parent
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("probe",type=Path);parser.add_argument("--engine",type=Path)
    parser.add_argument("--output",type=Path,default=ROOT/"target/rules-differential.json")
    parser.add_argument("--update-corpus",type=Path)
    args=parser.parse_args()
    cases=json.loads(subprocess.check_output([str(args.probe.resolve())],text=True,timeout=90))
    engine=args.engine or Path(os.environ.get("PIKAFISH_PATH",ROOT/("pikafish.exe" if os.name=="nt" else "assets/engine/pikafish")))
    if not engine.is_file():raise SystemExit("A real Pikafish engine is required")
    net=Path(os.environ.get("PIKAFISH_NNUE",ROOT/"assets/engine/pikafish.nnue")).resolve()
    process=subprocess.Popen([str(engine.resolve())],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,encoding="utf-8",errors="replace")
    output=queue.Queue()
    def reader():
        for line in process.stdout:output.put(line.strip())
        output.put(None)
    threading.Thread(target=reader,daemon=True).start()
    def send(line):process.stdin.write(line+"\n");process.stdin.flush()
    def until(predicate,timeout=30):
        lines=[];end=time.monotonic()+timeout
        while time.monotonic()<end:
            line=output.get(timeout=max(.01,end-time.monotonic()))
            if line is None:raise AssertionError(f"engine exited: {lines[-10:]}")
            lines.append(line)
            if predicate(line):return lines
        raise AssertionError("perft oracle timed out")
    verified=[]
    try:
        send("uci");identity=until(lambda line:line=="uciok")
        send("setoption name Threads value 1");send("setoption name Hash value 32")
        send(f"setoption name EvalFile value {net}");send("isready");until(lambda line:line=="readyok")
        for case in cases:
            send("position fen "+case["fen"]);send("go perft 2")
            lines=until(lambda line:line.startswith("Nodes searched:"))
            roots={match[1]:int(match[2]) for line in lines if (match:=re.fullmatch(r"([a-i][0-9][a-i][0-9]):\s*(\d+)",line))}
            nodes=int(lines[-1].split(":",1)[1])
            assert roots==case["roots"],f"{case['name']}: legal root/count mismatch: {roots} != {case['roots']}"
            assert nodes==case["perft2"]
            verified.append({k:case[k] for k in ["name","fen","perft1","perft2"]})
        result={"engine":str(engine),"identity":[line for line in identity if line.startswith("id ")],"cases":verified,"comparison":"full root move set and every depth-two branch count"}
        args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
        if args.update_corpus:
            args.update_corpus.write_text(json.dumps([c for c in verified if not c["name"].startswith("random-")],ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
        print(f"Pikafish differential: {len(verified)} positions passed")
    finally:
        if process.poll() is None:
            try:send("quit");process.wait(timeout=5)
            except (BrokenPipeError,subprocess.TimeoutExpired):process.kill();process.wait()
        process.stdin.close();process.stdout.close()
if __name__=="__main__":main()
