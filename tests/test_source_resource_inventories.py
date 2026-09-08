from __future__ import annotations

import copy
import gzip
import hashlib
import json
import struct
import sys
import tempfile
import unittest
import zipfile
from jsonschema import Draft202012Validator
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import build_source_resource_inventories as inventories


class SourceResourceInventoryTests(unittest.TestCase):
    @staticmethod
    def _write_manifest_fixture(
        repo_root: Path, *, byte_size: int, sha256: str
    ) -> tuple[Path, bytes]:
        item_root = repo_root / "ToS/source-witnesses/fixture-item"
        payload = (
            b'<TEI xmlns="http://www.tei-c.org/ns/1.0"><text><body>'
            b'<pb n="1"/></body></text></TEI>'
        )
        payload_path = item_root / "payload/sample.xml"
        payload_path.parent.mkdir(parents=True, exist_ok=True)
        payload_path.write_bytes(payload)
        manifest_path = item_root / "item.manifest.json"
        manifest_path.write_text(
            json.dumps(
                {
                    "item_id": "tos.item.fixture",
                    "payload_files": [
                        {
                            "file_id": "tos.file.sha256.fixture",
                            "relative_path": "payload/sample.xml",
                            "media_type": "application/tei+xml",
                            "byte_size": byte_size,
                            "sha256": sha256,
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        return manifest_path, payload

    def test_manifest_inventory_rejects_payload_byte_size_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary) / "Tree-of-Sophia"
            manifest_path, _ = self._write_manifest_fixture(
                repo_root, byte_size=0, sha256="0" * 64
            )
            actual_payload = manifest_path.parent / "payload/sample.xml"
            actual_bytes = actual_payload.read_bytes()
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["payload_files"][0]["byte_size"] = len(actual_bytes) + 1
            manifest["payload_files"][0]["sha256"] = inventories._sha256_bytes(
                actual_bytes
            )
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

            with self.assertRaisesRegex(
                inventories.InventoryBuildError, "byte size differs from manifest"
            ):
                inventories.build_inventory(
                    repo_root=repo_root,
                    manifest_path=manifest_path,
                    payload_source_root=repo_root / "ToS/source-witnesses",
                    event_date="2026-08-20",
                )

    def test_manifest_inventory_rejects_payload_sha256_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary) / "Tree-of-Sophia"
            manifest_path, _ = self._write_manifest_fixture(
                repo_root, byte_size=0, sha256="0" * 64
            )
            actual_payload = manifest_path.parent / "payload/sample.xml"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["payload_files"][0]["byte_size"] = actual_payload.stat().st_size
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

            with self.assertRaisesRegex(
                inventories.InventoryBuildError, "SHA-256 differs from manifest"
            ):
                inventories.build_inventory(
                    repo_root=repo_root,
                    manifest_path=manifest_path,
                    payload_source_root=repo_root / "ToS/source-witnesses",
                    event_date="2026-08-20",
                )

    def _new_profile_fixture(self, content: bytes, media_type: str, suffix: str) -> dict:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            item = repo / "ToS/source-witnesses/fixture-item"
            payload = item / ("payload/source" + suffix)
            payload.parent.mkdir(parents=True)
            payload.write_bytes(content)
            manifest = {"item_id": "tos.item.fixture", "payload_files": [{
                "file_id": "tos.file.sha256." + hashlib.sha256(content).hexdigest(),
                "relative_path": "payload/source" + suffix, "media_type": media_type,
                "byte_size": len(content), "sha256": hashlib.sha256(content).hexdigest()}]}
            manifest_path = item / "item.manifest.json"
            manifest_path.write_text(json.dumps(manifest))
            result = inventories.build_inventory(repo_root=repo, manifest_path=manifest_path,
                payload_source_root=repo / "ToS/source-witnesses", event_date="2026-09-08")
            self.assertEqual(content, payload.read_bytes())
            self.assertEqual(hashlib.sha256(content).hexdigest(), result["files"][0]["file_sha256"])
            schema = json.loads((REPO_ROOT / "ToS/contracts/source-resource-inventory.schema.json").read_text())
            self.assertEqual([], list(Draft202012Validator(schema).iter_errors(result)))
            self.assertFalse(result["source_text_included"])
            self.assertEqual("mechanical_metadata_only", result["inventory_authority"])
            return result

    def test_osis_chapter_verse_order_identifiers_and_codepoints_are_preserved(self) -> None:
        # Non-canonical combining-mark order is intentional; no NFC is allowed.
        hebrew = "ש\u05c1\u05b8"
        content = (f'<osis xmlns="{inventories.OSIS_NS}"><osisText><div type="book" osisID="Prov">'
            f'<chapter osisID="Prov.1"><verse osisID="Prov.1.2"><w>{hebrew}</w></verse>'
            '<verse osisID="Prov.1.1"><w>private</w><w>words</w></verse></chapter>'
            '<chapter osisID="Prov.2"><verse osisID="Prov.2.1"><w>another</w></verse></chapter>'
            '</div></osisText></osis>').encode()
        result = self._new_profile_fixture(content, "application/xml", ".xml")
        file = result["files"][0]
        self.assertEqual("osis_structure_v1", file["profile"])
        self.assertEqual({"resource_count": 5, "chapter_count": 2, "verse_count": 3, "word_count": 4}, file["summary"])
        self.assertEqual(["Prov.1", "Prov.1.2", "Prov.1.1", "Prov.2", "Prov.2.1"], [r["locator"]["osis_id"] for r in file["resources"]])
        self.assertEqual("osis-chapter-0001", file["resources"][2]["locator"]["parent_resource_id"])
        fingerprint = file["resources"][1]["content_fingerprint"]
        self.assertEqual("xml-character-data-preserved", fingerprint["normalization"])
        self.assertEqual(hashlib.sha256(hebrew.encode()).hexdigest(), fingerprint["sha256"])
        self.assertNotEqual(inventories._fingerprint(hebrew)["sha256"], fingerprint["sha256"])
        serialized = json.dumps(result, ensure_ascii=False)
        for text in (hebrew, "private", "words", "another"):
            self.assertNotIn(text, serialized)
        # The profile contract also rejects accidentally applying the TEI normalizer.
        wrong = copy.deepcopy(result)
        wrong["files"][0]["resources"][1]["content_fingerprint"]["normalization"] = "unicode-nfc-whitespace-collapse"
        schema = json.loads((REPO_ROOT / "ToS/contracts/source-resource-inventory.schema.json").read_text())
        self.assertTrue(list(Draft202012Validator(schema).iter_errors(wrong)))

    def test_osis_rejects_wrong_namespace_duplicate_ids_and_milestones(self) -> None:
        bodies = [
            '<osis xmlns="urn:wrong"><osisText/></osis>',
            f'<osis xmlns="{inventories.OSIS_NS}"><osisText><chapter osisID="Prov.1"><verse osisID="Prov.1.1"/><verse osisID="Prov.1.1"/></chapter></osisText></osis>',
            f'<osis xmlns="{inventories.OSIS_NS}"><osisText><chapter osisID="Prov.1"><verse osisID="Prov.1.1" sID="Prov.1.1"/></chapter></osisText></osis>',
            f'<osis xmlns="{inventories.OSIS_NS}"><osisText><chapter osisID="Prov.1"><verse osisID="Job.1.1"/></chapter></osisText></osis>',
        ]
        for content in bodies:
            with self.subTest(content=content[:40]):
                with self.assertRaises(inventories.InventoryBuildError):
                    self._new_profile_fixture(content.encode(), "application/osis+xml", ".xml")

    def test_generic_xml_does_not_acquire_an_osis_or_tei_identity(self) -> None:
        for content in (b'<book><chapter><verse>private</verse></chapter></book>', b'<broken'):
            with self.subTest(content=content):
                with self.assertRaises(inventories.InventoryBuildError):
                    self._new_profile_fixture(content, "application/xml", ".xml")

    def test_json_top_level_order_nested_counts_and_fingerprints_hide_all_text(self) -> None:
        key = "private-key-e\u0301"
        text = "Source-e\u0301"
        value = {key: text, "second-key": [{"nested-key": "more-source"}, "tail-source", 12, True, None]}
        result = self._new_profile_fixture(json.dumps(value, ensure_ascii=False).encode(), "application/json", ".json")
        file = result["files"][0]
        self.assertEqual("json_members_v1", file["profile"])
        self.assertEqual(3, file["summary"]["resource_count"])
        self.assertEqual(2, file["summary"]["top_level_member_count"])
        self.assertEqual({"object_count": 2, "array_count": 1, "string_count": 3, "number_count": 1,
            "boolean_count": 1, "null_count": 1, "object_key_count": 3}, file["summary"]["json_value_counts"])
        first = file["resources"][1]
        self.assertEqual(1, first["locator"]["json_member_index"])
        self.assertEqual("string", first["locator"]["json_value_type"])
        self.assertEqual(hashlib.sha256(key.encode()).hexdigest(), first["label_fingerprint"]["sha256"])
        self.assertEqual(hashlib.sha256(text.encode()).hexdigest(), first["content_fingerprint"]["sha256"])
        self.assertEqual("unicode-codepoints-preserved", first["content_fingerprint"]["normalization"])
        self.assertEqual("array", file["resources"][2]["locator"]["json_value_type"])
        serialized = json.dumps(result, ensure_ascii=False)
        for source in (key, text, "second-key", "nested-key", "more-source", "tail-source"):
            self.assertNotIn(source, serialized)

    def test_json_arrays_and_empty_containers_have_honest_structure(self) -> None:
        for source, count in ((b'[]', 0), (b'{}', 0), (b'["source", {}, false]', 3)):
            result = self._new_profile_fixture(source, "application/json", ".json")["files"][0]
            self.assertEqual(count, result["summary"]["top_level_member_count"])
            self.assertEqual(count + 1, len(result["resources"]))
            self.assertTrue(all("label_fingerprint" not in row for row in result["resources"]))

    def test_json_duplicate_keys_at_any_depth_and_nonstandard_constants_fail_closed(self) -> None:
        for source in (b'{"a":1,"a":2}', b'{"outer":{"a":1,"\\u0061":2}}'):
            with self.subTest(source=source):
                with self.assertRaisesRegex(inventories.InventoryBuildError, "duplicate JSON object key"):
                    self._new_profile_fixture(source, "application/json", ".json")
        for source in (b'{"x":NaN}', b'{"x":Infinity}', b'{"x":-Infinity}', b'{broken', b'"scalar"'):
            with self.subTest(source=source):
                with self.assertRaises(inventories.InventoryBuildError):
                    self._new_profile_fixture(source, "application/json", ".json")

    @staticmethod
    def _djvu_page(width: int, height: int, dpi: int) -> bytes:
        info = (
            struct.pack(">HH", width, height)
            + bytes((25, 0))
            + struct.pack("<H", dpi)
            + bytes((22, 1))
        )
        info_chunk = b"INFO" + struct.pack(">I", len(info)) + info
        form_payload = b"DJVU" + info_chunk
        return b"FORM" + struct.pack(">I", len(form_payload)) + form_payload

    @classmethod
    def _bundled_djvu(cls) -> bytes:
        components = [
            cls._djvu_page(1000, 2000, 300),
            b"FORM" + struct.pack(">I", 4) + b"DJVI",
            cls._djvu_page(1100, 2100, 400),
        ]
        directory_size = 3 + 4 * len(components)
        directory_chunk_size = 8 + directory_size + (directory_size % 2)
        first_page_offset = 16 + directory_chunk_size
        offsets = []
        next_offset = first_page_offset
        for component in components:
            offsets.append(next_offset)
            next_offset += len(component)
        directory = (
            bytes((0x81,))
            + struct.pack(">H", len(components))
            + b"".join(struct.pack(">I", offset) for offset in offsets)
        )
        directory_chunk = (
            b"DIRM"
            + struct.pack(">I", len(directory))
            + directory
            + (b"\x00" if len(directory) % 2 else b"")
        )
        root_payload = b"DJVM" + directory_chunk + b"".join(components)
        return b"AT&TFORM" + struct.pack(">I", len(root_payload)) + root_payload

    def test_epub_inventory_preserves_order_and_hides_text(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample.epub"
            with zipfile.ZipFile(path, "w") as zf:
                zf.writestr("mimetype", "application/epub+zip")
                zf.writestr(
                    "META-INF/container.xml",
                    """<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
                    <rootfiles><rootfile full-path="EPUB/package.opf"/></rootfiles>
                    </container>""",
                )
                zf.writestr(
                    "EPUB/package.opf",
                    """<package xmlns="http://www.idpf.org/2007/opf">
                    <manifest>
                      <item id="p1" href="page.xhtml" media-type="application/xhtml+xml"/>
                    </manifest>
                    <spine><itemref idref="p1"/></spine>
                    </package>""",
                )
                zf.writestr(
                    "EPUB/page.xhtml",
                    "<html><body><p>Visible source words.</p></body></html>",
                )
            payload = inventories.build_file_inventory(
                path,
                {
                    "file_id": "tos.file.sha256." + "a" * 64,
                    "sha256": "a" * 64,
                    "media_type": "application/epub+zip",
                },
            )

        self.assertEqual("epub_resources_v1", payload["profile"])
        self.assertEqual(4, payload["summary"]["member_count"])
        member = next(
            item
            for item in payload["resources"]
            if item["locator"]["member_path"] == "EPUB/page.xhtml"
        )
        self.assertEqual(1, member["locator"]["spine_index"])
        self.assertIn("content_fingerprint", member)
        self.assertNotIn("Visible source words", json.dumps(payload))
        self.assertTrue(
            all("text" not in resource for resource in payload["resources"])
        )

    def test_tei_inventory_enumerates_page_breaks_and_divisions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample.xml"
            path.write_text(
                """<TEI xmlns="http://www.tei-c.org/ns/1.0"><text><body>
                <pb n="1" facs="#f1"/><div n="1"><head>Heading</head>
                <p>Source text.</p><div type="contents"><head>Contents</head></div>
                </div></body></text></TEI>""",
                encoding="utf-8",
            )
            payload = inventories.build_file_inventory(
                path,
                {
                    "file_id": "tos.file.sha256." + "b" * 64,
                    "sha256": "b" * 64,
                    "media_type": "application/tei+xml",
                },
            )

        self.assertEqual("tei_structure_v1", payload["profile"])
        self.assertEqual(1, payload["summary"]["page_break_count"])
        self.assertEqual(2, payload["summary"]["division_count"])
        self.assertEqual(2, payload["summary"]["max_division_depth"])
        contents = next(
            item
            for item in payload["resources"]
            if item["structural_role"] == "contents"
        )
        self.assertEqual("tei-div-0001", contents["locator"]["parent_resource_id"])
        serialized = json.dumps(payload)
        self.assertNotIn("Source text", serialized)
        self.assertNotIn("Heading", serialized)

    def test_jp2_zip_inventory_preserves_leaf_order_and_member_fixity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample_jp2.zip"
            with zipfile.ZipFile(path, "w") as zf:
                zf.writestr("sample_jp2/sample_0000.jp2", b"first-image")
                zf.writestr("sample_jp2/sample_0001.jp2", b"second-image")
            payload = inventories.build_file_inventory(
                path,
                {
                    "file_id": "tos.file.sha256." + "f" * 64,
                    "sha256": "f" * 64,
                    "media_type": "application/zip",
                    "relative_path": "payload/sample_jp2.zip",
                },
            )

        self.assertEqual("jp2_zip_pages_v1", payload["profile"])
        self.assertEqual(2, payload["summary"]["page_count"])
        self.assertEqual(2, payload["summary"]["member_count"])
        self.assertEqual(0, payload["resources"][0]["locator"]["leaf_number"])
        self.assertEqual(2, payload["resources"][1]["locator"]["page_index"])
        self.assertEqual(
            inventories._sha256_bytes(b"second-image"),
            payload["resources"][1]["sha256"],
        )

    def test_scandata_inventory_preserves_leaf_to_page_geometry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample_scandata.xml"
            path.write_text(
                """<book><bookData><dpi>600</dpi><leafCount>2</leafCount></bookData>
                <pageData><page leafNum="0"><origWidth>100</origWidth>
                <origHeight>200</origHeight></page><page leafNum="1">
                <origWidth>110</origWidth><origHeight>210</origHeight>
                </page></pageData></book>""",
                encoding="utf-8",
            )
            payload = inventories.build_file_inventory(
                path,
                {
                    "file_id": "tos.file.sha256." + "1" * 64,
                    "sha256": "1" * 64,
                    "media_type": "application/xml",
                    "relative_path": "payload/sample_scandata.xml",
                },
            )

        self.assertEqual("scandata_pages_v1", payload["profile"])
        self.assertEqual(2, payload["summary"]["page_count"])
        self.assertEqual(
            {
                "page_index": 2,
                "leaf_number": 1,
                "width_pixels": 110,
                "height_pixels": 210,
                "resolution_dpi": 600,
            },
            payload["resources"][1]["locator"],
        )

    def test_pdfinfo_geometry_parser_accepts_named_page_size_suffix(self) -> None:
        sizes, rotations = inventories._parse_pdf_page_geometries(
            "\n".join(
                [
                    "Page    1 size:  595 x 842 pts (A4)",
                    "Page    1 rot:   0",
                    "Page    2 size:  428.442 x 739.703 pts",
                    "Page    2 rot:   90",
                ]
            ),
            page_count=2,
        )

        self.assertEqual(
            {
                1: (595.0, 842.0),
                2: (428.442, 739.703),
            },
            sizes,
        )
        self.assertEqual({1: 0, 2: 90}, rotations)

    def test_djvu_xml_inventory_emits_geometry_counts_and_no_text(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample.djvu.xml"
            path.write_text(
                """<DjVuXML><BODY><OBJECT width="100" height="200">
                <PARAM name="DPI" value="300"/><REGION><PARAGRAPH><LINE>
                <WORD>Visible</WORD><WORD>source</WORD>
                </LINE></PARAGRAPH></REGION></OBJECT></BODY></DjVuXML>""",
                encoding="utf-8",
            )
            payload = inventories.build_file_inventory(
                path,
                {
                    "file_id": "tos.file.sha256." + "c" * 64,
                    "sha256": "c" * 64,
                    "media_type": "application/vnd.djvu+xml",
                    "relative_path": "payload/sample.djvu.xml",
                },
            )

        self.assertEqual("djvu_xml_pages_v1", payload["profile"])
        self.assertEqual(1, payload["summary"]["page_count"])
        self.assertEqual(1, payload["summary"]["paragraph_count"])
        self.assertEqual(1, payload["summary"]["line_count"])
        self.assertEqual(2, payload["summary"]["word_count"])
        self.assertEqual(
            {
                "page_index": 1,
                "width_pixels": 100,
                "height_pixels": 200,
                "resolution_dpi": 300,
            },
            payload["resources"][0]["locator"],
        )
        serialized = json.dumps(payload)
        self.assertNotIn("Visible", serialized)

    def test_bundled_djvu_inventory_emits_page_geometry_without_ocr(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample.djvu"
            path.write_bytes(self._bundled_djvu())
            payload = inventories.build_file_inventory(
                path,
                {
                    "file_id": "tos.file.sha256." + "e" * 64,
                    "sha256": "e" * 64,
                    "media_type": "image/vnd.djvu",
                    "relative_path": "payload/sample.djvu",
                },
            )

        self.assertEqual("djvu_pages_v1", payload["profile"])
        self.assertEqual(2, payload["summary"]["page_count"])
        self.assertEqual(2, payload["summary"]["distinct_page_geometry_count"])
        self.assertEqual(
            {
                "page_index": 1,
                "width_pixels": 1000,
                "height_pixels": 2000,
                "resolution_dpi": 300,
            },
            payload["resources"][0]["locator"],
        )
        self.assertEqual("djvu_page", payload["resources"][1]["resource_kind"])
        serialized = json.dumps(payload)
        self.assertNotIn("content_fingerprint", serialized)
        self.assertNotIn("word_count", serialized)

    def test_abbyy_xml_gzip_inventory_emits_counts_and_no_text(self) -> None:
        xml = b"""<document xmlns="http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml">
        <page width="101" height="201" resolution="400"><block><text><par><line>
        <formatting><charParams wordStart="true">V</charParams>
        <charParams>i</charParams><charParams wordStart="true">s</charParams>
        </formatting></line></par></text></block></page></document>"""
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "sample.abbyy.xml.gz"
            with gzip.open(path, "wb") as target:
                target.write(xml)
            payload = inventories.build_file_inventory(
                path,
                {
                    "file_id": "tos.file.sha256." + "d" * 64,
                    "sha256": "d" * 64,
                    "media_type": "application/gzip",
                    "relative_path": "payload/sample.abbyy.xml.gz",
                },
            )

        self.assertEqual("abbyy_xml_pages_v1", payload["profile"])
        self.assertEqual(1, payload["summary"]["page_count"])
        self.assertEqual(1, payload["summary"]["paragraph_count"])
        self.assertEqual(1, payload["summary"]["line_count"])
        self.assertEqual(2, payload["summary"]["word_count"])
        self.assertEqual(
            {
                "page_index": 1,
                "width_pixels": 101,
                "height_pixels": 201,
                "resolution_dpi": 400,
            },
            payload["resources"][0]["locator"],
        )
        self.assertNotIn("Vis", json.dumps(payload))


if __name__ == "__main__":
    unittest.main()
