from __future__ import annotations

from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import Iterator

import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from build_kag_export import PRIMARY, verify_export  # noqa: E402
from corpus_store import (  # noqa: E402
    CorpusCandidate,
    CorpusStore,
    CorpusStoreError,
    ValidationIndex,
    canonical,
)
from downstream_status import DownstreamStatus  # noqa: E402
from publish_kag_release import (  # noqa: E402
    PROGRAM_PATHS,
    _verify_integration,
    build_release,
    status_release,
)


VALIDATOR_SHA256 = "1" * 64


@contextmanager
def _synthetic_store() -> Iterator[tuple[Path, str, Path, dict[str, bytes]]]:
    temporary = tempfile.TemporaryDirectory(prefix="tos-kag-publish-")
    root = Path(temporary.name)
    store_root = root / "store"
    source_root = root / "source"
    store = CorpusStore(store_root)
    source_bytes = {
        PRIMARY: canonical({"node_id": "tiny-node"}),
        "ToS/derived-exports/README.md": b"# tiny derived export\n",
        "ToS/public-compatibility/concept_node.example.json": canonical(
            {"node_id": "tiny-concept"}
        ),
        "ToS/public-compatibility/source_node.example.json": canonical(
            {
                "node_id": "tiny-node",
                "interpretation_layers": ["tiny-layer"],
                "relations": [{"relation_type": "tiny", "target_ref": "tiny-node"}],
            }
        ),
        "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md": b"# tiny capsule\n",
        "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md": b"# tiny entry\n",
    }
    updates: dict[str, dict[str, object]] = {}
    for relative, payload in source_bytes.items():
        source = source_root / relative
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(payload)
        updates[relative] = {
            "source": source,
            "sha256": hashlib.sha256(payload).hexdigest(),
            "size_bytes": len(payload),
            "mode": 0o644,
        }

    def synthetic_validator(
        candidate: CorpusCandidate,
        base: dict | None,
        affected: frozenset[str],
    ) -> ValidationIndex:
        del candidate, base, affected
        return ValidationIndex({"tiny-node": PRIMARY}, {})

    accepted = store.admit(
        base_revision=None,
        updates=updates,
        retirements={},
        validator_sha256=VALIDATOR_SHA256,
        validate=synthetic_validator,
    )
    try:
        yield store_root, accepted["revision"], root, source_bytes
    finally:
        temporary.cleanup()


_PRODUCER = r'''\
import argparse
import hashlib
import json
import sys
from pathlib import Path

PRIMARY = "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json"

parser = argparse.ArgumentParser()
parser.add_argument("--repo-root", required=True)
parser.add_argument("--artifact-root", required=True)
args = parser.parse_args()
repo_root = Path(args.repo_root)
artifact_root = Path(args.artifact_root)
kag_root = Path(__file__).resolve().parents[1]
mode = json.loads((kag_root / "mode.json").read_text())["mode"]
if mode == "fail":
    print("owner failure sentinel", file=sys.stderr)
    raise SystemExit(17)
primary = repo_root / PRIMARY
before = primary.read_bytes()
content_hash = hashlib.sha256(before).hexdigest()
if mode == "mutate":
    primary.write_bytes(before + b"consumer mutation\n")
if mode == "mismatch":
    content_hash = "0" * 64
if mode in {"control-fail", "control-mutate"}:
    manifest_path = repo_root / "kag" / "manifest.json"
    manifest = json.loads(manifest_path.read_bytes())
    if mode == "control-fail":
        manifest["source_surfaces"].append(
            {
                "repo": "Tree-of-Sophia",
                "path": "ToS/missing-controlled-source.json",
                "source_class": "tos_source",
                "role": "supporting",
                "authority": "authored_source",
            }
        )
    else:
        manifest["owner_surface"] = "kag/changed-owner.md"
    manifest_path.write_bytes(
        json.dumps(manifest, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    )
carrier = repo_root / "kag" / "indexes" / "hot.json"
carrier.parent.mkdir(parents=True, exist_ok=True)
carrier.write_bytes(b"generated hot carrier\n")
artifact_root.mkdir(parents=True, exist_ok=True)
config = {
    "content_hash": content_hash,
    "distribution_identity": {
        "local_id": "tiny-kag",
        "artifact_kind": "repository-index",
        "content_digest": "2" * 64,
        "schema_ref": "tiny-schema",
    },
}
(artifact_root / "config.json").write_bytes(
    json.dumps(config, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
)
'''

