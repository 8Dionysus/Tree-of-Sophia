"""Bounded, complete-or-error logical differences between selected projections.

Equal semantic descriptors reuse the caller's admitted baseline closure. This
does not freshly verify skipped parts, admit a baseline, or establish an epoch.
No source assembly, normalization, graph traversal, or publication occurs here.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib

from .projection_store import (
    ProjectionReader, ProjectionStoreError, MAX_ROOT_BYTES, INDEX_FORMAT,
    canonical_bytes, _strict_json, _key, _record_key,
)


SCHEMA = "tos_projection_diff_v1"


class ProjectionDiffError(ProjectionStoreError):
    """No complete difference was established. No partial packet is returned."""


class ProjectionDiffBudgetExceeded(ProjectionDiffError):
    """An explicit budget was exhausted; retry is the caller's decision."""


class ProjectionDiffRequiresBootstrap(ProjectionDiffError):
    """requires-bootstrap: collection identity or logical version changed."""


@dataclass(frozen=True)
class DiffLimits:
    max_opened_parts: int = 256
    max_decoded_bytes: int = 16 * 1024 * 1024
    max_keys: int = 4096
    max_output_bytes: int = 4 * 1024 * 1024

    def __post_init__(self):
        for value in vars(self).values():
            if type(value) is not int or value < 0:
                raise ValueError("diff limits must be nonnegative integers")


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _value_digest(value):
    # Existing projection JSON encoding, not Python equality (False != 0).
    # Dictionary order is not identity; no source-text normalization is added.
    try:
        return _sha(canonical_bytes(value))
    except (ValueError, TypeError, RecursionError) as error:
        raise ProjectionDiffError("projection value is not bounded canonical JSON") from error


class _Budget:
    def __init__(self, limits):
        self.limits = limits
        self.opened_parts = self.decoded_bytes = self.keys = 0

    def reserve(self, *, parts=0, decoded=0, keys=0):
        proposed = (self.opened_parts + parts, self.decoded_bytes + decoded, self.keys + keys)
        maximum = (self.limits.max_opened_parts, self.limits.max_decoded_bytes, self.limits.max_keys)
        if any(value > limit for value, limit in zip(proposed, maximum)):
            raise ProjectionDiffBudgetExceeded("projection diff read budget exceeded")
        self.opened_parts, self.decoded_bytes, self.keys = proposed


def _root_size(reader):
    # Unlike a stat-only currentness claim, this is only a pre-read size gate.
    if reader.path.is_symlink() or not reader.path.is_file():
        raise ProjectionDiffError("selected root must remain a regular file")
    size = reader.path.stat().st_size
    if size > MAX_ROOT_BYTES:
        raise ProjectionDiffBudgetExceeded("selected root exceeds format budget")
    return size


