#!/usr/bin/env python3
"""Prove the complete historical-evidence binding before corpus admission.

The source validator may need exact old repository files while validating a
new batch.  This bounded preflight binds every capture/root pair selected for
the admission, checks the capture and restored bytes, and computes the exact
validator identity before ``CorpusStore`` materializes a candidate.  It also
compares path references in changed metadata with the accepted base so an
acquisition cannot silently introduce a new external history dependency.

This tool performs no materialization, source validation, admission, rights
decision, or publication.  Every historical pair is explicit on the command
line.  Omitting a pair is an error rather than an implicit empty selection.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import mmap
import os
from pathlib import Path
import re
import stat
import sys
from typing import Any, Iterator


HEX64 = re.compile(r"[0-9a-f]{64}\Z")
SCHEMA = "tos_corpus_historical_evidence_preflight_v1"

# These are repository-relative non-ToS paths that may be historical
# validator inputs.  ToS source references are source closure and are handled
# by the ordinary batch converter/validator; payload and provider paths are
# not historical repository evidence.
EXTERNAL_ROOTS = frozenset({
    ".agents", ".github", "access", "docs", "evals", "kag", "manifests",
    "mechanics", "memo", "quests", "scripts", "stats", "tests",
})
_QUOTED_PATH = re.compile(
    rb'"((?:[A-Za-z0-9_.-]+/){1,24}[A-Za-z0-9_.@+(),-]+(?:#[^"\\]*)?)"'
)


class PreflightError(ValueError):
    """The selected evidence cannot satisfy its admission contract."""


def _canonical(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True,
                       separators=(",", ":"), allow_nan=False) + "\n").encode()


def _regular(path: Path, *, label: str) -> os.stat_result:
    path = path.absolute()
    if path != path.resolve() or path.is_symlink():
        raise PreflightError(f"{label} may not be linked: {path}")
    try:
        info = path.lstat()
    except OSError as exc:
        raise PreflightError(f"{label} is not readable: {path}") from exc
    if not stat.S_ISREG(info.st_mode):
        raise PreflightError(f"{label} must be a regular file: {path}")
    return info


def _directory(path: Path, *, label: str) -> None:
    path = path.absolute()
    if path != path.resolve() or path.is_symlink() or not path.is_dir():
        raise PreflightError(f"{label} must be an unlinked directory: {path}")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _load_program(software_root: Path):
    software_root = software_root.absolute()
    scripts = software_root / "scripts"
    _directory(scripts, label="selected software scripts")
    sys.path.insert(0, str(scripts))
    try:
        from corpus_admit import read_batch
        from corpus_archive import verify_capture
        from corpus_source_validation import SourceValidator, is_source_member
        from corpus_store import digest_file, read_json
    except ImportError as exc:
        raise PreflightError(f"cannot load selected admission program: {exc}") from exc
    return read_batch, verify_capture, SourceValidator, is_source_member, digest_file, read_json


def _iter_snapshot_files(snapshot: Path) -> Iterator[dict[str, Any]]:
    """Decode one member at a time from the accepted snapshot's files array."""
    _regular(snapshot, label="accepted base snapshot")
    marker = b'"files":['
    with snapshot.open("rb") as stream:
        mapped = mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ)
        try:
            start = mapped.find(marker)
            if start < 0:
                raise PreflightError("accepted snapshot has no files array")
            position = start + len(marker)
            decoder = json.JSONDecoder()
            while True:
                while mapped[position:position + 1] in (b" ", b"\n", b"\r", b"\t", b","):
                    position += 1
                if mapped[position:position + 1] == b"]":
                    return
                if mapped[position:position + 1] != b"{":
                    raise PreflightError("accepted snapshot files array is malformed")
                end = position
                depth = 0
                in_string = False
                escaped = False
                while end < len(mapped):
                    char = mapped[end]
                    if in_string:
                        if escaped:
                            escaped = False
                        elif char == 0x5C:
                            escaped = True
                        elif char == 0x22:
                            in_string = False
                    elif char == 0x22:
                        in_string = True
                    elif char == 0x7B:
                        depth += 1
                    elif char == 0x7D:
                        depth -= 1
                        if depth == 0:
                            end += 1
                            break
                    end += 1
                try:
                    # ``raw_decode`` counts Unicode characters, not mmap
                    # bytes.  Compare with the decoded slice length so valid
                    # non-ASCII repository paths remain admissible.
                    decoded = mapped[position:end].decode("utf-8")
                    value, consumed = decoder.raw_decode(decoded)
                except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                    raise PreflightError("accepted snapshot has malformed file metadata") from exc
                if consumed != len(decoded) or not isinstance(value, dict):
                    raise PreflightError("accepted snapshot file entry is malformed")
                yield value
                position = end
        finally:
            mapped.close()


