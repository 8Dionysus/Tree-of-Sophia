from __future__ import annotations

import hashlib
import json
import math
import re
import struct
import copy
import calendar
from collections import Counter
from functools import lru_cache
from typing import Any, Iterable
from .normalization_cache import active_cache
from .processing import Input
from .lens_pagination import normalize_pagination, paginate_lens


KNOWLEDGE_SOURCES = (
    "philosophy",
    "canon",
    "candidate-intake",
    "source-navigation",
    "source-claims",
    "semantic-interchange",
    "repository",
)
FILTER_OPERATORS = ("eq", "neq", "in", "contains", "prefix", "exists", "gt", "gte", "lt", "lte")
LAYOUTS = (
    "auto",
    "organic",
    "timeline",
    "flow",
    "evidence",
    "semantic",
    "infrastructure",
    "hierarchical",
    "radial",
    "matrix",
)
MAX_FILTERS = 32
MAX_TRAVERSAL_DEPTH = 5
MAX_NODE_LIMIT = 1000
MAX_RELATION_LIMIT = 2000
MAX_GROUP_LIMIT = 200
OVERVIEW_EXCLUDED_PREDICATES = {"has_text_unit", "has_anchor", "anchored_in", "annotation_member"}
_IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$")
_ATTRIBUTE_FIELD = re.compile(r"^(?:attributes|semantics)\.[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$")
_UNSAFE_PATH_SEGMENTS = {"__proto__", "prototype", "constructor"}

NODE_FIELDS = {
    "id",
    "entity_id",
    "native_id",
    "source_graph",
    "kind_id",
    "type_id",
    "type_mapping.status",
    "type_mapping.source_kind_id",
    "display.title.default",
    "display.title.ru",
    "display.title.en",
    "display.kind_label.default",
    "display.summary.default",
    "display.summary.ru",
    "display.summary.en",
    "display.summary_state",
    "epistemic.authority_layer",
    "epistemic.canon_status",
    "epistemic.review_posture",
    "epistemic.confidence",
    "graph_layers",
    "view_ids",
    "source_refs",
}
RELATION_FIELDS = {
    "id",
    "native_id",
    "source_graph",
    "from_id",
    "to_id",
    "predicate_id",
    "relation_type_id",
    "predicate_mapping.status",
    "predicate_mapping.source_predicate_id",
    "display.label.default",
    "display.label.ru",
    "display.label.en",
    "display.statement.default",
    "display.explanation.default",
    "display.explanation_state",
    "epistemic.authority_layer",
    "epistemic.canon_status",
    "epistemic.review_posture",
    "epistemic.confidence",
    "graph_layers",
    "view_ids",
    "source_refs",
}

ENTITY_REGISTRY_REF = "ToS/doctrine/semantic-interchange/entity-types.v1.json"
RELATION_REGISTRY_REF = "ToS/doctrine/semantic-interchange/relation-types.v1.json"
_TEMPORAL_CLAIM_PREDICATES = {
    "author_received_finished_copies_on",
    "first_publication_chronology",
    "official_publication_on",
    "printing_completed_in",
    "printing_completed_on",
    "public_sale_released_on",
    "title_page_year",
}


