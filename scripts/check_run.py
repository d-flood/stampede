import fcntl
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

from build_environment import build_environment, repository_directory, target_directory


ROOT = Path(__file__).resolve().parents[1]


def main():
    shared = repository_directory(ROOT)
    path = shared / "target/check/run.lock"
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print("./check is already running for this repository.", file=sys.stderr)
            return 75
        with build_environment(target_directory(ROOT), "stampede-e2e-") as env:
            root = env["TMPDIR"]
            env.update(STAMPEDE_CHECK_LOCKED=str(ROOT), STAMPEDE_E2E_ROOT=root)
            child = subprocess.Popen([str(ROOT / "check"), *sys.argv[1:]], env=env, start_new_session=True)
            stopping = None

            def interrupt(sig, frame):
                nonlocal stopping
                stopping = time.monotonic()
                try:
                    os.killpg(child.pid, sig)
                except ProcessLookupError:
                    pass

            signal.signal(signal.SIGINT, interrupt)
            signal.signal(signal.SIGTERM, interrupt)
            try:
                while True:
                    try:
                        status = child.wait(timeout=0.2)
                        return status if status >= 0 else 128 - status
                    except subprocess.TimeoutExpired:
                        if stopping is not None and time.monotonic() - stopping >= 5:
                            os.killpg(child.pid, signal.SIGKILL)
            finally:
                subprocess.run([sys.executable, str(ROOT / "scripts/check_cleanup.py"), root], check=True)


if __name__ == "__main__":
    sys.exit(main())
