from __future__ import annotations

import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import build_source_witness_catalog  # noqa: E402
import corpus_build_worker  # noqa: E402
import corpus_store  # noqa: E402


class CorpusBuildWorkerTests(unittest.TestCase):
    def test_failed_producer_cannot_mutate_admitted_source_or_cas(self) -> None:
        relative = "ToS/source-witnesses/fixture.json"
        original = b'{"fixture":"synthetic-source","version":1}\n'
        validator_sha256 = "a" * 64

        with tempfile.TemporaryDirectory(prefix="corpus-build-worker-") as raw:
            root = Path(raw)
            source = root / "input" / relative
            source.parent.mkdir(parents=True)
            source.write_bytes(original)

            store = corpus_store.CorpusStore(root / "store")
            source_digest = hashlib.sha256(original).hexdigest()

            def validate(
                candidate: corpus_store.CorpusCandidate,
                base: dict | None,
                affected: frozenset[str],
            ) -> corpus_store.ValidationIndex:
                del base, affected
                self.assertEqual(candidate.read_bytes(relative), original)
                return corpus_store.ValidationIndex(
                    {"synthetic:fixture": relative},
                    {},
                )

            snapshot = store.admit(
                base_revision=None,
                updates={
                    relative: {
                        "source": source,
                        "sha256": source_digest,
                        "size_bytes": len(original),
                        "mode": 0o644,
                    }
                },
                retirements={},
                validator_sha256=validator_sha256,
                validate=validate,
            )
            revision = snapshot["revision"]
            cas_object = store.root / "objects" / source_digest
            cas_before = cas_object.read_bytes()
            source_before = source.read_bytes()
            cas_digest_before = corpus_store.digest_file(cas_object)
            source_digest_before = corpus_store.digest_file(source)
            current_before = store.current()
            output = root / "built" / "snapshot"

            def producer_failure(view: Path) -> dict:
                producer_input = view / relative
                self.assertTrue(producer_input.is_file())
                producer_input.write_bytes(b"producer-corruption\n")
                raise RuntimeError("synthetic producer failure")

            with patch.object(
                build_source_witness_catalog,
                "render_outputs",
                side_effect=producer_failure,
            ):
                with self.assertRaisesRegex(RuntimeError, "synthetic producer failure"):
                    corpus_build_worker.compile_revision(store.root, revision, output)

            self.assertEqual(source.read_bytes(), source_before)
            self.assertEqual(corpus_store.digest_file(source), source_digest_before)
            self.assertEqual(cas_object.read_bytes(), cas_before)
            self.assertEqual(corpus_store.digest_file(cas_object), cas_digest_before)
            self.assertEqual(store.current(), current_before)
            self.assertFalse(output.exists())
            self.assertEqual(store.load(revision, verify_objects=True), snapshot)


if __name__ == "__main__":
    unittest.main()