def _safe_digest(value: Any, *, label: str) -> str:
    if not isinstance(value, str) or HEX64.fullmatch(value) is None:
        raise PreflightError(f"{label} is not a lowercase SHA-256")
    return value


def _capture_member_rows(capture: Path, *, is_source_member) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    members = capture / "members.jsonl"
    _regular(capture / "capture.json", label="capture manifest")
    _regular(members, label="capture members")
    manifest = json.loads((capture / "capture.json").read_text(encoding="utf-8"))
    if not isinstance(manifest, dict):
        raise PreflightError(f"capture manifest is not an object: {capture}")
    rows: list[dict[str, Any]] = []
    with members.open("r", encoding="utf-8") as stream:
        for number, line in enumerate(stream, 1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise PreflightError(f"capture members line {number} is invalid: {capture}") from exc
            if not isinstance(row, dict):
                raise PreflightError(f"capture member line {number} is not an object: {capture}")
            relative = row.get("path")
            if not isinstance(relative, str):
                raise PreflightError(f"capture member line {number} has no path: {capture}")
            if ("owner-local" not in Path(relative).parts and not is_source_member(relative)
                    and (not relative.startswith("ToS/")
                         or relative.startswith("ToS/derived-exports/")
                         or "payload" in Path(relative).parts)):
                rows.append(row)
    return manifest, rows


def _check_root_rows(root: Path, rows: list[dict[str, Any]], *, label: str,
                     digest_file) -> dict[str, Any]:
    _directory(root, label=label)
    missing: list[str] = []
    mismatches: list[dict[str, Any]] = []
    bytes_total = 0
    for row in rows:
        relative = row.get("path")
        if not isinstance(relative, str) or Path(relative).is_absolute() or ".." in Path(relative).parts:
            raise PreflightError(f"{label} contains unsafe evidence path: {relative!r}")
        target = root / relative
        try:
            info = _regular(target, label=f"{label} member")
        except PreflightError:
            missing.append(relative)
            continue
        actual = {
            "sha256": digest_file(target),
            "size_bytes": info.st_size,
            "mode": stat.S_IMODE(info.st_mode),
        }
        expected = {
            "sha256": row.get("sha256"),
            "size_bytes": row.get("size_bytes"),
            "mode": row.get("mode"),
        }
        bytes_total += info.st_size
        if actual != expected:
            mismatches.append({"path": relative, "expected": expected, "actual": actual})
    return {
        "path": str(root),
        "member_count": len(rows),
        "bytes": bytes_total,
        "missing": sorted(missing),
        "mismatches": mismatches,
        "complete": not missing and not mismatches,
    }


def _external_path(value: str) -> str | None:
    candidate = value.split("#", 1)[0]
    if (candidate.startswith(("ToS/", "payload/", "root/", "translation/", "text/", "http://", "https://"))
            or "://" in candidate or candidate.startswith("/")):
        return None
    parts = candidate.split("/")
    if len(parts) < 2 or parts[0] not in EXTERNAL_ROOTS:
        return None
    if any(part in {"", ".", ".."} for part in parts):
        return None
    if any(ord(char) < 0x20 for char in candidate):
        return None
    return candidate


def _external_refs(path: Path) -> set[str]:
    """Extract quoted repository paths without trusting a metadata schema."""
    if path.suffix not in {".json", ".jsonl"}:
        return set()
    found: set[str] = set()
    # Keep enough bytes to cover a long path split between read blocks.  The
    # carry is deliberately bounded; source paths are normalized repository
    # paths and the regex itself caps their component count.
    carry = b""
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(4 * 1024 * 1024), b""):
            block = carry + block
            for match in _QUOTED_PATH.finditer(block):
                try:
                    value = match.group(1).decode("utf-8")
                except UnicodeDecodeError:
                    continue
                ref = _external_path(value)
                if ref:
                    found.add(ref)
            carry = block[-8192:]
    # A final carried fragment is normally incomplete, but scanning it keeps
    # this helper correct for a file whose last value ends at the boundary.
    for match in _QUOTED_PATH.finditer(carry):
        try:
            value = match.group(1).decode("utf-8")
        except UnicodeDecodeError:
            continue
        ref = _external_path(value)
        if ref:
            found.add(ref)
    return found