_VALIDATOR = r'''\
import hashlib
import json
from pathlib import Path

PRIMARY = "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json"

def load_repo_local_kag_repository_index_family_with_manifest(
    provider_root,
    *,
    artifact_root,
    allow_shadow_git=False,
):
    del allow_shadow_git
    provider_root = Path(provider_root)
    artifact_root = Path(artifact_root)
    config = json.loads((artifact_root / "config.json").read_bytes())
    primary = provider_root / PRIMARY
    digest = hashlib.sha256(primary.read_bytes()).hexdigest()
    record = {
        "identity": {"path": PRIMARY, "content_hash": config["content_hash"]},
        "owner_return_route": {"owner": "tiny-owner", "surface": "source-return"},
        "computed_hash": digest,
    }
    source = {"records": [record]}
    manifest = {"distribution_identity": config["distribution_identity"]}
    return source, {"tiny": "family"}, manifest
'''

_LOCAL_SUBTREE = r'''\
import json
from pathlib import Path

CONTROL_PATHS = (
    "kag/AGENTS.md",
    "kag/README.md",
    "kag/edges/source-return.json",
    "kag/indexes/source-routes.json",
    "kag/manifest.json",
    "kag/nodes/export-route.json",
    "kag/nodes/source-export.json",
    "kag/projections/source-return.json",
    "kag/receipts/publication-route.json",
)


def _source_refs(value):
    if isinstance(value, dict):
        for key, child in value.items():
            if key in {"source_refs", "source_surfaces"}:
                if not isinstance(child, list) or not child:
                    raise ValueError("provider source references are missing")
                for reference in child:
                    if not isinstance(reference, dict):
                        raise ValueError("provider source reference is malformed")
                    if reference.get("repo") != "Tree-of-Sophia":
                        raise ValueError("provider source reference has the wrong owner")
                    path = reference.get("path")
                    if (not isinstance(path, str)
                            or not (path.startswith("ToS/") or path.startswith("kag/"))):
                        raise ValueError("provider source reference is not a ToS path")
                    yield path
            yield from _source_refs(child)
    elif isinstance(value, list):
        for child in value:
            yield from _source_refs(child)


def _validate_provider_home(repo, provider_root, *, prebuild=True):
    if repo != "Tree-of-Sophia" or prebuild:
        raise ValueError("postbuild provider-home validation is required")
    root = Path(provider_root)
    for relative in CONTROL_PATHS:
        path = root / relative
        if path.is_symlink() or not path.is_file():
            raise ValueError("provider control is missing: " + relative)
    for relative in CONTROL_PATHS:
        if not relative.endswith(".json"):
            continue
        document = json.loads((root / relative).read_bytes())
        for source_path in _source_refs(document):
            source = root / source_path
            if source.is_symlink() or not source.is_file():
                raise ValueError("provider source reference is missing: " + source_path)
'''

_QUERY = "print('query helper')\n"


def _fake_kag_root(root: Path, *, mode: str = "ok") -> Path:
    kag_root = root / "kag-root"
    (kag_root / "scripts" / "validators").mkdir(parents=True)
    (kag_root / "scripts" / "build_repo_local_kag_release.py").write_text(
        _PRODUCER, encoding="utf-8"
    )
    (kag_root / "scripts" / "query_repo_local_kag.py").write_text(
        _QUERY, encoding="utf-8"
    )
    (kag_root / "scripts" / "validators" / "__init__.py").write_text("", encoding="utf-8")
    (kag_root / "scripts" / "validators" / "repo_local_kag_index.py").write_text(
        _VALIDATOR, encoding="utf-8"
    )
    (kag_root / "scripts" / "validators" / "local_kag_subtree.py").write_text(
        _LOCAL_SUBTREE, encoding="utf-8"
    )
    (kag_root / "mode.json").write_bytes(canonical({"mode": mode}))
    return kag_root


