import json
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
        self.directory = tempfile.TemporaryDirectory(prefix="stampede-check-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        shutil.copy(os.environ.get("CHECK_TEST_SCRIPT", ROOT / "check"), self.root / "check")
        if (ROOT / "scripts").exists():
            shutil.copytree(ROOT / "scripts", self.root / "scripts")
        (self.root / "bin").mkdir()
        (self.root / "tests").mkdir()
        (self.root / "tests/runner_test.py").write_text('''from pathlib import Path
import unittest

class RunnerFixture(unittest.TestCase):
    def test_runs(self):
        Path("runner-executed").touch()
        self.assertFalse(Path("fail-runner").exists())
''')
        (self.root / "web/src").mkdir(parents=True)
        (self.root / "crates/stampede").mkdir(parents=True)
        for name in ["web/package.json", "web/pnpm-lock.yaml", "web/src/app.svelte", "crates/stampede/Cargo.toml"]:
            (self.root / name).write_text("initial")
        self.log = self.root / "calls"
        self.env = {key: value for key, value in os.environ.items() if key != "CARGO_TARGET_DIR"}
        self.env.update(PATH=f"{self.root}/bin:{os.environ['PATH']}", CHECK_TEST_ROOT=str(self.root))
        fake = '''#!/usr/bin/env python3
import os
from pathlib import Path
import subprocess
import sys
import time
root = Path(os.environ["CHECK_TEST_ROOT"])
with (root / "calls").open("a") as log:
    log.write(Path(sys.argv[0]).name + " " + " ".join(sys.argv[1:]) + "\\n")
if "playwright" in sys.argv:
    import json
    with (root / "browser-runs.jsonl").open("a") as log:
        log.write(json.dumps(dict(suite=os.environ.get("STAMPEDE_CHECK_PERF"), root=os.environ.get("STAMPEDE_E2E_ROOT"))) + "\\n")
if Path(sys.argv[0]).name == "cargo":
    import json
    target = Path(os.environ.get("CARGO_TARGET_DIR", Path.cwd() / "target"))
    target.mkdir(parents=True, exist_ok=True)
    artifact = target / "fixture-artifact"
    scratch_repository = subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=os.environ["TMPDIR"], capture_output=True).returncode == 0
    with (root / "builds.jsonl").open("a") as log:
        log.write(json.dumps(dict(target=str(target), source=os.environ.get("STAMPEDE_BUILD_ROOT"), reused=artifact.exists(), temporary=os.environ.get("TMPDIR", "/tmp"), node_cache=os.environ.get("NODE_COMPILE_CACHE"), binary=os.environ.get("STAMPEDE_E2E_BINARY"), scratch_repository=scratch_repository)) + "\\n")
    artifact.touch()
    scratch = Path(os.environ["TMPDIR"])
    (scratch / "compiler-temporary").write_text("temporary build output")
    cache = Path(os.environ["NODE_COMPILE_CACHE"])
    cache.mkdir(exist_ok=True)
    (cache / "compiled").write_text("cached compilation")
    if (root / "fail-cargo").exists():
        sys.exit(1)
    (root / "started").touch()
    while (root / "hold").exists():
        time.sleep(0.02)
    if "test" in sys.argv and "--no-run" not in sys.argv:
        passed = 0 if "--ignored" in sys.argv and (root / "empty-perf").exists() else 1
        print(f"test result: ok. {passed} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s")
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

    def test_default_checks_keep_performance_work_explicit(self):
        result = self.run_check()
        self.assertTrue(self.calls("pnpm -C web check"))
        self.assertTrue(self.calls("cargo test --workspace --no-run"))
        self.assertFalse(self.calls("cargo build"))
        self.assertFalse(self.calls("--ignored"))
        self.assertFalse(self.calls("--release"))
        browser = json.loads((self.root / "browser-runs.jsonl").read_text())
        self.assertEqual(browser["suite"], "0")
        self.assertTrue(browser["root"])
        self.assertIn("Rust test compilation passed in", result.stdout)
        self.assertIn("Browser tests passed in", result.stdout)

    def test_performance_checks_run_stress_and_browser_budgets(self):
        self.run_check("perf")
        self.assertTrue(self.calls("memory:: -- --ignored --test-threads=1"))
        self.assertFalse(self.calls("pnpm -C web check"))
        browser = json.loads((self.root / "browser-runs.jsonl").read_text())
        self.assertEqual(browser["suite"], "1")

    def test_empty_performance_suite_is_a_failure(self):
        (self.root / "empty-perf").touch()
        result = self.run_check("perf", success=False)
        self.assertIn("ran no tests", result.stderr)
        self.assertFalse(self.calls("playwright"))

    def test_all_checks_include_functional_and_performance_suites(self):
        self.run_check("all")
        browsers = [json.loads(line) for line in (self.root / "browser-runs.jsonl").read_text().splitlines()]
        self.assertEqual([browser["suite"] for browser in browsers], ["0", "1"])
        self.assertTrue(self.calls("--ignored"))

    def test_component_checks_need_neither_rust_nor_a_production_frontend(self):
        self.run_check("components", "text.test.ts")
        self.assertFalse(self.calls("cargo"))
        self.assertFalse(self.calls("pnpm -C web build"))
        self.assertTrue(self.calls("vitest run text.test.ts"))
        self.assertFalse(self.calls("playwright"))

    def test_runner_checks_need_neither_rust_nor_frontend_dependencies(self):
        self.run_check("runner")
        self.assertFalse(self.log.exists())
        self.assertTrue((self.root / "runner-executed").exists())

    def test_failed_runner_check_preserves_failure(self):
        (self.root / "fail-runner").touch()
        result = self.run_check("runner", success=False)
        self.assertEqual(result.returncode, 1)

    def test_failed_rust_build_does_not_start_browser_tests(self):
        (self.root / "fail-cargo").touch()
        result = self.run_check("e2e", success=False)
        self.assertEqual(result.returncode, 1)
        self.assertFalse(self.calls("playwright"))

    def test_pruning_is_explicit_and_does_not_build(self):
        artifact = self.root / "target/debug/deps/libold-aaaaaaaaaaaaaaaa.rlib"
        artifact.parent.mkdir(parents=True)
        artifact.touch()
        os.utime(artifact, (1, 1))
        (self.root / "target/debug/.fingerprint").mkdir()
        self.run_check("prune")
        self.assertFalse(artifact.exists())
        self.assertFalse(self.log.exists())

    def test_development_serve_reuses_frontend_and_builds_debug(self):
        shutil.copy(ROOT / "serve", self.root / "serve")
        binary = self.root / "target/debug/stampede"
        binary.parent.mkdir(parents=True)
        binary.write_text('''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys
Path(os.environ["CHECK_TEST_ROOT"]).joinpath("served.json").write_text(json.dumps(sys.argv[1:]))
''')
        binary.chmod(0o755)
        for _ in range(2):
            result = subprocess.run([str(self.root / "serve"), "--dev", "--port", "5678"], env=self.env, capture_output=True, text=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(json.loads((self.root / "served.json").read_text()), ["serve", "--port", "5678"])
        self.assertEqual(len(self.calls("pnpm -C web build")), 1)
        self.assertEqual(len(self.calls("cargo build")), 2)
        self.assertFalse(self.calls("--release"))

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
        (self.root / "crates/stampede/Cargo.toml").write_text("new version")
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

    def test_worktrees_reuse_build_artifacts(self):
        def git(*args):
            subprocess.run(["git", "-C", str(self.root), "-c", "user.name=check", "-c", "user.email=check@example.invalid", "-c", "commit.gpgsign=false", *args], check=True, capture_output=True)
        git("init", "-q")
        git("add", ".")
        git("commit", "-qm", "fixture")
        other = self.root / "other"
        git("worktree", "add", "--detach", str(other))
        self.run_check("rust")
        result = subprocess.run([str(other / "check"), "rust"], env=self.env, capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        builds = [json.loads(line) for line in (self.root / "builds.jsonl").read_text().splitlines()]
        self.assertEqual([build["reused"] for build in builds], [False, True])
        self.assertEqual(builds[0]["target"], builds[1]["target"])
        self.assertEqual([build["source"] for build in builds], [str(self.root), str(other)])
        self.assertEqual(builds[1]["binary"], str(Path(builds[1]["target"]) / "debug/stampede"))
        self.assertFalse((other / "target/fixture-artifact").exists())
        self.assertFalse(any(build["scratch_repository"] for build in builds))

    def test_temporary_build_files_stay_on_disk_and_are_cleaned(self):
        self.run_check("rust")
        build = json.loads((self.root / "builds.jsonl").read_text())
        temporary = Path(build["temporary"])
        self.assertTrue(temporary.is_relative_to(self.root / "target"), str(temporary))
        self.assertFalse(temporary.exists())
        self.assertTrue(Path(build["node_cache"]).is_relative_to(temporary))

    def test_failed_check_cleans_temporary_build_files(self):
        (self.root / "fail-cargo").touch()
        self.run_check("rust", success=False)
        build = json.loads((self.root / "builds.jsonl").read_text())
        self.assertFalse(Path(build["temporary"]).exists())
        self.assertFalse(Path(build["node_cache"]).exists())

    def test_configured_target_is_used_for_builds_and_binary(self):
        target = self.root / "custom-target"
        self.env["CARGO_TARGET_DIR"] = "custom-target"
        self.run_check("e2e", "--list")
        build = json.loads((self.root / "builds.jsonl").read_text())
        self.assertEqual(build["target"], str(target))
        self.assertEqual(build["binary"], str(target / "debug/stampede"))
        self.assertTrue(Path(build["temporary"]).is_relative_to(target))

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
            build = json.loads((self.root / "builds.jsonl").read_text())
            self.assertFalse(Path(build["temporary"]).exists())
        finally:
            if first.poll() is None:
                first.kill()
                first.communicate()
            (self.root / "hold").unlink()
        self.run_check("rust")

    def test_cleanup_stops_detached_children_without_touching_other_runs(self):
        marker = str(self.root / "run")
        detached = subprocess.Popen(["sleep", "60"], env={**self.env, "STAMPEDE_E2E_ROOT": marker}, start_new_session=True)
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
