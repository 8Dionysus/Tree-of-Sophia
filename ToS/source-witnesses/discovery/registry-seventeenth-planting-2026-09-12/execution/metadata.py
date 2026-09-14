"""Freeze the pinned Bilara tree and bounded openings for the large AN/SN wave.

The complete source bodies stay with the owner acquisition route.  This stage
retains only exact Git-tree rows and bounded opening prefixes used for identity
and title review.
"""
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import time
from urllib.request import Request, urlopen

ROOT = next(p for p in Path(__file__).resolve().parents if (p / "scripts/acquire_registry_sources.py").is_file())
BATCH_ROOT = ROOT / "ToS/source-witnesses/discovery/registry-seventeenth-planting-2026-09-12"
EVIDENCE = BATCH_ROOT / "evidence"
PIN = "d6d54741b7f2ddfeca82f02c3f95eb3990b4e351"
REPO = "suttacentral/bilara-data"
TREE_SOURCE = Path(os.environ["TOS_BILARA_TREE_SOURCE"]) if os.environ.get("TOS_BILARA_TREE_SOURCE") else BATCH_ROOT / "work" / "bilara-tree.json"
HEADERS = {"User-Agent": "Tree-of-Sophia-source-preparation", "Accept": "application/vnd.github+json"}


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha256(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def capture_opening(row: dict) -> None:
    """Retain one bounded prefix and a matching immutable receipt."""
    name = row["path"] + ".opening"
    target, receipt_path = EVIDENCE / name, EVIDENCE / (name + ".receipt.json")
    expected = min(1536, row["size"] - 1)
    if expected <= 0:
        raise ValueError(f"empty source file cannot provide an opening: {row['path']}")
    url = f"https://raw.githubusercontent.com/{REPO}/{PIN}/{row['upstream_path']}"
    if target.exists() or receipt_path.exists():
        if not target.exists() or not receipt_path.exists():
            raise ValueError(f"incomplete opening pair: {name}")
        body, receipt = target.read_bytes(), json.loads(receipt_path.read_text(encoding="utf-8"))
        if len(body) != expected or receipt.get("url") != url or receipt.get("retained_sha256") != sha256(body):
            raise ValueError(f"opening identity changed: {name}")
        return
    started, tick = now(), time.monotonic()
    with urlopen(Request(url, headers=HEADERS), timeout=45) as response:
        body = response.read(expected)
        status, final_url = response.status, response.url
    if len(body) != expected:
        raise ValueError(f"opening is shorter than requested: {name}")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(body)
    receipt = {
        "url": url, "final_url": final_url, "http_status": status,
        "started_at": started, "ended_at": now(),
        "elapsed_seconds": time.monotonic() - tick,
        "retained_ref": target.relative_to(ROOT).as_posix(),
        "retained_sha256": sha256(body), "retained_byte_size": len(body),
        "corpus_payload_fetched": True,
        "acquisition_scope": "bounded incomplete text opening for UID/title/language assessment; complete source not retained",
    }
    receipt_path.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def row_for(path: str, item: dict, language: str) -> dict:
    parts = path.split("/")
    # root/pli/ms/sutta/{an|sn}/anN/file.json
    district = parts[5]
    return {**item, "upstream_path": path, "language": language, "district": district}


def main() -> None:
    if not TREE_SOURCE.is_file():
        raise ValueError(
            "complete pinned Bilara tree staging is absent; set TOS_BILARA_TREE_SOURCE "
            "or place the staged response under the batch work directory"
        )
    source_bytes = TREE_SOURCE.read_bytes()
    source = json.loads(source_bytes)
    if source.get("sha") != PIN or source.get("truncated"):
        raise ValueError("staged Bilara tree is not the complete pinned response")
    all_rows: list[dict] = []
    for language, prefix, suffix in (
        ("pli", "root/pli/ms/sutta/", "_root-pli-ms.json"),
        ("en", "translation/en/sujato/sutta/", "_translation-en-sujato.json"),
    ):
        for item in source["tree"]:
            path = item.get("path", "")
            if not path.startswith(prefix) or not path.endswith(suffix) or item.get("type") != "blob":
                continue
            tail = path[len(prefix):]
            pieces = tail.split("/")
            if len(pieces) != 3:
                continue
            corpus, district, basename = pieces
            number = int(district[2:]) if district[2:].isdigit() else -1
            if not ((district.startswith("an") and 1 <= number <= 11) or (district.startswith("sn") and 36 <= number <= 56)):
                continue
            all_rows.append(row_for(path, item, language))
    all_rows.sort(key=lambda row: (row["language"], int(row["district"][2:]), row["path"]))
    if len(all_rows) != 4342:
        raise ValueError(f"AN1-11/SN36-56 closure differs: {len(all_rows)}")
    selected = [row for row in all_rows if "-" not in row["path"].rsplit("/", 1)[1].split("_", 1)[0]]
    deferred = [row for row in all_rows if row not in selected]
    if len(selected) != 3994 or len(deferred) != 348:
        raise ValueError(f"individual/grouped closure differs: selected={len(selected)} deferred={len(deferred)}")
    shared = BATCH_ROOT.relative_to(ROOT).as_posix()
    for row in deferred:
        row.update({
            "decision": "deferred",
            "rationale": "Supplied grouped UID spans multiple discourse numbers; the individual Work intake must not absorb this multi-discourse carrier or split the original file. Return to the source-witnesses collection/membership owner before full acquisition.",
            "opening_ref": f"{shared}/evidence/{row['path']}.opening",
            "next_owner": "ToS/source-witnesses collection and evidence-bearing membership design",
            "next_condition": "A reviewed identity design preserving the original aggregate file, constituent discourse identity and exact supplied segment keys.",
        })
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    # Selection-only metadata is derived from the complete pinned response; the
    # complete response remains in the approved host staging area.
    for language in ("pli", "en"):
        rows = [row for row in all_rows if row["language"] == language]
        write_json(EVIDENCE / f"bilara-{language}-selected-tree.json", {
            "sha": PIN,
            "source_url": f"https://api.github.com/repos/{REPO}/git/trees/{PIN}?recursive=1",
            "complete_response_sha256": sha256(source_bytes),
            "selection_only": True,
            "selected_scope": "AN1-11 and SN36-56, all individual and grouped files",
            "tree": rows,
            "truncated": False,
        })
        observed = now()
        receipt = {
            "url": f"https://api.github.com/repos/{REPO}/git/trees/{PIN}?recursive=1",
            "final_url": f"https://api.github.com/repos/{REPO}/git/trees/{PIN}?recursive=1",
            "http_status": 200,
            "started_at": observed, "ended_at": observed, "elapsed_seconds": 0.0,
            "response_sha256": sha256(source_bytes),
            "retained_sha256": sha256((EVIDENCE / f"bilara-{language}-selected-tree.json").read_bytes()),
            "retained_byte_size": (EVIDENCE / f"bilara-{language}-selected-tree.json").stat().st_size,
            "corpus_payload_fetched": False,
            "retained_ref": f"{shared}/evidence/bilara-{language}-selected-tree.json",
            "derivation": "selection-only rows derived from the complete pinned recursive tree retained in approved host staging",
        }
        write_json(EVIDENCE / f"bilara-{language}-selected-tree.json.receipt.json", receipt)
    write_json(BATCH_ROOT / "candidate-files.json", selected)
    write_json(BATCH_ROOT / "deferred-files.json", deferred)
    write_json(BATCH_ROOT / "batch-scope.json", {
        "scope": "Individually identified supplied discourse files in AN1-11 and SN36-56, Pali and English; grouped carriers explicitly deferred",
        "pin": PIN, "selected_works": 1997, "versions": 3994, "files": 3994,
        "payload_bytes": sum(row["size"] for row in selected),
        "pali_bytes": sum(row["size"] for row in selected if row["language"] == "pli"),
        "english_bytes": sum(row["size"] for row in selected if row["language"] == "en"),
        "all_paired_uids": 2171, "all_paired_files": 4342,
        "deferred_grouped_uids": sorted({row["path"].rsplit("/", 1)[1].split("_", 1)[0] for row in deferred}),
        "deferred_files": len(deferred), "deferred_bytes": sum(row["size"] for row in deferred),
        "new_corpus_records_expected": 3994, "new_current_forms_expected": 7988,
    })
    with ThreadPoolExecutor(max_workers=16) as pool:
        list(pool.map(capture_opening, all_rows))
    print(json.dumps({
        "all_files": len(all_rows), "individual_files": len(selected), "deferred_files": len(deferred),
        "individual_bytes": sum(row["size"] for row in selected),
        "deferred_bytes": sum(row["size"] for row in deferred),
        "tree_source_sha256": sha256(source_bytes),
    }, ensure_ascii=False))


if __name__ == "__main__":
    main()
