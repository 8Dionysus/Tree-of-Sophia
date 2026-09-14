#!/usr/bin/env python3
"""Launch the private Sophia demo without reusing another browser profile."""
from __future__ import annotations

import argparse
import contextlib
import fcntl
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time

from server import APP_ID, IDENTITY_ROUTE, release_digest

DEFAULT_CONFIG = Path.home() / ".config" / APP_ID / "config.json"
CONFIG_KEYS = {"schema", "release", "release_digest", "port", "browser", "profile", "cache", "runtime", "launcher", "server", "resource", "service_unit", "worker_unit"}


def load_config(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    if set(data) != CONFIG_KEYS or data["schema"] != "tos_sophia_demo_desktop_v1":
        raise ValueError("Unsupported Sophia desktop configuration")
    for name in ("release", "browser", "profile", "cache", "runtime", "launcher", "server", "resource"):
        if not isinstance(data[name], str) or not Path(data[name]).is_absolute() or any(c in data[name] for c in "\n\r\x00"):
            raise ValueError(f"Configuration path must be absolute: {name}")
    if data["port"] != 44339:
        raise ValueError("This installed desktop entry owns only loopback port 44339")
    if data["service_unit"] != APP_ID + ".service" or data["worker_unit"] != APP_ID + "-http.service":
        raise ValueError("Unexpected user-service owner identity")
    if data["profile"] == data["cache"] or Path(data["profile"]).is_relative_to(Path.home() / ".config" / "chromium"):
        raise ValueError("The app requires its own direct profile and cache directories")
    if len(data["release_digest"]) != 64 or any(c not in "0123456789abcdef" for c in data["release_digest"]):
        raise ValueError("Invalid release digest")
    data["config"] = str(path.resolve())
    return data


def worker_command(config: dict) -> list[str]:
    return ["/usr/bin/python3", config["server"], "--release", config["release"], "--port", str(config["port"]),
            "--release-digest", config["release_digest"], "--log", str(Path(config["runtime"]) / "server.log")]


def resource_command(config: dict, command: list[str], *, browser=False) -> list[str]:
    key = "desktop-browser" if browser else "desktop-http"
    unit = f"{APP_ID}-window-{os.getpid()}" if browser else config["worker_unit"].removesuffix(".service")
    return [config["resource"], "resource", "launch", "--class", "medium" if browser else "light", "--kind", "generic",
            "--activity", "foreground", "--latency", "interactive", "--unit", unit, "--timeout", "0",
            "--memory-demand-mib", "768" if browser else "64", "--demand-owner", "tree-of-sophia",
            "--demand-key", key, "--estimate-source", "owner-declared-desktop-startup", "--estimate-confidence", "conservative",
            "--no-same-dir", "--", *command]


def service_properties(unit: str) -> dict[str, str]:
    result = subprocess.run(["systemctl", "--user", "show", unit, "--property=ActiveState,SubState,MainPID,Result"],
                            text=True, capture_output=True, timeout=5, check=False)
    return dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)


def owns_worker(config: dict, pid: int | None = None) -> bool:
    properties = service_properties(config["worker_unit"])
    worker_pid = int(properties.get("MainPID", "0") or 0)
    if worker_pid <= 0 or (pid is not None and worker_pid != pid):
        return False
    try:
        actual = Path(f"/proc/{worker_pid}/cmdline").read_bytes().rstrip(b"\0").split(b"\0")
        return [os.fsdecode(part) for part in actual] == worker_command(config)
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        return False


def identity(config: dict) -> dict | None:
    # Direct HTTPConnection deliberately ignores proxy environment variables.
    connection = http.client.HTTPConnection("127.0.0.1", config["port"], timeout=1.0)
    try:
        connection.request("GET", IDENTITY_ROUTE)
        response = connection.getresponse()
        if response.status != 200:
            raise RuntimeError(f"Port {config['port']} answers HTTP {response.status}, without the expected demo identity")
        payload = response.read(16_385)
        if len(payload) > 16_384:
            raise RuntimeError("Unexpectedly large identity response")
        data = json.loads(payload)
        expected = {"schema": "tos_demo_server_identity_v1", "app_id": APP_ID,
                    "release": str(Path(config["release"]).resolve()), "release_digest": config["release_digest"]}
        if not isinstance(data, dict) or any(data.get(k) != v for k, v in expected.items()) or type(data.get("pid")) is not int:
            raise RuntimeError("Port 44339 serves a different application or release; nothing was stopped")
        if not owns_worker(config, data["pid"]):
            raise RuntimeError("Demo response does not belong to the expected systemd worker; nothing was stopped")
        return data
    except ConnectionRefusedError:
        return None
    except (json.JSONDecodeError, http.client.HTTPException, socket.timeout) as exc:
        raise RuntimeError(f"Port 44339 is occupied but has no verified demo response: {exc}") from exc
    finally:
        connection.close()


@contextlib.contextmanager
def launch_lock(timeout: float):
    base = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")) / APP_ID
    base.mkdir(mode=0o700, exist_ok=True)
    fd = os.open(base / "launch.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    deadline = time.monotonic() + timeout
    try:
        while True:
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB); break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise RuntimeError("Another Sophia launch is still checking the server; try again shortly")
                time.sleep(.1)
        yield
    finally:
        os.close(fd)


def log_tail(config: dict) -> str:
    path = Path(config["runtime"]) / "server.log"
    try:
        with path.open("rb") as stream:
            stream.seek(0, os.SEEK_END); size = stream.tell(); stream.seek(max(0, size - 2500))
            return stream.read().decode("utf-8", errors="replace")[-1500:]
    except OSError:
        return f"See journalctl --user -u {config['service_unit']} -n 30"


