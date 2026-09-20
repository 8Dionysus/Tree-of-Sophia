#!/usr/bin/env python3
"""Build the source-bound, software-only Tree of Sophia archive."""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

PACKAGING_ROOT = Path(__file__).resolve().parent
if PACKAGING_ROOT.as_posix() not in sys.path:
    sys.path.insert(0, PACKAGING_ROOT.as_posix())

from archive_common import (  # noqa: E402
    _write_deterministic_zip as _write_zip,
    sha256_file,
)

SOFTWARE_SOURCE_FILES = (
    "access/pyproject.toml",
    "access/README.md",
    "access/packaging/tos_build_backend.py",
)
TOS_SCHEMA_FILES = (
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/epistemic-evidence-projection.schema.json",
)
SOURCE_STATUS_PATHS = (
    "access/packaging/build_software_bundle.py",
    "access/packaging/archive_common.py",
    "access/pyproject.toml",
    "access/README.md",
    "access/packaging/tos_build_backend.py",
    ":(glob)access/src/tos_access/**/*.py",
    ":(exclude,glob)access/src/tos_access/**/runtime_data/**",
    ":(exclude,glob)access/src/tos_access/**/__pycache__/**",
    ":(glob)access/contracts/**/*.json",
    ":(glob)access/profiles/**/*.json",
    "access/web/src",
    "access/web/public",
    "access/web/index.html",
    ":(glob)access/web/package*.json",
    "access/web/tsconfig.json",
    "access/web/vite.config.ts",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/epistemic-evidence-projection.schema.json",
)
README = """# Tree of Sophia software package

Install the software from this archive:

```sh
pip install ./access
```

Select a compatible data release explicitly before running the reader:

```sh
export TOS_DATA_ROOT=/path/to/compatible-data
```

The software package contains no corpus or other production data. Data is
selected separately through `TOS_DATA_ROOT`.
"""


