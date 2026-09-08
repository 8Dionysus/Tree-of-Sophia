#!/usr/bin/env python3
"""Resolve one exact branch/backlog route for a planned source; never plant or download."""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]


class PreparationError(ValueError):
    """A proposed route does not return to one exact current owner anchor."""


def _read(root: Path, relative: str) -> tuple[Path, bytes]:
    path = Path(relative)
    resolved = (root / path).resolve()
    if path.is_absolute() or ".." in path.parts or not resolved.is_relative_to(root.resolve()):
        raise PreparationError(f"unsafe owner path: {relative}")
    return path, resolved.read_bytes()


def prepare_anchor(
    root: Path,
    *,
    atlas_row_id: str,
    source_table_index: int,
    source_row_index: int,
    source_label: str,
) -> dict:
    """Return a source-visible anchor snapshot, with no source or semantic admission."""
    if source_table_index < 1 or source_row_index < 1:
        raise PreparationError("source table and row indices must be positive")
    atlas_matches = []
    for path in sorted((root / "ToS/philosophy/atlas/master-tables").glob("*/rows.jsonl")):
        for line, text in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if text.strip() and json.loads(text).get("row_id") == atlas_row_id:
                atlas_matches.append({"path": path.relative_to(root).as_posix(), "line": line,
                    "row_sha256": hashlib.sha256(text.encode()).hexdigest()})
    if len(atlas_matches) != 1:
        raise PreparationError("atlas row must resolve exactly once in current master tables")
    _, manifest_bytes = _read(root, "ToS/philosophy/philosophy.manifest.json")
    manifest = json.loads(manifest_bytes)
    branches = []
    for reference in manifest.get("branch_manifests", []):
        path, content = _read(root, reference)
        branch = json.loads(content)
        if atlas_row_id in branch.get("atlas_rows", []) and branch.get("source_anchor_backlog"):
            if branch.get("path") != path.parent.as_posix():
                raise PreparationError("branch path differs from its canonical manifest")
            branches.append((path, content, branch))
    if len(branches) != 1:
        raise PreparationError("atlas row must have exactly one source-owning branch")
    branch_path, branch_bytes, branch = branches[0]
    backlog_path, backlog_bytes = _read(root, branch["source_anchor_backlog"])
    expected_backlog = branch_path.parent / "sources/source-anchor-backlog.jsonl"
    if backlog_path != expected_backlog:
        raise PreparationError("source backlog must belong to the exact branch")
    anchors = []
    for line, text in enumerate(backlog_bytes.decode("utf-8").splitlines(), 1):
        if not text.strip():
            continue
        row = json.loads(text)
        if row.get("source_table_index") == source_table_index and row.get("source_row_index") == source_row_index:
            if any(row.get(k) != v for k, v in {
                "atlas_row_id": atlas_row_id, "dossier_id": atlas_row_id,
                "branch_path": branch["path"], "source_label": source_label,
            }.items()):
                raise PreparationError("selected backlog row differs from exact atlas, branch or source label")
            anchors.append((line, row, hashlib.sha256(text.encode()).hexdigest()))
    if len(anchors) != 1:
        raise PreparationError("source backlog selector must resolve exactly once")
    line, row, row_digest = anchors[0]
    return {
        "schema_version": "tos_philosophy_source_anchor_preparation_v1",
        "status": "prepared-not-planted",
        "atlas_row_id": atlas_row_id,
        "dossier_id": atlas_row_id,
        "atlas_source": atlas_matches[0],
        "branch_path": branch["path"],
        "branch_manifest": {"path": branch_path.as_posix(), "sha256": hashlib.sha256(branch_bytes).hexdigest()},
        "source_backlog_anchor": {"path": backlog_path.as_posix(), "line": line,
            "source_table_index": source_table_index, "source_row_index": source_row_index, "source_label": source_label},
        "backlog_sha256": hashlib.sha256(backlog_bytes).hexdigest(),
        "backlog_row_sha256": row_digest,
        "source_backlog_record": row,
        "authority_boundary": "exact route preparation only; no source witness, payload, rights, semantics or canon is admitted",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--atlas-row", required=True)
    parser.add_argument("--source-table-index", type=int, required=True)
    parser.add_argument("--source-row-index", type=int, required=True)
    parser.add_argument("--source-label", required=True)
    args = parser.parse_args()
    try:
        payload = prepare_anchor(REPO_ROOT, atlas_row_id=args.atlas_row,
            source_table_index=args.source_table_index, source_row_index=args.source_row_index,
            source_label=args.source_label)
    except (OSError, ValueError, TypeError) as exc:
        print(f"Source planting preparation failed: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(payload, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
