from __future__ import annotations

import importlib.util
import hashlib
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


def commit_fixture(repo: Path, message: str) -> None:
    subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
    subprocess.run(["git", "commit", "-qm", message], cwd=repo, check=True)


def configure_preparer_fixture(preparer) -> None:
    preparer.AUTHORITATIVE_SOURCE_PATHS = (Path("canonical.json"),)
    preparer.MATERIALIZED_SOURCE_PATHS = (Path("mirror.json"),)
    preparer.EXISTENCE_ONLY_SOURCE_PATHS = (Path("support.md"),)
    preparer.DERIVED_EXPORT_PATHS = (Path("export.json"), Path("export.min.json"))
    preparer.BUILDER_PATH = Path("builder.py")
    preparer.VALIDATOR_PATH = Path("validator.py")
    preparer.FAMILY_VALIDATOR_PATH = Path("family_validator.py")
    preparer.FAMILY_MANIFEST_PATH = Path("kag/indexes/index_family.manifest.json")


def make_preparation_repo(tmp_path: Path, preparer) -> Path:
    manifest = {
        "schema_version": "aoa-repo-local-kag-family-manifest-v3",
        "family_identity": {
            "content_digest": "fixture-content",
            "source_snapshot": "fixture-source",
        },
        "summary": {"source_records": 5},
        "compatibility": {
            "files": [
                {
                    "kind": "artifact",
                    "path": "kag/indexes/compatibility.json",
                }
            ]
        },
    }
    files = {
        "canonical.json": "{\"node\":\"fixture\"}\n",
        "mirror.json": "{\"node\":\"fixture\"}\n",
        "support.md": "# fixture support\n",
        "export.json": "{\"export\":true}\n",
        "export.min.json": "{\"export\":true}\n",
        "builder.py": "# fixture builder\n",
        "validator.py": "# fixture validator\n",
        "family_validator.py": "import sys\nsys.exit(0)\n",
        "kag/indexes/compatibility.json": "fixture compatibility\n",
        "kag/indexes/index_family.manifest.json": json.dumps(manifest, sort_keys=True) + "\n",
        "kag/indexes/shards/source/00.jsonl": "placeholder\n",
    }
    repo = make_git_repo(tmp_path, files)
    selected = ["canonical.json", "mirror.json", "support.md", "export.json", "export.min.json"]
    rows = []
    for path_text in selected:
        identity = preparer._file_identity(repo, Path(path_text), "fixture source")
        rows.append(
            {
                "_kind": "source",
                "_key": f"source:{path_text}",
                "identity": {
                    "path": path_text,
                    "content_hash": identity["sha256"],
                    "git_blob_id": identity["head_blob"],
                    "repo": "Tree-of-Sophia",
                },
                "freshness": {"state": "current"},
                "signs": {"digest": identity["sha256"]},
            }
        )
    shard = repo / "kag/indexes/shards/source/00.jsonl"
    shard.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")
    commit_fixture(repo, "index fixture")
    return repo


def rewrite_family_manifest(repo: Path, files: object) -> None:
    manifest_path = repo / "kag/indexes/index_family.manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["compatibility"]["files"] = files
    manifest_path.write_text(json.dumps(manifest, sort_keys=True) + "\n", encoding="utf-8")
    commit_fixture(repo, "manifest fixture")


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


