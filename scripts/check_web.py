import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / "web"
CACHE = ROOT / "target/check"


def fingerprint(paths):
    digest = hashlib.sha256()
    for path in sorted(paths):
        if path.is_file():
            digest.update(str(path.relative_to(ROOT)).encode() + b"\0")
            digest.update(path.read_bytes())
            digest.update(b"\0")
    return digest.hexdigest()


def prepare():
    CACHE.mkdir(parents=True, exist_ok=True)
    dependencies = fingerprint([WEB / name for name in ["package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml", ".npmrc"]])
    installed = WEB / "node_modules/.check-inputs"
    if not installed.exists() or installed.read_text() != dependencies:
        subprocess.run(["pnpm", "-C", "web", "install", "--frozen-lockfile"], cwd=ROOT, check=True)
        installed.write_text(dependencies)
    inputs = fingerprint([
        *(path for path in WEB.iterdir() if path.name != "playwright.config.ts"),
        *(WEB / "src").rglob("*"),
        *(WEB / "static").rglob("*"),
        ROOT / "crates/oikonomos/Cargo.toml",
    ]) + os.environ.get("OIKONOMOS_BUILD_ID", "")
    stamp = CACHE / "web.json"
    output = WEB / "build"
    state = {"inputs": inputs, "output": fingerprint(output.rglob("*"))}
    try:
        previous = json.loads(stamp.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        previous = None
    if (output / "index.html").exists() and previous == state:
        print("Frontend unchanged; reusing build.", flush=True)
        return
    stamp.unlink(missing_ok=True)
    started = time.monotonic()
    log = CACHE / "web-build.log"
    print("Building frontend…", flush=True)
    with log.open("w") as stream:
        result = subprocess.run(["pnpm", "-C", "web", "build"], cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
    if result.returncode:
        print("\n".join(log.read_text().splitlines()[-60:]), file=sys.stderr)
        raise SystemExit(result.returncode)
    stamp.write_text(json.dumps({"inputs": inputs, "output": fingerprint(output.rglob("*"))}))
    print(f"Frontend built in {time.monotonic() - started:.1f}s; log: {log.relative_to(ROOT)}", flush=True)


if __name__ == "__main__":
    prepare()
