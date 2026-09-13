"""Pure exact-source target projections for normalized inspection packets.

Inspection can expose an exact owner target already carried by a normalized
item, but it cannot discover a path, resolve ``latest``, or ask the source
owner to read anything.  The owner-bound SourceReadService rechecks each
target before issuing a handle.
"""
from __future__ import annotations

import re
from typing import Any, Iterable

from .source_read import exact_target_from_record


_NORMALIZATION_TRANSFORM = "tos-knowledge-normalization-v2"
_BARE_DIGEST = re.compile(r"[a-f0-9]{64}\Z")


def source_read_target_for_item(item: Any) -> dict[str, Any] | None:
    """Return one exact target from an explicitly retained raw carrier.

    Only ``source_record.payload.properties.source_record`` and
    ``source_record.payload.properties.source_claim`` are accepted.  A
    ``claim_ref`` or a source path by itself is intentionally insufficient.
    If both source families are present, the carrier is ambiguous and yields
    no target.
    """
    if type(item) is not dict:
        return None
    envelope = item.get("source_record")
    if type(envelope) is not dict or set(envelope) != {
        "payload", "digest", "transform_version", "field_map"
    }:
        return None
    if (
        type(envelope.get("payload")) is not dict
        or type(envelope.get("digest")) is not str
        or _BARE_DIGEST.fullmatch(envelope["digest"]) is None
        or envelope.get("transform_version") != _NORMALIZATION_TRANSFORM
        or type(envelope.get("field_map")) is not dict
        or any(type(key) is not str or type(value) is not str
               for key, value in envelope["field_map"].items())
    ):
        return None
    payload = envelope["payload"]
    properties = payload.get("properties")
    if type(properties) is not dict:
        return None
    raw_metadata = properties.get("source_record")
    raw_claim = properties.get("source_claim")
    if raw_metadata is not None and raw_claim is not None:
        return None
    if raw_claim is not None:
        return exact_target_from_record(raw_claim, layer="claim_record")
    if raw_metadata is not None:
        return exact_target_from_record(raw_metadata, layer="metadata_record")
    return None


def source_read_targets(items: Iterable[Any], source_revision: Any) -> dict[str, dict[str, Any]]:
    """Build a deterministic exact-id target map for returned material.

    Duplicate returned IDs with different source carriers are omitted.  The
    map is a target projection only; its ``source_revision`` is the packet's
    selected snapshot revision and does not claim that an owner reader is
    configured or that the target is currently available.
    """
    if type(source_revision) is not str or _BARE_DIGEST.fullmatch(source_revision) is None:
        return {}
    seen: dict[str, dict[str, Any] | None] = {}
    conflicts: set[str] = set()
    for item in items:
        if type(item) is not dict or type(item.get("id")) is not str or not item["id"]:
            continue
        material_id = item["id"]
        target = source_read_target_for_item(item)
        if material_id in seen:
            if seen[material_id] != target:
                conflicts.add(material_id)
            continue
        seen[material_id] = target
    return {
        material_id: {"source_revision": source_revision, "target": target}
        for material_id, target in sorted(seen.items())
        if target is not None and material_id not in conflicts
    }
