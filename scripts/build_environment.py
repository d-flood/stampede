from contextlib import contextmanager
import fcntl
import os
from pathlib import Path
import re
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
    prune_build(target)
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


UNIT = re.compile(r'-([0-9a-f]{16})(?:\.|$)')


def prune_build(target, age=7 * 86400):
    if not target.exists():
        return
    cutoff = time.time() - age
    for profile in target.iterdir():
        if profile.is_symlink() or not (profile / '.fingerprint').is_dir():
            continue
        with (profile / '.cargo-lock').open('a') as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                continue
            units = {}
            for folder in ('deps', 'build', '.fingerprint'):
                for path in entries(profile / folder):
                    if match := UNIT.search(path.name):
                        units.setdefault(match[1], []).append(path)
            uplifted = [[path] for path in entries(profile) if path.is_file() and not path.name.startswith('.')]
            for paths in [*units.values(), *([path] for path in entries(profile / 'incremental')), *uplifted]:
                if max(used(path) for path in paths) < cutoff:
                    for path in paths:
                        remove(path)
            for path in entries(profile / 'deps'):
                if '.tmp' in path.name and used(path) < time.time() - 86400:
                    remove(path)


def entries(directory):
    return [path for path in directory.iterdir() if not path.is_symlink()] if directory.is_dir() else []


def used(path):
    if not path.is_dir():
        return max(path.stat().st_atime, path.stat().st_mtime)
    return max([path.stat().st_mtime, *(max(child.stat().st_atime, child.stat().st_mtime) for child in entries(path))])


def remove(path):
    try:
        shutil.rmtree(path) if path.is_dir() else path.unlink()
    except FileNotFoundError:
        pass
