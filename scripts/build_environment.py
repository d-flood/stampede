from contextlib import contextmanager
import fcntl
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import time


def repository_directory(root):
    result = subprocess.run(
        ["git", "rev-parse", "--path-format=absolute", "--show-toplevel", "--git-common-dir"],
        cwd=root, capture_output=True, text=True,
    )
    paths = result.stdout.splitlines()
    if result.returncode == 0 and len(paths) == 2 and Path(paths[0]).resolve() == root.resolve():
        return Path(paths[1]).parent
    return root


def target_directory(root):
    configured = os.environ.get("CARGO_TARGET_DIR")
    return (root / configured if configured else repository_directory(root) / "target").resolve()


@contextmanager
def build_environment(target, prefix):
    temporary = target / "tmp"
    temporary.mkdir(parents=True, exist_ok=True)
    prune_temporary(target)
    with tempfile.TemporaryDirectory(prefix=prefix, dir=temporary) as scratch:
        with (Path(scratch) / ".lease").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_SH)
            ceilings = os.pathsep.join(filter(None, [os.environ.get("GIT_CEILING_DIRECTORIES"), str(temporary)]))
            yield os.environ | {
                "CARGO_TARGET_DIR": str(target),
                "TMPDIR": scratch,
                "TMP": scratch,
                "TEMP": scratch,
                "CLAUDE_CODE_TMPDIR": scratch,
                "NODE_COMPILE_CACHE": str(Path(scratch) / "node-compile-cache"),
                "GIT_CEILING_DIRECTORIES": ceilings,
                "STAMPEDE_BUILD_SCRATCH": scratch,
            }


def prune_temporary(target):
    temporary = target / "tmp"
    if not temporary.exists():
        return
    active = set()
    for process in Path('/proc').iterdir():
        if not process.name.isdigit():
            continue
        try:
            for entry in (process / 'environ').read_bytes().split(b'\0'):
                name, separator, value = entry.partition(b'=')
                if separator and name in {b'TMPDIR', b'STAMPEDE_E2E_ROOT', b'STAMPEDE_BUILD_SCRATCH'}:
                    active.add(Path(os.fsdecode(value)))
        except (OSError, PermissionError):
            pass
    cutoff = time.time() - 86400
    for path in temporary.iterdir():
        if not path.name.startswith(('stampede-e2e-', 'stampede-rebuild-')) or path.is_symlink() or not path.is_dir():
            continue
        if path.stat().st_mtime >= cutoff or any(candidate == path or candidate.is_relative_to(path) for candidate in active):
            continue
        try:
            with (path / '.lease').open('a') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                shutil.rmtree(path)
        except (BlockingIOError, FileNotFoundError):
            pass