def _release_files(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


class PublishKagReleaseTests(unittest.TestCase):
    def test_builds_private_copy_and_verifies_full_release(self) -> None:
        with _synthetic_store() as (store_root, revision, root, source_bytes):
            kag_root = _fake_kag_root(root)
            release_root = root / "release"
            integration = build_release(store_root, revision, kag_root, release_root)

            self.assertEqual(integration["schema_version"], "tos_kag_integration_v1")
            self.assertEqual(integration["corpus_revision"], revision)
            self.assertEqual(integration["primary_source"]["identity"]["path"], PRIMARY)
            self.assertEqual(
                integration["primary_source"]["identity"]["content_hash"],
                hashlib.sha256(source_bytes[PRIMARY]).hexdigest(),
            )
            self.assertEqual(
                [entry["path"] for entry in integration["programs"]],
                list(PROGRAM_PATHS),
            )
            paths = {entry["path"] for entry in integration["files"]}
            self.assertIn("export/export.json", paths)
            self.assertIn("provider/Tree-of-Sophia/" + PRIMARY, paths)
            self.assertIn("provider/Tree-of-Sophia/kag/indexes/hot.json", paths)
            self.assertIn("artifacts/config.json", paths)
            for relative in (
                "kag/AGENTS.md",
                "kag/README.md",
                "kag/edges/source-return.json",
                "kag/indexes/source-routes.json",
                "kag/manifest.json",
                "kag/nodes/export-route.json",
                "kag/nodes/source-export.json",
                "kag/projections/source-return.json",
                "kag/receipts/publication-route.json",
            ):
                self.assertIn("provider/Tree-of-Sophia/" + relative, paths)
            self.assertNotIn("integration.json", paths)
            destination = release_root / "releases" / integration["integration_revision"]
            self.assertEqual(
                _verify_integration(destination, expected_revision=revision),
                integration,
            )
            status = status_release(release_root, revision)
            self.assertEqual(status["source_kind"], "corpus_revision")
            self.assertEqual(status["integration_revision"], integration["integration_revision"])
            self.assertEqual(status["primary_source"], integration["primary_source"])
            self.assertEqual(
                DownstreamStatus(release_root / "status", "kag").status(revision)["freshness"],
                "current",
            )
            self.assertEqual(
                verify_export(destination / "export"),
                json.loads((destination / "export" / "export.json").read_bytes()),
            )
            self.assertEqual(build_release(store_root, revision, kag_root, release_root), integration)

    def test_repeated_outputs_have_identical_release_bytes_and_revision(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            kag_root = _fake_kag_root(root)
            first_root = root / "release-first"
            second_root = root / "release-second"
            first = build_release(store_root, revision, kag_root, first_root)
            second = build_release(store_root, revision, kag_root, second_root)
            self.assertEqual(first, second)
            first_files = _release_files(first_root / "releases" / first["integration_revision"])
            second_files = _release_files(second_root / "releases" / second["integration_revision"])
            self.assertEqual(first_files, second_files)

    def test_owner_failures_do_not_publish(self) -> None:
        for mode in ("fail", "mutate", "mismatch", "control-mutate"):
            with self.subTest(mode=mode):
                with _synthetic_store() as (store_root, revision, root, _source_bytes):
                    kag_root = _fake_kag_root(root, mode=mode)
                    release_root = root / "release"
                    with self.assertRaises(CorpusStoreError):
                        build_release(store_root, revision, kag_root, release_root)
                    self.assertEqual(list((release_root / "releases").iterdir()), [])
                    status = DownstreamStatus(release_root / "status", "kag").status(revision)
                    self.assertEqual(status["freshness"], "missing")
                    self.assertEqual(status["state"]["latest"]["state"], "failed")

    def test_provider_home_rejects_missing_controlled_source_ref(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            kag_root = _fake_kag_root(root, mode="control-fail")
            release_root = root / "release"
            with self.assertRaisesRegex(CorpusStoreError, "validator probe failed"):
                build_release(store_root, revision, kag_root, release_root)
            self.assertEqual(list((release_root / "releases").iterdir()), [])
            status = DownstreamStatus(release_root / "status", "kag").status(revision)
            self.assertEqual(status["freshness"], "missing")
            self.assertEqual(status["state"]["latest"]["state"], "failed")

    def test_existing_release_is_verified_as_a_complete_membership(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            kag_root = _fake_kag_root(root)
            release_root = root / "release"
            first = build_release(store_root, revision, kag_root, release_root)
            destination = release_root / "releases" / first["integration_revision"]
            (destination / "unexpected.bin").write_bytes(b"tamper\n")
            with self.assertRaises(CorpusStoreError):
                build_release(store_root, revision, kag_root, release_root)
            self.assertTrue((destination / "unexpected.bin").exists())

    def test_prior_success_survives_a_later_failed_same_corpus(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            kag_root = _fake_kag_root(root)
            release_root = root / "release"
            first = build_release(store_root, revision, kag_root, release_root)
            (kag_root / "mode.json").write_bytes(canonical({"mode": "fail"}))
            with self.assertRaises(CorpusStoreError):
                build_release(store_root, revision, kag_root, release_root)
            destination = release_root / "releases" / first["integration_revision"]
            self.assertTrue(destination.is_dir())
            status = DownstreamStatus(release_root / "status", "kag").status(revision)
            self.assertEqual(status["freshness"], "current")
            self.assertEqual(
                status["state"]["last_success"]["artifact_revision"],
                first["integration_revision"],
            )
            self.assertEqual(status["state"]["latest"]["state"], "failed")

    def test_changed_program_yields_new_integration_and_preserves_old_release(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            kag_root = _fake_kag_root(root)
            release_root = root / "release"
            first = build_release(store_root, revision, kag_root, release_root)
            old_destination = release_root / "releases" / first["integration_revision"]
            old_bytes = _release_files(old_destination)
            query_program = kag_root / "scripts" / "query_repo_local_kag.py"
            query_program.write_text(query_program.read_text() + "# changed consumer\n")
            second = build_release(store_root, revision, kag_root, release_root)
            new_destination = release_root / "releases" / second["integration_revision"]
            self.assertNotEqual(first["integration_revision"], second["integration_revision"])
            self.assertTrue(old_destination.is_dir())
            self.assertTrue(new_destination.is_dir())
            self.assertEqual(_release_files(old_destination), old_bytes)
            self.assertEqual(
                _verify_integration(old_destination, expected_revision=revision),
                first,
            )

    def test_cli_build_and_status_are_json_subprocess_routes(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            kag_root = _fake_kag_root(root)
            release_root = root / "release"
            script = ROOT / "scripts" / "publish_kag_release.py"
            build = subprocess.run(
                [
                    sys.executable,
                    str(script),
                    "build",
                    "--store",
                    str(store_root),
                    "--revision",
                    revision,
                    "--kag-root",
                    str(kag_root),
                    "--release-root",
                    str(release_root),
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(build.returncode, 0, build.stderr)
            result = json.loads(build.stdout)
            destination = release_root / "releases" / result["integration_revision"]
            self.assertEqual(result["integration_revision"], json.loads(
                (destination / "integration.json").read_bytes()
            )["integration_revision"])
            status = subprocess.run(
                [
                    sys.executable,
                    str(script),
                    "status",
                    "--release-root",
                    str(release_root),
                    "--expected-revision",
                    revision,
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(status.returncode, 0, status.stderr)
            status_result = json.loads(status.stdout)
            self.assertEqual(status_result["freshness"], "current")
            self.assertEqual(status_result["source_kind"], "corpus_revision")
            self.assertEqual(
                status_result["integration_revision"],
                result["integration_revision"],
            )
            self.assertEqual(
                status_result["primary_source"],
                result["primary_source"],
            )

    def test_status_rejects_tampered_manifest_or_member(self) -> None:
        for tamper in ("manifest", "member"):
            with self.subTest(tamper=tamper):
                with _synthetic_store() as (store_root, revision, root, _source_bytes):
                    kag_root = _fake_kag_root(root)
                    release_root = root / "release"
                    integration = build_release(store_root, revision, kag_root, release_root)
                    destination = release_root / "releases" / integration["integration_revision"]
                    if tamper == "manifest":
                        manifest = destination / "integration.json"
                        manifest.write_bytes(manifest.read_bytes() + b"tampered\n")
                    else:
                        member = destination / "artifacts" / "config.json"
                        member.write_bytes(b"tampered member\n")
                    with self.assertRaises(CorpusStoreError):
                        status_release(release_root, revision)

    def test_missing_status_does_not_create_release_filesystem(self) -> None:
        with _synthetic_store() as (_store_root, revision, root, _source_bytes):
            release_root = root / "missing-release"
            before = sorted(path.name for path in root.iterdir())
            result = status_release(release_root, revision)
            self.assertEqual(result["freshness"], "missing")
            self.assertEqual(result["source_kind"], "corpus_revision")
            self.assertIsNone(result["integration_revision"])
            self.assertIsNone(result["primary_source"])
            self.assertFalse(release_root.exists())
            self.assertEqual(sorted(path.name for path in root.iterdir()), before)

    def test_stale_attempt_cannot_replace_newer_status(self) -> None:
        with _synthetic_store() as (_store_root, revision, root, _source_bytes):
            status = DownstreamStatus(root / "status", "kag")
            old_attempt = status.begin(revision)
            new_attempt = status.begin(revision)
            with self.assertRaisesRegex(ValueError, "stale"):
                status.fail(old_attempt, "old operation failed")
            current = status.status(revision)
            self.assertEqual(current["latest_attempt"]["attempt_id"], new_attempt)
            self.assertEqual(current["latest_attempt"]["state"], "running")


if __name__ == "__main__":
    unittest.main()
