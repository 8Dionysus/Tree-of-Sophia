from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import tempfile
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


if __name__ == "__main__":
    unittest.main()
