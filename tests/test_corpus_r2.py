from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from typing import Any, Callable


SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import corpus_r2


class FakeTransport:
    """A memory-backed transport with per-fetch fault injection."""

    def __init__(self) -> None:
        self.objects: dict[str, bytes] = {}
        self.fetch_calls: list[str] = []
        self.put_calls: list[tuple[str, int, str, str]] = []
        self.fetch_sequences: dict[str, list[bool | bytes]] = {}
        self.on_put: Callable[[str], None] | None = None

    def fetch(self, key: str, destination: Path) -> bool:
        self.fetch_calls.append(key)
        sequence = self.fetch_sequences.get(key)
        if sequence:
            value = sequence.pop(0)
            if value is False:
                return False
            payload = value
        else:
            if key not in self.objects:
                return False
            payload = self.objects[key]
        Path(destination).write_bytes(payload)
        return True

    def put(
        self,
        key: str,
        source: Path,
        *,
        byte_size: int,
        media_type: str,
        storage_class: str,
    ) -> None:
        payload = Path(source).read_bytes()
        self.put_calls.append((key, byte_size, media_type, storage_class))
        if len(payload) != byte_size:
            raise AssertionError("fake transport received a wrong byte size")
        self.objects[key] = payload
        if self.on_put is not None:
            callback, self.on_put = self.on_put, None
            callback(key)


