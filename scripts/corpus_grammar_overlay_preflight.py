#!/usr/bin/env python3
"""Check a source batch's grammar overlay before corpus materialization.

The source admission validator must compare its selected grammar with the
grammar already present in the accepted base revision.  This bounded check
does that comparison from the base snapshot's file metadata and the batch's
explicit updates, before ``CorpusStore`` materializes the full candidate.
It does not admit data, render catalogs, or perform semantic validation.

The selected software root is explicit so that the receipt binds the exact
validator implementation used by the eventual admission unit.  The current
validator's JSON grammar identity includes only the two schema namespaces;
document companions are reported separately as source-closure evidence.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import mmap
import os
import re
import stat
import sys
from pathlib import Path
from typing import Any, Iterator


GRAMMAR_PREFIXES = (
    "ToS/contracts/",
    "ToS/doctrine/semantic-interchange/",
)
DOCUMENTATION_COMPANIONS = {
    "ToS/contracts/CORPUS_RELEASE.md",
}
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
RECEIPT_SCHEMA = "tos_corpus_grammar_overlay_preflight_v1"


class PreflightError(ValueError):
    """A bounded preflight check could not establish its input contract."""


def _canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, ensure_ascii=False,
                       separators=(",", ":"), allow_nan=False) + "\n").encode()


def _regular(path: Path, *, label: str) -> os.stat_result:
    absolute = path.absolute()
    if absolute != path.resolve():
        raise PreflightError(f"{label} may not be linked: {path}")
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode):
        raise PreflightError(f"{label} must be a regular file: {path}")
    return info


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _is_grammar_path(relative: str) -> bool:
    return relative.endswith(".json") and relative.startswith(GRAMMAR_PREFIXES)


def _snapshot_files(snapshot: Path) -> Iterator[dict[str, Any]]:
    """Stream one object at a time from the large canonical files array."""

    _regular(snapshot, label="accepted snapshot")
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
                    # bytes.  Use the decoded slice length for non-ASCII
                    # repository paths.
                    decoded = mapped[position:end].decode("utf-8")
                    value, consumed = decoder.raw_decode(decoded)
                except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                    raise PreflightError("accepted snapshot contains malformed file metadata") from exc
                if consumed != len(decoded) or not isinstance(value, dict):
                    raise PreflightError("accepted snapshot file entry is malformed")
                yield value
                position = end
        finally:
            mapped.close()


def _row(entry: dict[str, Any], *, label: str) -> dict[str, Any]:
    path = entry.get("path")
    digest = entry.get("sha256")
    size = entry.get("size_bytes")
    mode = entry.get("mode")
    if (not isinstance(path, str) or not isinstance(digest, str) or not HEX64.fullmatch(digest)
            or type(size) is not int or size < 0 or type(mode) is not int):
        raise PreflightError(f"{label} has invalid source file metadata")
    return {"sha256": digest, "size_bytes": size, "mode": mode}


def _grammar_root_files(grammar_root: Path) -> dict[str, dict[str, Any]]:
    """Read actual validator grammar members without touching other source."""

    grammar_root = grammar_root.absolute()
    if grammar_root != grammar_root.resolve() or not grammar_root.is_dir():
        raise PreflightError(f"grammar root must be an explicit regular directory: {grammar_root}")
    actual: dict[str, dict[str, Any]] = {}
    for prefix in GRAMMAR_PREFIXES:
        base = grammar_root / prefix.rstrip("/")
        if base.is_symlink() or not base.is_dir():
            raise PreflightError(f"grammar namespace is missing or linked: {prefix}")
        for directory, directories, filenames in os.walk(base, followlinks=False):
            directory_path = Path(directory)
            if directory_path.is_symlink() or directory_path.resolve() != directory_path.absolute():
                raise PreflightError(f"grammar directory is linked: {directory_path}")
            directories.sort()
            for filename in sorted(filenames):
                path = directory_path / filename
                if path.suffix != ".json":
                    continue
                info = _regular(path, label="grammar member")
                relative = path.relative_to(grammar_root).as_posix()
                if relative in actual:
                    raise PreflightError(f"duplicate grammar member: {relative}")
                actual[relative] = {
                    "sha256": _sha256(path),
                    "size_bytes": info.st_size,
                    "mode": stat.S_IMODE(info.st_mode),
                }
    return actual


def _load_program(software_root: Path):
    """Import the selected admission reader and validator from one worktree."""

    software_root = software_root.absolute()
    scripts = software_root / "scripts"
    if (software_root != software_root.resolve() or scripts != scripts.resolve()
            or not scripts.is_dir()):
        raise PreflightError(f"software root must contain an explicit scripts directory: {software_root}")
    sys.path.insert(0, str(scripts))
    try:
        from corpus_admit import read_batch
        from corpus_source_validation import SourceValidator
        from corpus_store import CorpusStoreError, read_json
    except ImportError as exc:
        raise PreflightError(f"selected software root cannot load corpus admission modules: {exc}") from exc
    return read_batch, SourceValidator, CorpusStoreError, read_json


def _write_new(path: Path, value: dict[str, Any]) -> None:
    path = path.absolute()
    if path.exists() or path.is_symlink():
        raise PreflightError(f"refusing to replace preflight receipt: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("xb") as stream:
        stream.write(_canonical(value))
        stream.flush()
        os.fsync(stream.fileno())
    os.chmod(path, 0o644)


def run_preflight(
    *,
    store_root: Path,
    base_revision: str,
    batch_path: Path,
    input_root: Path,
    grammar_root: Path,
    software_root: Path,
    historical_captures: list[Path] | None = None,
    historical_roots: list[Path] | None = None,
) -> dict[str, Any]:
    if not HEX64.fullmatch(base_revision):
        raise PreflightError("base revision must be a lowercase SHA-256 revision")
    if (historical_captures is None) != (historical_roots is None):
        raise PreflightError(
            "historical evidence requires one capture and restored root for every pack"
        )
    if historical_captures is not None and len(historical_captures) != len(historical_roots):
        raise PreflightError(
            "historical evidence selection must contain paired capture/root paths"
        )
    if not historical_captures:
        historical_captures = None
        historical_roots = None
    store_root = store_root.absolute()
    batch_path = batch_path.absolute()
    input_root = input_root.absolute()
    grammar_root = grammar_root.absolute()
    software_root = software_root.absolute()
    read_batch, SourceValidator, _CorpusStoreError, read_json = _load_program(software_root)
    _regular(batch_path, label="source batch")
    batch, updates, retirements = read_batch(batch_path, input_root)
    if batch.get("base_revision") != base_revision:
        raise PreflightError("batch base revision differs from the requested base")

    pointer_path = store_root / "current.json"
    pointer = read_json(pointer_path)
    current_revision = pointer.get("current")
    pointer_matches = current_revision == base_revision
    snapshot = store_root / "revisions" / base_revision / "snapshot.json"
    snapshot_info = _regular(snapshot, label="accepted base snapshot")

    accepted_grammar: dict[str, dict[str, Any]] = {}
    accepted_companions: dict[str, dict[str, Any]] = {}
    snapshot_member_count = 0
    for entry in _snapshot_files(snapshot):
        snapshot_member_count += 1
        relative = entry.get("path")
        if not isinstance(relative, str):
            raise PreflightError("accepted snapshot file entry has no path")
        record = _row(entry, label="accepted snapshot file entry")
        if _is_grammar_path(relative):
            if relative in accepted_grammar:
                raise PreflightError(f"accepted snapshot repeats grammar member: {relative}")
            accepted_grammar[relative] = record
        elif relative in DOCUMENTATION_COMPANIONS:
            if relative in accepted_companions:
                raise PreflightError(f"accepted snapshot repeats documentation companion: {relative}")
            accepted_companions[relative] = record

    overlay = dict(accepted_grammar)
    retired_grammar: list[str] = []
    for relative in sorted(retirements):
        if _is_grammar_path(relative):
            retired_grammar.append(relative)
            overlay.pop(relative, None)
    grammar_updates: dict[str, dict[str, Any]] = {}
    companion_updates: dict[str, dict[str, Any]] = {}
    update_digest_checks: list[dict[str, Any]] = []
    for relative, update in sorted(updates.items()):
        if _is_grammar_path(relative):
            grammar_updates[relative] = {
                key: update[key] for key in ("sha256", "size_bytes", "mode")
            }
            overlay[relative] = grammar_updates[relative]
        elif relative in DOCUMENTATION_COMPANIONS:
            companion_updates[relative] = {
                key: update[key] for key in ("sha256", "size_bytes", "mode")
            }

    for relative, expected in sorted({**grammar_updates, **companion_updates}.items()):
        source = updates[relative]["source"]
        info = _regular(source, label="source update")
        actual = {
            "sha256": _sha256(source),
            "size_bytes": info.st_size,
            "mode": stat.S_IMODE(info.st_mode),
        }
        update_digest_checks.append({
            "path": relative,
            "expected": expected,
            "actual": actual,
            "matches": actual == expected,
        })

    actual_grammar = _grammar_root_files(grammar_root)
    missing = sorted(set(overlay) - set(actual_grammar))
    unexpected = sorted(set(actual_grammar) - set(overlay))
    digest_mismatches = []
    size_mismatches = []
    mode_mismatches = []
    for relative in sorted(set(overlay) & set(actual_grammar)):
        expected = overlay[relative]
        actual = actual_grammar[relative]
        if actual["sha256"] != expected["sha256"]:
            digest_mismatches.append({"path": relative, "expected": expected["sha256"], "actual": actual["sha256"]})
        if actual["size_bytes"] != expected["size_bytes"]:
            size_mismatches.append({"path": relative, "expected": expected["size_bytes"], "actual": actual["size_bytes"]})
        if actual["mode"] != expected["mode"]:
            mode_mismatches.append({"path": relative, "expected": expected["mode"], "actual": actual["mode"]})

    validator_options = {}
    if historical_captures is not None:
        validator_options.update(
            historical_capture=[path.absolute() for path in historical_captures],
            historical_root=[path.absolute() for path in historical_roots],
        )
    validator = SourceValidator(grammar_root, **validator_options)
    batch_validator = batch["validator_sha256"]
    identity_match = validator.sha256 == batch_validator
    grammar_identity = getattr(validator, "grammar_sha256", None)
    update_checks_ok = all(item["matches"] for item in update_digest_checks)
    issues = []
    if not pointer_matches:
        issues.append("accepted pointer does not equal requested base")
    if missing:
        issues.append("grammar overlay has missing current members")
    if unexpected:
        issues.append("current grammar has members absent from base-plus-batch overlay")
    if digest_mismatches:
        issues.append("grammar overlay has digest mismatches")
    if size_mismatches:
        issues.append("grammar overlay has size mismatches")
    if mode_mismatches:
        issues.append("grammar overlay has mode mismatches")
    if not update_checks_ok:
        issues.append("grammar/documentation update bytes do not match their batch rows")
    if not identity_match:
        issues.append("batch validator identity does not match selected software and grammar")

    return {
        "schema_version": RECEIPT_SCHEMA,
        "ok": not issues,
        "issues": issues,
        "base_revision": base_revision,
        "accepted_pointer": {
            "path": str(pointer_path),
            "current": current_revision,
            "matches_requested_base": pointer_matches,
        },
        "accepted_snapshot": {
            "path": str(snapshot),
            "size_bytes": snapshot_info.st_size,
            "member_count_scanned": snapshot_member_count,
            "grammar_member_count": len(accepted_grammar),
            "documentation_companion_count": len(accepted_companions),
        },
        "batch": {
            "path": str(batch_path),
            "sha256": _sha256(batch_path),
            "schema_version": batch["schema_version"],
            "update_count": len(updates),
            "retirement_count": len(retirements),
            "grammar_update_count": len(grammar_updates),
            "grammar_update_paths": sorted(grammar_updates),
            "documentation_companion_update_count": len(companion_updates),
            "documentation_companion_update_paths": sorted(companion_updates),
            "retired_grammar_paths": retired_grammar,
            "validator_sha256": batch_validator,
        },
        "software_binding": {
            "software_root": str(software_root),
            "validator_sha256_actual": validator.sha256,
            "grammar_identity_actual": grammar_identity,
            "validator_identity_matches_batch": identity_match,
            "historical_captures": [str(path.absolute()) for path in historical_captures or []],
            "historical_roots": [str(path.absolute()) for path in historical_roots or []],
            "historical_selection_explicit": historical_captures is not None,
            "historical_evidence_member_count": len(getattr(validator, "evidence", [])),
        },
        "overlay": {
            "accepted_grammar_members": len(accepted_grammar),
            "batch_grammar_overrides": len(grammar_updates),
            "expected_grammar_members": len(overlay),
            "actual_grammar_members": len(actual_grammar),
            "missing": missing,
            "unexpected": unexpected,
            "digest_mismatches": digest_mismatches,
            "size_mismatches": size_mismatches,
            "mode_mismatches": mode_mismatches,
            "update_digest_checks": update_digest_checks,
        },
        "documentation_companions": {
            "paths": sorted(DOCUMENTATION_COMPANIONS),
            "base_members": accepted_companions,
            "batch_updates": companion_updates,
            "identity_bound": False,
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
    parser.add_argument("--historical-capture", type=Path, action="append")
    parser.add_argument("--historical-root", type=Path, action="append")
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
    except Exception as exc:  # receipt the exact bounded failure for review
        result = {
            "schema_version": RECEIPT_SCHEMA,
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
    if args.output:
        _write_new(args.output, result)
    print(_canonical(result).decode(), end="")
    return status


if __name__ == "__main__":
    raise SystemExit(main())
