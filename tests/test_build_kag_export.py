from __future__ import annotations

from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
from typing import Iterator
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from build_kag_export import (  # noqa: E402
    CAPSULE,
    PRIMARY,
    SOURCE_PATHS,
    build_export,
    verify_export,
)
from corpus_store import (  # noqa: E402
    CorpusCandidate,
    CorpusStore,
    CorpusStoreError,
    ValidationIndex,
    canonical,
)


VALIDATOR_SHA256 = "1" * 64


def _write_json(path: Path, value: object) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(canonical(value))
    return path


@contextmanager
def _synthetic_store() -> Iterator[tuple[Path, str, Path, dict[str, bytes]]]:
    temporary = tempfile.TemporaryDirectory(prefix="tos-kag-export-")
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


def _copy_export(source: Path, destination: Path) -> Path:
    shutil.copytree(source, destination)
    return destination


def _refresh_export_identity(export: Path) -> None:
    manifest_path = export / "export.json"
    manifest = json.loads(manifest_path.read_bytes())
    body = {key: value for key, value in manifest.items() if key != "export_revision"}
    manifest["export_revision"] = hashlib.sha256(canonical(body)).hexdigest()
    manifest_path.write_bytes(canonical(manifest))


def _mutate_capsule(export: Path, mutate) -> None:
    capsule_path = export / "Tree-of-Sophia" / CAPSULE
    capsule = json.loads(capsule_path.read_bytes())
    mutate(capsule)
    capsule_path.write_bytes(canonical(capsule))
    manifest_path = export / "export.json"
    manifest = json.loads(manifest_path.read_bytes())
    capsule_entry = next(entry for entry in manifest["files"] if entry["path"] == CAPSULE)
    capsule_entry["sha256"] = hashlib.sha256(capsule_path.read_bytes()).hexdigest()
    capsule_entry["size_bytes"] = capsule_path.stat().st_size
    manifest_path.write_bytes(canonical(manifest))
    _refresh_export_identity(export)


class BuildKagExportTests(unittest.TestCase):
    def test_build_verify_and_primary_source_return_are_exact(self) -> None:
        with _synthetic_store() as (store_root, revision, root, source_bytes):
            output = root / "exports" / "tiny"
            manifest = build_export(store_root, revision, output)
            verified = verify_export(output)

            self.assertEqual(verified, manifest)
            self.assertEqual(manifest["corpus_revision"], revision)
            primary_digest = hashlib.sha256(source_bytes[PRIMARY]).hexdigest()
            self.assertEqual(
                manifest["primary_source"],
                {
                    "record_id": "tiny-node",
                    "path": PRIMARY,
                    "sha256": primary_digest,
                    "corpus_revision": revision,
                },
            )
            self.assertIn(CAPSULE, {entry["path"] for entry in manifest["files"]})
            self.assertEqual(json.loads((output / "Tree-of-Sophia" / CAPSULE).read_bytes())["object_id"], "tiny-node")

    def test_repeated_revision_has_same_identity_and_does_not_overwrite(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            first = root / "exports" / "first"
            second = root / "exports" / "second"
            first_manifest = build_export(store_root, revision, first)
            second_manifest = build_export(store_root, revision, second)

            self.assertEqual(first_manifest, second_manifest)
            self.assertEqual(first_manifest["export_revision"], second_manifest["export_revision"])
            with self.assertRaisesRegex(CorpusStoreError, "new regular path"):
                build_export(store_root, revision, first)

    def test_corrupt_cas_is_rejected_before_publishing_output(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            store = CorpusStore(store_root)
            manifest = store.load(revision)
            entry = next(item for item in manifest["files"] if item["path"] == PRIMARY)
            object_path = store._object(entry["sha256"])
            os.chmod(object_path, 0o644)
            try:
                object_path.write_bytes(b"corrupt CAS bytes\n")
            finally:
                os.chmod(object_path, 0o444)
            output = root / "exports" / "corrupt"

            with self.assertRaisesRegex(CorpusStoreError, "corrupt corpus object"):
                build_export(store_root, revision, output)
            self.assertFalse(output.exists())

    def test_verify_rejects_member_alteration_extra_member_and_symlink(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            original = root / "exports" / "original"
            build_export(store_root, revision, original)

            altered = _copy_export(original, root / "exports" / "altered")
            altered_file = altered / "Tree-of-Sophia" / SOURCE_PATHS[1]
            altered_file.write_bytes(altered_file.read_bytes() + b"changed\n")
            with self.assertRaisesRegex(CorpusStoreError, "digest mismatch"):
                verify_export(altered)

            extra = _copy_export(original, root / "exports" / "extra")
            (extra / "extra.bin").write_bytes(b"undeclared\n")
            with self.assertRaisesRegex(CorpusStoreError, "undeclared files"):
                verify_export(extra)

            linked = _copy_export(original, root / "exports" / "linked")
            linked_file = linked / "Tree-of-Sophia" / SOURCE_PATHS[2]
            outside = root / "outside.json"
            outside.write_bytes(linked_file.read_bytes())
            linked_file.unlink()
            linked_file.symlink_to(outside)
            with self.assertRaisesRegex(CorpusStoreError, "linked corpus path|symlinks"):
                verify_export(linked)

    def test_incompatible_source_node_id_is_rejected(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            original = root / "exports" / "original"
            build_export(store_root, revision, original)
            altered = _copy_export(original, root / "exports" / "incompatible")
            node_path = altered / "Tree-of-Sophia" / PRIMARY
            node = json.loads(node_path.read_bytes())
            node["node_id"] = "incompatible-node"
            node_path.write_bytes(canonical(node))
            manifest_path = altered / "export.json"
            manifest = json.loads(manifest_path.read_bytes())
            source_entry = next(
                entry for entry in manifest["files"] if entry["path"] == PRIMARY
            )
            source_entry["sha256"] = hashlib.sha256(node_path.read_bytes()).hexdigest()
            source_entry["size_bytes"] = node_path.stat().st_size
            manifest_path.write_bytes(canonical(manifest))
            _refresh_export_identity(altered)

            with self.assertRaisesRegex(CorpusStoreError, "canonical source"):
                verify_export(altered)

    def test_source_return_binding_mutations_are_rejected_after_rehash(self) -> None:
        with _synthetic_store() as (store_root, revision, root, _source_bytes):
            original = root / "exports" / "original"
            build_export(store_root, revision, original)
            cases = (
                (
                    "entry match value",
                    lambda capsule: capsule["entry_surface"].update(
                        {"match_value": "incompatible-node"}
                    ),
                    "invalid KAG source-return capsule",
                ),
                (
                    "section handles",
                    lambda capsule: capsule.update({"section_handles": ["other-layer"]}),
                    "invalid KAG source-return capsule",
                ),
                (
                    "relation target",
                    lambda capsule: capsule["direct_relations"][0].update(
                        {
                            "target_ref": (
                                "Tree-of-Sophia/ToS/public-compatibility/"
                                "source_node.example.json"
                            )
                        }
                    ),
                    "exact exported source contract",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    altered = _copy_export(original, root / "exports" / name.replace(" ", "-"))
                    _mutate_capsule(altered, mutate)
                    with self.assertRaisesRegex(CorpusStoreError, message):
                        verify_export(altered)


if __name__ == "__main__":
    unittest.main()
