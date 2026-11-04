import os
from pathlib import Path
import subprocess
import sys

from build_environment import build_environment, target_directory


root = Path(__file__).resolve().parents[1]
target = target_directory(root)
with build_environment(target, "stampede-rebuild-") as environment:
    environment.setdefault("CARGO_BUILD_JOBS", "4")
    subprocess.run([sys.executable, str(root / "scripts/check_web.py")], cwd=root, env=environment, check=True)
    subprocess.run(["cargo", "build", "--quiet"], cwd=root, env=environment, check=True)
os.execv(target / "debug/stampede", [str(target / "debug/stampede"), "serve", *sys.argv[1:]])
