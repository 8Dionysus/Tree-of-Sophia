"""Versioned, bounded, content-addressed storage of a ToS JSON projection.

The root is a small manifest, not the logical JSON document. Query callers
choose a collection or key; materialize() is an explicitly expensive export.
Parts are immutable. A writer publishes the root only after every part exists.
This module has no source, semantic, rights, or runtime admission authority.
"""
from __future__ import annotations

from collections import OrderedDict
from collections.abc import Iterable, Mapping
from dataclasses import dataclass
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import sqlite3
import tempfile
from typing import Any


FORMAT = "tos_partitioned_projection_v1"
INDEX_FORMAT = "tos_projection_partition_index_v1"
MAX_ROOT_BYTES = 256 * 1024
MAX_INDEX_BYTES = 128 * 1024
MAX_PART_BYTES = 8 * 1024 * 1024
MAX_KEY_BYTES = 4096
DEFAULT_PART_BYTES = 1024 * 1024
DEFAULT_CACHE_BYTES = 16 * 1024 * 1024
_HEX = re.compile(r"[0-9a-f]{64}\Z")
_COLLECTION = re.compile(r"[a-z][a-z0-9_]*(?:/[a-z][a-z0-9_]*)*\Z")


class ProjectionStoreError(ValueError):
    """A projection part, manifest, identity, or declared bound is invalid."""


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True,
                       separators=(",", ":"), allow_nan=False) + "\n").encode("utf-8")


def _digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _strict_json(raw: bytes) -> Any:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ProjectionStoreError(f"duplicate JSON member: {key}")
            result[key] = value
        return result
    try:
        return json.loads(raw, object_pairs_hook=pairs,
                          parse_constant=lambda _: (_ for _ in ()).throw(
                              ProjectionStoreError("non-finite JSON number")))
    except (ValueError, UnicodeError) as error:
        raise ProjectionStoreError(f"invalid projection JSON: {error}") from error


def _key(value: Any) -> str:
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > MAX_KEY_BYTES:
        raise ProjectionStoreError("projection record key must be a bounded nonempty string")
    return value


def _atomic_write(path: Path, raw: bytes) -> None:
    if path.is_symlink():
        raise ProjectionStoreError(f"refusing symlink output: {path}")
    if path.is_file() and path.read_bytes() == raw:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(prefix=".projection-", dir=path.parent, delete=False) as handle:
        temporary = Path(handle.name)
        try:
            handle.write(raw)
            handle.flush()
            os.fsync(handle.fileno())
        except BaseException:
            temporary.unlink(missing_ok=True)
            raise
    try:
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def _gzip(raw: bytes) -> bytes:
    output = io.BytesIO()
    # GzipFile fixes the filename, timestamp and OS header across hosts.
    with gzip.GzipFile(fileobj=output, mode="wb", filename="", mtime=0, compresslevel=6) as stream:
        stream.write(raw)
    return output.getvalue()


@dataclass(frozen=True)
class Collection:
    """One logical array (or mapping), with a stable per-record identity.

    Array enumeration order is named explicitly and is independent of hash
    partition placement. Mapping rows are (key, value) pairs.
    """
    rows: Iterable
    key_field: str | tuple[str, ...] | list[str] | None
    order_fields: tuple[str, ...] = ()


def _valid_key_field(field):
    return (field is None or isinstance(field, str) and bool(field)
            or isinstance(field, (list, tuple)) and bool(field)
            and all(isinstance(item, str) and item for item in field))


def _record_key(row, field):
    if isinstance(field, (list, tuple)):
        values = [_key(row.get(item)) for item in field]
        return json.dumps(values, ensure_ascii=False, separators=(",", ":"))
    return row.get(field)


def _set_collection(document: dict, name: str, value: Any) -> None:
    current = document
    parts = name.split("/")
    for part in parts[:-1]:
        if part not in current:
            current[part] = {}
        if not isinstance(current[part], dict):
            raise ProjectionStoreError(f"collection overlaps scalar metadata: {name}")
        current = current[part]
    if parts[-1] in current:
        raise ProjectionStoreError(f"collection already present in header: {name}")
    current[parts[-1]] = value


