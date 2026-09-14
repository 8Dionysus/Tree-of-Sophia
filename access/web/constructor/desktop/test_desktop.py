"""Behavior checks: private HTTP boundaries and desktop startup ownership, no GUI."""
from __future__ import annotations

import contextlib
import http.client
import io
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

import install
import launcher
import server


class DesktopTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="tos-desktop-check-", dir=os.environ.get("XDG_RUNTIME_DIR"))
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        runtime_env = patch.dict(os.environ, {"XDG_RUNTIME_DIR": str(self.root)})
        runtime_env.start(); self.addCleanup(runtime_env.stop)
        self.release = self.root / "release"; self.release.mkdir()
        (self.release / "constructor.html").write_text('<script src="assets/app.js"></script>')
        (self.release / "library.json").write_text('{"private_source": true}')
        (self.release / "assets").mkdir()
        (self.release / "assets" / "app.js").write_text("document.title='Sophia';")
        (self.release / "receipt.json").write_text('{"private": "do not serve"}')
        self.digest = server.release_digest(self.release)
        self.config = {"release": str(self.release), "release_digest": self.digest, "port": 44339,
                       "runtime": str(self.root / "runtime"), "worker_unit": "tos-sophia-demo-http.service",
                       "service_unit": "tos-sophia-demo.service"}

    def start_http(self):
        http = server.make_server(self.release, 0, self.digest)
        thread = threading.Thread(target=http.serve_forever, kwargs={"poll_interval": .02}, daemon=True); thread.start()
        self.addCleanup(http.server_close); self.addCleanup(http.shutdown)
        self.config["port"] = http.server_port
        return http

    def get(self, route, headers=None):
        connection = http.client.HTTPConnection("127.0.0.1", self.config["port"], timeout=2)
        try:
            connection.request("GET", route, headers=headers or {})
            response = connection.getresponse()
            return response.status, dict(response.getheaders()), response.read()
        finally:
            connection.close()

    def test_real_server_only_serves_allowlisted_release_and_identity(self):
        self.start_http()
        for route in ("/", "/constructor.html", "/library.json", "/assets/app.js"):
            status, headers, _ = self.get(route)
            self.assertEqual(status, 200)
            self.assertEqual(headers["X-Content-Type-Options"], "nosniff")
        for route in ("/receipt.json", "/assets/../receipt.json", "/assets/%2e%2e/receipt.json", "/assets/", "/assets/%00.js"):
            with self.subTest(route=route):
                self.assertEqual(self.get(route)[0], 404)
        status, _, data = self.get(server.IDENTITY_ROUTE)
        self.assertEqual(status, 200)
        identity = json.loads(data)
        self.assertEqual((identity["release"], identity["release_digest"], identity["pid"]),
                         (str(self.release), self.digest, os.getpid()))
        self.assertEqual(self.get(server.IDENTITY_ROUTE, {"Host": "unrelated.example"})[0], 403)

    def test_server_survives_closed_launcher_output_pipes(self):
        probe = socket.socket(); probe.bind(("127.0.0.1", 0)); port = probe.getsockname()[1]; probe.close()
        log = self.root / "server.log"
        process = subprocess.Popen([sys.executable, server.__file__, "--release", str(self.release), "--port", str(port),
            "--release-digest", self.digest, "--log", str(log)], stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        process.stdout.close(); process.stderr.close()
        self.config["port"] = port
        try:
            deadline = time.monotonic() + 3
            while True:
                try:
                    status, _, data = self.get(server.IDENTITY_ROUTE)
                    self.assertEqual(status, 200)
                    self.assertEqual(json.loads(data)["pid"], process.pid)
                    break
                except ConnectionRefusedError:
                    if process.poll() is not None or time.monotonic() > deadline:
                        self.fail(log.read_text() if log.exists() else "HTTP child never became ready")
                    time.sleep(.02)
            self.assertIsNone(process.poll())
            self.assertEqual(self.get("/constructor.html")[0], 200)
        finally:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=3)

    def test_symlink_asset_is_not_published(self):
        (self.release / "assets" / "secret").symlink_to(self.release / "receipt.json")
        with self.assertRaisesRegex(ValueError, "symlink"):
            server.make_server(self.release, 0, self.digest)

    def test_changed_release_fails_before_binding_or_systemctl(self):
        (self.release / "assets" / "app.js").write_text("changed")
        with self.assertRaisesRegex(ValueError, "differs"):
            server.make_server(self.release, 0, self.digest)
        with patch.object(launcher.subprocess, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "changed"):
                launcher.ensure_server(self.config)
            run.assert_not_called()

    def test_plausible_identity_from_unowned_service_is_rejected(self):
        self.start_http()
        with patch.object(launcher, "owns_worker", return_value=False), patch.object(launcher.subprocess, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "systemd worker"):
                launcher.ensure_server(self.config, timeout=.5)
            run.assert_not_called()

    def test_owned_ready_server_is_reused_without_start(self):
        self.start_http()
        with patch.dict(os.environ, {"XDG_RUNTIME_DIR": str(self.root)}), patch.object(launcher, "owns_worker", return_value=True), patch.object(launcher.subprocess, "run") as run:
            self.assertEqual(launcher.ensure_server(self.config)["pid"], os.getpid())
            run.assert_not_called()

    def test_concurrent_ensure_only_starts_once(self):
        listener = socket.socket(); listener.bind(("127.0.0.1", 0)); port = listener.getsockname()[1]; listener.close()
        self.config["port"] = port
        ready = threading.Event(); calls = []
        result = {"pid": 123, "app_id": server.APP_ID}
        def start(command, **kwargs):
            calls.append(command); ready.set(); return subprocess.CompletedProcess(command, 0, "", "")
        with patch.dict(os.environ, {"XDG_RUNTIME_DIR": str(self.root)}), patch.object(launcher, "identity", side_effect=lambda config: result if ready.is_set() else None), patch.object(launcher.subprocess, "run", side_effect=start):
            errors = []
            def ensure():
                try:
                    launcher.ensure_server(self.config, timeout=2)
                except Exception as exc:
                    errors.append(exc)
            threads = [threading.Thread(target=ensure) for _ in range(3)]
            for thread in threads: thread.start()
            for thread in threads: thread.join(3)
            self.assertFalse(errors)
            self.assertTrue(all(not thread.is_alive() for thread in threads))
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][-1], self.config["service_unit"])

    def test_stopping_never_targets_unowned_active_worker(self):
        with patch.object(launcher, "owns_worker", return_value=False), patch.object(launcher, "service_properties", return_value={"ActiveState": "active"}), patch.object(launcher.subprocess, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "Refusing"):
                launcher.stop_worker(self.config)
            run.assert_not_called()

    def make_plan(self):
        args = install.parse_args(["--release", str(self.release), "--application-root", str(self.root / "installed"),
            "--config", str(self.root / "config.json"), "--bin-dir", str(self.root / "bin"),
            "--applications-dir", str(self.root / "applications"), "--systemd-dir", str(self.root / "units"),
            "--runtime-root", str(self.root / "runtime"), "--cache-root", str(self.root / "cache")])
        return install.make_plan(args)

    def test_rendered_install_plan_is_private_and_on_demand(self):
        plan = self.make_plan()
        self.assertFalse((self.root / "installed").exists())
        self.assertFalse((self.root / "runtime").exists())
        config = plan["config"]
        self.assertNotEqual(config["profile"], config["cache"])
        self.assertEqual(config["release"], str(self.release))
        service_text = plan["files"][plan["service"]][0].decode()
        self.assertNotIn("WantedBy=", service_text)
        self.assertIn("--run-server", service_text)
        self.assertIn("--stop-worker", service_text)
        file = self.root / "tos-sophia-demo.desktop"
        file.write_bytes(plan["files"][plan["desktop"]][0])
        validator = shutil.which("desktop-file-validate")
        if validator:
            subprocess.run([validator, str(file)], check=True, capture_output=True)
            if shutil.which("systemd-analyze"):
                for target, (content, mode) in plan["files"].items():
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(content); target.chmod(mode)
                subprocess.run(["systemd-analyze", "--user", "verify", "--man=no", str(plan["service"])], check=True, capture_output=True)
        else:
            self.skipTest("desktop-file-validate is unavailable")

    def test_launcher_dry_run_never_starts_or_creates_profile(self):
        plan = self.make_plan()
        config = self.root / "config.json"; config.write_text(json.dumps(plan["config"]))
        with patch.object(launcher.subprocess, "run") as run, contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(launcher.main(["--config", str(config), "--dry-run"]), 0)
            run.assert_not_called()
        self.assertFalse(json.loads(out.getvalue())["starts"])
        self.assertFalse((self.root / "runtime").exists())
        browser = launcher.browser_command(plan["config"])
        self.assertNotIn("--no-sandbox", browser)
        self.assertIn("--disk-cache-size=33554432", browser)


if __name__ == "__main__":
    unittest.main()