def build_software_bundle(
    repo_root: Path,
    output: Path,
    *,
    source_ref: str,
    allow_dirty: bool = False,
) -> dict:
    """Build a deterministic software archive from the exact software allowlist."""
    repo_root = Path(repo_root).resolve()
    output = Path(output).absolute()
    sidecar = output.with_suffix(output.suffix + ".manifest.json")
    if os.path.lexists(output):
        raise RuntimeError(f"refusing to overwrite existing bundle: {output}")
    if os.path.lexists(sidecar):
        raise RuntimeError(f"refusing to overwrite existing bundle manifest: {sidecar}")
    if not isinstance(source_ref, str) or not source_ref.strip():
        raise RuntimeError("source_ref must be a non-empty string")

    access_root = repo_root / "access"
    copy_items: list[tuple[Path, Path]] = []

    for relative in SOFTWARE_SOURCE_FILES:
        source = repo_root / relative
        if source.is_symlink() or not source.is_file():
            raise RuntimeError(f"missing required software source: {relative}")
        copy_items.append((source, Path(relative)))

    package_root = access_root / "src/tos_access"
    if package_root.is_symlink() or not package_root.is_dir():
        raise RuntimeError(f"missing software package source: {package_root.relative_to(repo_root)}")
    package_files = []
    for path in package_root.rglob("*"):
        relative = path.relative_to(package_root)
        if {"runtime_data", "__pycache__"}.intersection(relative.parts):
            continue
        if path.is_symlink():
            raise RuntimeError(f"software package source must not contain symlinks: {path.relative_to(repo_root)}")
        if path.is_file() and path.suffix == ".py":
            package_files.append(path)
    if not package_files:
        raise RuntimeError("software package source has no Python files")
    for path in sorted(package_files, key=lambda item: item.relative_to(repo_root).as_posix()):
        copy_items.append((path, path.relative_to(repo_root)))

    for directory_name in ("contracts", "profiles"):
        source_root = access_root / directory_name
        if source_root.is_symlink() or not source_root.is_dir():
            raise RuntimeError(f"missing software {directory_name} directory: {source_root.relative_to(repo_root)}")
        json_files = []
        for path in source_root.rglob("*"):
            if path.is_symlink():
                raise RuntimeError(f"software {directory_name} must not contain symlinks: {path.relative_to(repo_root)}")
            if path.is_file() and path.suffix == ".json":
                json_files.append(path)
        for path in sorted(json_files, key=lambda item: item.relative_to(repo_root).as_posix()):
            copy_items.append((path, path.relative_to(repo_root)))

    web_dist = access_root / "web/dist"
    if web_dist.is_symlink() or not web_dist.is_dir():
        raise RuntimeError("web assets are missing: access/web/dist")
    web_files = []
    for path in web_dist.rglob("*"):
        if path.is_symlink():
            raise RuntimeError(f"web assets must not contain symlinks: {path.relative_to(repo_root)}")
        if path.is_file():
            web_files.append(path)
        elif not path.is_dir():
            raise RuntimeError(f"web assets must be regular files: {path.relative_to(repo_root)}")
    required_web_asset = web_dist / "assets/tos-graph.js"
    if required_web_asset.is_symlink() or not required_web_asset.is_file():
        raise RuntimeError("web assets are missing: access/web/dist/assets/tos-graph.js")
    for relative in TOS_SCHEMA_FILES:
        source = repo_root / relative
        if source.is_symlink() or not source.is_file():
            raise RuntimeError(f"missing required ToS schema: {relative}")
        copy_items.append((source, Path("access/src/tos_access/runtime_data") / relative))

    archive_paths = [relative.as_posix() for _, relative in copy_items]
    if len(archive_paths) != len(set(archive_paths)):
        raise RuntimeError("software allowlist produced duplicate archive paths")

    try:
        git_probe = subprocess.run(
            ["git", "rev-parse", "--is-inside-work-tree"],
            cwd=repo_root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    except OSError:
        git_available = False
    else:
        git_available = git_probe.returncode == 0 and git_probe.stdout.strip() == "true"
    if git_available:
        head_result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        git_head = head_result.stdout.strip()
        if head_result.returncode != 0 or not git_head:
            raise RuntimeError("Git checkout has no readable HEAD")
        if source_ref != git_head:
            raise RuntimeError("source_ref must equal Git HEAD")
        status_result = subprocess.run(
            [
                "git",
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--",
                *SOURCE_STATUS_PATHS,
            ],
            cwd=repo_root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if status_result.returncode != 0:
            raise RuntimeError("unable to inspect source allowlist status")
        source_dirty = bool(status_result.stdout.strip())
    else:
        source_dirty = True

    if source_dirty and not allow_dirty:
        raise RuntimeError("refusing to build from a dirty or non-Git source; pass --allow-dirty")

    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="tos-software-build-", dir=output.parent) as raw_temp:
        temp_root = Path(raw_temp)
        stage = temp_root / "stage"
        stage.mkdir()
        for source, relative in copy_items:
            target = stage / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            with source.open("rb") as source_stream, target.open("xb") as target_stream:
                shutil.copyfileobj(source_stream, target_stream, length=1024 * 1024)

        (stage / "access/src/tos_access/runtime_data/access/contracts").mkdir(parents=True, exist_ok=True)
        (stage / "access/src/tos_access/runtime_data/access/profiles").mkdir(parents=True, exist_ok=True)
        for directory_name in ("contracts", "profiles"):
            staged_root = stage / "access" / directory_name
            for path in sorted(staged_root.rglob("*.json"), key=lambda item: item.relative_to(stage).as_posix()):
                relative = path.relative_to(stage / "access")
                target = stage / "access/src/tos_access/runtime_data/access" / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                with path.open("rb") as source_stream, target.open("xb") as target_stream:
                    shutil.copyfileobj(source_stream, target_stream, length=1024 * 1024)

        web_runtime = stage / "access/src/tos_access/web_dist"
        web_runtime.mkdir(parents=True, exist_ok=True)
        for path in sorted(web_files, key=lambda item: item.relative_to(web_dist).as_posix()):
            target = web_runtime / path.relative_to(web_dist)
            target.parent.mkdir(parents=True, exist_ok=True)
            with path.open("rb") as source_stream, target.open("xb") as target_stream:
                shutil.copyfileobj(source_stream, target_stream, length=1024 * 1024)

        (stage / "README.md").write_text(README, encoding="utf-8")
        members = []
        for path in sorted(stage.rglob("*"), key=lambda item: item.relative_to(stage).as_posix()):
            if not path.is_file():
                continue
            relative = path.relative_to(stage).as_posix()
            if relative == "software.manifest.json":
                continue
            members.append(
                {
                    "path": relative,
                    "sha256": sha256_file(path),
                    "size_bytes": path.stat().st_size,
                }
            )
        members.sort(key=lambda item: item["path"])
        manifest = {
            "schema_version": "tos_software_bundle_manifest_v1",
            "software_ref": source_ref,
            "source_dirty": source_dirty,
            "data_included": False,
            "members": members,
        }
        (stage / "software.manifest.json").write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        staged_output = temp_root / output.name
        _write_zip(stage, staged_output)
        sidecar_payload = {
            **manifest,
            "archive_sha256": sha256_file(staged_output),
            "archive_size_bytes": staged_output.stat().st_size,
        }
        staged_sidecar = temp_root / sidecar.name
        staged_sidecar.write_text(
            json.dumps(sidecar_payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        if os.path.lexists(output):
            raise RuntimeError(f"refusing to overwrite existing bundle: {output}")
        if os.path.lexists(sidecar):
            raise RuntimeError(f"refusing to overwrite existing bundle manifest: {sidecar}")
        try:
            os.link(staged_output, output)
            os.link(staged_sidecar, sidecar)
        except FileExistsError as exc:
            raise RuntimeError("refusing to overwrite output or sidecar") from exc

    return sidecar_payload


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Build a software-only Tree of Sophia source archive")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-ref", required=True)
    parser.add_argument("--allow-dirty", action="store_true")
    args = parser.parse_args()
    manifest = build_software_bundle(
        Path.cwd(),
        args.output,
        source_ref=args.source_ref,
        allow_dirty=args.allow_dirty,
    )
    print(
        json.dumps(
            {
                "ok": True,
                "bundle": Path(args.output).absolute().as_posix(),
                "sha256": manifest["archive_sha256"],
            },
            indent=2,
        )
    )
