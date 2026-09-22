#!/usr/bin/env python3
"""Convert the seven local acquisition packages into one corpus batch.

The acquisition trees are custody evidence, not a corpus input root.  This
converter selects the named prepared package from each exact batch, binds the
acquired metadata and claims to their on-disk final forms, carries only the
referenced discovery closure, and creates a hard-link payload view with the
``source_payload_custody`` layout.  It never changes rights, publication,
semantic review, canon, or the accepted corpus pointer.

The output is an input package for ``corpus_admit.py``.  The historical seven
packages and the newer acquired-not-admitted handoff selector both feed one
atomic ``tos_corpus_batch_v1`` transaction so a dependency across inputs
cannot be admitted ahead of the source it names.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
from dataclasses import dataclass
import hashlib
import json
import mmap
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
from typing import Any, Iterable, Iterator


SOFTWARE_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BATCH_SELECTION = (
    (
        "t2-21-r065-consolatio-philosophiae-20260914",
        "registry-t2-21-2026-09-14",
        "ToS/source-witnesses/discovery/registry-t2-21-2026-09-14/prepared-source-packages.jsonl",
    ),
    (
        "r22-sixteenth-sn23-35-english-20260914",
        "registry-sixteenth-translation-r22-2026-09-14",
        "ToS/source-witnesses/discovery/registry-sixteenth-translation-r22-2026-09-14/prepared-source-packages.jsonl",
    ),
    (
        "r23-khuddaka-jataka-20260914",
        "registry-khuddaka-jataka-2026-09-15",
        "ToS/source-witnesses/discovery/registry-khuddaka-jataka-2026-09-15/prepared-source-packages.jsonl",
    ),
    (
        "r24-therapadana-20260914",
        "registry-therapadana-2026-09-15",
        "ToS/source-witnesses/discovery/registry-therapadana-2026-09-15/prepared-source-packages.jsonl",
    ),
    (
        "r25-khuddaka-minor-dedup-corrected-20260915",
        "registry-khuddaka-minor-2026-09-15",
        "ToS/source-witnesses/discovery/registry-khuddaka-minor-2026-09-15/prepared-source-packages.jsonl",
    ),
    (
        "r26-serbian-brankokovacevic-20260915",
        "registry-serbian-brankokovacevic-2026-09-15",
        "ToS/source-witnesses/discovery/registry-serbian-brankokovacevic-2026-09-15/prepared-source-packages.jsonl",
    ),
    (
        "r27-german-sabbamitta-an4-an5-20260915",
        "registry-german-sabbamitta-an4-an5-2026-09-15",
        "ToS/source-witnesses/discovery/registry-german-sabbamitta-an4-an5-2026-09-15/prepared-source-packages.jsonl",
    ),
)

BATCH_SCHEMA = "tos_corpus_batch_v1"
HANDOFF_SELECTION_SCHEMA = "tos_acquired_handoff_selection_v1"
HANDOFF_SCHEMA = "tos_acquisition_handoff_v1"
CLAIM_SUFFIXES = {
    "work-expression-claims.jsonl": "work-expression",
    "expression-edition-claims.jsonl": "expression-edition",
    "edition-item-claims.jsonl": "edition-item",
}
TOPOLOGY_EVENT_PATH = "ToS/source-witnesses/relations/provenance.jsonl"
TOPOLOGY_RELATION_ROUTES = {
    "work-expression-claims.jsonl": (
        "expression_claim_refs",
        "work.json",
        "work_expression_claims_materialized",
        "unreviewed-work-expression-topology-claims",
    ),
    "expression-edition-claims.jsonl": (
        "embodiment_claim_refs",
        "expression.json",
        "expression_edition_claims_materialized",
        "unreviewed-expression-edition-topology-claims",
    ),
    "edition-item-claims.jsonl": (
        "exemplar_claim_refs",
        "edition.json",
        "edition_item_claims_materialized",
        "unreviewed-edition-item-topology-claims",
    ),
}
SOURCE_PREFIX = "ToS/"
_HEX64 = re.compile(r"[0-9a-f]{64}\Z")
_REF = re.compile(r"^ToS/[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*$")
_FIXITY = re.compile(r"^([0-9a-f]{64})  (payload/[A-Za-z0-9._-]+)$")


class ConversionError(ValueError):
    """The selected acquisition evidence cannot form a closed candidate."""


@dataclass(frozen=True)
class BatchSpec:
    batch_id: str
    registry: str
    package_ref: str
    package_sha256: str | None = None


@dataclass(frozen=True)
class SourceFile:
    relative: str
    path: Path
    batch_id: str


@dataclass(frozen=True)
class DirectHandoff:
    """One immutable acquisition handoff normalized for the closure pass.

    The acquisition route owns the handoff and its custody evidence.  This
    record deliberately keeps that evidence separate from the corpus batch
    which this module assembles after topology closure.
    """

    root: Path
    handoff_ref: str
    handoff_sha256: str
    handoff_path: Path
    handoff: dict[str, Any]
    manifest_path: Path
    manifest: dict[str, Any]
    source_root: Path
    payload_root: Path
    source_records: list[dict[str, Any]]
    payloads: list[dict[str, Any]]
    context: dict[str, Any] | None


def _canonical(value: Any) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            )
            + "\n"
        ).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise ConversionError(f"cannot render canonical JSON: {exc}") from exc


def _read_json(path: Path, *, label: str) -> dict[str, Any]:
    try:
        raw = path.read_bytes()
        value = json.loads(raw.decode("utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ConversionError(f"{label} is not readable JSON: {path}") from exc
    if not isinstance(value, dict):
        raise ConversionError(f"{label} must be a JSON object: {path}")
    return value


def _read_jsonl(path: Path, *, label: str) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    try:
        stream = path.open("r", encoding="utf-8")
        with stream:
            for line_number, line in enumerate(stream, 1):
                if not line.strip():
                    continue
                try:
                    value = json.loads(line)
                except json.JSONDecodeError as exc:
                    raise ConversionError(
                        f"{label} has invalid JSON at line {line_number}: {path}"
                    ) from exc
                if not isinstance(value, dict):
                    raise ConversionError(
                        f"{label} line {line_number} is not an object: {path}"
                    )
                rows.append(value)
    except OSError as exc:
        raise ConversionError(f"cannot read {label}: {path}") from exc
    return rows


def _load_batch_selection(path: Path | None) -> tuple[list[BatchSpec], dict[str, Any]]:
    """Load an exact batch/package selection, retaining a default fixture.

    A future acquisition handoff can provide this small JSON object instead of
    changing this converter.  Package SHA-256 values are optional on input and
    are filled from the immutable acquisition package when the candidate is
    assembled; if supplied they are checked before any source is selected.
    """

    if path is None:
        raw: Any = {
            "schema_version": "tos_acquired_batch_selection_v1",
            "selection_id": "queued-seven-20260922",
            "batches": [
                {"batch_id": batch_id, "registry": registry, "package_ref": package_ref}
                for batch_id, registry, package_ref in DEFAULT_BATCH_SELECTION
            ],
        }
        source = "default-seven-migration-fixture"
    else:
        path = path.absolute()
        raw = _read_json(path, label="batch selection")
        source = str(path)
    if not isinstance(raw, dict) or raw.get("schema_version") != "tos_acquired_batch_selection_v1":
        raise ConversionError("batch selection has an unexpected schema_version")
    selection_id = raw.get("selection_id")
    rows = raw.get("batches")
    if not isinstance(selection_id, str) or not re.fullmatch(r"[A-Za-z0-9._-]+", selection_id):
        raise ConversionError("batch selection has an invalid selection_id")
    if not isinstance(rows, list) or not rows:
        raise ConversionError("batch selection must contain at least one batch")
    specs: list[BatchSpec] = []
    seen_batch_ids: set[str] = set()
    seen_packages: set[str] = set()
    for row in rows:
        if not isinstance(row, dict):
            raise ConversionError("batch selection rows must be objects")
        batch_id, registry, package_ref = row.get("batch_id"), row.get("registry"), row.get("package_ref")
        package_sha256 = row.get("package_sha256")
        if (
            not isinstance(batch_id, str)
            or not re.fullmatch(r"[A-Za-z0-9._-]+", batch_id)
            or not isinstance(registry, str)
            or not re.fullmatch(r"[A-Za-z0-9._-]+", registry)
            or not isinstance(package_ref, str)
            or not package_ref.startswith("ToS/")
            or "/topology-before/" in package_ref
            or "/work-before/" in package_ref
            or not package_ref.endswith("/prepared-source-packages.jsonl")
            or (package_sha256 is not None and (not isinstance(package_sha256, str) or _HEX64.fullmatch(package_sha256) is None))
        ):
            raise ConversionError("batch selection row has invalid exact package binding")
        if batch_id in seen_batch_ids or package_ref in seen_packages:
            raise ConversionError("batch selection repeats a batch or package")
        seen_batch_ids.add(batch_id)
        seen_packages.add(package_ref)
        specs.append(BatchSpec(batch_id, registry, package_ref, package_sha256))
    return specs, {"schema_version": raw["schema_version"], "selection_id": selection_id, "source": source, "batches": specs}


def _safe_handoff_ref(value: object, *, label: str) -> str:
    if not isinstance(value, str) or not value or "\\" in value or "\x00" in value:
        raise ConversionError(f"invalid {label}")
    parsed = PurePosixPath(value)
    if parsed.is_absolute() or str(parsed) != value or any(
        part in {"", ".", ".."} for part in parsed.parts
    ):
        raise ConversionError(f"unsafe {label}: {value}")
    return value


def _load_handoff_selection(path: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Load explicit handoff roots for one aggregated closure transaction.

    This is an invocation selector, not a durable acquisition registry.  Each
    row binds the immutable handoff bytes so a stale or silently replaced
    producer output is rejected before any candidate directory is created.
    """

    path = path.absolute()
    _regular(path, label="handoff selection")
    raw = _read_json(path, label="handoff selection")
    if raw.get("schema_version") != HANDOFF_SELECTION_SCHEMA:
        raise ConversionError("handoff selection has an unexpected schema_version")
    selection_id = raw.get("selection_id")
    rows = raw.get("handoffs")
    if not isinstance(selection_id, str) or not re.fullmatch(r"[A-Za-z0-9._-]+", selection_id):
        raise ConversionError("handoff selection has an invalid selection_id")
    if not isinstance(rows, list) or not rows:
        raise ConversionError("handoff selection must contain at least one handoff")
    normalized: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for row in rows:
        if not isinstance(row, dict):
            raise ConversionError("handoff selection rows must be objects")
        root_value = row.get("root")
        handoff_ref = row.get("handoff_ref")
        handoff_sha256 = row.get("sha256")
        if (
            not isinstance(root_value, str)
            or not Path(root_value).is_absolute()
            or Path(root_value).is_symlink()
            or not Path(root_value).is_dir()
            or not isinstance(handoff_ref, str)
            or not isinstance(handoff_sha256, str)
            or _HEX64.fullmatch(handoff_sha256) is None
        ):
            raise ConversionError("handoff selection row has an invalid root/ref/digest binding")
        _safe_handoff_ref(handoff_ref, label="handoff reference")
        root = Path(root_value).resolve()
        key = (str(root), handoff_ref)
        if key in seen:
            raise ConversionError("handoff selection repeats a root/reference")
        seen.add(key)
        normalized.append({"root": root, "handoff_ref": handoff_ref, "sha256": handoff_sha256})
    return normalized, {
        "schema_version": HANDOFF_SELECTION_SCHEMA,
        "selection_id": selection_id,
        "source": str(path),
        "handoffs": normalized,
    }


