#!/usr/bin/env python3
"""Prepare or explicitly install the local Sophia desktop entry; never start it."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from server import APP_ID, release_digest

SOURCE = Path(__file__).resolve().parent
DEFAULT_ROOT = Path("/srv/abyss-machine/storage/artifacts/tos-sophia-demo")


def quoted_argument(value: str, *, desktop=False) -> str:
    if any(c in value for c in "\n\r\x00"):
        raise ValueError("Multiline command paths are not supported")
    if desktop:
        # Desktop Entry string decoding occurs before Exec argument decoding.
        value = value.replace("\\", "\\\\\\\\").replace('"', '\\\\"').replace("`", "\\\\`").replace("$", "\\\\$")
    else:
        value = value.replace("\\", "\\\\").replace('"', '\\"').replace("$", "$$")
    return '"' + value.replace("%", "%%") + '"'


def make_plan(args) -> dict:
    root = args.application_root.resolve()
    release = args.release.resolve(strict=True)
    config_path = args.config.resolve()
    payload = root / "desktop"
    launcher, server = payload / "launcher.py", payload / "server.py"
    wrapper = args.bin_dir.resolve() / APP_ID
    desktop = args.applications_dir.resolve() / (APP_ID + ".desktop")
    service = args.systemd_dir.resolve() / (APP_ID + ".service")
    browser = str(args.browser.resolve(strict=True))
    resource = shutil.which("abyss-machine")
    if not resource:
        raise ValueError("The abyss-machine resource owner is required; no ungated fallback is installed")
    config = {"schema": "tos_sophia_demo_desktop_v1", "release": str(release), "release_digest": release_digest(release),
        "port": 44339, "browser": browser, "profile": str(args.runtime_root.resolve() / "profile"),
        "cache": str(args.cache_root.resolve()), "runtime": str(args.runtime_root.resolve()),
        "launcher": str(launcher), "server": str(server), "resource": resource,
        "service_unit": APP_ID + ".service", "worker_unit": APP_ID + "-http.service"}
    invoke = ["/usr/bin/python3", str(launcher), "--config", str(config_path)]
    service_text = (SOURCE / "tos-sophia-demo.service.in").read_text().replace("@START@", " ".join(quoted_argument(s) for s in [*invoke, "--run-server"]))
    service_text = service_text.replace("@STOP@", " ".join(quoted_argument(s) for s in [*invoke, "--stop-worker"]))
    desktop_text = (SOURCE / "tos-sophia-demo.desktop.in").read_text().replace("@EXEC@", quoted_argument(str(wrapper), desktop=True))
    desktop_text = desktop_text.replace("@BROWSER@", browser).replace("@ICON@", str(payload / "icon.svg"))
    wrapper_text = "#!/usr/bin/python3\nimport os, sys\nos.execv('/usr/bin/python3', " + repr(invoke) + " + sys.argv[1:])\n"
    files = {
        launcher: ((SOURCE / "launcher.py").read_bytes(), 0o700),
        server: ((SOURCE / "server.py").read_bytes(), 0o700),
        payload / "icon.svg": ((SOURCE / "icon.svg").read_bytes(), 0o644),
        config_path: ((json.dumps(config, ensure_ascii=False, indent=2) + "\n").encode(), 0o600),
        wrapper: (wrapper_text.encode(), 0o700),
        desktop: (desktop_text.encode(), 0o644),
        service: (service_text.encode(), 0o644),
    }
    return {"files": files, "config": config, "config_path": config_path, "desktop": desktop, "service": service, "wrapper": wrapper}


def install(plan: dict, replace=False) -> None:
    files = plan["files"]
    for target, (content, _) in files.items():
        if target.is_symlink():
            raise ValueError(f"Refusing to replace a symlink: {target}")
        if target.exists() and target.read_bytes() != content and not replace:
            raise ValueError(f"Existing file differs; review it before using --replace: {target}")
    state = subprocess.run(["systemctl", "--user", "is-active", APP_ID + ".service"], capture_output=True, text=True, check=False)
    if state.returncode not in {0, 3, 4}:
        raise ValueError("Cannot verify user-service state: " + state.stderr.strip())
    if state.stdout.strip() in {"active", "activating", "reloading"}:
        raise ValueError("The app server is active. Stop its owned service explicitly before changing this installation")
    validator = shutil.which("desktop-file-validate")
    if not validator:
        raise ValueError("desktop-file-validate is required for installation")
    with tempfile.TemporaryDirectory(prefix="tos-desktop-", dir=os.environ.get("XDG_RUNTIME_DIR")) as temporary:
        staged = Path(temporary) / (APP_ID + ".desktop")
        staged.write_bytes(files[plan["desktop"]][0])
        subprocess.run([validator, str(staged)], check=True)
        # The service points to files that will exist only after installation;
        # command syntax is generated as separate quoted systemd arguments.
    for target, (content, mode) in files.items():
        target.parent.mkdir(parents=True, exist_ok=True)
        temporary = target.with_name(target.name + ".new")
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
        try:
            with os.fdopen(fd, "wb") as stream:
                stream.write(content); stream.flush(); os.fsync(stream.fileno())
            temporary.replace(target)
        finally:
            if temporary.exists():
                temporary.unlink()
    subprocess.run(["systemctl", "--user", "daemon-reload"], check=True)
    if shutil.which("update-desktop-database"):
        subprocess.run(["update-desktop-database", str(plan["desktop"].parent)], check=True)


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--application-root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--release", type=Path, default=DEFAULT_ROOT / "releases" / "meaning-v2")
    parser.add_argument("--config", type=Path, default=Path.home() / ".config" / APP_ID / "config.json")
    parser.add_argument("--bin-dir", type=Path, default=Path.home() / ".local" / "bin")
    parser.add_argument("--applications-dir", type=Path, default=Path.home() / ".local" / "share" / "applications")
    parser.add_argument("--systemd-dir", type=Path, default=Path.home() / ".config" / "systemd" / "user")
    parser.add_argument("--runtime-root", type=Path, default=Path("/srv/abyss-machine/runtimes/tos-sophia-demo"))
    parser.add_argument("--cache-root", type=Path, default=Path("/srv/abyss-machine/cache/tos-sophia-demo"))
    parser.add_argument("--browser", type=Path, default=Path("/usr/bin/chromium-browser"))
    parser.add_argument("--install", action="store_true", help="Write the reviewed files and reload user-unit definitions, without starting anything")
    parser.add_argument("--replace", action="store_true", help="Allow replacement of existing inactive installation files")
    return parser.parse_args(argv)


def main(argv=None) -> int:
    args = parse_args(argv)
    try:
        plan = make_plan(args)
        if args.install:
            install(plan, replace=args.replace)
        print(json.dumps({"mode": "installed" if args.install else "dry-run", "release": plan["config"]["release"],
            "release_digest": plan["config"]["release_digest"], "files": [{"path": str(path), "bytes": len(content), "mode": oct(mode)} for path, (content, mode) in plan["files"].items()],
            "profile": plan["config"]["profile"], "cache": plan["config"]["cache"],
            "starts_services": False, "opens_browser": False, "creates_profile_or_cache": False}, ensure_ascii=False, indent=2))
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        print(str(exc), file=__import__("sys").stderr); return 2


if __name__ == "__main__":
    raise SystemExit(main())