def write_projection(path: Path, header: dict[str, Any], collections: Mapping[str, Collection], *,
                     target_part_bytes: int = DEFAULT_PART_BYTES,
                     work_dir: Path | None = None, prune: bool = False) -> dict[str, Any]:
    """Stage records on disk, emit a radix tree, then atomically publish root.

    Partition placement uses the stable key hash, never an array ordinal.
    Adding a record changes its leaf and ancestor indexes only. A leaf splits
    when its decoded bytes exceed the target; every individual row must also
    fit the hard part limit. No whole-projection JSON string is constructed.
    """
    path = Path(path).absolute()
    if not isinstance(header, dict) or not isinstance(header.get("schema_version"), str):
        raise ProjectionStoreError("logical projection header needs schema_version")
    if not 256 <= target_part_bytes <= MAX_PART_BYTES:
        raise ProjectionStoreError("invalid target partition size")
    if not collections:
        raise ProjectionStoreError("partitioned projection needs collections")
    header = _strict_json(canonical_bytes(header))
    names = sorted(collections)
    for name in names:
        spec = collections[name]
        if (not isinstance(spec.order_fields, (tuple, list))
                or any(not isinstance(field, str) or not field for field in spec.order_fields)):
            raise ProjectionStoreError("invalid collection ordering")
        if not _COLLECTION.fullmatch(name):
            raise ProjectionStoreError(f"invalid collection name: {name}")
    # Detect overlapping collection names and header members before any output.
    header_check = _strict_json(canonical_bytes(header))
    for name in names:
        _set_collection(header_check, name, None)
    part_dir = path.with_name(path.stem + ".parts")
    if part_dir.is_symlink():
        raise ProjectionStoreError("projection part directory cannot be a symlink")
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="tos-projection-stage-", dir=work_dir) as temporary:
        connection = sqlite3.connect(Path(temporary) / "records.sqlite")
        try:
            connection.execute("PRAGMA temp_store=FILE")
            connection.execute("PRAGMA cache_size=-8192")
            connection.execute("CREATE TABLE records (collection TEXT, key TEXT, hash TEXT, body BLOB, "
                               "PRIMARY KEY(collection,key)) WITHOUT ROWID")
            connection.execute("CREATE INDEX placement ON records(collection,hash)")
            for name, spec in collections.items():
                if not _valid_key_field(spec.key_field):
                    raise ProjectionStoreError("invalid key field")
                for row in spec.rows:
                    if spec.key_field is None:
                        key, value = row
                    else:
                        if not isinstance(row, dict):
                            raise ProjectionStoreError(f"{name}: array row is not an object")
                        key, value = _record_key(row, spec.key_field), row
                    key = _key(key)
                    raw = canonical_bytes({"key": key, "value": value})
                    if len(raw) > MAX_PART_BYTES:
                        raise ProjectionStoreError(f"{name}/{key}: individual record exceeds {MAX_PART_BYTES} bytes; "
                                                   "the owner must split the logical record")
                    try:
                        connection.execute("INSERT INTO records VALUES(?,?,?,?)",
                                           (name, key, _digest(key.encode("utf-8")), raw))
                    except sqlite3.IntegrityError as error:
                        raise ProjectionStoreError(f"{name}: duplicate record key {key}") from error
                connection.commit()

            def emit(raw: bytes, kind: str, prefix: str, count: int) -> dict:
                stored = _gzip(raw) if kind == "data" else raw
                digest = _digest(stored)
                suffix = ".jsonl.gz" if kind == "data" else ".index.json"
                destination = part_dir / digest[:2] / (digest + suffix)
                if destination.parent.is_symlink():
                    raise ProjectionStoreError("projection part parent cannot be a symlink")
                _atomic_write(destination, stored)
                return {"kind": kind, "prefix": prefix, "path": destination.relative_to(path.parent).as_posix(),
                        "sha256": digest, "size_bytes": len(stored), "decoded_bytes": len(raw),
                        "decoded_sha256": _digest(raw), "count": count}

            def partition(name: str, prefix: str) -> dict:
                condition = "collection=? AND hash>=? AND hash<?"
                arguments = (name, prefix, prefix + "g")
                count, size = connection.execute(
                    f"SELECT count(*),coalesce(sum(length(body)),0) FROM records WHERE {condition}",
                    arguments).fetchone()
                if count <= 1 or (len(prefix) >= 2 and size <= target_part_bytes):
                    raw = b"".join(row[0] for row in connection.execute(
                        f"SELECT body FROM records WHERE {condition} ORDER BY key", arguments))
                    if len(raw) > MAX_PART_BYTES:
                        raise ProjectionStoreError("partition exceeds hard byte bound")
                    return emit(raw, "data", prefix, count)
                if len(prefix) == 64:
                    raise ProjectionStoreError("distinct record keys collide beyond the hard partition bound")
                children = {}
                for digit in "0123456789abcdef":
                    child = prefix + digit
                    present = connection.execute(
                        "SELECT 1 FROM records WHERE collection=? AND hash>=? AND hash<? LIMIT 1",
                        (name, child, child + "g")).fetchone()
                    if present:
                        children[digit] = partition(name, child)
                raw = canonical_bytes({"schema_version": INDEX_FORMAT, "prefix": prefix,
                                       "count": count, "children": children})
                if len(raw) > MAX_INDEX_BYTES:
                    raise ProjectionStoreError("partition directory exceeds bound")
                return emit(raw, "index", prefix, count)

            result = {"schema_version": FORMAT, "logical_schema": header["schema_version"], "header": header,
                      "limits": {"root_bytes": MAX_ROOT_BYTES, "index_bytes": MAX_INDEX_BYTES,
                                 "part_bytes": MAX_PART_BYTES, "key_bytes": MAX_KEY_BYTES},
                      "collections": {name: {"key_field": collections[name].key_field,
                                             "order_fields": list(collections[name].order_fields),
                                             "root": partition(name, "")} for name in names}}
            raw = canonical_bytes(result)
            if len(raw) > MAX_ROOT_BYTES:
                raise ProjectionStoreError("projection root exceeds bound; move growing metadata into a collection")
            _atomic_write(path, raw)
        finally:
            connection.close()
    reader = ProjectionReader(path)
    if prune:
        # Only generator-owned, correctly named content objects in this exact
        # namespace may be retired. Git retains historical committed snapshots.
        current = set(reader.closure_paths())
        for candidate in part_dir.glob("*/*"):
            if (candidate in current or candidate.parent.is_symlink()
                    or not candidate.is_file() or candidate.is_symlink()):
                continue
            digest = candidate.name.split(".", 1)[0]
            if (_HEX.fullmatch(digest) and candidate.parent.name == digest[:2]
                    and candidate.name in {digest + ".index.json", digest + ".jsonl.gz"}
                    and _digest(candidate.read_bytes()) == digest):
                candidate.unlink()
    return result


