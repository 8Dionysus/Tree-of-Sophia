from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

import corpus_admit as admission  # noqa: E402
from corpus_admit import BATCH_KEYS, BATCH_SCHEMA, is_source_member, read_batch  # noqa: E402
from corpus_store import CorpusCandidate, CorpusStore, CorpusStoreError, ValidationIndex, canonical  # noqa: E402


class FixedValidator:
    """A wiring stub, not evidence of full source validation."""

    def __init__(self, identities: dict[str, str], *, sha256: str) -> None:
        self.identities = dict(identities)
        self.sha256 = sha256
        self.calls: list[tuple[CorpusCandidate, dict | None, frozenset[str]]] = []

    def __call__(
        self,
        candidate: CorpusCandidate,
        base: dict | None,
        affected: frozenset[str],
    ) -> ValidationIndex:
        self.calls.append((candidate, base, affected))
        return ValidationIndex(dict(self.identities), {})


class CorpusAdmissionTests(unittest.TestCase):
    VALIDATOR_SHA = "1" * 64

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="tos-corpus-admit-")
        self.root = Path(self.temporary.name)
        self.input_root = self.root / "input"
        self.input_root.mkdir()
        self.grammar_root = self.root / "grammar"
        self.grammar_root.mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _source(
        self,
        relative: str,
        payload: bytes = b"fixture source\n",
        *,
        mode: int = 0o644,
    ) -> dict:
        path = self.input_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        path.chmod(mode)
        return {
            "path": relative,
            "sha256": hashlib.sha256(payload).hexdigest(),
            "size_bytes": len(payload),
            "mode": mode,
        }

    def _batch(
        self,
        updates: list[dict],
        *,
        base_revision: str | None = None,
        retirements: list[dict] | None = None,
        validator_sha: str = VALIDATOR_SHA,
    ) -> dict:
        return {
            "schema_version": BATCH_SCHEMA,
            "base_revision": base_revision,
            "validator_sha256": validator_sha,
            "updates": updates,
            "retirements": [] if retirements is None else retirements,
        }

    def _write_batch(self, name: str, batch: dict) -> Path:
        path = self.root / name
        path.write_bytes(canonical(batch))
        return path

    def _admit(
        self,
        name: str,
        batch: dict,
        identities: dict[str, str],
        *,
        validator_sha: str = VALIDATOR_SHA,
    ) -> tuple[dict, CorpusStore, FixedValidator]:
        batch_path = self._write_batch(name, batch)
        store_root = self.root / "store"
        validator = FixedValidator(identities, sha256=validator_sha)
        with patch.object(admission, "SourceValidator", return_value=validator):
            receipt = admission.admit_batch(
                store_root,
                batch_path,
                self.input_root,
                self.grammar_root,
            )
        return receipt, CorpusStore(store_root), validator

    def test_read_batch_requires_canonical_strict_json(self) -> None:
        row = self._source("ToS/source-witnesses/works/fixture.md")
        batch = self._batch([row])
        parsed, updates, retirements = read_batch(
            self._write_batch("valid.json", batch), self.input_root
        )
        self.assertEqual(parsed, batch)
        self.assertEqual(retirements, {})
        self.assertEqual(updates[row["path"]]["sha256"], row["sha256"])
        self.assertEqual(updates[row["path"]]["size_bytes"], row["size_bytes"])
        self.assertEqual(updates[row["path"]]["mode"], row["mode"])
        self.assertEqual(updates[row["path"]]["source"], self.input_root / row["path"])

        noncanonical = self.root / "noncanonical.json"
        noncanonical.write_text(json.dumps(batch, indent=2) + "\n", encoding="utf-8")
        with self.assertRaisesRegex(CorpusStoreError, "canonical"):
            read_batch(noncanonical, self.input_root)

        duplicate = self.root / "duplicate.json"
        duplicate.write_text(
            '{"base_revision":null,"retirements":[],"schema_version":'
            '"tos_corpus_batch_v1","updates":[],"updates":[],'
            f'"validator_sha256":"{self.VALIDATOR_SHA}"}}\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(CorpusStoreError, "canonical"):
            read_batch(duplicate, self.input_root)

        extra = dict(batch)
        extra["unexpected"] = True
        with self.assertRaisesRegex(
            CorpusStoreError,
            "unsupported source batch",
        ):
            read_batch(self._write_batch("extra.json", extra), self.input_root)
        self.assertEqual(
            set(BATCH_KEYS),
            {"schema_version", "base_revision", "validator_sha256", "updates", "retirements"},
        )

    def test_read_batch_rejects_duplicate_paths_and_retirement_conflicts(self) -> None:
        row = self._source("ToS/source-witnesses/works/fixture.md")
        duplicate_updates = self._batch([row, dict(row)])
        with self.assertRaisesRegex(CorpusStoreError, "duplicate"):
            read_batch(self._write_batch("duplicate-updates.json", duplicate_updates), self.input_root)

        retirement = {
            "path": row["path"],
            "event_ref": "ToS/source-witnesses/works/event.json",
            "event_sha256": "a" * 64,
        }
        duplicate_retirements = self._batch([], retirements=[retirement, dict(retirement)])
        with self.assertRaisesRegex(CorpusStoreError, "duplicate"):
            read_batch(
                self._write_batch("duplicate-retirements.json", duplicate_retirements),
                self.input_root,
            )

        conflict = self._batch([row], retirements=[retirement])
        with self.assertRaisesRegex(CorpusStoreError, "conflicting"):
            read_batch(self._write_batch("update-retirement-conflict.json", conflict), self.input_root)

    def test_read_batch_requires_exact_update_and_retirement_fields(self) -> None:
        row = self._source("ToS/source-witnesses/works/fixture.md")
        missing_mode = dict(row)
        del missing_mode["mode"]
        with self.assertRaisesRegex(CorpusStoreError, "exact bytes and mode"):
            read_batch(
                self._write_batch("missing-mode.json", self._batch([missing_mode])),
                self.input_root,
            )

        extra_field = {**row, "extra": "not allowed"}
        with self.assertRaisesRegex(CorpusStoreError, "exact bytes and mode"):
            read_batch(
                self._write_batch("extra-update-field.json", self._batch([extra_field])),
                self.input_root,
            )

        missing_event = {"path": row["path"]}
        with self.assertRaisesRegex(CorpusStoreError, "exact event binding"):
            read_batch(
                self._write_batch("missing-event.json", self._batch([], retirements=[missing_event])),
                self.input_root,
            )

        extra_event = {
            "path": row["path"],
            "event_ref": "ToS/source-witnesses/works/event.json",
            "event_sha256": "a" * 64,
            "extra": True,
        }
        with self.assertRaisesRegex(CorpusStoreError, "exact event binding"):
            read_batch(
                self._write_batch("extra-event.json", self._batch([], retirements=[extra_event])),
                self.input_root,
            )

        bad_digest = {
            "path": row["path"],
            "event_ref": "ToS/source-witnesses/works/event.json",
            "event_sha256": "not-a-sha",
        }
        with self.assertRaisesRegex(CorpusStoreError, "SHA-256"):
            read_batch(
                self._write_batch("bad-event-digest.json", self._batch([], retirements=[bad_digest])),
                self.input_root,
            )

    def test_read_batch_rejects_traversal_symlink_and_missing_inputs(self) -> None:
        for index, relative in enumerate(("../escape.md", "/absolute.md", "ToS\\escape.md")):
            row = {
                "path": relative,
                "sha256": "a" * 64,
                "size_bytes": 1,
                "mode": 0o644,
            }
            with self.subTest(relative=relative):
                with self.assertRaises(CorpusStoreError):
                    read_batch(
                        self._write_batch(f"unsafe-{index}.json", self._batch([row])),
                        self.input_root,
                    )

        missing = self._source("ToS/source-witnesses/works/missing.md")
        (self.input_root / missing["path"]).unlink()
        with self.assertRaisesRegex(CorpusStoreError, "existing regular"):
            read_batch(self._write_batch("missing-input.json", self._batch([missing])), self.input_root)

        linked_relative = "ToS/source-witnesses/works/linked.md"
        linked = self.input_root / linked_relative
        external = self.root / "external.md"
        external.write_bytes(b"outside\n")
        linked.parent.mkdir(parents=True, exist_ok=True)
        linked.symlink_to(external)
        linked_row = {
            "path": linked_relative,
            "sha256": hashlib.sha256(external.read_bytes()).hexdigest(),
            "size_bytes": external.stat().st_size,
            "mode": 0o644,
        }
        with self.assertRaisesRegex(CorpusStoreError, "existing regular"):
            read_batch(
                self._write_batch("linked-input.json", self._batch([linked_row])),
                self.input_root,
            )

        linked_root = self.root / "input-link"
        linked_root.symlink_to(self.input_root, target_is_directory=True)
        valid = self._source("ToS/source-witnesses/works/valid.md")
        with self.assertRaisesRegex(CorpusStoreError, "explicit regular directory"):
            read_batch(
                self._write_batch("linked-root.json", self._batch([valid])),
                linked_root,
            )

    def test_source_member_boundary_retains_authored_markdown_only(self) -> None:
        allowed = (
            "ToS/source-witnesses/catalog/authored.md",
            "ToS/source-witnesses/works/authored.md",
            "ToS/derived-exports/authored.md",
        )
        for relative in allowed:
            self.assertTrue(is_source_member(relative), relative)

        rejected = (
            "ToS/source-witnesses/catalog/generated.json",
            "ToS/source-witnesses/works/payload/source.md",
            "ToS/source-witnesses/works/owner-local/source.md",
            "ToS/derived-exports/generated.json",
            "ToS/derived-exports/projection.bin",
            "access/README.md",
        )
        for relative in rejected:
            self.assertFalse(is_source_member(relative), relative)

        for index, relative in enumerate(rejected):
            row = self._source(relative, f"rejected {index}\n".encode())
            with self.subTest(rejected_path=relative):
                with self.assertRaisesRegex(CorpusStoreError, "non-source"):
                    read_batch(
                        self._write_batch(
                            f"rejected-{index}.json",
                            self._batch([row]),
                        ),
                        self.input_root,
                    )

        rows = [self._source(relative) for relative in allowed]
        _, updates, _ = read_batch(
            self._write_batch("authored-markdown.json", self._batch(rows)),
            self.input_root,
        )
        self.assertEqual(set(updates), set(allowed))

    def test_admit_batch_wires_exact_bytes_modes_and_store_revision(self) -> None:
        rows = [
            self._source(
                "ToS/source-witnesses/catalog/authored.md",
                b"catalog source\n",
                mode=0o755,
            ),
            self._source(
                "ToS/derived-exports/authored.md",
                b"derived authored source\n",
            ),
        ]
        identities = {"fixture:catalog": rows[0]["path"], "fixture:derived": rows[1]["path"]}
        receipt, store, validator = self._admit(
            "valid-admission.json",
            self._batch(rows),
            identities,
        )
        self.assertEqual(receipt["schema_version"], "tos_corpus_admission_receipt_v1")
        self.assertEqual(receipt["validator_sha256"], self.VALIDATOR_SHA)
        self.assertEqual(receipt["members"], 2)
        self.assertEqual(receipt["identities"], 2)
        self.assertEqual(receipt["source_bytes"], sum(row["size_bytes"] for row in rows))
        self.assertFalse(receipt["semantic_admission"])
        self.assertFalse(receipt["rights_change"])
        self.assertEqual(receipt["revision"], store.current())
        self.assertEqual(len(validator.calls), 1)

        snapshot = store.load(receipt["revision"], verify_objects=True)
        self.assertEqual(snapshot["validator_sha256"], self.VALIDATOR_SHA)
        self.assertEqual(
            [(entry["path"], entry["mode"]) for entry in snapshot["files"]],
            sorted((row["path"], row["mode"]) for row in rows),
        )
        self.assertEqual(
            {identity: path for identity, path in snapshot["identities"].items()},
            identities,
        )

    def test_admit_batch_binds_real_event_source_and_preserves_historic_object(self) -> None:
        target = self._source(
            "ToS/source-witnesses/works/retired.md",
            b"source to retire\n",
        )
        first, store, _ = self._admit(
            "event-base.json",
            self._batch([target]),
            {"fixture:target": target["path"]},
        )
        event = self._source(
            "ToS/source-witnesses/works/review-event.json",
            b"real source event fixture\n",
        )
        retirement = {
            "path": target["path"],
            "event_ref": event["path"],
            "event_sha256": event["sha256"],
        }
        receipt, retired_store, validator = self._admit(
            "event-retirement.json",
            self._batch(
                [event],
                base_revision=first["revision"],
                retirements=[retirement],
            ),
            {"fixture:event": event["path"]},
        )
        self.assertEqual(retired_store.current(), receipt["revision"])
        self.assertEqual(len(validator.calls), 1)
        self.assertEqual(
            retired_store.load(receipt["revision"], verify_objects=True)["retirements"],
            [{
                "path": target["path"],
                "sha256": target["sha256"],
                "event_ref": event["path"],
                "event_sha256": event["sha256"],
                "event_size_bytes": event["size_bytes"],
            }],
        )
        historic = self.root / "event-base-restore"
        store.restore(first["revision"], historic)
        self.assertEqual((historic / target["path"]).read_bytes(), b"source to retire\n")

    def test_validator_identity_mismatch_does_not_create_store(self) -> None:
        row = self._source("ToS/source-witnesses/works/fixture.md")
        store_root = self.root / "mismatch-store"
        validator = FixedValidator({"fixture": row["path"]}, sha256=self.VALIDATOR_SHA)
        batch_path = self._write_batch(
            "mismatch.json",
            self._batch([row], validator_sha="2" * 64),
        )
        with patch.object(admission, "SourceValidator", return_value=validator):
            with self.assertRaisesRegex(CorpusStoreError, "identity"):
                admission.admit_batch(store_root, batch_path, self.input_root, self.grammar_root)
        self.assertFalse(store_root.exists())
        self.assertEqual(validator.calls, [])

    def test_invalid_hash_batch_does_not_advance_current_after_valid_batch(self) -> None:
        first_row = self._source("ToS/source-witnesses/works/first.md", b"first\n")
        first_receipt, store, _ = self._admit(
            "first.json",
            self._batch([first_row]),
            {"fixture:first": first_row["path"]},
        )
        current_before = store.current()
        self.assertEqual(current_before, first_receipt["revision"])

        valid_second = self._source("ToS/source-witnesses/works/second.md", b"second\n")
        invalid_second = self._source("ToS/source-witnesses/works/third.md", b"third\n")
        invalid_second["sha256"] = "0" * 64
        second_batch = self._batch(
            [valid_second, invalid_second],
            base_revision=current_before,
        )
        validator = FixedValidator(
            {
                "fixture:first": first_row["path"],
                "fixture:second": valid_second["path"],
                "fixture:third": invalid_second["path"],
            },
            sha256=self.VALIDATOR_SHA,
        )
        with patch.object(admission, "SourceValidator", return_value=validator):
            with self.assertRaisesRegex(CorpusStoreError, "digest differs"):
                admission.admit_batch(
                    self.root / "store",
                    self._write_batch("invalid-second.json", second_batch),
                    self.input_root,
                    self.grammar_root,
                )
        self.assertEqual(store.current(), current_before)
        self.assertEqual(store.load(current_before, verify_objects=True), store.load(current_before))

    def test_noop_batch_preserves_current_revision_identity(self) -> None:
        row = self._source("ToS/source-witnesses/works/fixture.md")
        first_receipt, store, _ = self._admit(
            "initial.json",
            self._batch([row]),
            {"fixture": row["path"]},
        )
        pointer_before = (store.root / "current.json").read_bytes()
        noop = self._batch([], base_revision=first_receipt["revision"])
        noop_receipt, noop_store, validator = self._admit(
            "noop.json",
            noop,
            {"fixture": row["path"]},
        )
        self.assertEqual(noop_receipt["revision"], first_receipt["revision"])
        self.assertEqual(noop_store.current(), first_receipt["revision"])
        self.assertEqual((store.root / "current.json").read_bytes(), pointer_before)
        self.assertEqual(validator.calls, [])


if __name__ == "__main__":
    unittest.main()