class _Reader(ProjectionReader):
    def __init__(self, selected, binding, budget):
        self.budget = budget
        _root_size(selected)
        # The existing root reader uses a fixed bounded read (+ sentinel), so
        # reserve that bound even if a concurrent replacement grows the root.
        budget.reserve(decoded=MAX_ROOT_BYTES + 1)
        super().__init__(selected.path, cache_bytes=0)
        if self.snapshot_digest != binding or selected.snapshot_digest != binding:
            raise ProjectionDiffError("selected projection binding changed or differs")

    def _descriptor(self, descriptor, prefix):
        try:
            path = super()._descriptor(descriptor, prefix)
        except (TypeError, KeyError) as error:
            raise ProjectionDiffError("malformed projection part descriptor") from error
        if descriptor["kind"] == "index" and len(prefix) >= 64:
            raise ProjectionDiffError("partition index exceeds key hash depth")
        if descriptor["kind"] == "index" and (
                descriptor["size_bytes"] != descriptor["decoded_bytes"]
                or descriptor["sha256"] != descriptor["decoded_sha256"]):
            raise ProjectionDiffError("uncompressed index stored and decoded identities differ")
        return path

    def _load(self, descriptor, prefix):
        self._descriptor(descriptor, prefix)
        # Reserve before any part open/read/decompression. The parent also
        # prechecks the physical stored size against its bounded descriptor.
        self.budget.reserve(parts=1, decoded=descriptor["decoded_bytes"] + 1,
                            keys=descriptor["count"] if descriptor["kind"] == "data" else 0)
        return super()._load(descriptor, prefix)

    def _children(self, descriptor, prefix):
        index = _strict_json(self._load(descriptor, prefix))
        if (not isinstance(index, dict) or set(index) != {"schema_version", "prefix", "count", "children"}
                or index["schema_version"] != INDEX_FORMAT or index["prefix"] != prefix
                or type(index["count"]) is not int or index["count"] != descriptor["count"]
                or not isinstance(index["children"], dict) or not index["children"]):
            raise ProjectionDiffError("invalid partition directory")
        total = 0
        for digit, child in index["children"].items():
            if len(digit) != 1 or digit not in "0123456789abcdef":
                raise ProjectionDiffError("invalid partition branch")
            self._descriptor(child, prefix + digit)
            total += child["count"]
        if total != descriptor["count"]:
            raise ProjectionDiffError("partition directory count mismatch")
        return index["children"]

    def _rows(self, name, descriptor, prefix):
        raw = self._load(descriptor, prefix)
        previous, count = None, 0
        field = self.manifest["collections"][name]["key_field"]
        for line in raw.splitlines():
            # A lying count must not let arbitrary extra keys be decoded after
            # the pre-read reservation. Refuse before parsing the excess row.
            if count >= descriptor["count"]:
                raise ProjectionDiffError("partition row count mismatch")
            record = _strict_json(line)
            if not isinstance(record, dict) or set(record) != {"key", "value"}:
                raise ProjectionDiffError("invalid partition row")
            key = _key(record["key"])
            if (previous is not None and key <= previous) or not _sha(key.encode()).startswith(prefix):
                raise ProjectionDiffError("duplicate, unsorted, or misplaced partition key")
            value = record["value"]
            if field is not None and (not isinstance(value, dict) or _record_key(value, field) != key):
                raise ProjectionDiffError("partition key differs from record identity")
            previous, count = key, count + 1
            yield key, value
        if count != descriptor["count"]:
            raise ProjectionDiffError("partition row count mismatch")

    def verify_current(self):
        _root_size(self)
        self.budget.reserve(decoded=MAX_ROOT_BYTES + 1)
        self.require_current()


@dataclass
class _Leaf:
    rows: dict