class CorpusR2Tests(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory()
        self.root = Path(self._temporary.name)
        self.scratch = self.root / "scratch"
        self.scratch.mkdir()
        self.transport = FakeTransport()

    def tearDown(self) -> None:
        self._temporary.cleanup()

    def _source(self, data: bytes = b"abcdefghij") -> Path:
        source = self.root / "source.bin"
        source.write_bytes(data)
        return source

    def _upload(self, data: bytes = b"abcdefghij", *, chunk_bytes: int = 4) -> tuple[Path, dict[str, Any]]:
        source = self._source(data)
        result = corpus_r2.upload_file(
            self.transport,
            source,
            prefix="fixture",
            expected_sha256=hashlib.sha256(data).hexdigest(),
            expected_size=len(data),
            scratch_root=self.scratch,
            chunk_bytes=chunk_bytes,
        )
        return source, result

    def _tamper_manifest(self, mutate: Callable[[dict[str, Any]], None]) -> tuple[str, str]:
        _, result = self._upload()
        key = result["manifest_key"]
        payload = json.loads(self.transport.objects[key].decode("utf-8"))
        mutate(payload)
        encoded = json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8") + b"\n"
        self.transport.objects[key] = encoded
        return key, hashlib.sha256(encoded).hexdigest()

    def test_multi_chunk_round_trip_and_canonical_manifest(self) -> None:
        data = b"abcdefghij"
        source, result = self._upload(data)
        self.assertEqual(result["file_sha256"], hashlib.sha256(data).hexdigest())
        self.assertEqual(result["file_size_bytes"], len(data))
        self.assertEqual(result["chunk_count"], 3)
        self.assertTrue(result["readback_verified"])
        manifest = self.transport.objects[result["manifest_key"]]
        self.assertTrue(manifest.endswith(b"\n"))
        self.assertEqual(
            manifest,
            corpus_r2._canonical_json(json.loads(manifest.decode("utf-8"))),
        )

        output = self.root / "restored.bin"
        restored = corpus_r2.restore_file(
            self.transport,
            manifest_key=result["manifest_key"],
            expected_manifest_sha256=result["manifest_sha256"],
            output=output,
            scratch_root=self.scratch,
        )
        self.assertEqual(output.read_bytes(), data)
        self.assertEqual(restored["chunk_count"], 3)
        self.assertEqual(source.read_bytes(), data)

        part_keys = [key for key in self.transport.objects if "/parts/" in key]
        self.assertEqual(
            sorted(part_keys),
            [
                "fixture/files/sha256/"
                + hashlib.sha256(data).hexdigest()
                + "/parts/00000000-"
                + hashlib.sha256(b"abcd").hexdigest(),
                "fixture/files/sha256/"
                + hashlib.sha256(data).hexdigest()
                + "/parts/00000001-"
                + hashlib.sha256(b"efgh").hexdigest(),
                "fixture/files/sha256/"
                + hashlib.sha256(data).hexdigest()
                + "/parts/00000002-"
                + hashlib.sha256(b"ij").hexdigest(),
            ],
        )

    def test_empty_file_round_trip(self) -> None:
        data = b""
        source, result = self._upload(data)
        output = self.root / "empty.bin"
        corpus_r2.restore_file(
            self.transport,
            manifest_key=result["manifest_key"],
            expected_manifest_sha256=result["manifest_sha256"],
            output=output,
            scratch_root=self.scratch,
        )
        self.assertEqual(output.read_bytes(), data)
        self.assertEqual(source.read_bytes(), data)

    def test_existing_corrupt_part_is_refused_without_put(self) -> None:
        _, result = self._upload()
        part_key = next(key for key in self.transport.objects if "/parts/" in key)
        self.transport.objects[part_key] = b"corrupt"
        puts_before = len(self.transport.put_calls)
        with self.assertRaises(corpus_r2.CorpusR2Error):
            self._upload()
        self.assertEqual(len(self.transport.put_calls), puts_before)

    def test_failed_part_readback_does_not_publish_manifest(self) -> None:
        data = b"abcdefghij"
        source = self._source(data)
        full_sha = hashlib.sha256(data).hexdigest()
        first_part_key = (
            "fixture/files/sha256/"
            + full_sha
            + "/parts/00000000-"
            + hashlib.sha256(b"abcd").hexdigest()
        )
        self.transport.fetch_sequences[first_part_key] = [False, b"wrong readback"]
        with self.assertRaises(corpus_r2.CorpusR2Error):
            corpus_r2.upload_file(
                self.transport,
                source,
                prefix="fixture",
                expected_sha256=full_sha,
                expected_size=len(data),
                scratch_root=self.scratch,
                chunk_bytes=4,
            )
        manifest_key = "fixture/files/sha256/" + full_sha + "/manifest.json"
        self.assertNotIn(manifest_key, self.transport.objects)
        self.assertNotIn(manifest_key, [call[0] for call in self.transport.put_calls])

    def test_source_mutation_prevents_manifest(self) -> None:
        data = b"abcdefghij"
        source = self._source(data)
        full_sha = hashlib.sha256(data).hexdigest()

        def mutate_source(_key: str) -> None:
            source.write_bytes(b"abcdZZZZij")

        self.transport.on_put = mutate_source
        with self.assertRaises(corpus_r2.CorpusR2Error):
            corpus_r2.upload_file(
                self.transport,
                source,
                prefix="fixture",
                expected_sha256=full_sha,
                expected_size=len(data),
                scratch_root=self.scratch,
                chunk_bytes=4,
            )
        self.assertNotIn("fixture/files/sha256/" + full_sha + "/manifest.json", self.transport.objects)

    def test_resume_reuses_exact_objects(self) -> None:
        _, result = self._upload()
        puts_before = list(self.transport.put_calls)
        calls_before = len(self.transport.fetch_calls)
        second = self._upload()[1]
        self.assertEqual(second, result)
        self.assertEqual(self.transport.put_calls, puts_before)
        self.assertGreater(len(self.transport.fetch_calls), calls_before)

    def test_existing_output_and_broken_symlink_are_never_overwritten(self) -> None:
        _, result = self._upload()
        output = self.root / "existing.bin"
        output.write_bytes(b"keep")
        fetch_count = len(self.transport.fetch_calls)
        with self.assertRaises(corpus_r2.CorpusR2Error):
            corpus_r2.restore_file(
                self.transport,
                manifest_key=result["manifest_key"],
                expected_manifest_sha256=result["manifest_sha256"],
                output=output,
                scratch_root=self.scratch,
            )
        self.assertEqual(output.read_bytes(), b"keep")
        self.assertEqual(len(self.transport.fetch_calls), fetch_count)

        broken = self.root / "broken-link"
        broken.symlink_to(self.root / "missing-target")
        with self.assertRaises(corpus_r2.CorpusR2Error):
            corpus_r2.restore_file(
                self.transport,
                manifest_key=result["manifest_key"],
                expected_manifest_sha256=result["manifest_sha256"],
                output=broken,
                scratch_root=self.scratch,
            )
        self.assertTrue(broken.is_symlink())

    def test_tampered_manifest_hash_order_offset_and_key_are_rejected(self) -> None:
        variants: list[Callable[[dict[str, Any]], None]] = [
            lambda payload: payload["chunks"][0].__setitem__("sha256", "0" * 64),
            lambda payload: payload["chunks"].reverse(),
            lambda payload: payload["chunks"][1].__setitem__("offset", 0),
            lambda payload: payload["chunks"][0].__setitem__("key", "fixture/arbitrary"),
        ]
        for index, mutate in enumerate(variants):
            with self.subTest(variant=index):
                self.transport = FakeTransport()
                key, digest = self._tamper_manifest(mutate)
                output = self.root / f"tampered-{index}.bin"
                with self.assertRaises(corpus_r2.CorpusR2Error):
                    corpus_r2.restore_file(
                        self.transport,
                        manifest_key=key,
                        expected_manifest_sha256=digest,
                        output=output,
                        scratch_root=self.scratch,
                    )
                self.assertFalse(output.exists())

    def test_missing_or_corrupt_chunk_leaves_no_output(self) -> None:
        _, result = self._upload()
        part_key = next(key for key in self.transport.objects if "/parts/" in key)
        del self.transport.objects[part_key]
        missing_output = self.root / "missing.bin"
        with self.assertRaises(corpus_r2.CorpusR2Error):
            corpus_r2.restore_file(
                self.transport,
                manifest_key=result["manifest_key"],
                expected_manifest_sha256=result["manifest_sha256"],
                output=missing_output,
                scratch_root=self.scratch,
            )
        self.assertFalse(missing_output.exists())

        _, result = self._upload()
        part_key = next(key for key in self.transport.objects if "/parts/" in key)
        self.transport.objects[part_key] = b"corrupt"
        corrupt_output = self.root / "corrupt.bin"
        with self.assertRaises(corpus_r2.CorpusR2Error):
            corpus_r2.restore_file(
                self.transport,
                manifest_key=result["manifest_key"],
                expected_manifest_sha256=result["manifest_sha256"],
                output=corrupt_output,
                scratch_root=self.scratch,
            )
        self.assertFalse(corrupt_output.exists())

    def test_source_and_prefix_validation(self) -> None:
        source = self._source(b"safe")
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        for prefix in ("../unsafe", "/absolute", "a\\b", "a//b", "a/../b"):
            with self.subTest(prefix=prefix):
                with self.assertRaises(corpus_r2.CorpusR2Error):
                    corpus_r2.upload_file(
                        self.transport,
                        source,
                        prefix=prefix,
                        expected_sha256=digest,
                        expected_size=4,
                        scratch_root=self.scratch,
                        chunk_bytes=2,
                    )
        link = self.root / "source-link"
        link.symlink_to(source)
        with self.assertRaises(corpus_r2.CorpusR2Error):
            corpus_r2.upload_file(
                self.transport,
                link,
                prefix="fixture",
                expected_sha256=digest,
                expected_size=4,
                scratch_root=self.scratch,
                chunk_bytes=2,
            )


if __name__ == "__main__":
    unittest.main()
