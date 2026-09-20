from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest


SRC = Path(__file__).resolve().parents[1] / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

from tos_access.release_state import (  # noqa: E402
    BINDING_KEYS,
    PAIR_SCHEMA,
    POINTER_SCHEMA,
    REVOCATION_SCHEMA,
    ReleaseStateError,
    ReleaseStore,
    canonical,
    pair_id_for,
)


class ReleaseStateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="tos-release-state-")
        self.root = Path(self.temporary.name)
        self.store = ReleaseStore(self.root / "state")
        self.verifier_calls: list[tuple[dict, dict]] = []

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def _pair(seed: str) -> dict[str, str]:
        digest = (seed * 64)[:64]
        return {
            "schema_version": PAIR_SCHEMA,
            "software_sha256": digest,
            "data_revision": (chr(ord(seed) + 1) * 64)[:64],
            "data_manifest_sha256": (chr(ord(seed) + 2) * 64)[:64],
            "corpus_revision": (chr(ord(seed) + 3) * 64)[:64],
            "query_schema": "tos.query.v1",
            "compiler_version": "compiler-test-v1",
        }

    def _bindings(self, name: str = "one") -> dict[str, str]:
        return {
            "data_root": str((self.root / f"data-{name}").resolve()),
            "software_archive": str((self.root / f"software-{name}.zip").resolve()),
        }

    def _verify(self, pair: dict, bindings: dict) -> None:
        self.verifier_calls.append((pair, bindings))

    def _promote(self, pair: dict, bindings: dict, expected: str | None) -> str:
        return self.store.promote(
            pair,
            bindings,
            expected_current=expected,
            verify_pair=self._verify,
        )

    def test_initial_promotion_writes_exact_immutable_records_and_selection(self) -> None:
        pair = self._pair("a")
        bindings = self._bindings()
        pair_id = self._promote(pair, bindings, None)
        self.assertEqual(pair_id, pair_id_for(pair))
        self.assertEqual(
            set(json.loads((self.store.pairs / f"{pair_id}.json").read_text())),
            {
                "schema_version",
                "software_sha256",
                "data_revision",
                "data_manifest_sha256",
                "corpus_revision",
                "query_schema",
                "compiler_version",
            },
        )
        self.assertEqual(json.loads((self.store.bindings / f"{pair_id}.json").read_text()), bindings)
        self.assertEqual(
            json.loads(self.store.current_path.read_text()),
            {"schema_version": POINTER_SCHEMA, "current": pair_id, "previous": None},
        )
        self.assertEqual(
            self.store.read_selection(),
            {"pair_id": pair_id, "pair": pair, "bindings": bindings, "previous": None},
        )
        self.assertEqual(self.verifier_calls, [(pair, bindings)])

    def test_verify_failure_and_non_none_result_leave_pointer_unchanged(self) -> None:
        first = self._pair("a")
        first_id = self._promote(first, self._bindings(), None)
        second = self._pair("b")
        second_bindings = self._bindings("two")

        def rejected(pair: dict, bindings: dict) -> None:
            del pair, bindings
            raise RuntimeError("incompatible pair")

        with self.assertRaisesRegex(ReleaseStateError, "incompatible pair"):
            self.store.promote(
                second,
                second_bindings,
                expected_current=first_id,
                verify_pair=rejected,
            )
        self.assertEqual(self.store.read_selection()["pair_id"], first_id)
        self.assertFalse((self.store.pairs / f"{pair_id_for(second)}.json").exists())

        def returns_value(pair: dict, bindings: dict) -> str:
            del pair, bindings
            return "accepted"

        with self.assertRaisesRegex(ReleaseStateError, "must return None"):
            self.store.promote(
                second,
                second_bindings,
                expected_current=first_id,
                verify_pair=returns_value,
            )
        self.assertEqual(self.store.read_selection()["pair_id"], first_id)

    def test_compare_and_swap_rejects_conflicting_base(self) -> None:
        first_id = self._promote(self._pair("a"), self._bindings(), None)
        with self.assertRaisesRegex(ReleaseStateError, "expected_current"):
            self._promote(self._pair("b"), self._bindings("two"), None)
        self.assertEqual(self.store.read_selection()["pair_id"], first_id)

    def test_same_current_pair_is_idempotent_only_for_exact_bindings(self) -> None:
        pair = self._pair("a")
        bindings = self._bindings()
        pair_id = self._promote(pair, bindings, None)
        pointer_before = self.store.current_path.read_bytes()
        self.assertEqual(self._promote(dict(pair), dict(bindings), pair_id), pair_id)
        self.assertEqual(self.store.current_path.read_bytes(), pointer_before)
        with self.assertRaisesRegex(ReleaseStateError, "bindings"):
            self._promote(pair, self._bindings("different"), pair_id)
        self.assertEqual(self.store.current_path.read_bytes(), pointer_before)

    def test_corrupt_pair_or_binding_is_fail_closed(self) -> None:
        pair = self._pair("a")
        bindings = self._bindings()
        pair_id = self._promote(pair, bindings, None)
        pair_path = self.store.pairs / f"{pair_id}.json"
        original_pair = pair_path.read_bytes()
        pair_path.write_bytes(b'{"not":"the pair"}\n')
        with self.assertRaises(ReleaseStateError):
            self.store.read_selection()
        pair_path.write_bytes(original_pair)

        binding_path = self.store.bindings / f"{pair_id}.json"
        original_binding = binding_path.read_bytes()
        binding_path.write_bytes(b'{"data_root":"relative","software_archive":"relative"}\n')
        with self.assertRaises(ReleaseStateError):
            self.store.read_selection()
        binding_path.write_bytes(original_binding)
        self.assertEqual(self.store.read_selection()["pair_id"], pair_id)

    def test_rollback_rechecks_previous_with_callback_and_swaps_previous(self) -> None:
        first = self._pair("a")
        first_bindings = self._bindings()
        first_id = self._promote(first, first_bindings, None)
        second = self._pair("b")
        second_bindings = self._bindings("two")
        second_id = self._promote(second, second_bindings, first_id)
        self.verifier_calls.clear()

        rolled_back = self.store.rollback(expected_current=second_id, verify_pair=self._verify)
        self.assertEqual(rolled_back, first_id)
        self.assertEqual(self.store.read_selection()["pair_id"], first_id)
        self.assertEqual(self.store.read_selection()["previous"], second_id)
        self.assertEqual(self.verifier_calls, [(first, first_bindings)])

    def test_incompatible_rollback_callback_preserves_pointer(self) -> None:
        first_id = self._promote(self._pair("a"), self._bindings(), None)
        second_id = self._promote(self._pair("b"), self._bindings("two"), first_id)

        def incompatible(pair: dict, bindings: dict) -> str:
            del pair, bindings
            return "incompatible"

        with self.assertRaisesRegex(ReleaseStateError, "must return None"):
            self.store.rollback(expected_current=second_id, verify_pair=incompatible)
        self.assertEqual(self.store.read_selection()["pair_id"], second_id)

    def test_revoking_each_component_prevents_rollback_resurrection(self) -> None:
        first = self._pair("a")
        first_id = self._promote(first, self._bindings(), None)
        second_id = self._promote(self._pair("b"), self._bindings("two"), first_id)
        revocations: list[Path] = []
        for kind, digest in (
            ("data", first["data_revision"]),
            ("corpus", first["corpus_revision"]),
            ("software", first["software_sha256"]),
        ):
            record = self.store.revoke(
                kind,
                digest,
                reason=f"test {kind}",
                owner_ref="test:release-state",
            )
            self.assertEqual(record["schema_version"], REVOCATION_SCHEMA)
            revocations.append(self.store.revocations / kind / f"{digest}.json")
        with self.assertRaisesRegex(ReleaseStateError, "revoked"):
            self.store.rollback(expected_current=second_id, verify_pair=self._verify)
        self.assertEqual(self.store.read_selection()["pair_id"], second_id)
        self.assertTrue(all(path.is_file() for path in revocations))

    def test_current_revocation_does_not_delete_or_resurrect_revocation_on_rollback(self) -> None:
        first = self._pair("a")
        first_id = self._promote(first, self._bindings(), None)
        second = self._pair("b")
        second_id = self._promote(second, self._bindings("two"), first_id)
        record = self.store.revoke(
            "software",
            second["software_sha256"],
            reason="current test revocation",
            owner_ref="test:release-state",
        )
        revocation_path = self.store.revocations / "software" / f"{second['software_sha256']}.json"
        before = revocation_path.read_bytes()
        self.assertEqual(self.store.rollback(expected_current=second_id, verify_pair=self._verify), first_id)
        self.assertEqual(revocation_path.read_bytes(), before)
        self.assertEqual(json.loads(revocation_path.read_bytes()), record)

    def test_immutable_records_refuse_different_replacements(self) -> None:
        pair = self._pair("a")
        bindings = self._bindings()
        pair_id = self._promote(pair, bindings, None)
        pair_path = self.store.pairs / f"{pair_id}.json"
        original = pair_path.read_bytes()
        pair_path.write_bytes(original + b"tamper\n")
        with self.assertRaises(ReleaseStateError):
            self.store.promote(pair, bindings, expected_current=pair_id, verify_pair=self._verify)
        pair_path.write_bytes(original)

        record = self.store.revoke(
            "data",
            pair["data_revision"],
            reason="first reason",
            owner_ref="test:owner",
        )
        self.assertEqual(
            self.store.revoke(
                "data",
                pair["data_revision"],
                reason="first reason",
                owner_ref="test:owner",
            ),
            record,
        )
        with self.assertRaisesRegex(ReleaseStateError, "cannot be overwritten"):
            self.store.revoke(
                "data",
                pair["data_revision"],
                reason="different reason",
                owner_ref="test:owner",
            )

    def test_create_false_is_read_only_and_does_not_create_lock_or_state(self) -> None:
        state_root = self.root / "readonly-state"
        with self.assertRaises(ReleaseStateError):
            ReleaseStore(state_root, create=False)
        self.assertFalse(state_root.exists())

        source = self.root / "created-state"
        writer = ReleaseStore(source)
        pair = self._pair("a")
        bindings = self._bindings("readonly")
        pair_id = writer.promote(pair, bindings, expected_current=None, verify_pair=self._verify)
        lock_path = source / ".release.lock"
        lock_before = lock_path.read_bytes()
        entries_before = sorted(path.relative_to(source).as_posix() for path in source.rglob("*"))
        reader = ReleaseStore(source, create=False)
        self.assertEqual(reader.read_selection()["pair_id"], pair_id)
        reader.assert_available(pair)
        self.assertEqual(lock_path.read_bytes(), lock_before)
        self.assertEqual(
            sorted(path.relative_to(source).as_posix() for path in source.rglob("*")),
            entries_before,
        )

    def test_malformed_and_symlinked_state_is_rejected(self) -> None:
        with self.assertRaises(ReleaseStateError):
            self.store.promote(
                {"schema_version": PAIR_SCHEMA},
                self._bindings(),
                expected_current=None,
                verify_pair=self._verify,
            )
        with self.assertRaises(ReleaseStateError):
            self.store.promote(
                self._pair("a"),
                {"data_root": "relative", "software_archive": "/absolute"},
                expected_current=None,
                verify_pair=self._verify,
            )
        with self.assertRaises(ReleaseStateError):
            self.store.promote(
                self._pair("a"),
                self._bindings(),
                expected_current=None,
                verify_pair=None,  # type: ignore[arg-type]
            )

        link_target = self.root / "outside"
        link_target.mkdir()
        pairs = self.store.root / "pairs"
        pairs.rmdir()
        pairs.symlink_to(link_target, target_is_directory=True)
        with self.assertRaises(ReleaseStateError):
            ReleaseStore(self.store.root, create=False)

    def test_invalid_revocation_file_fails_closed(self) -> None:
        pair = self._pair("a")
        pair_id = self._promote(pair, self._bindings(), None)
        path = self.store.revocations / "data" / f"{pair['data_revision']}.json"
        path.write_bytes(b"{}\n")
        with self.assertRaisesRegex(ReleaseStateError, "revocation"):
            self.store.assert_available(pair)
        with self.assertRaises(ReleaseStateError):
            self.store.read_selection()
        self.assertEqual(json.loads(self.store.current_path.read_bytes())["current"], pair_id)

    def test_pair_id_is_canonical_and_binding_fields_are_exact(self) -> None:
        pair = self._pair("a")
        expected = hashlib.sha256(canonical(pair)).hexdigest()
        self.assertEqual(pair_id_for(pair), expected)
        self.assertEqual(BINDING_KEYS, frozenset({"data_root", "software_archive"}))


if __name__ == "__main__":
    unittest.main()