def ensure_server(config: dict, timeout=45.0) -> dict:
    if release_digest(Path(config["release"])) != config["release_digest"]:
        raise RuntimeError("Installed release files changed. Select a reviewed release before starting the app")
    with launch_lock(timeout):
        found = identity(config)
        if found:
            return found
        # Binding is only a preflight; the worker's bind and identity check handle the race.
        with socket.socket() as probe:
            probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            try:
                probe.bind(("127.0.0.1", config["port"]))
            except OSError as exc:
                raise RuntimeError("Port 44339 is occupied; the existing process was left untouched") from exc
        start = subprocess.run(["systemctl", "--user", "start", config["service_unit"]],
                               stdin=subprocess.DEVNULL, text=True, capture_output=True, timeout=8, check=False)
        if start.returncode:
            raise RuntimeError(f"Cannot start the installed demo service: {start.stderr.strip()}")
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            found = identity(config)
            if found:
                return found
            state = service_properties(config["service_unit"])
            if state.get("ActiveState") in {"failed", "inactive"}:
                raise RuntimeError(f"Demo service did not start ({state.get('Result', 'unknown')}):\n{log_tail(config)}")
            time.sleep(.2)
        raise RuntimeError(f"No verified demo response after {timeout:g}s. The service was not killed:\n{log_tail(config)}")


def stop_worker(config: dict) -> int:
    if not owns_worker(config):
        state = service_properties(config["worker_unit"])
        if state.get("ActiveState") in {"active", "activating"}:
            raise RuntimeError("Refusing to stop a worker whose exact command is not owned by this configuration")
        return 0
    return subprocess.run(["systemctl", "--user", "stop", config["worker_unit"]],
                          stdin=subprocess.DEVNULL, timeout=12, check=False).returncode


def run_server(config: dict) -> int:
    runtime = Path(config["runtime"]); runtime.mkdir(parents=True, mode=0o700, exist_ok=True)
    with (runtime / "server.log").open("ab", buffering=0) as log:
        # This process belongs to the installed on-demand user service. It waits
        # for resource launch for the complete HTTP-worker lifetime.
        result = subprocess.run(resource_command(config, worker_command(config)), stdin=subprocess.DEVNULL,
                                stdout=log, stderr=log, cwd=runtime, check=False)
    return result.returncode


def browser_command(config: dict) -> list[str]:
    return [config["browser"], f"--app=http://127.0.0.1:{config['port']}/constructor.html", f"--user-data-dir={config['profile']}",
            f"--disk-cache-dir={config['cache']}", "--disk-cache-size=33554432", f"--class={APP_ID}", "--no-first-run"]


def browser_worker(config: dict) -> None:
    for name in ("profile", "cache", "runtime"):
        Path(config[name]).mkdir(parents=True, mode=0o700, exist_ok=True)
    with open(os.devnull, "rb") as input_file, (Path(config["runtime"]) / "browser.log").open("ab", buffering=0) as log:
        os.dup2(input_file.fileno(), 0); os.dup2(log.fileno(), 1); os.dup2(log.fileno(), 2)
    os.execv(config["browser"], browser_command(config))


def report_error(message: str, config: dict | None, graphical: bool) -> None:
    print(message, file=sys.stderr)
    if config and graphical:
        runtime = Path(config["runtime"])
        try:
            runtime.mkdir(parents=True, mode=0o700, exist_ok=True)
            with (runtime / "launcher.log").open("a", encoding="utf-8") as stream:
                stream.write(time.strftime("%Y-%m-%d %H:%M:%S ") + message + "\n")
        except OSError:
            pass
    if graphical and shutil.which("zenity"):
        subprocess.run(["zenity", "--error", "--title=Древо Софии", "--text=" + message],
                       stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--ensure-server", action="store_true")
    mode.add_argument("--status", action="store_true")
    mode.add_argument("--dry-run", action="store_true")
    mode.add_argument("--run-server", action="store_true", help=argparse.SUPPRESS)
    mode.add_argument("--stop-worker", action="store_true", help=argparse.SUPPRESS)
    mode.add_argument("--browser-worker", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args(argv); config = None
    graphical = not any((args.ensure_server, args.status, args.dry_run, args.run_server, args.stop_worker, args.browser_worker))
    try:
        config = load_config(args.config)
        if args.dry_run:
            print(json.dumps({"url": f"http://127.0.0.1:{config['port']}/constructor.html", "release": config["release"],
                "service": config["service_unit"], "resource_server_argv": resource_command(config, worker_command(config)),
                "browser_argv": browser_command(config), "installs": False, "starts": False}, ensure_ascii=False, indent=2)); return 0
        if args.status:
            if release_digest(Path(config["release"])) != config["release_digest"]:
                raise RuntimeError("Installed release content changed; running identity is no longer sufficient")
            found = identity(config)
            print(json.dumps({"ready": bool(found), "identity": found, "service": service_properties(config["service_unit"])})); return 0 if found else 1
        if args.run_server:
            return run_server(config)
        if args.stop_worker:
            return stop_worker(config)
        if args.browser_worker:
            browser_worker(config); return 0
        found = ensure_server(config)
        if args.ensure_server:
            print(json.dumps({"ready": True, "identity": found})); return 0
        browser = ["/usr/bin/python3", config["launcher"], "--config", config["config"], "--browser-worker"]
        result = subprocess.run(resource_command(config, browser, browser=True), stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
        if result.returncode:
            raise RuntimeError(f"The separate browser window could not start (exit {result.returncode}):\n{(result.stdout + result.stderr)[-2000:]}\nSee browser.log")
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as exc:
        report_error(str(exc), config, graphical); return 2


if __name__ == "__main__":
    raise SystemExit(main())
