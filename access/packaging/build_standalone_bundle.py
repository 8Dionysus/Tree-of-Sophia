#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
import zipfile
from pathlib import Path
from typing import Any

FIXED_ZIP_TIME = (2020, 1, 1, 0, 0, 0)
BLOCKED_HOST_MARKERS = (b"/srv/" + b"AbyssOS", b"/srv/" + b"abyss-machine")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def source_identity(repo_root: Path, explicit: str | None, allow_dirty: bool) -> tuple[str, bool]:
    if explicit and not (repo_root / ".git").exists():
        return explicit, False
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    resolved = result.stdout.strip()
    if result.returncode or not resolved:
        if explicit:
            return explicit, False
        raise RuntimeError("--source-ref is required when building outside a Git checkout")
    status = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    dirty = bool(status.stdout.strip())
    if dirty and not allow_dirty:
        raise RuntimeError("refusing to identify a dirty source only by HEAD; pass --allow-dirty for a fingerprinted development candidate")
    return explicit or resolved, dirty


def source_fingerprint(repo_root: Path, allowlist: dict[str, Any]) -> str:
    digest = hashlib.sha256()
    access_root = repo_root / "access"
    ignored_parts = {"node_modules", "__pycache__", ".pytest_cache", "runtime_data", "web_dist"}
    paths = [
        path
        for path in access_root.rglob("*")
        if path.is_file() and not (set(path.relative_to(access_root).parts) & ignored_parts)
    ]
    paths.extend(
        repo_root / str(item["source_path"])
        for item in allowlist.get("subjects", [])
        if (repo_root / str(item["source_path"])).is_file()
    )
    for path in sorted(set(paths), key=lambda item: item.relative_to(repo_root).as_posix()):
        relative = path.relative_to(repo_root).as_posix()
        digest.update(relative.encode("utf-8") + b"\0")
        digest.update(sha256_file(path).encode("ascii") + b"\n")
    return digest.hexdigest()


def _ignored(_: str, names: list[str]) -> set[str]:
    blocked = {"node_modules", "__pycache__", ".pytest_cache", "runtime_data", "web_dist"}
    return {name for name in names if name in blocked or name.endswith(".pyc")}


def _scan_portable_code(access_root: Path) -> None:
    checked_suffixes = {".py", ".json", ".toml", ".ts", ".js", ".md", ".html"}
    for path in sorted(access_root.rglob("*")):
        if not path.is_file() or path.suffix not in checked_suffixes or "runtime_data" in path.parts:
            continue
        payload = path.read_bytes()
        for marker in BLOCKED_HOST_MARKERS:
            if marker in payload:
                raise RuntimeError(f"hard-coded host path in bundle source: {path.relative_to(access_root)}")


def _write_deterministic_zip(stage_root: Path, output: Path) -> None:
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(item for item in stage_root.rglob("*") if item.is_file()):
            relative = path.relative_to(stage_root).as_posix()
            info = zipfile.ZipInfo(relative, FIXED_ZIP_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (0o755 if path.name.endswith(".py") else 0o644) << 16
            archive.writestr(info, path.read_bytes(), compresslevel=9)


def build_bundle(
    repo_root: Path,
    output: Path,
    explicit_source_ref: str | None = None,
    *,
    allow_dirty: bool = False,
) -> dict[str, Any]:
    access_root = repo_root / "access"
    allowlist_path = access_root / "contracts/runtime-data.v1.json"
    allowlist = json.loads(allowlist_path.read_text(encoding="utf-8"))
    resolved_ref, source_dirty = source_identity(repo_root, explicit_source_ref, allow_dirty)
    resolved_fingerprint = source_fingerprint(repo_root, allowlist)
    if output.exists():
        raise RuntimeError(f"refusing to overwrite existing bundle: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="tos-standalone-build-") as raw_temp:
        stage = Path(raw_temp) / "tree-of-sophia-standalone"
        staged_access = stage / "access"
        shutil.copytree(access_root, staged_access, ignore=_ignored)
        runtime_data = staged_access / "src/tos_access/runtime_data"
        runtime_data.mkdir(parents=True)
        shutil.copytree(staged_access / "contracts", runtime_data / "access/contracts")
        shutil.copytree(staged_access / "profiles", runtime_data / "access/profiles")
        web_dist = staged_access / "web/dist"
        if not (web_dist / "assets/tos-graph.js").is_file():
            raise RuntimeError("web assets are missing; run npm --prefix access/web run build")
        shutil.copytree(web_dist, staged_access / "src/tos_access/web_dist")

        subjects: list[dict[str, Any]] = []
        for item in allowlist.get("subjects", []):
            source_path = Path(str(item["source_path"]))
            source = repo_root / source_path
            if not source.is_file():
                if item.get("required"):
                    raise RuntimeError(f"missing required runtime subject: {source_path.as_posix()}")
                continue
            target = runtime_data / source_path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
            subjects.append(
                {
                    "subject_id": item["subject_id"],
                    "source_path": source_path.as_posix(),
                    "bundle_path": target.relative_to(stage).as_posix(),
                    "sha256": sha256_file(target),
                    "size_bytes": target.stat().st_size,
                    "required": bool(item.get("required")),
                }
            )

        manifest = {
            "schema_version": "tos_standalone_bundle_manifest_v1",
            "artifact_class": "unknown",
            "artifact_class_candidate": "portable_export",
            "artifact_policy_posture": "unknown_class_not_yet_admitted_by_abyss_machine",
            "artifact_id": "tree-of-sophia-standalone",
            "source_owner": "Tree-of-Sophia",
            "source_ref": resolved_ref,
            "source_dirty": source_dirty,
            "source_fingerprint": resolved_fingerprint,
            "source_fingerprint_scope": "access-source-plus-runtime-subjects-v1",
            "consumer_intent": "installer",
            "access_policy": "read-only-allowlisted-projections",
            "subjects": subjects,
            "controls": {
                "present": ["source-ref", "subject-sha256", "subject-size", "runtime-data-allowlist", "archive-smoke"],
                "deferred": ["signature", "sbom", "durable-registry", "consumer-trust-gate"],
            },
            "authority_limit": "A built bundle is a producer candidate, not publication, promotion, or consumer admission.",
        }
        rendered = json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
        (stage / "bundle.manifest.json").write_text(rendered, encoding="utf-8")
        (staged_access / "src/tos_access/bundle.manifest.json").write_text(rendered, encoding="utf-8")
        _scan_portable_code(staged_access)
        _write_deterministic_zip(stage, output)

    sidecar = output.with_suffix(output.suffix + ".manifest.json")
    sidecar_payload = {
        **manifest,
        "archive_sha256": sha256_file(output),
        "archive_size_bytes": output.stat().st_size,
    }
    sidecar.write_text(
        json.dumps(sidecar_payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return sidecar_payload


def main() -> None:
    parser = argparse.ArgumentParser(description="Build a portable Tree of Sophia standalone bundle")
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-ref")
    parser.add_argument("--allow-dirty", action="store_true")
    args = parser.parse_args()
    manifest = build_bundle(
        args.repo_root.resolve(),
        args.output.resolve(),
        args.source_ref,
        allow_dirty=args.allow_dirty,
    )
    print(
        json.dumps(
            {"ok": True, "bundle": args.output.resolve().as_posix(), "sha256": manifest["archive_sha256"]},
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
