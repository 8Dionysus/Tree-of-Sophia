#!/usr/bin/env python3
"""Prepared philosophy dossier planting entrypoint."""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from functools import cache
from pathlib import Path
from typing import Any

from plant_table_i_prepared_dossiers import DOC_ROOT, DOSSIER_ID_PATTERN
from plant_table_i_prepared_dossiers import PACKAGES, PACKAGE_ROUTES, SUPPORTED_TABLES
from plant_table_i_prepared_dossiers import blocked_dossiers, discover_docx, extract_dossier_id
from plant_table_i_prepared_dossiers import docx_package_metadata
from plant_table_i_prepared_dossiers import load_jsonl as load_pipeline_jsonl
from plant_table_i_prepared_dossiers import plant_supported_packages
from plant_table_i_prepared_dossiers import parse_dossier

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


@cache
def validate_local_docx_contents(table_id: str) -> tuple[dict[str, str], ...]:
    """Read and identity-check every package DOCX without writing planting outputs."""

    master_rows = load_pipeline_jsonl(TABLE_ROOT / table_id / "rows.jsonl")
    master_rows_by_id = {
        str(row.get("row_id") or ""): row
        for row in master_rows
        if isinstance(row, dict) and row.get("row_id")
    }
    errors: list[dict[str, str]] = []
    try:
        paths = discover_docx(table_id)
    except (Exception, SystemExit) as exc:
        return (
            {
                "dossier_id": "",
                "path": table_id,
                "error_type": type(exc).__name__,
                "message": str(exc),
            },
        )
    for path in paths:
        dossier_id = extract_dossier_id(path)
        try:
            master_row = master_rows_by_id[dossier_id]
            parse_dossier(path, master_row, table_id)
            docx_package_metadata(path)
        except (Exception, SystemExit) as exc:
            errors.append(
                {
                    "dossier_id": dossier_id,
                    "path": path.relative_to(DOC_ROOT).as_posix(),
                    "error_type": type(exc).__name__,
                    "message": str(exc),
                }
            )
    return tuple(errors)


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
        expected_master_ids = sorted(set(expected) | set(missing_master_ids))
        master_row_ids = [str(row.get("row_id") or "") for row in rows]
        master_row_id_counts = Counter(master_row_ids)
        unexpected_master_ids = sorted(
            dossier_id
            for dossier_id in master_row_id_counts
            if dossier_id not in set(expected_master_ids)
        )
        missing_expected_master_ids = [
            dossier_id for dossier_id in expected_master_ids if master_row_id_counts[dossier_id] == 0
        ]
        duplicate_expected_master_ids = [
            dossier_id for dossier_id in expected_master_ids if master_row_id_counts[dossier_id] > 1
        ]
        matched_expected_master_ids = [
            dossier_id for dossier_id in expected_master_ids if master_row_id_counts[dossier_id] == 1
        ]
        master_rows_by_id = {
            str(row.get("row_id") or ""): row
            for row in rows
            if isinstance(row, dict) and master_row_id_counts[str(row.get("row_id") or "")] == 1
        }
        invalid_expected_master_rows: list[dict[str, Any]] = []
        for dossier_id in matched_expected_master_ids:
            row = master_rows_by_id[dossier_id]
            errors: list[str] = []
            if row.get("table_id") != table_id:
                errors.append("table_id_mismatch")
            normalized = row.get("normalized")
            if not isinstance(normalized, dict):
                errors.append("normalized_metadata_missing")
            elif normalized.get("row_id") != dossier_id:
                errors.append("normalized_row_id_mismatch")
            if errors:
                invalid_expected_master_rows.append(
                    {"dossier_id": dossier_id, "errors": errors}
                )
        master_expected_rows_valid = not invalid_expected_master_rows
        master_expected_ids_unique = (
            not missing_expected_master_ids
            and not duplicate_expected_master_ids
            and not unexpected_master_ids
        )
        structural_preflight_ready = (
            local_docx_ids_unique
            and unique_local_docx_ids == expected
            and master_expected_ids_unique
            and master_expected_rows_valid
        )
        docx_content_validation_performed = structural_preflight_ready
        docx_validation_errors = (
            list(validate_local_docx_contents(table_id))
            if docx_content_validation_performed
            else []
        )
        docx_contents_valid = (
            docx_content_validation_performed and not docx_validation_errors
        )
        package_ready = structural_preflight_ready and docx_contents_valid
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
            "expected_master_dossier_ids": expected_master_ids,
            "master_row_ids": master_row_ids,
            "master_expected_ids_unique": master_expected_ids_unique,
            "master_expected_rows_valid": master_expected_rows_valid,
            "invalid_expected_master_rows": invalid_expected_master_rows,
            "matched_expected_master_ids": matched_expected_master_ids,
            "missing_expected_master_ids": missing_expected_master_ids,
            "duplicate_expected_master_ids": duplicate_expected_master_ids,
            "unexpected_master_ids": unexpected_master_ids,
            "local_docx_ids": local_docx_ids,
            "local_docx_ids_unique": local_docx_ids_unique,
            "duplicate_local_docx_ids": duplicate_local_docx_ids,
            "matched_local_docx_ids": sorted(set(expected) & set(unique_local_docx_ids)),
            "missing_expected_docx_ids": sorted(set(expected) - set(unique_local_docx_ids)),
            "extra_local_docx_ids": sorted(set(unique_local_docx_ids) - set(expected)),
            "docx_content_validation_performed": docx_content_validation_performed,
            "docx_contents_valid": docx_contents_valid,
            "docx_validation_errors": docx_validation_errors,
            "readiness_scope": "package_local",
            "package_ready_to_plant": package_ready,
            "planting_mode": str(package.get("planting_mode") or "complete"),
            "master_alignment": f"{len(routed_ids)}/{len(rows)}",
            "master_expected_alignment": f"{len(matched_expected_master_ids)}/{len(expected_master_ids)}",
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


def require_aggregate_readiness() -> dict[str, Any]:
    readiness = readiness_payload()
    if readiness["ready_to_plant"]:
        return readiness
    failed_packages = sorted(
        table_id
        for table_id, ready in readiness["required_supported_package_readiness"].items()
        if not ready
    )
    raise SystemExit(
        "prepared dossier planting not ready for supported packages: "
        f"{', '.join(failed_packages)}; run --readiness for exact blockers"
    )


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
    require_aggregate_readiness()
    return plant_supported_packages()


if __name__ == "__main__":
    raise SystemExit(main())
