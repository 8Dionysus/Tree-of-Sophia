from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "validate_local_kag_provider.py"


def load_validator():
    spec = importlib.util.spec_from_file_location("tos_validate_local_kag_provider", SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def valid_record(source_path: str) -> dict[str, object]:
    return {
        "schema_version": "aoa-local-kag-record-v1",
        "repo": "Tree-of-Sophia",
        "local_id": "node:test:nested",
        "record_class": "node",
        "source_refs": [
            {
                "repo": "Tree-of-Sophia",
                "path": source_path,
                "source_class": "tos_source",
                "role": "primary",
                "authority": "authored_source",
            }
        ],
        "source_owner": "Tree-of-Sophia",
        "provenance_mode": "strict_source_linked",
        "derived_method": "test nested provider record",
        "generated_or_authored": "authored_control",
        "status": "active",
        "owner_return_route": {
            "repo": "Tree-of-Sophia",
            "surface": source_path,
            "route_kind": "authored_meaning",
        },
        "freshness": {
            "mode": "source_snapshot",
            "state": "current",
            "checked_ref": source_path,
        },
        "builder": {
            "route": "local KAG provider authoring",
            "surface": "kag/nodes/topic/nested.json",
        },
        "validator": {
            "route": "scripts/validate_local_kag_provider.py",
            "lane": "owner-local",
        },
        "storage_posture": {
            "git_surface": "portable_records",
            "payload_class": "node",
            "runtime_route": "source-repo",
        },
        "consumer_route": "aoa-kag registry",
        "node_kind": "source_surface",
        "label": "nested test record",
    }


def write_source_family(tmp_path: Path) -> Path:
    shard_relative = Path("kag/indexes/shards/source/00.jsonl")
    shard_path = tmp_path / shard_relative
    shard_path.parent.mkdir(parents=True)
    shard_bytes = (
        json.dumps(
            {
                "_kind": "source",
                "identity": {
                    "repo": "Tree-of-Sophia",
                    "path": "ToS/source-that-may-have-moved.md",
                    "content_hash": "stale-during-freeze",
                },
            },
            sort_keys=True,
        )
        + "\n"
    ).encode("utf-8")
    shard_path.write_bytes(shard_bytes)

    manifest_path = tmp_path / "kag/indexes/index_family.manifest.json"
    manifest_path.write_text(
        json.dumps(
            {
                "schema_version": "aoa-repo-local-kag-family-manifest-v3",
                "repo": {"name": "Tree-of-Sophia"},
                "family_identity": {
                    "content_digest": "frozen-family-digest",
                    "source_snapshot": "sha256:frozen-source-snapshot",
                },
                "shards": [
                    {
                        "path": shard_relative.as_posix(),
                        "bytes": len(shard_bytes),
                        "digest": "sha256:" + hashlib.sha256(shard_bytes).hexdigest(),
                        "records": 1,
                        "kind": "source",
                    }
                ],
                "summary": {"source_records": 1},
            }
        )
        + "\n",
        encoding="utf-8",
    )
    return shard_path


def test_repo_path_rejects_absolute_and_parent_escape_paths() -> None:
    validator = load_validator()

    for path_text in ("/tmp/outside.md", "../outside.md", "kag/../README.md"):
        with pytest.raises(validator.ValidationError):
            validator.repo_path(path_text, label="test source ref")


def test_validate_records_discovers_nested_provider_json(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    validator = load_validator()
    source_path = "ToS/source.md"
    (tmp_path / "ToS").mkdir()
    (tmp_path / source_path).write_text("source\n", encoding="utf-8")
    nested_dir = tmp_path / "kag" / "nodes" / "topic"
    nested_dir.mkdir(parents=True)
    (nested_dir / "nested.json").write_text(
        json.dumps(valid_record(source_path), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    monkeypatch.setattr(validator, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(validator, "KAG_ROOT", tmp_path / "kag")
    monkeypatch.setattr(validator, "RECORD_DIRS", {"nodes": "node"})

    groups = validator.validate_records()

    assert groups["nodes"][0]["local_id"] == "node:test:nested"


def test_family_rejects_deleted_source_despite_intact_shards(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    validator = load_validator()
    write_source_family(tmp_path)
    monkeypatch.setattr(validator, "REPO_ROOT", tmp_path)

    with pytest.raises(validator.ValidationError, match="record path is missing"):
        validator.validate_repo_local_family()


def test_family_integrity_rejects_changed_shard_bytes(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    validator = load_validator()
    shard_path = write_source_family(tmp_path)
    shard_path.write_bytes(shard_path.read_bytes() + b"{}\n")
    monkeypatch.setattr(validator, "REPO_ROOT", tmp_path)

    with pytest.raises(validator.ValidationError, match="bytes drifted"):
        validator.validate_repo_local_family()


def test_removed_freeze_only_option_cannot_bypass_currentness() -> None:
    with pytest.raises(SystemExit) as error:
        load_validator().main(["--freeze-only"])
    assert error.value.code == 2


def write_segmented_family(tmp_path: Path) -> Path:
    validator = load_validator()
    for relative in (
        "ToS/derived-exports/kag_export.min.json",
        "mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md",
    ):
        path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("source\n", encoding="utf-8")

    index_identity = {
        "local_id": "index:repo-local:source-surfaces",
        "content_digest": "a" * 64,
    }
    source_index = {
        "schema_version": "aoa-repo-local-kag-index-v2",
        "repo": {"name": "Tree-of-Sophia"},
        "index_identity": index_identity,
        "records": [
            {
                "identity": {
                    "repo": "Tree-of-Sophia",
                    "path": "ToS/derived-exports/kag_export.min.json",
                }
            }
        ],
    }
    index_path = tmp_path / "kag/indexes/source_surface_index.json"
    index_path.parent.mkdir(parents=True, exist_ok=True)
    index_path.write_text(json.dumps(source_index, sort_keys=True) + "\n", encoding="utf-8")

    row = {
        "_kind": "source",
        "_key": "source:tos:export",
        "identity": {
            "repo": "Tree-of-Sophia",
            "path": "ToS/derived-exports/kag_export.min.json",
        },
    }
    segment_bytes = (json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n").encode()
    segment_relative = Path("kag/indexes/segments/source/aa.jsonl")
    segment_path = tmp_path / segment_relative
    segment_path.parent.mkdir(parents=True, exist_ok=True)
    segment_path.write_bytes(segment_bytes)
    pin = {
        "schema_version": "tos-kag-provider-pin-v1",
        "consumer_repo": "Tree-of-Sophia",
        "provider_repo": "aoa-kag",
        "provider_revision": "d9b00bc456ea95dd8447311331ee83ba51afa023",
        "provider_action": ".github/actions/repo-local-kag-index/action.yml",
        "family_schema": validator.SEGMENTED_FAMILY_SCHEMA,
        "schema_ref": validator.SEGMENTED_SCHEMA_REF,
        "consumer_adapter": "scripts/validate_local_kag_provider.py",
        "migration": {
            "mode": "explicit-provider-pin-dual-read",
            "rollback": "retain-last-good-manifest-and-select-by-digest",
            "decision_ref": validator.SEGMENTED_DECISION_REF,
        },
        "source_refs": [
            {"repo": "Tree-of-Sophia", "path": "ToS/derived-exports/kag_export.min.json"}
        ],
    }
    (tmp_path / "kag").mkdir(exist_ok=True)
    (tmp_path / "kag/provider-pin.json").write_text(
        json.dumps(pin, sort_keys=True) + "\n", encoding="utf-8"
    )
    compatibility = {
        "view": "aoa-repo-local-kag-v2",
        "assembly": "deterministic-on-demand-from-segments",
        "requires_explicit_provider_pin": True,
        "files": [
            {
                "kind": "source",
                "path": "kag/indexes/source_surface_index.json",
                "schema_version": source_index["schema_version"],
                "content_digest": "a" * 64,
                "records": 1,
            }
        ],
    }
    manifest = {
        "schema_version": validator.SEGMENTED_FAMILY_SCHEMA,
        "repo": {"name": "Tree-of-Sophia"},
        "family_identity": {
            "local_id": "family:repo-local:segmented-record-corpus",
            "artifact_kind": "repo_local_kag_segmented_family",
            "content_digest": "0" * 64,
            "schema_ref": validator.SEGMENTED_SCHEMA_REF,
            "source_snapshot": "sha256:" + "a" * 64,
        },
        "producer_identity": {
            "version": "aoa-kag:segmented-family-producer-v1",
            "route": "aoa-kag:scripts/repo_local/segmented_family.py",
        },
        "candidate_identity": {
            "version": "aoa-kag:segmented-family-candidate-v1",
            "content_digest": "b" * 64,
            "source_index_content_digest": "a" * 64,
        },
        "migration": {
            "mode": "explicit-provider-pin-dual-read",
            "rollback": "retain-last-good-manifest-and-select-by-digest",
            "decision_ref": validator.SEGMENTED_DECISION_REF,
        },
        "budgets": {
            "part_bytes_max": 16 * 1024 * 1024,
            "request_bytes_max": 4 * 1024 * 1024,
            "legacy_owner_hard_bytes_max": validator.OWNER_HARD_BYTES_MAX,
            "tracked_bytes_max": validator.OWNER_HARD_BYTES_MAX,
            "fail_closed": True,
        },
        "summary": {
            "segments": 1,
            "canonical_records": 1,
            "logical_bytes": len(segment_bytes),
            "max_segment_bytes": len(segment_bytes),
            "tracked_bytes": 1,
            "control_bytes": 1,
        },
        "source_index_header": {
            "index_identity": index_identity,
        },
        "compatibility": compatibility,
        "segments": [
            {
                "kind": "source",
                "range": "aa",
                "path": segment_relative.as_posix(),
                "digest": "sha256:" + hashlib.sha256(segment_bytes).hexdigest(),
                "bytes": len(segment_bytes),
                "records": 1,
                "request_bytes_max": 4 * 1024 * 1024,
            }
        ],
    }
    manifest["family_identity"]["content_digest"] = validator.segmented_manifest_digest(manifest)
    manifest_path = tmp_path / "kag/indexes/index_family.manifest.json"
    manifest_path.write_text(json.dumps(manifest, sort_keys=True) + "\n", encoding="utf-8")
    return manifest_path


def test_segmented_family_adapter_validates_bounded_consumer_contract(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    validator = load_validator()
    write_segmented_family(tmp_path)
    monkeypatch.setattr(validator, "REPO_ROOT", tmp_path)
    validator.validate_repo_local_family()


def test_segmented_family_adapter_rejects_tampered_segment(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    validator = load_validator()
    write_segmented_family(tmp_path)
    manifest = json.loads((tmp_path / "kag/indexes/index_family.manifest.json").read_text())
    manifest["source_index_header"] = json.loads(
        (tmp_path / "kag/indexes/source_surface_index.json").read_text()
    )
    manifest["family_identity"]["content_digest"] = validator.segmented_manifest_digest(manifest)
    (tmp_path / "kag/indexes/index_family.manifest.json").write_text(
        json.dumps(manifest, sort_keys=True) + "\n"
    )
    monkeypatch.setattr(validator, "REPO_ROOT", tmp_path)
    (tmp_path / "kag/indexes/segments/source/aa.jsonl").write_bytes(b"tampered\n")
    with pytest.raises(validator.ValidationError, match="segment bytes drifted"):
        validator.validate_repo_local_family()
