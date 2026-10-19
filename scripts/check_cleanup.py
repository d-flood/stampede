import os
from pathlib import Path
import signal
import sys
import time


root = sys.argv[1].encode()
marker = b"OIKONOMOS_E2E_ROOT=" + root
test_marker = b"OIKONOMOS_TEST_ROOT=" + root


def processes():
    found = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit() or int(entry.name) in (os.getpid(), os.getppid()):
            continue
        try:
            environment = (entry / 'environ').read_bytes().split(b'\0')
            if marker in environment or test_marker in environment:
                found.append(int(entry.name))
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass
    return found


def stop(pids, sig):
    for pid in pids:
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass


stop(processes(), signal.SIGTERM)
deadline = time.monotonic() + 2
while processes() and time.monotonic() < deadline:
    time.sleep(0.05)
stop(processes(), signal.SIGKILL)
