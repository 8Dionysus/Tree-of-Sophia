from __future__ import annotations

import gzip
import json
import struct
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import build_source_resource_inventories as inventories


class SourceResourceInventoryTests(unittest.TestCase):
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
