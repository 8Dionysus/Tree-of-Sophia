#!/usr/bin/env python3
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
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
GENERATED_INDEX_NAMES = {
    "source_surface_index.json",
    "repo_artifact_index.json",
    "repo_anchor_index.json",
    "repo_entity_index.json",
    "repo_event_index.json",
    "repo_assertion_index.json",
    "repo_relation_index.json",
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
REPO_LOCAL_PROVIDER_PIN = Path("kag/provider-pin.json")
SEGMENTED_FAMILY_SCHEMA = "aoa-repo-local-kag-segmented-family-v1"
SEGMENTED_SCHEMA_REF = "aoa-kag:schemas/repo-local-kag-segmented-family.schema.json"
SEGMENTED_DECISION_REF = (
    "aoa-kag:docs/decisions/AOA-KAG-D-0040-bounded-segmented-kag-family.md"
)
SEGMENT_ROOT = Path("kag/indexes/segments")
MAX_RECORD_BYTES = 131_072
OWNER_HARD_BYTES_MAX = 48 * 1024 * 1024
SEGMENTED_DIGEST_RE = re.compile(r"^[a-f0-9]{64}$")
PROVIDER_REVISION_RE = re.compile(r"^[0-9a-f]{40}$")


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
                relative == REPO_LOCAL_FAMILY_MANIFEST
                or REPO_LOCAL_BUDGET_RECEIPT_ROOT in relative.parents
                or (
                    relative.parent == Path("kag/indexes")
                    and relative.name in GENERATED_INDEX_NAMES
                )
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
    schema_version = payload.get("schema_version")
    if schema_version == SEGMENTED_FAMILY_SCHEMA:
        validate_segmented_family(payload)
        return
    if schema_version != "aoa-repo-local-kag-family-manifest-v3":
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


def canonical_json_bytes(payload: object) -> bytes:
    return json.dumps(
        payload,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def segmented_manifest_digest(payload: dict[str, Any]) -> str:
    candidate = copy.deepcopy(payload)
    identity = candidate.get("family_identity")
    if not isinstance(identity, dict):
        fail("segmented family needs family_identity")
    identity["content_digest"] = "0" * 64
    return hashlib.sha256(canonical_json_bytes(candidate)).hexdigest()


def validate_segmented_pin() -> dict[str, Any]:
    label = REPO_LOCAL_PROVIDER_PIN.as_posix()
    pin = read_json(REPO_ROOT / REPO_LOCAL_PROVIDER_PIN)
    if pin.get("schema_version") != "tos-kag-provider-pin-v1":
        fail(f"{label} schema_version is invalid")
    if pin.get("consumer_repo") != REPO_NAME:
        fail(f"{label} consumer_repo is invalid")
    if pin.get("provider_repo") != "aoa-kag":
        fail(f"{label} provider_repo is invalid")
    revision = pin.get("provider_revision")
    if not isinstance(revision, str) or not PROVIDER_REVISION_RE.fullmatch(revision):
        fail(f"{label} provider_revision must be a full commit id")
    if pin.get("provider_action") != ".github/actions/repo-local-kag-index/action.yml":
        fail(f"{label} provider_action is invalid")
    if pin.get("family_schema") != SEGMENTED_FAMILY_SCHEMA:
        fail(f"{label} family_schema is invalid")
    if pin.get("schema_ref") != SEGMENTED_SCHEMA_REF:
        fail(f"{label} schema_ref is invalid")
    if pin.get("consumer_adapter") != "scripts/validate_local_kag_provider.py":
        fail(f"{label} consumer_adapter is invalid")
    migration = pin.get("migration")
    if not isinstance(migration, dict):
        fail(f"{label} migration is required")
    if migration.get("mode") != "explicit-provider-pin-dual-read":
        fail(f"{label} migration mode is invalid")
    if migration.get("rollback") != "retain-last-good-manifest-and-select-by-digest":
        fail(f"{label} migration rollback is invalid")
    if migration.get("decision_ref") != SEGMENTED_DECISION_REF:
        fail(f"{label} migration decision_ref is invalid")
    validate_source_refs(pin, label=label)
    return pin


def validate_segmented_family(payload: dict[str, Any]) -> None:
    """Validate a v1 segmented family without materialising its compatibility view."""
    label = REPO_LOCAL_FAMILY_MANIFEST.as_posix()
    if not isinstance(payload.get("repo"), dict) or payload["repo"].get("name") != REPO_NAME:
        fail(f"{label} repo.name is invalid")
    pin = validate_segmented_pin()
    if payload.get("schema_version") != pin.get("family_schema"):
        fail(f"{label} schema must match the source-owned provider pin")
    identity = payload.get("family_identity")
    if not isinstance(identity, dict):
        fail(f"{label} family_identity is required")
    digest = identity.get("content_digest")
    if not isinstance(digest, str) or not SEGMENTED_DIGEST_RE.fullmatch(digest):
        fail(f"{label} family_identity.content_digest is invalid")
    if digest != segmented_manifest_digest(payload):
        fail(f"{label} family_identity.content_digest drifted")
    if identity.get("schema_ref") != SEGMENTED_SCHEMA_REF:
        fail(f"{label} family_identity.schema_ref is invalid")
    source_snapshot = identity.get("source_snapshot")
    if not isinstance(source_snapshot, str) or not source_snapshot.startswith("sha256:"):
        fail(f"{label} family_identity.source_snapshot is invalid")
    producer = payload.get("producer_identity")
    if not isinstance(producer, dict) or producer.get("version") != "aoa-kag:segmented-family-producer-v1":
        fail(f"{label} producer_identity is invalid")
    if producer.get("route") != "aoa-kag:scripts/repo_local/segmented_family.py":
        fail(f"{label} producer_identity.route is invalid")
    candidate = payload.get("candidate_identity")
    if not isinstance(candidate, dict) or candidate.get("version") != "aoa-kag:segmented-family-candidate-v1":
        fail(f"{label} candidate_identity is invalid")
    migration = payload.get("migration")
    if not isinstance(migration, dict):
        fail(f"{label} migration is required")
    if migration.get("mode") != "explicit-provider-pin-dual-read":
        fail(f"{label} migration mode is invalid")
    if migration.get("rollback") != "retain-last-good-manifest-and-select-by-digest":
        fail(f"{label} migration rollback is invalid")
    if migration.get("decision_ref") != SEGMENTED_DECISION_REF:
        fail(f"{label} migration decision_ref is invalid")
    budgets = payload.get("budgets")
    if not isinstance(budgets, dict) or budgets.get("fail_closed") is not True:
        fail(f"{label} budgets must fail closed")
    part_limit = budgets.get("part_bytes_max")
    request_limit = budgets.get("request_bytes_max")
    for name, value in (("part_bytes_max", part_limit), ("request_bytes_max", request_limit)):
        if isinstance(value, bool) or not isinstance(value, int) or value < MAX_RECORD_BYTES:
            fail(f"{label} budgets.{name} is invalid")
    if part_limit > OWNER_HARD_BYTES_MAX or request_limit > part_limit:
        fail(f"{label} segmented budgets exceed owner authority")
    if budgets.get("legacy_owner_hard_bytes_max") != OWNER_HARD_BYTES_MAX:
        fail(f"{label} legacy owner ceiling is invalid")
    if budgets.get("tracked_bytes_max") != OWNER_HARD_BYTES_MAX:
        fail(f"{label} tracked byte ceiling is invalid")
    summary = payload.get("summary")
    if not isinstance(summary, dict):
        fail(f"{label} summary is required")
    descriptors = payload.get("segments")
    if not isinstance(descriptors, list) or not descriptors:
        fail(f"{label} segments are required")
    if summary.get("segments") != len(descriptors):
        fail(f"{label} summary.segments must match descriptors")
    seen_paths: set[str] = set()
    seen_keys: set[str] = set()
    total_bytes = 0
    total_records = 0
    max_segment_bytes = 0
    for descriptor in descriptors:
        if not isinstance(descriptor, dict):
            fail(f"{label} segment descriptor must be an object")
        path_text = descriptor.get("path")
        if not isinstance(path_text, str) or not path_text.startswith(f"{SEGMENT_ROOT.as_posix()}/"):
            fail(f"{label} segment path escapes the segmented root")
        if path_text in seen_paths:
            fail(f"{label} segment paths must be unique")
        seen_paths.add(path_text)
        path = repo_path(path_text, label=f"{label} segment path")
        if not path.is_file():
            fail(f"{label} segment path is missing: {path_text}")
        declared_bytes = descriptor.get("bytes")
        records = descriptor.get("records")
        descriptor_limit = descriptor.get("request_bytes_max")
        if (
            isinstance(declared_bytes, bool)
            or not isinstance(declared_bytes, int)
            or declared_bytes < 1
            or declared_bytes > request_limit
        ):
            fail(f"{label} segment byte budget is invalid: {path_text}")
        if isinstance(records, bool) or not isinstance(records, int) or records < 1:
            fail(f"{label} segment record count is invalid: {path_text}")
        if descriptor_limit != request_limit:
            fail(f"{label} segment request budget is not manifest-bound: {path_text}")
        digest_text = descriptor.get("digest")
        if not isinstance(digest_text, str) or not re.fullmatch(r"sha256:[a-f0-9]{64}", digest_text):
            fail(f"{label} segment digest is invalid: {path_text}")
        content = source_bytes(Path(path_text), path)
        if len(content) != declared_bytes:
            fail(f"{label} segment bytes drifted for {path_text}")
        if "sha256:" + hashlib.sha256(content).hexdigest() != digest_text:
            fail(f"{label} segment digest drifted for {path_text}")
        kind = descriptor.get("kind")
        range_text = descriptor.get("range")
        if not isinstance(kind, str) or not kind or not isinstance(range_text, str) or not re.fullmatch(r"[a-f0-9]{1,64}", range_text):
            fail(f"{label} segment identity is invalid: {path_text}")
        rows = content.splitlines()
        if len(rows) != records:
            fail(f"{label} segment records drifted for {path_text}")
        for line_number, line in enumerate(rows, start=1):
            if len(line) + 1 > MAX_RECORD_BYTES:
                fail(f"{path_text}:{line_number} exceeds record budget")
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                fail(f"{path_text}:{line_number} is invalid JSON: {exc}")
            if not isinstance(row, dict) or row.get("_kind") != kind:
                fail(f"{path_text}:{line_number} kind mismatch")
            key = row.get("_key")
            if not isinstance(key, str) or key in seen_keys:
                fail(f"{path_text}:{line_number} record key is invalid or duplicated")
            seen_keys.add(key)
        total_bytes += declared_bytes
        total_records += records
        max_segment_bytes = max(max_segment_bytes, declared_bytes)
    if summary.get("canonical_records") != total_records:
        fail(f"{label} summary.canonical_records must match segment records")
    if summary.get("logical_bytes") != total_bytes:
        fail(f"{label} summary.logical_bytes must match segment bytes")
    if summary.get("max_segment_bytes") != max_segment_bytes:
        fail(f"{label} summary.max_segment_bytes must match segments")
    if summary.get("max_segment_bytes", 0) > request_limit:
        fail(f"{label} max segment exceeds request budget")
    if summary.get("tracked_bytes", 0) != summary.get("control_bytes"):
        fail(f"{label} summary.tracked_bytes must equal control_bytes")
    source_header = payload.get("source_index_header")
    if not isinstance(source_header, dict):
        fail(f"{label} source_index_header is required")
    source_path = repo_path("kag/indexes/source_surface_index.json", label=f"{label} source index")
    source_index = read_json(source_path)
    if source_header.get("index_identity") != source_index.get("index_identity"):
        fail(f"{label} source_index_header identity drifted")
    source_digest = source_index.get("index_identity", {}).get("content_digest")
    if source_snapshot != "sha256:" + str(source_digest):
        fail(f"{label} source snapshot does not bind source index")
    compatibility = payload.get("compatibility")
    if not isinstance(compatibility, dict) or compatibility.get("requires_explicit_provider_pin") is not True:
        fail(f"{label} compatibility must require explicit provider pin")
    files = compatibility.get("files")
    if not isinstance(files, list) or not files:
        fail(f"{label} compatibility files are required")
    for file_record in files:
        if not isinstance(file_record, dict):
            fail(f"{label} compatibility file must be an object")
        file_path_text = file_record.get("path")
        if not isinstance(file_path_text, str):
            fail(f"{label} compatibility file path is invalid")
        file_path = repo_path(file_path_text, label=f"{label} compatibility file")
        if not file_path.is_file():
            fail(f"{label} compatibility file is missing: {file_path_text}")
        actual = read_json(file_path)
        actual_identity = actual.get("index_identity")
        expected_digest = file_record.get("content_digest")
        if not isinstance(actual_identity, dict) or actual_identity.get("content_digest") != expected_digest:
            fail(f"{label} compatibility digest drifted for {file_path_text}")
        collection = actual.get("records" if file_record.get("kind") == "source" else "entries")
        if not isinstance(collection, list) or file_record.get("records") != len(collection):
            fail(f"{label} compatibility record count drifted for {file_path_text}")


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
    parser = argparse.ArgumentParser(description="Validate the current ToS-local KAG provider.")
    parser.parse_args(argv)
    try:
        validate_manifest()
        groups = validate_records()
        validate_links(groups)
        validate_repo_local_family()
    except ValidationError as exc:
        print(f"[error] {exc}")
        return 1
    print("[ok] validated Tree-of-Sophia local KAG provider")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