def _load_direct_handoff(
    *,
    root: Path,
    handoff_ref: str,
    expected_sha256: str,
    base_revision: str,
) -> DirectHandoff:
    """Normalize one raw handoff using acquisition's shared verifier.

    The verifier is deliberately the acquisition owner's public seam.  It
    checks the handoff manifest, source/rights records, Item/File bindings,
    independent fixity, and local custody without loading the accepted store
    or producing a corpus batch.  This module then performs one aggregated
    accepted-index/topology closure pass for all returned handoffs.
    """

    _safe_handoff_ref(handoff_ref, label="handoff reference")
    root = root.absolute()
    if not root.is_dir() or root.is_symlink():
        raise ConversionError(f"handoff root is not a regular directory: {root}")
    handoff_path = root / PurePosixPath(handoff_ref)
    if handoff_path.is_symlink() or not handoff_path.is_file():
        raise ConversionError(f"handoff reference is not a regular file: {handoff_ref}")
    if _sha256(handoff_path) != expected_sha256:
        raise ConversionError(
            f"handoff digest changed; re-acquire immutable input: {handoff_ref}"
        )
    try:
        import acquisition_handoff_adapter as acquisition_adapter
    except ModuleNotFoundError as exc:
        raise ConversionError(
            "direct handoff intake requires acquisition_handoff_adapter.verify_handoff_for_intake "
            "from the acquisition route"
        ) from exc
    try:
        verifier_repo_root = getattr(acquisition_adapter, "REPO_ROOT", None)
        if verifier_repo_root is None:
            verifier_repo_root = getattr(
                getattr(acquisition_adapter, "acquisition", None),
                "REPO_ROOT",
                SOFTWARE_ROOT,
            )
        verified = acquisition_adapter.verify_handoff_for_intake(
            acquisition_root=root,
            handoff_ref=handoff_ref,
            expected_base_revision=base_revision,
            repo_root=verifier_repo_root,
        )
    except Exception as exc:
        raise ConversionError(f"acquisition handoff rejected: {exc}") from exc
    handoff = verified.handoff
    context = verified.context
    manifest = context.manifest
    batch_id = manifest.get("batch_id")
    batch_revision = manifest.get("batch_revision")
    if not isinstance(batch_id, str) or not re.fullmatch(
        r"tos\.acquisition-batch\.[A-Za-z0-9._-]+", batch_id
    ):
        raise ConversionError("direct handoff manifest has an invalid batch_id")
    if type(batch_revision) is not int or batch_revision < 1:
        raise ConversionError("direct handoff manifest has an invalid batch_revision")
    if handoff.get("batch_id") != batch_id or handoff.get("batch_revision") != batch_revision:
        raise ConversionError("direct handoff identity differs from its manifest")
    source_root = root / "source"
    payload_root = root / "payload"
    if (
        source_root.is_symlink()
        or not source_root.is_dir()
        or payload_root.is_symlink()
        or not payload_root.is_dir()
    ):
        raise ConversionError("direct handoff source/payload roots are missing")
    optional_context: dict[str, Any] | None = None
    context_ref = handoff.get("validation_context_ref")
    if context_ref is not None:
        _safe_handoff_ref(context_ref, label="handoff validation context")
        context_path = root / PurePosixPath(context_ref)
        if context_path.is_symlink() or not context_path.is_file():
            raise ConversionError("direct handoff validation context is not readable")
        optional_context = _read_json(context_path, label="handoff validation context")
        if optional_context.get("schema_version") != "tos_corpus_validation_context_v1":
            raise ConversionError("direct handoff validation context has an unexpected schema")
        context_sha256 = handoff.get("validation_context_sha256")
        if context_sha256 is not None and context_sha256 != _sha256(context_path):
            raise ConversionError("direct handoff validation context digest differs")
    return DirectHandoff(
        root=root,
        handoff_ref=handoff_ref,
        handoff_sha256=expected_sha256,
        handoff_path=handoff_path,
        handoff=handoff,
        manifest_path=root / "manifest.json",
        manifest=manifest,
        source_root=source_root,
        payload_root=payload_root,
        source_records=sorted(verified.selected_source_rows, key=lambda row: row["ref"]),
        payloads=list(verified.payloads),
        context=optional_context,
    )


def _regular(path: Path, *, label: str) -> os.stat_result:
    try:
        absolute = path.absolute()
        if absolute != path.resolve() or path.is_symlink():
            raise ConversionError(f"{label} may not be a symlink: {path}")
        info = path.stat()
    except ConversionError:
        raise
    except OSError as exc:
        raise ConversionError(f"{label} is not readable: {path}") from exc
    if not stat.S_ISREG(info.st_mode):
        raise ConversionError(f"{label} is not a regular file: {path}")
    return info


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as exc:
        raise ConversionError(f"cannot digest source file: {path}") from exc
    return digest.hexdigest()


def _handoff_payload_path(payload_root: Path, item_root_ref: str, relative_path: str) -> Path:
    item = PurePosixPath(item_root_ref)
    relative = PurePosixPath(relative_path)
    if (
        tuple(item.parts[:2]) != ("ToS", "source-witnesses")
        or not relative.parts
        or relative.parts[0] != "payload"
        or any(part in {"", ".", ".."} for part in (*item.parts, *relative.parts))
    ):
        raise ConversionError(f"invalid handoff payload path: {item_root_ref}/{relative_path}")
    candidates = (
        payload_root.joinpath(*item.parts[2:], *relative.parts),
        payload_root.joinpath(*item.parts, *relative.parts),
    )
    existing = [path for path in candidates if path.exists() or path.is_symlink()]
    if not existing:
        raise ConversionError(f"handoff payload is missing: {item_root_ref}/{relative_path}")
    if any(path.is_symlink() or not path.is_file() for path in existing):
        raise ConversionError(f"handoff payload is not a regular file: {item_root_ref}/{relative_path}")
    if len(existing) == 2 and _sha256(existing[0]) != _sha256(existing[1]):
        raise ConversionError(f"handoff payload layouts differ: {item_root_ref}/{relative_path}")
    return existing[0]


def _safe_source_ref(value: object) -> str | None:
    if not isinstance(value, str) or not value.startswith(SOURCE_PREFIX):
        return None
    candidate = value.split("#", 1)[0]
    candidate = re.sub(r":\d+\Z", "", candidate)
    if _REF.fullmatch(candidate) is None:
        return None
    return candidate


def _walk_refs(value: Any) -> Iterator[str]:
    if isinstance(value, str):
        ref = _safe_source_ref(value)
        if ref is not None:
            yield ref
    elif isinstance(value, dict):
        for child in value.values():
            yield from _walk_refs(child)
    elif isinstance(value, list):
        for child in value:
            yield from _walk_refs(child)


def _file_refs(path: Path) -> set[str]:
    """Extract path references from a JSON or JSONL metadata member."""

    if path.suffix not in {".json", ".jsonl"}:
        return set()
    references: set[str] = set()
    try:
        if path.suffix == ".jsonl":
            for row in _read_jsonl(path, label="metadata JSONL"):
                references.update(_walk_refs(row))
        else:
            references.update(_walk_refs(_read_json(path, label="metadata JSON")))
    except ConversionError:
        # The authoritative admission validator owns schema/parsing errors. A
        # malformed file must still be carried into the candidate so admission
        # rejects it rather than the converter silently replacing it.
        return set()
    return references


def _iter_snapshot_file_entries(snapshot: Path) -> Iterator[dict[str, Any]]:
    """Stream only the ``files`` array from the large canonical snapshot.

    The accepted snapshot is about 2 GB.  Loading its dependency and identity
    maps just to locate three relation objects would consume several GB of RAM.
    This bounded scanner maps the immutable file read-only and decodes one
    small member object at a time.  The full snapshot remains validated by
    ``CorpusStore`` during admission.
    """

    _regular(snapshot, label="accepted snapshot")
    marker = b'"files":['
    with snapshot.open("rb") as stream:
        mapped = mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ)
        try:
            start = mapped.find(marker)
            if start < 0:
                raise ConversionError(f"accepted snapshot has no files array: {snapshot}")
            position = start + len(marker)
            decoder = json.JSONDecoder()
            while True:
                while mapped[position : position + 1] in (b" ", b"\n", b"\r", b"\t", b","):
                    position += 1
                if mapped[position : position + 1] == b"]":
                    return
                if mapped[position : position + 1] != b"{":
                    raise ConversionError("accepted snapshot files array is malformed")
                # ``raw_decode`` needs text, but each file entry is bounded and
                # much smaller than the enclosing snapshot.
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
                    # ``raw_decode`` reports Unicode character offsets while
                    # the mmap scanner tracks byte offsets.  Compare against
                    # the decoded slice length so a valid non-ASCII path is
                    # not rejected as malformed metadata.
                    decoded = mapped[position:end].decode("utf-8")
                    value, consumed = decoder.raw_decode(decoded)
                except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                    raise ConversionError("accepted snapshot contains malformed file metadata") from exc
                if consumed != len(decoded) or not isinstance(value, dict):
                    raise ConversionError("accepted snapshot file entry is malformed")
                yield value
                position = end
        finally:
            mapped.close()


