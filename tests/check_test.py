import os
from pathlib import Path
import shutil
import subprocess
import time
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class Check(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="oikonomos-check-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        shutil.copy(os.environ.get("CHECK_TEST_SCRIPT", ROOT / "check"), self.root / "check")
        if (ROOT / "scripts").exists():
            shutil.copytree(ROOT / "scripts", self.root / "scripts")
        (self.root / "bin").mkdir()
        (self.root / "web/src").mkdir(parents=True)
        (self.root / "crates/oikonomos").mkdir(parents=True)
        for name in ["web/package.json", "web/pnpm-lock.yaml", "web/src/app.svelte", "crates/oikonomos/Cargo.toml"]:
            (self.root / name).write_text("initial")
        self.log = self.root / "calls"
        self.env = {**os.environ, "PATH": f"{self.root}/bin:{os.environ['PATH']}", "CHECK_TEST_ROOT": str(self.root)}
        fake = '''#!/usr/bin/env python3
import os
from pathlib import Path
import sys
import time
root = Path(os.environ["CHECK_TEST_ROOT"])
with (root / "calls").open("a") as log:
    log.write(Path(sys.argv[0]).name + " " + " ".join(sys.argv[1:]) + "\\n")
if Path(sys.argv[0]).name == "cargo":
    (root / "started").touch()
    while (root / "hold").exists():
        time.sleep(0.02)
if "install" in sys.argv:
    (root / "web/node_modules").mkdir(exist_ok=True)
if "build" in sys.argv and Path(sys.argv[0]).name == "pnpm":
    if (root / "fail").exists():
        sys.exit(1)
    (root / "web/build").mkdir(exist_ok=True)
    (root / "web/build/index.html").write_text("built")
'''
        for name in ["pnpm", "cargo"]:
            executable = self.root / "bin" / name
            executable.write_text(fake)
            executable.chmod(0o755)

    def run_check(self, *args, success=True):
        result = subprocess.run([str(self.root / "check"), *args], env=self.env, capture_output=True, text=True, timeout=15)
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def calls(self, command):
        return [line for line in self.log.read_text().splitlines() if command in line]

    def test_unchanged_e2e_reuses_frontend_and_development_binary(self):
        self.run_check("e2e", "--list")
        self.run_check("e2e", "--list")
        self.assertEqual(len(self.calls(" install ")), 1)
        self.assertEqual(len(self.calls("pnpm -C web build")), 1)
        self.assertFalse(self.calls("--release"))

    def test_frontend_additions_deletions_and_version_changes_invalidate_build(self):
        self.run_check("e2e", "--list")
        source = self.root / "web/src/new.svelte"
        source.write_text("new")
        self.run_check("e2e", "--list")
        source.unlink()
        self.run_check("e2e", "--list")
        (self.root / "crates/oikonomos/Cargo.toml").write_text("new version")
        self.run_check("e2e", "--list")
        self.assertEqual(len(self.calls("pnpm -C web build")), 4)
        self.assertEqual(len(self.calls(" install ")), 1)

    def test_missing_output_and_failed_build_are_not_cached(self):
        self.run_check("e2e", "--list")
        (self.root / "web/build/index.html").unlink()
        (self.root / "fail").touch()
        self.run_check("e2e", "--list", success=False)
        (self.root / "fail").unlink()
        self.run_check("e2e", "--list")
        self.assertEqual(len(self.calls("pnpm -C web build")), 3)

    def test_interrupted_cache_write_triggers_a_rebuild(self):
        self.run_check("e2e", "--list")
        (self.root / "target/check/web.json").write_text("{")
        self.run_check("e2e", "--list")
        self.assertEqual(len(self.calls("pnpm -C web build")), 2)

    def test_lockfile_change_reinstalls_dependencies(self):
        self.run_check("e2e", "--list")
        (self.root / "web/pnpm-lock.yaml").write_text("changed")
        self.run_check("e2e", "--list")
        self.assertEqual(len(self.calls(" install ")), 2)
        self.assertEqual(len(self.calls("pnpm -C web build")), 2)

    def test_overlapping_check_is_rejected_and_lock_is_released(self):
        hold = self.root / "hold"
        hold.touch()
        first = subprocess.Popen([str(self.root / "check"), "rust"], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            deadline = time.monotonic() + 5
            while not (self.root / "started").exists() and time.monotonic() < deadline:
                time.sleep(0.02)
            self.assertTrue((self.root / "started").exists())
            result = self.run_check("rust", success=False)
            self.assertIn("already running", result.stderr)
        finally:
            hold.unlink()
            first.communicate(timeout=10)
        self.run_check("rust")
        self.assertEqual(len(self.calls("cargo test")), 2)

    def test_worktrees_share_the_check_lock(self):
        def git(*args):
            subprocess.run(["git", "-C", str(self.root), "-c", "user.name=check", "-c", "user.email=check@example.invalid", "-c", "commit.gpgsign=false", *args], check=True, capture_output=True)
        git("init", "-q")
        git("add", ".")
        git("commit", "-qm", "fixture")
        other = self.root / "other"
        git("worktree", "add", "--detach", str(other))
        (self.root / "hold").touch()
        first = subprocess.Popen([str(self.root / "check"), "rust"], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            deadline = time.monotonic() + 5
            while not (self.root / "started").exists() and time.monotonic() < deadline:
                time.sleep(0.02)
            self.assertTrue((self.root / "started").exists())
            result = subprocess.run([str(other / "check"), "rust"], env=self.env, capture_output=True, text=True, timeout=3)
            self.assertEqual(result.returncode, 75, result.stdout + result.stderr)
        finally:
            (self.root / "hold").unlink()
            first.communicate(timeout=10)

    def test_interrupt_stops_children_and_releases_lock(self):
        (self.root / "hold").touch()
        first = subprocess.Popen([str(self.root / "check"), "rust"], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            deadline = time.monotonic() + 5
            while not (self.root / "started").exists() and time.monotonic() < deadline:
                time.sleep(0.02)
            self.assertTrue((self.root / "started").exists())
            first.terminate()
            first.communicate(timeout=10)
            self.assertNotEqual(first.returncode, 0)
        finally:
            if first.poll() is None:
                first.kill()
                first.communicate()
            (self.root / "hold").unlink()
        self.run_check("rust")

    def test_cleanup_stops_detached_children_without_touching_other_runs(self):
        marker = str(self.root / "run")
        detached = subprocess.Popen(["sleep", "60"], env={**self.env, "OIKONOMOS_E2E_ROOT": marker}, start_new_session=True)
        unrelated = subprocess.Popen(["sleep", "60"], start_new_session=True)
        try:
            subprocess.run(["python3", str(ROOT / "scripts/check_cleanup.py"), marker], check=True, timeout=5)
            detached.wait(timeout=3)
            self.assertIsNone(unrelated.poll())
        finally:
            for child in [detached, unrelated]:
                if child.poll() is None:
                    child.kill()
                child.wait()


if __name__ == "__main__":
    unittest.main()
