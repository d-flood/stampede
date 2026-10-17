import fcntl
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from importlib.machinery import SourceFileLoader
from importlib.util import module_from_spec, spec_from_loader
import json
from pathlib import Path
import shlex
import subprocess
import tempfile
import threading
import unittest
from unittest.mock import patch


loader = SourceFileLoader("rebuild", str(Path(__file__).resolve().parents[1] / "rebuild"))
rebuild = module_from_spec(spec_from_loader(loader.name, loader))
loader.exec_module(rebuild)


class Deployment(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.install = self.root / "installed"
        self.install.mkdir()
        self.unit = self.root / "oikonomos.service"
        self.binary = self.install / "oikonomos"
        self.candidate = self.root / "target/release/oikonomos"
        self.candidate.parent.mkdir(parents=True)
        self.calls = []
        self.live = None
        self.fail_build = False
        self.fail_start = False
        self.fail_health = False
        self.build = None
        for name, value in (("INSTALL", self.install), ("UNIT", self.unit), ("ROOT", self.root)):
            self.enterContext(patch.object(rebuild, name, value))
        self.enterContext(patch.object(rebuild.shutil, "which", return_value="available"))
        self.enterContext(patch.object(rebuild, "run", side_effect=self.run_command))
        self.enterContext(patch.object(rebuild, "readiness", side_effect=self.ready))

    def seed(self, binary=None):
        binary = binary or self.binary
        binary.parent.mkdir(parents=True, exist_ok=True)
        binary.write_text("old-build")
        self.unit.write_text(f'[Service]\nExecStart="{binary}" serve --port 5678 --config "{self.root}/a config.toml"\nKillMode=process\n[X-Oikonomos]\nPort=5678\nHttpsPort=0\n')
        self.live = "old-build"
        (self.install / "deployment.json").write_text(json.dumps(dict(buildId="old-build")))

    def run_command(self, *command, **kwargs):
        command = [str(part) for part in command]
        self.calls.append(command)
        if command[0] == "cargo":
            if self.fail_build:
                raise subprocess.CalledProcessError(1, command)
            self.build = kwargs["env"]["OIKONOMOS_BUILD_ID"]
            self.candidate.write_text(self.build)
        elif command[0] == str(self.binary) and command[1] == "install":
            port = command[command.index("--port") + 1]
            config = command[command.index("--config") + 1]
            data = command[command.index("--data-dir") + 1]
            self.unit.write_text(f'[Service]\nExecStart="{self.binary}" serve --port {port} --config "{config}" --data-dir "{data}"\nKillMode=process\n[X-Oikonomos]\nPort={port}\nHttpsPort=0\n')
        elif command[:3] == ["systemctl", "--user", "restart"]:
            if self.fail_start:
                self.fail_start = False
                raise subprocess.CalledProcessError(1, command)
            self.live = self.binary.read_text()

    def ready(self, port, build=None):
        if self.fail_health:
            self.fail_health = False
            raise RuntimeError("wrong build")
        self.assertEqual(self.live, build)
        return dict(version="0.1.0", buildId=self.live)

    def deploy(self, *args):
        rebuild.deploy(rebuild.parser().parse_args(args))

    def test_first_run_installs_and_remembers_configuration(self):
        self.deploy("--port", "5678", "--config", str(self.root / "a config.toml"))
        self.assertEqual(self.binary.read_text(), self.build)
        self.assertIn("--local-only", next(call for call in self.calls if call[0] == str(self.binary)))
        manifest = json.loads((self.install / "deployment.json").read_text())
        self.assertEqual(manifest["settings"]["port"], 5678)
        self.assertEqual(manifest["buildId"], self.build)
        self.calls.clear()
        self.deploy()
        self.assertFalse(any(call[0] == str(self.binary) for call in self.calls))
        self.assertEqual(json.loads((self.install / "deployment.json").read_text())["settings"], manifest["settings"])

    def test_failed_build_leaves_the_running_server_untouched(self):
        self.seed()
        old_unit = self.unit.read_text()
        self.fail_build = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.deploy()
        self.assertEqual(self.binary.read_text(), "old-build")
        self.assertEqual(self.unit.read_text(), old_unit)
        self.assertFalse(any("restart" in call for call in self.calls))

    def test_failed_restart_restores_previous_binary_and_configuration(self):
        self.seed()
        old_manifest = (self.install / "deployment.json").read_text()
        self.fail_start = True
        with self.assertRaises(subprocess.CalledProcessError):
            self.deploy("--port", "6789")
        self.assertEqual(self.live, "old-build")
        self.assertIn("--port 5678", self.unit.read_text())
        self.assertEqual((self.install / "deployment.json").read_text(), old_manifest)

    def test_wrong_build_is_rolled_back(self):
        self.seed()
        self.fail_health = True
        with self.assertRaisesRegex(RuntimeError, "wrong build"):
            self.deploy()
        self.assertEqual(self.live, "old-build")

    def test_first_start_failure_removes_the_failed_service(self):
        self.fail_start = True
        with patch.object(rebuild.subprocess, "run") as stopped:
            with self.assertRaises(subprocess.CalledProcessError):
                self.deploy()
        self.assertFalse(self.unit.exists())
        self.assertFalse(self.binary.exists())
        self.assertFalse((self.install / "deployment.json").exists())
        self.assertIn("disable", stopped.call_args.args[0])

    def test_migration_keeps_a_copy_before_the_build_replaces_its_output(self):
        self.seed(self.candidate)
        self.fail_health = True
        with self.assertRaises(RuntimeError):
            self.deploy()
        self.assertEqual(self.binary.read_text(), "old-build")
        command = next(line for line in self.unit.read_text().splitlines() if line.startswith("ExecStart="))
        self.assertEqual(shlex.split(command.removeprefix("ExecStart="))[0], str(self.binary))

    def test_deployment_lock_rejects_a_second_rebuild(self):
        with (self.install / "rebuild.lock").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with patch("sys.argv", ["rebuild"]):
                with self.assertRaisesRegex(RuntimeError, "another Oikonomos rebuild"):
                    rebuild.main()
        self.assertEqual(self.calls, [])

    def test_configuration_paths_are_unescaped_before_reinstalling(self):
        self.seed()
        self.unit.write_text(self.unit.read_text().replace("a config.toml", "a $$config%%.toml"))
        self.deploy("--port", "6789")
        command = next(call for call in self.calls if call[0] == str(self.binary))
        self.assertEqual(command[command.index("--config") + 1], str(self.root / "a $config%.toml"))

    def test_rollback_restores_the_previous_tailscale_target(self):
        self.seed()
        self.unit.write_text(self.unit.read_text().replace("HttpsPort=0", "HttpsPort=8443"))
        self.fail_health = True
        with self.assertRaises(RuntimeError):
            self.deploy("--port", "6789")
        self.assertIn(["tailscale", "serve", "--bg", "--https=8443", "http://127.0.0.1:5678"], self.calls)


class Readiness(unittest.TestCase):
    def test_waits_for_the_exact_build_id(self):
        builds = iter(["old-build", "new-build"])

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200)
                self.end_headers()
                self.wfile.write(json.dumps(dict(version="0.1.0", buildId=next(builds))).encode())

            def log_message(self, *args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        try:
            self.assertEqual(rebuild.readiness(server.server_port, "new-build", seconds=2)["buildId"], "new-build")
        finally:
            server.shutdown()
            server.server_close()
            thread.join()


if __name__ == "__main__":
    unittest.main()
