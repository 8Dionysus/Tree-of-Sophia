"""Verify and install a software artifact without a production data snapshot."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import sys
import tempfile
import zipfile

SCHEMA = "tos_software_bundle_manifest_v1"
TOS_SCHEMAS = {
    "semantic-entity-type-registry.schema.json",
    "semantic-relation-type-registry.schema.json",
    "epistemic-evidence-projection.schema.json",
}


def digest(stream) -> str:
    value = hashlib.sha256()
    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
        value.update(chunk)
    return value.hexdigest()


def software_member(name: str) -> bool:
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts or "\\" in name or path.as_posix() != name or ":" in name:
        return False
    if name in {"README.md", "software.manifest.json", "access/README.md", "access/pyproject.toml", "access/packaging/tos_build_backend.py"}:
        return True
    runtime = "access/src/tos_access/runtime_data/"
    if name.startswith(runtime):
        subject = name[len(runtime):]
        if subject.startswith(("access/contracts/", "access/profiles/")):
            return path.suffix == ".json"
        return subject in {"ToS/contracts/" + schema for schema in TOS_SCHEMAS}
    if name.startswith("access/src/tos_access/web_dist/"):
        return True
    if name.startswith("access/src/tos_access/"):
        return path.suffix == ".py" and "__pycache__" not in path.parts
    return name.startswith(("access/contracts/", "access/profiles/")) and path.suffix == ".json"


def verify_archive(bundle: Path) -> dict:
    sidecar = bundle.with_suffix(bundle.suffix + ".manifest.json")
    external = json.loads(sidecar.read_text(encoding="utf-8"))
    with bundle.open("rb") as stream:
        if digest(stream) != external.get("archive_sha256"):
            raise RuntimeError("software archive digest mismatch")
    if bundle.stat().st_size != external.get("archive_size_bytes"):
        raise RuntimeError("software archive size mismatch")
    with zipfile.ZipFile(bundle) as archive:
        entries = archive.infolist()
        names = [item.filename for item in entries]
        if len(names) != len(set(names)):
            raise RuntimeError("duplicate software archive members")
        for item in entries:
            mode = stat.S_IFMT(item.external_attr >> 16)
            if not software_member(item.filename) or item.is_dir() or mode not in (0, stat.S_IFREG):
                raise RuntimeError(f"non-software or unsafe archive member: {item.filename}")
        manifest = json.loads(archive.read("software.manifest.json"))
        if manifest.get("schema_version") != SCHEMA or manifest.get("data_included") is not False:
            raise RuntimeError("unsupported software manifest")
        if not isinstance(manifest.get("software_ref"), str) or not manifest["software_ref"].strip() or not isinstance(manifest.get("source_dirty"), bool):
            raise RuntimeError("missing software source identity")
        if any(external.get(key) != value for key, value in manifest.items()):
            raise RuntimeError("software manifests disagree")
        members = manifest.get("members")
        if not isinstance(members, list):
            raise RuntimeError("software manifest members must be a list")
        paths = [item.get("path") for item in members if isinstance(item, dict)]
        if len(paths) != len(members) or len(paths) != len(set(paths)):
            raise RuntimeError("invalid or duplicate software manifest entries")
        if set(paths) != set(names) - {"software.manifest.json"}:
            raise RuntimeError("software manifest does not cover the exact archive")
        for item in members:
            info = archive.getinfo(item["path"])
            with archive.open(info) as stream:
                actual = digest(stream)
            if actual != item.get("sha256") or info.file_size != item.get("size_bytes"):
                raise RuntimeError(f"software member integrity mismatch: {item['path']}")
    return manifest


def installed_probe(bundle: Path) -> dict:
    """Build a real wheel and install it in a fresh, dependency-free venv."""
    env = {key: value for key, value in os.environ.items()
           if key != "PYTHONPATH" and not key.startswith(("TOS_", "AOA_"))}
    env["PIP_DISABLE_PIP_VERSION_CHECK"] = "1"
    with tempfile.TemporaryDirectory(prefix="tos-software-install-", dir=bundle.parent) as raw:
        root = Path(raw)
        source, outside, wheels, venv = (root / part for part in ("source", "outside", "wheels", "venv"))
        outside.mkdir()
        with zipfile.ZipFile(bundle) as archive:
            archive.extractall(source)

        def run(command):
            result = subprocess.run(command, cwd=outside, env=env, text=True, capture_output=True)
            if result.returncode:
                raise RuntimeError(f"software install probe failed: {result.stdout}\n{result.stderr}")
            return result.stdout

        run([sys.executable, "-m", "pip", "wheel", "--no-deps", "--no-build-isolation", "--no-cache-dir",
             "--wheel-dir", str(wheels), str(source / "access")])
        wheel_files = list(wheels.glob("*.whl"))
        if len(wheel_files) != 1:
            raise RuntimeError("software build did not produce exactly one wheel")
        run([sys.executable, "-m", "venv", str(venv)])
        python = venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        run([str(python), "-m", "pip", "install", "--no-deps", "--no-index", str(wheel_files[0])])
        result = run([str(python), "-c", """
import json
from tos_access.core import ToSAccessCore
from tos_access.locations import data_root, program_path, web_root
core = ToSAccessCore.discover()
contracts = core.knowledge_exploration_contracts()
assert contracts['request']['$schema']
assert program_path('ToS/contracts/semantic-entity-type-registry.schema.json').is_file()
assert web_root() is not None
assert not core.index_exists(), 'software wheel unexpectedly includes corpus'
assert not (data_root() / 'ToS/source-witnesses').exists()
print(json.dumps({'installed': True, 'api_contracts': True, 'web_assets': True, 'data_included': False}))
"""])
        return json.loads(result)


def validate(bundle: Path, *, install: bool = True) -> dict:
    manifest = verify_archive(bundle)
    result = {"schema_version": "tos_software_validation_v1", "ok": True,
              "software_ref": manifest["software_ref"], "source_dirty": manifest["source_dirty"],
              "data_validated": False, "archive_members": len(manifest["members"])}
    if install:
        result["installation"] = installed_probe(bundle)
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--integrity-only", action="store_true")
    args = parser.parse_args()
    print(json.dumps(validate(args.bundle.resolve(), install=not args.integrity_only), indent=2))


if __name__ == "__main__":
    main()
