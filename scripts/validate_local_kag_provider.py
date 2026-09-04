#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
KAG_ROOT = REPO_ROOT / "kag"
REPO_NAME = "Tree-of-Sophia"
REQUIRED_RECORD_CLASSES = {"node", "edge", "index", "projection", "receipt"}
RECORD_DIRS = {
    "nodes": "node",
    "edges": "edge",
    "indexes": "index",
    "projections": "projection",
    "receipts": "receipt",
}
REQUIRED_RECORD_FIELDS = {
    "schema_version",
    "repo",
    "local_id",
    "record_class",
    "source_refs",
    "source_owner",
    "provenance_mode",
    "derived_method",
    "generated_or_authored",
    "status",
    "owner_return_route",
    "freshness",
    "builder",
    "validator",
    "storage_posture",
    "consumer_route",
}
REPO_LOCAL_FAMILY_MANIFEST = Path("kag/indexes/index_family.manifest.json")
REPO_LOCAL_BUDGET_RECEIPT_ROOT = Path("kag/receipts/index_family_budget")
KAG_SURFACE_FREEZE_MANIFEST = Path("kag/indexes/hot_profile.json")


class ValidationError(RuntimeError):
    pass


def fail(message: str) -> None:
    raise ValidationError(message)


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"missing required file: {path.relative_to(REPO_ROOT).as_posix()}")
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path.relative_to(REPO_ROOT).as_posix()}: {exc}")
    if not isinstance(payload, dict):
        fail(f"{path.relative_to(REPO_ROOT).as_posix()} must be a JSON object")
    return payload


def source_bytes(relative_path: Path, path: Path) -> bytes:
    try:
        return subprocess.run(
            ("git", "show", f":{relative_path.as_posix()}"),
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
        ).stdout
    except (subprocess.CalledProcessError, FileNotFoundError):
        return path.read_bytes()


