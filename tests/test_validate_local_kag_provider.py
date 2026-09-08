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