def _accepted_index(
    snapshot: Path,
    *,
    include_entries: bool = False,
) -> tuple[
    set[str],
    dict[str, dict[str, Any]],
    dict[str, dict[str, Any]],
    dict[str, Any],
]:
    paths: set[str] = set()
    selected: dict[str, dict[str, Any]] = {}
    record_entries: dict[str, dict[str, Any]] = {}
    accepted_entries: dict[str, dict[str, Any]] = {}
    topology_event_entry: dict[str, Any] | None = None
    claim_paths = {
        "ToS/source-witnesses/relations/work-expression/work-expression-claims.jsonl",
        "ToS/source-witnesses/relations/expression-edition/expression-edition-claims.jsonl",
        "ToS/source-witnesses/relations/edition-item/edition-item-claims.jsonl",
    }
    for entry in _iter_snapshot_file_entries(snapshot):
        path = entry.get("path")
        if not isinstance(path, str):
            raise ConversionError("accepted snapshot file entry has no path")
        paths.add(path)
        if include_entries:
            accepted_entries[path] = {
                key: entry.get(key)
                for key in ("path", "sha256", "size_bytes", "mode")
                if key in entry
            }
        if path in claim_paths:
            selected[path] = entry
        if path.endswith(("/work.json", "/expression.json", "/edition.json")):
            record_entries[path] = entry
        if path == TOPOLOGY_EVENT_PATH:
            topology_event_entry = entry
    if set(selected) != claim_paths:
        raise ConversionError("accepted snapshot is missing a relation claim object")
    if topology_event_entry is None:
        raise ConversionError("accepted snapshot is missing the topology provenance event")
    if include_entries:
        # Keep the historical four-value API for callers which only need the
        # topology indexes.  The direct handoff route opts into this compact
        # path-to-digest map while streaming the same snapshot exactly once.
        return paths, selected, record_entries, topology_event_entry, accepted_entries  # type: ignore[return-value]
    return paths, selected, record_entries, topology_event_entry