def sha256_source(relative_path: Path, path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(source_bytes(relative_path, path))
    return digest.hexdigest()


def repo_path(path_text: str, *, label: str) -> Path:
    relative_path = Path(path_text)
    if relative_path.is_absolute() or ".." in relative_path.parts:
        fail(f"{label} must stay inside {REPO_NAME}: {path_text}")
    absolute_path = (REPO_ROOT / relative_path).resolve()
    try:
        absolute_path.relative_to(REPO_ROOT.resolve())
    except ValueError:
        fail(f"{label} must stay inside {REPO_NAME}: {path_text}")
    return absolute_path


def source_refs_in(payload: Any):
    if isinstance(payload, dict):
        refs = payload.get("source_refs")
        if isinstance(refs, list):
            for item in refs:
                if isinstance(item, dict):
                    yield item
        surfaces = payload.get("source_surfaces")
        if isinstance(surfaces, list):
            for item in surfaces:
                if isinstance(item, dict):
                    yield item
        for value in payload.values():
            yield from source_refs_in(value)
    elif isinstance(payload, list):
        for item in payload:
            yield from source_refs_in(item)


def validate_source_refs(payload: Any, *, label: str) -> None:
    refs = list(source_refs_in(payload))
    if not refs:
        fail(f"{label} must keep source refs")
    for ref in refs:
        if ref.get("repo") != REPO_NAME:
            fail(f"{label} source ref must stay inside {REPO_NAME}")
        source_path = ref.get("path")
        if not isinstance(source_path, str) or not source_path:
            fail(f"{label} source ref must keep a path")
        if not repo_path(source_path, label=f"{label} source ref").is_file():
            fail(f"{label} source ref is missing: {source_path}")


def validate_manifest() -> dict[str, Any]:
    manifest = read_json(KAG_ROOT / "manifest.json")
    if manifest.get("schema_version") != "aoa-local-kag-manifest-v1":
        fail("kag/manifest.json schema_version is invalid")
    if manifest.get("repo") != REPO_NAME:
        fail("kag/manifest.json repo is invalid")
    if manifest.get("owner_surface") != "kag/AGENTS.md":
        fail("kag/manifest.json owner_surface must be kag/AGENTS.md")
    if set(manifest.get("record_classes", [])) != REQUIRED_RECORD_CLASSES:
        fail("kag/manifest.json must name every local KAG record class")
    routes = {
        route.get("route")
        for route in manifest.get("validation_routes", [])
        if isinstance(route, dict)
    }
    if "scripts/validate_local_kag_provider.py" not in routes:
        fail("kag/manifest.json must route through scripts/validate_local_kag_provider.py")
    validate_source_refs(manifest, label="kag/manifest.json")
    return manifest


def validate_records() -> dict[str, list[dict[str, Any]]]:
    groups: dict[str, list[dict[str, Any]]] = {}
    for directory_name, record_class in RECORD_DIRS.items():
        directory = KAG_ROOT / directory_name
        if not directory.is_dir():
            fail(f"kag/{directory_name}/ must exist")
        paths = sorted(directory.rglob("*.json"))
        if not paths:
            fail(f"kag/{directory_name}/ must contain JSON records")
        records: list[dict[str, Any]] = []
        for path in paths:
            relative = path.relative_to(REPO_ROOT)
            relative_path = relative.as_posix()
            if (
                relative in {REPO_LOCAL_FAMILY_MANIFEST, KAG_SURFACE_FREEZE_MANIFEST}
                or REPO_LOCAL_BUDGET_RECEIPT_ROOT in relative.parents
            ):
                continue
            record = read_json(path)
            missing = REQUIRED_RECORD_FIELDS - set(record)
            if missing:
                fail(f"{relative_path} missing fields: {', '.join(sorted(missing))}")
            if record.get("schema_version") != "aoa-local-kag-record-v1":
                fail(f"{relative_path} schema_version is invalid")
            if record.get("repo") != REPO_NAME:
                fail(f"{relative_path} repo is invalid")
            if record.get("source_owner") != REPO_NAME:
                fail(f"{relative_path} source_owner is invalid")
            if record.get("record_class") != record_class:
                fail(f"{relative_path} record_class must be {record_class}")
            if not isinstance(record.get("local_id"), str) or not record["local_id"]:
                fail(f"{relative_path} must keep local_id")
            validate_source_refs(record, label=relative_path)
            records.append(record)
        groups[directory_name] = records
    return groups


def validate_repo_local_family() -> None:
    payload = read_json(REPO_ROOT / REPO_LOCAL_FAMILY_MANIFEST)
    label = REPO_LOCAL_FAMILY_MANIFEST.as_posix()
    if payload.get("schema_version") != "aoa-repo-local-kag-family-manifest-v3":
        fail(f"{label} schema_version is invalid")
    repo = payload.get("repo")
    if not isinstance(repo, dict) or repo.get("name") != REPO_NAME:
        fail(f"{label} repo.name is invalid")
    shards = payload.get("shards")
    if not isinstance(shards, list) or not shards:
        fail(f"{label} must keep shards")
    source_records = 0
    for shard_index, shard in enumerate(shards):
        if not isinstance(shard, dict):
            fail(f"{label} shard {shard_index} must be an object")
        shard_path_text = shard.get("path")
        if not isinstance(shard_path_text, str) or not shard_path_text:
            fail(f"{label} shard {shard_index} must keep path")
        shard_relative = Path(shard_path_text)
        if shard_relative.parts[:3] != ("kag", "indexes", "shards"):
            fail(f"{label} shard {shard_index} must stay under kag/indexes/shards")
        shard_path = repo_path(shard_path_text, label=f"{label} shard path")
        shard_bytes = source_bytes(shard_relative, shard_path)
        if shard.get("bytes") != len(shard_bytes):
            fail(f"{label} bytes drifted for {shard_path_text}")
        expected_digest = "sha256:" + hashlib.sha256(shard_bytes).hexdigest()
        if shard.get("digest") != expected_digest:
            fail(f"{label} digest drifted for {shard_path_text}")
        lines = shard_bytes.splitlines()
        if shard.get("records") != len(lines):
            fail(f"{label} record count drifted for {shard_path_text}")
        if shard.get("kind") != "source":
            continue
        source_records += len(lines)
        for line_index, line in enumerate(lines, start=1):
            try:
                record = json.loads(line)
            except json.JSONDecodeError as exc:
                fail(f"{shard_path_text}:{line_index} is invalid JSON: {exc}")
            if not isinstance(record, dict) or record.get("_kind") != "source":
                fail(f"{shard_path_text}:{line_index} must be a source record")
            identity = record.get("identity")
            if not isinstance(identity, dict) or identity.get("repo") != REPO_NAME:
                fail(f"{shard_path_text}:{line_index} identity.repo is invalid")
            source_path = identity.get("path")
            if not isinstance(source_path, str) or not source_path:
                fail(f"{shard_path_text}:{line_index} must keep identity.path")
            if Path(source_path) == REPO_LOCAL_FAMILY_MANIFEST:
                fail(f"{label} must not index itself")
            absolute_path = repo_path(source_path, label=f"{label} record path")
            if not absolute_path.is_file():
                fail(f"{label} record path is missing: {source_path}")
            expected_hash = sha256_source(Path(source_path), absolute_path)
            if identity.get("content_hash") != expected_hash:
                fail(f"{label} content_hash drifted for {source_path}")
            signs = record.get("signs")
            if isinstance(signs, dict) and signs.get("digest") != expected_hash:
                fail(f"{label} signs.digest drifted for {source_path}")
            for ref in source_refs_in(record):
                if ref.get("repo") != REPO_NAME:
                    fail(f"{label} source ref must stay inside {REPO_NAME}")
                ref_path = ref.get("path")
                if not isinstance(ref_path, str) or not ref_path:
                    fail(f"{label} source ref must keep a path")
                if not repo_path(ref_path, label=f"{label} source ref").is_file():
                    fail(f"{label} source ref is missing: {ref_path}")
    summary = payload.get("summary")
    if not isinstance(summary, dict) or summary.get("source_records") != source_records:
        fail(f"{label} summary.source_records must match source shards")


def validate_kag_surface_freeze() -> None:
    freeze = read_json(REPO_ROOT / KAG_SURFACE_FREEZE_MANIFEST)
    label = KAG_SURFACE_FREEZE_MANIFEST.as_posix()
    if freeze.get("schema_version") != "tos_kag_surface_freeze_v1":
        fail(f"{label} schema_version is invalid")
    if freeze.get("owner_repo") != REPO_NAME:
        fail(f"{label} owner_repo is invalid")
    if freeze.get("state") != "frozen":
        fail(f"{label} must remain frozen until an explicit ToS operator command")
    surface = freeze.get("surface")
    if not isinstance(surface, dict) or surface.get("manifest") != REPO_LOCAL_FAMILY_MANIFEST.as_posix():
        fail(f"{label} must bind the repo-local KAG family manifest")
    manifest = read_json(REPO_ROOT / REPO_LOCAL_FAMILY_MANIFEST)
    identity = manifest.get("family_identity")
    if not isinstance(identity, dict):
        fail(f"{REPO_LOCAL_FAMILY_MANIFEST.as_posix()} family_identity is missing")
    if surface.get("family_digest") != identity.get("content_digest"):
        fail(f"{label} family digest drifted; explicit unfreeze is required before regeneration")
    if surface.get("source_snapshot") != identity.get("source_snapshot"):
        fail(f"{label} source snapshot drifted; explicit unfreeze is required before regeneration")
    unfreeze = freeze.get("unfreeze")
    if not isinstance(unfreeze, dict) or unfreeze.get("requires") != "explicit ToS operator command":
        fail(f"{label} must keep an explicit ToS operator unfreeze route")


def validate_links(groups: dict[str, list[dict[str, Any]]]) -> None:
    all_records = [record for records in groups.values() for record in records]
    ids = [record["local_id"] for record in all_records]
    if len(ids) != len(set(ids)):
        fail("local KAG record ids must be unique")
    id_set = set(ids)
    node_ids = {record["local_id"] for record in groups["nodes"]}

    for edge in groups["edges"]:
        for key in ("from_id", "to_id"):
            if edge.get(key) not in node_ids:
                fail(f"{edge['local_id']} {key} must point to a local node")
        if not edge.get("edge_trace"):
            fail(f"{edge['local_id']} must keep edge_trace")

    for group_name in ("indexes", "projections"):
        for record in groups[group_name]:
            source_ids = record.get("source_record_ids")
            if not isinstance(source_ids, list) or not source_ids:
                fail(f"{record['local_id']} must keep source_record_ids")
            missing = sorted(record_id for record_id in source_ids if record_id not in id_set)
            if missing:
                fail(f"{record['local_id']} references unknown records: {', '.join(missing)}")

    for receipt in groups["receipts"]:
        fallback_route = receipt.get("fallback_route")
        if not isinstance(fallback_route, str) or not fallback_route:
            fail(f"{receipt['local_id']} must keep fallback_route")
        if not repo_path(fallback_route, label=f"{receipt['local_id']} fallback_route").exists():
            fail(f"{receipt['local_id']} fallback_route is missing: {fallback_route}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate the ToS-local KAG provider or its frozen surface binding.")
    parser.add_argument(
        "--freeze-only",
        action="store_true",
        help="check only the exact frozen family binding; skip provider record and freshness validation",
    )
    args = parser.parse_args(argv)
    try:
        if not args.freeze_only:
            validate_manifest()
            groups = validate_records()
            validate_links(groups)
            validate_repo_local_family()
        validate_kag_surface_freeze()
    except ValidationError as exc:
        print(f"[error] {exc}")
        return 1
    if args.freeze_only:
        print("[paused] validated frozen ToS KAG surface binding")
    else:
        print("[ok] validated Tree-of-Sophia local KAG provider")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
