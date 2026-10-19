from contextlib import contextmanager
import os
from pathlib import Path
import subprocess
import tempfile


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
    with tempfile.TemporaryDirectory(prefix=prefix, dir=temporary) as scratch:
        ceilings = os.pathsep.join(filter(None, [os.environ.get("GIT_CEILING_DIRECTORIES"), str(temporary)]))
        yield os.environ | {
            "CARGO_TARGET_DIR": str(target),
            "TMPDIR": scratch,
            "TMP": scratch,
            "TEMP": scratch,
            "NODE_COMPILE_CACHE": str(Path(scratch) / "node-compile-cache"),
            "GIT_CEILING_DIRECTORIES": ceilings,
        }