def _objects(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _strings(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [str(item) for item in value if isinstance(item, str) and item]


def _registry_items(value: Any, key: str) -> list[dict[str, Any]]:
    if not isinstance(value, dict):
        return []
    return _objects(value.get(key))


def _acyclic_hierarchy(
    entries: dict[str, dict[str, Any]],
    parent_field: str,
    label: str,
) -> list[str]:
    violations: list[str] = []
    states: dict[str, int] = {}

    def visit(identifier: str, trail: tuple[str, ...]) -> None:
        state = states.get(identifier, 0)
        if state == 2:
            return
        if state == 1:
            cycle = " -> ".join((*trail, identifier))
            violations.append(f"{label} hierarchy contains a cycle: {cycle}")
            return
        states[identifier] = 1
        entry = entries[identifier]
        for parent in _strings(entry.get(parent_field)):
            if parent not in entries:
                violations.append(f"{label} {identifier} references missing parent {parent}")
                continue
            visit(parent, (*trail, identifier))
        states[identifier] = 2

    for identifier in sorted(entries):
        visit(identifier, ())
    return violations


def validate_semantic_registries(
    entity_registry: Any,
    relation_registry: Any,
    *, previous_entity_registry: Any = None, previous_relation_registry: Any = None,
) -> dict[str, Any]:
    """Validate hierarchy and crosswalk invariants without claiming semantic review."""

    violations: list[str] = []
    entity_entries_list = _registry_items(entity_registry, "types")
    relation_entries_list = _registry_items(relation_registry, "relations")
    entity_entries: dict[str, dict[str, Any]] = {}
    relation_entries: dict[str, dict[str, Any]] = {}

    for entry in entity_entries_list:
        identifier = _string(entry.get("type_id"))
        if not identifier:
            violations.append("entity registry contains an entry without type_id")
            continue
        if identifier in entity_entries:
            violations.append(f"duplicate entity type_id {identifier}")
        entity_entries[identifier] = entry
    for entry in relation_entries_list:
        identifier = _string(entry.get("relation_type_id"))
        if not identifier:
            violations.append("relation registry contains an entry without relation_type_id")
            continue
        if identifier in relation_entries:
            violations.append(f"duplicate relation_type_id {identifier}")
        relation_entries[identifier] = entry
    for previous, current, entries_key, id_key in ((previous_entity_registry, entity_registry, 'types', 'type_id'),
                                                   (previous_relation_registry, relation_registry, 'relations', 'relation_type_id')):
        if not previous:
            continue
        now = {entry[id_key]: entry for entry in _registry_items(current, entries_key)}
        for entry in _registry_items(previous, entries_key):
            if entry[id_key] not in now:
                violations.append(f"registry removed historical identity {entry[id_key]}; retain a deprecated entry")
        if previous != current and current.get('registry_version', 0) <= previous.get('registry_version', 0):
            violations.append('changed registry must increase registry_version')
    properties_seen: set[str] = set()
    for definition in (entity_registry or {}).get('property_definitions', []):
        identifier = definition.get('property_id')
        if not identifier or identifier in properties_seen:
            violations.append(f'duplicate or missing property_id {identifier}')
        properties_seen.add(identifier)
        if not _allowed_field(str(definition.get('field', '')), 'node'):
            violations.append(f'property {identifier} has an unsupported query field')
        for owner_type in definition.get('applies_to', []):
            if owner_type not in entity_entries:
                violations.append(f'property {identifier} refers to unregistered type {owner_type}')

    fallback_type = _string(entity_registry.get("fallback_type_id")) if isinstance(entity_registry, dict) else None
    fallback_relation = (
        _string(relation_registry.get("fallback_relation_type_id"))
        if isinstance(relation_registry, dict)
        else None
    )
    if fallback_type not in entity_entries:
        violations.append(f"entity fallback {fallback_type!r} is not registered")
    if fallback_relation not in relation_entries:
        violations.append(f"relation fallback {fallback_relation!r} is not registered")

    violations.extend(_acyclic_hierarchy(entity_entries, "parent_type_ids", "entity"))
    violations.extend(
        _acyclic_hierarchy(relation_entries, "parent_relation_type_ids", "relation")
    )
    for entries, field, label in ((entity_entries, "supersedes_type_id", "entity supersession"),
                                   (relation_entries, "supersedes_relation_type_id", "relation supersession")):
        links = {key: {"previous": [value[field]] if value.get(field) else []} for key, value in entries.items()}
        violations.extend(_acyclic_hierarchy(links, "previous", label))
        for key, entry in entries.items():
            if entry.get("abstract") and entry.get("source_mappings"):
                violations.append(f"abstract {label} {key} cannot map source instances")

    entity_mappings: dict[tuple[str, str], str] = {}
    for identifier, entry in entity_entries.items():
        for mapping in _objects(entry.get("source_mappings")):
            key = (_string(mapping.get("source_graph")) or "", _string(mapping.get("source_kind_id")) or "")
            if not all(key):
                violations.append(f"entity mapping on {identifier} is incomplete")
                continue
            prior = entity_mappings.get(key)
            if prior and prior != identifier:
                violations.append(f"entity source mapping {key!r} is owned by both {prior} and {identifier}")
            entity_mappings[key] = identifier

    relation_mappings: dict[tuple[str, str, str], str] = {}
    for identifier, entry in relation_entries.items():
        for endpoint_type in [*_strings(entry.get("domain_type_ids")), *_strings(entry.get("range_type_ids"))]:
            if endpoint_type not in entity_entries:
                violations.append(f"relation {identifier} references missing entity type {endpoint_type}")
        for parent in _strings(entry.get("parent_relation_type_ids")):
            if parent not in relation_entries:
                violations.append(f"relation {identifier} references missing parent {parent}")
        inverse = _string(entry.get("inverse_relation_type_id"))
        if inverse and inverse not in relation_entries:
            violations.append(f"relation {identifier} references missing inverse {inverse}")
        elif inverse:
            reciprocal = _string(relation_entries[inverse].get("inverse_relation_type_id"))
            if reciprocal != identifier:
                violations.append(f"relation inverse {identifier} -> {inverse} is not reciprocal")
        cardinality = entry.get("cardinality") if isinstance(entry.get("cardinality"), dict) else {}
        for minimum_key, maximum_key in (
            ("per_subject_min", "per_subject_max"),
            ("per_object_min", "per_object_max"),
        ):
            minimum = cardinality.get(minimum_key)
            maximum = cardinality.get(maximum_key)
            if isinstance(minimum, int) and isinstance(maximum, int) and minimum > maximum:
                violations.append(f"relation {identifier} has {minimum_key} greater than {maximum_key}")
        for mapping in _objects(entry.get("source_mappings")):
            key = (
                _string(mapping.get("source_graph")) or "",
                _string(mapping.get("source_predicate_id")) or "",
                _string(mapping.get("scope")) or "",
            )
            if not all(key):
                violations.append(f"relation mapping on {identifier} is incomplete")
                continue
            prior = relation_mappings.get(key)
            if prior and prior != identifier:
                violations.append(f"relation source mapping {key!r} is owned by both {prior} and {identifier}")
            relation_mappings[key] = identifier

    return {
        "valid": not violations,
        "violations": sorted(set(violations)),
        "entity_type_count": len(entity_entries),
        "relation_type_count": len(relation_entries),
        "entity_mapping_count": len(entity_mappings),
        "relation_mapping_count": len(relation_mappings),
    }


def _entity_registry_indexes(
    registry: Any,
) -> tuple[dict[str, dict[str, Any]], dict[tuple[str, str], str], str]:
    entries = {
        str(item["type_id"]): item
        for item in _registry_items(registry, "types")
        if _string(item.get("type_id"))
    }
    mappings: dict[tuple[str, str], str] = {}
    for type_id, entry in entries.items():
        for mapping in _objects(entry.get("source_mappings")):
            source_graph = _string(mapping.get("source_graph"))
            source_kind_id = _string(mapping.get("source_kind_id"))
            if source_graph and source_kind_id:
                mappings[(source_graph, source_kind_id)] = type_id
    fallback = (
        _string(registry.get("fallback_type_id"))
        if isinstance(registry, dict)
        else None
    ) or "tos.entity.unmapped"
    return entries, mappings, fallback


def _relation_registry_indexes(
    registry: Any,
) -> tuple[
    dict[str, dict[str, Any]],
    dict[tuple[str, str, str], str],
    str,
]:
    entries = {
        str(item["relation_type_id"]): item
        for item in _registry_items(registry, "relations")
        if _string(item.get("relation_type_id"))
    }
    mappings: dict[tuple[str, str, str], str] = {}
    for relation_type_id, entry in entries.items():
        for mapping in _objects(entry.get("source_mappings")):
            source_graph = _string(mapping.get("source_graph"))
            source_predicate_id = _string(mapping.get("source_predicate_id"))
            scope = _string(mapping.get("scope"))
            if source_graph and source_predicate_id and scope:
                mappings[(source_graph, source_predicate_id, scope)] = relation_type_id
    fallback = (
        _string(registry.get("fallback_relation_type_id"))
        if isinstance(registry, dict)
        else None
    ) or "tos.relation.unmapped"
    return entries, mappings, fallback


def _strict_strings(
    value: Any,
    name: str,
    *,
    maximum: int,
    unique: bool = False,
) -> list[str]:
    if value is None:
        return []
    if not isinstance(value, list):
        raise ValueError(f"{name} must be an array")
    if len(value) > maximum:
        raise ValueError(f"{name} must contain at most {maximum} values")
    if any(not isinstance(item, str) or not item for item in value):
        raise ValueError(f"{name} must contain only non-empty strings")
    result = list(value)
    if unique and len(set(result)) != len(result):
        raise ValueError(f"{name} must contain unique values")
    return result


def _strict_object(value: Any, name: str, allowed: set[str]) -> dict[str, Any]:
    if value is None:
        return {}
    if not isinstance(value, dict):
        raise ValueError(f"{name} must be an object")
    unknown = sorted(set(value) - allowed)
    if unknown:
        raise ValueError(f"unknown {name} fields: {', '.join(unknown)}")
    return value


def _string(value: Any) -> str | None:
    return value.strip() if isinstance(value, str) and value.strip() else None


def _humanize(value: str) -> str:
    text = re.sub(r"[-_.]+", " ", value).strip()
    return text or "unnamed"


def _localized(
    default: str,
    *,
    ru: str | None = None,
    en: str | None = None,
    original: str | None = None,
) -> dict[str, str | None]:
    return {
        "default": default,
        "ru": _string(ru),
        "en": _string(en),
        "original": _string(original),
    }


def _localized_from(value: Any, fallback: str) -> dict[str, str | None]:
    if isinstance(value, dict):
        default = _string(value.get("default")) or _string(value.get("ru")) or _string(value.get("en")) or fallback
        return _localized(
            default,
            ru=_string(value.get("ru")),
            en=_string(value.get("en")),
            original=_string(value.get("original")),
        )
    return _localized(_string(value) or fallback)


def _display_text(value: Any) -> str | None:
    """Recognize source prose in any declared display language, without translating it."""
    if isinstance(value, dict):
        return next((_string(value.get(key)) for key in ('default', 'ru', 'en', 'original')
                     if _string(value.get(key))), None)
    return _string(value)


def _lens_localized(value: Any, fallback: str, name: str) -> dict[str, str | None]:
    if value is None:
        return _localized(fallback)
    if isinstance(value, str):
        normalized = _string(value)
        if normalized is None:
            raise ValueError(f"{name} must be a non-empty string or localized object")
        return _localized(normalized)
    if not isinstance(value, dict):
        raise ValueError(f"{name} must be a non-empty string or localized object")
    unknown = sorted(set(value) - {"default", "ru", "en", "original"})
    if unknown:
        raise ValueError(f"unknown {name} fields: {', '.join(unknown)}")
    for key, item in value.items():
        if item is not None and not isinstance(item, str):
            raise ValueError(f"{name}.{key} must be a string or null")
    if "default" in value and _string(value.get("default")) is None:
        raise ValueError(f"{name}.default must be a non-empty string")
    return _localized_from(value, fallback)


def _source_refs(item: dict[str, Any], *fallbacks: str | None) -> list[str]:
    refs: set[str] = set(_strings(item.get("source_refs")))
    for key in ("source_ref", "source_path", "path", "owner_surface"):
        value = _string(item.get(key))
        if value:
            refs.add(value)
    refs.update(value for value in fallbacks if value)
    return sorted(refs) or ["ToS/source_home.manifest.json"]


def _existing_display(item: dict[str, Any]) -> dict[str, Any]:
    display = item.get("display")
    return dict(display) if isinstance(display, dict) else {}


def _multilingual_labels(item: dict[str, Any]) -> dict[str, Any]:
    multilingual = item.get("multilingual")
    if not isinstance(multilingual, dict):
        return {}
    labels = multilingual.get("label")
    return dict(labels) if isinstance(labels, dict) else {}


def _node_display(
    item: dict[str, Any],
    kind_id: str,
    source_refs: list[str],
    type_entry: dict[str, Any] | None = None,
) -> dict[str, Any]:
    existing = _existing_display(item)
    properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
    explicit_label = (
        _string(item.get("label"))
        or _string(item.get("canonical_label"))
        or _string(properties.get("preferred_label"))
    )
    path_label = _string(item.get("title")) or _string(item.get("name")) or _string(item.get("path")) or _string(item.get("declared_path"))
    label = explicit_label or path_label or _humanize(str(item.get("node_id") or item.get("id") or "node"))
    labels = _multilingual_labels(item)
    title = _localized_from(existing.get("title"), label)
    if title["ru"] is None:
        title["ru"] = _string(labels.get("ru"))
    if title["en"] is None:
        title["en"] = _string(labels.get("en"))
    if title["original"] is None:
        title["original"] = _string(labels.get("original"))
    variant_labels = properties.get("variant_labels")
    if isinstance(variant_labels, list):
        for variant in variant_labels:
            if not isinstance(variant, dict):
                continue
            language = _string(variant.get("language"))
            value = _string(variant.get("value"))
            if language in {"ru", "en"} and value and title[language] is None:
                title[language] = value

    registry_labels = type_entry.get("labels") if isinstance(type_entry, dict) and isinstance(type_entry.get("labels"), dict) else {}
    kind_label = _localized_from(
        existing.get("kind_label"),
        _string(registry_labels.get("default")) or _humanize(kind_id),
    )
    for language in ("ru", "en"):
        if kind_label[language] is None:
            kind_label[language] = _string(registry_labels.get(language))
    authored_summary = next(
        (
            _string(value)
            for value in (
                _display_text(existing.get("summary")),
                item.get("distilled_thesis"),
                item.get("summary"),
                item.get("description"),
                item.get("role"),
                item.get("purpose"),
                properties.get("distilled_thesis"),
                properties.get("summary"),
                properties.get("description"),
                properties.get("role"),
                properties.get("purpose"),
                properties.get("comment"),
            )
            if _string(value)
        ),
        None,
    )
    state = _string(existing.get("summary_state"))
    if authored_summary:
        summary_default = authored_summary
        # Access can preserve an authored marker supplied by an owning source,
        # but it must not mint that authority merely because text is present in
        # a projection.
        state = state or "source-derived"
    else:
        # Technical status, paths and identifiers remain in the structured
        # packet. They are not a substitute for a description of the subject.
        state = state or "metadata-synthesis"
        summary_default = "Развёрнутое описание пока не добавлено."
    summary = _localized_from(existing.get("summary"), summary_default)
    summary["default"] = summary_default
    if not authored_summary:
        summary["ru"] = summary_default
        summary["en"] = "A detailed description has not been added yet."
    provenance = dict(existing.get("provenance")) if isinstance(existing.get("provenance"), dict) else {}
    provenance.setdefault(
        "title",
        "projected-label" if explicit_label else ("projected-path" if path_label else "identifier-fallback"),
    )
    provenance.setdefault("summary", state)
    provenance.setdefault("source_summary_available", bool(authored_summary))
    return {
        "title": title,
        "kind_label": kind_label,
        "summary": summary,
        "summary_state": state,
        "provenance": provenance,
    }


def _epistemic(item: dict[str, Any], default_authority: str | None = None) -> dict[str, Any]:
    properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
    authority_layer = (
        _string(properties.get("authority_posture"))
        or _string(item.get("authority_layer"))
        or default_authority
    )
    canon_status = _string(properties.get("canon_status")) or _string(item.get("status"))
    review_posture = (
        _string(properties.get("review_posture"))
        or _string(properties.get("review_status"))
        or _string(item.get("review_status"))
    )
    confidence: str | int | float | None = None
    for raw_confidence in (
        properties.get("confidence"),
        properties.get("master_confidence"),
        item.get("confidence"),
    ):
        if isinstance(raw_confidence, bool):
            continue
        if isinstance(raw_confidence, (int, float)) and math.isfinite(raw_confidence):
            confidence = raw_confidence
            break
        confidence_text = _string(raw_confidence)
        if confidence_text:
            confidence = confidence_text
            break
    return {
        "authority_layer": authority_layer,
        "canon_status": canon_status,
        "review_posture": review_posture or "not-recorded",
        "confidence": confidence,
    }


def _source_claim_kind(item: dict[str, Any], claim_predicate: str | None = None) -> str:
    properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
    node_kind = _string(item.get("node_kind")) or "knowledge-object"
    if node_kind == "identity":
        return _string(properties.get("identity_kind")) or "identity"
    if node_kind == "provenance_event":
        return "provenance-event"
    if node_kind != "literal":
        return node_kind
    value = properties.get("value")
    if claim_predicate == "provision_activity" or (
        isinstance(value, dict) and _string(value.get("provision_kind"))
    ):
        return "provision-activity"
    if claim_predicate in _TEMPORAL_CLAIM_PREDICATES:
        return "temporal-assertion"
    if isinstance(value, dict) and (
        isinstance(value.get("interval"), dict)
        or isinstance(value.get("temporal"), dict)
        or _string(value.get("date"))
    ):
        return "temporal-assertion"
    return "literal"


def _semantic_entity_id(
    item: dict[str, Any],
    source_graph: str,
    native: str,
    normalized_id: str,
) -> str:
    properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
    # A reference is not the referring object's identity. Literal objects in
    # particular carry claim_ref only to preserve their assertion context.
    identity_fields = {"identity": "identity_ref", "claim": "claim_ref",
                       "provenance_event": "event_ref", "review": "review_ref"}
    own_reference = identity_fields.get(str(item.get("node_kind"))) if source_graph == "source-claims" else None
    for value in (
        properties.get(own_reference) if own_reference else None,
        properties.get("record_id"),
        item.get("record_id"),
        item.get("node_id"),
        native,
    ):
        candidate = _string(value)
        if candidate and candidate.startswith("tos."):
            return candidate
    return normalized_id


def _normalized_time(value: Any, *, raw_source_field: str | None = None) -> dict[str, Any] | None:
    if isinstance(value, str) and value.strip():
        raw = value.strip()
        precision = (
            "day"
            if re.fullmatch(r"-?\d{4}-\d{2}-\d{2}", raw)
            else "month"
            if re.fullmatch(r"-?\d{4}-\d{2}", raw)
            else "year"
            if re.fullmatch(r"-?\d{4}", raw)
            else None
        )
        if precision:
            parts = re.fullmatch(r"(-?\d{4})(?:-(\d{2}))?(?:-(\d{2}))?", raw)
            year, month, day = int(parts[1]), int(parts[2] or 1), int(parts[3] or 1)
            if not 1 <= month <= 12 or not 1 <= day <= calendar.monthrange(year, month)[1]:
                precision = None
        return {
            "kind": "date-assertion" if precision else "unparsed-period-label",
            **({"sort_start": year * 10000 + month * 100 + day,
                "sort_end": year * 10000 + (12 if precision == 'year' else month) * 100
                + (31 if precision == 'year' else calendar.monthrange(year, month)[1] if precision == 'month' else day),
                "comparison_calendar": "proleptic-gregorian", "year_numbering": "astronomical"} if precision else {}),
            "value": raw,
            "raw": raw,
            "precision": precision,
            "normalization_status": (
                "source-literal-parsed" if precision else "source-literal-unparsed"
            ),
            "source_field": raw_source_field,
        }
    if not isinstance(value, dict):
        return None
    if "start" in value or "end" in value:
        bounds = {key: copy.deepcopy(value[key]) for key in ("start", "end") if key in value}
        issues = []
        parsed = [_normalized_time(bounds.get(key)) for key in ("start", "end")]
        if all(p and p.get("normalization_status") == "source-literal-parsed" for p in parsed):
            if parsed[0]["sort_start"] > parsed[1]["sort_end"]:
                issues.append("reversed-interval")
        return {"kind": "interval-assertion", "interval": bounds, "raw": copy.deepcopy(value),
                "normalization_status": "structured-source", "source_field": raw_source_field,
                "issues": issues,
                **({"sort_start": parsed[0]["sort_start"], "sort_end": parsed[1]["sort_end"],
                    "comparison_calendar": "proleptic-gregorian", "year_numbering": "astronomical"}
                   if not issues and all(p and 'sort_start' in p for p in parsed)
                   and value.get('calendar') in (None, 'gregorian', 'proleptic-gregorian') else {})}
    if isinstance(value.get("interval"), dict):
        normalized = _normalized_time({**value['interval'], 'calendar': value.get('calendar')}) or {}
        return {
            **{key: normalized[key] for key in ('sort_start', 'sort_end', 'comparison_calendar', 'year_numbering') if key in normalized},
            "kind": _string(value.get("chronology_kind")) or "interval-assertion",
            "calendar": _string(value.get("calendar")),
            "interval": dict(value["interval"]),
            "sequence_posture": value.get("sequence_posture"),
            "publication_posture": value.get("publication_posture"),
            "scope": value.get("scope"),
            "stages": list(value.get("stages")) if isinstance(value.get("stages"), list) else [],
            "ordering_warning": value.get("ordering_warning"),
            "normalization_status": "structured-source",
            "source_field": raw_source_field,
            "raw": copy.deepcopy(value),
            "issues": (_normalized_time(value["interval"]) or {}).get("issues", []),
        }
    temporal = value.get("temporal") if isinstance(value.get("temporal"), dict) else value
    if temporal is not value and ('start' in temporal or 'end' in temporal or 'interval' in temporal):
        result = _normalized_time(temporal, raw_source_field=raw_source_field)
        if result is not None:
            result['raw'] = copy.deepcopy(value)
        return result
    if any(
        key in temporal
        for key in ("date", "value", "period", "start", "end", "year", "month", "day")
    ):
        value_from_parts = None
        issues = []
        if temporal.get("year") is not None:
            try:
                year = int(temporal['year'])
                value_from_parts = ('-' if year < 0 else '') + f'{abs(year):04d}'
                if temporal.get("month") is not None:
                    value_from_parts += f"-{int(temporal['month']):02d}"
                if temporal.get("day") is not None:
                    if temporal.get('month') is None:
                        raise ValueError('day without month')
                    value_from_parts += f"-{int(temporal['day']):02d}"
            except (TypeError, ValueError):
                value_from_parts = None
                issues.append('invalid-date-parts')
        literal = temporal.get('value', temporal.get('date', value_from_parts))
        parsed = _normalized_time(literal) if isinstance(literal, str) else None
        if literal is not None and (not parsed or parsed.get('precision') is None):
            issues.append('unparsed-date-value')
        return {
            **({key: parsed[key] for key in ('sort_start', 'sort_end', 'comparison_calendar', 'year_numbering') if key in parsed}
               if parsed and temporal.get('calendar') in (None, 'gregorian', 'proleptic-gregorian') and not issues else {}),
            "kind": _string(temporal.get("kind")) or "date-assertion",
            "calendar": _string(temporal.get("calendar")),
            "value": temporal.get("value", temporal.get("date", value_from_parts)),
            "period": temporal.get("period"),
            "precision": temporal.get("precision") or (parsed or {}).get('precision'),
            "role": temporal.get("role"),
            "source_posture": temporal.get("source_posture"),
            "normalization_status": "structured-source",
            "source_field": raw_source_field,
            "raw": copy.deepcopy(value),
            "issues": issues,
        }
    return None


def _node_semantics(
    item: dict[str, Any],
    source_graph: str,
    source_kind_id: str,
) -> dict[str, Any]:
    properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
    semantics: dict[str, Any] = {}
    if properties.get("packet_id"):
        semantics["annotation"] = {"packet_id": properties["packet_id"], "packet_version": properties.get("packet_version"),
                                   "content_available": properties.get("content_available"),
                                   "publication_posture": properties.get("publication_posture")}
        if source_kind_id == "annotation-claim":
            semantics["claim"] = {"claim_id": properties.get("claim_id"), "claim_version": properties.get("claim_version"),
                                  "proposition": properties.get("proposition"), "review_status": properties.get("claim_status"),
                                  "contract_ref": "ToS/contracts/semantic-annotation-packet-v2.schema.json"}
    if source_graph == "source-claims":
        value = properties.get("value")
        time = _normalized_time(value, raw_source_field="properties.value")
        if time:
            semantics["time"] = time
        if isinstance(value, dict):
            places = [dict(place) for place in _objects(value.get("places"))]
            if places:
                semantics["space"] = {
                    "kind": "claim-scoped-place-mentions",
                    "mentions": places,
                    "normalization_status": "source-declared",
                }
        if source_kind_id == "place":
            semantics["space"] = {
                "kind": "place-identity",
                "place_id": _string(properties.get("identity_ref")),
                "identity_status": properties.get("identity_status"),
                "normalization_status": "source-declared",
            }
        if source_kind_id == "claim":
            semantics["claim"] = {
                "claim_id": _string(properties.get("claim_ref")),
                "source_predicate_id": _string(properties.get("predicate")),
                "review_status": properties.get("review_status"),
                "epistemic_status": properties.get("epistemic_status"),
                "claim_version": properties.get("claim_version"),
            }
    else:
        period = properties.get("period") if "period" in properties else item.get("temporal_context")
        time = _normalized_time(period, raw_source_field="properties.period" if "period" in properties else "temporal_context")
        if time:
            semantics["time"] = time
        if source_graph == "source-navigation" and source_kind_id == "place":
            semantics["space"] = {
                "kind": "place-identity",
                "place_id": _string(item.get("node_id")),
                "identity_status": item.get("identity_status"),
                "normalization_status": "source-declared",
            }
        if source_graph == "source-navigation" and source_kind_id == "region":
            semantics["space"] = {
                "kind": "navigation-region",
                "normalization_status": "not-a-place-identity",
            }
    return semantics


def _normalize_node(
    item: dict[str, Any],
    source_graph: str,
    *,
    native_id: str | None = None,
    identity_id: str | None = None,
    kind_id: str | None = None,
    source_kind_id: str | None = None,
    entity_type_entries: dict[str, dict[str, Any]] | None = None,
    entity_type_mappings: dict[tuple[str, str], str] | None = None,
    fallback_type_id: str = "tos.entity.unmapped",
) -> dict[str, Any]:
    native = native_id or _string(item.get("node_id")) or _string(item.get("id")) or _string(item.get("path")) or "unnamed"
    properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
    semantic_kind = source_kind_id or kind_id or _string(properties.get("original_node_type")) or _string(item.get("node_type")) or _string(item.get("node_kind")) or _string(item.get("resource_kind")) or "knowledge-object"
    type_id = (entity_type_mappings or {}).get((source_graph, semantic_kind), fallback_type_id)
    type_entry = (entity_type_entries or {}).get(type_id)
    ancestors, frontier = set(), [type_id]
    while frontier:
        ancestor = frontier.pop()
        if ancestor in ancestors:
            continue
        ancestors.add(ancestor)
        frontier.extend((entity_type_entries or {}).get(ancestor, {}).get('parent_type_ids', []))
    cache = active_cache.get()
    if cache:
        identifier = f'{source_graph}:{identity_id or native}'
        return cache.normalize('node', identifier,
            [source_graph, native, identity_id, semantic_kind, type_id, fallback_type_id],
            [Input('source-node:' + identifier, item), Input('entity-type:' + type_id, [type_entry, sorted(ancestors)])],
            lambda: _normalize_node(item, source_graph, native_id=native_id, identity_id=identity_id,
                kind_id=kind_id, source_kind_id=source_kind_id, entity_type_entries=entity_type_entries,
                entity_type_mappings=entity_type_mappings, fallback_type_id=fallback_type_id))
    for mapping in (type_entry or {}).get("source_mappings", []):
        if mapping.get("source_graph") == source_graph and mapping.get("source_kind_id") == semantic_kind and mapping.get("labels"):
            # Keep the full family as the shared cache dependency; specialize
            # only its display, not hierarchy, identity or authored labels.
            type_entry = {**type_entry, "labels": mapping["labels"]}
            break
    refs = _source_refs(item)
    attributes = dict(properties)
    for key, value in item.items():
        if key not in {
            "id", "node_id", "label", "canonical_label", "node_type", "node_kind",
            "resource_kind", "display", "multilingual", "properties", "source_ref",
            "source_refs", "graph_layers", "view_ids",
        } and key not in attributes:
            attributes[key] = value
    normalized_id = f"{source_graph}:{identity_id or native}"
    normalized = {
        "id": normalized_id,
        "entity_id": _semantic_entity_id(item, source_graph, native, normalized_id),
        "native_id": native,
        "source_graph": source_graph,
        "kind_id": semantic_kind,
        "type_id": type_id,
        "type_mapping": {
            "status": "mapped" if type_id != fallback_type_id else "unmapped",
            "source_kind_id": semantic_kind,
            "registry_ref": ENTITY_REGISTRY_REF,
        },
        "display": _node_display(item, semantic_kind, refs, type_entry),
        "epistemic": _epistemic(item, "derived-export" if source_graph in {"philosophy", "repository", "source-navigation"} else "canon"),
        "graph_layers": list(dict.fromkeys(_strings(item.get("graph_layers")))) or ([str(item["layer"])] if _string(item.get("layer")) else []),
        "view_ids": list(dict.fromkeys(_strings(item.get("view_ids")))),
        "source_refs": refs,
        "attributes": attributes,
        "semantics": _node_semantics(item, source_graph, semantic_kind),
        "source_record": _source_record(item, attributes),
    }
    normalized["semantics"]["type_ancestors"] = sorted(ancestors)
    _stamp_content_revision(normalized)
    return normalized


def _source_record(item: dict[str, Any], attributes: dict[str, Any]) -> dict[str, Any]:
    """Lossless envelope of the supplied public input, never a private-file read."""
    properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
    escape = lambda key: str(key).replace("~", "~0").replace("/", "~1")
    return {"payload": copy.deepcopy(item), "digest": _stable_digest(item),
            "transform_version": "tos-knowledge-normalization-v2",
            "field_map": {f"attributes.{key}": ("/properties/" if key in properties else "/") + escape(key)
                          for key in attributes}}


def _relation_display(
    item: dict[str, Any],
    predicate_id: str,
    left: dict[str, Any] | None,
    right: dict[str, Any] | None,
    source_refs: list[str],
    relation_type_entry: dict[str, Any] | None = None,
) -> dict[str, Any]:
    existing = _existing_display(item)
    properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
    registry_labels = (
        relation_type_entry.get("labels")
        if isinstance(relation_type_entry, dict) and isinstance(relation_type_entry.get("labels"), dict)
        else {}
    )
    # Multiple source carriers of the same predicate do not make a type a
    # broad family. Only distinct predicate meanings require native labels.
    mapped_predicates = {mapping.get("source_predicate_id")
                         for mapping in (relation_type_entry or {}).get("source_mappings", [])}
    exact_type_label = len(mapped_predicates) <= 1
    label_fallback = (
        _string(properties.get("relation_label"))
        or (_string(registry_labels.get("default")) if exact_type_label else None)
        or _humanize(predicate_id)
    )
    label = _localized_from(existing.get("label"), label_fallback)
    for language in ("ru", "en"):
        if label[language] is None:
            # A family label is not a translation of every concrete predicate.
            label[language] = _string(registry_labels.get(language)) if exact_type_label else None
    inverse = existing.get("inverse_label")
    registry_inverse = (
        relation_type_entry.get("inverse_labels")
        if isinstance(relation_type_entry, dict) and isinstance(relation_type_entry.get("inverse_labels"), dict)
        else None
    )
    inverse_label = (
        _localized_from(
            inverse or registry_inverse,
            _string((registry_inverse or {}).get("default"))
            or _humanize(str(item.get("inverse_predicate_id") or "inverse relation")),
        )
        if inverse or registry_inverse
        else None
    )
    left_title = ((left or {}).get("display") or {}).get("title", {}).get("default") or str(item.get("from_id") or "unknown source")
    right_title = ((right or {}).get("display") or {}).get("title", {}).get("default") or str(item.get("to_id") or "unknown target")
    statement_default = f"{left_title} — {label['default']} → {right_title}."
    statement = _localized_from(existing.get("statement"), statement_default)
    statement["default"] = statement_default if not _string(statement.get("default")) else statement["default"]
    if not _display_text(existing.get("statement")):
        for language in ("ru", "en"):
            if not label[language]:
                continue
            left_labels = ((left or {}).get("display") or {}).get("title") or {}
            right_labels = ((right or {}).get("display") or {}).get("title") or {}
            localized_left = left_labels.get(language) or left_labels.get("original") or left_title
            localized_right = right_labels.get(language) or right_labels.get("original") or right_title
            statement[language] = f"{localized_left} — {label[language]} → {localized_right}."
    explanation_value = next(
        (
            _string(value)
            for value in (
                _display_text(existing.get("explanation")),
                item.get("note"),
                item.get("comment"),
                properties.get("comment"),
                properties.get("note"),
                properties.get("description"),
            )
            if _string(value)
        ),
        None,
    )
    state = _string(existing.get("explanation_state"))
    if explanation_value:
        explanation_default = explanation_value
        state = state or "source-derived"
    else:
        explanation_default = "Отдельное пояснение к этой связи пока не добавлено."
        state = state or "metadata-synthesis"
    explanation = _localized_from(existing.get("explanation"), explanation_default)
    explanation["default"] = explanation_default
    if not explanation_value:
        explanation["ru"] = explanation_default
        explanation["en"] = "A separate explanation of this relationship has not been added yet."
    provenance = dict(existing.get("provenance")) if isinstance(existing.get("provenance"), dict) else {}
    provenance.setdefault("label", "projected-predicate-label" if existing.get("label") or _string(properties.get("relation_label"))
                          else "registry-label" if exact_type_label and registry_labels else "identifier-fallback")
    provenance.setdefault("statement", "endpoint-label-synthesis")
    provenance.setdefault("explanation", state)
    provenance.setdefault("source_explanation_available", bool(explanation_value))
    return {
        "label": label,
        "inverse_label": inverse_label,
        "statement": statement,
        "explanation": explanation,
        "explanation_state": state,
        "provenance": provenance,
    }


def _normalize_relation(
    item: dict[str, Any],
    source_graph: str,
    nodes_by_id: dict[str, dict[str, Any]],
    *,
    native_id: str | None = None,
    identity_id: str | None = None,
    relation_type_entries: dict[str, dict[str, Any]] | None = None,
    relation_type_mappings: dict[tuple[str, str, str], str] | None = None,
    fallback_relation_type_id: str = "tos.relation.unmapped",
) -> dict[str, Any]:
    native = native_id or _string(item.get("edge_id")) or _string(item.get("id")) or "unnamed-relation"
    left_native = _string(item.get("from_id")) or "unknown-source"
    right_native = _string(item.get("to_id")) or "unknown-target"
    left_source_graph = _string(item.get("from_source_graph")) or source_graph
    right_source_graph = _string(item.get("to_source_graph")) or source_graph
    left_id = f"{left_source_graph}:{left_native}"
    right_id = f"{right_source_graph}:{right_native}"
    predicate_id = _string(item.get("predicate_id")) or "related_to"
    relation_type_id = (relation_type_mappings or {}).get(
        (source_graph, predicate_id, "edge"),
        fallback_relation_type_id,
    )
    relation_type_entry = (relation_type_entries or {}).get(relation_type_id)
    for mapping in (relation_type_entry or {}).get("source_mappings", []):
        if mapping.get("source_graph") == source_graph and mapping.get("source_predicate_id") == predicate_id and mapping.get("scope") == "edge" and mapping.get("labels"):
            relation_type_entry = {**relation_type_entry, "labels": mapping["labels"], "definition": mapping.get("definition"), "source_mappings": [mapping]}
            break
    cache = active_cache.get()
    if cache:
        identifier = f'{source_graph}:{identity_id or native}'
        dependencies = [Input('source-relation:' + identifier, item),
                        Input(f'relation-type:{source_graph}:{predicate_id}', relation_type_entry)]
        dependencies.extend(cache.node_title(id, (nodes_by_id.get(id) or {}).get('display', {}).get('title'))
                            for id in dict.fromkeys((left_id, right_id)))
        return cache.normalize('relation', identifier,
            [source_graph, native, identity_id, relation_type_id, fallback_relation_type_id], dependencies,
            lambda: _normalize_relation(item, source_graph, nodes_by_id, native_id=native_id, identity_id=identity_id,
                relation_type_entries=relation_type_entries, relation_type_mappings=relation_type_mappings,
                fallback_relation_type_id=fallback_relation_type_id))
    refs = _source_refs(item)
    properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
    attributes = dict(properties)
    for key, value in item.items():
        if key not in {
            "id", "edge_id", "from_id", "to_id", "predicate_id", "display",
            "properties", "source_ref", "source_refs", "graph_layers", "view_ids",
            "from_source_graph", "to_source_graph",
        } and key not in attributes:
            attributes[key] = value
    semantics: dict[str, Any] = {}
    if relation_type_id == "tos.relation.has-normalized-place":
        semantics["space"] = {
            "roles": sorted(set(_strings(properties.get("spatial_roles")))),
            "literal_forms": sorted(set(_strings(properties.get("spatial_literal_forms")))),
            "normalization_status": "source-declared",
        }
    if relation_type_id == "tos.relation.has-normalized-agent":
        semantics["responsibility"] = {
            "roles": sorted(set(_strings(properties.get("agent_roles")))),
            "literal_forms": sorted(set(_strings(properties.get("agent_literal_forms")))),
            "normalization_status": "source-declared",
        }
    normalized = {
        "id": f"{source_graph}:{identity_id or native}",
        "native_id": native,
        "source_graph": source_graph,
        "from_id": left_id,
        "to_id": right_id,
        "predicate_id": predicate_id,
        "relation_type_id": relation_type_id,
        "predicate_mapping": {
            "status": "mapped" if relation_type_id != fallback_relation_type_id else "unmapped",
            "source_predicate_id": predicate_id,
            "registry_ref": RELATION_REGISTRY_REF,
        },
        "display": _relation_display(
            item,
            predicate_id,
            nodes_by_id.get(left_id),
            nodes_by_id.get(right_id),
            refs,
            relation_type_entry,
        ),
        "epistemic": _epistemic(item, "derived-export" if source_graph != "canon" else "canon"),
        "graph_layers": list(dict.fromkeys(_strings(item.get("graph_layers")))) or ([str(item["layer"])] if _string(item.get("layer")) else []),
        "view_ids": list(dict.fromkeys(_strings(item.get("view_ids")))),
        "source_refs": refs,
        "attributes": attributes,
        "semantics": semantics,
        "source_record": _source_record(item, attributes),
    }
    _stamp_content_revision(normalized)
    return normalized


def build_knowledge_graph(
    corpus: dict[str, Any],
    philosophy: dict[str, Any],
    bibliographic_claims: dict[str, Any] | None = None,
    entity_type_registry: dict[str, Any] | None = None,
    relation_type_registry: dict[str, Any] | None = None,
) -> dict[str, Any]:
    bibliographic = bibliographic_claims if isinstance(bibliographic_claims, dict) else {}
    entity_registry = entity_type_registry if isinstance(entity_type_registry, dict) else {}
    relation_registry = relation_type_registry if isinstance(relation_type_registry, dict) else {}
    if active_cache.get():
        active_cache.get().scheduler.evaluate(Input('source-registry:entities', entity_registry))
        active_cache.get().scheduler.evaluate(Input('source-registry:relations', relation_registry))
    if entity_registry or relation_registry:
        registry_report = validate_semantic_registries(entity_registry, relation_registry)
        if not registry_report["valid"]:
            raise ValueError("invalid semantic registry: " + "; ".join(registry_report["violations"]))
    entity_entries, entity_mappings, fallback_type_id = _entity_registry_indexes(entity_registry)
    relation_entries, relation_mappings, fallback_relation_type_id = _relation_registry_indexes(relation_registry)
    source_revision = _stable_digest(
        {
            "corpus": corpus,
            "philosophy": philosophy,
            "bibliographic_claims": bibliographic,
            "entity_type_registry": entity_registry,
            "relation_type_registry": relation_registry,
        }
    )
    nodes: list[dict[str, Any]] = []
    for item in _objects(philosophy.get("nodes")):
        nodes.append(
            _normalize_node(
                item,
                "philosophy",
                entity_type_entries=entity_entries,
                entity_type_mappings=entity_mappings,
                fallback_type_id=fallback_type_id,
            )
        )
    for item in _objects(corpus.get("nodes")):
        nodes.append(
            _normalize_node(
                item,
                "canon",
                entity_type_entries=entity_entries,
                entity_type_mappings=entity_mappings,
                fallback_type_id=fallback_type_id,
            )
        )
    navigation = corpus.get("source_navigation") if isinstance(corpus.get("source_navigation"), dict) else {}
    for item in _objects(navigation.get("nodes")):
        nodes.append(
            _normalize_node(
                item,
                "source-navigation",
                entity_type_entries=entity_entries,
                entity_type_mappings=entity_mappings,
                fallback_type_id=fallback_type_id,
            )
        )
    claim_traces = {
        str(item.get("claim_ref")): item
        for item in _objects(bibliographic.get("claim_traces"))
        if _string(item.get("claim_ref"))
    }
    if active_cache.get():
        for claim_ref, trace in claim_traces.items():
            active_cache.get().scheduler.evaluate(Input('source-claim-trace:' + claim_ref, trace))
    claim_predicates_by_object = {
        str(item.get("object_node_id")): str(item.get("predicate"))
        for item in claim_traces.values()
        if _string(item.get("object_node_id")) and _string(item.get("predicate"))
    }
    bibliographic_nodes = _objects(bibliographic.get("nodes"))
    bibliographic_nodes_by_native = {
        str(item.get("node_id")): item
        for item in bibliographic_nodes
        if _string(item.get("node_id"))
    }
    for item in bibliographic_nodes:
        native = _string(item.get("node_id")) or "unnamed"
        source_kind_id = _source_claim_kind(item, claim_predicates_by_object.get(native))
        material = dict(item)
        material["graph_layers"] = sorted({*_strings(item.get("graph_layers")), "bibliographic-claim"})
        nodes.append(
            _normalize_node(
                material,
                "source-claims",
                source_kind_id=source_kind_id,
                entity_type_entries=entity_entries,
                entity_type_mappings=entity_mappings,
                fallback_type_id=fallback_type_id,
            )
        )
    repository_root = _normalize_node(
        {
            "node_id": "tree-of-sophia",
            "label": "Tree of Sophia",
            "node_type": "repository-root",
            "summary": "Корень индексированной структуры репозитория Tree of Sophia.",
            "source_ref": "ToS/source_home.manifest.json",
            "view_ids": ["corpus-topology"],
            "authority_layer": "source_home",
        },
        "repository",
        identity_id="root:tree-of-sophia",
        entity_type_entries=entity_entries,
        entity_type_mappings=entity_mappings,
        fallback_type_id=fallback_type_id,
    )
    nodes.append(repository_root)
    repository_items: list[tuple[str, int, dict[str, Any], str, str]] = []
    for collection, kind in (("branches", "repository-branch"), ("manifests", "repository-manifest"), ("resources", "repository-resource")):
        for order, item in enumerate(_objects(corpus.get(collection))):
            native = _string(item.get("id")) or _string(item.get("path")) or f"{collection}:{order}"
            material = dict(item)
            identity_kind = {"branches": "branch", "manifests": "manifest", "resources": "resource"}[collection]
            identity = f"{identity_kind}:{native}"
            if collection == "branches":
                material["view_ids"] = sorted({*_strings(item.get("view_ids")), "corpus-topology"})
            nodes.append(
                _normalize_node(
                    material,
                    "repository",
                    native_id=native,
                    identity_id=identity,
                    kind_id=kind if collection != "resources" else f"repository-{_string(item.get('resource_kind')) or 'resource'}",
                    entity_type_entries=entity_entries,
                    entity_type_mappings=entity_mappings,
                    fallback_type_id=fallback_type_id,
                )
            )
            repository_items.append((collection, order, material, native, identity))

    nodes_by_id = {node["id"]: node for node in nodes}
    relation_sources: list[tuple[str, dict[str, Any], str | None]] = []
    relation_sources.extend(("philosophy", item, None) for item in _objects(philosophy.get("edges")))
    pack_paths = {
        str(item.get("pack_id")): str(item.get("path"))
        for item in _objects(corpus.get("relation_packs"))
        if _string(item.get("pack_id")) and _string(item.get("path"))
    }
    for item in _objects(corpus.get("relation_edges")):
        material = dict(item)
        pack_path = pack_paths.get(str(item.get("pack_id") or ""))
        if pack_path and not material.get("source_ref"):
            material["source_ref"] = pack_path
        views = set(_strings(material.get("view_ids")))
        if material.get("owner_branch") == "ToS/canon":
            views.add("route-graph")
        if material.get("owner_branch") == "ToS/candidate-intake":
            views.add("promotion-flow")
        material["view_ids"] = sorted(views)
        edge_native = _string(material.get("edge_id")) or _string(material.get("id"))
        pack_id = _string(material.get("pack_id"))
        identity = f"{pack_id}:{edge_native}" if pack_id and edge_native else None
        relation_source = "canon" if material.get("owner_branch") == "ToS/canon" else "candidate-intake"
        relation_sources.append((relation_source, material, identity))

    # Canon node-local relations are authored semantic assertions too.  The
    # compact corpus index preserves the complete node contract in
    # ``properties``; materialize those relations instead of making consumers
    # reverse-engineer them from an opaque payload.
    canon_nodes_by_source_path: dict[str, str] = {}
    for item in _objects(corpus.get("nodes")):
        source_path = _string(item.get("source_path"))
        node_id = _string(item.get("node_id"))
        if source_path and node_id:
            canon_nodes_by_source_path[source_path] = node_id
        properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
        relation_field = (
            "relations"
            if isinstance(properties.get("relations"), list)
            else "lineage_relations"
        )
        for order, relation in enumerate(_objects(properties.get(relation_field))):
            predicate_id = _string(relation.get("relation"))
            target_ref = _string(relation.get("target_ref"))
            if not node_id or not predicate_id or not target_ref:
                continue
            material = {
                "edge_id": f"node-relation:{hashlib.sha256(f'{node_id}\0{relation_field}\0{order}\0{predicate_id}\0{target_ref}'.encode('utf-8')).hexdigest()[:24]}",
                "from_id": node_id,
                "to_id": target_ref,
                "predicate_id": predicate_id,
                "source_ref": source_path,
                "graph_layers": ["authored-node-relation"],
                "authority_layer": item.get("authority_layer") or "canon",
                "properties": {
                    "relation_label": _humanize(predicate_id),
                    "derivation": "authored-node-contract-relation",
                    "source_field": f"{relation_field}[{order}]",
                    "review_status": "source-recorded",
                },
            }
            relation_sources.append(("canon", material, None))
    relation_sources.extend(("source-navigation", item, None) for item in _objects(navigation.get("edges")))

    for item in _objects(bibliographic.get("edges")):
        material = dict(item)
        material["predicate_id"] = _string(item.get("edge_kind")) or "related_to"
        material["source_ref"] = _string(item.get("source_claim_file_ref")) or "ToS/source-witnesses/catalog/claims.jsonl"
        material["graph_layers"] = ["bibliographic-claim"]
        properties = dict(item.get("properties")) if isinstance(item.get("properties"), dict) else {}
        claim_ref = _string(item.get("claim_ref"))
        trace = claim_traces.get(claim_ref or "")
        object_node = (
            bibliographic_nodes_by_native.get(str(trace.get("object_node_id")))
            if isinstance(trace, dict)
            else None
        )
        object_properties = (
            object_node.get("properties")
            if isinstance(object_node, dict) and isinstance(object_node.get("properties"), dict)
            else {}
        )
        value = object_properties.get("value") if isinstance(object_properties, dict) else None
        target_node = bibliographic_nodes_by_native.get(str(item.get("to_id")))
        target_properties = (
            target_node.get("properties")
            if isinstance(target_node, dict) and isinstance(target_node.get("properties"), dict)
            else {}
        )
        target_identity = _string(target_properties.get("identity_ref"))
        if material["predicate_id"] == "has_normalized_place" and isinstance(value, dict):
            matching = [
                place
                for place in _objects(value.get("places"))
                if _string(place.get("normalized_place_ref")) == target_identity
            ]
            properties["spatial_roles"] = sorted(
                {
                    *_strings(properties.get("spatial_roles")),
                    *(_string(place.get("role")) for place in matching if _string(place.get("role"))),
                }
            )
            properties["spatial_literal_forms"] = sorted(
                {
                    *_strings(properties.get("spatial_literal_forms")),
                    *(_string(place.get("literal_form")) for place in matching if _string(place.get("literal_form"))),
                }
            )
        if material["predicate_id"] == "has_normalized_agent" and isinstance(value, dict):
            matching = [
                agent
                for agent in _objects(value.get("agents"))
                if _string(agent.get("normalized_agent_ref")) == target_identity
            ]
            properties["agent_roles"] = sorted(
                {_string(agent.get("role")) for agent in matching if _string(agent.get("role"))}
            )
            properties["agent_literal_forms"] = sorted(
                {_string(agent.get("literal_form")) for agent in matching if _string(agent.get("literal_form"))}
            )
        material["properties"] = properties
        relation_sources.append(("source-claims", material, None))

    branch_items = _objects(corpus.get("branches"))
    branch_by_path = {
        str(item.get("path")): str(item.get("id"))
        for item in branch_items
        if _string(item.get("path")) and _string(item.get("id"))
    }

    def owning_branch_id(item: dict[str, Any]) -> str | None:
        owner = _string(item.get("owner_branch")) or _string(item.get("declared_path")) or _string(item.get("path"))
        if not owner:
            return None
        matches = [path for path in branch_by_path if owner == path or owner.startswith(path + "/")]
        return branch_by_path[max(matches, key=len)] if matches else None

    for branch in branch_items:
        branch_id = _string(branch.get("id"))
        if not branch_id:
            continue
        relation_sources.append(
            (
                "repository",
                {
                    "edge_id": f"corpus-topology:{branch_id}",
                    "from_id": "root:tree-of-sophia",
                    "to_id": f"branch:{branch_id}",
                    "predicate_id": "contains",
                    "source_ref": _string(branch.get("owner_surface")) or _string(branch.get("path")),
                    "view_ids": ["corpus-topology"],
                    "authority_layer": branch.get("authority_layer"),
                    "properties": {
                        "relation_label": "contains",
                        "note": "Структурная связь получена из source_home manifest.",
                        "derivation": "source-home-branch-membership",
                    },
                },
                None,
            )
        )
    for collection, order, item, native, identity in repository_items:
        if collection == "branches":
            continue
        branch_id = owning_branch_id(item)
        if not branch_id:
            continue
        item_path = _string(item.get("path")) or native
        digest = hashlib.sha256(item_path.encode("utf-8")).hexdigest()[:16]
        predicate_id = "owns_manifest" if collection == "manifests" else "owns_resource"
        relation_sources.append(
            (
                "repository",
                {
                    "edge_id": f"{predicate_id}:{branch_id}:{digest}",
                    "from_id": f"branch:{branch_id}",
                    "to_id": identity,
                    "predicate_id": predicate_id,
                    "source_ref": item_path,
                    "authority_layer": item.get("authority_layer"),
                    "properties": {
                        "relation_label": _humanize(predicate_id),
                        "note": f"Структурная связь выведена из owner_branch для {item_path}.",
                        "derivation": "indexed-owner-branch",
                        "collection": collection,
                        "source_order": order,
                    },
                },
                None,
            )
        )

    nodes_by_id = {node["id"]: node for node in nodes}
    representations_by_entity: dict[str, dict[str, list[dict[str, Any]]]] = {}
    for node in nodes:
        entity_id = _string(node.get("entity_id"))
        if not entity_id or not entity_id.startswith("tos."):
            continue
        representations_by_entity.setdefault(entity_id, {}).setdefault(str(node["source_graph"]), []).append(node)
    representation_pairs = [
        (entity_id, claim, navigation)
        for entity_id, representations in sorted(representations_by_entity.items())
        for claim in representations.get("source-claims", [])
        for navigation in representations.get("source-navigation", [])
    ]
    for entity_id, claim_representation, navigation_representation in representation_pairs:
        relation_sources.append(
            (
                "semantic-interchange",
                {
                    "edge_id": f"projects:{entity_id}:" + _stable_digest([claim_representation['id'], navigation_representation['id']])[:16],
                    "from_id": claim_representation["native_id"],
                    "from_source_graph": "source-claims",
                    "to_id": navigation_representation["native_id"],
                    "to_source_graph": "source-navigation",
                    "predicate_id": "projects",
                    "source_refs": sorted(
                        {
                            *_strings(claim_representation.get("source_refs")),
                            *_strings(navigation_representation.get("source_refs")),
                            ENTITY_REGISTRY_REF,
                        }
                    ),
                    "graph_layers": ["semantic-interchange"],
                    "properties": {
                        "derivation": "shared-persistent-tos-entity-id",
                        "entity_id": entity_id,
                        "relation_label": "projects",
                        "note": "Связь соединяет две проекции одного объявленного устойчивого ToS ID и не создаёт новое утверждение same_as.",
                    },
                },
                None,
            )
        )

    # A source-witness record can explicitly cite an authored canon node.  An
    # exact path match is strong enough to expose a grounding route, but not to
    # claim identity equivalence.  No label or text-similarity inference is
    # used here.
    for item in _objects(navigation.get("nodes")):
        navigation_node_id = _string(item.get("node_id"))
        properties = item.get("properties") if isinstance(item.get("properties"), dict) else {}
        source_record = (
            properties.get("source_record")
            if isinstance(properties.get("source_record"), dict)
            else {}
        )
        declared_refs = sorted(
            {
                *_strings(properties.get("source_refs")),
                *_strings(source_record.get("source_refs")),
            }
        )
        if not navigation_node_id:
            continue
        for source_ref in declared_refs:
            canon_node_id = canon_nodes_by_source_path.get(source_ref)
            if not canon_node_id:
                continue
            relation_sources.append(
                (
                    "semantic-interchange",
                    {
                        "edge_id": f"grounded-in:{hashlib.sha256(f'{navigation_node_id}\0{source_ref}\0{canon_node_id}'.encode('utf-8')).hexdigest()[:24]}",
                        "from_id": navigation_node_id,
                        "from_source_graph": "source-navigation",
                        "to_id": canon_node_id,
                        "to_source_graph": "canon",
                        "predicate_id": "grounded_in",
                        "source_refs": sorted(
                            {
                                *_source_refs(item),
                                source_ref,
                                RELATION_REGISTRY_REF,
                            }
                        ),
                        "graph_layers": ["semantic-interchange"],
                        "properties": {
                            "derivation": "authored-source-record-source-ref",
                            "relation_label": "grounded in",
                            "review_status": "source-recorded",
                            "declared_source_ref": source_ref,
                            "note": "Связь построена только из точной авторской source_refs-ссылки на canon node; она не утверждает same_as.",
                        },
                    },
                    None,
                )
            )

    # Relation endpoints are part of the public graph even when the compact
    # corpus index has no full node payload for them.
    for source_graph, item, _identity in relation_sources:
        for endpoint_key in ("from_id", "to_id"):
            native = _string(item.get(endpoint_key))
            if not native:
                continue
            endpoint_source = _string(item.get("from_source_graph" if endpoint_key == "from_id" else "to_source_graph")) or source_graph
            identifier = f"{endpoint_source}:{native}"
            if identifier in nodes_by_id:
                continue
            placeholder = _normalize_node(
                {
                    "node_id": native,
                    "label": _humanize(native),
                    "node_type": "relation-endpoint",
                    "source_refs": _source_refs(item),
                    "authority_layer": item.get("authority_layer"),
                },
                endpoint_source,
                entity_type_entries=entity_entries,
                entity_type_mappings=entity_mappings,
                fallback_type_id=fallback_type_id,
            )
            nodes.append(placeholder)
            nodes_by_id[identifier] = placeholder

    relations = [
        _normalize_relation(
            item,
            source_graph,
            nodes_by_id,
            identity_id=identity,
            relation_type_entries=relation_entries,
            relation_type_mappings=relation_mappings,
            fallback_relation_type_id=fallback_relation_type_id,
        )
        for source_graph, item, identity in relation_sources
    ]

    claim_updates = {}
    for claim_ref, trace in claim_traces.items():
        claim_node_id = f"source-claims:{trace.get('claim_node_id')}"
        claim_node = nodes_by_id.get(claim_node_id)
        if claim_node is None:
            continue
        source_predicate_id = _string(trace.get("predicate")) or "related_to"
        relation_type_id = relation_mappings.get(
            ("source-claims", source_predicate_id, "claim-predicate"),
            fallback_relation_type_id,
        )
        subject_id = f"source-claims:{trace.get('subject_node_id')}"
        object_id = f"source-claims:{trace.get('object_node_id')}"
        subject = nodes_by_id.get(subject_id)
        object_node = nodes_by_id.get(object_id)
        claim_updates[claim_node_id] = ({
            **dict(claim_node.get("semantics", {}).get("claim") or {}),
            "claim_id": claim_ref,
            "source_predicate_id": source_predicate_id,
            "relation_type_id": relation_type_id,
            "predicate_mapping_status": "mapped" if relation_type_id != fallback_relation_type_id else "unmapped",
            "subject_node_id": subject_id,
            "subject_entity_id": (subject or {}).get("entity_id"),
            "object_node_id": object_id,
            "object_entity_id": (object_node or {}).get("entity_id"),
            "normalized_identity_node_ids": [
                f"source-claims:{identifier}"
                for identifier in _strings(trace.get("normalized_identity_node_ids"))
            ],
            "evidence_node_ids": [
                f"source-claims:{identifier}"
                for identifier in _strings(trace.get("evidence_node_ids"))
            ],
            "review_status": trace.get("review_status"),
            "epistemic_status": trace.get("epistemic_status"),
        }, dict(trace))
    inherited_views = {}
    for relation in relations:
        view_ids = set(_strings(relation.get("view_ids")))
        if not view_ids:
            continue
        for endpoint in (str(relation["from_id"]), str(relation["to_id"])):
            inherited_views.setdefault(endpoint, set()).update(view_ids)
    nodes = [_finalize_knowledge_node(node, claim_updates.get(node['id']),
                                      sorted(inherited_views.get(node['id'], set()))) for node in nodes]
    nodes.sort(key=lambda item: (str(item["source_graph"]), str(item["id"])))
    relations.sort(key=lambda item: (str(item["source_graph"]), str(item["id"])))
    source_counts = Counter(str(item["source_graph"]) for item in nodes)
    node_summary_states = Counter(str(item["display"]["summary_state"]) for item in nodes)
    relation_explanation_states = Counter(str(item["display"]["explanation_state"]) for item in relations)
    nodes_without_source_summary = sum(
        item["display"]["provenance"].get("source_summary_available") is False
        for item in nodes
    )
    relations_without_source_explanation = sum(
        item["display"]["provenance"].get("source_explanation_available") is False
        for item in relations
    )
    graph = {
        "schema": "tos_knowledge_graph_v1",
        "source_revision": source_revision,
        "nodes": nodes,
        "relations": relations,
        "counts": {
            "nodes": len(nodes),
            "relations": len(relations),
            "sources": dict(sorted(source_counts.items())),
            "display_coverage": {
                "node_titles": len(nodes),
                "node_summaries": len(nodes),
                "node_summary_states": dict(sorted(node_summary_states.items())),
                "nodes_without_source_summary": nodes_without_source_summary,
                "relation_labels": len(relations),
                "relation_statements": len(relations),
                "relation_explanations": len(relations),
                "relation_explanation_states": dict(sorted(relation_explanation_states.items())),
                "relations_without_source_explanation": relations_without_source_explanation,
            },
            "semantic_mapping": {
                "mapped_nodes": sum(item["type_mapping"]["status"] == "mapped" for item in nodes),
                "unmapped_nodes": sum(item["type_mapping"]["status"] == "unmapped" for item in nodes),
                "mapped_relations": sum(item["predicate_mapping"]["status"] == "mapped" for item in relations),
                "unmapped_relations": sum(item["predicate_mapping"]["status"] == "unmapped" for item in relations),
                "cross_layer_relations": sum(item["source_graph"] == "semantic-interchange" for item in relations),
            },
        },
        "authority_boundary": {
            "is_source": False,
            "is_canon": False,
            "writes_to_tree": False,
            "source_owner": "Tree-of-Sophia",
            "note": "This normalized graph is a consumer read model. Every item returns to its ToS source_refs.",
        },
    }
    if entity_registry and relation_registry:
        semantic_report = validate_knowledge_semantics(graph, entity_registry, relation_registry)
        if not semantic_report["valid"]:
            raise ValueError("knowledge semantic invariant violation: " + "; ".join(semantic_report["violations"][:20]))
        graph["counts"]["semantic_validation"] = semantic_report
    return graph


def _finalize_knowledge_node(node, claim_update, inherited_views):
    cache = active_cache.get()
    if cache:
        identifier = node['id']
        return cache.memo('final-node', identifier, [
            Input('base-node:' + identifier, node),
            Input('claim-finalization:' + identifier, claim_update),
            Input('inherited-views:' + identifier, inherited_views),
        ], lambda: _final_node_value(node, claim_update, inherited_views))
    return _final_node_value(node, claim_update, inherited_views)


def _final_node_value(node, claim_update, inherited_views):
    result = copy.deepcopy(node)
    changed = not result.get('content_revision')
    if claim_update is not None:
        claim, trace = copy.deepcopy(claim_update)
        result.setdefault('semantics', {})['claim'] = claim
        result['attributes']['claim_trace'] = trace
        changed = True
    if inherited_views:
        views = sorted({*_strings(result.get('view_ids')), *inherited_views})
        changed = changed or views != result.get('view_ids')
        result['view_ids'] = views
    # Normalization already stamped the complete base node. Keep that revision
    # if finalization did not change content, while preserving copy isolation.
    if changed:
        _stamp_content_revision(result)
    return result


def _type_is_a(
    type_id: str | None,
    allowed_type_ids: Iterable[str],
    entity_entries: dict[str, dict[str, Any]],
) -> bool:
    if not type_id:
        return False
    allowed = set(allowed_type_ids)
    frontier = [type_id]
    seen: set[str] = set()
    while frontier:
        current = frontier.pop()
        if current in allowed:
            return True
        if current in seen:
            continue
        seen.add(current)
        entry = entity_entries.get(current)
        if entry:
            frontier.extend(_strings(entry.get("parent_type_ids")))
    return False


def _validation_digest(value: Any) -> str:
    """Private change fingerprint, never a public cross-language revision.

    JSON encoding avoids millions of per-scalar framing/hash calls on warm
    validation. Keep non-finite values rejected and bind this implementation
    through the normalization processor version before admitting cached checks.
    """
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True,
                         separators=(",", ":"), allow_nan=False)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def validate_knowledge_semantics(
    graph: Any,
    entity_registry: Any,
    relation_registry: Any,
) -> dict[str, Any]:
    """Check registry binding and endpoint contracts for the complete read model."""

    registry_report = validate_semantic_registries(entity_registry, relation_registry)
    violations = list(registry_report["violations"])
    entity_entries, _entity_mappings, fallback_type_id = _entity_registry_indexes(entity_registry)
    relation_entries, _relation_mappings, fallback_relation_type_id = _relation_registry_indexes(relation_registry)
    nodes = _objects(graph.get("nodes")) if isinstance(graph, dict) else []
    relations = _objects(graph.get("relations")) if isinstance(graph, dict) else []
    nodes_by_id: dict[str, dict[str, Any]] = {}

    def validate_node(node):
        violations = []
        node_id = _string(node.get("id"))
        type_id = _string(node.get("type_id"))
        mapping = node.get("type_mapping") if isinstance(node.get("type_mapping"), dict) else {}
        if not node_id:
            violations.append("knowledge node has no id")
            return violations
        if type_id not in entity_entries:
            violations.append(f"node {node_id} uses unregistered type {type_id!r}")
        elif entity_entries[type_id].get("abstract"):
            violations.append(f"node {node_id} instantiates abstract type {type_id}")
        expected_type = _entity_mappings.get((str(node.get("source_graph")), str(mapping.get("source_kind_id"))), fallback_type_id)
        if type_id != expected_type:
            violations.append(f"node {node_id} disagrees with its registered source type mapping")
        expected_status = "unmapped" if type_id == fallback_type_id else "mapped"
        if mapping.get("status") != expected_status:
            violations.append(f"node {node_id} has inconsistent type mapping status")
        if not _string(mapping.get("source_kind_id")):
            violations.append(f"node {node_id} does not preserve source_kind_id")
        if not _string(node.get("entity_id")):
            violations.append(f"node {node_id} has no stable entity_id or representation fallback")
        if not _strings(node.get("source_refs")):
            violations.append(f"node {node_id} has no source_refs")
        for definition in (entity_registry or {}).get("property_definitions", []):
            applies = _type_is_a(type_id, definition.get("applies_to", []), entity_entries) if definition.get("inherited") else type_id in definition.get("applies_to", [])
            if not applies:
                continue
            value = _field(node, definition["field"])
            if value is None:
                if definition.get("required"):
                    violations.append(f"node {node_id} lacks required property {definition['property_id']}")
                continue
            valid_type = {"string": isinstance(value, str), "boolean": isinstance(value, bool),
                          "number": isinstance(value, (int, float)) and not isinstance(value, bool),
                          "string-array": isinstance(value, list) and all(isinstance(v, str) for v in value)}
            if not valid_type.get(definition.get("value_type"), False):
                violations.append(f"node {node_id} has invalid property {definition['property_id']}")
        semantics = node.get("semantics") if isinstance(node.get("semantics"), dict) else {}
        if type_id == "tos.entity.temporal-assertion":
            time = semantics.get("time") if isinstance(semantics.get("time"), dict) else {}
            if time.get("normalization_status") not in {
                "structured-source",
                "source-literal-parsed",
                "source-literal-unparsed",
            }:
                violations.append(
                    f"temporal assertion {node_id} is not backed by an explicit source time value"
                )
        if type_id == "tos.entity.place":
            space = semantics.get("space") if isinstance(semantics.get("space"), dict) else {}
            if space.get("kind") != "place-identity":
                violations.append(f"Place {node_id} is missing place-identity semantics")
        if type_id == "tos.entity.navigation-region":
            space = semantics.get("space") if isinstance(semantics.get("space"), dict) else {}
            if space.get("kind") != "navigation-region":
                violations.append(f"navigation Region {node_id} is missing its non-Place marker")
        return violations

    node_occurrences = Counter(str(n.get('id')) for n in nodes)
    incremental = active_cache.get() is not None
    node_digests = {str(n.get('id')): _validation_digest(n) for n in nodes} if incremental else {}
    registry_digest = _validation_digest([entity_registry, relation_registry]) if incremental else None

    def checked(kind, identifier, context, compute, *, unique=True):
        cache = active_cache.get()
        if cache is None or not unique:
            return compute()
        return cache.memo('validate-' + kind, identifier, [
            Input('validation-registry', registry_digest),
            Input('validation-context:' + kind + ':' + identifier, context),
        ], compute)

    def references(ids):
        return [[str(identifier), node_digests.get(str(identifier))] for identifier in ids]

    for node in nodes:
        identifier = _string(node.get('id'))
        if identifier:
            if identifier in nodes_by_id:
                violations.append(f"duplicate knowledge node id {identifier}")
            nodes_by_id[identifier] = node
        violations.extend(checked('node', identifier or '<missing>', node_digests.get(identifier),
                                  lambda node=node: validate_node(node),
                                  unique=bool(identifier) and node_occurrences[identifier] == 1))

    def validate_endpoints(
        owner_id: str,
        relation_type_id: str,
        from_node: dict[str, Any] | None,
        to_node: dict[str, Any] | None,
        violations: list[str],
    ) -> None:
        entry = relation_entries.get(relation_type_id)
        if entry is None:
            violations.append(f"{owner_id} uses unregistered relation type {relation_type_id!r}")
            return
        if relation_type_id == fallback_relation_type_id:
            return
        if from_node is None or to_node is None:
            violations.append(f"{owner_id} has an unresolved normalized endpoint")
            return
        if not _type_is_a(_string(from_node.get("type_id")), _strings(entry.get("domain_type_ids")), entity_entries):
            violations.append(
                f"{owner_id} domain {_string(from_node.get('type_id'))!r} is outside {entry.get('domain_type_ids')}"
            )
        if not _type_is_a(_string(to_node.get("type_id")), _strings(entry.get("range_type_ids")), entity_entries):
            violations.append(
                f"{owner_id} range {_string(to_node.get('type_id'))!r} is outside {entry.get('range_type_ids')}"
            )

    relation_ids: set[str] = set()
    outgoing: dict[tuple[str, str], list[dict[str, Any]]] = {}
    incoming: dict[tuple[str, str], list[dict[str, Any]]] = {}
    claims_by_entity = {str(n.get("entity_id")): n for n in nodes if n.get("type_id") == "tos.entity.claim"}
    gaps: list[dict[str, str]] = []
    def validate_relation(relation):
        violations, gaps = [], []
        relation_id = _string(relation.get("id")) or "<missing relation id>"
        relation_type_id = _string(relation.get("relation_type_id"))
        mapping = relation.get("predicate_mapping") if isinstance(relation.get("predicate_mapping"), dict) else {}
        if relation_type_id not in relation_entries:
            violations.append(f"relation {relation_id} uses unregistered type {relation_type_id!r}")
            return violations, gaps
        entry = relation_entries[relation_type_id]
        if entry.get("abstract"):
            violations.append(f"relation {relation_id} instantiates abstract type {relation_type_id}")
        if entry.get("evidence_required") and not _strings(relation.get("source_refs")):
            violations.append(f"relation {relation_id} has no evidence-bearing source ref")
        posture = (relation.get("epistemic") or {}).get("review_posture")
        if entry.get("review_requirement") != "none" and not posture:
            violations.append(f"relation {relation_id} has no recorded review state")
        if posture == "not-recorded":
            gaps.append({"id": relation_id, "kind": "review-not-recorded"})
        expected_status = "unmapped" if relation_type_id == fallback_relation_type_id else "mapped"
        if mapping.get("status") != expected_status:
            violations.append(f"relation {relation_id} has inconsistent predicate mapping status")
        if not _string(mapping.get("source_predicate_id")):
            violations.append(f"relation {relation_id} does not preserve source_predicate_id")
        expected_type = _relation_mappings.get(
            (str(relation.get('source_graph')), str(mapping.get('source_predicate_id')), 'edge'),
            fallback_relation_type_id,
        )
        if relation_type_id != expected_type:
            violations.append(f'relation {relation_id} disagrees with its registered source mapping')
        left = nodes_by_id.get(str(relation.get("from_id")))
        right = nodes_by_id.get(str(relation.get("to_id")))
        validate_endpoints(relation_id, relation_type_id, left, right, violations)
        attrs = relation.get('attributes') or {}
        if entry.get('assertion_mode') == 'reified-claim':
            supporting_claim = claims_by_entity.get(str(attrs.get('claim_ref')))
            if not supporting_claim:
                violations.append(f'relation {relation_id} lacks a resolved supporting Claim')
            elif left and right and left.get('type_id') != 'tos.entity.claim':
                contract = supporting_claim.get('semantics', {}).get('claim', {})
                if (left.get('entity_id'), right.get('entity_id')) != (contract.get('subject_entity_id'), contract.get('object_entity_id')):
                    violations.append(f'relation {relation_id} disagrees with supporting Claim endpoints')
        if relation_type_id == "tos.relation.projects" and left and right:
            if left.get("entity_id") != right.get("entity_id"):
                violations.append(f"projection relation {relation_id} connects different entity_id values")
        if relation_type_id == "tos.relation.same-as":
            if left and right and not (
                _type_is_a(str(left.get('type_id')), [str(right.get('type_id'))], entity_entries)
                or _type_is_a(str(right.get('type_id')), [str(left.get('type_id'))], entity_entries)
            ):
                violations.append(f"same_as relation {relation_id} connects incompatible entity types")
            if len(_strings(relation.get("source_refs"))) < 1:
                violations.append(f"same_as relation {relation_id} has no evidence-bearing source ref")
            if (relation.get("epistemic") or {}).get("review_posture") not in {"accepted", "verified", "reviewed_equivalence"}:
                violations.append(f"same_as relation {relation_id} lacks accepted review posture")
            attrs = relation.get("attributes") or {}
            claim_node = claims_by_entity.get(str(attrs.get("claim_ref")))
            claim = (claim_node or {}).get("semantics", {}).get("claim", {})
            review = nodes_by_id.get(str(attrs.get("review_node_id")))
            review_data = (review or {}).get("attributes", {})
            exact_pair = left and right and {left.get("entity_id"), right.get("entity_id")} == {claim.get("subject_entity_id"), claim.get("object_entity_id")}
            reviewed_exact_claim = (review and _type_is_a(str(review.get("type_id")), ['tos.entity.review'], entity_entries)
                                    and review_data.get("claim_ref") == claim.get("claim_id")
                                    and review_data.get("claim_version") == claim.get("claim_version")
                                    and review_data.get("decision") == "accepted")
            evidence = claim.get("evidence_node_ids", [])
            if not (exact_pair and claim.get('relation_type_id') == relation_type_id
                    and claim.get("claim_version") is not None and reviewed_exact_claim and evidence
                    and all(e in nodes_by_id and _type_is_a(str(nodes_by_id[e].get("type_id")), ['tos.entity.evidence'], entity_entries) for e in evidence)):
                violations.append(f"same_as relation {relation_id} lacks resolved evidence and exact-version review")
        return violations, gaps

    relation_occurrences = Counter(str(r.get('id')) for r in relations)
    relation_digests = {str(r.get('id')): _validation_digest(r) for r in relations} if incremental else {}
    for relation in relations:
        identifier = _string(relation.get('id')) or '<missing relation id>'
        relation_type = _string(relation.get('relation_type_id'))
        if relation_type in relation_entries:
            if identifier in relation_ids or identifier == '<missing relation id>':
                violations.append(f"duplicate or missing relation id {identifier}")
            relation_ids.add(identifier)
            outgoing.setdefault((str(relation.get('from_id')), relation_type), []).append(relation)
            incoming.setdefault((str(relation.get('to_id')), relation_type), []).append(relation)
        attrs = relation.get('attributes') or {}
        supporting = claims_by_entity.get(str(attrs.get('claim_ref'))) or {}
        evidence = supporting.get('semantics', {}).get('claim', {}).get('evidence_node_ids', [])
        context = [relation_digests.get(identifier),
                   references([relation.get('from_id'), relation.get('to_id'), attrs.get('review_node_id'), *evidence]),
                   _validation_digest(supporting)] if incremental else None
        errors, missing = checked('relation', identifier, context,
                                  lambda relation=relation: validate_relation(relation),
                                  unique=relation_occurrences[identifier] == 1)
        violations.extend(errors)
        gaps.extend(missing)

    for table, maximum_key in ((outgoing, "per_subject_max"), (incoming, "per_object_max")):
        for (endpoint, relation_type), edges in table.items():
            maximum = relation_entries[relation_type].get("cardinality", {}).get(maximum_key)
            # Semantic cardinality constrains one assertion context. Distinct
            # conflicting claims are preserved, not forced into one global fact.
            scoped_counts = Counter((e.get('attributes') or {}).get('claim_ref') if relation_entries[relation_type].get('assertion_mode') == 'reified-claim' else None for e in edges)
            if maximum is not None and max(scoped_counts.values()) > maximum:
                violations.append(f"{endpoint} violates {relation_type} {maximum_key}={maximum}")

    def validate_claim(node):
        violations, gaps = [], []
        claim_count = 0
        semantics = node.get("semantics") if isinstance(node.get("semantics"), dict) else {}
        claim = semantics.get("claim") if isinstance(semantics.get("claim"), dict) else None
        if not claim or not claim.get("subject_node_id") or not claim.get("object_node_id"):
            violations.append(f"claim {node.get('id')} lacks its subject/object contract")
            return violations, gaps, claim_count
        for predicate, field in (("tos.relation.has-subject", "subject_node_id"), ("tos.relation.has-object", "object_node_id")):
            edges = outgoing.get((str(node["id"]), predicate), [])
            if len(edges) != 1 or edges[0].get("to_id") != claim[field]:
                violations.append(f"claim {node['id']} must have exactly one consistent {predicate}")
        for evidence_id in claim.get("evidence_node_ids", []):
            if evidence_id not in nodes_by_id:
                violations.append(f"claim {node['id']} has unresolved evidence {evidence_id}")
        if not claim.get("evidence_node_ids"):
            gaps.append({"id": str(node['id']), "kind": "claim-evidence-not-projected"})
        claim_count += 1
        relation_type_id = _string(claim.get("relation_type_id")) or fallback_relation_type_id
        validate_endpoints(
            f"claim {claim.get('claim_id')}",
            relation_type_id,
            nodes_by_id.get(str(claim.get("subject_node_id"))),
            nodes_by_id.get(str(claim.get("object_node_id"))),
            violations,
        )
        return violations, gaps, claim_count

    claim_count = 0
    for node in nodes:
        if node.get('type_id') != 'tos.entity.claim':
            continue
        identifier = str(node.get('id'))
        semantics = node.get('semantics') if isinstance(node.get('semantics'), dict) else {}
        claim = semantics.get('claim') if isinstance(semantics.get('claim'), dict) else {}
        adjacent = [e for predicate in ('tos.relation.has-subject', 'tos.relation.has-object')
                    for e in outgoing.get((identifier, predicate), [])]
        context = [node_digests.get(identifier),
                   _validation_digest(adjacent),
                   references([claim.get('subject_node_id'), claim.get('object_node_id'),
                               *claim.get('evidence_node_ids', [])])] if incremental else None
        errors, missing, count = checked('claim', identifier, context,
                                         lambda node=node: validate_claim(node),
                                         unique=node_occurrences[identifier] == 1)
        violations.extend(errors)
        gaps.extend(missing)
        claim_count += count

    return {
        "valid": not violations,
        "violations": sorted(set(violations)),
        "registered_node_count": sum(_string(node.get("type_id")) in entity_entries for node in nodes),
        "unmapped_node_count": sum(node.get("type_id") == fallback_type_id for node in nodes),
        "registered_relation_count": sum(_string(item.get("relation_type_id")) in relation_entries for item in relations),
        "unmapped_relation_count": sum(item.get("relation_type_id") == fallback_relation_type_id for item in relations),
        "claim_contract_count": claim_count,
        "cross_layer_relation_count": sum(item.get("source_graph") == "semantic-interchange" for item in relations),
        "gaps": gaps,
    }


def _allowed_field(field: str, kind: str) -> bool:
    fields = NODE_FIELDS if kind == "node" else RELATION_FIELDS
    if field in fields:
        return True
    if not _ATTRIBUTE_FIELD.fullmatch(field):
        return False
    return not any(segment in _UNSAFE_PATH_SEGMENTS for segment in field.split("."))


def _filter_value(value: Any, name: str) -> Any:
    values = value if isinstance(value, list) else [value]
    if len(values) > 100:
        raise ValueError(f"{name} must contain at most 100 scalar values")
    normalized: list[Any] = []
    for item in values:
        if isinstance(item, (dict, list)):
            raise ValueError(f"{name} must contain only scalar values")
        if isinstance(item, float) and not math.isfinite(item):
            raise ValueError(f"{name} numbers must be finite")
        if isinstance(item, str) and len(item) > 1024:
            raise ValueError(f"{name} strings must contain at most 1024 characters")
        normalized.append(item)
    return normalized if isinstance(value, list) else normalized[0]


def _bounded_integer(value: Any, name: str, default: int, minimum: int, maximum: int) -> int:
    if value is None:
        return default
    if isinstance(value, bool):
        raise ValueError(f"{name} must be between {minimum} and {maximum}")
    try:
        parsed = int(value)
    except (TypeError, ValueError) as exc:
        raise ValueError(f"{name} must be between {minimum} and {maximum}") from exc
    if parsed < minimum or parsed > maximum:
        raise ValueError(f"{name} must be between {minimum} and {maximum}")
    return parsed


def _normalize_filter_group(value: Any, kind: str) -> dict[str, Any]:
    source = _strict_object(value, f"{kind}_query", {"enabled", "match", "filters"})
    enabled = source.get("enabled", True)
    if not isinstance(enabled, bool):
        raise ValueError(f"{kind}_query.enabled must be a boolean")
    match = source.get("match", "all")
    if match not in {"all", "any"}:
        raise ValueError(f"{kind}_query.match must be all or any")
    filters = source.get("filters", [])
    if not isinstance(filters, list) or len(filters) > MAX_FILTERS:
        raise ValueError(f"{kind}_query.filters must contain at most {MAX_FILTERS} filters")
    normalized: list[dict[str, Any]] = []
    for item in filters:
        if not isinstance(item, dict):
            raise ValueError(f"{kind} filters must be objects")
        unknown = sorted(set(item) - {"field", "op", "value"})
        if unknown:
            raise ValueError(f"unknown {kind} filter fields: {', '.join(unknown)}")
        field = _string(item.get("field")) or ""
        if not _allowed_field(field, kind):
            raise ValueError(f"unsupported {kind} filter field: {field}")
        operator = _string(item.get("op")) or ""
        if operator not in FILTER_OPERATORS:
            raise ValueError(f"unsupported {kind} filter operator: {operator}")
        if "value" not in item:
            raise ValueError(f"{kind} filter {field} is missing value")
        filter_value = _filter_value(item["value"], f"{kind} filter {field}")
        if operator in {"eq", "neq"} and isinstance(filter_value, list):
            raise ValueError(f"{kind} {operator} filter {field} requires a scalar value")
        if operator == "exists" and not isinstance(filter_value, bool):
            raise ValueError(f"{kind} exists filter {field} requires a boolean value")
        if operator == "prefix" and not isinstance(filter_value, str):
            raise ValueError(f"{kind} prefix filter {field} requires a string value")
        if operator in {"gt", "gte", "lt", "lte"} and (
            isinstance(filter_value, bool) or not isinstance(filter_value, (int, float))
        ):
            raise ValueError(f"{kind} numeric filter {field} requires a number value")
        normalized.append({"field": field, "op": operator, "value": filter_value})
    return {"enabled": enabled, "match": match, "filters": normalized}


def _normalize_sorts(value: Any, kind: str) -> list[dict[str, str]]:
    if value is None:
        return [{"field": "id", "direction": "asc"}]
    if not isinstance(value, list) or len(value) > 8:
        raise ValueError(f"sort_{kind}s must contain at most 8 fields")
    result: list[dict[str, str]] = []
    for item in value:
        if not isinstance(item, dict):
            raise ValueError(f"sort_{kind}s entries must be objects")
        unknown = sorted(set(item) - {"field", "direction"})
        if unknown:
            raise ValueError(f"unknown {kind} sort fields: {', '.join(unknown)}")
        field = _string(item.get("field")) or ""
        if not _allowed_field(field, kind):
            raise ValueError(f"unsupported {kind} sort field: {field}")
        direction = _string(item.get("direction")) or "asc"
        if direction not in {"asc", "desc"}:
            raise ValueError(f"sort direction must be asc or desc: {field}")
        result.append({"field": field, "direction": direction})
    return result or [{"field": "id", "direction": "asc"}]


def _normalize_paths(value: Any) -> list[dict[str, Any]]:
    if value is None:
        return []
    if not isinstance(value, list) or len(value) > 4:
        raise ValueError("path_query must contain at most 4 conditions")
    result = []
    for raw in value:
        condition = _strict_object(raw, "path condition", {"path_id", "quantifier", "steps"})
        path_id = _string(condition.get("path_id")) or ""
        if not _IDENTIFIER.fullmatch(path_id) or any(p['path_id'] == path_id for p in result):
            raise ValueError("path_id must be a unique safe stable identifier")
        quantifier = condition.get("quantifier", "exists")
        if quantifier not in {"exists", "not_exists"}:
            raise ValueError("path quantifier must be exists or not_exists")
        steps = condition.get("steps")
        if not isinstance(steps, list) or not 1 <= len(steps) <= 4:
            raise ValueError("path steps must contain between 1 and 4 steps")
        normalized = []
        for raw_step in steps:
            step = _strict_object(raw_step, "path step", {"direction", "node_query", "relation_query"})
            direction = step.get("direction", "outgoing")
            if direction not in {"incoming", "outgoing", "either"}:
                raise ValueError("path direction must be outgoing, incoming, or either")
            normalized.append({"direction": direction,
                               "node_query": _normalize_filter_group(step.get("node_query"), "node"),
                               "relation_query": _normalize_filter_group(step.get("relation_query"), "relation")})
        result.append({"path_id": path_id, "quantifier": quantifier, "steps": normalized})
    return result


def _path_witness(start: str, condition: dict[str, Any], nodes: dict, adjacency: dict,
                  budget: list[int]) -> dict[str, Any] | None:
    # Fixed-length walks; revisiting a node is legal. Budget exhaustion is an
    # error, never a false negative (especially important for not_exists).
    def walk(current, index, node_ids, relation_ids):
        if index == len(condition['steps']):
            return {"path_id": condition['path_id'], "node_ids": node_ids, "relation_ids": relation_ids}
        step = condition['steps'][index]
        for relation in adjacency.get(current, []):
            budget[0] -= 1
            if budget[0] < 0:
                raise ValueError("path query exceeded the execution safety ceiling")
            if not step['relation_query']['enabled'] or not _matches_group(relation, step['relation_query']):
                continue
            for neighbor in _relation_neighbors(relation, current, step['direction']):
                if neighbor not in nodes or not step['node_query']['enabled'] or not _matches_group(nodes[neighbor], step['node_query']):
                    continue
                found = walk(neighbor, index + 1, [*node_ids, neighbor], [*relation_ids, relation['id']])
                if found is not None:
                    return found
        return None
    return walk(start, 0, [start], [])


def normalize_lens_spec(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError("lens spec must be an object")
    allowed_top = {
        "schema_version", "lens_id", "title", "description", "language", "sources", "seed",
        "node_query", "relation_query", "traversal", "composition", "presentation", "limits", "detail", "path_query", "explain", "pagination",
    }
    unknown = sorted(set(value) - allowed_top)
    if unknown:
        raise ValueError(f"unknown lens spec fields: {', '.join(unknown)}")
    if value.get("schema_version") != "tos_lens_spec_v1":
        raise ValueError("lens spec schema_version must be tos_lens_spec_v1")
    lens_id = _string(value.get("lens_id")) or ""
    if not _IDENTIFIER.fullmatch(lens_id):
        raise ValueError("lens_id must be a safe stable identifier")
    sources = value.get("sources", list(KNOWLEDGE_SOURCES))
    if not isinstance(sources, list) or not sources:
        raise ValueError("sources must be a non-empty array")
    if len(sources) > len(KNOWLEDGE_SOURCES):
        raise ValueError(f"sources must contain at most {len(KNOWLEDGE_SOURCES)} values")
    if any(not isinstance(item, str) or not item for item in sources):
        raise ValueError("sources must contain only non-empty strings")
    normalized_sources = list(sources)
    if len(set(normalized_sources)) != len(normalized_sources):
        raise ValueError("sources must contain unique values")
    unknown_sources = sorted(set(normalized_sources) - set(KNOWLEDGE_SOURCES))
    if unknown_sources:
        raise ValueError(f"unsupported knowledge sources: {', '.join(unknown_sources)}")

    selection = _strict_object(value.get("seed"), "seed", {"focus_node_id", "node_ids", "text_query"})
    focus_value = selection.get("focus_node_id")
    if focus_value is None:
        focus_node_id = None
    else:
        focus_node_id = _string(focus_value)
        if focus_node_id is None:
            raise ValueError("seed.focus_node_id must be a non-empty string or null")
        if len(focus_node_id) > 1024:
            raise ValueError("seed.focus_node_id exceeds 1024 characters")
    node_ids = _strict_strings(selection.get("node_ids"), "seed.node_ids", maximum=100)
    text_query = _string(selection.get("text_query")) or ""
    if len(text_query) > 256:
        raise ValueError("seed.text_query exceeds 256 characters")

    traversal_value = _strict_object(value.get("traversal"), "traversal", {"depth", "direction", "predicate_ids", "profile"})
    profile = traversal_value.get("profile", "all")
    if profile not in {"all", "overview"}:
        raise ValueError("traversal.profile must be all or overview")
    depth = _bounded_integer(traversal_value.get("depth"), "traversal.depth", 0, 0, MAX_TRAVERSAL_DEPTH)
    direction = _string(traversal_value.get("direction")) or "either"
    if direction not in {"outgoing", "incoming", "either"}:
        raise ValueError("traversal.direction must be outgoing, incoming, or either")
    predicate_ids = _strict_strings(
        traversal_value.get("predicate_ids"),
        "traversal.predicate_ids",
        maximum=100,
        unique=True,
    )

    composition_value = _strict_object(
        value.get("composition"),
        "composition",
        {"endpoint_policy", "group_by", "sort_nodes", "sort_relations"},
    )
    endpoint_policy = _string(composition_value.get("endpoint_policy")) or "both"
    if endpoint_policy not in {"both", "either", "independent"}:
        raise ValueError("composition.endpoint_policy must be both, either, or independent")
    group_by = _strict_strings(
        composition_value.get("group_by"),
        "composition.group_by",
        maximum=4,
        unique=True,
    )
    for field in group_by:
        if not (_allowed_field(field, "node") or _allowed_field(field, "relation")):
            raise ValueError(f"unsupported group field: {field}")

    presentation_value = _strict_object(
        value.get("presentation"),
        "presentation",
        {"layout", "color_by", "lane_by", "size_by", "inspector_fields"},
    )
    layout = _string(presentation_value.get("layout")) or "auto"
    if layout not in LAYOUTS:
        raise ValueError(f"unsupported presentation layout: {layout}")
    presentation_fields: dict[str, str | None] = {}
    for key in ("color_by", "lane_by", "size_by"):
        field = _string(presentation_value.get(key))
        if field and not (_allowed_field(field, "node") or _allowed_field(field, "relation")):
            raise ValueError(f"unsupported presentation field: {field}")
        presentation_fields[key] = field
    inspector_fields = _strict_strings(
        presentation_value.get("inspector_fields"),
        "presentation.inspector_fields",
        maximum=32,
        unique=True,
    )
    for field in inspector_fields:
        # Inspector selection can expose a whole stable envelope as well as a leaf.
        if field not in {"display", "display.summary", "epistemic", "source_refs", "attributes"} and not (
            _allowed_field(field, "node") or _allowed_field(field, "relation")
        ):
            raise ValueError(f"unsupported inspector field: {field}")

    limits_value = _strict_object(value.get("limits"), "limits", {"nodes", "relations", "groups"})
    limits = {
        "nodes": _bounded_integer(limits_value.get("nodes"), "nodes", 200, 1, MAX_NODE_LIMIT),
        "relations": _bounded_integer(limits_value.get("relations"), "relations", 400, 0, MAX_RELATION_LIMIT),
        "groups": _bounded_integer(limits_value.get("groups"), "groups", 100, 1, MAX_GROUP_LIMIT),
    }
    language = _string(value.get("language")) or "auto"
    detail = value.get('detail', 'full')
    if detail not in {'full', 'compact'}:
        raise ValueError('detail must be full or compact')
    if language not in {"auto", "ru", "en", "original"}:
        raise ValueError("language must be auto, ru, en, or original")
    explain = value.get('explain', False)
    if not isinstance(explain, bool):
        raise ValueError('explain must be a boolean')
    return {
        "schema_version": "tos_lens_spec_v1",
        "lens_id": lens_id,
        "title": _lens_localized(value.get("title"), _humanize(lens_id), "title"),
        "description": _lens_localized(
            value.get("description"),
            f"Declarative knowledge lens {lens_id}.",
            "description",
        ),
        "language": language,
        "detail": detail,
        "path_query": _normalize_paths(value.get('path_query')),
        "explain": explain,
        "pagination": normalize_pagination(value.get('pagination')),
        "sources": normalized_sources,
        "seed": {"focus_node_id": focus_node_id, "node_ids": node_ids, "text_query": text_query},
        "node_query": _normalize_filter_group(value.get("node_query"), "node"),
        "relation_query": _normalize_filter_group(value.get("relation_query"), "relation"),
        "traversal": {"depth": depth, "direction": direction, "predicate_ids": predicate_ids, "profile": profile},
        "composition": {
            "endpoint_policy": endpoint_policy,
            "group_by": group_by,
            "sort_nodes": _normalize_sorts(composition_value.get("sort_nodes"), "node"),
            "sort_relations": _normalize_sorts(composition_value.get("sort_relations"), "relation"),
        },
        "presentation": {
            "layout": layout,
            **presentation_fields,
            "inspector_fields": inspector_fields or ["display", "epistemic", "source_refs"],
        },
        "limits": limits,
    }


def _field(item: dict[str, Any], path: str) -> Any:
    current: Any = item
    for segment in path.split("."):
        if not isinstance(current, dict) or segment not in current:
            return None
        current = current[segment]
    return current


def _number(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    return None


def _matches_filter(item: dict[str, Any], rule: dict[str, Any]) -> bool:
    actual = _field(item, str(rule["field"]))
    expected = rule["value"]
    operator = rule["op"]
    if operator == "exists":
        return (actual is not None) is bool(expected)
    if operator == "eq":
        return actual == expected or (isinstance(actual, list) and expected in actual)
    if operator == "neq":
        return not _matches_filter(item, {**rule, "op": "eq"})
    if operator == "in":
        values = expected if isinstance(expected, list) else [expected]
        return bool(set(actual) & set(values)) if isinstance(actual, list) else actual in values
    if operator == "contains":
        if isinstance(actual, list):
            values = expected if isinstance(expected, list) else [expected]
            return all(value in actual for value in values)
        if isinstance(expected, list):
            return False
        return str(expected).lower() in str(actual or "").lower()
    if operator == "prefix":
        return str(actual or "").lower().startswith(str(expected).lower())
    left = _number(actual)
    right = _number(expected)
    if left is None or right is None:
        return False
    return {
        "gt": left > right,
        "gte": left >= right,
        "lt": left < right,
        "lte": left <= right,
    }.get(str(operator), False)


def _matches_group(item: dict[str, Any], group: dict[str, Any]) -> bool:
    filters = group["filters"]
    if not filters:
        return True
    results = [_matches_filter(item, rule) for rule in filters]
    return all(results) if group["match"] == "all" else any(results)


def _searchable(item: dict[str, Any]) -> str:
    return json.dumps(item, ensure_ascii=False, sort_keys=True).lower()


def _sort_items(items: Iterable[dict[str, Any]], rules: list[dict[str, str]]) -> list[dict[str, Any]]:
    result = list(items)
    # Stable sorting from the least significant field makes mixed directions deterministic.
    result.sort(key=lambda item: str(item.get("id") or ""))
    for rule in reversed(rules):
        field = rule["field"]
        reverse = rule["direction"] == "desc"
        result.sort(key=lambda item: str(_field(item, field) or "").lower(), reverse=reverse)
    return result


def _relation_neighbors(relation: dict[str, Any], node_id: str, direction: str) -> list[str]:
    left = str(relation.get("from_id") or "")
    right = str(relation.get("to_id") or "")
    neighbors: list[str] = []
    if direction in {"outgoing", "either"} and left == node_id:
        neighbors.append(right)
    if direction in {"incoming", "either"} and right == node_id:
        neighbors.append(left)
    return [item for item in neighbors if item]


def _groups(nodes: list[dict[str, Any]], relations: list[dict[str, Any]], fields: list[str], limit: int) -> list[dict[str, Any]]:
    groups: list[dict[str, Any]] = []
    for field in fields:
        values: dict[str, dict[str, Any]] = {}
        for kind, items in (("node", nodes), ("relation", relations)):
            for item in items:
                raw = _field(item, field)
                members = raw if isinstance(raw, list) else [raw]
                for member in members:
                    if member is None:
                        continue
                    key = str(member)
                    entry = values.setdefault(key, {"field": field, "value": member, "node_ids": [], "relation_ids": []})
                    entry[f"{kind}_ids"].append(item["id"])
        for key in sorted(values, key=str.casefold):
            entry = values[key]
            entry["node_count"] = len(entry["node_ids"])
            entry["relation_count"] = len(entry["relation_ids"])
            groups.append(entry)
            if len(groups) >= limit:
                return groups
    return groups


@lru_cache(maxsize=4096)
def _short_digest_string(value: str) -> bytes:
    """Reuse bounded small wire tokens; the caller bypasses long text values."""
    encoded = value.encode("utf-8")
    return b"s" + str(len(encoded)).encode("ascii") + b":" + encoded


def _stable_digest(value: Any) -> str:
    digest = hashlib.sha256()

    def update(item: Any) -> None:
        if item is None:
            digest.update(b"n;")
        elif isinstance(item, bool):
            digest.update(b"b1;" if item else b"b0;")
        elif isinstance(item, (int, float)):
            number = float(item)
            if not math.isfinite(number):
                raise ValueError("stable digest cannot encode a non-finite number")
            if number == 0:
                number = 0.0
            digest.update(b"d")
            digest.update(struct.pack(">d", number).hex().encode("ascii"))
            digest.update(b";")
        elif isinstance(item, str):
            if len(item) <= 256:
                digest.update(_short_digest_string(item))
            else:
                encoded = item.encode("utf-8")
                digest.update(b"s")
                digest.update(str(len(encoded)).encode("ascii"))
                digest.update(b":")
                digest.update(encoded)
        elif isinstance(item, list):
            digest.update(b"a")
            digest.update(str(len(item)).encode("ascii"))
            digest.update(b"[")
            for child in item:
                update(child)
            digest.update(b"]")
        elif isinstance(item, dict):
            keys = sorted(item, key=str)
            digest.update(b"o")
            digest.update(str(len(keys)).encode("ascii"))
            digest.update(b"{")
            for key in keys:
                update(str(key))
                update(item[key])
            digest.update(b"}")
        else:
            raise TypeError(f"stable digest cannot encode {type(item).__name__}")

    update(value)
    return digest.hexdigest()


def _content_revision(item: dict[str, Any]) -> str:
    return _stable_digest({key: value for key, value in item.items() if key != "content_revision"})


def _stamp_content_revision(item: dict[str, Any]) -> None:
    item["content_revision"] = _content_revision(item)


def _resolve_focus_node(nodes: list[dict[str, Any]], requested_id: str | None) -> dict[str, Any] | None:
    if requested_id is None:
        return None
    exact = [item for item in nodes if str(item.get("id")) == requested_id]
    if exact:
        return exact[0]
    entity_matches = [item for item in nodes if str(item.get("entity_id")) == requested_id]
    if entity_matches:
        source_priority = {
            "source-navigation": 0,
            "canon": 1,
            "source-claims": 2,
            "philosophy": 3,
            "candidate-intake": 4,
            "repository": 5,
            "semantic-interchange": 6,
        }
        return min(
            entity_matches,
            key=lambda item: (source_priority.get(str(item.get("source_graph")), 99), str(item.get("id"))),
        )
    native = [item for item in nodes if str(item.get("native_id")) == requested_id]
    if not native:
        raise ValueError(f"unknown ToS knowledge focus: {requested_id}")
    if len(native) > 1:
        matches = ", ".join(sorted(str(item.get("id")) for item in native))
        raise ValueError(
            f"ambiguous ToS knowledge focus {requested_id}: {matches}; use a namespaced node id"
        )
    return native[0]


def _focus_payload(
    spec: dict[str, Any],
    nodes: Iterable[dict[str, Any]],
    resolved_node: dict[str, Any] | None = None,
) -> dict[str, Any] | None:
    requested_id = spec["seed"]["focus_node_id"]
    if requested_id is None:
        return None
    materialized = list(nodes)
    node = None
    if resolved_node is not None:
        node = next(
            (
                item
                for item in materialized
                if str(item.get("id")) == str(resolved_node.get("id"))
            ),
            None,
        )
    if node is None and resolved_node is None:
        node = _resolve_focus_node(materialized, requested_id)
    if node is None:
        raise RuntimeError("resolved knowledge focus is missing from the lens result")
    resolved_by = (
        "id"
        if node["id"] == requested_id
        else "entity_id"
        if node["entity_id"] == requested_id
        else "native_id"
    )
    return {
        "requested_id": requested_id,
        "resolved_by": resolved_by,
        "node_id": node["id"],
        "entity_id": node["entity_id"],
        "native_id": node["native_id"],
        "source_graph": node["source_graph"],
        "kind_id": node["kind_id"],
        "type_id": node["type_id"],
        "display": node["display"],
    }


def execute_knowledge_lens(graph: dict[str, Any], spec_value: Any) -> dict[str, Any]:
    spec = normalize_lens_spec(spec_value)
    sources = set(spec["sources"])
    nodes = [item for item in _objects(graph.get("nodes")) if item.get("source_graph") in sources]
    relations = [item for item in _objects(graph.get("relations")) if item.get("source_graph") in sources]
    all_nodes_by_id = {str(item["id"]): item for item in nodes}
    focus_node = _resolve_focus_node(nodes, spec["seed"]["focus_node_id"])
    selected_ids = set(spec["seed"]["node_ids"])
    text_query = str(spec["seed"]["text_query"]).lower()

    candidates = []
    adjacency: dict[str, list] = {}
    for relation in sorted(relations, key=lambda item: item['id']):
        for endpoint in set((relation['from_id'], relation['to_id'])):
            adjacency.setdefault(endpoint, []).append(relation)
    path_budget = [100_000]
    path_proofs = {}
    if spec["node_query"]["enabled"]:
        for node in nodes:
            if selected_ids and not selected_ids.intersection({str(node["id"]), str(node["native_id"]), str(node["entity_id"])}):
                continue
            if text_query and text_query not in _searchable(node):
                continue
            if _matches_group(node, spec["node_query"]):
                proofs = []
                for condition in spec['path_query']:
                    witness = _path_witness(node['id'], condition, all_nodes_by_id, adjacency, path_budget)
                    if (witness is not None) != (condition['quantifier'] == 'exists'):
                        break
                    proofs.append(witness or {'path_id': condition['path_id'], 'absence_in_scope': True})
                else:
                    candidates.append(node)
                    path_proofs[node['id']] = proofs
    sorted_nodes = _sort_items(candidates, spec["composition"]["sort_nodes"])
    selected_nodes: dict[str, dict[str, Any]] = {}
    inclusion = {}
    if focus_node is not None:
        selected_nodes[str(focus_node["id"])] = focus_node
        inclusion[focus_node['id']] = {'kind': 'focus'}
    for item in sorted_nodes:
        if len(selected_nodes) >= spec["limits"]["nodes"]:
            break
        selected_nodes.setdefault(str(item["id"]), item)
        inclusion.setdefault(item['id'], {'kind': 'selector', 'path_witnesses': path_proofs.get(item['id'], [])})

    relation_candidates = []
    if spec["relation_query"]["enabled"]:
        relation_candidates = [
            item
            for item in relations
            if _matches_group(item, spec["relation_query"])
            and (spec['traversal']['profile'] != 'overview' or item.get('predicate_id') not in OVERVIEW_EXCLUDED_PREDICATES)
            and (
                not spec["traversal"]["predicate_ids"]
                or item.get("predicate_id") in set(spec["traversal"]["predicate_ids"])
            )
        ]
    relation_candidates = _sort_items(relation_candidates, spec["composition"]["sort_relations"])

    frontier = list(selected_nodes)
    traversed_relation_ids: set[str] = set()
    for depth in range(spec["traversal"]["depth"]):
        next_frontier: list[str] = []
        frontier_set = set(frontier)
        for relation in relation_candidates:
            relation_id = str(relation["id"])
            touched = False
            for node_id in frontier:
                for neighbor_id in _relation_neighbors(relation, node_id, spec["traversal"]["direction"]):
                    touched = True
                    if neighbor_id not in selected_nodes and neighbor_id in all_nodes_by_id and len(selected_nodes) < spec["limits"]["nodes"]:
                        selected_nodes[neighbor_id] = all_nodes_by_id[neighbor_id]
                        inclusion[neighbor_id] = {'kind': 'traversal', 'via_node_id': node_id,
                                                  'via_relation_id': relation_id, 'depth': depth + 1}
                        next_frontier.append(neighbor_id)
            if touched and len(traversed_relation_ids) < spec["limits"]["relations"]:
                traversed_relation_ids.add(relation_id)
        frontier = list(dict.fromkeys(next_frontier))
        if not frontier:
            break

    endpoint_policy = spec["composition"]["endpoint_policy"]
    selection_basis = set(selected_nodes)
    selected_relations: list[dict[str, Any]] = []
    eligible_relation_count = 0
    for relation in relation_candidates:
        left = str(relation["from_id"])
        right = str(relation["to_id"])
        left_selected = left in selection_basis
        right_selected = right in selection_basis
        allowed = (
            (endpoint_policy == "both" and left_selected and right_selected)
            or (endpoint_policy == "either" and (left_selected or right_selected))
            or endpoint_policy == "independent"
            or str(relation["id"]) in traversed_relation_ids
        )
        if not allowed:
            continue
        eligible_relation_count += 1
        if len(selected_relations) >= spec["limits"]["relations"]:
            continue
        missing = list(dict.fromkeys(node_id for node_id in (left, right) if node_id not in selected_nodes))
        if len(selected_nodes) + len(missing) > spec["limits"]["nodes"]:
            continue
        for node_id in missing:
            if node_id in all_nodes_by_id:
                selected_nodes[node_id] = all_nodes_by_id[node_id]
                inclusion[node_id] = {'kind': 'endpoint', 'via_relation_id': relation['id']}
        if left in selected_nodes and right in selected_nodes:
            selected_relations.append(relation)

    final_nodes = _sort_items(selected_nodes.values(), spec["composition"]["sort_nodes"])
    final_relations = _sort_items(selected_relations, spec["composition"]["sort_relations"])
    focus = _focus_payload(spec, final_nodes, focus_node)
    groups = _groups(final_nodes, final_relations, spec["composition"]["group_by"], spec["limits"]["groups"])
    refs = sorted({ref for item in [*final_nodes, *final_relations] for ref in _strings(item.get("source_refs"))})
    missing_node_summaries = sum(item["display"]["summary_state"] == "missing" for item in final_nodes)
    missing_relation_explanations = sum(item["display"]["explanation_state"] == "missing" for item in final_relations)
    nodes_without_source_summary = sum(
        item["display"]["provenance"].get("source_summary_available") is False
        for item in final_nodes
    )
    relations_without_source_explanation = sum(
        item["display"]["provenance"].get("source_explanation_available") is False
        for item in final_relations
    )
    matched_node_ids = {str(item["id"]) for item in candidates}
    if focus_node is not None:
        matched_node_ids.add(str(focus_node["id"]))
    truncated_nodes = max(0, len(matched_node_ids) - spec["limits"]["nodes"])
    truncated_relations = max(0, eligible_relation_count - len(final_relations))
    fingerprint_material = {
        "execution_version": "tos-lens-execution-v2",
        "source_revision": graph.get("source_revision"),
        "lens": {k: v for k, v in spec.items() if k != 'pagination'},
        "nodes": [[item["id"], item["content_revision"]] for item in final_nodes],
        "relations": [[item["id"], item["content_revision"]] for item in final_relations],
        "groups": groups,
    }
    return paginate_lens({
        "schema": "tos_lens_result_v1",
        "source_revision": str(graph.get("source_revision") or ""),
        "lens": spec,
        "fingerprint": _stable_digest(fingerprint_material),
        "presentation": spec["presentation"],
        "focus": focus,
        **({'inclusion': {'nodes': inclusion,
                          'relations': {r['id']: {'kind': 'traversal' if r['id'] in traversed_relation_ids else 'endpoint-policy',
                                                  'endpoint_policy': endpoint_policy} for r in final_relations},
                          'authority': 'query-execution-not-semantic-proof'}} if spec['explain'] else {}),
        "nodes": [_lens_carrier(item, spec['detail']) for item in final_nodes],
        "relations": [_lens_carrier(item, spec['detail']) for item in final_relations],
        "groups": groups,
        "facets": {
            "node_kinds": dict(sorted(Counter(str(item["kind_id"]) for item in final_nodes).items())),
            "predicates": dict(sorted(Counter(str(item["predicate_id"]) for item in final_relations).items())),
            "sources": dict(sorted(Counter(str(item["source_graph"]) for item in final_nodes).items())),
        },
        "counts": {
            "available_nodes": len(nodes),
            "available_relations": len(relations),
            "matched_nodes": len(matched_node_ids),
            "matched_relations": len(relation_candidates),
            "eligible_relations": eligible_relation_count,
            "nodes": len(final_nodes),
            "relations": len(final_relations),
            "groups": len(groups),
            "truncated_nodes": truncated_nodes,
            "truncated_relations": truncated_relations,
            "missing_node_summaries": missing_node_summaries,
            "missing_relation_explanations": missing_relation_explanations,
            "nodes_without_source_summary": nodes_without_source_summary,
            "relations_without_source_explanation": relations_without_source_explanation,
        },
        "source_refs": refs,
        "warnings": [
            *( [f"{missing_node_summaries} nodes expose an explicit missing-summary state"] if missing_node_summaries else [] ),
            *( [f"{missing_relation_explanations} relations expose an explicit missing-explanation state"] if missing_relation_explanations else [] ),
            *( [f"{nodes_without_source_summary} nodes use transparent metadata synthesis because no source summary is projected"] if nodes_without_source_summary else [] ),
            *( [f"{relations_without_source_explanation} relations use transparent metadata synthesis because no source explanation is projected"] if relations_without_source_explanation else [] ),
            *( [f"node selector exceeded its bounded result by {truncated_nodes} nodes"] if truncated_nodes else [] ),
            *( [f"relation selector exceeded its bounded result by {truncated_relations} relations"] if truncated_relations else [] ),
        ],
        "authority_boundary": dict(graph.get("authority_boundary") or {}),
        "agent_summary": {
            "lens_id": spec["lens_id"],
            "focus_node_id": focus["node_id"] if focus is not None else None,
            "node_count": len(final_nodes),
            "relation_count": len(final_relations),
            "group_count": len(groups),
            "source_ref_count": len(refs),
            "is_source": False,
            "writes_to_tree": False,
        },
    })


def _lens_carrier(item: dict[str, Any], detail: str) -> dict[str, Any]:
    if detail == 'full':
        return item
    return {**{key: value for key, value in item.items() if key != 'source_record'}, 'attributes': {}}


def focus_knowledge_node(
    graph: dict[str, Any],
    node_id: str,
    *,
    sources: list[str] | None = None,
    depth: int = 1,
    direction: str = "either",
    predicate_ids: list[str] | None = None,
    node_limit: int = 200,
    relation_limit: int = 400,
    profile: str = "overview",
) -> dict[str, Any]:
    """Build one radial LensSpec around an exact or uniquely namespaced node identity."""
    identifier = _string(node_id)
    if identifier is None:
        raise ValueError("knowledge focus node id is required")
    source_set = _normalized_source_filter(sources)
    selected_sources = [source for source in KNOWLEDGE_SOURCES if source in source_set]
    return execute_knowledge_lens(
        graph,
        {
            "schema_version": "tos_lens_spec_v1",
            "lens_id": "focus-neighborhood",
            "title": {"default": f"Focus: {identifier}"},
            "description": {
                "default": f"Bounded knowledge neighborhood centered on {identifier}."
            },
            "sources": selected_sources,
            "seed": {"focus_node_id": identifier},
            "node_query": {"enabled": False},
            "relation_query": {"enabled": True},
            "traversal": {
                "depth": depth,
                "direction": direction,
                "predicate_ids": predicate_ids or [],
                "profile": profile,
            },
            "composition": {
                "endpoint_policy": "both",
                "group_by": [],
                "sort_nodes": [{"field": "id", "direction": "asc"}],
                "sort_relations": [{"field": "id", "direction": "asc"}],
            },
            "presentation": {
                "layout": "radial",
                "color_by": "kind_id",
                "lane_by": "epistemic.authority_layer",
                "size_by": None,
                "inspector_fields": ["display", "epistemic", "source_refs", "attributes"],
            },
            "limits": {"nodes": node_limit, "relations": relation_limit, "groups": 100},
        },
    )


def _layout_from_hint(value: Any) -> str:
    hint = str(value or "").casefold()
    for needle, layout in (
        ("timeline", "timeline"),
        ("flow", "flow"),
        ("corridor", "flow"),
        ("dag", "evidence"),
        ("evidence", "evidence"),
        ("semantic", "semantic"),
        ("infrastructure", "infrastructure"),
        ("layered", "hierarchical"),
        ("radial", "radial"),
        ("matrix", "matrix"),
    ):
        if needle in hint:
            return layout
    return "organic"


def saved_lens_specs(corpus: dict[str, Any], philosophy: dict[str, Any]) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    for view in _objects(philosophy.get("views")):
        view_id = _string(view.get("view_id"))
        if not view_id:
            continue
        result.append(
            normalize_lens_spec(
                {
                    "schema_version": "tos_lens_spec_v1",
                    "lens_id": view_id,
                    "title": {"default": _string(view.get("title")) or _humanize(view_id)},
                    "description": {"default": _string(view.get("review_intent")) or f"Source-owned ToS lens {view_id}."},
                    "sources": ["philosophy"],
                    "node_query": {"match": "all", "filters": [{"field": "view_ids", "op": "contains", "value": view_id}]},
                    "relation_query": {"match": "all", "filters": [{"field": "view_ids", "op": "contains", "value": view_id}]},
                    "composition": {"endpoint_policy": "both", "group_by": [], "sort_nodes": [{"field": "id", "direction": "asc"}], "sort_relations": [{"field": "id", "direction": "asc"}]},
                    "presentation": {"layout": _layout_from_hint(view.get("layout_hint")), "color_by": "kind_id", "lane_by": "epistemic.canon_status", "size_by": None, "inspector_fields": ["display", "epistemic", "source_refs"]},
                    "limits": {"nodes": 1000, "relations": 2000, "groups": 100},
                }
            )
        )
    corpus_sources = {
        "corpus-topology": ["repository"],
        "route-graph": ["canon"],
        "promotion-flow": ["canon"],
    }
    for view in _objects(corpus.get("graph_views")):
        view_id = _string(view.get("view_id"))
        if not view_id or view_id not in corpus_sources:
            continue
        if any(item["lens_id"] == view_id for item in result):
            continue
        result.append(
            normalize_lens_spec(
                {
                    "schema_version": "tos_lens_spec_v1",
                    "lens_id": view_id,
                    "title": {"default": _string(view.get("title")) or _humanize(view_id)},
                    "description": {"default": _string(view.get("purpose")) or f"ToS corpus lens {view_id}."},
                    "sources": corpus_sources[view_id],
                    "node_query": {"match": "all", "filters": [{"field": "view_ids", "op": "contains", "value": view_id}]},
                    "relation_query": {"match": "all", "filters": [{"field": "view_ids", "op": "contains", "value": view_id}]},
                    "composition": {"endpoint_policy": "both", "group_by": [], "sort_nodes": [{"field": "id", "direction": "asc"}], "sort_relations": [{"field": "id", "direction": "asc"}]},
                    "presentation": {"layout": _layout_from_hint(view.get("layout_hint")), "color_by": "kind_id", "lane_by": "epistemic.authority_layer", "size_by": None, "inspector_fields": ["display", "epistemic", "source_refs"]},
                    "limits": {"nodes": 1000, "relations": 2000, "groups": 100},
                }
            )
        )
    return sorted(result, key=lambda item: str(item["lens_id"]))


def _json_value_kind(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "array"
    return "object"


def _attribute_values(attributes: dict[str, Any], prefix: str = "attributes") -> Iterable[tuple[str, Any]]:
    for key in sorted(attributes):
        field = f"{prefix}.{key}"
        if not _allowed_field(field, "node"):
            continue
        value = attributes[key]
        yield field, value
        if isinstance(value, dict):
            yield from _attribute_values(value, field)


def _attribute_catalog(items: list[dict[str, Any]], kind: str) -> list[dict[str, Any]]:
    stats: dict[str, dict[str, Any]] = {}
    for item in items:
        attributes = item.get("attributes")
        if not isinstance(attributes, dict):
            continue
        for field, value in _attribute_values(attributes):
            if not _allowed_field(field, kind):
                continue
            entry = stats.get(field)
            if entry is None:
                entry = stats[field] = {
                    "field": field,
                    "item_count": 0,
                    "value_types": Counter(),
                    "array_item_types": Counter(),
                    "sources": set(),
                    "examples": [],
                    "_example_keys": set(),
                }
            entry["item_count"] += 1
            entry["value_types"][_json_value_kind(value)] += 1
            entry["sources"].add(str(item.get("source_graph") or ""))
            candidates = value if isinstance(value, list) else [value]
            if isinstance(value, list):
                entry["array_item_types"].update(_json_value_kind(candidate) for candidate in candidates)
            if len(entry["examples"]) >= 5:
                continue
            for candidate in candidates:
                if len(entry["examples"]) >= 5:
                    break
                if isinstance(candidate, (dict, list)) or candidate is None:
                    continue
                encoded = json.dumps(candidate, ensure_ascii=False, sort_keys=True)
                if len(encoded) > 180 or encoded in entry["_example_keys"]:
                    continue
                entry["_example_keys"].add(encoded)
                entry["examples"].append(candidate)
    result: list[dict[str, Any]] = []
    for field in sorted(stats):
        entry = stats[field]
        result.append(
            {
                "field": field,
                "item_count": entry["item_count"],
                "value_types": dict(sorted(entry["value_types"].items())),
                "array_item_types": dict(sorted(entry["array_item_types"].items())),
                "sources": sorted(source for source in entry["sources"] if source),
                "examples": entry["examples"],
            }
        )
    return result


def _facet_catalog(items: list[dict[str, Any]], fields: Iterable[str]) -> dict[str, list[dict[str, Any]]]:
    result: dict[str, list[dict[str, Any]]] = {}
    for field in fields:
        counts: Counter[str] = Counter()
        for item in items:
            raw = _field(item, field)
            values = raw if isinstance(raw, list) else [raw]
            counts.update(str(value) for value in values if value is not None and str(value))
        result[field] = [
            {"value": value, "count": count}
            for value, count in sorted(counts.items(), key=lambda pair: pair[0].casefold())
        ]
    return result


def knowledge_catalog(
    graph: dict[str, Any],
    corpus: dict[str, Any],
    philosophy: dict[str, Any],
    entity_type_registry: Any = None,
    relation_type_registry: Any = None,
) -> dict[str, Any]:
    nodes = _objects(graph.get("nodes"))
    relations = _objects(graph.get("relations"))
    kind_counts = Counter(str(item["kind_id"]) for item in nodes)
    predicate_counts = Counter(str(item["predicate_id"]) for item in relations)
    type_counts = Counter(str(item["type_id"]) for item in nodes)
    relation_type_counts = Counter(str(item["relation_type_id"]) for item in relations)
    entity_entries, _entity_mappings, fallback_type_id = _entity_registry_indexes(
        entity_type_registry
    )
    relation_entries, _relation_mappings, fallback_relation_type_id = (
        _relation_registry_indexes(relation_type_registry)
    )
    kinds = []
    for kind_id in sorted(kind_counts):
        instances = [item for item in nodes if item["kind_id"] == kind_id]
        example = instances[0]
        kinds.append(
            {
                "kind_id": kind_id,
                "display": example["display"]["kind_label"],
                "count": kind_counts[kind_id],
                "type_ids": sorted({str(item["type_id"]) for item in instances}),
                "mapping_statuses": sorted(
                    {
                        str((item.get("type_mapping") or {}).get("status"))
                        for item in instances
                    }
                ),
            }
        )
    predicates = []
    for predicate_id in sorted(predicate_counts):
        instances = [item for item in relations if item["predicate_id"] == predicate_id]
        example = instances[0]
        relation_type_ids = sorted(
            {str(item["relation_type_id"]) for item in instances}
        )
        predicates.append(
            {
                "predicate_id": predicate_id,
                "display": example["display"]["label"],
                "count": predicate_counts[predicate_id],
                "relation_type_ids": relation_type_ids,
                "mapping_statuses": sorted(
                    {
                        str((item.get("predicate_mapping") or {}).get("status"))
                        for item in instances
                    }
                ),
                "semantic_definitions": [
                    {
                        "relation_type_id": relation_type_id,
                        "labels": relation_entries[relation_type_id].get("labels"),
                        "definition": relation_entries[relation_type_id].get("definition"),
                        "domain_type_ids": relation_entries[relation_type_id].get(
                            "domain_type_ids"
                        ),
                        "range_type_ids": relation_entries[relation_type_id].get(
                            "range_type_ids"
                        ),
                    }
                    for relation_type_id in relation_type_ids
                    if relation_type_id in relation_entries
                ],
            }
        )
    claim_relation_type_counts: Counter[str] = Counter()
    for node in nodes:
        semantics = node.get("semantics") if isinstance(node.get("semantics"), dict) else {}
        claim = semantics.get("claim") if isinstance(semantics.get("claim"), dict) else {}
        relation_type_id = _string(claim.get("relation_type_id"))
        if relation_type_id:
            claim_relation_type_counts[relation_type_id] += 1

    entity_registry_entries = []
    for type_id, entry in sorted(entity_entries.items()):
        entity_registry_entries.append(
            {
                **entry,
                "instance_count": type_counts[type_id],
            }
        )
    relation_registry_entries = []
    for relation_type_id, entry in sorted(relation_entries.items()):
        relation_registry_entries.append(
            {
                **entry,
                "edge_instance_count": relation_type_counts[relation_type_id],
                "claim_instance_count": claim_relation_type_counts[relation_type_id],
                "instance_count": (
                    relation_type_counts[relation_type_id]
                    + claim_relation_type_counts[relation_type_id]
                ),
            }
        )

    entity_route_defs = (
        (
            "concept",
            ("concept", "principle"),
            (),
            ("tos.entity.concept", "tos.entity.principle"),
            (),
        ),
        (
            "author",
            ("agent",),
            ("authored_by",),
            ("tos.entity.agent",),
            ("tos.relation.authored-by",),
        ),
        (
            "work",
            ("work",),
            ("authored_by", "has_expression"),
            ("tos.entity.work",),
            ("tos.relation.authored-by", "tos.relation.has-expression"),
        ),
        (
            "word",
            ("lexeme", "word", "token", "word-occurrence"),
            ("occurs_in", "expresses_concept"),
            (),
            (),
        ),
        (
            "tradition",
            ("tradition", "school_tradition"),
            (),
            ("tos.entity.tradition", "tos.entity.school-tradition"),
            (),
        ),
        (
            "place",
            ("place", "region"),
            (),
            ("tos.entity.place",),
            ("tos.relation.has-normalized-place",),
        ),
        (
            "source-object",
            ("work", "expression", "edition", "item", "file", "source_witness"),
            (),
            (
                "tos.entity.intellectual-object",
                "tos.entity.source-witness",
            ),
            (),
        ),
    )
    nodes_by_id = {str(item["id"]): item for item in nodes}
    entity_routes = []
    for (
        route_id,
        candidate_kinds,
        confirming_predicates,
        candidate_types,
        confirming_relation_types,
    ) in entity_route_defs:
        available_kinds = [kind for kind in candidate_kinds if kind in kind_counts]
        candidate_kind_set = set(candidate_kinds)
        confirming_relations = [
            relation
            for relation in relations
            if relation.get("predicate_id") in confirming_predicates
            and any(
                (nodes_by_id.get(str(endpoint)) or {}).get("kind_id") in candidate_kind_set
                for endpoint in (relation.get("from_id"), relation.get("to_id"))
            )
        ]
        available_predicates = sorted({str(relation["predicate_id"]) for relation in confirming_relations})
        typed_nodes = [
            node
            for node in nodes
            if candidate_types
            and _type_is_a(str(node.get("type_id") or ""), candidate_types, entity_entries)
        ]
        typed_node_ids = {str(node["id"]) for node in typed_nodes}
        typed_relations = [
            relation
            for relation in relations
            if relation.get("relation_type_id") in confirming_relation_types
            and any(
                str(endpoint) in typed_node_ids
                for endpoint in (relation.get("from_id"), relation.get("to_id"))
            )
        ]
        available_types = sorted({str(node["type_id"]) for node in typed_nodes})
        available_relation_types = sorted(
            {str(relation["relation_type_id"]) for relation in typed_relations}
        )
        semantic_mode = bool(entity_entries and candidate_types)
        availability = (
            "available"
            if (typed_nodes if semantic_mode else available_kinds)
            else "not_projected"
        )
        has_confirming_relation = bool(
            typed_relations if semantic_mode else confirming_relations
        )
        expects_confirmation = bool(
            confirming_relation_types if semantic_mode else confirming_predicates
        )
        role_readiness = (
            "not_projected"
            if availability == "not_projected"
            else "kind_only"
            if expects_confirmation and not has_confirming_relation
            else "confirmed"
        )
        entity_routes.append(
            {
                "route_id": route_id,
                "candidate_kind_ids": list(candidate_kinds),
                "available_kind_ids": available_kinds,
                "confirming_predicate_ids": list(confirming_predicates),
                "available_confirming_predicate_ids": available_predicates,
                "confirming_relation_count": len(confirming_relations),
                "candidate_type_ids": list(candidate_types),
                "available_type_ids": available_types,
                "confirming_relation_type_ids": list(confirming_relation_types),
                "available_confirming_relation_type_ids": available_relation_types,
                "semantic_confirming_relation_count": len(typed_relations),
                "node_count": len(typed_nodes) if semantic_mode else sum(
                    kind_counts[kind] for kind in available_kinds
                ),
                "availability": availability,
                "role_readiness": role_readiness,
                "note": (
                    "No trustworthy nodes of these kinds are projected; the backend will not synthesize them from unrelated predicates."
                    if availability == "not_projected"
                    else "Kinds are projected, but no confirming relation currently establishes the requested contextual role."
                    if role_readiness == "kind_only"
                    else "Kinds select candidate entities; predicates establish contextual roles such as authorship."
                    if confirming_predicates
                    else "Kinds are source-derived candidates and retain their exact kind_id on every node."
                ),
            }
        )
    return {
        "schema": "tos_knowledge_catalog_v1",
        "source_revision": graph.get("source_revision"),
        "contract_refs": {
            "public_bundle": "/api/knowledge/contracts",
            "knowledge_api": "access/contracts/knowledge-api.v1.json",
            "lens_spec": "access/contracts/lens-spec.v1.schema.json",
            "lens_result": "access/contracts/lens-result.v1.schema.json",
            "knowledge_graph": "access/contracts/knowledge-graph.v1.schema.json",
            "entity_type_registry_schema": "ToS/contracts/semantic-entity-type-registry.schema.json",
            "relation_type_registry_schema": "ToS/contracts/semantic-relation-type-registry.schema.json",
            "entity_type_registry": ENTITY_REGISTRY_REF,
            "relation_type_registry": RELATION_REGISTRY_REF,
        },
        "counts": graph.get("counts", {}),
        "node_kinds": kinds,
        "predicates": predicates,
        "semantic_registries": {
            "properties": copy.deepcopy((entity_type_registry or {}).get("property_definitions", [])),
            "entity_types": {
                "registry_id": entity_type_registry.get("registry_id")
                if isinstance(entity_type_registry, dict)
                else None,
                "registry_version": entity_type_registry.get("registry_version")
                if isinstance(entity_type_registry, dict)
                else None,
                "source_refs": _strings(entity_type_registry.get("source_refs"))
                if isinstance(entity_type_registry, dict)
                else [],
                "fallback_type_id": fallback_type_id,
                "mapped_instance_count": len(nodes) - type_counts[fallback_type_id],
                "unmapped_instance_count": type_counts[fallback_type_id],
                "entries": entity_registry_entries,
            },
            "relation_types": {
                "registry_id": relation_type_registry.get("registry_id")
                if isinstance(relation_type_registry, dict)
                else None,
                "registry_version": relation_type_registry.get("registry_version")
                if isinstance(relation_type_registry, dict)
                else None,
                "source_refs": _strings(relation_type_registry.get("source_refs"))
                if isinstance(relation_type_registry, dict)
                else [],
                "fallback_relation_type_id": fallback_relation_type_id,
                "mapped_edge_instance_count": len(relations)
                - relation_type_counts[fallback_relation_type_id],
                "unmapped_edge_instance_count": relation_type_counts[
                    fallback_relation_type_id
                ],
                "entries": relation_registry_entries,
            },
        },
        "lenses": saved_lens_specs(corpus, philosophy),
        "capabilities": {
            "execution_version": "tos-lens-execution-v2",
            "path_query": {"conditions": 4, "steps_per_condition": 4,
                           "quantifiers": ["exists", "not_exists"], "combination": "all",
                           "scope": "node-selector-roots-and-selected-sources", "walks_may_revisit_nodes": True},
            "inclusion": {"request_field": "explain", "authority": "query-execution-not-semantic-proof"},
            "pagination": {"request_field": "pagination", "scope": "bounded-lens-result",
                           "snapshot_bound": True, "historical_snapshot_retention": False,
                           "reexecutes_bounded_lens": True, "maximum_primary_nodes": 100,
                           "maximum_relations": 100, "context_endpoints_may_repeat": True,
                           "changed_query_or_snapshot_http_status": 409},
            "neighborhood_profiles": [
                {"profile": "overview", "definition": "Bibliographic and conceptual overview; dense text-unit and anchor membership is expanded separately.", "excluded_predicates": sorted(OVERVIEW_EXCLUDED_PREDICATES)},
                {"profile": "all", "definition": "All declared relation kinds, including detailed text structure; result limits still apply.", "excluded_predicates": []},
            ],
            "sources": list(KNOWLEDGE_SOURCES),
            "filter_operators": list(FILTER_OPERATORS),
            "operator_value_contracts": {
                "eq": "scalar",
                "neq": "scalar",
                "in": "scalar-or-scalar-array",
                "contains": "scalar-or-scalar-array",
                "prefix": "string",
                "exists": "boolean",
                "gt": "number",
                "gte": "number",
                "lt": "number",
                "lte": "number",
            },
            "node_fields": sorted(NODE_FIELDS),
            "relation_fields": sorted(RELATION_FIELDS),
            "attribute_field_pattern": _ATTRIBUTE_FIELD.pattern,
            "node_attribute_fields": _attribute_catalog(nodes, "node"),
            "relation_attribute_fields": _attribute_catalog(relations, "relation"),
            "facets": {
                "nodes": _facet_catalog(
                    nodes,
                    (
                        "source_graph",
                        "kind_id",
                        "type_id",
                        "type_mapping.status",
                        "epistemic.authority_layer",
                        "epistemic.canon_status",
                        "epistemic.review_posture",
                        "graph_layers",
                        "view_ids",
                    ),
                ),
                "relations": _facet_catalog(
                    relations,
                    (
                        "source_graph",
                        "predicate_id",
                        "relation_type_id",
                        "predicate_mapping.status",
                        "epistemic.authority_layer",
                        "epistemic.canon_status",
                        "epistemic.review_posture",
                        "graph_layers",
                        "view_ids",
                    ),
                ),
            },
            "layouts": list(LAYOUTS),
            "endpoint_policies": ["both", "either", "independent"],
            "focus": {
                "seed_field": "seed.focus_node_id",
                "resolution_order": ["id", "entity_id", "unique_native_id"],
                "shared_entity_id_resolution": "source-priority-then-node-id",
                "ambiguous_native_id": "rejected",
                "default_depth": 1,
                "default_direction": "either",
                "default_layout": "radial",
            },
            "entity_routes": entity_routes,
            "maximums": {
                "filters_per_item_kind": MAX_FILTERS,
                "traversal_depth": MAX_TRAVERSAL_DEPTH,
                "nodes": MAX_NODE_LIMIT,
                "relations": MAX_RELATION_LIMIT,
                "groups": MAX_GROUP_LIMIT,
            },
        },
        "authority_boundary": graph.get("authority_boundary", {}),
    }


def _normalized_source_filter(sources: Any) -> set[str]:
    if sources is None:
        return set(KNOWLEDGE_SOURCES)
    if not isinstance(sources, list):
        raise ValueError("sources must be an array")
    normalized = {str(item) for item in sources if isinstance(item, str) and item}
    unknown = sorted(normalized - set(KNOWLEDGE_SOURCES))
    if unknown:
        raise ValueError(f"unsupported knowledge sources: {', '.join(unknown)}")
    return normalized or set(KNOWLEDGE_SOURCES)


def _knowledge_search_rank(item: dict[str, Any], needle: str, *, relation: bool) -> tuple[int, str]:
    native_id = str(item.get("native_id") or "").lower()
    item_id = str(item.get("id") or "").lower()
    display = item.get("display") if isinstance(item.get("display"), dict) else {}
    primary = display.get("label") if relation else display.get("title")
    title = str(primary.get("default") or "").lower() if isinstance(primary, dict) else ""
    if not needle:
        return (3, item_id)
    if needle in {item_id, native_id, title}:
        return (0, item_id)
    if item_id.startswith(needle) or native_id.startswith(needle) or title.startswith(needle):
        return (1, item_id)
    return (2, item_id)


class KnowledgeSearchIndex:
    """Serialized search documents for one immutable, caller-owned snapshot.

    Preserve the existing JSON substring semantics, including metadata and
    escaping. This index saves serialization, not the substring scan itself.
    """
    def __init__(self, graph):
        self.graph = graph
        self.nodes = tuple((item, _searchable(item)) for item in _objects(graph.get('nodes')))
        self.relations = tuple((item, _searchable(item)) for item in _objects(graph.get('relations')))


def search_knowledge_graph(
    graph: dict[str, Any],
    query: str = "",
    *,
    sources: list[str] | None = None,
    kind_ids: list[str] | None = None,
    predicate_ids: list[str] | None = None,
    offset: int = 0,
    limit: int = 40,
    search_index: KnowledgeSearchIndex | None = None,
) -> dict[str, Any]:
    normalized_query = str(query).strip()
    if len(normalized_query) > 256:
        raise ValueError("knowledge search query exceeds 256 characters")
    bounded_offset = _bounded_integer(offset, "offset", 0, 0, 100_000)
    bounded_limit = _bounded_integer(limit, "limit", 40, 1, 100)
    source_filter = _normalized_source_filter(sources)
    kind_filter = set(_strings(kind_ids))
    predicate_filter = set(_strings(predicate_ids))
    if len(kind_filter) > 100 or len(predicate_filter) > 100:
        raise ValueError("knowledge search kind and predicate filters must contain at most 100 values")
    needle = normalized_query.lower()
    if search_index is not None and search_index.graph is not graph:
        raise ValueError('search index belongs to a different snapshot')
    node_documents = (search_index.nodes if search_index is not None else
                      ((item, None) for item in _objects(graph.get('nodes'))))
    relation_documents = (search_index.relations if search_index is not None else
                          ((item, None) for item in _objects(graph.get('relations'))))

    nodes = [
        item
        for item, document in node_documents
        if item.get("source_graph") in source_filter
        and (not kind_filter or item.get("kind_id") in kind_filter)
        and (not needle or needle in (document if document is not None else _searchable(item)))
    ]
    relations = [
        item
        for item, document in relation_documents
        if item.get("source_graph") in source_filter
        and (not predicate_filter or item.get("predicate_id") in predicate_filter)
        and (not needle or needle in (document if document is not None else _searchable(item)))
    ]
    nodes.sort(key=lambda item: _knowledge_search_rank(item, needle, relation=False))
    relations.sort(key=lambda item: _knowledge_search_rank(item, needle, relation=True))
    selected_nodes = nodes[bounded_offset : bounded_offset + bounded_limit]
    selected_relations = relations[bounded_offset : bounded_offset + bounded_limit]
    return {
        "schema": "tos_knowledge_search_v1",
        "source_revision": graph["source_revision"],
        "query": normalized_query,
        "filters": {
            "sources": sorted(source_filter),
            "kind_ids": sorted(kind_filter),
            "predicate_ids": sorted(predicate_filter),
        },
        "page": {"offset": bounded_offset, "limit_per_kind": bounded_limit},
        "counts": {
            "matching_nodes": len(nodes),
            "matching_relations": len(relations),
            "returned_nodes": len(selected_nodes),
            "returned_relations": len(selected_relations),
        },
        "nodes": selected_nodes,
        "relations": selected_relations,
        "authority_boundary": graph.get("authority_boundary", {}),
    }


def inspect_knowledge_node(graph: dict[str, Any], identifier: str, relation_limit: int = 200) -> dict[str, Any]:
    item_id = str(identifier).strip()
    if not item_id:
        raise ValueError("knowledge node id is required")
    nodes = _objects(graph.get("nodes"))
    exact = [item for item in nodes if item.get("id") == item_id]
    entity = [] if exact else [item for item in nodes if item.get("entity_id") == item_id]
    matches = exact or entity or [item for item in nodes if item.get("native_id") == item_id]
    if not matches:
        raise KeyError(f"unknown ToS knowledge node: {item_id}")
    match_ids = {str(item["id"]) for item in matches}
    bounded_limit = _bounded_integer(relation_limit, "relation_limit", 200, 0, 1000)
    all_relations = [
        item
        for item in _objects(graph.get("relations"))
        if item.get("from_id") in match_ids or item.get("to_id") in match_ids
    ]
    all_relations.sort(key=lambda item: str(item.get("id") or ""))
    selected_relations = all_relations[:bounded_limit]
    return {
        "schema": "tos_knowledge_node_packet_v1",
        "source_revision": graph["source_revision"],
        "requested_id": item_id,
        "ambiguous_native_id": not exact and not entity and len(matches) > 1,
        "shared_entity_id": len(entity) > 1,
        "matches": matches,
        "related_relations": selected_relations,
        "counts": {
            "matches": len(matches),
            "related_relations": len(all_relations),
            "returned_relations": len(selected_relations),
        },
        "source_refs": sorted({ref for item in [*matches, *selected_relations] for ref in _strings(item.get("source_refs"))}),
        "authority_boundary": graph.get("authority_boundary", {}),
    }


def inspect_knowledge_relation(graph: dict[str, Any], identifier: str) -> dict[str, Any]:
    item_id = str(identifier).strip()
    if not item_id:
        raise ValueError("knowledge relation id is required")
    relations = _objects(graph.get("relations"))
    exact = [item for item in relations if item.get("id") == item_id]
    matches = exact or [item for item in relations if item.get("native_id") == item_id]
    if not matches:
        raise KeyError(f"unknown ToS knowledge relation: {item_id}")
    endpoint_ids = {
        str(endpoint)
        for item in matches
        for endpoint in (item.get("from_id"), item.get("to_id"))
        if isinstance(endpoint, str) and endpoint
    }
    endpoints = [item for item in _objects(graph.get("nodes")) if item.get("id") in endpoint_ids]
    endpoints.sort(key=lambda item: str(item.get("id") or ""))
    return {
        "schema": "tos_knowledge_relation_packet_v1",
        "source_revision": graph["source_revision"],
        "requested_id": item_id,
        "ambiguous_native_id": not exact and len(matches) > 1,
        "matches": matches,
        "endpoints": endpoints,
        "counts": {"matches": len(matches), "endpoints": len(endpoints)},
        "source_refs": sorted({ref for item in [*matches, *endpoints] for ref in _strings(item.get("source_refs"))}),
        "authority_boundary": graph.get("authority_boundary", {}),
    }