class ProjectionReader:
    """Read one immutable manifest snapshot with a byte-bounded part cache."""

    def __init__(self, path: Path, *, cache_bytes: int = DEFAULT_CACHE_BYTES):
        self.path = Path(path).absolute()
        if self.path.is_symlink() or not self.path.is_file():
            raise ProjectionStoreError("projection root must be a regular file")
        if self.path.stat().st_size > MAX_ROOT_BYTES:
            raise ProjectionStoreError("projection root exceeds bound")
        with self.path.open("rb") as stream:
            self._root_bytes = stream.read(MAX_ROOT_BYTES + 1)
        if len(self._root_bytes) > MAX_ROOT_BYTES:
            raise ProjectionStoreError("projection root exceeds bound")
        self.manifest = _strict_json(self._root_bytes)
        root = self.manifest
        if (not isinstance(root, dict) or set(root) != {"schema_version", "logical_schema", "header", "limits", "collections"}
                or root.get("schema_version") != FORMAT or not isinstance(root.get("header"), dict)
                or root.get("logical_schema") != root["header"].get("schema_version")
                or not isinstance(root.get("collections"), dict) or not root["collections"]):
            raise ProjectionStoreError("invalid partitioned projection manifest")
        if root["limits"] != {"root_bytes": MAX_ROOT_BYTES, "index_bytes": MAX_INDEX_BYTES,
                              "part_bytes": MAX_PART_BYTES, "key_bytes": MAX_KEY_BYTES}:
            raise ProjectionStoreError("unknown projection limits")
        header_check = _strict_json(canonical_bytes(root["header"]))
        for name, spec in root["collections"].items():
            if not _COLLECTION.fullmatch(name) or not isinstance(spec, dict) or set(spec) != {"key_field", "order_fields", "root"}:
                raise ProjectionStoreError("invalid collection descriptor")
            if not _valid_key_field(spec['key_field']):
                raise ProjectionStoreError("invalid collection key field")
            if (not isinstance(spec['order_fields'], list)
                    or any(not isinstance(v, str) or not v for v in spec['order_fields'])):
                raise ProjectionStoreError("invalid collection ordering")
            _set_collection(header_check, name, None)
            self._descriptor(spec["root"], "")
        self.snapshot_digest = _digest(self._root_bytes)
        self.cache_bytes = max(0, cache_bytes)
        self._cache: OrderedDict[tuple, bytes] = OrderedDict()
        self._cached_bytes = 0
        self.bytes_read = 0
        self.parts_read = 0

    def metadata(self) -> dict[str, Any]:
        return _strict_json(canonical_bytes(self.manifest["header"]))

    def _descriptor(self, descriptor: Any, prefix: str) -> Path:
        fields = {"kind", "prefix", "path", "sha256", "size_bytes", "decoded_bytes", "decoded_sha256", "count"}
        if not isinstance(descriptor, dict) or set(descriptor) != fields:
            raise ProjectionStoreError("invalid part descriptor")
        kind = descriptor["kind"]
        digest = descriptor["sha256"]
        if (kind not in {"data", "index"} or descriptor["prefix"] != prefix
                or len(prefix) > 64 or any(v not in "0123456789abcdef" for v in prefix)
                or not isinstance(digest, str) or not _HEX.fullmatch(digest)
                or not isinstance(descriptor["decoded_sha256"], str)
                or not _HEX.fullmatch(descriptor["decoded_sha256"])):
            raise ProjectionStoreError("invalid part identity")
        bound = MAX_PART_BYTES if kind == "data" else MAX_INDEX_BYTES
        for field in ("size_bytes", "decoded_bytes", "count"):
            if type(descriptor[field]) is not int or descriptor[field] < 0:
                raise ProjectionStoreError("invalid part count or size")
        if descriptor["decoded_bytes"] > bound or descriptor["size_bytes"] > bound + 65536:
            raise ProjectionStoreError("part exceeds declared format bounds")
        suffix = ".jsonl.gz" if kind == "data" else ".index.json"
        expected = PurePosixPath(self.path.stem + ".parts", digest[:2], digest + suffix).as_posix()
        if descriptor["path"] != expected:
            raise ProjectionStoreError("part path is outside the exact content-addressed namespace")
        return self.path.parent / expected

    def _load(self, descriptor: dict, prefix: str) -> bytes:
        path = self._descriptor(descriptor, prefix)
        cache_key = (descriptor["sha256"], descriptor["kind"], descriptor["size_bytes"],
                     descriptor["decoded_bytes"], descriptor["decoded_sha256"])
        if cache_key in self._cache:
            self._cache.move_to_end(cache_key)
            raw = self._cache[cache_key]
            if len(raw) != descriptor["decoded_bytes"] or _digest(raw) != descriptor["decoded_sha256"]:
                raise ProjectionStoreError("cached part decoded identity mismatch")
            return raw
        if any(p.is_symlink() for p in (path, path.parent, path.parent.parent)) or not path.is_file():
            raise ProjectionStoreError(f"missing or symlink projection part: {path}")
        if path.stat().st_size != descriptor["size_bytes"]:
            raise ProjectionStoreError(f"projection part size mismatch: {path}")
        with path.open("rb") as stream:
            stored = stream.read(descriptor["size_bytes"] + 1)
        if len(stored) != descriptor["size_bytes"]:
            raise ProjectionStoreError("projection part size changed while reading")
        self.bytes_read += len(stored)
        self.parts_read += 1
        if _digest(stored) != descriptor["sha256"]:
            raise ProjectionStoreError(f"projection part digest mismatch: {path}")
        try:
            if descriptor["kind"] == "data":
                with gzip.GzipFile(fileobj=io.BytesIO(stored)) as stream:
                    raw = stream.read(descriptor["decoded_bytes"] + 1)
            else:
                raw = stored
        except (OSError, EOFError) as error:
            raise ProjectionStoreError("invalid compressed projection part") from error
        if len(raw) != descriptor["decoded_bytes"] or _digest(raw) != descriptor["decoded_sha256"]:
            raise ProjectionStoreError("projection part decoded identity mismatch")
        if len(raw) <= self.cache_bytes:
            while self._cache and self._cached_bytes + len(raw) > self.cache_bytes:
                _, old = self._cache.popitem(last=False)
                self._cached_bytes -= len(old)
            self._cache[cache_key] = raw
            self._cached_bytes += len(raw)
        return raw

    def _children(self, descriptor: dict, prefix: str) -> dict:
        index = _strict_json(self._load(descriptor, prefix))
        if (not isinstance(index, dict) or set(index) != {"schema_version", "prefix", "count", "children"}
                or index["schema_version"] != INDEX_FORMAT or index["prefix"] != prefix
                or index["count"] != descriptor["count"] or not isinstance(index["children"], dict)
                or not index["children"] or len(prefix) >= 64):
            raise ProjectionStoreError("invalid partition directory")
        total = 0
        for digit, child in index["children"].items():
            if digit not in "0123456789abcdef" or len(digit) != 1:
                raise ProjectionStoreError("invalid partition branch")
            self._descriptor(child, prefix + digit)
            total += child["count"]
        if total != descriptor["count"]:
            raise ProjectionStoreError("partition directory count mismatch")
        return index["children"]

    def _rows(self, name: str, descriptor: dict, prefix: str):
        raw = self._load(descriptor, prefix)
        count = 0
        previous = None
        spec = self.manifest["collections"][name]
        for line in raw.splitlines():
            record = _strict_json(line)
            if not isinstance(record, dict) or set(record) != {"key", "value"}:
                raise ProjectionStoreError("invalid partition row")
            key = _key(record["key"])
            if (previous is not None and key <= previous) or not _digest(key.encode()).startswith(prefix):
                raise ProjectionStoreError("duplicate, unsorted, or misplaced partition key")
            previous = key
            value = record["value"]
            if spec["key_field"] is not None and (
                    not isinstance(value, dict) or _record_key(value, spec["key_field"]) != key):
                raise ProjectionStoreError("partition key differs from record identity")
            count += 1
            yield key, value
        if count != descriptor["count"]:
            raise ProjectionStoreError("partition row count mismatch")

    def iter_items(self, name: str):
        """Stream verified (stable key, logical value) pairs in partition order."""
        try:
            root = self.manifest["collections"][name]["root"]
        except KeyError as error:
            raise ProjectionStoreError(f"unknown projection collection: {name}") from error
        def visit(descriptor, prefix):
            if descriptor["kind"] == "data":
                yield from self._rows(name, descriptor, prefix)
            else:
                for digit, child in sorted(self._children(descriptor, prefix).items()):
                    yield from visit(child, prefix + digit)
        yield from visit(root, "")

    def iter_collection(self, name: str):
        """Stream array values; mapping collections stream {'key','value'} rows."""
        mapping = self.manifest["collections"][name]["key_field"] is None
        for key, value in self.iter_items(name):
            yield {"key": key, "value": value} if mapping else value

    def get(self, name: str, key: str) -> Any | None:
        key = _key(key)
        hashed = _digest(key.encode())
        descriptor = self.manifest["collections"][name]["root"]
        prefix = ""
        while descriptor["kind"] == "index":
            children = self._children(descriptor, prefix)
            digit = hashed[len(prefix)]
            if digit not in children:
                return None
            descriptor = children[digit]
            prefix += digit
        # Consume the whole selected leaf before returning, checking its count
        # and every key. No unrelated leaf is opened.
        found = None
        for candidate, value in self._rows(name, descriptor, prefix):
            if candidate == key:
                found = value
        return found

    def closure_paths(self, *, verify_data: bool = True):
        """Exact validated manifest closure for packaging, never a file glob."""
        yield self.path
        seen = {self.path}
        def visit(name, descriptor, prefix):
            path = self._descriptor(descriptor, prefix)
            if path not in seen:
                seen.add(path)
                yield path
            if descriptor["kind"] == "index":
                for digit, child in sorted(self._children(descriptor, prefix).items()):
                    yield from visit(name, child, prefix + digit)
            elif verify_data:
                for _ in self._rows(name, descriptor, prefix):
                    pass
        for name, spec in sorted(self.manifest["collections"].items()):
            yield from visit(name, spec["root"], "")

    def materialize(self) -> dict[str, Any]:
        """Explicit whole-document export; never used by an ordinary query."""
        result = self.metadata()
        for name, spec in self.manifest["collections"].items():
            if spec["key_field"] is None:
                value = dict(sorted(self.iter_items(name)))
            else:
                fields = spec["order_fields"] or (spec["key_field"] if isinstance(spec["key_field"], list) else [spec["key_field"]])
                value = sorted((row for _, row in self.iter_items(name)),
                               key=lambda row: tuple(str(row.get(field, "")) for field in fields))
            _set_collection(result, name, value)
        return result

    def require_current(self) -> None:
        if self.path.is_symlink():
            raise ProjectionStoreError("projection snapshot changed during operation")
        with self.path.open("rb") as stream:
            current = stream.read(MAX_ROOT_BYTES + 1)
        if current != self._root_bytes:
            raise ProjectionStoreError("projection snapshot changed during operation")


def is_partitioned(path: Path) -> bool:
    """Small-root probe; a legacy monolith is never read just for detection."""
    path = Path(path)
    if not path.is_file() or path.stat().st_size > MAX_ROOT_BYTES:
        return False
    with path.open("rb") as stream:
        raw = stream.read(MAX_ROOT_BYTES + 1)
    if len(raw) > MAX_ROOT_BYTES:
        return False
    value = _strict_json(raw)
    return isinstance(value, dict) and value.get("schema_version") == FORMAT


def load_projection(path: Path) -> dict[str, Any]:
    """Explicit full export compatibility for source validators and CLI export."""
    if is_partitioned(path):
        return ProjectionReader(path).materialize()
    value = _strict_json(Path(path).read_bytes())
    if not isinstance(value, dict):
        raise ProjectionStoreError("logical projection must be an object")
    return value
