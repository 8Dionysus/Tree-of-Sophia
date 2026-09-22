from __future__ import annotations

import hashlib
import json
from pathlib import Path
from pathlib import PurePosixPath
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import convert_acquired_corpus_batches as converter  # noqa: E402
import corpus_grammar_overlay_preflight as grammar_preflight  # noqa: E402
import corpus_historical_evidence_preflight as history_preflight  # noqa: E402


def _canonical(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("utf-8")


class SnapshotReaderBoundaryTests(unittest.TestCase):
    def test_all_intake_readers_accept_unicode_snapshot_paths(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-intake-unicode-") as raw:
            snapshot = Path(raw) / "snapshot.json"
            entry = {
                "path": "ToS/source-witnesses/works/пример/work.json",
                "sha256": hashlib.sha256(b"fixture").hexdigest(),
                "size_bytes": 7,
                "mode": 0o644,
            }
            snapshot.write_bytes(_canonical({"files": [entry]}))

            readers = (
                converter._iter_snapshot_file_entries,
                grammar_preflight._snapshot_files,
                history_preflight._iter_snapshot_files,
            )
            for reader in readers:
                with self.subTest(reader=reader.__module__ + "." + reader.__name__):
                    self.assertEqual(list(reader(snapshot)), [entry])


class HistoricalContextBoundaryTests(unittest.TestCase):
    BASE = "e6b296b17d9e91bc444caf020b976f6a0e5e6f026f1992474bfa2977420407ea"

    def test_converter_rejects_unpaired_context_before_materialization(self) -> None:
        with self.assertRaisesRegex(converter.ConversionError, "paired capture/root"):
            converter.convert(
                acquisition_root=Path("/does/not/need/to/be/read"),
                output_root=Path("/does/not/need/to/be/written"),
                store_root=Path("/does/not/need/to/be/read"),
                base_revision=self.BASE,
                grammar_root=Path("/does/not/need/to/be/read"),
                historical_capture=[Path("capture")],
                historical_root=[],
            )

    def test_grammar_preflight_omits_empty_context_and_forwards_pairs(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-intake-context-") as raw:
            root = Path(raw)
            store = root / "store"
            revision = store / "revisions" / self.BASE
            revision.mkdir(parents=True)
            grammar = root / "grammar"
            contract = grammar / "ToS/contracts/tiny.json"
            contract.parent.mkdir(parents=True)
            contract.write_bytes(_canonical({"type": "object"}))
            (grammar / "ToS/doctrine/semantic-interchange").mkdir(parents=True)
            input_root = root / "input"
            input_root.mkdir()
            batch_path = root / "batch.json"
            batch_path.write_bytes(b"fixture\n")
            snapshot_entry = {
                "path": "ToS/contracts/tiny.json",
                "sha256": hashlib.sha256(contract.read_bytes()).hexdigest(),
                "size_bytes": contract.stat().st_size,
                "mode": 0o644,
            }
            (revision / "snapshot.json").write_bytes(
                _canonical({"files": [snapshot_entry]})
            )
            (store / "current.json").write_bytes(
                _canonical({"current": self.BASE})
            )
            batch = {
                "schema_version": "tos_corpus_batch_v1",
                "base_revision": self.BASE,
                "validator_sha256": "a" * 64,
                "updates": [],
                "retirements": [],
            }
            calls: list[dict] = []

            class Validator:
                sha256 = "a" * 64
                grammar_sha256 = "b" * 64
                evidence: list[dict] = []

                def __init__(self, _grammar: Path, **kwargs: object) -> None:
                    calls.append(kwargs)

            def fake_read_json(_path: Path) -> dict:
                return {"current": self.BASE}

            with patch.object(
                grammar_preflight,
                "_load_program",
                return_value=(
                    lambda _batch_path, _input_root: (batch, {}, {}),
                    Validator,
                    RuntimeError,
                    fake_read_json,
                ),
            ):
                no_history = grammar_preflight.run_preflight(
                    store_root=store,
                    base_revision=self.BASE,
                    batch_path=batch_path,
                    input_root=input_root,
                    grammar_root=grammar,
                    software_root=root,
                )
                self.assertTrue(no_history["ok"])
                self.assertEqual(calls[-1], {})

                empty_history = grammar_preflight.run_preflight(
                    store_root=store,
                    base_revision=self.BASE,
                    batch_path=batch_path,
                    input_root=input_root,
                    grammar_root=grammar,
                    software_root=root,
                    historical_captures=[],
                    historical_roots=[],
                )
                self.assertTrue(empty_history["ok"])
                self.assertEqual(calls[-1], {})

                paired = grammar_preflight.run_preflight(
                    store_root=store,
                    base_revision=self.BASE,
                    batch_path=batch_path,
                    input_root=input_root,
                    grammar_root=grammar,
                    software_root=root,
                    historical_captures=[root / "capture"],
                    historical_roots=[root / "history"],
                )
                self.assertTrue(paired["ok"])
                self.assertEqual(
                    calls[-1],
                    {
                        "historical_capture": [root / "capture"],
                        "historical_root": [root / "history"],
                    },
                )

    def test_grammar_preflight_rejects_unpaired_context_before_reads(self) -> None:
        with self.assertRaisesRegex(grammar_preflight.PreflightError, "one capture"):
            grammar_preflight.run_preflight(
                store_root=Path("/does/not/need/to/be/read"),
                base_revision=self.BASE,
                batch_path=Path("/does/not/need/to/be/read"),
                input_root=Path("/does/not/need/to/be/read"),
                grammar_root=Path("/does/not/need/to/be/read"),
                software_root=Path("/does/not/need/to/be/read"),
                historical_captures=[Path("capture")],
                historical_roots=None,
            )


class DirectHandoffBoundaryTests(unittest.TestCase):
    BASE = "e6b296b17d9e91bc444caf020b976f6a0e5e6f026f1992474bfa2977420407ea"

    def test_handoff_selector_binds_multiple_immutable_inputs(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-handoff-selection-") as raw:
            root = Path(raw)
            handoffs = []
            for index in range(2):
                handoff_root = root / f"handoff-{index}"
                (handoff_root / "receipts").mkdir(parents=True)
                handoff_path = handoff_root / "receipts" / "handoff.json"
                handoff_path.write_bytes(f"handoff-{index}\n".encode())
                handoffs.append(
                    {
                        "root": str(handoff_root),
                        "handoff_ref": "receipts/handoff.json",
                        "sha256": hashlib.sha256(handoff_path.read_bytes()).hexdigest(),
                    }
                )
            selection_path = root / "selection.json"
            selection_path.write_bytes(
                _canonical(
                    {
                        "schema_version": converter.HANDOFF_SELECTION_SCHEMA,
                        "selection_id": "two-producers",
                        "handoffs": handoffs,
                    }
                )
            )
            rows, selection = converter._load_handoff_selection(selection_path)
            self.assertEqual("two-producers", selection["selection_id"])
            self.assertEqual(2, len(rows))
            self.assertEqual({"receipts/handoff.json"}, {row["handoff_ref"] for row in rows})

    def test_direct_loader_calls_shared_verifier_and_checks_selector_digest(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-handoff-verifier-") as raw:
            root = Path(raw)
            (root / "receipts").mkdir(parents=True)
            (root / "source").mkdir()
            (root / "payload").mkdir()
            handoff_path = root / "receipts" / "handoff.json"
            handoff_path.write_bytes(b"immutable handoff\n")
            digest = hashlib.sha256(handoff_path.read_bytes()).hexdigest()
            calls: list[dict[str, object]] = []
            fake = SimpleNamespace(
                verify_handoff_for_intake=lambda **kwargs: (
                    calls.append(kwargs)
                    or SimpleNamespace(
                        handoff={
                            "batch_id": "tos.acquisition-batch.fixture",
                            "batch_revision": 1,
                        },
                        context=SimpleNamespace(
                            manifest={
                                "batch_id": "tos.acquisition-batch.fixture",
                                "batch_revision": 1,
                            }
                        ),
                        selected_source_rows=[],
                        payloads=[],
                    )
                )
            )
            with patch.dict(sys.modules, {"acquisition_handoff_adapter": fake}):
                normalized = converter._load_direct_handoff(
                    root=root,
                    handoff_ref="receipts/handoff.json",
                    expected_sha256=digest,
                    base_revision=self.BASE,
                )
            self.assertEqual(1, len(calls))
            self.assertEqual(self.BASE, calls[0]["expected_base_revision"])
            self.assertEqual("tos.acquisition-batch.fixture", normalized.manifest["batch_id"])
            with patch.dict(sys.modules, {"acquisition_handoff_adapter": fake}):
                with self.assertRaisesRegex(converter.ConversionError, "digest changed"):
                    converter._load_direct_handoff(
                        root=root,
                        handoff_ref="receipts/handoff.json",
                        expected_sha256="0" * 64,
                        base_revision=self.BASE,
                    )

    def test_handoff_claim_index_rejects_divergent_duplicate_ids(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-handoff-claims-") as raw:
            root = Path(raw)
            relation = root / "ToS/source-witnesses/relations/work-expression"
            relation.mkdir(parents=True)
            path = relation / "work-expression-claims.jsonl"
            first = {"claim_id": "claim.same", "evidence_refs": ["ToS/source-witnesses/works/a/work.json"]}
            path.write_bytes(_canonical(first))
            other = root / "other"
            other_relation = other / "ToS/source-witnesses/relations/work-expression"
            other_relation.mkdir(parents=True)
            (other_relation / path.name).write_bytes(
                _canonical({**first, "predicate": "has_expression"})
            )
            with self.assertRaisesRegex(converter.ConversionError, "differs across handoffs"):
                converter._index_handoff_topology_claims(
                    [("a", root), ("b", other)]
                )

    def test_handoff_claim_index_normalizes_batch_local_streams(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-handoff-local-claims-") as raw:
            root = Path(raw)
            relation = root / "ToS/source-witnesses/relations/russian-batch/claims"
            relation.mkdir(parents=True)
            path = relation / "work-expression-claims.jsonl"
            path.write_bytes(
                _canonical(
                    {
                        "claim_id": "claim.batch-local",
                        "evidence_refs": [
                            "ToS/source-witnesses/works/a/work.json"
                        ],
                    }
                )
            )

            indexed = converter._index_handoff_topology_claims([("batch", root)])

            self.assertEqual(
                "ToS/source-witnesses/relations/work-expression/work-expression-claims.jsonl",
                indexed["claim.batch-local"][0],
            )

    def test_direct_candidates_exclude_source_prefixed_operational_delta(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-handoff-delta-") as raw:
            root = Path(raw)
            source = root / "source"
            (source / "ToS/source-witnesses/discovery/test").mkdir(parents=True)
            kept = source / "ToS/source-witnesses/works/new/work.json"
            kept.parent.mkdir(parents=True)
            kept.write_bytes(b"{}\n")
            delta = source / "ToS/source-witnesses/discovery/test/provenance-delta.json"
            delta.write_bytes(b"{}\n")
            candidates = converter._source_candidates(
                None,
                set(),
                [],
                handoff_source_roots=[("fixture", source)],
                excluded_paths={"ToS/source-witnesses/discovery/test/provenance-delta.json"},
            )
            self.assertIn(kept.relative_to(source).as_posix(), candidates)
            self.assertNotIn(delta.relative_to(source).as_posix(), candidates)

    def test_convert_preserves_same_file_id_at_distinct_item_destinations(self) -> None:
        """The direct closure pass counts bindings, not only content IDs."""

        with tempfile.TemporaryDirectory(prefix="tos-handoff-convert-") as raw:
            root = Path(raw)
            store = root / "store"
            base = "a" * 64
            revision = store / "revisions" / base
            objects = store / "objects"
            revision.mkdir(parents=True)
            objects.mkdir()

            claim_paths = sorted(converter.CLAIM_SUFFIXES)
            accepted_files: list[dict[str, object]] = []
            for suffix in claim_paths:
                relative = (
                    "ToS/source-witnesses/relations/"
                    f"{converter.CLAIM_SUFFIXES[suffix]}/{suffix}"
                )
                body = b""
                digest = hashlib.sha256(body).hexdigest()
                (objects / digest).write_bytes(body)
                accepted_files.append(
                    {
                        "path": relative,
                        "sha256": digest,
                        "size_bytes": 0,
                        "mode": 0o644,
                    }
                )

            event_relative = converter.TOPOLOGY_EVENT_PATH
            event = {
                "event_id": "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31",
                "event_version": 1,
                "inputs": [],
                "outputs": [],
                "method": {"configuration": {}},
            }
            event_body = _canonical(event)
            event_digest = hashlib.sha256(event_body).hexdigest()
            (objects / event_digest).write_bytes(event_body)
            accepted_files.append(
                {
                    "path": event_relative,
                    "sha256": event_digest,
                    "size_bytes": len(event_body),
                    "mode": 0o644,
                }
            )
            snapshot = {
                "schema_version": "tos_corpus_snapshot_v1",
                "base_revision": None,
                "validator_sha256": "b" * 64,
                "files": sorted(accepted_files, key=lambda row: row["path"]),
                "identities": {},
                "dependencies": {},
                "retirements": [],
                "revision": base,
            }
            (revision / "snapshot.json").write_bytes(_canonical(snapshot))
            (store / "current.json").write_bytes(
                _canonical(
                    {
                        "schema_version": "tos_corpus_pointer_v1",
                        "current": base,
                        "previous": None,
                    }
                )
            )

            handoffs: dict[str, converter.DirectHandoff] = {}
            selector_rows: list[dict[str, str]] = []
            shared_body = b"shared content-addressed file\n"
            shared_digest = hashlib.sha256(shared_body).hexdigest()

            for index in range(2):
                batch_root = root / f"handoff-{index}"
                source_root = batch_root / "source"
                payload_root = batch_root / "payload"
                source_root.mkdir(parents=True)
                payload_root.mkdir()
                batch_id = f"tos.acquisition-batch.fixture-{index}"
                item_root = f"ToS/source-witnesses/works/fixture/item-{index}"
                item_ref = f"tos.item.fixture.item-{index}"
                rights_ref = f"{item_root}/rights.json"
                provenance_ref = f"{item_root}/provenance.jsonl"
                manifest_ref = f"{item_root}/item.manifest.json"
                payload = {
                    "item_ref": item_ref,
                    "file_ref": f"tos.file.sha256.{shared_digest}",
                    "item_root_ref": item_root,
                    "relative_path": "payload/shared.txt",
                    "byte_size": len(shared_body),
                    "sha256": shared_digest,
                }
                item_manifest = {
                    "schema_version": "tos_source_item_manifest_v1",
                    "item_id": item_ref,
                    "rights_ref": rights_ref,
                    "provenance_ref": provenance_ref,
                    "payload_files": [payload],
                }
                source_values = {
                    f"{item_root}/item.json": _canonical({"item_id": item_ref}),
                    manifest_ref: _canonical(item_manifest),
                    rights_ref: _canonical({"rights_id": f"tos.rights.fixture.item-{index}"}),
                    provenance_ref: _canonical(
                        {
                            "rights_basis_ref": rights_ref,
                            "outputs": [
                                {
                                    "ref": f"{item_root}/payload/shared.txt",
                                    "sha256": shared_digest,
                                }
                            ],
                        }
                    ),
                }
                records: list[dict[str, str]] = []
                for relative, body in source_values.items():
                    path = source_root / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(body)
                    records.append(
                        {
                            "ref": relative,
                            "sha256": hashlib.sha256(body).hexdigest(),
                        }
                    )
                delta_relative = "ToS/source-witnesses/discovery/provenance-delta.json"
                (source_root / delta_relative).parent.mkdir(parents=True, exist_ok=True)
                (source_root / delta_relative).write_bytes(b"operational\n")
                payload_path = payload_root / PurePosixPath(item_root).relative_to(
                    "ToS/source-witnesses"
                ) / "payload/shared.txt"
                payload_path.parent.mkdir(parents=True, exist_ok=True)
                payload_path.write_bytes(shared_body)
                handoff_path = batch_root / "receipts/handoff.json"
                handoff_path.parent.mkdir(parents=True)
                handoff_path.write_bytes(b"handoff\n")
                manifest_path = batch_root / "manifest.json"
                manifest_path.write_bytes(b"{}\n")
                handoff = {
                    "batch_id": batch_id,
                    "provenance_delta": {"ref": f"source/{delta_relative}"},
                }
                handoffs[batch_id] = converter.DirectHandoff(
                    root=batch_root,
                    handoff_ref="receipts/handoff.json",
                    handoff_sha256=hashlib.sha256(handoff_path.read_bytes()).hexdigest(),
                    handoff_path=handoff_path,
                    handoff=handoff,
                    manifest_path=manifest_path,
                    manifest={"batch_id": batch_id},
                    source_root=source_root,
                    payload_root=payload_root,
                    source_records=records,
                    payloads=[payload],
                    context=None,
                )
                selector_rows.append(
                    {
                        "root": str(batch_root),
                        "handoff_ref": "receipts/handoff.json",
                        "sha256": handoffs[batch_id].handoff_sha256,
                    }
                )

            selection = root / "selection.json"
            selection.write_bytes(
                _canonical(
                    {
                        "schema_version": converter.HANDOFF_SELECTION_SCHEMA,
                        "selection_id": "same-file-two-items",
                        "handoffs": selector_rows,
                    }
                )
            )
            grammar = root / "grammar"
            (grammar / "ToS/contracts").mkdir(parents=True)
            (grammar / "ToS/contracts/tiny.json").write_bytes(_canonical({"type": "object"}))

            class Validator:
                sha256 = "b" * 64

                def __init__(self, _grammar: Path, **_kwargs: object) -> None:
                    pass

            fake_module = SimpleNamespace(
                SourceValidator=Validator,
                verify_validation_context=lambda value, **_kwargs: value,
            )
            fake_adapter = SimpleNamespace(
                verify_validation_context=lambda value, **_kwargs: value,
            )

            def load_handoff(*, root: Path, handoff_ref: str, expected_sha256: str, base_revision: str):
                del handoff_ref, expected_sha256, base_revision
                batch_id = root.name
                return handoffs[f"tos.acquisition-batch.fixture-{batch_id.removeprefix('handoff-')}"]

            output = root / "candidate"
            with patch.dict(
                sys.modules,
                {
                    "corpus_source_validation": fake_module,
                    "acquisition_handoff_adapter": fake_adapter,
                },
            ), patch.object(converter, "_load_direct_handoff", side_effect=load_handoff):
                receipt = converter.convert(
                    acquisition_root=None,
                    handoff_selection=selection,
                    output_root=output,
                    store_root=store,
                    base_revision=base,
                    grammar_root=grammar,
                )

            self.assertEqual(2, receipt["batch_count"])
            self.assertEqual(2, receipt["payload_count"])
            self.assertEqual(1, receipt["payload_unique_file_count"])
            update_paths = {
                row["path"]
                for row in json.loads(
                    (output / receipt["candidate_batch_ref"]).read_text()
                )["updates"]
            }
            self.assertNotIn("ToS/source-witnesses/discovery/provenance-delta.json", update_paths)
            for index in range(2):
                self.assertEqual(
                    shared_body,
                    (
                        output
                        / "payload"
                        / "works"
                        / f"fixture/item-{index}/payload/shared.txt"
                    ).read_bytes(),
                )



if __name__ == "__main__":
    unittest.main()
