"""Acquire a large prepared registry batch with one bounded topology refresh.

Each target still goes through the generic owner acquisition function. The
topology event is a derived aggregate, so refreshing and snapshotting it once
after the complete claim set avoids one multi-megabyte preimage per Item.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import sys

ROOT = next(p for p in Path(__file__).resolve().parents if (p / "scripts/acquire_registry_sources.py").is_file())
sys.path.insert(0, str(ROOT / "scripts"))
import acquire_registry_sources as owner


def utcnow() -> str:
    return datetime.now(timezone.utc).isoformat()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--preparation-receipt", required=True, type=Path)
    args = parser.parse_args()
    manifest_path = args.manifest.resolve()
    preparation, packages = owner.load_preparation(ROOT, manifest_path)
    owner.check_preparation_receipt(ROOT, manifest_path, args.preparation_receipt.resolve())
    targets = preparation["targets"]
    owner.preflight_identities(ROOT, targets, packages)
    evidence_root = manifest_path.parent
    original_refresh = owner.refresh_topology
    deferred_calls = 0

    def defer_refresh(*_args, **_kwargs):
        nonlocal deferred_calls
        deferred_calls += 1

    owner.refresh_topology = defer_refresh
    completed = []
    try:
        for index, target in enumerate(targets, 1):
            result = owner.install_target(ROOT, manifest_path, preparation, target, packages[target["slug"]])
            completed.append(result)
            if index % 100 == 0 or index == len(targets):
                print(json.dumps({"progress": index, "total": len(targets), "target": target["slug"], "status": result["status"]}), flush=True)
    finally:
        owner.refresh_topology = original_refresh
    original_refresh(ROOT, evidence_root, utcnow())
    result = {
        "schema_version": "tos_registry_batch_acquisition_receipt_v1",
        "completed_at": utcnow(),
        "manifest_ref": manifest_path.relative_to(ROOT).as_posix(),
        "manifest_sha256": owner.sha256(manifest_path.read_bytes()),
        "target_count": len(completed),
        "target_files": sum(value["files"] for value in completed),
        "target_bytes": sum(value["bytes"] for value in completed),
        "deferred_topology_refresh_calls": deferred_calls,
        "topology_refresh_count": 1,
        "topology_snapshot_posture": "one preimage before final aggregate refresh; per-target owner records, claims, Items and provenance were written through the generic acquisition route",
        "source_text_admitted": False,
        "semantic_alignment_admitted": False,
        "rights_or_publication_expanded": False,
    }
    (evidence_root / "batch-acquisition-receipt.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
