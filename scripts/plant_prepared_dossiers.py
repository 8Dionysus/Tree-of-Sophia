#!/usr/bin/env python3
"""Prepared philosophy dossier planting entrypoint."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any

from plant_table_i_prepared_dossiers import BRANCHES as TABLE_I_BRANCHES
from plant_table_i_prepared_dossiers import DOC_ROOT, main as plant_table_i

REPO_ROOT = Path(__file__).resolve().parents[1]
TABLE_ROOT = REPO_ROOT / "ToS/philosophy/atlas/master-tables"


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def discover_local_docx_ids() -> dict[str, list[str]]:
    ids_by_section: dict[str, list[str]] = {}
    if not DOC_ROOT.exists():
        return ids_by_section
    for path in sorted(DOC_ROOT.glob("*/*.docx")):
        match = re.search(r"\bA\d{2}\b", path.name)
        if not match:
            continue
        ids_by_section.setdefault(path.parent.name, []).append(match.group(0))
    return {section: sorted(ids) for section, ids in sorted(ids_by_section.items())}


def table_readiness(table_id: str) -> dict[str, Any]:
    rows = load_jsonl(TABLE_ROOT / table_id / "rows.jsonl")
    local_docx_ids = sorted({item for ids in discover_local_docx_ids().values() for item in ids})
    if table_id == "table-i":
        expected = sorted(TABLE_I_BRANCHES)
        return {
            "table_id": table_id,
            "row_count": len(rows),
            "supported": True,
            "planting_entrypoint": "scripts/plant_prepared_dossiers.py --table table-i --plant",
            "package_implementation": "scripts/plant_table_i_prepared_dossiers.py",
            "route_map_ref": "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json",
            "expected_dossier_ids": expected,
            "local_docx_ids": local_docx_ids,
            "matched_local_docx_ids": sorted(set(expected) & set(local_docx_ids)),
            "missing_expected_docx_ids": sorted(set(expected) - set(local_docx_ids)),
            "extra_local_docx_ids": sorted(set(local_docx_ids) - set(expected)),
            "ready_to_plant": sorted(local_docx_ids) == expected,
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
        "matched_local_docx_ids": [],
        "missing_expected_docx_ids": [],
        "extra_local_docx_ids": [],
        "ready_to_plant": False,
        "next_route": "add a source-owned dossier id and branch route map before planting this table",
    }


def readiness_payload() -> dict[str, Any]:
    tables = {table_id: table_readiness(table_id) for table_id in ("table-i", "table-ii", "table-iii")}
    return {
        "schema_version": "tos_prepared_dossier_planting_readiness_v1",
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": "ToS/philosophy/atlas/README.md",
        "doc_root": str(DOC_ROOT),
        "local_docx_sections": discover_local_docx_ids(),
        "tables": tables,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Inspect or run prepared philosophy dossier planting.")
    parser.add_argument("--table", choices=["table-i", "table-ii", "table-iii"], help="Target master table package.")
    parser.add_argument("--readiness", action="store_true", help="Print planting readiness JSON and exit.")
    parser.add_argument("--plant", action="store_true", help="Run the supported planting package.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.readiness or not args.plant:
        print(json.dumps(readiness_payload(), ensure_ascii=False, indent=2, sort_keys=True))
        return 0
    if args.table != "table-i":
        raise SystemExit(f"{args.table} is not plantable until a dossier id and branch route map exists")
    return plant_table_i()


if __name__ == "__main__":
    raise SystemExit(main())