def _base_source_path(store: Path, entry: dict[str, Any]) -> Path:
    digest = _safe_digest(entry.get("sha256"), label="accepted object digest")
    path = store / "objects" / digest
    _regular(path, label="accepted source object")
    info = path.stat()
    if info.st_size != entry.get("size_bytes") or _sha256(path) != digest:
        raise PreflightError(f"accepted object does not match snapshot: {entry.get('path')}")
    return path


def _write_exclusive(path: Path, value: dict[str, Any]) -> None:
    path = path.absolute()
    if path.exists() or path.is_symlink():
        raise PreflightError(f"refusing to replace preflight receipt: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("xb") as stream:
        stream.write(_canonical(value))
        stream.flush()
        os.fsync(stream.fileno())
    os.chmod(path, 0o644)


def run_preflight(*, store_root: Path, base_revision: str, batch_path: Path,
                  input_root: Path, grammar_root: Path, software_root: Path,
                  historical_captures: list[Path], historical_roots: list[Path]) -> dict[str, Any]:
    if HEX64.fullmatch(base_revision) is None:
        raise PreflightError("base revision must be a lowercase SHA-256")
    if not historical_captures or len(historical_captures) != len(historical_roots):
        raise PreflightError("every admission must name one historical capture and root pair")
    store_root = store_root.absolute()
    batch_path = batch_path.absolute()
    input_root = input_root.absolute()
    grammar_root = grammar_root.absolute()
    software_root = software_root.absolute()
    (read_batch, verify_capture, SourceValidator, is_source_member,
     digest_file, read_json) = _load_program(software_root)
    _regular(batch_path, label="source batch")
    batch, updates, retirements = read_batch(batch_path, input_root)
    if retirements:
        raise PreflightError("historical preflight currently requires a non-retirement batch")
    if batch.get("base_revision") != base_revision:
        raise PreflightError("batch base revision differs from requested base")
    pointer_path = store_root / "current.json"
    pointer = read_json(pointer_path)
    pointer_current = pointer.get("current")
    pointer_matches = pointer_current == base_revision
    snapshot = store_root / "revisions" / base_revision / "snapshot.json"
    snapshot_info = _regular(snapshot, label="accepted base snapshot")
    base_entries: dict[str, dict[str, Any]] = {}
    scanned = 0
    for entry in _iter_snapshot_files(snapshot):
        scanned += 1
        relative = entry.get("path")
        if not isinstance(relative, str):
            raise PreflightError("accepted snapshot entry has no path")
        if relative in base_entries:
            raise PreflightError(f"accepted snapshot repeats path: {relative}")
        base_entries[relative] = entry

    capture_results: list[dict[str, Any]] = []
    all_rows: list[dict[str, Any]] = []
    for index, (capture, root) in enumerate(zip(historical_captures, historical_roots)):
        capture = capture.absolute()
        root = root.absolute()
        manifest = verify_capture(capture)
        selected_manifest, rows = _capture_member_rows(capture, is_source_member=is_source_member)
        if selected_manifest != manifest:
            raise PreflightError(f"capture manifest changed during verification: {capture}")
        root_result = _check_root_rows(root, rows, label=f"historical root {index}", digest_file=digest_file)
        capture_json = capture / "capture.json"
        capture_results.append({
            "index": index,
            "capture": str(capture),
            "root": str(root),
            "capture_json_sha256": _sha256(capture_json),
            "members_jsonl_sha256": manifest.get("members_sha256"),
            "archive_sha256": manifest.get("archive_sha256"),
            "archive_size_bytes": manifest.get("archive_size_bytes"),
            "source_git_commit": manifest.get("source_git_commit"),
            "manifest_member_count": manifest.get("member_count"),
            "manifest_source_bytes": manifest.get("source_bytes"),
            "selected_evidence_members": len(rows),
            "selected_evidence_bytes": sum(int(row.get("size_bytes", 0)) for row in rows),
            "root_verification": root_result,
        })
        all_rows.extend(rows)

    # Construct the exact identity the admission unit will use.  This repeats
    # the capture verification inside SourceValidator by design: the receipt
    # binds the program's own constructor, not a parallel reimplementation.
    validator = SourceValidator(
        grammar_root,
        historical_capture=[item.absolute() for item in historical_captures],
        historical_root=[item.absolute() for item in historical_roots],
    )
    validator_sha = validator.sha256
    validator_evidence = list(validator.evidence)
    evidence_by_path = {row["path"]: row for row in validator_evidence}
    if len(evidence_by_path) != len(validator_evidence):
        raise PreflightError("selected historical captures overlap")
    selected_paths = {row.get("path") for row in all_rows}
    if selected_paths != set(evidence_by_path):
        raise PreflightError("capture selection differs from SourceValidator evidence selection")

    roots_by_path = getattr(validator, "_evidence_roots", {})
    roots_by_path = {path: str(root) for path, root in roots_by_path.items()}
    by_root: dict[str, dict[str, int]] = {}
    for row in validator_evidence:
        root = roots_by_path.get(row["path"])
        if root is None:
            raise PreflightError(f"validator did not bind an evidence root: {row['path']}")
        stats = by_root.setdefault(root, {"member_count": 0, "bytes": 0})
        stats["member_count"] += 1
        stats["bytes"] += int(row["size_bytes"])
    for item in capture_results:
        stats = by_root.get(item["root"], {"member_count": 0, "bytes": 0})
        item["validator_evidence_members"] = stats["member_count"]
        item["validator_evidence_bytes"] = stats["bytes"]
        if stats["member_count"] != item["selected_evidence_members"]:
            raise PreflightError(f"validator evidence count differs for {item['root']}")

    # Compare only changed metadata files.  The accepted object is immutable;
    # reading it by digest makes the comparison independent of any checkout.
    added_external: dict[str, list[str]] = {}
    removed_external: dict[str, list[str]] = {}
    base_external_total: set[str] = set()
    candidate_external_total: set[str] = set()
    for relative, update in sorted(updates.items()):
        candidate_path = update["source"]
        candidate_refs = _external_refs(candidate_path)
        base_refs: set[str] = set()
        previous = base_entries.get(relative)
        if previous is not None:
            base_path = _base_source_path(store_root, previous)
            base_refs = _external_refs(base_path)
        added = sorted(candidate_refs - base_refs)
        removed = sorted(base_refs - candidate_refs)
        if added:
            added_external[relative] = added
        if removed:
            removed_external[relative] = removed
        base_external_total.update(base_refs)
        candidate_external_total.update(candidate_refs)

    evidence_paths = set(evidence_by_path)
    added_refs = sorted(set().union(*(set(rows) for rows in added_external.values())) if added_external else set())
    uncovered = sorted(ref for ref in added_refs if ref not in evidence_paths)
    issues: list[str] = []
    if not pointer_matches:
        issues.append("accepted pointer does not equal requested base")
    if any(not item["root_verification"]["complete"] for item in capture_results):
        issues.append("historical evidence root is incomplete or changed")
    if validator.sha256 != batch.get("validator_sha256"):
        issues.append("batch validator identity does not match selected software and evidence")
    if uncovered:
        issues.append("changed metadata adds external references outside selected historical evidence")

    return {
        "schema_version": SCHEMA,
        "ok": not issues,
        "issues": issues,
        "base": {
            "revision": base_revision,
            "pointer_path": str(pointer_path),
            "pointer_current": pointer_current,
            "pointer_matches": pointer_matches,
            "snapshot_path": str(snapshot),
            "snapshot_size_bytes": snapshot_info.st_size,
            "snapshot_members_scanned": scanned,
        },
        "batch": {
            "path": str(batch_path),
            "sha256": _sha256(batch_path),
            "schema_version": batch["schema_version"],
            "base_revision": batch["base_revision"],
            "validator_sha256_declared": batch["validator_sha256"],
            "update_count": len(updates),
            "retirement_count": len(retirements),
            "source_bytes": sum(int(row["size_bytes"]) for row in updates.values()),
        },
        "software_binding": {
            "software_root": str(software_root),
            "validator_sha256_actual": validator_sha,
            "grammar_sha256": getattr(validator, "grammar_sha256", None),
            "validator_evidence_members": len(validator_evidence),
            "validator_evidence_bytes": sum(int(row["size_bytes"]) for row in validator_evidence),
        },
        "historical_selection": {
            "pair_count": len(capture_results),
            "pairs": capture_results,
            "complete_capture_root_binding": all(item["root_verification"]["complete"] for item in capture_results),
            "evidence_member_count": len(validator_evidence),
            "evidence_bytes": sum(int(row["size_bytes"]) for row in validator_evidence),
        },
        "metadata_external_reference_comparison": {
            "changed_files_scanned": len(updates),
            "files_with_added_refs": added_external,
            "files_with_removed_refs": removed_external,
            "base_external_refs": sorted(base_external_total),
            "candidate_external_refs": sorted(candidate_external_total),
            "added_external_refs": added_refs,
            "added_refs_covered_by_selected_evidence": sorted(ref for ref in added_refs if ref in evidence_paths),
            "added_refs_without_selected_evidence": uncovered,
            "new_metadata_adds_uncovered_external_refs": bool(uncovered),
        },
        "materialization_performed": False,
        "full_validation_performed": False,
        "admission_performed": False,
        "rights_or_publication_change": False,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--store", type=Path, required=True)
    parser.add_argument("--base-revision", required=True)
    parser.add_argument("--batch", type=Path, required=True)
    parser.add_argument("--input-root", type=Path, required=True)
    parser.add_argument("--grammar-root", type=Path, required=True)
    parser.add_argument("--software-root", type=Path, required=True)
    parser.add_argument("--historical-capture", type=Path, action="append", required=True)
    parser.add_argument("--historical-root", type=Path, action="append", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    try:
        result = run_preflight(
            store_root=args.store,
            base_revision=args.base_revision,
            batch_path=args.batch,
            input_root=args.input_root,
            grammar_root=args.grammar_root,
            software_root=args.software_root,
            historical_captures=args.historical_capture,
            historical_roots=args.historical_root,
        )
    except Exception as exc:
        result = {
            "schema_version": SCHEMA,
            "ok": False,
            "issues": [f"{type(exc).__name__}: {exc}"],
            "materialization_performed": False,
            "full_validation_performed": False,
            "admission_performed": False,
            "rights_or_publication_change": False,
        }
        status = 2
    else:
        status = 0 if result["ok"] else 1
    rendered = _canonical(result)
    if args.output:
        _write_exclusive(args.output, result)
    sys.stdout.buffer.write(rendered)
    return status


if __name__ == "__main__":
    raise SystemExit(main())
