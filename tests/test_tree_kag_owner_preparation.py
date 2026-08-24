from __future__ import annotations

import importlib.util
import json
import os
import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = (
    REPO_ROOT
    / "mechanics/boundary-bridge/parts/derived-kag-seam/scripts/prepare_tree_kag_owner_pair.py"
)


def load_preparer():
    spec = importlib.util.spec_from_file_location("tos_prepare_tree_kag_owner_pair", SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def make_git_repo(tmp_path: Path, files: dict[str, str]) -> Path:
    repo = tmp_path / "repo"
    repo.mkdir()
    subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
    subprocess.run(["git", "config", "user.email", "test@example.invalid"], cwd=repo, check=True)
    subprocess.run(["git", "config", "user.name", "Test"], cwd=repo, check=True)
    for relative, content in files.items():
        path = repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    subprocess.run(["git", "add", "."], cwd=repo, check=True)
    subprocess.run(["git", "commit", "-qm", "fixture"], cwd=repo, check=True)
    return repo


def test_dependency_classes_preserve_materialized_and_existence_only_layers() -> None:
    preparer = load_preparer()

    result = preparer.classify_dependency_paths(
        ("ToS/source.json", "ToS/source.json"),
        ("ToS/support.md", "ToS/source.json"),
    )

    assert result == {
        "materialized": ["ToS/source.json"],
        "existence_only": ["ToS/support.md"],
    }


def test_preparation_fails_closed_on_dirty_worktree(monkeypatch: pytest.MonkeyPatch) -> None:
    preparer = load_preparer()

    def fake_git(_repo_root: Path, *args: str) -> str:
        if args == ("rev-parse", "--show-toplevel"):
            return str(REPO_ROOT)
        if args == ("status", "--porcelain=v1"):
            return " M unrelated-user-work"
        raise AssertionError(f"unexpected git call after dirty-state rejection: {args}")

    monkeypatch.setattr(preparer, "_git", fake_git)
    with pytest.raises(preparer.PreparationError, match="clean isolated worktree"):
        preparer.build_owner_preparation(REPO_ROOT)


def test_canonical_only_drift_cannot_claim_parity() -> None:
    preparer = load_preparer()
    canonical = {
        "sha256": "canonical",
        "git_blob": "canonical-blob",
        "head_blob": "canonical-head",
        "file_kind": "regular_file",
        "resolved_path": "canonical.json",
        "device": 1,
        "inode": 1,
    }
    mirror = {**canonical, "sha256": "mirror", "git_blob": "mirror-blob", "head_blob": "mirror-head", "resolved_path": "mirror.json", "inode": 2}
    with pytest.raises(preparer.PreparationError, match="failed observed parity"):
        preparer._canonical_mirror_parity(canonical, mirror)


def test_file_identity_rejects_symlink_and_foreign_path(tmp_path: Path) -> None:
    preparer = load_preparer()
    repo = make_git_repo(tmp_path, {"target.txt": "payload\n"})
    (repo / "link.txt").symlink_to("target.txt")
    subprocess.run(["git", "add", "link.txt"], cwd=repo, check=True)
    subprocess.run(["git", "commit", "-qm", "symlink"], cwd=repo, check=True)

    with pytest.raises(preparer.PreparationError, match="symlink"):
        preparer._file_identity(repo, Path("link.txt"), "symlink_probe")
    with pytest.raises(preparer.PreparationError, match="inside the candidate"):
        preparer._file_identity(repo, Path("../foreign.txt"), "foreign_probe")


def test_file_identity_rejects_non_regular_file(tmp_path: Path) -> None:
    preparer = load_preparer()
    repo = make_git_repo(tmp_path, {"regular.txt": "payload\n"})
    (repo / "directory").mkdir()
    with pytest.raises(preparer.PreparationError, match="unsupported file kind"):
        preparer._file_identity(repo, Path("directory"), "directory_probe")


def test_file_identity_rejects_parent_directory_symlink(tmp_path: Path) -> None:
    preparer = load_preparer()
    repo = make_git_repo(tmp_path, {"real/source.txt": "payload\n"})
    (repo / "alias").symlink_to(repo / "real", target_is_directory=True)

    with pytest.raises(preparer.PreparationError, match="confined"):
        preparer._file_identity(repo, Path("alias/source.txt"), "parent_symlink_probe")


def test_hard_link_alias_is_not_admissible(tmp_path: Path) -> None:
    preparer = load_preparer()
    repo = make_git_repo(tmp_path, {"authority.json": "{}\n"})
    (repo / "mirror.json").hardlink_to(repo / "authority.json")
    subprocess.run(["git", "add", "mirror.json"], cwd=repo, check=True)
    subprocess.run(["git", "commit", "-qm", "alias"], cwd=repo, check=True)
    authority = preparer._file_identity(repo, Path("authority.json"), "authority")
    mirror = preparer._file_identity(repo, Path("mirror.json"), "mirror")
    with pytest.raises(preparer.PreparationError, match="hard-link/alias"):
        preparer._assert_no_aliases([authority, mirror])


def test_source_index_row_must_match_current_bytes_and_head_blob(tmp_path: Path) -> None:
    preparer = load_preparer()
    repo = make_git_repo(tmp_path, {"source.json": "current\n"})
    shard = repo / "kag/indexes/shards/source/00.jsonl"
    shard.parent.mkdir(parents=True)
    shard.write_text(
        json.dumps(
                {
                    "identity": {
                        "path": "source.json",
                        "content_hash": "stale",
                        "git_blob_id": "stale",
                        "repo": "Tree-of-Sophia",
                    },
                "freshness": {"state": "current"},
                "signs": {"digest": "stale"},
            }
        )
        + "\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "add", "kag"], cwd=repo, check=True)
    subprocess.run(["git", "commit", "-qm", "index"], cwd=repo, check=True)
    record = preparer._file_identity(repo, Path("source.json"), "source")
    with pytest.raises(preparer.PreparationError, match="source-index row is stale"):
        preparer._source_index_rows(repo, {"source.json"}, {"source.json": record})


def test_source_index_rejects_foreign_symlinked_shard(tmp_path: Path) -> None:
    preparer = load_preparer()
    repo = make_git_repo(tmp_path, {"source.json": "current\n"})
    shard = repo / "kag/indexes/shards/source/00.jsonl"
    shard.parent.mkdir(parents=True)
    foreign = tmp_path / "foreign.jsonl"
    foreign.write_text("{}\n", encoding="utf-8")
    shard.symlink_to(foreign)

    with pytest.raises(preparer.PreparationError, match="symlink"):
        preparer._source_index_rows(repo, set(), {})


def test_source_index_requires_explicit_sign_digest(tmp_path: Path) -> None:
    preparer = load_preparer()
    repo = make_git_repo(tmp_path, {"source.json": "current\n"})
    record = preparer._file_identity(repo, Path("source.json"), "source")
    shard = repo / "kag/indexes/shards/source/00.jsonl"
    shard.parent.mkdir(parents=True)
    shard.write_text(
        json.dumps(
            {
                "identity": {
                    "path": "source.json",
                    "repo": "Tree-of-Sophia",
                    "content_hash": record["sha256"],
                    "git_blob_id": record["head_blob"],
                },
                "freshness": {"state": "current"},
                "signs": {},
            }
        )
        + "\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "add", "kag"], cwd=repo, check=True)
    subprocess.run(["git", "commit", "-qm", "index"], cwd=repo, check=True)
    with pytest.raises(preparer.PreparationError, match="signs.digest"):
        preparer._source_index_rows(repo, {"source.json"}, {"source.json": record})


def test_file_identity_rejects_bytes_not_bound_to_head(tmp_path: Path) -> None:
    preparer = load_preparer()
    repo = make_git_repo(tmp_path, {"source.txt": "current\n"})
    (repo / "source.txt").write_text("drifted\n", encoding="utf-8")

    with pytest.raises(preparer.PreparationError, match="bytes/index"):
        preparer._file_identity(repo, Path("source.txt"), "drift_probe")


def test_expected_candidate_and_producer_seal_rejects_drift() -> None:
    preparer = load_preparer()
    observation = {
        "candidate": {"head": "head", "tree": "tree", "parent": "parent"},
        "builder": {"sha256": "builder"},
        "validator": {"sha256": "validator"},
        "family_validator": {"sha256": "family-validator"},
        "environment": {"sha256": "environment"},
    }
    expected = {
        "head": "different-head",
        "tree": "tree",
        "parent": "parent",
        "builder_sha256": "builder",
        "validator_sha256": "validator",
        "family_validator_sha256": "family-validator",
        "environment_sha256": "environment",
    }
    with pytest.raises(preparer.PreparationError, match="seal mismatch"):
        preparer._validate_expected_seal(observation, expected)


def test_environment_drift_changes_environment_seal(monkeypatch: pytest.MonkeyPatch) -> None:
    preparer = load_preparer()
    before = preparer._environment_identity()["sha256"]
    monkeypatch.setenv("TZ", "tree-owner-adversarial-zone")
    after = preparer._environment_identity()["sha256"]
    assert before != after


def test_environment_seal_binds_path_and_git_configuration(monkeypatch: pytest.MonkeyPatch) -> None:
    preparer = load_preparer()
    before = preparer._environment_identity()["sha256"]
    monkeypatch.setenv("PATH", os.environ["PATH"] + ":/tree-owner-test-path")
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    after = preparer._environment_identity()["sha256"]
    assert before != after


def test_final_revalidation_rejects_replacement() -> None:
    preparer = load_preparer()
    with pytest.raises(preparer.PreparationError, match="TOCTOU"):
        preparer._assert_revalidated(
            {"candidate": {"head": "before"}},
            {"candidate": {"head": "after"}},
        )


def test_blocked_external_contract_is_non_success_for_cli(monkeypatch: pytest.MonkeyPatch) -> None:
    preparer = load_preparer()
    monkeypatch.setattr(
        preparer,
        "build_owner_preparation",
        lambda *_args, **_kwargs: {"status": "blocked_external_kag_contract"},
    )
    expected_args = [
        "--check",
        "--expected-head",
        "head",
        "--expected-tree",
        "tree",
        "--expected-parent",
        "parent",
        "--expected-builder-sha256",
        "builder",
        "--expected-validator-sha256",
        "validator",
        "--expected-family-validator-sha256",
        "family-validator",
        "--expected-environment-sha256",
        "environment",
    ]
    assert preparer.main(expected_args) == preparer.BLOCKED_EXTERNAL_EXIT