def test_environment_packet_redacts_secret_like_and_unknown_git_values(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    preparer = load_preparer()
    monkeypatch.setenv("GIT_SSH_COMMAND", "secret-placeholder")
    monkeypatch.setenv("GIT_UNKNOWN_OWNER_VALUE", "another-secret")

    identity = preparer._environment_identity()
    serialized = json.dumps(identity, sort_keys=True)

    assert "secret-placeholder" not in serialized
    assert "another-secret" not in serialized
    assert "GIT_SSH_COMMAND" not in identity["variables"]
    assert identity["git_variables"]["GIT_SSH_COMMAND"] == {
        "present": True,
        "length": len("secret-placeholder"),
        "sha256": hashlib.sha256(b"secret-placeholder").hexdigest(),
        "redacted": True,
    }
    assert identity["git_variables"]["GIT_UNKNOWN_OWNER_VALUE"]["redacted"] is True
    assert identity["redaction_policy"]["raw_git_values_recorded"] is False


def test_build_owner_preparation_runs_real_packet_boundary(tmp_path: Path) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)

    packet = preparer.build_owner_preparation(repo)

    assert packet["status"] == "blocked_external_kag_contract"
    assert packet["candidate"]["status"] == "clean"
    assert packet["candidate"]["final_revalidation"] == "passed"
    assert packet["tree_source"]["family"]["local_validator"]["exit_code"] == 0
    assert packet["tree_currentness"]["family"] == "validated_current"
    assert packet["tree_source"]["family"]["compatibility_records"][0]["identity"]["is_symlink"] is False
    assert packet["claim_boundary"]["semantic_pair_emitted"] is False


