#!/usr/bin/env python3
"""Build tracked, text-free resource inventories from local source payloads.

The inventories enumerate PDF and bundled DjVu pages, EPUB container resources,
and TEI page breaks/divisions, plus page geometry and counts from provider
DjVu/ABBYY OCR companions. They may contain one-way fingerprints of source text
or OCR but never source text itself. Bibliographic, textual, linguistic, rights,
and semantic judgments remain outside this generator.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import mimetypes
import posixpath
import re
import subprocess
import sys
import unicodedata
import xml.etree.ElementTree as ET
import zipfile
from collections import Counter
from html.parser import HTMLParser
from pathlib import Path
from typing import Any, Iterable


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = Path("ToS/source-witnesses")
INVENTORY_NAME = "resource-inventory.json"
SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/source-resource-inventory.schema.json"
)
AUTHORITY_BOUNDARY = (
    "resource enumeration, geometry, ordering, counts, and one-way fingerprints "
    "only; no source text, bibliographic acceptance, textual acceptance, rights "
    "clearance, translation, semantics, or canon authority"
)
TEI_NS = "http://www.tei-c.org/ns/1.0"


class InventoryBuildError(RuntimeError):
    """Raised when a payload cannot produce an honest deterministic inventory."""


class _VisibleTextParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self.hidden_depth = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag.lower() in {"script", "style"}:
            self.hidden_depth += 1

    def handle_endtag(self, tag: str) -> None:
        if tag.lower() in {"script", "style"} and self.hidden_depth:
            self.hidden_depth -= 1

    def handle_data(self, data: str) -> None:
        if not self.hidden_depth:
            self.parts.append(data)


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _verify_payload_fixity(
    payload_path: Path, payload_entry: dict[str, Any]
) -> None:
    expected_size = payload_entry.get("byte_size")
    expected_sha256 = payload_entry.get("sha256")
    relative_path = payload_entry.get("relative_path", payload_path.name)
    if not isinstance(expected_size, int) or isinstance(expected_size, bool):
        raise InventoryBuildError(
            f"manifest payload byte_size is invalid for {relative_path}"
        )
    if not isinstance(expected_sha256, str):
        raise InventoryBuildError(
            f"manifest payload sha256 is invalid for {relative_path}"
        )
    try:
        actual_size = payload_path.stat().st_size
        actual_sha256 = _sha256_path(payload_path)
    except OSError as exc:
        raise InventoryBuildError(
            f"cannot verify payload fixity for {relative_path}: {exc}"
        ) from exc
    if actual_size != expected_size:
        raise InventoryBuildError(
            f"payload byte size differs from manifest for {relative_path}: "
            f"expected {expected_size}, got {actual_size}"
        )
    if actual_sha256 != expected_sha256:
        raise InventoryBuildError(
            f"payload SHA-256 differs from manifest for {relative_path}: "
            f"expected {expected_sha256}, got {actual_sha256}"
        )


def _normalize_text(text: str) -> str:
    return " ".join(unicodedata.normalize("NFC", text).split())


def _fingerprint(text: str) -> dict[str, Any]:
    normalized = _normalize_text(text)
    return {
        "algorithm": "sha256",
        "normalization": "unicode-nfc-whitespace-collapse",
        "sha256": _sha256_bytes(normalized.encode("utf-8")),
        "character_count": len(normalized),
    }


def _run(command: tuple[str, ...]) -> str:
    result = subprocess.run(
        command,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if result.returncode:
        raise InventoryBuildError(
            f"{' '.join(command[:2])} failed with {result.returncode}: "
            f"{result.stderr.strip()}"
        )
    return result.stdout


def _parse_pdf_page_geometries(
    box_info: str,
    *,
    page_count: int,
) -> tuple[dict[int, tuple[float, float]], dict[int, int]]:
    sizes = {
        int(match.group(1)): (float(match.group(2)), float(match.group(3)))
        for match in re.finditer(
            r"^Page\s+(\d+)\s+size:\s+([0-9.]+)\s+x\s+([0-9.]+)\s+pts"
            r"(?:\s+\([^)]*\))?\s*$",
            box_info,
            re.MULTILINE,
        )
    }
    rotations = {
        int(match.group(1)): int(match.group(2))
        for match in re.finditer(
            r"^Page\s+(\d+)\s+rot:\s+(-?\d+)\s*$",
            box_info,
            re.MULTILINE,
        )
    }
    if set(sizes) != set(range(1, page_count + 1)):
        raise InventoryBuildError("pdfinfo did not enumerate every declared page")
    return sizes, rotations


def _pdf_inventory(
    payload_path: Path,
    *,
    file_id: str,
    file_sha256: str,
    media_type: str,
) -> dict[str, Any]:
    base_info = _run(("pdfinfo", str(payload_path)))
    pages_match = re.search(r"^Pages:\s+(\d+)\s*$", base_info, re.MULTILINE)
    if pages_match is None:
        raise InventoryBuildError(
            f"pdfinfo did not report a page count for {payload_path}"
        )
    page_count = int(pages_match.group(1))

    box_info = _run(
        ("pdfinfo", "-f", "1", "-l", str(page_count), "-box", str(payload_path))
    )
    try:
        sizes, rotations = _parse_pdf_page_geometries(
            box_info,
            page_count=page_count,
        )
    except InventoryBuildError as exc:
        raise InventoryBuildError(f"{exc}: {payload_path}") from exc

    image_counts: Counter[int] = Counter()
    for line in _run(("pdfimages", "-list", str(payload_path))).splitlines():
        match = re.match(r"^\s*(\d+)\s+\d+\s+(?:image|mask|smask)\s+", line)
        if match:
            image_counts[int(match.group(1))] += 1

    resources: list[dict[str, Any]] = []
    for page_index in range(1, page_count + 1):
        width, height = sizes[page_index]
        resources.append(
            {
                "resource_id": f"pdf-page-{page_index:04d}",
                "resource_kind": "pdf_page",
                "locator": {
                    "page_index": page_index,
                    "width_points": width,
                    "height_points": height,
                    "rotation_degrees": rotations.get(page_index, 0),
                },
                "structural_role": "page",
                "image_resource_count": image_counts[page_index],
            }
        )

    geometries = {
        (resource["locator"]["width_points"], resource["locator"]["height_points"])
        for resource in resources
    }
    return {
        "file_id": file_id,
        "file_sha256": file_sha256,
        "media_type": media_type,
        "profile": "pdf_pages_v1",
        "summary": {
            "resource_count": len(resources),
            "page_count": page_count,
            "image_resource_count": sum(image_counts.values()),
            "distinct_page_geometry_count": len(geometries),
        },
        "resources": resources,
    }


def _epub_package(
    zf: zipfile.ZipFile,
) -> tuple[dict[str, dict[str, str]], dict[str, int]]:
    try:
        container = ET.fromstring(zf.read("META-INF/container.xml"))
    except (KeyError, ET.ParseError) as exc:
        raise InventoryBuildError(
            f"EPUB container metadata is unreadable: {exc}"
        ) from exc
    rootfile = container.find(".//{*}rootfile")
    if rootfile is None or not rootfile.get("full-path"):
        raise InventoryBuildError("EPUB container has no rootfile")
    opf_path = rootfile.get("full-path")
    assert opf_path is not None
    try:
        package = ET.fromstring(zf.read(opf_path))
    except (KeyError, ET.ParseError) as exc:
        raise InventoryBuildError(
            f"EPUB package metadata is unreadable: {exc}"
        ) from exc

    opf_parent = posixpath.dirname(opf_path)
    manifest: dict[str, dict[str, str]] = {}
    id_to_path: dict[str, str] = {}
    for item in package.findall(".//{*}manifest/{*}item"):
        item_id = item.get("id")
        href = item.get("href")
        if not item_id or not href:
            continue
        member_path = posixpath.normpath(posixpath.join(opf_parent, href))
        id_to_path[item_id] = member_path
        manifest[member_path] = {
            "media_type": item.get("media-type") or "application/octet-stream",
            "properties": item.get("properties") or "",
        }
    spine = {}
    for index, itemref in enumerate(package.findall(".//{*}spine/{*}itemref"), start=1):
        member_path = id_to_path.get(itemref.get("idref") or "")
        if member_path:
            spine[member_path] = index
    return manifest, spine


def _html_fingerprint(payload: bytes) -> dict[str, Any]:
    parser = _VisibleTextParser()
    parser.feed(payload.decode("utf-8", errors="replace"))
    parser.close()
    return _fingerprint(" ".join(parser.parts))


def _epub_inventory(
    payload_path: Path,
    *,
    file_id: str,
    file_sha256: str,
    media_type: str,
) -> dict[str, Any]:
    resources: list[dict[str, Any]] = []
    with zipfile.ZipFile(payload_path) as zf:
        bad_member = zf.testzip()
        if bad_member is not None:
            raise InventoryBuildError(
                f"EPUB member failed CRC verification: {bad_member}"
            )
        manifest, spine = _epub_package(zf)
        for container_order, info in enumerate(
            (entry for entry in zf.infolist() if not entry.is_dir()),
            start=1,
        ):
            payload = zf.read(info.filename)
            declared = manifest.get(info.filename, {})
            guessed = mimetypes.guess_type(info.filename)[0]
            member_media_type = (
                declared.get("media_type")
                or ("application/epub+zip" if info.filename == "mimetype" else None)
                or guessed
                or "application/octet-stream"
            )
            properties = set(declared.get("properties", "").split())
            if info.filename == "META-INF/container.xml":
                role = "container_metadata"
            elif info.filename.endswith(".opf"):
                role = "package_metadata"
            elif "nav" in properties:
                role = "navigation"
            elif info.filename in spine:
                role = "spine_resource"
            else:
                role = "auxiliary_resource"
            locator: dict[str, Any] = {
                "member_path": info.filename,
                "container_order": container_order,
            }
            if info.filename in spine:
                locator["spine_index"] = spine[info.filename]
            resource: dict[str, Any] = {
                "resource_id": f"epub-member-{container_order:04d}",
                "resource_kind": "epub_member",
                "locator": locator,
                "media_type": member_media_type,
                "byte_size": info.file_size,
                "sha256": _sha256_bytes(payload),
                "structural_role": role,
            }
            if member_media_type in {
                "application/xhtml+xml",
                "text/html",
            }:
                resource["content_fingerprint"] = _html_fingerprint(payload)
            resources.append(resource)

    xhtml_count = sum(
        1
        for resource in resources
        if resource.get("media_type") in {"application/xhtml+xml", "text/html"}
    )
    image_count = sum(
        1
        for resource in resources
        if str(resource.get("media_type", "")).startswith("image/")
    )
    return {
        "file_id": file_id,
        "file_sha256": file_sha256,
        "media_type": media_type,
        "profile": "epub_resources_v1",
        "summary": {
            "resource_count": len(resources),
            "member_count": len(resources),
            "spine_item_count": len(
                {
                    resource["locator"]["spine_index"]
                    for resource in resources
                    if "spine_index" in resource["locator"]
                }
            ),
            "xhtml_count": xhtml_count,
            "image_resource_count": image_count,
        },
        "resources": resources,
    }


def _jp2_zip_inventory(
    payload_path: Path,
    *,
    file_id: str,
    file_sha256: str,
    media_type: str,
) -> dict[str, Any]:
    resources: list[dict[str, Any]] = []
    member_pattern = re.compile(r"(?:^|/)[^/]+_(\d{4})\.jp2$")
    try:
        with zipfile.ZipFile(payload_path) as zf:
            bad_member = zf.testzip()
            if bad_member is not None:
                raise InventoryBuildError(
                    f"JP2 ZIP member failed CRC verification: {bad_member}"
                )
            members = [entry for entry in zf.infolist() if not entry.is_dir()]
            for container_order, info in enumerate(members, start=1):
                match = member_pattern.search(info.filename)
                if match is None:
                    raise InventoryBuildError(
                        f"JP2 ZIP contains a non-page member: {info.filename}"
                    )
                leaf_number = int(match.group(1))
                if leaf_number != container_order - 1:
                    raise InventoryBuildError(
                        "JP2 ZIP leaf numbering is not zero-based and contiguous"
                    )
                member = zf.read(info.filename)
                resources.append(
                    {
                        "resource_id": f"jp2-page-{container_order:04d}",
                        "resource_kind": "image_page",
                        "locator": {
                            "page_index": container_order,
                            "leaf_number": leaf_number,
                            "member_path": info.filename,
                            "container_order": container_order,
                        },
                        "media_type": "image/jp2",
                        "byte_size": info.file_size,
                        "sha256": _sha256_bytes(member),
                        "structural_role": "page",
                    }
                )
    except (OSError, zipfile.BadZipFile) as exc:
        raise InventoryBuildError(f"JP2 ZIP is unreadable: {exc}") from exc
    if not resources:
        raise InventoryBuildError("JP2 ZIP yielded no page resources")
    return {
        "file_id": file_id,
        "file_sha256": file_sha256,
        "media_type": media_type,
        "profile": "jp2_zip_pages_v1",
        "summary": {
            "resource_count": len(resources),
            "page_count": len(resources),
            "member_count": len(resources),
        },
        "resources": resources,
    }


def _scandata_inventory(
    payload_path: Path,
    *,
    file_id: str,
    file_sha256: str,
    media_type: str,
) -> dict[str, Any]:
    try:
        root = ET.parse(payload_path).getroot()
    except (OSError, ET.ParseError) as exc:
        raise InventoryBuildError(f"scandata XML is unreadable: {exc}") from exc
    if _local_name(root.tag) != "book":
        raise InventoryBuildError("scandata XML root is not book")
    leaf_count_text = root.findtext("./bookData/leafCount")
    dpi_text = root.findtext("./bookData/dpi")
    try:
        leaf_count = int(leaf_count_text or "")
        dpi = int(dpi_text or "")
    except ValueError as exc:
        raise InventoryBuildError("scandata XML has invalid book geometry") from exc
    resources: list[dict[str, Any]] = []
    for page_index, page in enumerate(root.findall("./pageData/page"), start=1):
        try:
            leaf_number = int(page.attrib["leafNum"])
            width = int(page.findtext("origWidth") or "")
            height = int(page.findtext("origHeight") or "")
        except (KeyError, ValueError) as exc:
            raise InventoryBuildError(
                f"scandata page {page_index} has invalid identity or geometry"
            ) from exc
        if leaf_number != page_index - 1:
            raise InventoryBuildError(
                "scandata leaf numbering is not zero-based and contiguous"
            )
        resources.append(
            {
                "resource_id": f"scandata-page-{page_index:04d}",
                "resource_kind": "scan_data_page",
                "locator": {
                    "page_index": page_index,
                    "leaf_number": leaf_number,
                    "width_pixels": width,
                    "height_pixels": height,
                    "resolution_dpi": dpi,
                },
                "structural_role": "page",
            }
        )
    if len(resources) != leaf_count:
        raise InventoryBuildError(
            f"scandata leaf count drifted: declared {leaf_count}, got {len(resources)}"
        )
    geometries = {
        (
            resource["locator"]["width_pixels"],
            resource["locator"]["height_pixels"],
            resource["locator"]["resolution_dpi"],
        )
        for resource in resources
    }
    return {
        "file_id": file_id,
        "file_sha256": file_sha256,
        "media_type": media_type,
        "profile": "scandata_pages_v1",
        "summary": {
            "resource_count": len(resources),
            "page_count": len(resources),
            "distinct_page_geometry_count": len(geometries),
        },
        "resources": resources,
    }


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _element_path(parent_path: str, element: ET.Element, index: int) -> str:
    return f"{parent_path}/{_local_name(element.tag)}[{index}]"


def _tei_inventory(
    payload_path: Path,
    *,
    file_id: str,
    file_sha256: str,
    media_type: str,
) -> dict[str, Any]:
    try:
        root = ET.parse(payload_path).getroot()
    except ET.ParseError as exc:
        raise InventoryBuildError(f"TEI XML is not well formed: {exc}") from exc
    text = root.find(f".//{{{TEI_NS}}}text")
    if text is None:
        raise InventoryBuildError("TEI payload has no text element")

    resources: list[dict[str, Any]] = []
    counters = {"pb": 0, "div": 0}
    page_state: list[str | None] = [None]
    max_depth = 0

    def walk(
        element: ET.Element,
        *,
        path: str,
        division_depth: int,
        parent_division_id: str | None,
    ) -> None:
        nonlocal max_depth
        sibling_counts: Counter[str] = Counter()
        for child in element:
            name = _local_name(child.tag)
            sibling_counts[name] += 1
            child_path = _element_path(path, child, sibling_counts[name])
            if name == "pb":
                counters["pb"] += 1
                page_label = child.get("n")
                facs = child.get("facs")
                if page_label:
                    page_state[0] = page_label
                elif facs:
                    page_state[0] = facs
                locator: dict[str, Any] = {"tei_path": child_path}
                if page_label:
                    locator["tei_page_label"] = page_label
                if facs:
                    locator["tei_facs_ref"] = facs
                if parent_division_id:
                    locator["parent_resource_id"] = parent_division_id
                resources.append(
                    {
                        "resource_id": f"tei-pb-{counters['pb']:04d}",
                        "resource_kind": "tei_page_break",
                        "locator": locator,
                        "structural_role": "page_break",
                    }
                )
                walk(
                    child,
                    path=child_path,
                    division_depth=division_depth,
                    parent_division_id=parent_division_id,
                )
                continue

            if name == "div":
                counters["div"] += 1
                resource_id = f"tei-div-{counters['div']:04d}"
                current_depth = division_depth + 1
                max_depth = max(max_depth, current_depth)
                locator = {
                    "tei_path": child_path,
                    "tei_depth": current_depth,
                }
                if page_state[0]:
                    locator["tei_page_label"] = page_state[0]
                if child.get("n"):
                    locator["tei_n"] = child.get("n")
                if child.get("type"):
                    locator["tei_type"] = child.get("type")
                if parent_division_id:
                    locator["parent_resource_id"] = parent_division_id
                resource: dict[str, Any] = {
                    "resource_id": resource_id,
                    "resource_kind": "tei_division",
                    "locator": locator,
                    "structural_role": (
                        "contents" if child.get("type") == "contents" else "division"
                    ),
                    "content_fingerprint": _fingerprint("".join(child.itertext())),
                }
                head = child.find(f"./{{{TEI_NS}}}head")
                if head is not None:
                    resource["label_fingerprint"] = _fingerprint(
                        "".join(head.itertext())
                    )
                resources.append(resource)
                walk(
                    child,
                    path=child_path,
                    division_depth=current_depth,
                    parent_division_id=resource_id,
                )
                continue

            walk(
                child,
                path=child_path,
                division_depth=division_depth,
                parent_division_id=parent_division_id,
            )

    walk(text, path="TEI/text[1]", division_depth=0, parent_division_id=None)
    if not resources:
        raise InventoryBuildError(
            "TEI payload yielded no page-break or division resources"
        )
    return {
        "file_id": file_id,
        "file_sha256": file_sha256,
        "media_type": media_type,
        "profile": "tei_structure_v1",
        "summary": {
            "resource_count": len(resources),
            "page_break_count": counters["pb"],
            "division_count": counters["div"],
            "max_division_depth": max_depth,
        },
        "resources": resources,
    }


def _djvu_xml_inventory(
    payload_path: Path,
    *,
    file_id: str,
    file_sha256: str,
    media_type: str,
) -> dict[str, Any]:
    try:
        root = ET.parse(payload_path).getroot()
    except ET.ParseError as exc:
        raise InventoryBuildError(f"DjVu XML is not well formed: {exc}") from exc
    if _local_name(root.tag) != "DjVuXML":
        raise InventoryBuildError("DjVu XML payload has no DjVuXML root")

    resources: list[dict[str, Any]] = []
    for page_index, page in enumerate(root.findall(".//OBJECT"), start=1):
        try:
            width = int(page.attrib["width"])
            height = int(page.attrib["height"])
        except (KeyError, ValueError) as exc:
            raise InventoryBuildError(
                f"DjVu XML page {page_index} has invalid geometry"
            ) from exc
        dpi = None
        for parameter in page.findall("./PARAM"):
            if parameter.get("name") == "DPI" and parameter.get("value"):
                try:
                    dpi = int(parameter.get("value", ""))
                except ValueError as exc:
                    raise InventoryBuildError(
                        f"DjVu XML page {page_index} has invalid DPI"
                    ) from exc
                break
        if dpi is None:
            raise InventoryBuildError(f"DjVu XML page {page_index} has no DPI")

        paragraphs = page.findall(".//PARAGRAPH")
        lines = page.findall(".//LINE")
        words = page.findall(".//WORD")
        page_text = " ".join(word.text or "" for word in words)
        resources.append(
            {
                "resource_id": f"djvu-ocr-page-{page_index:04d}",
                "resource_kind": "ocr_page",
                "locator": {
                    "page_index": page_index,
                    "width_pixels": width,
                    "height_pixels": height,
                    "resolution_dpi": dpi,
                },
                "structural_role": "page",
                "paragraph_count": len(paragraphs),
                "line_count": len(lines),
                "word_count": len(words),
                "content_fingerprint": _fingerprint(page_text),
            }
        )
    if not resources:
        raise InventoryBuildError("DjVu XML payload yielded no page resources")
    geometries = {
        (
            resource["locator"]["width_pixels"],
            resource["locator"]["height_pixels"],
            resource["locator"]["resolution_dpi"],
        )
        for resource in resources
    }
    return {
        "file_id": file_id,
        "file_sha256": file_sha256,
        "media_type": media_type,
        "profile": "djvu_xml_pages_v1",
        "summary": {
            "resource_count": len(resources),
            "page_count": len(resources),
            "paragraph_count": sum(
                resource["paragraph_count"] for resource in resources
            ),
            "line_count": sum(resource["line_count"] for resource in resources),
            "word_count": sum(resource["word_count"] for resource in resources),
            "distinct_page_geometry_count": len(geometries),
        },
        "resources": resources,
    }


def _djvu_page_info(data: bytes, *, form_offset: int) -> tuple[int, int, int]:
    if data[form_offset : form_offset + 4] != b"FORM":
        raise InventoryBuildError(
            f"DjVu directory target at byte {form_offset} is not a FORM chunk"
        )
    form_size = int.from_bytes(data[form_offset + 4 : form_offset + 8], "big")
    form_end = form_offset + 8 + form_size
    if form_end > len(data):
        raise InventoryBuildError(
            f"DjVu page FORM at byte {form_offset} exceeds the payload"
        )
    if data[form_offset + 8 : form_offset + 12] != b"DJVU":
        raise InventoryBuildError(
            f"DjVu directory target at byte {form_offset} is not a DJVU page"
        )

    cursor = form_offset + 12
    while cursor + 8 <= form_end:
        chunk_type = data[cursor : cursor + 4]
        chunk_size = int.from_bytes(data[cursor + 4 : cursor + 8], "big")
        chunk_start = cursor + 8
        chunk_end = chunk_start + chunk_size
        if chunk_end > form_end:
            raise InventoryBuildError(
                f"DjVu {chunk_type!r} chunk at byte {cursor} exceeds its page FORM"
            )
        if chunk_type == b"INFO":
            if chunk_size < 10:
                raise InventoryBuildError(
                    f"DjVu INFO chunk at byte {cursor} is shorter than 10 bytes"
                )
            info = data[chunk_start : chunk_start + 10]
            width = int.from_bytes(info[0:2], "big")
            height = int.from_bytes(info[2:4], "big")
            dpi = int.from_bytes(info[6:8], "little")
            if width < 1 or height < 1 or dpi < 1:
                raise InventoryBuildError(
                    f"DjVu INFO chunk at byte {cursor} has invalid page geometry"
                )
            return width, height, dpi
        cursor = chunk_end + (chunk_size % 2)

    raise InventoryBuildError(
        f"DjVu page FORM at byte {form_offset} contains no INFO chunk"
    )


def _djvu_page_form_offsets(data: bytes) -> list[int]:
    if not data.startswith(b"AT&TFORM") or len(data) < 16:
        raise InventoryBuildError("DjVu payload has no AT&T FORM header")
    root_size = int.from_bytes(data[8:12], "big")
    root_end = 12 + root_size
    if root_end != len(data):
        raise InventoryBuildError(
            "DjVu root FORM size does not match the payload byte size"
        )

    root_type = data[12:16]
    if root_type == b"DJVU":
        return [4]
    if root_type != b"DJVM":
        raise InventoryBuildError(f"DjVu root FORM has unsupported type {root_type!r}")

    cursor = 16
    directory: bytes | None = None
    while cursor + 8 <= root_end:
        chunk_type = data[cursor : cursor + 4]
        chunk_size = int.from_bytes(data[cursor + 4 : cursor + 8], "big")
        chunk_start = cursor + 8
        chunk_end = chunk_start + chunk_size
        if chunk_end > root_end:
            raise InventoryBuildError(
                f"DjVu root chunk {chunk_type!r} at byte {cursor} exceeds the root FORM"
            )
        if chunk_type == b"DIRM":
            directory = data[chunk_start:chunk_end]
            break
        cursor = chunk_end + (chunk_size % 2)

    if directory is None:
        raise InventoryBuildError("bundled DjVu payload has no DIRM chunk")
    if len(directory) < 3:
        raise InventoryBuildError("DjVu DIRM chunk is shorter than three bytes")

    entry_count = int.from_bytes(directory[1:3], "big")
    offsets_end = 3 + 4 * entry_count
    if entry_count < 1 or len(directory) < offsets_end:
        raise InventoryBuildError("DjVu DIRM chunk has an invalid entry table")
    component_offsets = [
        int.from_bytes(directory[index : index + 4], "big")
        for index in range(3, offsets_end, 4)
    ]
    if component_offsets != sorted(set(component_offsets)):
        raise InventoryBuildError(
            "DjVu DIRM component offsets are not strictly increasing and unique"
        )
    if any(offset < 16 or offset + 12 > root_end for offset in component_offsets):
        raise InventoryBuildError(
            "DjVu DIRM component offset falls outside the root FORM"
        )

    page_offsets: list[int] = []
    for offset in component_offsets:
        if data[offset : offset + 4] != b"FORM":
            raise InventoryBuildError(
                f"DjVu DIRM target at byte {offset} is not a FORM chunk"
            )
        component_size = int.from_bytes(data[offset + 4 : offset + 8], "big")
        component_end = offset + 8 + component_size
        if component_end > root_end:
            raise InventoryBuildError(
                f"DjVu DIRM component at byte {offset} exceeds the root FORM"
            )
        component_type = data[offset + 8 : offset + 12]
        if component_type == b"DJVU":
            page_offsets.append(offset)
        elif component_type not in {b"DJVI", b"THUM"}:
            raise InventoryBuildError(
                f"DjVu DIRM component at byte {offset} has unsupported FORM type "
                f"{component_type!r}"
            )
    if not page_offsets:
        raise InventoryBuildError("bundled DjVu payload has no DJVU page components")
    return page_offsets


def _djvu_inventory(
    payload_path: Path,
    *,
    file_id: str,
    file_sha256: str,
    media_type: str,
) -> dict[str, Any]:
    try:
        data = payload_path.read_bytes()
    except OSError as exc:
        raise InventoryBuildError(f"DjVu payload is unreadable: {exc}") from exc

    resources: list[dict[str, Any]] = []
    for page_index, form_offset in enumerate(
        _djvu_page_form_offsets(data),
        start=1,
    ):
        width, height, dpi = _djvu_page_info(data, form_offset=form_offset)
        resources.append(
            {
                "resource_id": f"djvu-page-{page_index:04d}",
                "resource_kind": "djvu_page",
                "locator": {
                    "page_index": page_index,
                    "width_pixels": width,
                    "height_pixels": height,
                    "resolution_dpi": dpi,
                },
                "structural_role": "page",
            }
        )

    geometries = {
        (
            resource["locator"]["width_pixels"],
            resource["locator"]["height_pixels"],
            resource["locator"]["resolution_dpi"],
        )
        for resource in resources
    }
    return {
        "file_id": file_id,
        "file_sha256": file_sha256,
        "media_type": media_type,
        "profile": "djvu_pages_v1",
        "summary": {
            "resource_count": len(resources),
            "page_count": len(resources),
            "distinct_page_geometry_count": len(geometries),
        },
        "resources": resources,
    }


def _abbyy_xml_inventory(
    payload_path: Path,
    *,
    file_id: str,
    file_sha256: str,
    media_type: str,
) -> dict[str, Any]:
    namespace = "{http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml}"
    resources: list[dict[str, Any]] = []
    try:
        with gzip.open(payload_path, "rb") as source:
            for _event, page in ET.iterparse(source, events=("end",)):
                if page.tag != f"{namespace}page":
                    continue
                page_index = len(resources) + 1
                try:
                    width = int(page.attrib["width"])
                    height = int(page.attrib["height"])
                    dpi = int(page.attrib["resolution"])
                except (KeyError, ValueError) as exc:
                    raise InventoryBuildError(
                        f"ABBYY XML page {page_index} has invalid geometry"
                    ) from exc
                paragraphs = list(page.iter(f"{namespace}par"))
                lines = list(page.iter(f"{namespace}line"))
                characters = list(page.iter(f"{namespace}charParams"))
                page_text = "".join(character.text or "" for character in characters)
                word_count = sum(
                    character.get("wordStart") == "true" for character in characters
                )
                resources.append(
                    {
                        "resource_id": f"abbyy-ocr-page-{page_index:04d}",
                        "resource_kind": "ocr_page",
                        "locator": {
                            "page_index": page_index,
                            "width_pixels": width,
                            "height_pixels": height,
                            "resolution_dpi": dpi,
                        },
                        "structural_role": "page",
                        "paragraph_count": len(paragraphs),
                        "line_count": len(lines),
                        "word_count": word_count,
                        "content_fingerprint": _fingerprint(page_text),
                    }
                )
                page.clear()
    except (OSError, EOFError, ET.ParseError) as exc:
        raise InventoryBuildError(f"ABBYY XML gzip is unreadable: {exc}") from exc
    if not resources:
        raise InventoryBuildError("ABBYY XML payload yielded no page resources")
    geometries = {
        (
            resource["locator"]["width_pixels"],
            resource["locator"]["height_pixels"],
            resource["locator"]["resolution_dpi"],
        )
        for resource in resources
    }
    return {
        "file_id": file_id,
        "file_sha256": file_sha256,
        "media_type": media_type,
        "profile": "abbyy_xml_pages_v1",
        "summary": {
            "resource_count": len(resources),
            "page_count": len(resources),
            "paragraph_count": sum(
                resource["paragraph_count"] for resource in resources
            ),
            "line_count": sum(resource["line_count"] for resource in resources),
            "word_count": sum(resource["word_count"] for resource in resources),
            "distinct_page_geometry_count": len(geometries),
        },
        "resources": resources,
    }


def build_file_inventory(
    payload_path: Path, payload_entry: dict[str, Any]
) -> dict[str, Any]:
    media_type = payload_entry["media_type"]
    kwargs = {
        "file_id": payload_entry["file_id"],
        "file_sha256": payload_entry["sha256"],
        "media_type": media_type,
    }
    if media_type == "application/pdf":
        return _pdf_inventory(payload_path, **kwargs)
    if media_type == "application/epub+zip":
        return _epub_inventory(payload_path, **kwargs)
    if media_type == "application/zip" and payload_entry["relative_path"].endswith(
        "_jp2.zip"
    ):
        return _jp2_zip_inventory(payload_path, **kwargs)
    if media_type == "image/vnd.djvu":
        return _djvu_inventory(payload_path, **kwargs)
    if media_type == "application/vnd.djvu+xml":
        return _djvu_xml_inventory(payload_path, **kwargs)
    if media_type == "application/gzip" and payload_entry["relative_path"].endswith(
        ".abbyy.xml.gz"
    ):
        return _abbyy_xml_inventory(payload_path, **kwargs)
    if media_type in {"application/xml", "text/xml"} and payload_entry[
        "relative_path"
    ].endswith("_scandata.xml"):
        return _scandata_inventory(payload_path, **kwargs)
    if media_type in {"application/tei+xml", "application/xml", "text/xml"}:
        return _tei_inventory(payload_path, **kwargs)
    raise InventoryBuildError(
        f"no resource-inventory profile for media type {media_type}: {payload_path}"
    )


def _default_event_ref(item_id: str, event_date: str) -> str:
    suffix = item_id.removeprefix("tos.item.")
    return f"tos.event.resource-inventory.{suffix}.{event_date}"


def _preserve_prior_pdf_number_shapes(
    file_inventories: list[dict[str, Any]], prior: dict[str, Any]
) -> None:
    """Keep legacy integral JSON numbers stable when their values did not move."""
    prior_files = {
        entry.get("file_id"): entry
        for entry in prior.get("files", [])
        if isinstance(entry, dict) and isinstance(entry.get("file_id"), str)
    }
    for file_inventory in file_inventories:
        if file_inventory.get("profile") != "pdf_pages_v1":
            continue
        prior_file = prior_files.get(file_inventory.get("file_id"))
        if not isinstance(prior_file, dict):
            continue
        prior_resources = {
            entry.get("resource_id"): entry
            for entry in prior_file.get("resources", [])
            if isinstance(entry, dict) and isinstance(entry.get("resource_id"), str)
        }
        for resource in file_inventory.get("resources", []):
            if not isinstance(resource, dict):
                continue
            prior_resource = prior_resources.get(resource.get("resource_id"))
            if not isinstance(prior_resource, dict):
                continue
            locator = resource.get("locator")
            prior_locator = prior_resource.get("locator")
            if not isinstance(locator, dict) or not isinstance(prior_locator, dict):
                continue
            for field in ("width_points", "height_points"):
                current_value = locator.get(field)
                prior_value = prior_locator.get(field)
                if (
                    isinstance(prior_value, int)
                    and not isinstance(prior_value, bool)
                    and isinstance(current_value, float)
                    and current_value.is_integer()
                    and current_value == prior_value
                ):
                    locator[field] = prior_value


def build_inventory(
    *,
    repo_root: Path,
    manifest_path: Path,
    payload_source_root: Path,
    event_date: str,
) -> dict[str, Any] | None:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    relative_item_dir = manifest_path.parent.relative_to(repo_root / SOURCE_ROOT)
    payload_source_dir = payload_source_root / relative_item_dir
    file_inventories: list[dict[str, Any]] = []
    for payload_entry in manifest["payload_files"]:
        payload_path = payload_source_dir / payload_entry["relative_path"]
        if not payload_path.is_file():
            return None
        _verify_payload_fixity(payload_path, payload_entry)
        file_inventories.append(build_file_inventory(payload_path, payload_entry))

    output_path = manifest_path.parent / INVENTORY_NAME
    event_ref = _default_event_ref(manifest["item_id"], event_date)
    inventory_version = 1
    supersedes_inventory_ref: str | None = None
    prior: dict[str, Any] = {}
    if output_path.is_file():
        try:
            prior = json.loads(output_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            prior = {}
        prior_event_ref = prior.get("provenance_event_ref")
        if isinstance(prior_event_ref, str):
            event_ref = prior_event_ref
        prior_version = prior.get("inventory_version")
        if isinstance(prior_version, int) and prior_version >= 1:
            inventory_version = prior_version
        prior_supersedes = prior.get("supersedes_inventory_ref")
        if isinstance(prior_supersedes, str):
            supersedes_inventory_ref = prior_supersedes
    _preserve_prior_pdf_number_shapes(file_inventories, prior)
    return {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_source_resource_inventory_v1",
        "item_id": manifest["item_id"],
        "generated_from_manifest_ref": manifest_path.relative_to(repo_root).as_posix(),
        "inventory_authority": "mechanical_metadata_only",
        "source_text_included": False,
        "files": file_inventories,
        "generator": {
            "name": "build_source_resource_inventories.py",
            "version": "1",
        },
        "provenance_event_ref": event_ref,
        "inventory_version": inventory_version,
        "supersedes_inventory_ref": supersedes_inventory_ref,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def render_inventory(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=False) + "\n"


def iter_manifests(repo_root: Path) -> Iterable[Path]:
    return sorted((repo_root / SOURCE_ROOT).rglob("item.manifest.json"))


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Build text-free resource inventories from local source payloads."
    )
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        help=(
            "Source-witness root containing local item payloads; defaults to the "
            "current checkout's ToS/source-witnesses."
        ),
    )
    parser.add_argument(
        "--event-date",
        default="2026-07-28",
        help="Date suffix for new provenance event refs (YYYY-MM-DD).",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    payload_source_root = (
        args.payload_source_root.resolve()
        if args.payload_source_root
        else (repo_root / SOURCE_ROOT).resolve()
    )
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", args.event_date):
        parser.error("--event-date must use YYYY-MM-DD")

    processed = 0
    skipped = 0
    drift: list[str] = []
    for manifest_path in iter_manifests(repo_root):
        payload = build_inventory(
            repo_root=repo_root,
            manifest_path=manifest_path,
            payload_source_root=payload_source_root,
            event_date=args.event_date,
        )
        output_path = manifest_path.parent / INVENTORY_NAME
        if payload is None:
            skipped += 1
            continue
        processed += 1
        rendered = render_inventory(payload)
        if args.check:
            current = (
                output_path.read_text(encoding="utf-8") if output_path.is_file() else ""
            )
            if current != rendered:
                drift.append(output_path.relative_to(repo_root).as_posix())
        else:
            output_path.write_text(rendered, encoding="utf-8")

    if processed == 0:
        print(
            "[skip] no local source payloads were available for resource inventory",
            file=sys.stderr,
        )
        return 0
    if drift:
        for path in drift:
            print(f"[drift] {path}", file=sys.stderr)
        return 1
    verb = "verified" if args.check else "wrote"
    print(
        f"[ok] {verb} {processed} source resource inventories"
        + (f"; skipped {skipped} absent local payload set(s)" if skipped else "")
    )
    print(
        "[boundary] inventories contain mechanical metadata and one-way "
        "fingerprints, not source text or content acceptance"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
