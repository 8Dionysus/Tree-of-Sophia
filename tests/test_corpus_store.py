from __future__ import annotations

from contextlib import redirect_stderr
import hashlib
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from typing import Any, Callable
from unittest.mock import patch


SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import corpus_store  # noqa: E402


Validator = Callable[
    [corpus_store.CorpusCandidate, dict[str, Any] | None, frozenset[str]],
    corpus_store.ValidationIndex,
]


class RecordValidator:
    """A fixed validator over a complete disposable copy of the view."""

    def __init__(self, store_root: Path) -> None:
        self.store_root = store_root.resolve()
        self.calls: list[dict[str, Any]] = []

    def __call__(
        self,
        candidate: corpus_store.CorpusCandidate,
        base: dict[str, Any] | None,
        affected: frozenset[str],
    ) -> corpus_store.ValidationIndex:
        del base
        view = candidate.materialize(candidate.paths)
        resolved_view = view.resolve()
        if resolved_view == self.store_root or not resolved_view.is_dir():
            raise corpus_store.CorpusStoreError("validator did not receive a disposable view")
        self.calls.append({"view": resolved_view, "affected": set(affected)})

        records_by_path: dict[str, list[dict[str, Any]]] = {}
        identities: dict[str, str] = {}
        paths = sorted(
            path
            for path in view.rglob("*")
            if path.is_file()
        )
        for path in paths:
            relative = path.relative_to(view).as_posix()
            try:
                payload = json.loads(path.read_bytes())
            except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
                raise corpus_store.CorpusStoreError("invalid JSON record") from exc
            records = payload if isinstance(payload, list) else [payload]
            if not records or any(not isinstance(record, dict) for record in records):
                raise corpus_store.CorpusStoreError("record file must contain JSON records")
            records_by_path[relative] = []
            for record in records:
                if set(record) != {"id", "links"}:
                    raise corpus_store.CorpusStoreError("record fields are invalid")
                identity = record["id"]
                links = record["links"]
                if (
                    not isinstance(identity, str)
                    or not identity
                    or not isinstance(links, list)
                    or any(not isinstance(target, str) or not target for target in links)
                ):
                    raise corpus_store.CorpusStoreError("record identity or links are invalid")
                if identity in identities:
                    raise corpus_store.CorpusStoreError("duplicate record identity")
                identities[identity] = relative
                records_by_path[relative].append(record)

        dependencies: dict[str, list[str]] = {path: [] for path in records_by_path}
        for source, records in records_by_path.items():
            targets: set[str] = set()
            for record in records:
                for target_identity in record["links"]:
                    if target_identity not in identities:
                        raise corpus_store.CorpusStoreError("record references an unknown target")
                    targets.add(identities[target_identity])
            dependencies[source] = sorted(targets)
        return corpus_store.ValidationIndex(identities, dependencies)


