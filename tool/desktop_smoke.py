"""Run rendering smoke with bounded progress, captured logs and graceful exit checks."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time


def wait_for_result(output, process, timeout=300, stall_timeout=90):
    started = changed = time.monotonic()
    last_stage = None
    while time.monotonic() - started < timeout:
        stage_file = output / "startup.txt"
        stage = stage_file.read_text() if stage_file.exists() else "waiting for native startup"
        if stage != last_stage:
            print("Desktop smoke:", stage, flush=True)
            last_stage, changed = stage, time.monotonic()
        try:
            result = json.loads((output / "result.json").read_text(encoding="utf-8"))
        except (FileNotFoundError, json.JSONDecodeError):
            result = None
        if result is not None:
            if result.get("scenes") != 29 or result.get("realEngine") not in ("passed", "assets unavailable"):
                raise AssertionError(f"Invalid smoke result: {result}")
            return result
        if process.poll() is not None:
            raise AssertionError(f"App exited with {process.returncode} before result at {last_stage}")
        if time.monotonic() - changed >= stall_timeout:
            raise AssertionError(f"Desktop smoke stalled for {stall_timeout}s at {last_stage}")
        time.sleep(.2)
    raise AssertionError(f"Desktop smoke exceeded {timeout}s at {last_stage}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    for name in ("result.json", "startup.txt"):
        (output / name).unlink(missing_ok=True)
    # A distinct save directory and stale-result removal make repeat runs meaningful.
    env = {**os.environ, "XIANGQI_DATA_DIR": str(output / "data")}
    with (output / "app.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen([str(args.binary.resolve()), "--smoke-test", str(output)],
                                   env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            result = wait_for_result(output, process)
            code = process.wait(timeout=30)
            if code:
                raise AssertionError(f"Smoke app exited with {code}")
            print(json.dumps(result, ensure_ascii=False), flush=True)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)


if __name__ == "__main__":
    main()