def _hardlink(source: Path, destination: Path, *, label: str) -> None:
    info = _regular(source, label=label)
    if destination.exists() or destination.is_symlink():
        raise ConversionError(f"refusing to replace candidate member: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        os.link(source, destination, follow_symlinks=False)
    except OSError as exc:
        raise ConversionError(f"cannot hard-link {source} to {destination}") from exc
    if destination.stat().st_size != info.st_size:
        raise ConversionError(f"hard-linked member changed during conversion: {source}")


def _select_source(
    selected_paths: dict[str, SourceFile],
    relative: str,
    source: SourceFile,
) -> None:
    """Select one source copy and reject divergent duplicated evidence."""

    previous = selected_paths.get(relative)
    if previous is None:
        selected_paths[relative] = source
        return
    if previous.path == source.path:
        return
    previous_digest = _sha256(previous.path)
    source_digest = _sha256(source.path)
    if previous_digest != source_digest:
        raise ConversionError(
            f"duplicated acquisition metadata differs for {relative}: "
            f"{previous.batch_id}={previous_digest}, {source.batch_id}={source_digest}"
        )


def _copy_with_append(source: Path, destination: Path, rows: Iterable[dict[str, Any]]) -> int:
    """Write an immutable base claim file plus canonical new claim rows."""

    _regular(source, label="accepted claim object")
    if destination.exists() or destination.is_symlink():
        raise ConversionError(f"refusing to replace candidate claim file: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    count = 0
    try:
        with source.open("rb") as input_stream, destination.open("xb") as output:
            for block in iter(lambda: input_stream.read(1024 * 1024), b""):
                output.write(block)
            for row in rows:
                output.write(_canonical(row))
                count += 1
            output.flush()
            os.fsync(output.fileno())
    except OSError as exc:
        raise ConversionError(f"cannot assemble claim file: {destination}") from exc
    return count


def _topology_route(path: str) -> tuple[str, str, str, str]:
    """Return the identity-link field and event metadata for a claim route."""

    try:
        return TOPOLOGY_RELATION_ROUTES[path.rsplit("/", 1)[-1]]
    except KeyError as exc:
        raise ConversionError(f"unknown topology claim route: {path}") from exc


def _extend_accepted_topology_records(
    *,
    store_root: Path,
    metadata_root: Path,
    accepted_paths: set[str],
    record_entries: dict[str, dict[str, Any]],
    claim_rows: dict[str, list[dict[str, Any]]],
    selected_paths: dict[str, SourceFile],
    available_claim_ids: set[str],
    dangling_claim_refs: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    """Add only the new outgoing claim IDs to accepted identity records.

    Acquisition packages contain a complete Work/Expression/Edition record,
    but a path already present in the accepted revision is a shared identity,
    not permission to replace that record with a package copy.  The source
    contract requires the record's outgoing claim-ref list to close over every
    current topology claim.  This helper therefore starts from the exact
    accepted CAS object and changes only the one link field and its version.
    """

    additions: dict[str, tuple[str, list[str]]] = {}
    for claim_path, rows in sorted(claim_rows.items()):
        field, evidence_basename, _count_field, _role = _topology_route(claim_path)
        for claim in rows:
            claim_id = claim.get("claim_id")
            evidence_refs = claim.get("evidence_refs")
            if not isinstance(claim_id, str) or not isinstance(evidence_refs, list):
                raise ConversionError(f"topology claim is missing identity/evidence: {claim_path}")
            targets = [
                value
                for value in evidence_refs
                if isinstance(value, str) and Path(value).name == evidence_basename
            ]
            if len(targets) != 1:
                raise ConversionError(
                    f"topology claim has {len(targets)} {evidence_basename} evidence refs: {claim_id}"
                )
            target = targets[0]
            prior = additions.get(target)
            if prior is None:
                additions[target] = (field, [claim_id])
            else:
                prior_field, claim_ids_for_target = prior
                if prior_field != field:
                    raise ConversionError(f"topology identity is bound to multiple link fields: {target}")
                claim_ids_for_target.append(claim_id)

    dangling_by_target: dict[tuple[str, str], list[str]] = defaultdict(list)
    for item in dangling_claim_refs:
        relative = item.get("record_ref")
        field = item.get("field")
        claim_id = item.get("claim_id")
        if (
            not isinstance(relative, str)
            or not isinstance(field, str)
            or not isinstance(claim_id, str)
            or field not in {route[0] for route in TOPOLOGY_RELATION_ROUTES.values()}
        ):
            raise ConversionError(f"invalid dangling topology correction: {item!r}")
        key = (relative, field)
        if claim_id in dangling_by_target[key]:
            raise ConversionError(f"dangling topology correction repeats a claim: {relative} {claim_id}")
        dangling_by_target[key].append(claim_id)

    targets: dict[tuple[str, str], list[str]] = {
        (relative, field): list(claim_ids)
        for relative, (field, claim_ids) in additions.items()
    }
    for key in dangling_by_target:
        targets.setdefault(key, [])

    extensions: list[dict[str, Any]] = []
    for (relative, field), claim_ids_for_target in sorted(targets.items()):
        removed_claim_ids = sorted(set(dangling_by_target.get((relative, field), [])))
        if relative in accepted_paths:
            if removed_claim_ids:
                raise ConversionError(
                    "accepted topology record contains an unresolved claim ref: "
                    f"{relative}: {removed_claim_ids}"
                )
            entry = record_entries.get(relative)
            if entry is None:
                raise ConversionError(f"accepted topology record lacks a snapshot entry: {relative}")
            base_digest = entry.get("sha256")
            if not isinstance(base_digest, str) or _HEX64.fullmatch(base_digest) is None:
                raise ConversionError(f"accepted topology record has invalid digest: {relative}")
            base_path = store_root / "objects" / base_digest
            base_origin = "accepted_cas"
            _regular(base_path, label="accepted topology record object")
            if _sha256(base_path) != base_digest:
                raise ConversionError(f"accepted topology record object digest mismatch: {relative}")
        else:
            selected = selected_paths.get(relative)
            if selected is None:
                raise ConversionError(f"new topology record was not selected: {relative}")
            base_path = selected.path
            _regular(base_path, label="acquired topology record")
            base_digest = _sha256(base_path)
            base_origin = "acquired_metadata"
        base = _read_json(base_path, label="accepted topology record")
        if base.get("record_type") not in {"work", "expression", "edition"}:
            raise ConversionError(f"accepted topology record has unexpected type: {relative}")
        prior_refs = base.get(field)
        version = base.get("record_version")
        if not isinstance(prior_refs, list) or any(not isinstance(value, str) for value in prior_refs):
            raise ConversionError(f"accepted topology record has invalid {field}: {relative}")
        if type(version) is not int or version < 1:
            raise ConversionError(f"accepted topology record has invalid record_version: {relative}")
        if len(set(prior_refs)) != len(prior_refs):
            raise ConversionError(f"accepted topology record repeats a prior {field}: {relative}")
        if len(set(claim_ids_for_target)) != len(claim_ids_for_target):
            raise ConversionError(f"new topology claims repeat for {relative}")
        if len(set(removed_claim_ids)) != len(removed_claim_ids):
            raise ConversionError(f"dangling topology claims repeat for {relative}")
        missing_removed = [claim_id for claim_id in removed_claim_ids if claim_id not in prior_refs]
        if missing_removed:
            raise ConversionError(
                f"dangling topology correction is absent from predecessor {relative}: {missing_removed}"
            )
        if any(claim_id in available_claim_ids for claim_id in removed_claim_ids):
            raise ConversionError(
                f"topology correction would remove an available claim: {relative}: {removed_claim_ids}"
            )
        retained_prior_refs = [claim_id for claim_id in prior_refs if claim_id not in removed_claim_ids]
        if set(retained_prior_refs).intersection(claim_ids_for_target):
            # A package may already have carried one of the exact new rows.
            # Preserve that source assertion and append only the missing IDs.
            additions_for_record = [
                claim_id for claim_id in claim_ids_for_target if claim_id not in retained_prior_refs
            ]
        else:
            additions_for_record = list(claim_ids_for_target)
        if not additions_for_record and not removed_claim_ids:
            continue
        successor = dict(base)
        successor[field] = [*retained_prior_refs, *additions_for_record]
        successor["record_version"] = version + 1
        destination = metadata_root / relative
        _write_canonical(destination, successor)
        selected_paths[relative] = SourceFile(relative, destination, "generated")
        extensions.append(
            {
                "path": relative,
                "field": field,
                "base_sha256": base_digest,
                "candidate_sha256": _sha256(destination),
                "base_origin": base_origin,
                "base_record_version": version,
                "candidate_record_version": successor["record_version"],
                "appended_claim_ids": additions_for_record,
                "removed_claim_ids": removed_claim_ids,
                "correction_reason": (
                    "removed staged claim refs without an exact current topology carrier"
                    if removed_claim_ids
                    else None
                ),
                "base_preserved": True,
            }
        )
    return extensions


def _topology_configuration(claims: Iterable[dict[str, Any]]) -> dict[str, Any]:
    counts = defaultdict(int)
    for claim in claims:
        predicate = claim.get("predicate")
        if isinstance(predicate, str):
            counts[predicate] += 1
    return {
        "canon_promotion_performed": False,
        "edition_item_claims_materialized": counts["exemplified_by"],
        "expression_edition_claims_materialized": counts["embodied_by"],
        "human_review_performed": False,
        "semantic_claims_created": 0,
        "source_text_admitted": False,
        "textual_equivalence_claims_created": 0,
        "topology_claims_reviewed": 0,
        "work_expression_claims_materialized": counts["has_expression"],
    }


def _topology_input_role(relative: str) -> str:
    kind = "item-embodiment-manifest" if Path(relative).name == "item.manifest.json" else Path(relative).stem + "-topology-record"
    return "declared-" + kind


def _accepted_claim_ids(
    store_root: Path, claim_entries: dict[str, dict[str, Any]]
) -> set[str]:
    """Read only the three compact accepted relation objects for their IDs."""

    return set(_accepted_claim_rows(store_root, claim_entries))


def _accepted_claim_rows(
    store_root: Path, claim_entries: dict[str, dict[str, Any]]
) -> dict[str, tuple[str, dict[str, Any]]]:
    """Read accepted relation rows once, retaining exact canonical carriers."""

    rows_by_id: dict[str, tuple[str, dict[str, Any]]] = {}
    for relative, entry in sorted(claim_entries.items()):
        digest = entry.get("sha256")
        if not isinstance(digest, str) or _HEX64.fullmatch(digest) is None:
            raise ConversionError(f"accepted relation object has an invalid digest: {relative}")
        path = store_root / "objects" / digest
        _regular(path, label="accepted relation object")
        if _sha256(path) != digest:
            raise ConversionError(f"accepted relation object digest mismatch: {relative}")
        for row in _read_jsonl(path, label="accepted relation claims"):
            claim_id = row.get("claim_id")
            if not isinstance(claim_id, str) or claim_id in rows_by_id:
                raise ConversionError(f"accepted relation claim identity is invalid or duplicated: {relative}")
            rows_by_id[claim_id] = (relative, row)
    return rows_by_id


def _index_acquisition_topology_claims(
    acquisition_root: Path, specs: Iterable[BatchSpec]
) -> dict[str, tuple[str, dict[str, Any], Path]]:
    """Index exact dependency claim rows without importing whole relation files."""

    indexed: dict[str, tuple[str, dict[str, Any], Path]] = {}
    for spec in specs:
        metadata_root = acquisition_root / spec.batch_id / "metadata"
        relation_root = metadata_root / "ToS/source-witnesses/relations"
        for path in sorted(relation_root.glob("*/*.jsonl")):
            if path.name not in TOPOLOGY_RELATION_ROUTES:
                continue
            relative = path.relative_to(metadata_root).as_posix()
            for row in _read_jsonl(path, label="acquisition topology claims"):
                claim_id = row.get("claim_id")
                if not isinstance(claim_id, str):
                    raise ConversionError(f"acquisition topology claim has no claim_id: {relative}")
                previous = indexed.get(claim_id)
                if previous is not None and _canonical(previous[1]) != _canonical(row):
                    raise ConversionError(f"acquisition topology claim differs across packages: {claim_id}")
                indexed.setdefault(claim_id, (relative, row, path))
    return indexed


def _index_handoff_topology_claims(
    source_roots: Iterable[tuple[str, Path]],
) -> dict[str, tuple[str, dict[str, Any], Path]]:
    """Index claim rows from explicit handoff source roots only."""

    indexed: dict[str, tuple[str, dict[str, Any], Path]] = {}
    for batch_id, source_root in source_roots:
        relation_root = source_root / "ToS/source-witnesses/relations"
        if not relation_root.is_dir() or relation_root.is_symlink():
            continue
        for path in sorted(relation_root.glob("*/*.jsonl")):
            if path.name not in TOPOLOGY_RELATION_ROUTES:
                continue
            relative = path.relative_to(source_root).as_posix()
            for row in _read_jsonl(path, label="handoff topology claims"):
                claim_id = row.get("claim_id")
                if not isinstance(claim_id, str):
                    raise ConversionError(f"handoff topology claim has no claim_id: {relative}")
                previous = indexed.get(claim_id)
                if previous is not None:
                    if previous[0] != relative or _canonical(previous[1]) != _canonical(row):
                        raise ConversionError(f"handoff topology claim differs across handoffs: {claim_id}")
                indexed.setdefault(claim_id, (relative, row, path))
    return indexed


def _collect_topology_dependencies(
    *,
    acquisition_root: Path | None,
    specs: list[BatchSpec],
    accepted_paths: set[str],
    candidates: dict[str, list[SourceFile]],
    selected_paths: dict[str, SourceFile],
    claim_rows: dict[str, list[dict[str, Any]]],
    accepted_claim_ids: set[str],
    payload_root: Path,
    claim_sources: dict[str, tuple[str, dict[str, Any], Path]] | None = None,
    source_roots: dict[str, Path] | None = None,
    payload_roots: dict[str, Path] | None = None,
) -> tuple[dict[str, list[dict[str, Any]]], list[dict[str, Any]], dict[str, Any], int, int]:
    """Resolve staged dependency claim refs before the source closure scan.

    Prepared packages can carry a Work/Expression/Edition record whose older
    topology refs belong to another acquired stream.  If the exact row exists
    in one of the selected packages, bring that row and its evidence closure
    into this atomic batch.  A missing row is retained as an explicit
    correction candidate; the record successor later removes only that
    unresolvable staged ref with a receipt, rather than silently accepting a
    dangling identity field.
    """

    if claim_sources is None:
        if acquisition_root is None:
            raise ConversionError("topology claim source roots are missing")
        claim_sources = _index_acquisition_topology_claims(acquisition_root, specs)
    known = set(accepted_claim_ids)
    for rows in claim_rows.values():
        known.update(row.get("claim_id") for row in rows if isinstance(row.get("claim_id"), str))
    pending: list[tuple[str, str, str]] = []
    processed_records: set[str] = set()
    dangling: list[dict[str, Any]] = []
    dependency_rows: dict[str, tuple[str, dict[str, Any], Path]] = {}
    dependency_items: set[str] = set()
    dependency_record_refs: set[str] = set()
    dependency_metadata_refs: set[str] = set()
    unresolved_evidence: list[dict[str, Any]] = []
    dependency_payload_unavailable: list[dict[str, Any]] = []
    dependency_payload_count = 0
    dependency_payload_bytes = 0

    def payload_source_for(batch_id: str) -> Path | None:
        if payload_roots is not None and batch_id in payload_roots:
            return payload_roots[batch_id]
        if acquisition_root is not None:
            return acquisition_root / batch_id / "payload"
        return None

    def enqueue_record(relative: str, field: str, claim_id: str) -> None:
        pending.append((relative, field, claim_id))

    # Only newly selected identity records can introduce dependencies.  Base
    # records are resolved by the immutable accepted revision/event bindings.
    # The set is revisited after each dependency evidence record is selected.
    while True:
        discovered = False
        for relative, source in sorted(selected_paths.items()):
            if relative in processed_records or relative in accepted_paths:
                continue
            if Path(relative).name not in {"work.json", "expression.json", "edition.json"}:
                continue
            _regular(source.path, label="selected topology record")
            record = _read_json(source.path, label="selected topology record")
            processed_records.add(relative)
            discovered = True
            for field in ("expression_claim_refs", "embodiment_claim_refs", "exemplar_claim_refs"):
                values = record.get(field)
                if not isinstance(values, list):
                    continue
                for claim_id in values:
                    if not isinstance(claim_id, str) or claim_id in known:
                        continue
                    enqueue_record(relative, field, claim_id)

        while pending:
            relative, field, claim_id = pending.pop()
            if claim_id in known:
                continue
            candidate = claim_sources.get(claim_id)
            if candidate is None:
                dangling.append(
                    {
                        "record_ref": relative,
                        "field": field,
                        "claim_id": claim_id,
                        "reason": "no exact row in accepted relation objects or selected acquisition relation packages",
                    }
                )
                continue
            claim_path, row, source_path = candidate
            existing = dependency_rows.get(claim_id)
            if existing is not None and _canonical(existing[1]) != _canonical(row):
                raise ConversionError(f"dependency claim differs from its indexed row: {claim_id}")
            dependency_rows[claim_id] = candidate
            known.add(claim_id)
            for evidence_ref in row.get("evidence_refs", []):
                if not isinstance(evidence_ref, str) or not evidence_ref.startswith(SOURCE_PREFIX):
                    raise ConversionError(f"dependency claim has invalid evidence ref: {claim_id}")
                if evidence_ref in accepted_paths or evidence_ref in selected_paths:
                    continue
                options = candidates.get(evidence_ref, [])
                if not options:
                    unresolved_evidence.append(
                        {"claim_id": claim_id, "evidence_ref": evidence_ref}
                    )
                    continue
                selected = options[0]
                if evidence_ref.endswith(("/item.json", "/item.manifest.json")):
                    item_ref = evidence_ref.removesuffix("/item.json").removesuffix("/item.manifest.json")
                    payload_candidates = [
                        option
                        for option in options
                        if (
                            payload_source_for(option.batch_id) is not None
                            and (
                                payload_source_for(option.batch_id)
                                / PurePosixPath(item_ref).relative_to("ToS/source-witnesses")
                                / "payload"
                            ).is_dir()
                        )
                    ]
                    if payload_candidates:
                        selected = payload_candidates[0]
                _select_source(selected_paths, evidence_ref, selected)
                dependency_metadata_refs.add(evidence_ref)
                if evidence_ref.endswith(("/item.json", "/item.manifest.json")):
                    item_ref = evidence_ref.removesuffix("/item.json").removesuffix("/item.manifest.json")
                    if item_ref in dependency_items:
                        continue
                    dependency_items.add(item_ref)
                    metadata_base = (
                        source_roots[selected.batch_id]
                        if source_roots is not None and selected.batch_id in source_roots
                        else acquisition_root / selected.batch_id / "metadata"
                        if acquisition_root is not None
                        else None
                    )
                    payload_base = payload_source_for(selected.batch_id)
                    if metadata_base is None or payload_base is None:
                        raise ConversionError(f"dependency roots are unavailable: {item_ref}")
                    for item_file in _all_item_files(metadata_base, item_ref):
                        item_relative = item_file.relative_to(metadata_base).as_posix()
                        if item_relative not in selected_paths:
                            dependency_metadata_refs.add(item_relative)
                        _select_source(
                            selected_paths,
                            item_relative,
                            SourceFile(item_relative, item_file, selected.batch_id),
                        )
                    manifest = _read_json(
                        metadata_base / f"{item_ref}/item.manifest.json",
                        label="dependency Item manifest",
                    )
                    payload_item_root = (
                        payload_base
                        / PurePosixPath(item_ref).relative_to("ToS/source-witnesses")
                        / "payload"
                    )
                    if payload_item_root.is_dir() and not payload_item_root.is_symlink():
                        linked, bytes_count = _verify_fixity_and_link_payload(
                            payload_base,
                            item_ref,
                            payload_root,
                            manifest,
                            metadata_root=metadata_base,
                            source_payload_root=payload_base,
                        )
                        dependency_payload_count += linked
                        dependency_payload_bytes += bytes_count
                    else:
                        dependency_payload_unavailable.append(
                            {
                                "item_ref": item_ref,
                                "batch_id": selected.batch_id,
                                "reason": "dependency Item is topology evidence only; no selected-package payload custody",
                            }
                        )
                    discovered = True
                elif Path(evidence_ref).name in {"work.json", "expression.json", "edition.json"}:
                    dependency_record_refs.add(evidence_ref)

        if not discovered:
            break

    for claim_id, (claim_path, row, source_path) in sorted(dependency_rows.items()):
        if any(claim_id == existing.get("claim_id") for existing in claim_rows.get(claim_path, [])):
            continue
        claim_rows[claim_path].append(row)

    summary = {
        "dependency_claim_count": len(dependency_rows),
        "dependency_record_count": len(dependency_record_refs),
        "dependency_metadata_file_count": len(dependency_metadata_refs),
        "dependency_item_count": len(dependency_items),
        "dependency_payload_count": dependency_payload_count,
        "dependency_payload_bytes": dependency_payload_bytes,
        "dependency_payload_unavailable": dependency_payload_unavailable,
        "dangling_claim_ref_count": len(dangling),
        "dangling_claim_refs": dangling,
        "unresolved_evidence": unresolved_evidence,
    }
    if unresolved_evidence:
        raise ConversionError("dependency claim evidence is unavailable: " + str(unresolved_evidence[:3]))
    return dependency_rows, dangling, summary, dependency_payload_count, dependency_payload_bytes


def _refresh_topology_event(
    *,
    store_root: Path,
    metadata_root: Path,
    accepted_event_entry: dict[str, Any],
    selected_paths: dict[str, SourceFile],
    claim_rows: dict[str, list[dict[str, Any]]],
    topology_ended_at: str | None,
) -> dict[str, Any]:
    """Create the candidate successor of the one legacy topology event.

    The accepted event remains immutable in its CAS object.  Its input map is
    reused as exact historical bindings for unchanged evidence, while changed
    accepted identity records and newly acquired Items receive their candidate
    digests.  The event ID and warning/authority posture stay fixed; only the
    materialized claim outputs, closure inputs, configuration counts, version,
    and deterministic end timestamp move with the candidate corpus.
    """

    base_digest = accepted_event_entry.get("sha256")
    if not isinstance(base_digest, str) or _HEX64.fullmatch(base_digest) is None:
        raise ConversionError("accepted topology event has an invalid digest")
    base_path = store_root / "objects" / base_digest
    _regular(base_path, label="accepted topology event object")
    if _sha256(base_path) != base_digest:
        raise ConversionError("accepted topology event object digest mismatch")
    rows = _read_jsonl(base_path, label="accepted topology event")
    if len(rows) != 1:
        raise ConversionError("accepted topology provenance must contain exactly one event")
    event = rows[0]
    if event.get("event_id") != "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31":
        raise ConversionError("accepted topology event ID is not the owned route")
    if type(event.get("event_version")) is not int or event["event_version"] < 1:
        raise ConversionError("accepted topology event has invalid event_version")
    original_inputs = event.get("inputs")
    if not isinstance(original_inputs, list):
        raise ConversionError("accepted topology event inputs are not a list")
    input_map: dict[str, dict[str, Any]] = {}
    for item in original_inputs:
        if not isinstance(item, dict) or not isinstance(item.get("ref"), str):
            raise ConversionError("accepted topology event has malformed input")
        ref = item["ref"]
        if ref in input_map:
            raise ConversionError(f"accepted topology event repeats input ref: {ref}")
        digest = item.get("sha256")
        if not isinstance(digest, str) or _HEX64.fullmatch(digest) is None:
            raise ConversionError(f"accepted topology event input has invalid digest: {ref}")
        input_map[ref] = item

    all_claims: list[dict[str, Any]] = []
    outputs: list[dict[str, Any]] = []
    for claim_path, _new_rows in sorted(claim_rows.items()):
        route = _topology_route(claim_path)
        candidate_path = metadata_root / claim_path
        _regular(candidate_path, label="candidate topology claim file")
        claims = _read_jsonl(candidate_path, label="candidate topology claims")
        all_claims.extend(claims)
        outputs.append(
            {
                "ref": claim_path,
                "role": route[3],
                "sha256": _sha256(candidate_path),
            }
        )
        for claim in claims:
            evidence_refs = claim.get("evidence_refs")
            if not isinstance(evidence_refs, list):
                raise ConversionError(f"candidate topology claim has no evidence_refs: {claim_path}")
            for relative in evidence_refs:
                if not isinstance(relative, str):
                    raise ConversionError(f"candidate topology claim has invalid evidence ref: {claim_path}")
                selected = selected_paths.get(relative)
                if selected is not None:
                    source = selected.path
                    _regular(source, label="candidate topology evidence")
                    digest = _sha256(source)
                else:
                    prior = input_map.get(relative)
                    if prior is None:
                        raise ConversionError(
                            f"topology event lacks exact accepted input for evidence: {relative}"
                        )
                    digest = prior["sha256"]
                previous = input_map.get(relative)
                role = previous.get("role") if previous is not None else _topology_input_role(relative)
                if not isinstance(role, str) or not role:
                    raise ConversionError(f"topology event input has no role: {relative}")
                input_map[relative] = {"ref": relative, "role": role, "sha256": digest}

    event["inputs"] = [input_map[key] for key in sorted(input_map)]
    event["outputs"] = outputs
    method = event.get("method")
    if not isinstance(method, dict):
        raise ConversionError("accepted topology event method is not an object")
    method["configuration"] = _topology_configuration(all_claims)
    event["event_version"] += 1
    if topology_ended_at is not None:
        if not isinstance(topology_ended_at, str) or not topology_ended_at:
            raise ConversionError("topology_ended_at must be a non-empty timestamp")
        event["ended_at"] = topology_ended_at
    destination = metadata_root / TOPOLOGY_EVENT_PATH
    _write_canonical(destination, event)
    selected_paths[TOPOLOGY_EVENT_PATH] = SourceFile(TOPOLOGY_EVENT_PATH, destination, "generated")
    return {
        "path": TOPOLOGY_EVENT_PATH,
        "base_sha256": base_digest,
        "candidate_sha256": _sha256(destination),
        "base_event_version": event["event_version"] - 1,
        "candidate_event_version": event["event_version"],
        "base_input_count": len(original_inputs),
        "candidate_input_count": len(event["inputs"]),
        "candidate_output_count": len(event["outputs"]),
        "base_preserved": True,
    }


def _verify_fixity_and_link_payload(
    batch_root: Path,
    item_ref: str,
    output_payload_root: Path,
    manifest: dict[str, Any],
    *,
    metadata_root: Path | None = None,
    source_payload_root: Path | None = None,
) -> tuple[int, int]:
    payload_files = manifest.get("payload_files")
    if not isinstance(payload_files, list) or not payload_files:
        raise ConversionError(f"acquired Item manifest has no payload_files: {item_ref}")
    metadata_root = batch_root / "metadata" if metadata_root is None else metadata_root
    source_payload_root = batch_root / "payload" if source_payload_root is None else source_payload_root
    source_payload_item = (
        source_payload_root
        / PurePosixPath(item_ref).relative_to("ToS/source-witnesses")
        / "payload"
    )
    if not source_payload_item.is_dir() or source_payload_item.is_symlink():
        raise ConversionError(f"acquired payload directory is missing: {source_payload_item}")
    fixity_path = metadata_root / item_ref / "fixity.sha256"
    fixity: dict[str, str] = {}
    if fixity_path.is_file():
        for line in fixity_path.read_text(encoding="utf-8").splitlines():
            match = _FIXITY.fullmatch(line)
            if match is None:
                raise ConversionError(f"invalid fixity row: {fixity_path}")
            fixity[match.group(2)] = match.group(1)
    total_bytes = 0
    linked = 0
    for payload in payload_files:
        if not isinstance(payload, dict):
            raise ConversionError(f"invalid payload entry: {item_ref}")
        relative = payload.get("relative_path")
        digest = payload.get("sha256")
        size = payload.get("byte_size")
        file_id = payload.get("file_id")
        if (
            not isinstance(relative, str)
            or not relative.startswith("payload/")
            or relative.count("/") != 1
            or not isinstance(digest, str)
            or _HEX64.fullmatch(digest) is None
            or file_id != f"tos.file.sha256.{digest}"
            or type(size) is not int
            or size < 0
        ):
            raise ConversionError(f"invalid payload binding: {item_ref}")
        if fixity and fixity.get(relative) != digest:
            raise ConversionError(f"fixity differs from Item manifest: {item_ref}/{relative}")
        source = source_payload_item / relative.removeprefix("payload/")
        info = _regular(source, label="acquired payload")
        if info.st_size != size:
            raise ConversionError(f"payload byte size differs from manifest: {source}")
        destination = output_payload_root / PurePosixPath(item_ref).relative_to(
            "ToS/source-witnesses"
        ) / relative
        _hardlink(source, destination, label="acquired payload")
        total_bytes += size
        linked += 1
    if len(fixity) != linked:
        raise ConversionError(f"fixity does not cover exactly Item payload files: {fixity_path}")
    return linked, total_bytes


def _all_item_files(metadata_root: Path, item_ref: str) -> list[Path]:
    item_root = metadata_root / item_ref
    _regular(item_root / "item.json", label="acquired Item record")
    files: list[Path] = []
    for child in sorted(item_root.iterdir()):
        if child.is_symlink() or not child.is_file():
            continue
        files.append(child)
    required = {"item.json", "item.manifest.json", "rights.json", "provenance.jsonl",
                "resource-inventory.json", "forensic-report.md", "fixity.sha256"}
    names = {path.name for path in files}
    missing = sorted(required - names)
    if missing:
        raise ConversionError(f"acquired Item metadata is incomplete at {item_ref}: {missing}")
    return files


def _source_candidates(
    acquisition_root: Path | None,
    accepted_paths: set[str],
    specs: Iterable[BatchSpec],
    *,
    handoff_source_roots: Iterable[tuple[str, Path]] = (),
    excluded_paths: set[str] | None = None,
) -> dict[str, list[SourceFile]]:
    candidates: dict[str, list[SourceFile]] = defaultdict(list)
    excluded_paths = excluded_paths or set()

    def scan(metadata_root: Path, batch_id: str, *, legacy: bool) -> None:
        if not metadata_root.is_dir() or metadata_root.is_symlink():
            raise ConversionError(f"source metadata root is missing: {metadata_root}")
        to_s = metadata_root / "ToS"
        if not to_s.is_dir() or to_s.is_symlink():
            raise ConversionError(f"source metadata root is missing: {to_s}")
        for path in to_s.rglob("*"):
            if not path.is_file() or path.is_symlink():
                continue
            relative = path.relative_to(metadata_root).as_posix()
            if relative in excluded_paths:
                continue
            if legacy and ("/topology-before/" in relative or "/work-before/" in relative):
                continue
            if relative in accepted_paths:
                continue
            candidates[relative].append(SourceFile(relative, path, batch_id))

    for spec in specs:
        if acquisition_root is None:
            raise ConversionError("legacy acquisition root is missing")
        metadata_root = acquisition_root / spec.batch_id / "metadata"
        scan(metadata_root, spec.batch_id, legacy=True)
    for batch_id, source_root in handoff_source_roots:
        scan(source_root, batch_id, legacy=False)
    return candidates


def _acquisition_directory_ref(
    acquisition_root: Path | None,
    relative: str,
    specs: Iterable[BatchSpec],
    *,
    handoff_source_roots: Iterable[Path] = (),
) -> bool:
    """Return whether a source reference names a carried metadata directory."""

    if acquisition_root is not None:
        for spec in specs:
            candidate = acquisition_root / spec.batch_id / "metadata" / relative
            if candidate.is_dir() and not candidate.is_symlink():
                return True
    for source_root in handoff_source_roots:
        candidate = source_root / relative
        if candidate.is_dir() and not candidate.is_symlink():
            return True
    return False


def _batch_receipt_refs(batch_root: Path, package_ref: str) -> dict[str, Any]:
    refs: dict[str, Any] = {"package_ref": package_ref}
    for name in ("batch-acquisition-receipt.json", "handoff.json"):
        path = batch_root / "receipts" / name
        if path.is_file():
            refs[name] = {"path": str(path), "sha256": _sha256(path), "size_bytes": path.stat().st_size}
    return refs


def _claim_key(path: str) -> str:
    suffix = path.rsplit("/", 1)[-1]
    try:
        return CLAIM_SUFFIXES[suffix]
    except KeyError as exc:
        raise ConversionError(f"unknown relation claim path: {path}") from exc


def _write_canonical(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        raise ConversionError(f"refusing to replace generated intake file: {path}")
    try:
        path.write_bytes(_canonical(value))
        os.chmod(path, 0o644)
    except OSError as exc:
        raise ConversionError(f"cannot write generated intake file: {path}") from exc


def convert(
    *,
    acquisition_root: Path | None,
    output_root: Path,
    store_root: Path,
    base_revision: str,
    grammar_root: Path,
    batch_selection: Path | None = None,
    handoff_selection: Path | None = None,
    historical_capture: list[Path] | None = None,
    historical_root: list[Path] | None = None,
) -> dict[str, Any]:
    if (acquisition_root is None) == (handoff_selection is None):
        raise ConversionError(
            "choose exactly one input route: acquisition_root or handoff_selection"
        )
    if acquisition_root is not None:
        acquisition_root = acquisition_root.absolute()
    if handoff_selection is not None:
        handoff_selection = handoff_selection.absolute()
    output_root = output_root.absolute()
    store_root = store_root.absolute()
    grammar_root = grammar_root.absolute()
    if (historical_capture is None) != (historical_root is None):
        raise ConversionError(
            "historical evidence requires one capture and restored root for every pack"
        )
    if historical_capture is None:
        # SourceValidator deliberately supports a corpus with no historical
        # evidence.  Keep the empty context explicit at the converter boundary
        # without passing [] into the validator, where [] means a malformed
        # attempted historical selection rather than no selection.
        historical_captures = None
        historical_roots = None
    else:
        if len(historical_capture) != len(historical_root):
            raise ConversionError(
                "historical evidence selection must contain paired capture/root paths"
            )
        if not historical_capture:
            historical_captures = None
            historical_roots = None
        else:
            historical_captures = [path.absolute() for path in historical_capture]
            historical_roots = [path.absolute() for path in historical_root]
    if output_root.exists() or output_root.is_symlink():
        raise ConversionError(f"intake output must be a new path: {output_root}")
    if _HEX64.fullmatch(base_revision) is None:
        raise ConversionError("base_revision must be a lowercase SHA-256 revision")
    snapshot = store_root / "revisions" / base_revision / "snapshot.json"
    pointer_path = store_root / "current.json"
    pointer = _read_json(pointer_path, label="accepted corpus pointer")
    if pointer.get("current") != base_revision:
        raise ConversionError(
            f"accepted corpus pointer changed: expected {base_revision}, "
            f"found {pointer.get('current')}"
        )
    direct_handoffs: list[DirectHandoff] = []
    direct_selection: dict[str, Any] | None = None
    if handoff_selection is not None:
        if batch_selection is not None:
            raise ConversionError("handoff selection cannot be combined with batch selection")
        handoff_rows, direct_selection = _load_handoff_selection(handoff_selection)
        seen_direct_batch_ids: set[str] = set()
        for row in handoff_rows:
            handoff = _load_direct_handoff(
                root=row["root"],
                handoff_ref=row["handoff_ref"],
                expected_sha256=row["sha256"],
                base_revision=base_revision,
            )
            batch_id = handoff.handoff.get("batch_id")
            if not isinstance(batch_id, str) or batch_id in seen_direct_batch_ids:
                raise ConversionError("handoff selection repeats an acquisition batch")
            seen_direct_batch_ids.add(batch_id)
            direct_handoffs.append(handoff)
        specs: list[BatchSpec] = []
        selection: dict[str, Any] = direct_selection
    else:
        if batch_selection is not None and acquisition_root is None:
            raise ConversionError("batch selection requires an acquisition root")
        specs, selection = _load_batch_selection(batch_selection)
    accepted_index = _accepted_index(snapshot, include_entries=bool(direct_handoffs))
    if direct_handoffs:
        accepted_paths, claim_entries, record_entries, topology_event_entry, accepted_entries = accepted_index
    else:
        accepted_paths, claim_entries, record_entries, topology_event_entry = accepted_index
        accepted_entries = {}
    accepted_claim_rows = _accepted_claim_rows(store_root, claim_entries)
    accepted_claim_ids = set(accepted_claim_rows)
    handoff_source_roots = [
        (str(item.handoff.get("batch_id")), item.source_root) for item in direct_handoffs
    ]
    handoff_payload_roots = {
        str(item.handoff.get("batch_id")): item.payload_root for item in direct_handoffs
    }
    excluded_handoff_paths = {
        item.handoff.get("provenance_delta", {}).get("ref")
        for item in direct_handoffs
        if isinstance(item.handoff.get("provenance_delta"), dict)
        and isinstance(item.handoff.get("provenance_delta", {}).get("ref"), str)
    }
    candidates = _source_candidates(
        acquisition_root,
        accepted_paths,
        specs,
        handoff_source_roots=handoff_source_roots,
        excluded_paths={value for value in excluded_handoff_paths if isinstance(value, str)},
    )
    metadata_root = output_root / "source"
    payload_root = output_root / "payload"
    metadata_root.mkdir(parents=True)
    payload_root.mkdir()

    selected_paths: dict[str, SourceFile] = {}
    selected_refs: set[str] = set()
    package_rows: dict[str, list[dict[str, Any]]] = {}
    package_sha256: dict[str, str] = {}
    per_batch: list[dict[str, Any]] = []
    claim_rows: dict[str, list[dict[str, Any]]] = defaultdict(list)
    claim_ids: dict[str, set[str]] = defaultdict(set)
    existing_work: list[dict[str, Any]] = []
    closure_ignored_refs: dict[str, list[str]] = defaultdict(list)
    payload_count = 0
    payload_bytes = 0
    record_extensions: list[dict[str, Any]] = []
    topology_event_update: dict[str, Any] | None = None

    if direct_handoffs:
        # Direct handoffs are normalized into the same selected-path and claim
        # maps used by the legacy package route.  The acquisition verifier has
        # already checked source/rights/fixity/custody; this pass only binds
        # those rows to the one accepted index and additive topology closure.
        direct_claim_sources = _index_handoff_topology_claims(handoff_source_roots)
        seen_payload_files: set[str] = set()
        seen_payload_rows: dict[str, dict[str, Any]] = {}
        for handoff in direct_handoffs:
            batch_id = str(handoff.handoff.get("batch_id"))
            batch_paths: set[str] = set()
            for row in handoff.source_records:
                relative = row["ref"]
                source = handoff.source_root / relative
                if relative.endswith(tuple(CLAIM_SUFFIXES)):
                    for claim in _read_jsonl(source, label="handoff relation claims"):
                        claim_id = claim.get("claim_id")
                        if not isinstance(claim_id, str):
                            raise ConversionError(f"handoff relation claim has no claim_id: {relative}")
                        accepted = accepted_claim_rows.get(claim_id)
                        if accepted is not None:
                            if _canonical(accepted[1]) != _canonical(claim):
                                raise ConversionError(
                                    f"handoff relation claim would overwrite accepted bytes: {claim_id}"
                                )
                            continue
                        key = _claim_key(relative)
                        prior_rows = claim_rows[relative]
                        prior = next(
                            (candidate for candidate in prior_rows if candidate.get("claim_id") == claim_id),
                            None,
                        )
                        if prior is not None:
                            if _canonical(prior) != _canonical(claim):
                                raise ConversionError(
                                    f"handoff relation claim differs across handoffs: {claim_id}"
                                )
                            continue
                        if claim_id in claim_ids[key]:
                            raise ConversionError(f"duplicate acquired claim identity: {claim_id}")
                        claim_ids[key].add(claim_id)
                        claim_rows[relative].append(claim)
                    continue
                if relative in accepted_paths:
                    accepted_entry = accepted_entries.get(relative)
                    if accepted_entry is None or accepted_entry.get("sha256") != row.get("sha256"):
                        raise ConversionError(
                            f"handoff source would overwrite accepted bytes: {relative}"
                        )
                    digest = accepted_entry.get("sha256")
                    if not isinstance(digest, str):
                        raise ConversionError(f"accepted source entry lacks digest: {relative}")
                    accepted_object = store_root / "objects" / digest
                    _regular(accepted_object, label="accepted handoff source object")
                    if _sha256(accepted_object) != digest:
                        raise ConversionError(f"accepted handoff source object digest differs: {relative}")
                    continue
                _select_source(
                    selected_paths,
                    relative,
                    SourceFile(relative, source, batch_id),
                )
                batch_paths.add(relative)

            for payload in handoff.payloads:
                file_ref = payload["file_ref"]
                previous_payload = seen_payload_rows.get(file_ref)
                if previous_payload is not None and _canonical(previous_payload) != _canonical(payload):
                    raise ConversionError(f"handoff payload differs across inputs: {file_ref}")
                seen_payload_rows.setdefault(file_ref, payload)
                source = _handoff_payload_path(
                    handoff.payload_root,
                    payload["item_root_ref"],
                    payload["relative_path"],
                )
                destination = payload_root / PurePosixPath(
                    payload["item_root_ref"]
                ).relative_to("ToS/source-witnesses") / payload["relative_path"]
                if destination.exists() or destination.is_symlink():
                    info = _regular(destination, label="candidate payload")
                    if info.st_size != payload["byte_size"] or _sha256(destination) != payload["sha256"]:
                        raise ConversionError(f"handoff payload differs across inputs: {file_ref}")
                else:
                    _hardlink(source, destination, label="handoff payload")
                if file_ref not in seen_payload_files:
                    seen_payload_files.add(file_ref)
                    payload_count += 1
                    payload_bytes += payload["byte_size"]

            per_batch.append(
                {
                    "batch_id": batch_id,
                    "registry": "acquired-handoff",
                    "package_ref": handoff.handoff_ref,
                    "package_sha256": handoff.handoff_sha256,
                    "row_count": len(handoff.source_records),
                    "payload_count": len(handoff.payloads),
                    "payload_bytes": sum(payload["byte_size"] for payload in handoff.payloads),
                    "selected_metadata_paths": len(batch_paths),
                    "existing_work_rows": 0,
                    "existing_work_preimages": 0,
                    "existing_work_record_refs_absent_from_base": 0,
                    "source_receipts": {
                        "handoff": {
                            "path": str(handoff.handoff_path),
                            "sha256": handoff.handoff_sha256,
                        },
                        "manifest": {
                            "path": str(handoff.manifest_path),
                            "sha256": _sha256(handoff.manifest_path),
                        },
                    },
                    "rights_posture": "preserved_from_acquired_item_metadata; no publication or server-processing expansion",
                }
            )

    for spec in specs:
        batch_root = acquisition_root / spec.batch_id
        package_path = batch_root / "metadata" / spec.package_ref
        rows = _read_jsonl(package_path, label="prepared source package")
        if not rows:
            raise ConversionError(f"prepared package is empty: {package_path}")
        package_rows[spec.batch_id] = rows
        package_sha256[spec.batch_id] = _sha256(package_path)
        if spec.package_sha256 is not None and package_sha256[spec.batch_id] != spec.package_sha256:
            raise ConversionError(
                f"selected package digest differs from the handoff binding: {spec.package_ref}"
            )
        _select_source(
            selected_paths,
            spec.package_ref,
            SourceFile(spec.package_ref, package_path, spec.batch_id),
        )
        selected_refs.add(spec.package_ref)
        batch_paths: set[str] = set()
        batch_payload_count = 0
        batch_payload_bytes = 0
        batch_existing_work = 0
        batch_missing_existing_work = 0
        batch_preimages = 0
        actual_claims: dict[str, dict[str, dict[str, Any]]] = {}
        for claim_path in {
            claim["path"] for row in rows for claim in row.get("claims", [])
        }:
            relation_path = batch_root / "metadata" / claim_path
            actual_claims[claim_path] = {}
            for claim in _read_jsonl(relation_path, label="acquired relation claims"):
                claim_id = claim.get("claim_id")
                if isinstance(claim_id, str):
                    actual_claims[claim_path][claim_id] = claim

        for row in rows:
            if row.get("status") != "prepared-not-acquired":
                raise ConversionError("prepared package status changed unexpectedly")
            records = row.get("records")
            claims = row.get("claims")
            if not isinstance(records, dict) or not isinstance(claims, list):
                raise ConversionError(f"prepared package row is incomplete: {spec.batch_id}")
            row_paths = set(records)
            for record_ref in sorted(row_paths):
                if record_ref in accepted_paths:
                    # A package's copied Work/Expression/Edition record is
                    # not allowed to replace the accepted base.  The actual
                    # acquired Item closure remains selected below.
                    continue
                source = batch_root / "metadata" / record_ref
                _regular(source, label="acquired source record")
                _select_source(
                    selected_paths,
                    record_ref,
                    SourceFile(record_ref, source, spec.batch_id),
                )
                batch_paths.add(record_ref)
            item_refs = [ref for ref in row_paths if ref.endswith("/item.json")]
            if len(item_refs) != 1:
                raise ConversionError(f"prepared row must have exactly one Item: {spec.batch_id}")
            item_ref = item_refs[0].removesuffix("/item.json")
            manifest_ref = f"{item_ref}/item.manifest.json"
            manifest_path = batch_root / "metadata" / manifest_ref
            manifest = _read_json(manifest_path, label="acquired Item manifest")
            for item_file in _all_item_files(batch_root / "metadata", item_ref):
                relative = item_file.relative_to(batch_root / "metadata").as_posix()
                _select_source(
                    selected_paths,
                    relative,
                    SourceFile(relative, item_file, spec.batch_id),
                )
                batch_paths.add(relative)
            linked, bytes_count = _verify_fixity_and_link_payload(
                batch_root, item_ref, payload_root, manifest
            )
            batch_payload_count += linked
            batch_payload_bytes += bytes_count
            payload_count += linked
            payload_bytes += bytes_count
            selected_refs.add(manifest_ref)
            for claim in claims:
                claim_path = claim.get("path")
                record = claim.get("record")
                if not isinstance(claim_path, str) or not isinstance(record, dict):
                    raise ConversionError(f"invalid prepared relation claim: {spec.batch_id}")
                actual = actual_claims.get(claim_path, {}).get(record.get("claim_id"))
                if actual is None or actual != record:
                    raise ConversionError(
                        f"prepared claim differs from acquired claim file: {claim_path}"
                    )
                key = _claim_key(claim_path)
                claim_id = record.get("claim_id")
                if not isinstance(claim_id, str) or claim_id in claim_ids[key]:
                    raise ConversionError(f"duplicate acquired claim identity: {claim_id}")
                claim_ids[key].add(claim_id)
                claim_rows[claim_path].append(record)
            work = row.get("existing_work")
            if isinstance(work, dict):
                batch_existing_work += 1
                record_ref = work.get("record_ref")
                preimage_ref = work.get("preimage_ref")
                expected_preimage = work.get("sha256")
                if not all(isinstance(value, str) for value in (record_ref, preimage_ref, expected_preimage)):
                    raise ConversionError(f"invalid existing_work binding: {spec.batch_id}")
                preimage_source = batch_root / "metadata" / preimage_ref
                if not preimage_source.is_file():
                    candidates_for_preimage = candidates.get(preimage_ref, [])
                    if len(candidates_for_preimage) != 1:
                        raise ConversionError(f"existing Work preimage is unavailable: {preimage_ref}")
                    preimage_source = candidates_for_preimage[0].path
                _regular(preimage_source, label="existing Work preimage")
                if _sha256(preimage_source) != expected_preimage:
                    raise ConversionError(f"existing Work preimage digest differs: {preimage_ref}")
                _select_source(
                    selected_paths,
                    preimage_ref,
                    SourceFile(preimage_ref, preimage_source, spec.batch_id),
                )
                batch_paths.add(preimage_ref)
                batch_preimages += 1
                in_base = record_ref in accepted_paths
                if not in_base:
                    batch_missing_existing_work += 1
                existing_work.append(
                    {
                        "batch_id": spec.batch_id,
                        "record_ref": record_ref,
                        "record_ref_in_base": in_base,
                        "preimage_ref": preimage_ref,
                        "preimage_sha256": expected_preimage,
                    }
                )

        per_batch.append(
            {
                "batch_id": spec.batch_id,
                "registry": spec.registry,
                "package_ref": spec.package_ref,
                "package_sha256": package_sha256[spec.batch_id],
                "row_count": len(rows),
                "payload_count": batch_payload_count,
                "payload_bytes": batch_payload_bytes,
                "selected_metadata_paths": len(batch_paths),
                "existing_work_rows": batch_existing_work,
                "existing_work_preimages": batch_preimages,
                "existing_work_record_refs_absent_from_base": batch_missing_existing_work,
                "source_receipts": _batch_receipt_refs(batch_root, spec.package_ref),
                "rights_posture": "preserved_from_acquired_item_metadata; no publication or server-processing expansion",
            }
        )

    (
        dependency_rows,
        dangling_claim_refs,
        topology_dependency_closure,
        dependency_payload_count,
        dependency_payload_bytes,
    ) = _collect_topology_dependencies(
        acquisition_root=acquisition_root,
        specs=specs,
        accepted_paths=accepted_paths,
        candidates=candidates,
        selected_paths=selected_paths,
        claim_rows=claim_rows,
        accepted_claim_ids=accepted_claim_ids,
        payload_root=payload_root,
        claim_sources=direct_claim_sources if direct_handoffs else None,
        source_roots={batch_id: root for batch_id, root in handoff_source_roots}
        if direct_handoffs
        else None,
        payload_roots=handoff_payload_roots if direct_handoffs else None,
    )
    payload_count += dependency_payload_count
    payload_bytes += dependency_payload_bytes

    # Claims are the only selected members which intentionally combine an
    # immutable accepted file with newly acquired rows.
    for claim_path, rows in sorted(claim_rows.items()):
        if claim_path not in claim_entries:
            raise ConversionError(f"relation claim path is absent from accepted base: {claim_path}")
        source = store_root / "objects" / claim_entries[claim_path]["sha256"]
        destination = metadata_root / claim_path
        _copy_with_append(source, destination, rows)
        selected_paths[claim_path] = SourceFile(claim_path, destination, "combined")

    # Relation rows append to the accepted topology files, so every accepted
    # Work/Expression/Edition identity touched by those rows needs its exact
    # outgoing claim-ref successor as well.  The helper starts from the base
    # CAS object and writes only that additive closure.
    record_extensions = _extend_accepted_topology_records(
        store_root=store_root,
        metadata_root=metadata_root,
        accepted_paths=accepted_paths,
        record_entries=record_entries,
        claim_rows=claim_rows,
        selected_paths=selected_paths,
        available_claim_ids=accepted_claim_ids
        | {
            claim_id
            for rows in claim_rows.values()
            for claim_id in (row.get("claim_id") for row in rows)
            if isinstance(claim_id, str)
        },
        dangling_claim_refs=dangling_claim_refs,
    )

    # The owner route permits one legacy topology event.  Preserve its exact
    # accepted object as a CAS-bound preimage while producing a new version
    # whose inputs/outputs/configuration digest the combined candidate.
    topology_event_update = _refresh_topology_event(
        store_root=store_root,
        metadata_root=metadata_root,
        accepted_event_entry=topology_event_entry,
        selected_paths=selected_paths,
        claim_rows=claim_rows,
        topology_ended_at=None,
    )

    # Add the exact discovery closure.  References already accepted remain
    # resolved by the base revision and are not copied into the candidate.
    pending = list(sorted(selected_paths))
    scanned: set[str] = set()
    missing: set[str] = set()
    while pending:
        relative = pending.pop()
        if relative in scanned:
            continue
        scanned.add(relative)
        source = selected_paths[relative].path
        for ref in sorted(_file_refs(source)):
            if ref in accepted_paths or ref in selected_paths:
                continue
            options = candidates.get(ref, [])
            if not options:
                if "/payload/" in ref:
                    # Payload custody is carried in the separate hard-linked
                    # view.  It is intentionally absent from source metadata
                    # updates and is resolved by source_payload_custody.
                    closure_ignored_refs["payload_path"].append(ref)
                    continue
                if _acquisition_directory_ref(
                    acquisition_root,
                    ref,
                    specs,
                    handoff_source_roots=[root for _batch_id, root in handoff_source_roots],
                ):
                    # Item directory refs are valid existence anchors, not
                    # metadata files.  Selected Item companions recreate the
                    # directory in the candidate.
                    closure_ignored_refs["metadata_directory"].append(ref)
                    continue
                missing.add(ref)
                continue
            # If a later batch copied a file, all available candidates must
            # carry the same bytes.  The earliest exact source remains the
            # provenance anchor.
            selected = options[0]
            _select_source(selected_paths, ref, selected)
            pending.append(ref)
    if missing:
        raise ConversionError(
            "acquired metadata has unresolved source references: "
            + ", ".join(sorted(missing)[:20])
            + (f" ... ({len(missing)} total)" if len(missing) > 20 else "")
        )

    # Materialize metadata as hard links where possible.  The three assembled
    # relation files are already newly written and are copied below.
    for relative, source in sorted(selected_paths.items()):
        destination = metadata_root / relative
        if source.batch_id in {"combined", "generated"}:
            continue
        _hardlink(source.path, destination, label="acquired metadata")

    updates: list[dict[str, Any]] = []
    for relative in sorted(selected_paths):
        source = metadata_root / relative
        info = _regular(source, label="candidate metadata")
        mode = stat.S_IMODE(info.st_mode)
        if mode not in (0o644, 0o755):
            raise ConversionError(
                f"candidate metadata has unsupported mode {mode:o}: {relative}"
            )
        updates.append(
            {
                "path": relative,
                "sha256": _sha256(source),
                "size_bytes": source.stat().st_size,
                "mode": mode,
            }
        )

    # Keep the batch manifest strictly in the corpus admission schema.  The
    # human-readable conversion evidence lives in the sibling receipt below.
    from corpus_source_validation import SourceValidator

    validator_options = {"payload_source_root": payload_root}
    if historical_captures is not None:
        validator_options.update(
            historical_capture=historical_captures,
            historical_root=historical_roots,
        )
    validator = SourceValidator(grammar_root, **validator_options)
    if direct_handoffs:
        try:
            import acquisition_handoff_adapter as acquisition_adapter
        except ModuleNotFoundError as exc:
            raise ConversionError(
                "direct handoff intake requires acquisition_handoff_adapter validation-context verifier"
            ) from exc
        expected_history = [
            {
                "capture_ref": str(capture),
                "restored_root_ref": str(restored),
            }
            for capture, restored in zip(historical_captures or [], historical_roots or [])
        ]
        for handoff in direct_handoffs:
            context = handoff.context
            if context is None:
                continue
            try:
                bound_context = acquisition_adapter.verify_validation_context(
                    context,
                    validator_sha256=validator.sha256,
                )
            except Exception as exc:
                raise ConversionError(f"direct handoff validation context rejected: {exc}") from exc
            grammar_ref = bound_context.get("grammar_root_ref")
            if not isinstance(grammar_ref, str) or str(Path(grammar_ref).absolute()) != str(grammar_root):
                raise ConversionError("direct handoff grammar root differs from selected validator context")
            actual_history = [
                {
                    "capture_ref": row.get("capture_ref"),
                    "restored_root_ref": row.get("restored_root_ref"),
                }
                for row in bound_context.get("historical_evidence", [])
                if isinstance(row, dict)
            ]
            if actual_history != expected_history:
                raise ConversionError("direct handoff historical context differs from selected evidence")
    batch = {
        "schema_version": BATCH_SCHEMA,
        "base_revision": base_revision,
        "validator_sha256": validator.sha256,
        "updates": updates,
        "retirements": [],
    }
    batch_ref = f"manifests/tos-corpus-batch-{selection['selection_id']}.json"
    _write_canonical(output_root / batch_ref, batch)
    selection_path: Path | None = None
    if not direct_handoffs:
        selection_record = {
            "schema_version": "tos_acquired_batch_selection_v1",
            "selection_id": selection["selection_id"],
            "base_revision": base_revision,
            "batches": [
                {
                    "batch_id": spec.batch_id,
                    "registry": spec.registry,
                    "package_ref": spec.package_ref,
                    "package_sha256": package_sha256[spec.batch_id],
                }
                for spec in specs
            ],
        }
        selection_path = output_root / "receipts" / "batch-selection.json"
        _write_canonical(selection_path, selection_record)

    receipt = {
        "schema_version": "tos_acquired_corpus_conversion_receipt_v1",
        "base_revision": base_revision,
        "candidate_batch_ref": batch_ref,
        "candidate_batch_sha256": _sha256(output_root / batch_ref),
        "validator_sha256": validator.sha256,
        "source_input_root": "source",
        "payload_source_root": "payload",
        "batch_ids": (
            [str(item.handoff.get("batch_id")) for item in direct_handoffs]
            if direct_handoffs
            else [spec.batch_id for spec in specs]
        ),
        "batch_count": len(direct_handoffs) if direct_handoffs else len(specs),
        "row_count": sum(item["row_count"] for item in per_batch),
        "metadata_update_count": len(updates),
        "payload_count": payload_count,
        "payload_bytes": payload_bytes,
        "claim_rows_added": {path: len(rows) for path, rows in sorted(claim_rows.items())},
        "topology_dependency_closure": topology_dependency_closure,
        "input_kind": "acquired-not-admitted-handoff" if direct_handoffs else "prepared-not-acquired-selection",
        "batch_selection_ref": (
            str(selection_path.relative_to(output_root)) if selection_path is not None else None
        ),
        "batch_selection_sha256": _sha256(selection_path) if selection_path is not None else None,
        "handoff_selection_ref": (
            str(handoff_selection) if direct_handoffs and handoff_selection is not None else None
        ),
        "handoff_selection_sha256": (
            _sha256(handoff_selection) if direct_handoffs and handoff_selection is not None else None
        ),
        "handoffs": [
            {
                "root": str(item.root),
                "handoff_ref": item.handoff_ref,
                "handoff_sha256": item.handoff_sha256,
                "batch_id": item.handoff.get("batch_id"),
                "manifest_sha256": _sha256(item.manifest_path),
            }
            for item in direct_handoffs
        ],
        "record_extension_count": len(record_extensions),
        "record_extensions": record_extensions,
        "topology_event_update": topology_event_update,
        "existing_work_rows": len(existing_work),
        "existing_work_record_refs_absent_from_base": sum(
            1 for item in existing_work if not item["record_ref_in_base"]
        ),
        "existing_work_bindings": existing_work,
        "batches": per_batch,
        "rights_posture": {
            "visibility": "preserved per acquired Item metadata; local_only remains local_only",
            "server_processing": "not authorized unless exact acquired rights record says otherwise",
            "publication": "not performed",
        },
        "historical_evidence": {
            "captures": [str(path) for path in historical_captures or []],
            "restored_roots": [str(path) for path in historical_roots or []],
            "selection_explicit": historical_captures is not None,
            "validator_sha256": validator.sha256,
            "topology_before_copied": False,
            "work_before_copied_only_when_bound_by_existing_work": True,
            "accepted_base_preserved": True,
        },
        "closure_ignored_refs": {
            kind: {"count": len(refs), "first": sorted(set(refs))[:20]}
            for kind, refs in sorted(closure_ignored_refs.items())
        },
    }
    _write_canonical(output_root / "receipts" / "conversion-receipt.json", receipt)
    return receipt


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--acquisition-root", type=Path)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--store", type=Path, required=True)
    parser.add_argument("--base-revision", required=True)
    parser.add_argument("--grammar-root", type=Path, required=True)
    parser.add_argument("--batch-selection", type=Path)
    parser.add_argument(
        "--handoff-selection",
        type=Path,
        help="explicit JSON selector for one or more acquired-not-admitted handoffs",
    )
    parser.add_argument("--historical-capture", type=Path, action="append")
    parser.add_argument("--historical-root", type=Path, action="append")
    args = parser.parse_args(argv)
    try:
        receipt = convert(
            acquisition_root=args.acquisition_root,
            output_root=args.output_root,
            store_root=args.store,
            base_revision=args.base_revision,
            grammar_root=args.grammar_root,
            batch_selection=args.batch_selection,
            handoff_selection=args.handoff_selection,
            historical_capture=args.historical_capture,
            historical_root=args.historical_root,
        )
    except (ConversionError, OSError, ValueError) as exc:
        print(f"conversion rejected: {exc}", file=sys.stderr)
        return 2
    print(_canonical(receipt).decode("utf-8"), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