def test_build_owner_preparation_preserves_missing_compatibility_snapshot(
    tmp_path: Path,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    rewrite_family_manifest(repo, [{"path": "kag/indexes/on-demand.json"}])

    packet = preparer.build_owner_preparation(repo)

    family = packet["tree_source"]["family"]
    assert family["identity_binding"]["state"] == "partial_snapshot"
    assert family["identity_binding"]["missing_compatibility_files"] == [
        "kag/indexes/on-demand.json"
    ]
    assert family["compatibility_records"][0]["identity"] is None
    assert family["compatibility_records"][0]["state"] == "missing"


def test_build_owner_preparation_rejects_missing_compatibility_under_parent_symlink(
    tmp_path: Path,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    outside = tmp_path / "outside"
    outside.mkdir()
    (repo / "kag/indexes/alias").symlink_to(outside, target_is_directory=True)
    rewrite_family_manifest(
        repo,
        [{"kind": "artifact", "path": "kag/indexes/alias/missing.json"}],
    )

    with pytest.raises(preparer.PreparationError, match="confined"):
        preparer.build_owner_preparation(repo)


def test_build_owner_preparation_rejects_parent_replacement_during_missing_final_window(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    alias = repo / "kag/indexes/alias"
    alias.mkdir()
    (alias / "anchor.txt").write_text("anchor\n", encoding="utf-8")
    commit_fixture(repo, "alias fixture")
    rewrite_family_manifest(
        repo,
        [{"kind": "artifact", "path": "kag/indexes/alias/missing.json"}],
    )
    outside = tmp_path / "outside"
    outside.mkdir()
    original_lstat = preparer.os.lstat
    missing_lstat_hits = 0
    swapped = False

    def racing_lstat(path, *, dir_fd=None):
        nonlocal missing_lstat_hits, swapped
        try:
            return original_lstat(path, dir_fd=dir_fd)
        except FileNotFoundError:
            if path == "missing.json" and dir_fd is not None:
                missing_lstat_hits += 1
                if missing_lstat_hits == 2:
                    alias.rename(repo / "kag/indexes/alias.old")
                    alias.symlink_to(outside, target_is_directory=True)
                    swapped = True
            raise

    monkeypatch.setattr(preparer.os, "lstat", racing_lstat)
    monkeypatch.setattr(
        preparer.os,
        "supports_dir_fd",
        set(preparer.os.supports_dir_fd) | {racing_lstat},
    )

    with pytest.raises(preparer.PreparationError, match="parent path changed"):
        preparer.build_owner_preparation(repo)

    assert swapped is True
    assert missing_lstat_hits >= 2


def test_build_owner_preparation_rejects_embedded_nul_manifest_path(
    tmp_path: Path,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    rewrite_family_manifest(repo, [{"kind": "artifact", "path": "\x00"}])

    with pytest.raises(preparer.PreparationError, match="embedded NUL"):
        preparer.build_owner_preparation(repo)


def test_build_owner_preparation_rejects_unencodable_manifest_path(
    tmp_path: Path,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    rewrite_family_manifest(repo, [{"kind": "artifact", "path": "\ud800"}])

    with pytest.raises(preparer.PreparationError, match="unsupported OS-invalid input"):
        preparer.build_owner_preparation(repo)


def test_cli_rejects_embedded_nul_as_controlled_exit(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    rewrite_family_manifest(repo, [{"kind": "artifact", "path": "\x00"}])

    result = preparer.main(["--repo-root", str(repo), "--json"])

    captured = capsys.readouterr()
    assert result == 2
    assert "embedded NUL" in captured.err
    assert "Traceback" not in captured.err


def test_build_owner_preparation_preserves_stale_family_negative(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    monkeypatch.setattr(
        preparer,
        "_run_local_family_validator",
        lambda _repo: {
            "command": ["fixture-validator"],
            "exit_code": 1,
            "state": "stale_or_invalid",
            "stdout_tail": [],
            "stderr_tail": ["stale family"],
            "claim_limit": "fixture",
        },
    )

    packet = preparer.build_owner_preparation(repo)

    assert packet["tree_currentness"]["family"] == "blocked_stale_or_invalid"
    assert packet["dependency_witness"]["complete_for_external_semantic_pair"] is False


def test_build_owner_preparation_rejects_manifest_traversal_target(
    tmp_path: Path,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    (repo.parent / "outside.txt").write_text("outside\n", encoding="utf-8")
    rewrite_family_manifest(repo, [{"path": "../outside.txt"}])

    with pytest.raises(preparer.PreparationError, match="must stay inside the candidate"):
        preparer.build_owner_preparation(repo)


def test_build_owner_preparation_rejects_manifest_symlink_target(tmp_path: Path) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    (repo / "kag/indexes/compatibility-link.json").symlink_to("compatibility.json")
    rewrite_family_manifest(repo, [{"path": "kag/indexes/compatibility-link.json"}])

    with pytest.raises(preparer.PreparationError, match="symlink"):
        preparer.build_owner_preparation(repo)


def test_build_owner_preparation_rejects_malformed_manifest_compatibility(
    tmp_path: Path,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    rewrite_family_manifest(repo, ["not-an-object"])

    with pytest.raises(preparer.PreparationError, match=r"compatibility.files\[0\] must be an object"):
        preparer.build_owner_preparation(repo)


def test_build_owner_preparation_rejects_observation_replacement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    original_capture = preparer._capture_observation
    calls = 0

    def racing_capture(root: Path, *, run_family_validator: bool = True):
        nonlocal calls
        calls += 1
        observation = original_capture(root, run_family_validator=run_family_validator)
        if calls == 2:
            observation["candidate"] = {
                **observation["candidate"],
                "head": "replacement-observed-at-boundary",
            }
        return observation

    monkeypatch.setattr(preparer, "_capture_observation", racing_capture)
    with pytest.raises(preparer.PreparationError, match="TOCTOU"):
        preparer.build_owner_preparation(repo)


def test_build_owner_preparation_rejects_file_replacement_race(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    preparer = load_preparer()
    configure_preparer_fixture(preparer)
    repo = make_preparation_repo(tmp_path, preparer)
    original_open = preparer.os.open
    replaced = False

    def racing_open(path, flags, mode=0o777, *, dir_fd=None):
        nonlocal replaced
        file_descriptor = original_open(path, flags, mode, dir_fd=dir_fd)
        if not replaced and path == "canonical.json" and dir_fd is not None:
            replacement = repo / "canonical.replacement"
            replacement.write_text("replacement\n", encoding="utf-8")
            (repo / "canonical.json").unlink()
            replacement.rename(repo / "canonical.json")
            replaced = True
        return file_descriptor

    monkeypatch.setattr(preparer.os, "open", racing_open)
    with pytest.raises(preparer.PreparationError, match="replaced while"):
        preparer.build_owner_preparation(repo)


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