def diff_projections(before: ProjectionReader, after: ProjectionReader, *,
                     expected_before_sha256: str, expected_after_sha256: str,
                     trusted_baseline_sha256: str,
                     limits: DiffLimits | None = None, include_rows: bool = False) -> dict:
    """Return a complete bounded logical diff, or raise without a partial result.

    Explicit bindings select exact root bytes. ``trusted_baseline_sha256`` is
    the caller's assertion that this exact baseline closure was independently
    admitted; this function does not establish that assertion. Equal subtree
    descriptors (all fields except relocation path) are skipped on that basis.
    Skipped target files need not even exist: this is logical delta evidence,
    NOT target-closure availability/integrity certification or source admission.

    Limits cover both sides together: part opens, declared decoded bytes
    (reserving the fixed format cap for initial/final root reads), leaf keys
    including unchanged keys in opened leaves, and canonical JSON output bytes. No iterator or partial
    result escapes on failure. Rows are all included or all omitted as selected;
    requesting rows never silently downgrades on output-budget exhaustion.
    """
    if not isinstance(before, ProjectionReader) or not isinstance(after, ProjectionReader):
        raise TypeError("two explicitly selected ProjectionReaders are required")
    if type(include_rows) is not bool:
        raise TypeError("include_rows must be boolean")
    bindings = (expected_before_sha256, expected_after_sha256, trusted_baseline_sha256)
    if any(not isinstance(value, str) or len(value) != 64
           or any(c not in "0123456789abcdef" for c in value) for value in bindings):
        raise ProjectionDiffError("exact sha256 bindings and baseline trust are required")
    if trusted_baseline_sha256 != expected_before_sha256:
        raise ProjectionDiffError("trusted baseline differs from selected before binding")
    if limits is not None and not isinstance(limits, DiffLimits):
        raise TypeError("limits must be DiffLimits")
    budget = _Budget(limits or DiffLimits())
    left = _Reader(before, expected_before_sha256, budget)
    right = _Reader(after, expected_after_sha256, budget)
    old, new = left.manifest, right.manifest
    if any(not isinstance(m["logical_schema"], str) or not m["logical_schema"] for m in (old, new)):
        raise ProjectionDiffError("logical schema version must be a nonempty string")
    if old["logical_schema"] != new["logical_schema"]:
        raise ProjectionDiffRequiresBootstrap("requires-bootstrap: logical schema version changed")
    if set(old["collections"]) != set(new["collections"]):
        raise ProjectionDiffRequiresBootstrap("requires-bootstrap: collection set changed")
    for name in old["collections"]:
        for field in ("key_field", "order_fields"):
            if canonical_bytes(old["collections"][name][field]) != canonical_bytes(new["collections"][name][field]):
                raise ProjectionDiffRequiresBootstrap("requires-bootstrap: collection key/order identity changed")

    packet = {"schema_version": SCHEMA, "complete": True,
              "before_sha256": expected_before_sha256, "after_sha256": expected_after_sha256,
              "baseline_trust": {"sha256": trusted_baseline_sha256, "asserted_by": "caller",
                                 "established_by_diff": False},
              "target_closure_verified": False, "establishes_epoch": False,
              "rows_included": include_rows, "header_change": None, "changes": []}
    if _value_digest(old["header"]) != _value_digest(new["header"]):
        packet["header_change"] = {"before_sha256": _value_digest(old["header"]),
                                   "after_sha256": _value_digest(new["header"]),
                                   "before": old["header"], "after": new["header"]}
    output_bytes = len(canonical_bytes(packet))
    if output_bytes > budget.limits.max_output_bytes:
        raise ProjectionDiffBudgetExceeded("projection diff header/output budget exceeded")

    def side(rows, key):
        present = key in rows
        result = {"present": present, "sha256": _value_digest(rows[key]) if present else None}
        if include_rows and present:
            result["row"] = rows[key]
        return result

    def compare_rows(name, a, b):
        nonlocal output_bytes
        for key in sorted(a.keys() | b.keys()):
            # Membership is distinct from mapping null; never use get(None).
            if key in a and key in b and _value_digest(a[key]) == _value_digest(b[key]):
                continue
            change = {"collection": name, "key": key,
                      "operation": "insert" if key not in a else "delete" if key not in b else "replace",
                      "before": side(a, key), "after": side(b, key)}
            # Each item replaces [] or appends after a comma. canonical_bytes
            # includes one newline, so this update is exact.
            output_bytes += len(canonical_bytes(change)) - 1 + bool(packet["changes"])
            if output_bytes > budget.limits.max_output_bytes:
                raise ProjectionDiffBudgetExceeded("projection diff output budget exceeded")
            packet["changes"].append(change)

    def rows(reader, name, descriptor, prefix):
        if isinstance(descriptor, _Leaf):
            return descriptor.rows
        # Consume through the terminal count check before comparing anything.
        return dict(reader._rows(name, descriptor, prefix))

    def branches(reader, name, descriptor, prefix):
        if isinstance(descriptor, dict) and descriptor["kind"] == "index":
            return reader._children(descriptor, prefix)
        result = {}
        for key, value in rows(reader, name, descriptor, prefix).items():
            digit = _sha(key.encode("utf-8"))[len(prefix)]
            result.setdefault(digit, _Leaf({})).rows[key] = value
        return result

    def walk(name, a, b, prefix):
        for reader, descriptor in ((left, a), (right, b)):
            if not isinstance(descriptor, _Leaf):
                reader._descriptor(descriptor, prefix)
        if not isinstance(a, _Leaf) and not isinstance(b, _Leaf):
            identity_a = {k: v for k, v in a.items() if k != "path"}
            identity_b = {k: v for k, v in b.items() if k != "path"}
            if canonical_bytes(identity_a) == canonical_bytes(identity_b):
                return
        a_index = isinstance(a, dict) and a["kind"] == "index"
        b_index = isinstance(b, dict) and b["kind"] == "index"
        if a_index or b_index:
            aa, bb = branches(left, name, a, prefix), branches(right, name, b, prefix)
            for digit in sorted(aa.keys() | bb.keys()):
                walk(name, aa.get(digit, _Leaf({})), bb.get(digit, _Leaf({})), prefix + digit)
        else:
            compare_rows(name, rows(left, name, a, prefix), rows(right, name, b, prefix))

    for name in sorted(old["collections"]):
        walk(name, old["collections"][name]["root"], new["collections"][name]["root"], "")
    packet["changes"].sort(key=lambda item: (item["collection"], item["key"]))
    left.verify_current()
    right.verify_current()
    # Final defensive exact check includes all packet framing. Never return a
    # success packet if a budget or freshness check failed earlier.
    if len(canonical_bytes(packet)) > budget.limits.max_output_bytes:
        raise ProjectionDiffBudgetExceeded("projection diff output budget exceeded")
    return packet