class CorpusStoreTests(unittest.TestCase):
    def test_streamed_canonical_bytes_match_legacy_encoding(self) -> None:
        values = [
            {
                "ascii": "plain",
                "escaped": "quote \" slash \\ newline\n unicode é",
                "float": -12.5,
                "integer": 9007199254740993,
                "nested": [True, None, {"z": 0, "a": 1}],
            },
            ["z", {"b": 2, "a": 1}, 0.0],
        ]
        for value in values:
            with self.subTest(value=value):
                streamed = b"".join(corpus_store._canonical_blocks(value, block_size=7))
                self.assertEqual(streamed, corpus_store.canonical(value))
                self.assertEqual(
                    corpus_store._canonical_digest(value),
                    hashlib.sha256(corpus_store.canonical(value)).hexdigest(),
                )

        with self.assertRaises(ValueError):
            list(corpus_store._canonical_blocks({"not_finite": float("nan")}))

    def test_stage_timing_is_opt_in_and_emits_bounded_stderr_events(self) -> None:
        captured = io.StringIO()
        with patch.dict(os.environ, {"TOS_CORPUS_TIMINGS": "1"}):
            with redirect_stderr(captured):
                with corpus_store.stage_timing("fixture", members=2):
                    pass
        events = [json.loads(line) for line in captured.getvalue().splitlines()]
        self.assertEqual([event["event"] for event in events], ["start", "end"])
        self.assertEqual([event["stage"] for event in events], ["fixture", "fixture"])
        self.assertEqual(events[1]["status"], "ok")
        self.assertEqual(events[1]["members"], 2)
        self.assertGreaterEqual(events[1]["finished_at"], events[0]["started_at"])

        captured = io.StringIO()
        with patch.dict(os.environ, {"TOS_CORPUS_TIMINGS": "0"}):
            with redirect_stderr(captured):
                with corpus_store.stage_timing("fixture"):
                    pass
        self.assertEqual(captured.getvalue(), "")

    def test_object_namespace_sync_failure_cannot_publish_and_retry_reuses_objects(self):
        baseline = self._admit(base_revision=None, updates={
            'base.json': self._update('base.json', {'id': 'base', 'links': []})})
        updates = {name + '.json': self._update(name + '.json', {'id': name, 'links': ['base']})
                   for name in ('new-a', 'new-b')}
        before = (self.store.root / 'current.json').read_bytes()
        calls_before = len(self.validator.calls)
        real_sync = corpus_store._sync_dir

        def fail_object_namespace(path):
            if path == self.store.root / 'objects':
                self.assertTrue(all(self.store._object(row['sha256']).is_file()
                                    for row in updates.values()))
                raise OSError('object namespace durability failure')
            real_sync(path)

        with patch.object(corpus_store, '_sync_dir', side_effect=fail_object_namespace):
            with self.assertRaisesRegex(OSError, 'namespace durability'):
                self._admit(base_revision=baseline['revision'], updates=updates)
        self.assertEqual((self.store.root / 'current.json').read_bytes(), before)
        self.assertEqual(len(self.validator.calls), calls_before)
        accepted = self._admit(base_revision=baseline['revision'], updates=updates)
        self.assertEqual(self.store.load(accepted['revision'], verify_objects=True), accepted)
        self.assertEqual(set(accepted['identities']), {'base', 'new-a', 'new-b'})

    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory(prefix="corpus-store-tests-")
        self.root = Path(self._temporary.name)
        self.source_root = self.root / "sources"
        self.source_root.mkdir()
        self.store = corpus_store.CorpusStore(self.root / "store")
        self.validator = RecordValidator(self.store.root)
        self.validator_sha256 = "a" * 64

    def tearDown(self) -> None:
        self._temporary.cleanup()

    def _write_payload(
        self,
        relative: str,
        payload: object,
        *,
        mode: int = 0o644,
        raw: bytes | None = None,
    ) -> Path:
        path = self.source_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(corpus_store.canonical(payload) if raw is None else raw)
        os.chmod(path, mode)
        return path

    def _update(
        self,
        relative: str,
        payload: object,
        *,
        mode: int = 0o644,
        raw: bytes | None = None,
        source_relative: str | None = None,
    ) -> dict[str, object]:
        source = self._write_payload(
            relative if source_relative is None else source_relative,
            payload,
            mode=mode,
            raw=raw,
        )
        content = source.read_bytes()
        return {
            "source": source,
            "sha256": hashlib.sha256(content).hexdigest(),
            "size_bytes": len(content),
            "mode": mode,
        }

    def _admit(
        self,
        *,
        base_revision: str | None,
        updates: dict[str, dict[str, object]],
        retirements: dict[str, dict[str, str]] | None = None,
        validator_sha256: str | None = None,
        validate: Validator | None = None,
    ) -> dict[str, Any]:
        return self.store.admit(
            base_revision=base_revision,
            updates=updates,
            retirements={} if retirements is None else retirements,
            validator_sha256=self.validator_sha256 if validator_sha256 is None else validator_sha256,
            validate=self.validator if validate is None else validate,
        )

    def _base_records(self, *, include_extra: bool = False) -> dict[str, dict[str, object]]:
        updates = {
            "records/a.json": self._update("records/a.json", {"id": "a", "links": ["b"]}),
            "records/b.json": self._update("records/b.json", {"id": "b", "links": []}),
        }
        if include_extra:
            updates["records/c.json"] = self._update(
                "records/c.json",
                {"id": "c", "links": ["a"]},
            )
            updates["records/d.json"] = self._update(
                "records/d.json",
                {"id": "d", "links": []},
            )
        return updates

    def test_initial_admission_and_exact_restore(self) -> None:
        updates = self._base_records()
        updates["records/b.json"] = self._update(
            "records/b.json",
            {"id": "b", "links": []},
            mode=0o755,
        )
        expected = {path: dict(update) for path, update in updates.items()}
        manifest = self._admit(base_revision=None, updates=updates)
        revision = manifest["revision"]
        self.assertEqual(self.store.current(), revision)
        self.assertEqual(manifest["validator_sha256"], self.validator_sha256)
        self.assertEqual(
            {entry["path"] for entry in manifest["files"]},
            {"records/a.json", "records/b.json"},
        )
        self.assertTrue(self.validator.calls)
        self.assertNotEqual(self.validator.calls[0]["view"], self.store.root.resolve())

        output = self.root / "restored"
        restored_manifest = self.store.restore(revision, output)
        self.assertEqual(restored_manifest, manifest)
        for relative, update in expected.items():
            restored = output / relative
            self.assertEqual(restored.read_bytes(), Path(update["source"]).read_bytes())
            self.assertEqual(restored.stat().st_mode & 0o777, update["mode"])
        self.assertEqual(self.store.load(revision, verify_objects=True), manifest)

    def test_bad_sha_is_rejected_and_no_current_pointer_is_created(self) -> None:
        update = self._update("records/a.json", {"id": "a", "links": []})
        update["sha256"] = "0" * 64
        with self.assertRaises(corpus_store.CorpusStoreError):
            self._admit(base_revision=None, updates={"records/a.json": update})
        self.assertIsNone(self.store.current())
        self.assertFalse((self.store.root / "current.json").exists())

    def test_invalid_json_bad_reference_and_duplicate_id_preserve_current_and_base_bytes(self) -> None:
        base_updates = self._base_records()
        base = self._admit(base_revision=None, updates=base_updates)
        base_revision = base["revision"]
        base_output = self.root / "base-before"
        self.store.restore(base_revision, base_output)
        base_bytes = {
            relative: (base_output / relative).read_bytes()
            for relative in ("records/a.json", "records/b.json")
        }

        invalid_json = self._update(
            "records/a.json",
            {"id": "a", "links": []},
            raw=b"{ this is not JSON\n",
            source_relative="inputs/invalid.json",
        )
        bad_reference = self._update(
            "records/a.json",
            {"id": "a", "links": ["missing"]},
            source_relative="inputs/bad-reference.json",
        )
        duplicate_updates = {
            "records/a.json": self._update(
                "records/a.json",
                {"id": "a", "links": []},
                source_relative="inputs/duplicate-a.json",
            ),
            "records/c.json": self._update(
                "records/c.json",
                {"id": "a", "links": []},
                source_relative="inputs/duplicate-c.json",
            ),
        }
        batches = [
            {"records/a.json": invalid_json},
            {"records/a.json": bad_reference},
            duplicate_updates,
        ]
        for batch in batches:
            with self.subTest(batch=batch):
                with self.assertRaises(corpus_store.CorpusStoreError):
                    self._admit(base_revision=base_revision, updates=batch)
                self.assertEqual(self.store.current(), base_revision)
                output = self.root / ("base-after-" + str(len(list(self.root.glob("base-after-*")))))
                self.store.restore(base_revision, output)
                for relative, content in base_bytes.items():
                    self.assertEqual((output / relative).read_bytes(), content)

    def test_no_op_revision_is_stable(self) -> None:
        updates = self._base_records()
        first = self._admit(base_revision=None, updates=updates)
        calls_before = len(self.validator.calls)
        repeated = {
            path: self._update(path, json.loads(Path(update["source"]).read_bytes()))
            for path, update in updates.items()
        }
        second = self._admit(base_revision=first["revision"], updates=repeated)
        self.assertEqual(second, first)
        self.assertEqual(self.store.current(), first["revision"])
        self.assertEqual(len(self.validator.calls), calls_before)
        self.assertEqual(len(list((self.store.root / "revisions").iterdir())), 1)

    def test_source_change_and_validator_write_are_rejected(self) -> None:
        updates = self._base_records()
        base = self._admit(base_revision=None, updates=updates)
        base_revision = base["revision"]

        source_change = self._update("records/c.json", {"id": "c", "links": []})
        original_copyfileobj = corpus_store.shutil.copyfileobj

        def mutate_after_copy(source_stream: Any, destination_stream: Any, length: int) -> None:
            original_copyfileobj(source_stream, destination_stream, length)
            Path(source_change["source"]).write_bytes(
                corpus_store.canonical({"id": "c", "links": ["a", "b", "changed"]})
            )

        with self.assertRaises(corpus_store.CorpusStoreError):
            with patch.object(corpus_store.shutil, "copyfileobj", side_effect=mutate_after_copy):
                self._admit(base_revision=base_revision, updates={"records/c.json": source_change})
        self.assertEqual(self.store.current(), base_revision)

        destination_corruption = self._update("records/c.json", {"id": "c", "links": ["b"]})

        def corrupt_destination(source_stream: Any, destination_stream: Any, length: int) -> None:
            original_copyfileobj(source_stream, destination_stream, length)
            destination_stream.seek(0)
            destination_stream.write(b"!")
            destination_stream.seek(0, 2)

        # Hashing the source while streaming is not enough: a write fault in
        # the staged destination must still be caught before CAS publication.
        with self.assertRaisesRegex(corpus_store.CorpusStoreError, "digest differs"):
            with patch.object(corpus_store.shutil, "copyfileobj", side_effect=corrupt_destination):
                self._admit(
                    base_revision=base_revision,
                    updates={"records/c.json": destination_corruption},
                )
        self.assertEqual(self.store.current(), base_revision)

        new_update = self._update("records/c.json", {"id": "c", "links": ["b"]})

        def writing_validator(
            candidate: corpus_store.CorpusCandidate,
            validator_base: dict[str, Any] | None,
            affected: frozenset[str],
        ) -> corpus_store.ValidationIndex:
            view = candidate.materialize(candidate.paths)
            index = self.validator(candidate, validator_base, affected)
            (view / "records/a.json").write_bytes(b"validator changed the disposable view\n")
            return index

        with self.assertRaises(corpus_store.CorpusStoreError):
            self._admit(
                base_revision=base_revision,
                updates={"records/c.json": new_update},
                validate=writing_validator,
            )
        self.assertEqual(self.store.current(), base_revision)
        output = self.root / "after-source-and-validator-failures"
        self.store.restore(base_revision, output)
        self.assertEqual(
            (output / "records/a.json").read_bytes(),
            corpus_store.canonical({"id": "a", "links": ["b"]}),
        )

    def test_two_batches_using_the_same_base_second_is_rejected(self) -> None:
        base = self._admit(base_revision=None, updates=self._base_records())
        base_revision = base["revision"]
        first_update = self._update("records/a.json", {"id": "a", "links": []})
        first = self._admit(
            base_revision=base_revision,
            updates={"records/a.json": first_update},
        )
        self.assertNotEqual(first["revision"], base_revision)

        second_update = self._update("records/b.json", {"id": "b", "links": ["a"]})
        with self.assertRaises(corpus_store.CorpusStoreError):
            self._admit(
                base_revision=base_revision,
                updates={"records/b.json": second_update},
            )
        self.assertEqual(self.store.current(), first["revision"])

    def test_valid_update_reports_transitive_incoming_affected_paths(self) -> None:
        base = self._admit(base_revision=None, updates=self._base_records(include_extra=True))
        base_revision = base["revision"]
        changed = self._update("records/b.json", {"id": "b", "links": ["d"]})
        updated = self._admit(
            base_revision=base_revision,
            updates={"records/b.json": changed},
        )
        self.assertNotEqual(updated["revision"], base_revision)
        affected = self.validator.calls[-1]["affected"]
        self.assertEqual(
            affected,
            {"records/a.json", "records/b.json", "records/c.json"},
        )
        self.assertNotIn("records/d.json", affected)

    def test_retirement_keeps_explicit_event_and_historic_restore(self) -> None:
        event = self._update(
            "records/retirement-event.json",
            {"id": "retirement-event", "links": []},
        )
        base_updates = self._base_records()
        base_updates["records/retirement-event.json"] = event
        base = self._admit(base_revision=None, updates=base_updates)
        base_revision = base["revision"]
        updated_a = self._update("records/a.json", {"id": "a", "links": []})
        retired = self._admit(
            base_revision=base_revision,
            updates={"records/a.json": updated_a},
            retirements={
                "records/b.json": {
                    "event_ref": "records/retirement-event.json",
                    "event_sha256": event["sha256"],
                },
            },
        )
        self.assertEqual(self.store.current(), retired["revision"])
        self.assertEqual(
            [entry["path"] for entry in retired["files"]],
            ["records/a.json", "records/retirement-event.json"],
        )
        self.assertEqual(
            retired["retirements"],
            [{
                "path": "records/b.json",
                "sha256": next(
                    entry["sha256"]
                    for entry in base["files"]
                    if entry["path"] == "records/b.json"
                ),
                "event_ref": "records/retirement-event.json",
                "event_sha256": event["sha256"],
                "event_size_bytes": event["size_bytes"],
            }],
        )

        historic = self.root / "historic-restore"
        self.store.restore(base_revision, historic)
        self.assertEqual(
            (historic / "records/b.json").read_bytes(),
            corpus_store.canonical({"id": "b", "links": []}),
        )
        current = self.root / "current-restore"
        self.store.restore(retired["revision"], current)
        self.assertFalse((current / "records/b.json").exists())
        self.assertTrue((current / "records/retirement-event.json").exists())

    def test_retirement_event_can_be_updated_in_the_same_candidate(self) -> None:
        base = self._admit(base_revision=None, updates=self._base_records())
        event = self._update(
            "records/new-retirement-event.json",
            {"id": "new-retirement-event", "links": []},
        )
        retired = self._admit(
            base_revision=base["revision"],
            updates={
                "records/a.json": self._update("records/a.json", {"id": "a", "links": []}),
                "records/new-retirement-event.json": event,
            },
            retirements={
                "records/b.json": {
                    "event_ref": "records/new-retirement-event.json",
                    "event_sha256": event["sha256"],
                },
            },
        )
        self.assertEqual(self.store.current(), retired["revision"])
        self.assertEqual(
            retired["retirements"][0]["event_ref"],
            "records/new-retirement-event.json",
        )
        self.assertEqual(retired["retirements"][0]["event_size_bytes"], event["size_bytes"])
        self.assertEqual(self.store.load(retired["revision"], verify_objects=True), retired)

    def test_retirement_requires_present_matching_and_nonretired_event(self) -> None:
        event = self._update(
            "records/retirement-event.json",
            {"id": "retirement-event", "links": []},
        )
        base_updates = self._base_records()
        base_updates["records/retirement-event.json"] = event
        base = self._admit(base_revision=None, updates=base_updates)
        base_revision = base["revision"]
        cases = [
            {
                "records/b.json": {
                    "event_ref": "records/missing-event.json",
                    "event_sha256": "0" * 64,
                },
            },
            {
                "records/b.json": {
                    "event_ref": "records/retirement-event.json",
                    "event_sha256": "0" * 64,
                },
            },
            {
                "records/b.json": {
                    "event_ref": "records/b.json",
                    "event_sha256": next(
                        entry["sha256"]
                        for entry in base["files"]
                        if entry["path"] == "records/b.json"
                    ),
                },
            },
            {
                "records/b.json": {
                    "event_ref": "records/retirement-event.json",
                    "event_sha256": event["sha256"],
                },
                "records/retirement-event.json": {
                    "event_ref": "records/a.json",
                    "event_sha256": next(
                        entry["sha256"]
                        for entry in base["files"]
                        if entry["path"] == "records/a.json"
                    ),
                },
            },
        ]
        for index, retirement in enumerate(cases):
            with self.subTest(index=index):
                with self.assertRaises(corpus_store.CorpusStoreError):
                    self._admit(base_revision=base_revision, updates={}, retirements=retirement)
                self.assertEqual(self.store.current(), base_revision)

    def test_historical_retirement_event_object_is_verified_after_event_path_retired(self) -> None:
        event = self._update(
            "records/retirement-event.json",
            {"id": "retirement-event", "links": []},
        )
        base_updates = self._base_records()
        base_updates["records/retirement-event.json"] = event
        base = self._admit(base_revision=None, updates=base_updates)
        retired_target = self._admit(
            base_revision=base["revision"],
            updates={"records/a.json": self._update("records/a.json", {"id": "a", "links": []})},
            retirements={
                "records/b.json": {
                    "event_ref": "records/retirement-event.json",
                    "event_sha256": event["sha256"],
                },
            },
        )
        retired_event = self._admit(
            base_revision=retired_target["revision"],
            updates={},
            retirements={
                "records/retirement-event.json": {
                    "event_ref": "records/a.json",
                    "event_sha256": next(
                        entry["sha256"]
                        for entry in retired_target["files"]
                        if entry["path"] == "records/a.json"
                    ),
                },
            },
        )
        self.assertEqual(self.store.load(retired_event["revision"]), retired_event)
        event_object = self.store.root / "objects" / event["sha256"]
        os.chmod(event_object, 0o644)
        event_object.write_bytes(b"tampered historical event\n")
        with self.assertRaises(corpus_store.CorpusStoreError):
            self.store.load(retired_event["revision"], verify_objects=True)
        self.assertEqual(self.store.current(), retired_event["revision"])

    def test_retirement_history_allows_same_path_again_with_a_new_event(self) -> None:
        event_one = self._update(
            "records/retirement-event-one.json",
            {"id": "retirement-event-one", "links": []},
        )
        base_updates = self._base_records()
        base_updates[event_one["source"].relative_to(self.source_root).as_posix()] = event_one
        base = self._admit(base_revision=None, updates=base_updates)
        retired_once = self._admit(
            base_revision=base["revision"],
            updates={"records/a.json": self._update("records/a.json", {"id": "a", "links": []})},
            retirements={
                "records/b.json": {
                    "event_ref": "records/retirement-event-one.json",
                    "event_sha256": event_one["sha256"],
                },
            },
        )

        event_two = self._update(
            "records/retirement-event-two.json",
            {"id": "retirement-event-two", "links": []},
        )
        reintroduced = self._admit(
            base_revision=retired_once["revision"],
            updates={
                "records/b.json": self._update("records/b.json", {"id": "b", "links": []}),
                "records/retirement-event-two.json": event_two,
            },
        )
        retired_twice = self._admit(
            base_revision=reintroduced["revision"],
            updates={},
            retirements={
                "records/b.json": {
                    "event_ref": "records/retirement-event-two.json",
                    "event_sha256": event_two["sha256"],
                },
            },
        )
        self.assertEqual(self.store.current(), retired_twice["revision"])
        self.assertEqual(
            [event["path"] for event in retired_twice["retirements"]],
            ["records/b.json", "records/b.json"],
        )
        self.assertNotEqual(
            retired_twice["retirements"][0]["event_sha256"],
            retired_twice["retirements"][1]["event_sha256"],
        )
        self.assertEqual(self.store.load(retired_twice["revision"], verify_objects=True), retired_twice)

        historic = self.root / "reintroduced-history"
        self.store.restore(retired_twice["revision"], historic)
        self.assertFalse((historic / "records/b.json").exists())
        self.assertTrue((historic / "records/retirement-event-one.json").exists())
        self.assertTrue((historic / "records/retirement-event-two.json").exists())
        original = self.root / "original-history"
        self.store.restore(base["revision"], original)
        self.assertTrue((original / "records/b.json").exists())

    def test_exact_historical_retirement_event_reuse_leaves_pointer_unchanged(self) -> None:
        event = self._update(
            "records/retirement-event.json",
            {"id": "retirement-event", "links": []},
        )
        base_updates = self._base_records()
        base_updates["records/retirement-event.json"] = event
        base = self._admit(base_revision=None, updates=base_updates)
        retired = self._admit(
            base_revision=base["revision"],
            updates={"records/a.json": self._update("records/a.json", {"id": "a", "links": []})},
            retirements={
                "records/b.json": {
                    "event_ref": "records/retirement-event.json",
                    "event_sha256": event["sha256"],
                },
            },
        )
        reintroduced = self._admit(
            base_revision=retired["revision"],
            updates={"records/b.json": self._update("records/b.json", {"id": "b", "links": []})},
        )
        with self.assertRaisesRegex(corpus_store.CorpusStoreError, "already exists"):
            self._admit(
                base_revision=reintroduced["revision"],
                updates={},
                retirements={
                    "records/b.json": {
                        "event_ref": "records/retirement-event.json",
                        "event_sha256": event["sha256"],
                    },
                },
            )
        self.assertEqual(self.store.current(), reintroduced["revision"])
        self.assertEqual(self.store.load(reintroduced["revision"], verify_objects=True), reintroduced)

    def test_retired_identity_cannot_be_reused_at_another_path(self) -> None:
        event = self._update(
            "records/retirement-event.json",
            {"id": "retirement-event", "links": []},
        )
        base_updates = self._base_records()
        base_updates["records/retirement-event.json"] = event
        base = self._admit(base_revision=None, updates=base_updates)
        retired = self._admit(
            base_revision=base["revision"],
            updates={"records/a.json": self._update("records/a.json", {"id": "a", "links": []})},
            retirements={
                "records/b.json": {
                    "event_ref": "records/retirement-event.json",
                    "event_sha256": event["sha256"],
                },
            },
        )
        reused = self._update("records/c.json", {"id": "b", "links": []})
        with self.assertRaises(corpus_store.CorpusStoreError):
            self._admit(
                base_revision=retired["revision"],
                updates={"records/c.json": reused},
            )
        self.assertEqual(self.store.current(), retired["revision"])

    def test_validator_identity_change_forces_all_paths_into_affected_set(self) -> None:
        base = self._admit(base_revision=None, updates=self._base_records(include_extra=True))
        changed_validator = "b" * 64
        updated = self._admit(
            base_revision=base["revision"],
            updates={},
            validator_sha256=changed_validator,
        )
        self.assertNotEqual(updated["revision"], base["revision"])
        self.assertEqual(updated["validator_sha256"], changed_validator)
        self.assertEqual(
            self.validator.calls[-1]["affected"],
            {"records/a.json", "records/b.json", "records/c.json", "records/d.json"},
        )

    def test_corrupt_object_and_malformed_manifest_fail_closed(self) -> None:
        base = self._admit(base_revision=None, updates=self._base_records())
        revision = base["revision"]
        entry = next(item for item in base["files"] if item["path"] == "records/a.json")
        object_path = self.store.root / "objects" / entry["sha256"]
        os.chmod(object_path, 0o644)
        object_path.write_bytes(b"corrupt object bytes\n")
        with self.assertRaises(corpus_store.CorpusStoreError):
            self.store.load(revision, verify_objects=True)
        # A no-op returns the prior verified manifest; it does not re-attest object health.
        self.assertEqual(self._admit(base_revision=revision, updates={}), base)
        self.assertEqual(self.store.current(), revision)
        changed_b = self._update("records/b.json", {"id": "b", "links": ["a"]})
        with self.assertRaises(corpus_store.CorpusStoreError):
            self._admit(base_revision=revision, updates={"records/b.json": changed_b})
        self.assertEqual(self.store.current(), revision)

        second_store = corpus_store.CorpusStore(self.root / "second-store")
        second_validator = RecordValidator(second_store.root)
        second_updates = {
            "records/a.json": self._update(
                "records/a.json",
                {"id": "a", "links": []},
                source_relative="inputs/second-a.json",
            ),
            "records/b.json": self._update(
                "records/b.json",
                {"id": "b", "links": []},
                source_relative="inputs/second-b.json",
            ),
        }
        second_manifest = second_store.admit(
            base_revision=None,
            updates=second_updates,
            retirements={},
            validator_sha256=self.validator_sha256,
            validate=second_validator,
        )
        second_revision = second_manifest["revision"]
        snapshot = second_store.root / "revisions" / second_revision / "snapshot.json"
        canonical_snapshot = snapshot.read_bytes()
        snapshot.write_bytes(canonical_snapshot[:1] + b" " + canonical_snapshot[1:])
        with self.assertRaises(corpus_store.CorpusStoreError):
            second_store.load(second_revision)
        snapshot.write_bytes(canonical_snapshot)
        snapshot.write_bytes(b"{malformed manifest\n")
        with self.assertRaises((corpus_store.CorpusStoreError, json.JSONDecodeError)):
            second_store.load(second_revision)
        with self.assertRaises((corpus_store.CorpusStoreError, json.JSONDecodeError)):
            second_store.admit(
                base_revision=second_revision,
                updates={},
                retirements={},
                validator_sha256=self.validator_sha256,
                validate=second_validator,
            )
        self.assertEqual(second_store.current(), second_revision)

    def test_selective_candidate_reads_related_objects_only_and_closes_after_admission(self) -> None:
        paths = {
            "records/a.json",
            "records/b.json",
            "records/c.json",
        }
        base_updates = {
            "records/a.json": self._update(
                "records/a.json", {"id": "a", "links": ["b"]}
            ),
            "records/b.json": self._update(
                "records/b.json", {"id": "b", "links": []}
            ),
            "records/c.json": self._update(
                "records/c.json", {"id": "c", "links": []}
            ),
        }
        base = self._admit(base_revision=None, updates=base_updates)
        base_revision = base["revision"]
        seen_candidates: list[corpus_store.CorpusCandidate] = []

        def selective_validator(
            candidate: corpus_store.CorpusCandidate,
            validator_base: dict[str, Any] | None,
            affected: frozenset[str],
        ) -> corpus_store.ValidationIndex:
            self.assertIsNotNone(validator_base)
            assert validator_base is not None
            seen_candidates.append(candidate)
            self.assertEqual(set(candidate.paths), paths)
            self.assertEqual(affected, {"records/a.json"})
            record_a = json.loads(candidate.read_bytes("records/a.json"))
            record_b = json.loads(candidate.read_bytes("records/b.json"))
            self.assertEqual(record_a, {"id": "a", "links": ["b"]})
            self.assertEqual(record_b, {"id": "b", "links": []})
            self.assertEqual(
                validator_base["identities"],
                {"a": "records/a.json", "b": "records/b.json", "c": "records/c.json"},
            )
            self.assertEqual(
                validator_base["dependencies"],
                {
                    "records/a.json": ["records/b.json"],
                    "records/b.json": [],
                    "records/c.json": [],
                },
            )
            return corpus_store.ValidationIndex(
                dict(validator_base["identities"]),
                {
                    path: list(targets)
                    for path, targets in validator_base["dependencies"].items()
                },
            )

        c_entry = next(entry for entry in base["files"] if entry["path"] == "records/c.json")
        c_object = self.store.root / "objects" / c_entry["sha256"]
        os.chmod(c_object, 0o644)
        c_object.write_bytes(b"corrupt unrelated c\n")
        update_one = self._update(
            "records/a.json",
            {"id": "a", "links": ["b"]},
            raw=b'{ "id": "a", "links": ["b"] }\n',
        )
        with patch.object(self.store, "_verify_object", wraps=self.store._verify_object) as verify:
            updated = self._admit(
                base_revision=base_revision,
                updates={"records/a.json": update_one},
                validate=selective_validator,
            )
        self.assertNotEqual(updated["revision"], base_revision)
        self.assertEqual(self.store.current(), updated["revision"])
        touched = [call.args[0]["path"] for call in verify.call_args_list]
        self.assertEqual(set(touched), {"records/a.json", "records/b.json"})
        self.assertNotIn("records/c.json", touched)

        with self.assertRaises(corpus_store.CorpusStoreError):
            self.store.load(updated["revision"], verify_objects=True)
        with self.assertRaises(corpus_store.CorpusStoreError):
            self.store.restore(updated["revision"], self.root / "corrupt-restore")
        self.assertFalse((self.root / "corrupt-restore").exists())

        b_entry = next(entry for entry in updated["files"] if entry["path"] == "records/b.json")
        b_object = self.store.root / "objects" / b_entry["sha256"]
        os.chmod(b_object, 0o644)
        b_object.write_bytes(b"corrupt related b\n")
        update_two = self._update(
            "records/a.json",
            {"id": "a", "links": ["b"]},
            raw=b'{"id":"a","links":["b"]}\n',
        )
        current_before = self.store.current()
        with self.assertRaises(corpus_store.CorpusStoreError):
            self._admit(
                base_revision=updated["revision"],
                updates={"records/a.json": update_two},
                validate=selective_validator,
            )
        self.assertEqual(self.store.current(), current_before)

        retained = seen_candidates[0]
        with self.assertRaisesRegex(corpus_store.CorpusStoreError, "closed"):
            retained.entry("records/a.json")
        with self.assertRaisesRegex(corpus_store.CorpusStoreError, "closed"):
            retained.read_bytes("records/a.json")

    def test_validator_base_is_mutable_but_isolated_from_accepted_manifest(self) -> None:
        base = self._admit(base_revision=None, updates=self._base_records())
        base_revision = base["revision"]

        def mutating_validator(
            candidate: corpus_store.CorpusCandidate,
            validator_base: dict[str, Any] | None,
            affected: frozenset[str],
        ) -> corpus_store.ValidationIndex:
            del candidate, affected
            self.assertIsNotNone(validator_base)
            assert validator_base is not None
            identities = dict(validator_base["identities"])
            dependencies = {
                path: list(targets)
                for path, targets in validator_base["dependencies"].items()
            }
            # The validator API intentionally supplies ordinary mutable JSON.
            # Admission must still protect its accepted base from such writes.
            validator_base["files"][0]["path"] = "validator-mutated.json"
            validator_base["identities"].clear()
            validator_base["dependencies"].clear()
            return corpus_store.ValidationIndex(identities, dependencies)

        updated = self._admit(
            base_revision=base_revision,
            updates={
                "records/a.json": self._update(
                    "records/a.json", {"id": "a", "links": []}
                )
            },
            validate=mutating_validator,
        )
        self.assertNotEqual(updated["revision"], base_revision)
        self.assertEqual(self.store.load(base_revision), base)
        self.assertEqual(
            [entry["path"] for entry in updated["files"]],
            ["records/a.json", "records/b.json"],
        )

    def test_restore_refuses_existing_regular_file_and_broken_symlink(self) -> None:
        base = self._admit(
            base_revision=None,
            updates={"records/a.json": self._update("records/a.json", {"id": "a", "links": []})},
        )
        existing = self.root / "existing-output"
        existing.write_bytes(b"keep this output\n")
        with self.assertRaises(corpus_store.CorpusStoreError):
            self.store.restore(base["revision"], existing)
        self.assertEqual(existing.read_bytes(), b"keep this output\n")

        broken = self.root / "broken-output"
        broken.symlink_to(self.root / "missing-target")
        with self.assertRaises(corpus_store.CorpusStoreError):
            self.store.restore(base["revision"], broken)
        self.assertTrue(broken.is_symlink())

    def test_20000_small_synthetic_records_single_scale_scenario(self) -> None:
        records = [
            {"id": f"synthetic-{index:05d}", "links": []}
            for index in range(20_000)
        ]
        manifest = self._admit(
            base_revision=None,
            updates={"records/synthetic.json": self._update("records/synthetic.json", records)},
        )
        self.assertEqual(len(manifest["identities"]), 20_000)
        self.assertEqual(manifest["files"][0]["path"], "records/synthetic.json")
        self.assertEqual(self.store.current(), manifest["revision"])
        self.assertEqual(self.store.load(manifest["revision"], verify_objects=True), manifest)
        total_bytes = sum(
            path.stat().st_size
            for path in self.store.root.rglob("*")
            if path.is_file()
        )
        self.assertLess(total_bytes, 30 * 1024 * 1024)


if __name__ == "__main__":
    unittest.main()
