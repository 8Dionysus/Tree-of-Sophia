#!/usr/bin/env python3
"""Prepared philosophy dossier planting entrypoint."""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from pathlib import Path
from typing import Any

from plant_table_i_prepared_dossiers import DOC_ROOT, DOSSIER_ID_PATTERN
from plant_table_i_prepared_dossiers import PACKAGES, PACKAGE_ROUTES, SUPPORTED_TABLES
from plant_table_i_prepared_dossiers import blocked_dossiers, main as plant_supported_packages

REPO_ROOT = Path(__file__).resolve().parents[1]
TABLE_ROOT = REPO_ROOT / "ToS/philosophy/atlas/master-tables"


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def discover_local_docx_ids(sections: tuple[str, ...]) -> dict[str, list[str]]:
    ids_by_section: dict[str, list[str]] = {}
    if not DOC_ROOT.exists():
        return ids_by_section
    for section in sections:
        for path in sorted((DOC_ROOT / section).glob("*.docx")):
            match = DOSSIER_ID_PATTERN.search(path.name)
            if not match:
                continue
            ids_by_section.setdefault(section, []).append(match.group(0))
    return {section: sorted(ids) for section, ids in sorted(ids_by_section.items())}


def table_readiness(table_id: str) -> dict[str, Any]:
    rows = load_jsonl(TABLE_ROOT / table_id / "rows.jsonl")
    package = PACKAGES[table_id]
    sections = tuple(str(value) for value in package.get("docx_sections", []))
    local_sections = discover_local_docx_ids(sections)
    local_docx_ids = sorted(item for ids in local_sections.values() for item in ids)
    local_docx_id_counts = Counter(local_docx_ids)
    local_docx_ids_unique = all(count == 1 for count in local_docx_id_counts.values())
    duplicate_local_docx_ids = sorted(
        dossier_id for dossier_id, count in local_docx_id_counts.items() if count > 1
    )
    unique_local_docx_ids = sorted(local_docx_id_counts)
    if table_id in SUPPORTED_TABLES:
        routed_ids = sorted(PACKAGE_ROUTES[table_id])
        blocked_ids = sorted(blocked_dossiers(table_id))
        expected = sorted(set(routed_ids) | set(blocked_ids))
        missing_master_ids = [str(value) for value in package.get("missing_master_dossier_ids", [])]
        master_row_ids = [str(row.get("row_id") or "") for row in rows]
        master_row_id_counts = Counter(master_row_ids)
        missing_expected_master_ids = [
            dossier_id for dossier_id in expected if master_row_id_counts[dossier_id] == 0
        ]
        duplicate_expected_master_ids = [
            dossier_id for dossier_id in expected if master_row_id_counts[dossier_id] > 1
        ]
        matched_expected_master_ids = [
            dossier_id for dossier_id in expected if master_row_id_counts[dossier_id] == 1
        ]
        master_expected_ids_unique = (
            not missing_expected_master_ids and not duplicate_expected_master_ids
        )
        package_ready = (
            local_docx_ids_unique
            and unique_local_docx_ids == expected
            and master_expected_ids_unique
        )
        return {
            "table_id": table_id,
            "row_count": len(rows),
            "supported": True,
            "planting_entrypoint": "scripts/plant_prepared_dossiers.py --plant",
            "planting_scope": "all_supported_packages",
            "package_implementation": "scripts/plant_table_i_prepared_dossiers.py",
            "route_map_ref": "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json",
            "docx_sections": list(sections),
            "expected_dossier_ids": expected,
            "routed_dossier_ids": routed_ids,
            "blocked_dossier_ids": blocked_ids,
            "missing_master_dossier_ids": missing_master_ids,
            "master_row_ids": master_row_ids,
            "master_expected_ids_unique": master_expected_ids_unique,
            "matched_expected_master_ids": matched_expected_master_ids,
            "missing_expected_master_ids": missing_expected_master_ids,
            "duplicate_expected_master_ids": duplicate_expected_master_ids,
            "local_docx_ids": local_docx_ids,
            "local_docx_ids_unique": local_docx_ids_unique,
            "duplicate_local_docx_ids": duplicate_local_docx_ids,
            "matched_local_docx_ids": sorted(set(expected) & set(unique_local_docx_ids)),
            "missing_expected_docx_ids": sorted(set(expected) - set(unique_local_docx_ids)),
            "extra_local_docx_ids": sorted(set(unique_local_docx_ids) - set(expected)),
            "readiness_scope": "package_local",
            "package_ready_to_plant": package_ready,
            "planting_mode": str(package.get("planting_mode") or "complete"),
            "master_alignment": f"{len(routed_ids)}/{len(rows)}",
            "master_expected_alignment": f"{len(matched_expected_master_ids)}/{len(expected)}",
            "input_admission": f"{len(routed_ids)}/{len(expected)}",
        }
    return {
        "table_id": table_id,
        "row_count": len(rows),
        "supported": False,
        "planting_entrypoint": None,
        "package_implementation": None,
        "route_map_ref": "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json",
        "expected_dossier_ids": [],
        "local_docx_ids": [],
        "local_docx_ids_unique": True,
        "duplicate_local_docx_ids": [],
        "matched_local_docx_ids": [],
        "missing_expected_docx_ids": [],
        "extra_local_docx_ids": [],
        "readiness_scope": "package_local",
        "package_ready_to_plant": False,
        "next_route": "add a source-owned dossier id and branch route map before planting this table",
    }


def readiness_payload(table_id: str | None = None) -> dict[str, Any]:
    table_ids = (table_id,) if table_id else ("table-i", "table-ii", "table-iii")
    supported_readiness = {candidate: table_readiness(candidate) for candidate in SUPPORTED_TABLES}
    tables = {
        candidate: supported_readiness.get(candidate) or table_readiness(candidate)
        for candidate in table_ids
    }
    required_supported_package_readiness = {
        candidate: bool(supported_readiness[candidate]["package_ready_to_plant"])
        for candidate in SUPPORTED_TABLES
    }
    return {
        "schema_version": "tos_prepared_dossier_planting_readiness_v1",
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": "ToS/philosophy/atlas/README.md",
        "doc_root": str(DOC_ROOT),
        "planting_scope": "all_supported_packages",
        "selected_table_id": table_id,
        "required_supported_package_readiness": required_supported_package_readiness,
        "ready_to_plant": all(required_supported_package_readiness.values()),
        "local_docx_sections": {
            candidate: discover_local_docx_ids(
                tuple(str(value) for value in PACKAGES[candidate].get("docx_sections", []))
            )
            for candidate in table_ids
        },
        "tables": tables,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Inspect or run prepared philosophy dossier planting.")
    parser.add_argument(
        "--table",
        choices=["table-i", "table-ii", "table-iii"],
        help="Limit readiness output to one master-table package; planting is aggregate-only.",
    )
    parser.add_argument("--readiness", action="store_true", help="Print planting readiness JSON and exit.")
    parser.add_argument("--plant", action="store_true", help="Run the supported planting package.")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.readiness or not args.plant:
        print(json.dumps(readiness_payload(args.table), ensure_ascii=False, indent=2, sort_keys=True))
        return 0
    if args.table is not None:
        raise SystemExit(
            "prepared-dossier planting is aggregate-only because shared atlas and graph outputs cover all supported "
            "packages; use --plant without --table"
        )
    return plant_supported_packages()


if __name__ == "__main__":
    raise SystemExit(main())
