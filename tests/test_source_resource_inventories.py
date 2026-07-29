from __future__ import annotations

import gzip
import json
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
