"""Bounded COW candidates, never selected-root or source publication.

The caller independently admits the baseline and retains its parts. This module
does not establish source pairing, closure availability, epochs or authority.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
import stat
import uuid
from typing import Any

from .projection_store import (
    ProjectionReader, ProjectionStoreError, MAX_ROOT_BYTES, MAX_INDEX_BYTES,
    MAX_PART_BYTES, DEFAULT_PART_BYTES, INDEX_FORMAT, canonical_bytes,
    _strict_json, _digest, _key, _record_key, _set_collection, _gzip,
)
from .projection_diff import _Reader as _CheckedReader


class ProjectionMutationError(ProjectionStoreError):
    """No complete candidate was established; immutable leftovers may remain."""


class ProjectionMutationBudgetExceeded(ProjectionMutationError):
    pass


class ProjectionMutationRequiresBootstrap(ProjectionMutationError):
    pass


MISSING = object()


@dataclass(frozen=True)
class ProjectionChange:
    collection: str
    key: str
    before_present: bool
    before_sha256: str | None
    after_present: bool
    after_value: Any = MISSING


@dataclass(frozen=True)
class ProjectionHeaderChange:
    before_sha256: str
    after_value: dict


@dataclass(frozen=True)
class MutationLimits:
    max_changes: int = 4096
    max_input_bytes: int = 16 * 1024 * 1024
    max_opened_parts: int = 256
    max_stored_read_bytes: int = 16 * 1024 * 1024
    max_decoded_bytes: int = 16 * 1024 * 1024
    max_keys: int = 4096
    max_written_parts: int = 256
    max_written_decoded_bytes: int = 16 * 1024 * 1024
    max_written_stored_bytes: int = 16 * 1024 * 1024
    max_result_bytes: int = 16 * 1024 * 1024

    def __post_init__(self):
        if any(type(v) is not int or v < 0 for v in vars(self).values()):
            raise ValueError("mutation limits must be nonnegative integers")


class _Budget:
    def __init__(self, limits):
        self.limits = limits
        self.usage = {name.removeprefix("max_"): 0 for name in vars(limits)}

    def take(self, **amounts):
        for name, amount in amounts.items():
            if self.usage[name] + amount > getattr(self.limits, "max_" + name):
                raise ProjectionMutationBudgetExceeded("projection mutation budget exceeded: " + name)
        for name, amount in amounts.items():
            self.usage[name] += amount

    def reserve(self, *, parts=0, decoded=0, keys=0):
        self.take(opened_parts=parts, decoded_bytes=decoded, keys=keys)


class _MutationReader(_CheckedReader):
    def _load(self, descriptor, prefix):
        self._descriptor(descriptor, prefix)
        self.budget.take(stored_read_bytes=descriptor["size_bytes"] + 1)
        return super()._load(descriptor, prefix)

    def verify_binding(self):
        self.verify_current()


class _SnapshotMutationReader(_MutationReader):
    def __init__(self, snapshot, binding, budget):
        self.budget = budget
        self.path = snapshot.namespace_path
        self._root_bytes = snapshot.root_bytes
        self._binding = binding
        # These bytes already exist; reserve their actual size before decoding.
        # The namespace path resolves parts only, never a selected root file.
        budget.reserve(decoded=len(self._root_bytes))
        self.verify_binding()
        self._initialize_manifest(0)

    def verify_binding(self):
        if _digest(self._root_bytes) != self._binding:
            raise ProjectionMutationError("immutable projection binding differs")


class _ViewReader(ProjectionReader):
    def __init__(self, root_bytes, namespace_path):
        self.path = namespace_path
        self._root_bytes = root_bytes
        self._initialize_manifest(0)

    def require_current(self):
        raise ProjectionMutationError("immutable candidate view is not a selected current root")


@dataclass(frozen=True)
class ProjectionSnapshotView:
    """Immutable bytes in an existing namespace, not a selected-file reader.

    It intentionally is not a ProjectionReader and cannot enter diff_projections
    or selected-file mutation as a purported currently selected publication.
    Explicit snapshot staging preserves this nonpublication boundary.
    """
    root_bytes: bytes
    namespace_path: Path

    def __post_init__(self):
        if type(self.root_bytes) is not bytes or len(self.root_bytes) > MAX_ROOT_BYTES:
            raise ProjectionMutationError("immutable root bytes exceed format bound")
        object.__setattr__(self, "namespace_path", Path(self.namespace_path).absolute())
        _ViewReader(self.root_bytes, self.namespace_path)

    @property
    def snapshot_digest(self):
        return _digest(self.root_bytes)

    def metadata(self):
        return _ViewReader(self.root_bytes, self.namespace_path).metadata()

    def lookup(self, collection, key):
        """Return explicit presence and value, including a present JSON null."""
        reader = _ViewReader(self.root_bytes, self.namespace_path)
        key = _key(key)
        if collection not in reader.manifest["collections"]:
            raise ProjectionMutationError("unknown projection collection")
        descriptor = reader.manifest["collections"][collection]["root"]
        prefix, hashed = "", _digest(key.encode("utf-8"))
        while descriptor["kind"] == "index":
            digit = hashed[len(prefix)]
            children = reader._children(descriptor, prefix)
            if digit not in children:
                return {"present": False}
            descriptor, prefix = children[digit], prefix + digit
        rows = dict(reader._rows(collection, descriptor, prefix))
        return {"present": True, "value": rows[key]} if key in rows else {"present": False}

    def iter_items(self, collection):
        return _ViewReader(self.root_bytes, self.namespace_path).iter_items(collection)

    def materialize(self):
        """Explicit potentially complete export, never used by mutation."""
        return _ViewReader(self.root_bytes, self.namespace_path).materialize()

    def require_current(self):
        raise ProjectionMutationError("immutable candidate view is not a selected current root")


@dataclass(frozen=True)
class ProjectionCandidate:
    namespace_path: Path
    before_sha256: str
    root_bytes: bytes
    delta_bytes: bytes
    created_parts: tuple[str, ...]
    accounting: tuple[tuple[str, int], ...]
    published: bool = False
    establishes_epoch: bool = False
    target_closure_verified: bool = False

    @property
    def after_sha256(self):
        return _digest(self.root_bytes)

    def snapshot(self):
        return ProjectionSnapshotView(self.root_bytes, self.namespace_path)

    def delta(self):
        return _strict_json(self.delta_bytes)


def _sha(value):
    if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
        raise ProjectionMutationError("exact lowercase SHA-256 required")
    return value


def _json_bytes(value, maximum):
    # Refuse Python coercions (tuple, non-string dict keys, custom objects).
    # A minimum-byte walk stops oversized containers/strings before an encoder
    # or type-validation pass can traverse an arbitrarily large caller input.
    minimum = 1  # canonical trailing newline

    def charge(size):
        nonlocal minimum
        minimum += size
        if minimum > maximum:
            raise ProjectionMutationBudgetExceeded("bounded JSON input exceeded")

    def check(item, depth=0):
        if depth > 128:
            raise ProjectionMutationError("mutation JSON nesting exceeds 128 levels")
        if type(item) is dict:
            charge(2 + max(0, len(item) - 1) + len(item))
            for key, child in item.items():
                if type(key) is not str:
                    raise ProjectionMutationError("JSON object keys must be strings")
                charge(len(key) + 2)
                check(child, depth + 1)
        elif type(item) is list:
            charge(2 + max(0, len(item) - 1))
            for child in item:
                check(child, depth + 1)
        elif type(item) is str:
            charge(len(item) + 2)
        elif type(item) not in (str, int, float, bool, type(None)):
            raise ProjectionMutationError("exact JSON value required")
        else:
            charge(1)
    check(value)
    result = bytearray()
    encoder = json.JSONEncoder(ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    for chunk in encoder.iterencode(value):
        for offset in range(0, len(chunk), 4096):
            piece = chunk[offset:offset + 4096].encode("utf-8")
            if len(result) + len(piece) + 1 > maximum:
                raise ProjectionMutationBudgetExceeded("bounded JSON encoding exceeded")
            result.extend(piece)
    if len(result) + 1 > maximum:
        raise ProjectionMutationBudgetExceeded("bounded JSON encoding exceeded")
    return bytes(result) + b"\n"


def _install(path, raw, budget):
    """No replace, pinned no-follow namespace, bounded collision comparison."""
    parent = path.parent
    namespace = parent.parent
    # The selected root parent is caller-owned. Same-UID hostile path swaps are
    # outside the cooperating writer contract; never follow namespace symlinks.
    base_fd = os.open(namespace.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    namespace_fd = directory_fd = None
    temporary_name = None
    try:
        try:
            os.mkdir(namespace.name, mode=0o700, dir_fd=base_fd)
        except FileExistsError:
            pass
        namespace_fd = os.open(namespace.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=base_fd)
        try:
            os.mkdir(parent.name, mode=0o700, dir_fd=namespace_fd)
        except FileExistsError:
            pass
        directory_fd = os.open(parent.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=namespace_fd)

        def existing():
            info = os.stat(path.name, dir_fd=directory_fd, follow_symlinks=False)
            if not stat.S_ISREG(info.st_mode) or info.st_size != len(raw):
                raise ProjectionMutationError("immutable destination conflicts with candidate")
            budget.take(opened_parts=1, stored_read_bytes=len(raw) + 1)
            fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=directory_fd)
            with os.fdopen(fd, "rb") as stream:
                if stream.read(len(raw) + 1) != raw:
                    raise ProjectionMutationError("immutable destination bytes differ")
                os.fsync(stream.fileno())

        try:
            existing()
            created = False
        except FileNotFoundError:
            # Random exclusive temporary basename is not a publication name.
            temporary_name = ".cow-" + uuid.uuid4().hex
            fd = os.open(temporary_name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                         0o600, dir_fd=directory_fd)
            with os.fdopen(fd, "wb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
            try:
                os.link(temporary_name, path.name, src_dir_fd=directory_fd,
                        dst_dir_fd=directory_fd, follow_symlinks=False)
                created = True
            except FileExistsError:
                existing()
                created = False
            os.unlink(temporary_name, dir_fd=directory_fd)
            temporary_name = None
        for fd in (directory_fd, namespace_fd, base_fd):
            os.fsync(fd)
        return created
    finally:
        if temporary_name is not None and directory_fd is not None:
            os.unlink(temporary_name, dir_fd=directory_fd)
        for fd in (directory_fd, namespace_fd, base_fd):
            if fd is not None:
                os.close(fd)


def stage_projection_changes(before: ProjectionReader, *, expected_before_sha256: str,
                             trusted_baseline_sha256: str, changes,
                             header_change: ProjectionHeaderChange | None = None,
                             limits: MutationLimits | None = None,
                             target_part_bytes: int = DEFAULT_PART_BYTES) -> ProjectionCandidate:
    """Stage a complete candidate in the original namespace, or raise.

    No root replacement, namespace scan/prune, full-input staging, source epoch,
    authority or implicit bootstrap. Caller owns baseline trust and part lifetime.
    Limits charge prospective unique part writes even when content already exists.
    A failure may retain unselected immutable parts, never a partial candidate.
    """
    if not isinstance(before, ProjectionReader) or isinstance(before, _ViewReader):
        raise TypeError("an explicitly selected ProjectionReader is required")
    return _stage_from(_MutationReader, before, expected_before_sha256,
                       trusted_baseline_sha256, changes, header_change, limits,
                       target_part_bytes)


def stage_projection_snapshot_changes(before: ProjectionSnapshotView, *,
                                      expected_before_sha256: str,
                                      trusted_baseline_sha256: str, changes,
                                      header_change: ProjectionHeaderChange | None = None,
                                      limits: MutationLimits | None = None,
                                      target_part_bytes: int = DEFAULT_PART_BYTES) -> ProjectionCandidate:
    """Stage from exact immutable root bytes without reading a selected root.

    The caller owns baseline trust, part retention, selection and publication
    CAS. This function establishes none of them. It checks only the supplied
    byte binding and touched parts, using the selected route's COW core/budgets.
    The namespace root file may be absent or have unrelated selected bytes.
    """
    if not isinstance(before, ProjectionSnapshotView):
        raise TypeError("an explicit ProjectionSnapshotView is required")
    return _stage_from(_SnapshotMutationReader, before, expected_before_sha256,
                       trusted_baseline_sha256, changes, header_change, limits,
                       target_part_bytes)


def _stage_from(reader_type, before, binding, trust, changes, header_change, limits, target):
    if limits is not None and not isinstance(limits, MutationLimits):
        raise TypeError("limits must be MutationLimits")
    if type(target) is not int or not 256 <= target <= MAX_PART_BYTES:
        raise ProjectionMutationError("invalid target partition size")
    if _sha(binding) != _sha(trust):
        raise ProjectionMutationError("trusted baseline differs from selected binding")
    budget = _Budget(limits or MutationLimits())
    try:
        reader = reader_type(before, binding, budget)
        return _stage(reader, binding, trust, changes, header_change, target, budget)
    except ProjectionMutationError:
        raise
    except (ProjectionStoreError, TypeError, ValueError, UnicodeError, RecursionError) as error:
        raise ProjectionMutationError(str(error)) from error


def _stage(reader, binding, trust, changes, header_change, target, budget):
    manifest = _strict_json(reader._root_bytes)
    for spec in manifest["collections"].values():
        field, order = spec["key_field"], spec["order_fields"]
        keys = field if isinstance(field, list) else [field]
        if field is not None and order and not set(keys) <= set(order):
            raise ProjectionMutationRequiresBootstrap("requires-bootstrap: total declared key order required")
    header_frame = None
    if header_change is not None:
        if not isinstance(header_change, ProjectionHeaderChange):
            raise ProjectionMutationError("explicit header change required")
        if _sha(header_change.before_sha256) != _digest(canonical_bytes(manifest["header"])):
            raise ProjectionMutationError("header precondition differs")
        raw = _json_bytes(header_change.after_value, min(MAX_ROOT_BYTES, budget.limits.max_input_bytes))
        budget.take(input_bytes=len(raw))
        header = _strict_json(raw)
        if not isinstance(header, dict) or type(header.get("schema_version")) is not str:
            raise ProjectionMutationError("logical header object and schema required")
        if header["schema_version"] != manifest["logical_schema"]:
            raise ProjectionMutationRequiresBootstrap("requires-bootstrap: logical schema changed")
        header_check = _strict_json(raw)
        for name in manifest["collections"]:
            _set_collection(header_check, name, None)
        header_frame = {"before_sha256": header_change.before_sha256,
                        "after_sha256": _digest(raw), "after": header}
        manifest["header"] = header
    grouped, frames = {}, []
    for change in changes:
        budget.take(changes=1)
        if not isinstance(change, ProjectionChange):
            raise ProjectionMutationError("explicit ProjectionChange required")
        name, key = change.collection, _key(change.key)
        if type(name) is not str or name not in manifest["collections"]:
            raise ProjectionMutationError("unknown projection collection")
        group = grouped.setdefault(name, {})
        if key in group:
            raise ProjectionMutationError("duplicate mutation target")
        if type(change.before_present) is not bool or type(change.after_present) is not bool:
            raise ProjectionMutationError("presence flags must be booleans")
        if change.before_present:
            _sha(change.before_sha256)
        elif change.before_sha256 is not None:
            raise ProjectionMutationError("absent before state cannot carry a digest")
        if not change.after_present and change.after_value is not MISSING:
            raise ProjectionMutationError("absent after state cannot carry a value")
        if not change.before_present and not change.after_present:
            raise ProjectionMutationError("absent-to-absent is not a mutation")
        frame = {"collection": name, "key": key,
                 "before": {"present": change.before_present, "sha256": change.before_sha256},
                 "after": {"present": change.after_present, "sha256": None}}
        if change.after_present:
            raw = _json_bytes(change.after_value, min(MAX_PART_BYTES, budget.limits.max_input_bytes))
            value = _strict_json(raw)
            field = manifest["collections"][name]["key_field"]
            if field is not None and (not isinstance(value, dict) or _record_key(value, field) != key):
                raise ProjectionMutationError("replacement key differs from addressed identity")
            frame["after"].update(sha256=_digest(raw), value=value)
            # Row framing is part of the hard leaf bound, never omit a field.
            _json_bytes({"key": key, "value": value}, MAX_PART_BYTES)
        frozen = _json_bytes(frame, budget.limits.max_input_bytes)
        budget.take(input_bytes=len(frozen))
        group[key] = frame
        frames.append(frame)

    parts = {}

    def emit(raw, kind, prefix, count):
        if len(raw) > (MAX_PART_BYTES if kind == "data" else MAX_INDEX_BYTES):
            raise ProjectionMutationBudgetExceeded("output part exceeds format bound")
        stored = _gzip(raw) if kind == "data" else raw
        digest = _digest(stored)
        suffix = ".jsonl.gz" if kind == "data" else ".index.json"
        relative = Path(reader.path.stem + ".parts") / digest[:2] / (digest + suffix)
        if relative not in parts:
            budget.take(written_parts=1, written_decoded_bytes=len(raw), written_stored_bytes=len(stored))
            parts[relative] = stored
        return {"kind": kind, "prefix": prefix, "path": relative.as_posix(),
                "sha256": digest, "size_bytes": len(stored), "decoded_bytes": len(raw),
                "decoded_sha256": _digest(raw), "count": count}

    def index(children, prefix):
        count = sum(child["count"] for child in children.values())
        raw = _json_bytes({"schema_version": INDEX_FORMAT, "prefix": prefix,
                           "count": count, "children": children}, MAX_INDEX_BYTES)
        return emit(raw, "index", prefix, count)

    def partition(rows, prefix):
        encoded = [canonical_bytes({"key": key, "value": value}) for key, value in sorted(rows.items())]
        size = sum(map(len, encoded))
        if len(rows) <= 1 or len(prefix) >= 2 and size <= target:
            if size > MAX_PART_BYTES:
                raise ProjectionMutationBudgetExceeded("output leaf exceeds format bound")
            return emit(b"".join(encoded), "data", prefix, len(rows))
        if len(prefix) >= 64:
            raise ProjectionMutationError("record hashes exceed bounded radix depth")
        branches = {}
        for key, value in rows.items():
            branches.setdefault(_digest(key.encode())[len(prefix)], {})[key] = value
        return index({digit: partition(child, prefix + digit) for digit, child in sorted(branches.items())}, prefix)

    def walk(name, descriptor, prefix, updates):
        if descriptor is not None and descriptor["kind"] == "index":
            children = reader._children(descriptor, prefix)
            groups = {}
            for key, frame in updates.items():
                groups.setdefault(_digest(key.encode())[len(prefix)], {})[key] = frame
            for digit, group in sorted(groups.items()):
                child = walk(name, children.get(digit), prefix + digit, group)
                if child is None:
                    children.pop(digit, None)
                else:
                    children[digit] = child
            if not children:
                return partition({}, "") if not prefix else None
            return index(children, prefix)
        rows = {} if descriptor is None else dict(reader._rows(name, descriptor, prefix))
        for key, frame in updates.items():
            present = key in rows
            if present != frame["before"]["present"] or (present and
                    _digest(canonical_bytes(rows[key])) != frame["before"]["sha256"]):
                raise ProjectionMutationError("record precondition differs: " + key)
        for key, frame in updates.items():
            if frame["after"]["present"]:
                rows[key] = frame["after"]["value"]
            else:
                del rows[key]
        if not rows and prefix:
            return None
        return partition(rows, prefix)

    for name, updates in sorted(grouped.items()):
        spec = manifest["collections"][name]
        spec["root"] = walk(name, spec["root"], "", updates)
    root_bytes = _json_bytes(manifest, MAX_ROOT_BYTES)
    ProjectionSnapshotView(root_bytes, reader.path)
    delta = {"schema_version": "tos_projection_mutation_candidate_v1", "complete": True,
             "before_sha256": binding, "after_sha256": _digest(root_bytes),
             "baseline_trust": {"sha256": trust, "asserted_by": "caller", "established_by_mutation": False},
             "published": False, "establishes_epoch": False, "target_closure_verified": False,
             "header_change": header_frame,
             "changes": sorted(frames, key=lambda frame: (frame["collection"], frame["key"]))}
    delta_bytes = _json_bytes(delta, budget.limits.max_result_bytes)
    budget.take(result_bytes=len(root_bytes) + len(delta_bytes))
    reader.verify_binding()
    created = []
    for relative, raw in parts.items():
        if _install(reader.path.parent / relative, raw, budget):
            created.append(relative.as_posix())
    reader.verify_binding()
    return ProjectionCandidate(reader.path, binding, root_bytes, delta_bytes,
                               tuple(created), tuple(sorted(budget.usage.items())))
