from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import zipfile


ACCESS_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ACCESS_ROOT.parent
PACKAGING_ROOT = ACCESS_ROOT / "packaging"
if PACKAGING_ROOT.as_posix() not in sys.path:
    sys.path.insert(0, PACKAGING_ROOT.as_posix())

from build_software_bundle import build_software_bundle  # noqa: E402
from validate_software_bundle import verify_archive  # noqa: E402


class SoftwareBundleBoundaryTests(unittest.TestCase):
    SOURCE_REF = "synthetic-fixture"
    SENTINEL = b"PRIVATE-CORPUS-SENTINEL\n"
    TOS_SCHEMAS = (
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/semantic-relation-type-registry.schema.json",
        "ToS/contracts/epistemic-evidence-projection.schema.json",
    )

    @staticmethod
    def _sha256(value: bytes) -> str:
        return hashlib.sha256(value).hexdigest()

    def _make_repo(self, root: Path) -> None:
        access = root / "access"
        for relative in ("pyproject.toml", "README.md", "packaging/tos_build_backend.py"):
            source = ACCESS_ROOT / relative
            target = access / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)

        source_package = ACCESS_ROOT / "src/tos_access"
        for source in sorted(source_package.rglob("*.py")):
            relative = source.relative_to(ACCESS_ROOT / "src")
            if {"runtime_data", "__pycache__"}.intersection(relative.parts):
                continue
            target = access / "src" / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)

        for directory_name in ("contracts", "profiles"):
            source_root = ACCESS_ROOT / directory_name
            for source in sorted(source_root.rglob("*.json")):
                relative = source.relative_to(source_root)
                target = access / directory_name / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, target)

        for relative in self.TOS_SCHEMAS:
            source = REPO_ROOT / relative
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)

        web_asset = access / "web/dist/assets/tos-graph.js"
        web_asset.parent.mkdir(parents=True, exist_ok=True)
        web_asset.write_bytes(b"synthetic graph asset\n")

        private_payload = root / "ToS/source-witnesses/private/payload.txt"
        private_payload.parent.mkdir(parents=True, exist_ok=True)
        private_payload.write_bytes(self.SENTINEL)
        (root / "data").mkdir()
        (root / "data/private-corpus.json").write_bytes(self.SENTINEL)
        (root / "tests").mkdir()
        (root / "tests/live-corpus.py").write_bytes(self.SENTINEL)

    def _build(self, root: Path, name: str = "candidate.zip") -> tuple[Path, dict]:
        output = root / name
        manifest = build_software_bundle(
            root,
            output,
            source_ref=self.SOURCE_REF,
            allow_dirty=True,
        )
        return output, manifest

    @staticmethod
    def _sidecar(bundle: Path) -> Path:
        return bundle.with_suffix(bundle.suffix + ".manifest.json")

    @staticmethod
    def _read_entries(bundle: Path) -> list[tuple[zipfile.ZipInfo, bytes]]:
        with zipfile.ZipFile(bundle) as archive:
            return [(info, archive.read(info.filename)) for info in archive.infolist()]

    @staticmethod
    def _replace_archive(bundle: Path, entries: list[tuple[zipfile.ZipInfo, bytes]]) -> None:
        replacement = bundle.with_name(bundle.name + ".rewrite")
        with zipfile.ZipFile(replacement, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for info, payload in entries:
                archive.writestr(info, payload)
        os.replace(replacement, bundle)

    def test_software_bundle_is_deterministic_and_excludes_corpus(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self._make_repo(root)
            first, first_manifest = self._build(root, "first.zip")
            second, second_manifest = self._build(root, "second.zip")

            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertEqual(self._sidecar(first).read_bytes(), self._sidecar(second).read_bytes())
            self.assertEqual(first_manifest, second_manifest)
            embedded = verify_archive(first)
            self.assertEqual(embedded["software_ref"], self.SOURCE_REF)
            self.assertFalse(embedded["data_included"])
            self.assertTrue(embedded["source_dirty"])

            with zipfile.ZipFile(first) as archive:
                names = archive.namelist()
                payload = b"".join(archive.read(name) for name in names)
            self.assertIn("access/src/tos_access/web_dist/assets/tos-graph.js", names)
            self.assertTrue(
                all(
                    name.startswith("access/src/tos_access/runtime_data/ToS/contracts/")
                    for name in names
                    if name.startswith("access/src/tos_access/runtime_data/ToS/contracts/")
                )
            )
            self.assertFalse(any(name.startswith("ToS/source-witnesses/") for name in names))
            self.assertFalse(any(name.startswith("data/") for name in names))
            self.assertFalse(any(name.startswith("tests/") for name in names))
            self.assertFalse(any(name.startswith("access/web/dist/") for name in names))
            self.assertNotIn(self.SENTINEL, payload)
            self.assertNotIn(b"access/web/dist", payload)

    def test_existing_output_and_sidecar_are_refused_without_overwrite(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self._make_repo(root)
            output = root / "candidate.zip"
            sidecar = self._sidecar(output)
            output_sentinel = b"existing archive sentinel"
            sidecar_sentinel = b"existing sidecar sentinel"
            output.write_bytes(output_sentinel)
            sidecar.write_bytes(sidecar_sentinel)

            with self.assertRaisesRegex(RuntimeError, "refusing to overwrite existing bundle"):
                build_software_bundle(root, output, source_ref=self.SOURCE_REF, allow_dirty=True)
            self.assertEqual(output.read_bytes(), output_sentinel)
            self.assertEqual(sidecar.read_bytes(), sidecar_sentinel)

            output.unlink()
            with self.assertRaisesRegex(RuntimeError, "refusing to overwrite existing bundle manifest"):
                build_software_bundle(root, output, source_ref=self.SOURCE_REF, allow_dirty=True)
            self.assertFalse(output.exists())
            self.assertEqual(sidecar.read_bytes(), sidecar_sentinel)

    def test_missing_web_asset_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self._make_repo(root)
            (root / "access/web/dist/assets/tos-graph.js").unlink()
            with self.assertRaisesRegex(
                RuntimeError,
                "web assets are missing: access/web/dist/assets/tos-graph.js",
            ):
                self._build(root)

    def test_symlink_web_source_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self._make_repo(root)
            external = root / "external-web"
            (external / "assets").mkdir(parents=True)
            (external / "assets/tos-graph.js").write_bytes(b"external web source\n")
            web_dist = root / "access/web/dist"
            shutil.rmtree(web_dist)
            web_dist.symlink_to(external, target_is_directory=True)
            with self.assertRaisesRegex(RuntimeError, "web assets are missing: access/web/dist"):
                self._build(root)

    def test_archive_corruption_without_sidecar_update_is_rejected_by_digest(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self._make_repo(root)
            bundle, _ = self._build(root)
            sidecar_before = self._sidecar(bundle).read_bytes()
            corrupted = bytearray(bundle.read_bytes())
            corrupted[len(corrupted) // 2] ^= 0x01
            bundle.write_bytes(corrupted)

            with self.assertRaisesRegex(RuntimeError, "software archive digest mismatch"):
                verify_archive(bundle)
            self.assertEqual(self._sidecar(bundle).read_bytes(), sidecar_before)

    def test_rewritten_member_is_rejected_even_when_external_archive_digest_is_recomputed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self._make_repo(root)
            bundle, _ = self._build(root)
            member_path = "access/README.md"
            entries = []
            for info, payload in self._read_entries(bundle):
                if info.filename == member_path:
                    payload += b"\nmutated outside embedded manifest\n"
                entries.append((info, payload))
            self._replace_archive(bundle, entries)

            sidecar = self._sidecar(bundle)
            external = json.loads(sidecar.read_text(encoding="utf-8"))
            external["archive_sha256"] = self._sha256(bundle.read_bytes())
            external["archive_size_bytes"] = bundle.stat().st_size
            sidecar.write_text(json.dumps(external, indent=2, sort_keys=True) + "\n", encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, f"software member integrity mismatch: {member_path}"):
                verify_archive(bundle)

    def test_unsafe_member_is_rejected_with_matching_embedded_and_external_hashes(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self._make_repo(root)
            bundle, _ = self._build(root)
            unsafe_path = "ToS/source-witnesses/private/payload.txt"
            unsafe_payload = self.SENTINEL
            entries = []
            manifest_info = None
            manifest = None
            for info, payload in self._read_entries(bundle):
                if info.filename == "software.manifest.json":
                    manifest_info = info
                    manifest = json.loads(payload)
                else:
                    entries.append((info, payload))
            self.assertIsNotNone(manifest_info)
            self.assertIsNotNone(manifest)
            manifest["members"].append(
                {
                    "path": unsafe_path,
                    "sha256": self._sha256(unsafe_payload),
                    "size_bytes": len(unsafe_payload),
                }
            )
            manifest["members"].sort(key=lambda item: item["path"])
            unsafe_info = zipfile.ZipInfo(unsafe_path, (1980, 1, 1, 0, 0, 0))
            unsafe_info.compress_type = zipfile.ZIP_DEFLATED
            unsafe_info.external_attr = 0o644 << 16
            entries.append((unsafe_info, unsafe_payload))
            entries.append(
                (
                    manifest_info,
                    (json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8"),
                )
            )
            self._replace_archive(bundle, entries)

            sidecar = self._sidecar(bundle)
            external = json.loads(sidecar.read_text(encoding="utf-8"))
            external.update(manifest)
            external["archive_sha256"] = self._sha256(bundle.read_bytes())
            external["archive_size_bytes"] = bundle.stat().st_size
            sidecar.write_text(
                json.dumps(external, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )

            self.assertIn(
                unsafe_path,
                {member["path"] for member in manifest["members"]},
            )
            with self.assertRaisesRegex(
                RuntimeError,
                f"non-software or unsafe archive member: {unsafe_path}",
            ):
                verify_archive(bundle)


if __name__ == "__main__":
    unittest.main()
