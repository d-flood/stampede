import json
import os
import signal
import sys
import time
from pathlib import Path


def fail(message):
    print(message, file=sys.stderr)
    sys.exit(1)


def root():
    return Path(os.environ.get('STAMPEDE_TEST_ROOT') or os.environ['STAMPEDE_E2E_ROOT'])


def option(args, name, default=None):
    return args[args.index(name) + 1] if name in args else default


def environment(home, values=''):
    env = dict(os.environ, HOME=str(home), STAMPEDE_FAKE_SPRITE=str(home))
    if values:
        env.update(pair.split('=', 1) for pair in values.split(','))
    return env


def started(pid):
    try:
        return Path(f'/proc/{pid}/stat').read_text().rsplit(') ', 1)[1].split()[19]
    except FileNotFoundError:
        return None


def stop(record, group=False):
    pid = record['pid']
    if record['started'] is not None and started(pid) == record['started']:
        try:
            (os.killpg if group else os.kill)(pid, signal.SIGKILL)
        except ProcessLookupError:
            return
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            try:
                state = Path(f'/proc/{pid}/stat').read_text().rsplit(') ', 1)[1].split()[0]
                if state == 'Z' or started(pid) != record['started']:
                    break
            except FileNotFoundError:
                break
            time.sleep(0.01)


def stop_descendants(home):
    marker = b'STAMPEDE_FAKE_SPRITE=' + os.fsencode(home)
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        pid = int(entry.name)
        try:
            stamp = started(pid)
            if marker in (entry / 'environ').read_bytes().split(b'\0'):
                stop({'pid': pid, 'started': stamp})
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass


def inject(root, *verbs):
    failure = root / 'sprite-fail'
    if failure.exists():
        lines = failure.read_text().splitlines()
        if lines[0] in verbs:
            fail(lines[1])
