"""Capture the pinned SN23–35 Bilara trees and bounded source openings.

This preparation helper retains only immutable Git-tree metadata and bounded
opening prefixes. Complete source bodies are acquired later by the generic
owner acquisition route after a preparation checkpoint.
"""
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import time
from urllib.request import Request, urlopen

ROOT = next(p for p in Path(__file__).resolve().parents if (p / "scripts/acquire_registry_sources.py").is_file())
BATCH_ROOT = ROOT / "ToS/source-witnesses/discovery/registry-sixteenth-planting-2026-09-10"
EVIDENCE = BATCH_ROOT / "evidence"
PIN = "d6d54741b7f2ddfeca82f02c3f95eb3990b4e351"
REPO = "suttacentral/bilara-data"
DISTRICTS = range(23, 36)
HEADERS = {"User-Agent": "Tree-of-Sophia-source-preparation", "Accept": "application/vnd.github+json"}


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha256(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def capture(name: str, url: str, *, opening: bool = False, expected_size: int | None = None) -> dict:
    """Capture one immutable response and a receipt, resuming only exact pairs."""
    target = EVIDENCE / name
    receipt_path = EVIDENCE / (name + ".receipt.json")
    if target.exists() or receipt_path.exists():
        if not target.exists() or not receipt_path.exists():
            raise ValueError(f"incomplete evidence pair: {name}")
        body, receipt = target.read_bytes(), json.loads(receipt_path.read_text())
        if receipt.get("url") != url or receipt.get("retained_sha256") != sha256(body):
            raise ValueError(f"evidence identity or digest changed: {name}")
        if expected_size is not None and len(body) != expected_size:
            raise ValueError(f"opening size changed: {name}")
        return json.loads(body) if not opening else receipt
    started, tick = now(), time.monotonic()
    with urlopen(Request(url, headers=HEADERS), timeout=45) as response:
        body = response.read(expected_size if expected_size is not None else 8_000_001)
        status, final_url = response.status, response.url
    if expected_size is not None and len(body) != expected_size:
        raise ValueError(f"bounded opening is shorter than requested: {name}")
    if expected_size is None and len(body) >= 8_000_001:
        raise ValueError(f"metadata response exceeds bounded limit: {name}")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(body)
    ended = now()
    receipt = {
        "url": url,
        "final_url": final_url,
        "http_status": status,
        "started_at": started,
        "ended_at": ended,
        "elapsed_seconds": time.monotonic() - tick,
        "retained_ref": target.relative_to(ROOT).as_posix(),
        "retained_sha256": sha256(body),
        "retained_byte_size": len(body),
        "corpus_payload_fetched": bool(opening),
        "acquisition_scope": (
            "bounded incomplete text opening for UID/title/language assessment; complete source not retained"
            if opening else "complete immutable Git tree metadata"
        ),
    }
    receipt_path.write_bytes((json.dumps(receipt, ensure_ascii=False, indent=2) + "\n").encode())
    return json.loads(body) if not opening else receipt


def tree(name: str, tree_sha: str) -> dict:
    url = f"https://api.github.com/repos/{REPO}/git/trees/{tree_sha}"
    return capture(name, url)


def main() -> None:
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    top = {
        "pli": tree("bilara-sn-tree.json", "7f36c43e1d760e5934fc1a8b6b8af588e7c457e9"),
        "en": tree("bilara-sn-en-tree.json", "04128dd163b1099e8289acebfc0a41ef18b5ac9f"),
    }
    rows: list[dict] = []
    for language, prefix in (("pli", "root/pli/ms"), ("en", "translation/en/sujato")):
        for number in DISTRICTS:
            district = f"sn{number}"
            parent = next((row for row in top[language]["tree"] if row["path"] == district), None)
            if parent is None or parent["type"] != "tree":
                raise ValueError(f"missing pinned district tree: {language}/{district}")
            data = tree(f"bilara-{district}{'-en' if language == 'en' else ''}-tree.json", parent["sha"])
            if data.get("truncated") or any(row["type"] != "blob" for row in data["tree"]):
                raise ValueError(f"district tree is not a complete blob listing: {language}/{district}")
            for item in data["tree"]:
                if item["type"] != "blob" or not item["path"].endswith(".json"):
                    raise ValueError(f"unexpected district entry: {language}/{district}/{item.get('path')}")
                upstream = f"{prefix}/sutta/sn/{district}/{item['path']}"
                rows.append({
                    **item,
                    "upstream_path": upstream,
                    "language": language,
                    "district": district,
                })
    if len(rows) != 756:
        raise ValueError(f"SN23–35 complete tree closure differs: {len(rows)}")
    selected = [row for row in rows if "-" not in row["path"].split("_", 1)[0]]
    deferred = [row for row in rows if row not in selected]
    if len(selected) != 658 or len(deferred) != 98:
        raise ValueError(f"SN23–35 individual/grouped closure differs: selected={len(selected)} deferred={len(deferred)}")
    (BATCH_ROOT / "candidate-files.json").write_bytes((json.dumps(selected, ensure_ascii=False, indent=2) + "\n").encode())
    deferred_records = []
    for row in deferred:
        opening_ref = f"{BATCH_ROOT.relative_to(ROOT).as_posix()}/evidence/{row['path']}.opening"
        deferred_records.append({
            **row,
            "decision": "deferred",
            "rationale": "Supplied grouped UID spans multiple discourse numbers; the individual Work intake must not absorb this multi-discourse carrier or split the original file. Return to the source-witnesses collection/membership owner before full acquisition.",
            "opening_ref": opening_ref,
            "next_owner": "ToS/source-witnesses collection and evidence-bearing membership design",
            "next_condition": "A reviewed identity design preserving the original aggregate file, constituent discourse identity and exact supplied segment keys.",
        })
    (BATCH_ROOT / "deferred-files.json").write_bytes((json.dumps(deferred_records, ensure_ascii=False, indent=2) + "\n").encode())

    def capture_opening(row: dict) -> None:
        size = min(1536, row["size"] - 1)
        if size <= 0:
            raise ValueError(f"empty source file cannot provide an opening: {row['path']}")
        url = f"https://raw.githubusercontent.com/{REPO}/{PIN}/{row['upstream_path']}"
        capture(row["path"] + ".opening", url, opening=True, expected_size=size)

    with ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(capture_opening, rows))
    print(json.dumps({"all_files": len(rows), "selected_files": len(selected), "deferred_files": len(deferred), "selected_bytes": sum(row["size"] for row in selected), "deferred_bytes": sum(row["size"] for row in deferred)}))


if __name__ == "__main__":
    main()
