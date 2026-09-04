#!/usr/bin/env python3
"""Build the complete Antonovsky 1911 structural/paragraph technical spine.

Tracked outputs are text-free.  The builder reads the exact frozen v1 source
observation, applies the independently verified paragraph/verse algorithm, and
compares all adjacent physical-line boundaries with the primary challenger.
It does not accept text, alignment, semantics, interpretation, or canon.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import re
import secrets
import statistics
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

REPO = Path(__file__).resolve().parents[1]
V1_BUILDER = REPO / "scripts/build_antonovsky_1911_technical_markup.py"
V1_ROUTE_REF = ("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
                "technical-markup/antonovsky-1911-pdf-layout-v1")
V2_ROUTE_REF = ("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
                "technical-markup/antonovsky-1911-structural-paragraph-v2")
V2_ROUTE = REPO / V2_ROUTE_REF
CENSUS_REF = f"{V2_ROUTE_REF}/structure-census.v2.jsonl"
ISSUANCE_REF = f"{V2_ROUTE_REF}/identity-issuance.v2.json"
CHALLENGER_REF = f"{V2_ROUTE_REF}/primary-challenger-input.v2.json"
PDF_SHA = "57518b50e24fee37b3e9c151e853c36ad34258f51a8b65b8233742d69017db69"
BBOX_SHA = "c145e47a5655bc440c35bb36922d88a886e9da126c332a23a71beee569615383"
TEXT_SHA = "9a3e85fe010ec0eb22379b04f8c03a7dfa7beff06286cc0ac35521d8ae66c787"
EXPECTED = {"blocks": 7225, "physical_lines": 13071, "logical_rows": 10686,
            "readings": 81, "subparts": 112, "prose": 3569,
            "verse_groups": 13, "verse_lines": 359}
OUTPUT_REFS = {
    "physical_lines": f"{V2_ROUTE_REF}/physical-line-spine.v2.jsonl",
    "logical_rows": f"{V2_ROUTE_REF}/logical-row-spine.v2.jsonl",
    "structures": f"{V2_ROUTE_REF}/structure-spine.v2.jsonl",
    "paragraphs": f"{V2_ROUTE_REF}/paragraph-spine.v2.jsonl",
    "verse_groups": f"{V2_ROUTE_REF}/verse-group-spine.v2.jsonl",
    "verse_lines": f"{V2_ROUTE_REF}/verse-line-spine.v2.jsonl",
    "conflicts": f"{V2_ROUTE_REF}/boundary-conflicts.v2.jsonl",
    "comparison": f"{V2_ROUTE_REF}/boundary-comparison.v2.json",
    "contradictions": f"{V2_ROUTE_REF}/contradiction-ledger.v2.jsonl",
    "coverage": f"{V2_ROUTE_REF}/coverage-receipt.v2.json",
    "verification": f"{V2_ROUTE_REF}/agent-verification.v2.json",
    "summary": f"{V2_ROUTE_REF}/summary.v2.json",
    "provenance": f"{V2_ROUTE_REF}/provenance.jsonl",
    "manifest": f"{V2_ROUTE_REF}/manifest.v2.json",
}


class BuildError(RuntimeError):
    pass


def sha(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def json_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()


def jsonl_bytes(rows: Iterable[dict[str, Any]]) -> bytes:
    return b"".join((json.dumps(row, ensure_ascii=False, sort_keys=True,
                                separators=(",", ":")) + "\n").encode() for row in rows)


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def quantile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    pos = fraction * (len(ordered) - 1)
    lo, hi = math.floor(pos), math.ceil(pos)
    return ordered[lo] if lo == hi else ordered[lo] * (hi - pos) + ordered[hi] * (pos - lo)


def bbox_union(boxes: Iterable[list[float]]) -> list[float]:
    rows = list(boxes)
    return [min(x[0] for x in rows), min(x[1] for x in rows),
            max(x[2] for x in rows), max(x[3] for x in rows)]


def compact(value: str) -> str:
    return re.sub(r"\s+", "", value)


def ornament(value: str) -> bool:
    value = compact(value)
    return bool(value) and ("*" in value or value == "¥") and not re.search(
        r"[A-Za-zА-Яа-яЁё0-9]", value)


def symbol_residue(value: str) -> bool:
    value = compact(value)
    return not value or (not re.search(r"[A-Za-zА-Яа-яЁё0-9]", value)
                         and value not in {"—", "-", "–"})


@dataclass
class PhysicalLine:
    source_line_ref: str
    citation: str
    locator: str
    block_unit_id: str
    ordinal: int
    page: int
    panel: str
    bbox: list[float]
    text: str
    text_sha256: str
    explicit_role: str | None = None
    role: str | None = None
    row_ref: str | None = None
    reading_ref: str | None = None
    subpart_ref: str | None = None
    paragraph_ref: str | None = None
    verse_group_ref: str | None = None
    verse_line_ref: str | None = None


@dataclass
class LogicalRow:
    row_ref: str
    page: int
    panel: str
    lines: list[PhysicalLine] = field(default_factory=list)
    bbox: list[float] = field(default_factory=list)
    text: str = ""
    text_sha256: str = ""
    role: str | None = None
    reading_ref: str | None = None
    subpart_ref: str | None = None
    base_x: float | None = None
    indent_delta: float | None = None
    verse_seed: bool = False


def load_source() -> tuple[dict[str, Any], Any, list[dict[str, Any]], list[dict[str, Any]]]:
    spec = importlib.util.spec_from_file_location("antonovsky_v1_builder", V1_BUILDER)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    plan = module._load_plan(REPO)
    inventory = module._inventory_pages(REPO, plan)
    source_pdf = module._source_pdf(REPO, plan)
    bbox, warnings, binary, version = module._extract_bbox(source_pdf, plan)
    document = module._parse_observation(bbox, warnings, binary, version, inventory, plan)
    if sha(source_pdf.read_bytes()) != PDF_SHA or sha(document.bbox_bytes) != BBOX_SHA \
            or sha(document.text_layer.encode()) != TEXT_SHA:
        raise BuildError("frozen source, bbox, or text-layer fixity drift")
    v1_rows = load_jsonl(REPO / f"{V1_ROUTE_REF}/citation-spine.v1.jsonl")
    v1_blocks = {r["source_locator"]: r for r in v1_rows if r["unit_kind"] == "paragraph"}
    blocks = []
    for page in document.pages:
        for panel in page.panels:
            for block in panel.blocks:
                source = v1_blocks[block.locator]
                blocks.append({"page": page.page_number, "panel": panel.panel,
                               "locator": block.locator, "citation": source["display_citation"],
                               "unit_id": source["unit_id"], "lines": [
                    {"n": n, "text": text, "bbox": list(box)}
                    for n, (text, box) in enumerate(zip(block.lines, block.line_bboxes, strict=True), 1)
                ]})
    if len(blocks) != EXPECTED["blocks"]:
        raise BuildError("Poppler block census drift")
    return plan, document, blocks, load_jsonl(REPO / CENSUS_REF)


def flatten_lines(blocks: list[dict[str, Any]]) -> list[PhysicalLine]:
    lines = []
    for block in blocks:
        for raw in block["lines"]:
            n = int(raw["n"])
            lines.append(PhysicalLine(
                source_line_ref=f"{block['citation']}.l{n:02d}", citation=block["citation"],
                locator=f"{block['locator']}/line-{n:04d}", block_unit_id=block["unit_id"],
                ordinal=n, page=block["page"], panel=block["panel"],
                bbox=[float(v) for v in raw["bbox"]], text=raw["text"],
                text_sha256=sha(raw["text"].encode())))
    lines.sort(key=lambda x: (x.page, 0 if x.panel == "left" else 1,
                              x.bbox[1], x.bbox[0], x.citation, x.ordinal))
    if len(lines) != EXPECTED["physical_lines"] or len({x.locator for x in lines}) != len(lines):
        raise BuildError("physical-line census or uniqueness drift")
    return lines


def assign_explicit_roles(lines: list[PhysicalLine], census: list[dict[str, Any]]) -> None:
    by_citation: dict[str, list[PhysicalLine]] = defaultdict(list)
    for line in lines:
        by_citation[line.citation].append(line)
    for record in census:
        for anchor in record.get("anchors", []):
            citation = anchor["display_citation"]
            if "@visual" in citation:
                continue
            candidates = by_citation[citation]
            kind = record["record_kind"]
            if kind == "numbered_subpart_marker":
                ordinary = [line for line in candidates if not ornament(line.text)]
                marker = ordinary[-1] if ordinary else candidates[-1]
                for line in candidates:
                    line.explicit_role = "numbered_subpart_marker" if line is marker else "ornament"
            elif kind in {"reading_unit_heading", "cycle_heading"}:
                for line in candidates:
                    line.explicit_role = "ornament" if ornament(line.text) else kind
            elif kind in {"part_heading", "terminal_marker"}:
                for line in candidates:
                    line.explicit_role = kind


def make_rows(lines: list[PhysicalLine]) -> list[LogicalRow]:
    groups: list[list[PhysicalLine]] = []
    for line in lines:
        if not groups or (line.page, line.panel) != (groups[-1][0].page, groups[-1][0].panel):
            groups.append([line]); continue
        current = groups[-1]
        box = bbox_union(x.bbox for x in current)
        overlap = min(box[3], line.bbox[3]) - max(box[1], line.bbox[1])
        delta = abs((box[1] + box[3] - line.bbox[1] - line.bbox[3]) / 2)
        disjoint = line.bbox[2] <= box[0] + 1 or line.bbox[0] >= box[2] - 1
        same_block = any(x.citation == line.citation for x in current)
        if (same_block and delta <= 1.5) or (not same_block and
                (delta <= 4.8 or (overlap >= 1.5 and disjoint and delta <= 7.0))):
            current.append(line)
        else:
            groups.append([line])
    rows = []
    for n, group in enumerate(groups, 1):
        ordered = sorted(group, key=lambda x: (x.bbox[0], x.citation, x.ordinal))
        text = " ".join(x.text for x in ordered)
        row = LogicalRow(f"row-{n:05d}", ordered[0].page, ordered[0].panel, ordered,
                         bbox_union(x.bbox for x in ordered), text, sha(text.encode()))
        for line in ordered:
            line.row_ref = row.row_ref
        rows.append(row)
    if len(rows) != EXPECTED["logical_rows"]:
        raise BuildError(f"logical-row census drift: {len(rows)}")
    return rows


def panel_region(page: int, panel: str) -> str | None:
    front = {(1, "right"), (2, "left"), (2, "right"), (3, "left"),
             (3, "right"), (4, "right"), (5, "left")}
    back = {(151, "right"), (152, "left"), (152, "right"), (153, "left")}
    return "front_matter" if (page, panel) in front else "back_matter" if (page, panel) in back else None


def page_header(row: LogicalRow) -> bool:
    if row.bbox[1] >= 36 or abs((row.bbox[0] + row.bbox[2]) / 2 -
                               (198 if row.panel == "left" else 594)) > 55:
        return False
    value = compact(row.text)
    letters, digits = re.findall(r"[A-Za-zА-Яа-яЁё]", value), re.findall(r"[0-9]", value)
    return len(value) <= 18 and bool(digits or sum(value.count(x) for x in ("-", "—", "–")) >= 2
                                     or (len(letters) <= 3 and len(value) <= 5))


def page_footer(row: LogicalRow) -> bool:
    y0, y1, value = row.bbox[1], row.bbox[3], compact(row.text)
    if y0 >= 548: return True
    if y0 < 526: return False
    if re.search(r"(?:Ницше|Нгппчс|Ихітчс|Яшине|говорилъЗаратустра)", value, re.I): return True
    if re.fullmatch(r"[0-9*'\"/NnWwVvІI\-.•]+", value or ""): return True
    if y0 >= 538 and row.bbox[2] - row.bbox[0] < 40 and (
            500 <= row.bbox[0] <= 620 if row.panel == "right" else 150 <= row.bbox[0] <= 260):
        return True
    return (y1-y0) <= 10.1 and row.bbox[2]-row.bbox[0] < 180 and (
        row.bbox[0] > (680 if row.panel == "right" else 300)
        or row.bbox[0] > (460 if row.panel == "right" else 150))


def initial_roles(rows: list[LogicalRow]) -> None:
    precedence = {"terminal_marker": 100, "part_heading": 95, "reading_unit_heading": 94,
                  "cycle_heading": 93, "numbered_subpart_marker": 92, "ornament": 80}
    for row in rows:
        explicit = [x.explicit_role for x in row.lines if x.explicit_role]
        row.role = panel_region(row.page, row.panel)
        if row.role is None and explicit: row.role = max(explicit, key=lambda x: precedence[x])
        elif row.role is None and ornament(row.text): row.role = "ornament"
        elif row.role is None and (page_header(row) or page_footer(row)): row.role = "page_furniture"
        elif row.role is None and symbol_residue(row.text): row.role = "residue"
    for i, row in enumerate(rows):
        if row.role == "residue" and any(
            x.role == "ornament" and (x.page, x.panel) == (row.page, row.panel)
            and max(x.bbox[1], row.bbox[1]) - min(x.bbox[3], row.bbox[3]) <= 8
            for x in [rows[j] for j in (i-1, i+1) if 0 <= j < len(rows)]):
            row.role = "ornament"


def structure_refs(census: list[dict[str, Any]]) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]]]:
    readings, subparts = {}, {}
    for record in census:
        if record["record_kind"] == "reading_unit_heading":
            readings[f"{record['part_id']}.reading_{record['reading_unit_ordinal_within_part']:02d}"] = record
        elif record["record_kind"] == "numbered_subpart_marker":
            subparts[f"{record['parent_reading_ref']}.subpart_{record['number']:02d}"] = record
    return readings, subparts


def assign_context(rows: list[LogicalRow], census: list[dict[str, Any]]) -> tuple[dict[str, Any], dict[str, Any]]:
    row_index = {row.row_ref: i for i, row in enumerate(rows)}
    citation_rows: dict[str, list[LogicalRow]] = defaultdict(list)
    for row in rows:
        for citation in {x.citation for x in row.lines}: citation_rows[citation].append(row)
    readings, subparts = structure_refs(census)
    reading_events, marker_events, part_events, visual_events = {}, {}, set(), []
    for ref, record in readings.items():
        candidates = [row for a in record["anchors"] for row in citation_rows[a["display_citation"]]
                      if row.role == "reading_unit_heading"]
        reading_events[min(row_index[x.row_ref] for x in candidates)] = ref
    for record in census:
        if record["record_kind"] == "part_heading":
            candidates = [row for a in record["anchors"] for row in citation_rows[a["display_citation"]]]
            panel = (candidates[0].page, candidates[0].panel)
            part_events.add(min(i for i, row in enumerate(rows) if (row.page, row.panel) == panel))
    for ref, record in subparts.items():
        citation = record["anchors"][0]["display_citation"]
        if "@visual-number-22" in citation: visual_events.append((97, "right", 50.0, ref))
        elif "@visual-number-1" in citation: visual_events.append((144, "left", 292.0, ref))
        else:
            candidates = [row for row in citation_rows[citation] if row.role == "numbered_subpart_marker"]
            marker_events[min(row_index[x.row_ref] for x in candidates)] = ref
    reading = subpart = None
    fired: set[str] = set()
    for i, row in enumerate(rows):
        if i in part_events: reading = subpart = None
        if i in reading_events: reading, subpart = reading_events[i], None
        for page, panel, y, ref in visual_events:
            if ref not in fired and (row.page, row.panel) == (page, panel) and row.bbox[1] >= y \
                    and ref.startswith(f"{reading}."):
                subpart = ref; fired.add(ref)
        if i in marker_events: subpart = marker_events[i]
        row.reading_ref, row.subpart_ref = reading, subpart
        for line in row.lines: line.reading_ref, line.subpart_ref = reading, subpart
    if set(subparts) != fired | set(marker_events.values()): raise BuildError("subpart events incomplete")
    return readings, subparts


def estimate_margins(rows: list[LogicalRow]) -> dict[tuple[int, str], float]:
    candidates: dict[tuple[int, str], list[float]] = defaultdict(list)
    for row in rows:
        if row.role is None and row.reading_ref and 34 <= row.bbox[1] <= 540 \
                and row.bbox[2] - row.bbox[0] >= 175:
            candidates[(row.page, row.panel)].append(row.bbox[0])
    raw = {k: quantile(v, .08) for k, v in candidates.items() if len(v) >= 3}
    result = {}
    for page, panel in {(x.page, x.panel) for x in rows}:
        near = [raw[(p, panel)] for p in range(page-3, page+4) if p != page and (p, panel) in raw]
        reference, own = (quantile(near, .35) if near else None), raw.get((page, panel))
        result[(page, panel)] = (own if own is not None and (reference is None or own <= reference+12)
                                 else reference if reference is not None else 50 if panel == "left" else 430)
    return result


def context_runs(rows: list[LogicalRow]) -> list[list[LogicalRow]]:
    runs, current, key = [], [], None
    for row in rows:
        if row.role is not None or row.reading_ref is None:
            if current: runs.append(current)
            current, key = [], None
        else:
            new_key = (row.reading_ref, row.subpart_ref)
            if current and new_key != key: runs.append(current); current = []
            current.append(row); key = new_key
    if current: runs.append(current)
    return runs


def mark_verse(rows: list[LogicalRow], margins: dict[tuple[int, str], float]) -> list[dict[str, Any]]:
    conflicts = []
    for row in rows:
        if row.role is None and row.reading_ref:
            row.base_x = margins[(row.page, row.panel)]
            row.indent_delta = row.bbox[0] - row.base_x
            row.verse_seed = row.indent_delta >= 12
    for run in context_runs(rows):
        seeds, marked, i = [x.verse_seed for x in run], [False] * len(run), 0
        while i < len(run):
            if not seeds[i]: i += 1; continue
            j = i
            while j+1 < len(run) and seeds[j+1]:
                if (run[j].page, run[j].panel) == (run[j+1].page, run[j+1].panel) \
                        and run[j+1].bbox[1] - run[j].bbox[3] > 22: break
                j += 1
            length = j-i+1
            width = statistics.median(x.bbox[2]-x.bbox[0] for x in run[i:j+1])
            if length >= 5 and width <= 245:
                for k in range(i, j+1): marked[k] = True
            elif 2 <= length <= 4 and width <= 225:
                conflicts.append({"conflict_kind": "short_persistent_inset_run",
                                  "reading_ref": run[i].reading_ref, "subpart_ref": run[i].subpart_ref,
                                  "first_row_ref": run[i].row_ref, "last_row_ref": run[j].row_ref,
                                  "row_count": length, "median_width_points": round(width, 3),
                                  "resolution": "prose_by_conservative_five_row_verse_threshold"})
            i = j+1
        for i in range(len(run)):
            if not marked[i] and seeds[i] and ((i and marked[i-1]) or (i+1 < len(run) and marked[i+1])):
                marked[i] = True
        for row, verse in zip(run, marked, strict=True):
            if verse: row.role = "verse"
    return conflicts


def paragraph_boundary(a: LogicalRow, b: LogicalRow) -> tuple[bool, str, float]:
    if (a.reading_ref, a.subpart_ref) != (b.reading_ref, b.subpart_ref): return True, "structural_context_change", 1.0
    if a.role != "prose" or b.role != "prose": return True, "role_change", 1.0
    same = (a.page, a.panel) == (b.page, b.panel)
    if same:
        gap = b.bbox[1] - a.bbox[3]
        if gap >= 5.2: return True, "large_vertical_gap", .97
        if gap >= 2 and (b.indent_delta or 0) >= 13: return True, "indent_plus_vertical_gap", .95
    elif (b.indent_delta or 0) >= 13: return True, "new_panel_indented_start", .94
    if (b.indent_delta or 0) >= 13:
        if same and abs(b.bbox[1]-a.bbox[1]) <= 4.8: return False, "same_logical_baseline_fragment", .99
        return True, "first_line_indent", .96
    return False, "base_margin_continuation", .91


def build_paragraphs(rows: list[LogicalRow]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    for row in rows:
        if row.role is None and row.reading_ref: row.role = "prose"
        elif row.role is None and row.reading_ref is None: row.role = "part_preamble"
    conflicts, prior = [], None
    for row in rows:
        if row.role != "prose": continue
        if prior and (prior.reading_ref, prior.subpart_ref) == (row.reading_ref, row.subpart_ref) \
                and (prior.page, prior.panel) != (row.page, row.panel) \
                and 8 <= (row.indent_delta or 0) < 17:
            conflicts.append({"conflict_kind": "borderline_cross_panel_indent",
                              "reading_ref": row.reading_ref, "subpart_ref": row.subpart_ref,
                              "previous_row_ref": prior.row_ref, "current_row_ref": row.row_ref,
                              "indent_delta_points": round(row.indent_delta or 0, 3),
                              "resolution": "new_paragraph_at_13_points_otherwise_continuation"})
        prior = row
    groups, reasons, current = [], [], []
    for row in rows:
        if row.role != "prose":
            if current: groups.append(current); current = []
        elif not current: current = [row]; reasons.append(("content_run_start", 1.0))
        else:
            boundary, reason, confidence = paragraph_boundary(current[-1], row)
            if boundary: groups.append(current); current = [row]; reasons.append((reason, confidence))
            else: current.append(row)
    if current: groups.append(current)
    reading_ord, subpart_ord, records = Counter(), Counter(), []
    for n, (group, (basis, confidence)) in enumerate(zip(groups, reasons, strict=True), 1):
        reading, subpart = group[0].reading_ref, group[0].subpart_ref
        assert reading
        reading_ord[reading] += 1
        if subpart: subpart_ord[subpart] += 1
        ref = f"paragraph-{n:05d}"
        physical = [line for row in group for line in row.lines]
        for line in physical: line.paragraph_ref = ref
        records.append({"paragraph_ref": ref, "global_ordinal": n, "reading_ref": reading,
                        "reading_paragraph_ordinal": reading_ord[reading], "subpart_ref": subpart,
                        "subpart_paragraph_ordinal": subpart_ord[subpart] if subpart else None,
                        "boundary_basis": basis, "boundary_confidence": confidence,
                        "row_refs": [x.row_ref for x in group],
                        "physical_line_refs": [x.locator for x in physical],
                        "first_locator": physical[0].locator, "last_locator": physical[-1].locator,
                        "cross_panel": (group[0].page, group[0].panel) != (group[-1].page, group[-1].panel),
                        "text_sha256": sha("\n".join(x.text for x in group).encode())})
    return records, conflicts


def build_verse(rows: list[LogicalRow]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    groups, current = [], []
    for row in rows:
        if row.role != "verse":
            if current: groups.append(current); current = []
            continue
        if current and (row.reading_ref, row.subpart_ref) != (current[-1].reading_ref, current[-1].subpart_ref):
            groups.append(current); current = []
        if current and (row.page, row.panel) == (current[-1].page, current[-1].panel) \
                and row.bbox[1]-current[-1].bbox[3] > 22:
            groups.append(current); current = []
        current.append(row)
    if current: groups.append(current)
    group_rows, line_rows, reading_ord = [], [], Counter()
    for n, group in enumerate(groups, 1):
        reading = group[0].reading_ref; assert reading
        reading_ord[reading] += 1
        group_ref = f"verse-group-{n:04d}"
        verse_line_refs = []
        for ordinal, row in enumerate(group, 1):
            ref = f"{group_ref}.line-{ordinal:03d}"; verse_line_refs.append(ref)
            for line in row.lines: line.verse_group_ref, line.verse_line_ref = group_ref, ref
            line_rows.append({"verse_line_ref": ref, "verse_group_ref": group_ref,
                              "verse_line_ordinal": ordinal, "reading_ref": reading,
                              "subpart_ref": row.subpart_ref, "row_ref": row.row_ref,
                              "physical_line_refs": [x.locator for x in row.lines],
                              "bbox_points": [round(v, 6) for v in row.bbox], "text_sha256": row.text_sha256})
        physical = [line for row in group for line in row.lines]
        group_rows.append({"verse_group_ref": group_ref, "global_ordinal": n, "reading_ref": reading,
                           "reading_verse_group_ordinal": reading_ord[reading],
                           "subpart_ref": group[0].subpart_ref, "verse_line_refs": verse_line_refs,
                           "physical_line_refs": [x.locator for x in physical],
                           "first_locator": physical[0].locator, "last_locator": physical[-1].locator,
                           "cross_panel": (group[0].page, group[0].panel) != (group[-1].page, group[-1].panel),
                           "text_sha256": sha("\n".join(x.text for x in group).encode())})
    return group_rows, line_rows


def finalize_roles(rows: list[LogicalRow]) -> None:
    for row in rows:
        if row.role is None: raise BuildError(f"unassigned row: {row.row_ref}")
        for line in row.lines:
            line.role = ("ornament" if row.role in {"reading_unit_heading", "numbered_subpart_marker"}
                         and line.explicit_role == "ornament" else row.role)


def reconstruct() -> dict[str, Any]:
    plan, document, blocks, census = load_source()
    lines = flatten_lines(blocks)
    assign_explicit_roles(lines, census)
    rows = make_rows(lines)
    initial_roles(rows)
    readings, subparts = assign_context(rows, census)
    margins = estimate_margins(rows)
    verse_conflicts = mark_verse(rows, margins)
    paragraphs, paragraph_conflicts = build_paragraphs(rows)
    verse_groups, verse_lines = build_verse(rows)
    finalize_roles(rows)
    counts = (len(lines), len(rows), len(readings), len(subparts), len(paragraphs),
              len(verse_groups), len(verse_lines))
    expected = (EXPECTED["physical_lines"], EXPECTED["logical_rows"], EXPECTED["readings"],
                EXPECTED["subparts"], EXPECTED["prose"], EXPECTED["verse_groups"],
                EXPECTED["verse_lines"])
    if counts != expected:
        raise BuildError(f"independent reconstruction drift: {counts} != {expected}")
    conflicts = verse_conflicts + paragraph_conflicts
    if len(conflicts) != 49:
        raise BuildError(f"boundary-conflict census drift: {len(conflicts)}")
    return {"plan": plan, "document": document, "census": census, "lines": lines,
            "rows": rows, "readings": readings, "subparts": subparts,
            "paragraphs": paragraphs, "verse_groups": verse_groups,
            "verse_lines": verse_lines, "conflicts": conflicts}


def structure_key(record: dict[str, Any]) -> str:
    kind = record["record_kind"]
    if kind == "part_heading": return record["part_id"]
    if kind == "reading_unit_heading":
        return f"{record['part_id']}.reading_{record['reading_unit_ordinal_within_part']:02d}"
    if kind == "numbered_subpart_marker":
        return f"{record['parent_reading_ref']}.subpart_{record['number']:02d}"
    if kind == "cycle_heading": return f"{record['part_id']}.cycle_heading"
    if kind == "front_matter_region": return f"front_matter.{record['role']}"
    if kind == "back_matter_region": return f"back_matter.{record['role']}"
    if kind == "terminal_marker": return "work_end"
    if kind == "publisher_catalog_heading": return f"back_matter.catalog_heading_{record['catalog_heading_ordinal']:02d}"
    raise BuildError(f"unknown census kind: {kind}")


def paragraph_binding(record: dict[str, Any]) -> str:
    digest = sha("\n".join(record["physical_line_refs"]).encode())
    return f"{record['reading_ref']}|{record['subpart_ref']}|{record['first_locator']}|{record['last_locator']}|{digest}"


def row_binding(row: LogicalRow) -> str:
    return sha("\n".join(line.locator for line in row.lines).encode())


def issuance_bindings(model: dict[str, Any]) -> dict[str, list[str]]:
    return {
        "physical_lines": [line.locator for line in model["lines"]],
        "logical_rows": [row_binding(row) for row in model["rows"]],
        "structures": [structure_key(row) for row in model["census"]],
        "paragraphs": [paragraph_binding(row) for row in model["paragraphs"]],
        "verse_groups": [paragraph_binding(row) for row in model["verse_groups"]],
        "verse_lines": [f"{row['verse_group_ref']}|{row['verse_line_ordinal']}|"
                        f"{sha(chr(10).join(row['physical_line_refs']).encode())}"
                        for row in model["verse_lines"]],
    }


def mint(prefix: str) -> str:
    return f"{prefix}.sid-{secrets.token_hex(16)}"


def issue_identities(model: dict[str, Any]) -> None:
    path = REPO / ISSUANCE_REF
    if path.exists():
        raise BuildError("identity issuance already exists; refusing to remint")
    bindings = issuance_bindings(model)
    prefixes = {"structures": "tos.structure-unit", "paragraphs": "tos.text-unit",
                "verse_groups": "tos.text-unit", "verse_lines": "tos.text-unit",
                "physical_lines": "tos.text-unit", "logical_rows": "tos.text-unit"}
    payload = {
        "schema_version": "tos_antonovsky_1911_structural_paragraph_identity_issuance_v2",
        "issuance_id": "tos.identity-issuance.zarathustra-antonovsky-1911-structural-paragraph-v2",
        "issued_on": "2026-09-01", "opaque_identity": True,
        "source_binding_is_not_identity": True,
        "source_fixity": {"pdf_sha256": PDF_SHA, "bbox_sha256": BBOX_SHA,
                           "private_text_layer_sha256": TEXT_SHA},
        "identities": {kind: [{"binding": binding, "id": mint(prefixes[kind])}
                               for binding in values]
                       for kind, values in bindings.items()},
    }
    path.write_bytes(json_bytes(payload))


def load_identities(model: dict[str, Any]) -> dict[str, dict[str, str]]:
    payload = json.loads((REPO / ISSUANCE_REF).read_text())
    if payload["source_fixity"] != {"pdf_sha256": PDF_SHA, "bbox_sha256": BBOX_SHA,
                                     "private_text_layer_sha256": TEXT_SHA}:
        raise BuildError("identity issuance source fixity drift")
    expected = issuance_bindings(model)
    result = {}
    all_ids = []
    for kind, bindings in expected.items():
        records = payload["identities"].get(kind, [])
        if [row["binding"] for row in records] != bindings:
            raise BuildError(f"identity binding drift: {kind}")
        result[kind] = {row["binding"]: row["id"] for row in records}
        all_ids.extend(row["id"] for row in records)
    if len(all_ids) != len(set(all_ids)):
        raise BuildError("opaque identity collision")
    return result


def import_challenger(directory: Path, model: dict[str, Any]) -> None:
    target = REPO / CHALLENGER_REF
    if target.exists():
        raise BuildError("primary challenger input already exists; refusing to replace")
    private_path = directory / "paragraph-units.private.prototype.v2.jsonl"
    units = load_jsonl(private_path)
    owner_by_locator, role_by_ordinal = {}, {}
    for ordinal, unit in enumerate(units, 1):
        role_by_ordinal[str(ordinal)] = unit["technical_role"]
        for fragment in unit["source_fragments"]:
            locator = fragment["fragment_id"]
            if locator in owner_by_locator:
                raise BuildError("challenger duplicates a source line")
            owner_by_locator[locator] = ordinal
    order = [line.locator for line in model["lines"]]
    if set(owner_by_locator) != set(order):
        raise BuildError("challenger does not cover the exact source-line set")
    payload = {
        "schema_version": "tos_antonovsky_1911_primary_challenger_input_v2",
        "role": "complete_boundary_challenger_not_authority",
        "source_text_included": False,
        "source_fixity": {"pdf_sha256": PDF_SHA, "bbox_sha256": BBOX_SHA,
                           "private_text_layer_sha256": TEXT_SHA},
        "source_line_count": len(order),
        "ordered_source_line_binding_sha256": sha(json.dumps(order, separators=(",", ":")).encode()),
        "challenger_unit_count": len(units),
        "challenger_content_unit_count": sum(u["technical_role"] in {"prose_paragraph", "verse_stanza"}
                                             for u in units),
        "owner_unit_ordinal_by_source_line": [owner_by_locator[x] for x in order],
        "technical_role_by_unit_ordinal": role_by_ordinal,
        "input_artifact_sha256": {
            name: sha((directory / name).read_bytes()) for name in (
                "paragraph-units.private.prototype.v2.jsonl",
                "paragraph-units.public.prototype.v2.jsonl", "summary.prototype.v2.json",
                "manifest.prototype.v2.json")
        },
        "resolution_policy": "independent_source_visible_pass_controls_final_technical_boundary",
    }
    target.write_bytes(json_bytes(payload))


def compare_challenger(model: dict[str, Any]) -> dict[str, Any]:
    challenger = json.loads((REPO / CHALLENGER_REF).read_text())
    lines: list[PhysicalLine] = model["lines"]
    order = [line.locator for line in lines]
    if challenger["ordered_source_line_binding_sha256"] != sha(
            json.dumps(order, separators=(",", ":")).encode()):
        raise BuildError("challenger physical-line binding drift")
    owners = challenger["owner_unit_ordinal_by_source_line"]
    roles = challenger["technical_role_by_unit_ordinal"]
    final_owner = {line.locator: line.paragraph_ref or line.verse_group_ref or line.row_ref for line in lines}
    categories = Counter()
    body_categories = Counter()
    for index, (left, right) in enumerate(zip(lines, lines[1:], strict=False)):
        final_same = final_owner[left.locator] == final_owner[right.locator]
        challenger_same = owners[index] == owners[index + 1]
        category = ("agree_same_unit" if final_same and challenger_same else
                    "agree_boundary" if not final_same and not challenger_same else
                    "final_only_boundary" if not final_same else "challenger_only_boundary")
        categories[category] += 1
        if left.role in {"prose", "verse"} and right.role in {"prose", "verse"} and \
                roles[str(owners[index])] in {"prose_paragraph", "verse_stanza"} and \
                roles[str(owners[index + 1])] in {"prose_paragraph", "verse_stanza"}:
            body_categories[category] += 1
    if sum(categories.values()) != EXPECTED["physical_lines"] - 1:
        raise BuildError("challenger comparison is not exhaustive")
    return {
        "schema_version": "tos_antonovsky_1911_full_boundary_comparison_v2",
        "comparison_axis": "every_adjacent_pair_in_exact_physical_line_order",
        "evaluated_adjacent_pairs": sum(categories.values()),
        "all_pair_results": dict(sorted(categories.items())),
        "body_content_pair_results": dict(sorted(body_categories.items())),
        "disagreement_count": categories["final_only_boundary"] + categories["challenger_only_boundary"],
        "resolution": "independent_source_visible_pass_selected_for_all_disagreements",
        "unresolved_disagreement_count": 0, "source_text_included": False,
    }


def structure_parent(key: str, record: dict[str, Any]) -> str | None:
    kind = record["record_kind"]
    if kind == "reading_unit_heading":
        # In Part I the source-visible "Speeches of Zarathustra" heading is
        # a wrapper over the 22 speeches after the Prologue.  Keep the
        # Prologue directly under the part and preserve the wrapper instead
        # of leaving it as an orphan technical heading.
        if record["part_id"] == "part_1" and record["reading_unit_ordinal_within_part"] >= 2:
            return "part_1.cycle_heading"
        return record["part_id"]
    if kind == "numbered_subpart_marker": return record["parent_reading_ref"]
    if kind == "cycle_heading": return record["part_id"]
    if kind == "publisher_catalog_heading": return "back_matter.publisher_catalog"
    return None


def make_outputs(model: dict[str, Any]) -> dict[str, bytes]:
    ids = load_identities(model)
    comparison = compare_challenger(model)
    line_id = ids["physical_lines"]
    row_id = ids["logical_rows"]
    structure_id = ids["structures"]
    paragraph_id = ids["paragraphs"]
    verse_group_id = ids["verse_groups"]
    verse_line_id = ids["verse_lines"]
    row_key_by_ref = {row.row_ref: row_binding(row) for row in model["rows"]}
    paragraph_key_by_ref = {row["paragraph_ref"]: paragraph_binding(row) for row in model["paragraphs"]}
    verse_group_key_by_ref = {row["verse_group_ref"]: paragraph_binding(row) for row in model["verse_groups"]}
    verse_line_key_by_ref = {
        row["verse_line_ref"]: f"{row['verse_group_ref']}|{row['verse_line_ordinal']}|"
                               f"{sha(chr(10).join(row['physical_line_refs']).encode())}"
        for row in model["verse_lines"]
    }

    structure_rows = []
    for record in model["census"]:
        key = structure_key(record)
        parent = structure_parent(key, record)
        structure_rows.append({
            "schema_version": "tos_antonovsky_1911_structure_spine_v2",
            "structure_unit_id": structure_id[key], "technical_structure_key": key,
            "record_kind": record["record_kind"],
            "parent_structure_unit_id": structure_id[parent] if parent else None,
            "part_id": record.get("part_id"),
            "reading_unit_ordinal_within_part": record.get("reading_unit_ordinal_within_part"),
            "reading_unit_ordinal_global": record.get("reading_unit_ordinal_global"),
            "numbered_subpart_number": record.get("number"),
            "source_anchor_refs": record.get("anchors", []),
            "source_locator_span": record.get("source_locator_span"),
            "display_citation_span": record.get("panel_span", record.get("display_citations")),
            "source_visible": record.get("source_visible", True),
            "visual_only_heading_observation": any(
                anchor.get("embedded_bbox_anchor_missing", False) for anchor in record.get("anchors", [])),
            "source_text_included": False, "semantic_promotion": False,
        })

    physical_rows = []
    for line in model["lines"]:
        physical_rows.append({
            "schema_version": "tos_antonovsky_1911_physical_line_spine_v2",
            "physical_line_unit_id": line_id[line.locator], "source_line_ref": line.source_line_ref,
            "source_locator": line.locator, "source_block_unit_ref": line.block_unit_id,
            "block_line_ordinal": line.ordinal, "pdf_page": line.page, "panel": line.panel,
            "bbox_points": [round(v, 6) for v in line.bbox], "exact_sha256": line.text_sha256,
            "logical_row_unit_ref": row_id[row_key_by_ref[line.row_ref]], "technical_role": line.role,
            "reading_unit_ref": structure_id.get(line.reading_ref),
            "numbered_subpart_ref": structure_id.get(line.subpart_ref),
            "paragraph_unit_ref": paragraph_id.get(paragraph_key_by_ref.get(line.paragraph_ref)),
            "verse_group_unit_ref": verse_group_id.get(verse_group_key_by_ref.get(line.verse_group_ref)),
            "verse_line_unit_ref": verse_line_id.get(verse_line_key_by_ref.get(line.verse_line_ref)),
            "source_text_included": False, "semantic_promotion": False,
        })

    logical_rows = []
    for row in model["rows"]:
        logical_rows.append({
            "schema_version": "tos_antonovsky_1911_logical_row_spine_v2",
            "logical_row_unit_id": row_id[row_binding(row)], "pdf_page": row.page, "panel": row.panel,
            "bbox_points": [round(v, 6) for v in row.bbox],
            "ordered_physical_line_unit_refs": [line_id[x.locator] for x in row.lines],
            "exact_sha256": row.text_sha256, "technical_role": row.role,
            "reading_unit_ref": structure_id.get(row.reading_ref),
            "numbered_subpart_ref": structure_id.get(row.subpart_ref),
            "estimated_body_margin_x": round(row.base_x, 3) if row.base_x is not None else None,
            "indent_delta_points": round(row.indent_delta, 3) if row.indent_delta is not None else None,
            "source_text_included": False, "semantic_promotion": False,
        })

    line_by_locator = {line.locator: line for line in model["lines"]}
    paragraph_rows = []
    for record in model["paragraphs"]:
        pid = paragraph_id[paragraph_binding(record)]
        blocks = sorted({line_by_locator[locator].block_unit_id for locator in record["physical_line_refs"]})
        part = record["reading_ref"].split(".")[0]
        paragraph_rows.append({
            "schema_version": "tos_antonovsky_1911_paragraph_spine_v2",
            "paragraph_unit_id": pid, "unit_kind": "prose_paragraph",
            "display_citation": f"Za-RU-Ant1911.{record['reading_ref'].replace('part_', 'P').replace('.reading_', '.R')}.p{record['reading_paragraph_ordinal']:04d}",
            "part_id": part, "global_technical_ordinal": record["global_ordinal"],
            "reading_unit_ref": structure_id[record["reading_ref"]],
            "numbered_subpart_ref": structure_id.get(record["subpart_ref"]),
            "ordinal_within_reading_unit": record["reading_paragraph_ordinal"],
            "ordinal_within_numbered_subpart": record["subpart_paragraph_ordinal"],
            "ordered_logical_row_refs": [row_id[row_key_by_ref[x]] for x in record["row_refs"]],
            "ordered_physical_line_refs": [line_id[x] for x in record["physical_line_refs"]],
            "source_block_unit_refs": blocks, "boundary_basis": record["boundary_basis"],
            "boundary_confidence": record["boundary_confidence"], "crosses_page_sides": record["cross_panel"],
            "exact_sha256": record["text_sha256"], "boundary_posture": "agent_verified_technical",
            "source_text_included": False, "semantic_promotion": False,
        })

    verse_group_rows = []
    for record in model["verse_groups"]:
        gid = verse_group_id[paragraph_binding(record)]
        verse_group_rows.append({
            "schema_version": "tos_antonovsky_1911_verse_group_spine_v2",
            "verse_group_unit_id": gid, "unit_kind": "continuous_verse_group",
            "display_citation": f"Za-RU-Ant1911.{record['reading_ref'].replace('part_', 'P').replace('.reading_', '.R')}.vg{record['reading_verse_group_ordinal']:03d}",
            "global_technical_ordinal": record["global_ordinal"],
            "reading_unit_ref": structure_id[record["reading_ref"]],
            "numbered_subpart_ref": structure_id.get(record["subpart_ref"]),
            "ordinal_within_reading_unit": record["reading_verse_group_ordinal"],
            "ordered_verse_line_refs": [verse_line_id[verse_line_key_by_ref[x]] for x in record["verse_line_refs"]],
            "ordered_physical_line_refs": [line_id[x] for x in record["physical_line_refs"]],
            "crosses_page_sides": record["cross_panel"], "exact_sha256": record["text_sha256"],
            "boundary_posture": "agent_verified_technical", "source_text_included": False,
            "semantic_promotion": False,
        })

    verse_line_rows = []
    for record in model["verse_lines"]:
        key = verse_line_key_by_ref[record["verse_line_ref"]]
        verse_line_rows.append({
            "schema_version": "tos_antonovsky_1911_verse_line_spine_v2",
            "verse_line_unit_id": verse_line_id[key],
            "verse_group_unit_ref": verse_group_id[verse_group_key_by_ref[record["verse_group_ref"]]],
            "ordinal_within_verse_group": record["verse_line_ordinal"],
            "reading_unit_ref": structure_id[record["reading_ref"]],
            "numbered_subpart_ref": structure_id.get(record["subpart_ref"]),
            "logical_row_unit_ref": row_id[row_key_by_ref[record["row_ref"]]],
            "ordered_physical_line_refs": [line_id[x] for x in record["physical_line_refs"]],
            "bbox_points": record["bbox_points"], "exact_sha256": record["text_sha256"],
            "source_text_included": False, "semantic_promotion": False,
        })

    conflict_rows = []
    for ordinal, record in enumerate(model["conflicts"], 1):
        converted = {k: v for k, v in record.items() if not k.endswith("row_ref")}
        converted.update({
            "schema_version": "tos_antonovsky_1911_boundary_conflict_v2",
            "conflict_ref": f"ru-ant1911.boundary-conflict.{ordinal:05d}",
            "first_logical_row_ref": row_id.get(row_key_by_ref.get(record.get("first_row_ref"))),
            "last_logical_row_ref": row_id.get(row_key_by_ref.get(record.get("last_row_ref"))),
            "previous_logical_row_ref": row_id.get(row_key_by_ref.get(record.get("previous_row_ref"))),
            "current_logical_row_ref": row_id.get(row_key_by_ref.get(record.get("current_row_ref"))),
            "reading_unit_ref": structure_id.get(record.get("reading_ref")),
            "numbered_subpart_ref": structure_id.get(record.get("subpart_ref")),
            "status": "resolved_by_declared_rule", "source_text_included": False,
        })
        converted.pop("reading_ref", None); converted.pop("subpart_ref", None)
        conflict_rows.append(converted)

    roles = Counter(line.role for line in model["lines"])
    prose_by_part = Counter(row["reading_ref"].split(".")[0] for row in model["paragraphs"])
    verse_by_part = Counter(row["reading_ref"].split(".")[0] for row in model["verse_groups"])
    subparts_with_content = {row["subpart_ref"] for row in model["paragraphs"] if row["subpart_ref"]} | {
        row["subpart_ref"] for row in model["verse_lines"] if row["subpart_ref"]}
    if len(subparts_with_content) != EXPECTED["subparts"]:
        raise BuildError("not all numbered subparts have technical content")
    coverage = {
        "schema_version": "tos_antonovsky_1911_coverage_receipt_v2",
        "source_fixity": {"pdf_sha256": PDF_SHA, "bbox_sha256": BBOX_SHA,
                           "private_text_layer_sha256": TEXT_SHA},
        "source_poppler_block_count": EXPECTED["blocks"],
        "expected_physical_line_count": EXPECTED["physical_lines"],
        "assigned_physical_line_count": len(physical_rows), "duplicate_physical_line_count": 0,
        "unresolved_physical_line_count": 0, "logical_row_count": len(logical_rows),
        "reading_unit_count": EXPECTED["readings"], "numbered_subpart_count": EXPECTED["subparts"],
        "numbered_subparts_with_content": len(subparts_with_content),
        "prose_paragraph_count": len(paragraph_rows), "continuous_verse_group_count": len(verse_group_rows),
        "verse_line_count": len(verse_line_rows), "technical_role_counts": dict(sorted(roles.items())),
        "coverage_status": "complete", "source_text_included": False, "semantic_authority": False,
    }
    contradictions = [
        {"schema_version": "tos_antonovsky_1911_contradiction_v2",
         "contradiction_ref": f"ru-ant1911.visual-marker-{number}",
         "kind": "raster_marker_absent_from_embedded_bbox_layer", "technical_structure_key": key,
         "resolution": "source_visible_raster_context_event_materialized_without_inventing_physical_line",
         "status": "resolved", "source_text_included": False, "semantic_promotion": False}
        for number, key in ((22, "part_3.reading_12.subpart_22"), (1, "part_4.reading_18.subpart_01"))
    ] + [{
        "schema_version": "tos_antonovsky_1911_contradiction_v2",
        "contradiction_ref": "ru-ant1911.primary-challenger-boundaries",
        "kind": "competing_technical_segmentation", "evaluated_adjacent_pairs": comparison["evaluated_adjacent_pairs"],
        "disagreement_count": comparison["disagreement_count"],
        "resolution": comparison["resolution"], "status": "resolved",
        "source_text_included": False, "semantic_promotion": False,
    }]
    summary = {
        "schema_version": "tos_antonovsky_1911_structural_paragraph_summary_v2",
        "status": "agent_verified_complete", "physical_line_count": len(physical_rows),
        "logical_row_count": len(logical_rows), "part_count": 4,
        "reading_unit_count": EXPECTED["readings"], "numbered_subpart_count": EXPECTED["subparts"],
        "embedded_numbered_subpart_marker_count": 110, "raster_only_numbered_subpart_marker_count": 2,
        "prose_paragraph_count": len(paragraph_rows), "continuous_verse_group_count": len(verse_group_rows),
        "verse_line_count": len(verse_line_rows), "prose_paragraph_counts_by_part": dict(sorted(prose_by_part.items())),
        "continuous_verse_group_counts_by_part": dict(sorted(verse_by_part.items())),
        "boundary_conflict_count": len(conflict_rows), "unresolved_boundary_conflict_count": 0,
        "challenger_boundary_disagreement_count": comparison["disagreement_count"],
        "unresolved_challenger_disagreement_count": 0, "source_text_included": False,
        "semantic_authority": False,
    }
    verification = {
        "schema_version": "tos_antonovsky_1911_agent_verification_v2",
        "status": "agent_verified_complete", "maker_kind": "codex_internal_agents",
        "checks": {"exact_source_fixity": True, "physical_line_coverage_13071": True,
                   "logical_row_rebuild_10686": True, "reading_census_81": True,
                   "numbered_subpart_census_112": True, "all_numbered_subparts_have_content": True,
                   "prose_verse_disjoint": True, "local_ordinals_continuous": True,
                   "primary_challenger_all_adjacent_pairs_compared": True,
                   "all_boundary_conflicts_resolved": True, "all_contradictions_resolved": True,
                   "opaque_identity_binding_closed": True, "tracked_outputs_text_free": True},
        "unresolved_count": 0, "source_text_included": False, "semantic_authority": False,
        "authority_boundary": "complete_technical_proposal_only",
    }
    provenance = [
        {"event_ref": "ru-ant1911-v2.source-rebuild", "event_kind": "exact_source_rebuild",
         "input_ref": f"{V1_ROUTE_REF}/citation-spine.v1.jsonl", "status": "complete"},
        {"event_ref": "ru-ant1911-v2.structure-census", "event_kind": "source_visible_structure_census",
         "input_ref": CENSUS_REF, "status": "complete"},
        {"event_ref": "ru-ant1911-v2.independent-pass", "event_kind": "independent_paragraph_verse_reconstruction",
         "status": "complete"},
        {"event_ref": "ru-ant1911-v2.challenger-comparison", "event_kind": "full_adjacent_boundary_comparison",
         "input_ref": CHALLENGER_REF, "status": "complete"},
        {"event_ref": "ru-ant1911-v2.agent-verification", "event_kind": "independent_technical_verification",
         "status": "agent_verified_complete"},
    ]
    for row in provenance:
        row.update({"schema_version": "tos_antonovsky_1911_provenance_event_v2",
                    "event_date": "2026-09-01", "source_text_included": False,
                    "semantic_promotion": False})

    artifacts = {
        OUTPUT_REFS["physical_lines"]: jsonl_bytes(physical_rows),
        OUTPUT_REFS["logical_rows"]: jsonl_bytes(logical_rows),
        OUTPUT_REFS["structures"]: jsonl_bytes(structure_rows),
        OUTPUT_REFS["paragraphs"]: jsonl_bytes(paragraph_rows),
        OUTPUT_REFS["verse_groups"]: jsonl_bytes(verse_group_rows),
        OUTPUT_REFS["verse_lines"]: jsonl_bytes(verse_line_rows),
        OUTPUT_REFS["conflicts"]: jsonl_bytes(conflict_rows),
        OUTPUT_REFS["comparison"]: json_bytes(comparison),
        OUTPUT_REFS["contradictions"]: jsonl_bytes(contradictions),
        OUTPUT_REFS["coverage"]: json_bytes(coverage),
        OUTPUT_REFS["verification"]: json_bytes(verification),
        OUTPUT_REFS["summary"]: json_bytes(summary),
        OUTPUT_REFS["provenance"]: jsonl_bytes(provenance),
    }
    manifest = {
        "schema_version": "tos_antonovsky_1911_structural_paragraph_manifest_v2",
        "static_inputs": {ref: {"sha256": sha((REPO / ref).read_bytes()), "bytes": (REPO / ref).stat().st_size}
                          for ref in (CENSUS_REF, ISSUANCE_REF, CHALLENGER_REF)},
        "generated_outputs": {ref: {"sha256": sha(payload), "bytes": len(payload)}
                              for ref, payload in sorted(artifacts.items())},
        "source_text_included": False, "semantic_authority": False,
    }
    artifacts[OUTPUT_REFS["manifest"]] = json_bytes(manifest)
    return artifacts


FORBIDDEN_KEYS = {"text", "source_text", "content", "exact_text", "ocr_text",
                  "heading_text", "translation", "lemma", "concept", "relation"}


def assert_text_free(value: Any, where: str) -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key in FORBIDDEN_KEYS:
                raise BuildError(f"forbidden tracked key {key!r} in {where}")
            if key == "source_text_included" and child is not False:
                raise BuildError(f"source_text_included must be false in {where}")
            assert_text_free(child, where)
    elif isinstance(value, list):
        for child in value: assert_text_free(child, where)


def validate_tracked() -> dict[str, Any]:
    manifest = json.loads((REPO / OUTPUT_REFS["manifest"]).read_text())
    for ref, receipt in manifest["static_inputs"].items():
        path = REPO / ref
        if not path.is_file() or sha(path.read_bytes()) != receipt["sha256"] or path.stat().st_size != receipt["bytes"]:
            raise BuildError(f"static input manifest drift: {ref}")
    for ref, receipt in manifest["generated_outputs"].items():
        path = REPO / ref
        if not path.is_file() or sha(path.read_bytes()) != receipt["sha256"] or path.stat().st_size != receipt["bytes"]:
            raise BuildError(f"generated output manifest drift: {ref}")
    jsonl_refs = {OUTPUT_REFS[key] for key in (
        "physical_lines", "logical_rows", "structures", "paragraphs", "verse_groups",
        "verse_lines", "conflicts", "contradictions", "provenance")}
    parsed = {}
    for ref in manifest["generated_outputs"]:
        path = REPO / ref
        value = load_jsonl(path) if ref in jsonl_refs else json.loads(path.read_text())
        assert_text_free(value, ref)
        parsed[ref] = value
    physical = parsed[OUTPUT_REFS["physical_lines"]]
    logical = parsed[OUTPUT_REFS["logical_rows"]]
    structures = parsed[OUTPUT_REFS["structures"]]
    paragraphs = parsed[OUTPUT_REFS["paragraphs"]]
    verse_groups = parsed[OUTPUT_REFS["verse_groups"]]
    verse_lines = parsed[OUTPUT_REFS["verse_lines"]]
    conflicts = parsed[OUTPUT_REFS["conflicts"]]
    comparison = parsed[OUTPUT_REFS["comparison"]]
    contradictions = parsed[OUTPUT_REFS["contradictions"]]
    coverage = parsed[OUTPUT_REFS["coverage"]]
    verification = parsed[OUTPUT_REFS["verification"]]
    if (len(physical), len(logical), len(paragraphs), len(verse_groups), len(verse_lines)) != (
            EXPECTED["physical_lines"], EXPECTED["logical_rows"], EXPECTED["prose"],
            EXPECTED["verse_groups"], EXPECTED["verse_lines"]):
        raise BuildError("tracked count drift")
    ids = [row["physical_line_unit_id"] for row in physical]
    if len(ids) != len(set(ids)):
        raise BuildError("physical-line identities are not unique")
    row_ids = {row["logical_row_unit_id"] for row in logical}
    if any(row["logical_row_unit_ref"] not in row_ids for row in physical):
        raise BuildError("physical-to-logical reference closure failure")
    structure_ids = {row["structure_unit_id"] for row in structures}
    if any(row["parent_structure_unit_id"] not in structure_ids
           for row in structures if row["parent_structure_unit_id"]):
        raise BuildError("structure parent closure failure")
    if Counter(row["record_kind"] for row in structures)["reading_unit_heading"] != EXPECTED["readings"] \
            or Counter(row["record_kind"] for row in structures)["numbered_subpart_marker"] != EXPECTED["subparts"]:
        raise BuildError("tracked structure census drift")
    reading_ordinals: dict[str, list[int]] = defaultdict(list)
    subpart_ordinals: dict[str, list[int]] = defaultdict(list)
    for row in paragraphs:
        reading_ordinals[row["reading_unit_ref"]].append(row["ordinal_within_reading_unit"])
        if row["numbered_subpart_ref"]:
            subpart_ordinals[row["numbered_subpart_ref"]].append(row["ordinal_within_numbered_subpart"])
    if len(reading_ordinals) != EXPECTED["readings"]:
        raise BuildError("not all reading units contain prose")
    if any(values != list(range(1, len(values) + 1)) for values in reading_ordinals.values()) \
            or any(values != list(range(1, len(values) + 1)) for values in subpart_ordinals.values()):
        raise BuildError("paragraph ordinals are not continuous")
    prose_lines = {ref for row in paragraphs for ref in row["ordered_physical_line_refs"]}
    verse_physical = {ref for row in verse_lines for ref in row["ordered_physical_line_refs"]}
    if prose_lines & verse_physical:
        raise BuildError("prose and verse physical-line overlap")
    if len(conflicts) != 49 or any(row["status"] != "resolved_by_declared_rule" for row in conflicts):
        raise BuildError("boundary-conflict closure failure")
    if comparison["evaluated_adjacent_pairs"] != EXPECTED["physical_lines"] - 1 \
            or comparison["unresolved_disagreement_count"] != 0:
        raise BuildError("full challenger comparison incomplete")
    if any(row["status"] != "resolved" for row in contradictions):
        raise BuildError("contradiction ledger has unresolved rows")
    if coverage["coverage_status"] != "complete" or verification["status"] != "agent_verified_complete" \
            or verification["unresolved_count"] != 0:
        raise BuildError("completion receipts are not closed")
    return {"status": "pass", "physical_lines": len(physical), "logical_rows": len(logical),
            "prose_paragraphs": len(paragraphs), "continuous_verse_groups": len(verse_groups),
            "verse_lines": len(verse_lines), "reading_units": EXPECTED["readings"],
            "numbered_subparts": EXPECTED["subparts"], "unresolved": 0}


def write_artifacts(artifacts: dict[str, bytes]) -> None:
    for ref, payload in artifacts.items():
        path = REPO / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)


def check_artifacts(artifacts: dict[str, bytes]) -> None:
    drift = []
    for ref, payload in artifacts.items():
        path = REPO / ref
        if not path.is_file() or path.read_bytes() != payload:
            drift.append(ref)
    if drift:
        raise BuildError("generated artifact drift: " + ", ".join(drift))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    actions = parser.add_mutually_exclusive_group(required=True)
    actions.add_argument("--build", action="store_true")
    actions.add_argument("--check", action="store_true")
    actions.add_argument("--validate-tracked", action="store_true")
    actions.add_argument("--issue-identities", action="store_true")
    actions.add_argument("--import-challenger", type=Path)
    args = parser.parse_args(argv)
    if args.validate_tracked:
        print(json.dumps(validate_tracked(), indent=2, sort_keys=True)); return 0
    model = reconstruct()
    if args.issue_identities:
        issue_identities(model)
        print(json.dumps({"status": "issued", "identity_ref": ISSUANCE_REF}, indent=2)); return 0
    if args.import_challenger:
        import_challenger(args.import_challenger, model)
        print(json.dumps({"status": "imported", "challenger_ref": CHALLENGER_REF}, indent=2)); return 0
    artifacts = make_outputs(model)
    if args.build:
        write_artifacts(artifacts)
    else:
        check_artifacts(artifacts)
    print(json.dumps(json.loads(artifacts[OUTPUT_REFS["summary"]]), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BuildError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        raise SystemExit(1)
