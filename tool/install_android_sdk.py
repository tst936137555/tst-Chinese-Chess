"""Install pinned CI SDK packages, retrying failed downloads with a clean staging area."""
import argparse
from pathlib import Path
import shutil
import subprocess
import time


def install_packages(sdk_root, packages, *, sdkmanager="sdkmanager", attempts=3, retry_delay=10):
    if attempts < 1 or not packages or retry_delay < 0:
        raise ValueError("SDK packages, a positive attempt count and a nonnegative delay are required")
    sdk_root = Path(sdk_root).resolve(strict=True)
    command = [str(sdkmanager), f"--sdk_root={sdk_root}", *packages]
    for attempt in range(1, attempts + 1):
        print(f"Android SDK install attempt {attempt}/{attempts}: {packages}", flush=True)
        result = subprocess.run(command, check=False)
        if result.returncode == 0:
            return 0
        if attempt == attempts:
            print(f"Android SDK installation failed after {attempts} attempts", flush=True)
            return result.returncode
        # sdkmanager stages downloaded ZIPs and partial extraction under SDK/.temp.
        # This CI helper runs serially; retain installed packages and accepted licenses.
        staging = sdk_root / ".temp"
        if staging.resolve() != staging:
            raise ValueError("SDK download staging directory must not redirect outside SDK/.temp")
        if staging.exists():
            shutil.rmtree(staging)
        delay = retry_delay * attempt
        print(f"SDK install exited with {result.returncode}; retrying in {delay}s", flush=True)
        time.sleep(delay)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk-root", required=True, type=Path)
    parser.add_argument("packages", nargs="+")
    args = parser.parse_args()
    raise SystemExit(install_packages(args.sdk_root, args.packages))


if __name__ == "__main__":
    main()
