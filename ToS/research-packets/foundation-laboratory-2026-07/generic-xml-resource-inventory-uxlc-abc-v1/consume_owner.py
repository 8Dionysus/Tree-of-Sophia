#!/usr/bin/env python3
"""Independent consumer for candidate source return.

This program deliberately does not import the candidate builder and does not
read the sealed evaluation manifest.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import resource
import time
from collections import Counter
from pathlib import Path
from typing import Any

from lxml import etree


LAB_DIR = Path(__file__).resolve().parent
MANIFEST_PATH = LAB_DIR / "input-manifest.json"
PUBLIC_ROOT = LAB_DIR / "public-synthetic" / "run-1"
RECEIPT_PATH = LAB_DIR / "independent-consumer-receipt.json"
DOCTYPE_PATTERN = re.compile(br"<!DOCTYPE\s", re.IGNORECASE)

FILE_BINDING_KEYS = frozenset(
    {"input_id", "file_id", "sha256", "byte_size", "media_type"}
)
PARSER_POSTURE_KEYS = frozenset(
    {
        "parser",
        "mode",
        "recover",
        "load_dtd",
        "dtd_validation",
        "resolve_entities",
        "no_network",
        "huge_tree",
        "xinclude",
        "reject_doctype",
    }
)
A_RESOURCE_KEYS = frozenset({"resource_id", "resource_kind", "locator"})
B_RESOURCE_KEYS = frozenset(
    {
        "resource_id",
        "resource_kind",
        "expanded_name",
        "locator",
        "element_child_count",
        "attribute_count",
        "attribute_expanded_names",
    }
)
B_SUMMARY_KEYS = frozenset(
    {
        "resource_count",
        "attribute_count",
        "max_depth",
        "namespace_uris",
        "ordered_topology_sha256",
        "unordered_element_shape_sha256",
    }
)
B_SCOPE_KEYS = frozenset({"node_kinds", "excluded", "path_identity"})
PROVIDER_CONTEXT_KEYS = frozenset(
    {"provider", "expression", "edition", "book_code", "selector", "projection_authority"}
)
PROJECTION_RECORD_KEYS = frozenset(
    {"resource_id", "resource_kind", "provider_coordinate", "source_element_path", "generic_resource_ref"}
)
C_SUMMARY_KEYS = frozenset(
    {"resource_count", "verse_count", "word_count", "word_counts_by_verse"}
)
A_TOP_LEVEL_KEYS = frozenset(
    {
        "schema_version",
        "lab_id",
        "candidate",
        "file_binding",
        "parser_posture",
        "scope",
        "resources",
        "source_text_included",
        "element_return_supported",
        "intrinsic_ids_claimed",
        "authority_boundary",
    }
)
B_TOP_LEVEL_KEYS = frozenset(
    {
        "schema_version",
        "lab_id",
        "candidate",
        "file_binding",
        "parser_posture",
        "scope",
        "summary",
        "resources",
        "source_text_included",
        "element_content_fingerprints_included",
        "intrinsic_ids_claimed",
        "cross_file_identity_claimed",
        "tei_classification_claimed",
        "authority_boundary",
    }
)
C_TOP_LEVEL_KEYS = frozenset(
    {
        "schema_version",
        "lab_id",
        "candidate",
        "file_binding",
        "parser_posture",
        "provider_context",
        "summary",
        "resources",
        "source_text_included",
        "generic_xml_owner_claimed",
        "intrinsic_or_cross_corpus_word_ids_claimed",
        "accepted_structure_claimed",
        "authority_boundary",
    }
)
BC_TOP_LEVEL_KEYS = frozenset(
    {"schema_version", "lab_id", "candidate", "owner", "projection", "source_text_included", "authority_boundary"}
)
B_CLAIM_POSTURE_KEYS = frozenset(
    {
        "source_text_included",
        "element_content_fingerprints_included",
        "intrinsic_ids_claimed",
        "cross_file_identity_claimed",
        "tei_classification_claimed",
    }
)
EXPECTED_SCHEMA_VERSIONS = {
    "A": "tos_lab_generic_xml_candidate_a_v1",
    "B": "tos_lab_generic_xml_candidate_b_v1",
    "C": "tos_lab_generic_xml_candidate_c_v1",
    "BC": "tos_lab_generic_xml_candidate_bc_v1",
}
EXPECTED_B_SCOPE = {
    "node_kinds": ["element"],
    "excluded": [
        "text",
        "tail",
        "attribute_values",
        "comments",
        "processing_instructions",
        "dtd",
        "entity_expansions",
    ],
    "path_identity": "expanded-name plus one-based same-name sibling position under exact file binding",
}


def require_exact_keys(value: Any, expected: set[str] | frozenset[str], label: str) -> None:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    actual = set(value)
    if actual != set(expected):
        missing = sorted(set(expected) - actual)
        unexpected = sorted(actual - set(expected))
        raise ValueError(
            f"{label} shape mismatch: missing={missing} unexpected={unexpected}"
        )


def canonical_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def expanded_name(raw_name: str) -> dict[str, str | None]:
    if raw_name.startswith("{"):
        namespace_uri, local_name = raw_name[1:].split("}", 1)
        return {"namespace_uri": namespace_uri, "local_name": local_name}
    return {"namespace_uri": None, "local_name": raw_name}


def strict_parse(payload: bytes) -> etree._Element:
    if DOCTYPE_PATTERN.search(payload):
        raise ValueError("DOCTYPE forbidden")
    parser = etree.XMLParser(
        recover=False,
        load_dtd=False,
        dtd_validation=False,
        resolve_entities=False,
        no_network=True,
        huge_tree=False,
        remove_comments=False,
        remove_pis=False,
        strip_cdata=False,
        collect_ids=False,
    )
    root = etree.fromstring(payload, parser=parser)
    docinfo = root.getroottree().docinfo
    if docinfo.doctype or docinfo.internalDTD is not None or docinfo.externalDTD is not None:
        raise ValueError("DOCTYPE forbidden")
    return root


def element_children(element: etree._Element) -> list[etree._Element]:
    return [child for child in element if isinstance(child.tag, str)]


def direct_children_named(element: etree._Element, local_name: str) -> list[etree._Element]:
    return [
        child
        for child in element_children(element)
        if expanded_name(child.tag) == {"namespace_uri": None, "local_name": local_name}
    ]


def exactly_one_direct_child(element: etree._Element, local_name: str) -> etree._Element:
    children = direct_children_named(element, local_name)
    if len(children) != 1:
        raise ValueError(f"provider shape requires one {local_name} subtree")
    return children[0]


def path_for(element: etree._Element) -> list[dict[str, Any]]:
    chain = list(reversed(list(element.iterancestors()))) + [element]
    path: list[dict[str, Any]] = []
    for current in chain:
        parent = current.getparent()
        if parent is None:
            same_name_sibling_position = 1
        else:
            matching = [
                child
                for child in element_children(parent)
                if expanded_name(child.tag) == expanded_name(current.tag)
            ]
            if current not in matching:
                raise ValueError("provider path element is not a child of its parent")
            same_name_sibling_position = matching.index(current) + 1
        path.append(
            {
                "expanded_name": expanded_name(current.tag),
                "same_name_sibling_position": same_name_sibling_position,
            }
        )
    return path


def topology_digest_bytes(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
            separators=(",", ": "),
        )
        + "\n"
    ).encode("utf-8")


def topology_digest_object(value: Any) -> str:
    return sha256_bytes(topology_digest_bytes(value))


def independent_b_resources(root: etree._Element) -> list[dict[str, Any]]:
    """Rebuild B's structural metadata from the source, not from its payload."""
    elements = [element for element in root.iter() if isinstance(element.tag, str)]
    resource_ids = {
        element: f"xml-element-{position:06d}"
        for position, element in enumerate(elements, start=1)
    }
    resources: list[dict[str, Any]] = []
    for preorder, element in enumerate(elements, start=1):
        parent = element.getparent()
        if parent is None:
            child_position = 1
            same_name_position = 1
            parent_resource_id = None
        else:
            children = element_children(parent)
            child_position = children.index(element) + 1
            same_name_siblings = [
                sibling
                for sibling in children
                if expanded_name(sibling.tag) == expanded_name(element.tag)
            ]
            same_name_position = same_name_siblings.index(element) + 1
            parent_resource_id = resource_ids[parent]
        attribute_names = sorted(
            (expanded_name(name) for name in element.attrib.keys()),
            key=lambda item: ((item["namespace_uri"] or ""), item["local_name"]),
        )
        resources.append(
            {
                "resource_id": resource_ids[element],
                "expanded_name": expanded_name(element.tag),
                "locator": {
                    "preorder": preorder,
                    "depth": len(list(element.iterancestors())),
                    "parent_resource_id": parent_resource_id,
                    "element_child_position": child_position,
                    "same_name_sibling_position": same_name_position,
                    "path": path_for(element),
                },
                "element_child_count": len(element_children(element)),
                "attribute_count": len(element.attrib),
                "attribute_expanded_names": attribute_names,
            }
        )
    return resources


def independent_unordered_shape_payload(root: etree._Element) -> list[dict[str, Any]]:
    shapes: list[dict[str, Any]] = []
    for element in root.iter():
        if not isinstance(element.tag, str):
            continue
        child_names = sorted(
            (expanded_name(child.tag) for child in element_children(element)),
            key=lambda item: ((item["namespace_uri"] or ""), item["local_name"]),
        )
        attribute_names = sorted(
            (expanded_name(name) for name in element.attrib.keys()),
            key=lambda item: ((item["namespace_uri"] or ""), item["local_name"]),
        )
        shapes.append(
            {
                "expanded_name": expanded_name(element.tag),
                "child_expanded_names_multiset": child_names,
                "attribute_expanded_names": attribute_names,
            }
        )
    return sorted(shapes, key=topology_digest_bytes)


def expected_b_summary(root: etree._Element) -> dict[str, Any]:
    resources = independent_b_resources(root)
    return {
        "resource_count": len(resources),
        "attribute_count": sum(resource["attribute_count"] for resource in resources),
        "max_depth": max(resource["locator"]["depth"] for resource in resources),
        "namespace_uris": sorted(
            {
                resource["expanded_name"]["namespace_uri"]
                for resource in resources
                if resource["expanded_name"]["namespace_uri"] is not None
            }
        ),
        "ordered_topology_sha256": topology_digest_object(
            [
                {
                    "resource_id": resource["resource_id"],
                    "expanded_name": resource["expanded_name"],
                    "locator": resource["locator"],
                    "element_child_count": resource["element_child_count"],
                    "attribute_count": resource["attribute_count"],
                    "attribute_expanded_names": resource["attribute_expanded_names"],
                }
                for resource in resources
            ]
        ),
        "unordered_element_shape_sha256": topology_digest_object(
            independent_unordered_shape_payload(root)
        ),
    }


def expected_provider_records(
    root: etree._Element,
) -> list[tuple[str, list[dict[str, Any]]]]:
    if expanded_name(root.tag) != {"namespace_uri": None, "local_name": "Tanach"}:
        raise ValueError("source is not the registered UXLC provider shape")
    tanach = exactly_one_direct_child(root, "tanach")
    book = exactly_one_direct_child(tanach, "book")
    chapter = exactly_one_direct_child(book, "c")
    verses = direct_children_named(chapter, "v")
    if not verses:
        raise ValueError("provider shape requires verses")

    expected: list[tuple[str, list[dict[str, Any]]]] = [
        ("provider_book", path_for(book)),
        ("provider_chapter", path_for(chapter)),
    ]
    for verse in verses:
        expected.append(("provider_verse", path_for(verse)))
        expected.extend(
            ("provider_word", path_for(word))
            for word in direct_children_named(verse, "w")
        )
    return expected


def provider_path_key(resource_kind: str, path: list[dict[str, Any]]) -> str:
    return json.dumps(
        [resource_kind, path],
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    )


def validate_file_binding(
    payload: dict[str, Any],
    source: bytes,
    label: str,
    expected_input_id: str | None = None,
) -> None:
    binding = payload.get("file_binding")
    if not isinstance(binding, dict):
        raise ValueError(f"{label} file binding missing")
    require_exact_keys(binding, FILE_BINDING_KEYS, f"{label} file binding")
    source_digest = sha256_bytes(source)
    if binding.get("sha256") != source_digest:
        raise ValueError(f"{label} file binding mismatch")
    if binding.get("byte_size") != len(source):
        raise ValueError(f"{label} byte-size binding mismatch")
    if binding.get("file_id") != f"tos.file.sha256.{source_digest}":
        raise ValueError(f"{label} file ID mismatch")
    if expected_input_id is not None and binding.get("input_id") != expected_input_id:
        raise ValueError(f"{label} input ID mismatch")
    if binding.get("media_type") != "application/xml":
        raise ValueError(f"{label} media type mismatch")


def validate_b_claim_posture(owner: dict[str, Any], label: str = "B") -> None:
    if any(owner.get(key) is not False for key in B_CLAIM_POSTURE_KEYS):
        raise ValueError(f"{label} claim posture mismatch")


def resolve_path(root: etree._Element, path: list[dict[str, Any]]) -> etree._Element:
    if not path:
        raise ValueError("empty path")
    first = path[0]
    if first["expanded_name"] != expanded_name(root.tag):
        raise ValueError("root expanded name mismatch")
    if first["same_name_sibling_position"] != 1:
        raise ValueError("root sibling position mismatch")
    current = root
    for step in path[1:]:
        matching = [
            child
            for child in element_children(current)
            if expanded_name(child.tag) == step["expanded_name"]
        ]
        ordinal = step["same_name_sibling_position"]
        if ordinal < 1 or ordinal > len(matching):
            raise ValueError("path ordinal out of range")
        current = matching[ordinal - 1]
    return current


def validate_b(owner: dict[str, Any], root: etree._Element) -> dict[str, Any]:
    resources = owner["resources"]
    if not isinstance(resources, list):
        raise ValueError("B resources must be a list")
    require_exact_keys(owner.get("scope"), B_SCOPE_KEYS, "B scope")
    if owner["scope"] != EXPECTED_B_SCOPE:
        raise ValueError("B scope value mismatch")
    elements = [element for element in root.iter() if isinstance(element.tag, str)]
    if len(resources) != len(elements):
        raise ValueError("resource/element count mismatch")
    resource_ids = [resource.get("resource_id") for resource in resources]
    if any(not isinstance(resource_id, str) or not resource_id for resource_id in resource_ids):
        raise ValueError("resource IDs are missing")
    duplicate_ids = {
        resource_id
        for resource_id, count in Counter(resource_ids).items()
        if count > 1
    }
    if duplicate_ids:
        raise ValueError("resource IDs must be unique")
    element_to_resource = {element: resource for element, resource in zip(elements, resources)}
    for expected_preorder, resource in enumerate(resources, start=1):
        require_exact_keys(resource, B_RESOURCE_KEYS, "B resource")
        if resource.get("resource_kind") != "xml_element":
            raise ValueError("resource kind mismatch")
        element = resolve_path(root, resource["locator"]["path"])
        if element is not elements[expected_preorder - 1]:
            raise ValueError("path does not return registered preorder element")
        if resource["expanded_name"] != expanded_name(element.tag):
            raise ValueError("expanded name mismatch")
        if resource["locator"]["preorder"] != expected_preorder:
            raise ValueError("preorder mismatch")
        depth = len(list(element.iterancestors()))
        if resource["locator"]["depth"] != depth:
            raise ValueError("depth mismatch")
        parent = element.getparent()
        if parent is None:
            expected_parent_ref = None
            expected_child_position = 1
            expected_same_name_position = 1
        else:
            siblings = element_children(parent)
            expected_parent_ref = element_to_resource[parent]["resource_id"]
            expected_child_position = siblings.index(element) + 1
            matching = [
                sibling
                for sibling in siblings
                if expanded_name(sibling.tag) == expanded_name(element.tag)
            ]
            expected_same_name_position = matching.index(element) + 1
        if resource["locator"]["parent_resource_id"] != expected_parent_ref:
            raise ValueError("parent ref mismatch")
        if resource["locator"]["element_child_position"] != expected_child_position:
            raise ValueError("element child position mismatch")
        if resource["locator"]["same_name_sibling_position"] != expected_same_name_position:
            raise ValueError("same-name sibling position mismatch")
        if resource["element_child_count"] != len(element_children(element)):
            raise ValueError("element child count mismatch")
        if resource["attribute_count"] != len(element.attrib):
            raise ValueError("attribute count mismatch")
        expected_attributes = sorted(
            (expanded_name(name) for name in element.attrib.keys()),
            key=lambda item: ((item["namespace_uri"] or ""), item["local_name"]),
        )
        if resource["attribute_expanded_names"] != expected_attributes:
            raise ValueError("attribute expanded names mismatch")
    expected_summary = expected_b_summary(root)
    summary = owner.get("summary")
    require_exact_keys(summary, B_SUMMARY_KEYS, "B summary")
    if summary != expected_summary:
        raise ValueError("B summary mismatch")
    return {
        "resource_count": len(resources),
        "resolved_exactly_once": len(resources),
        "path_failures": 0,
        "metadata_mismatches": 0,
        "summary": expected_summary,
    }


def validate_provider_coordinate(
    record: dict[str, Any],
    element: etree._Element,
    projection: dict[str, Any],
) -> None:
    coordinate = record.get("provider_coordinate")
    context = projection.get("provider_context")
    if not isinstance(coordinate, dict) or not isinstance(context, dict):
        raise ValueError("provider coordinate/context missing")
    if coordinate.get("book") != context.get("book_code"):
        raise ValueError("provider book coordinate mismatch")

    resource_kind = record.get("resource_kind")
    if resource_kind == "provider_book":
        if set(coordinate) != {"book"}:
            raise ValueError("provider book coordinate shape mismatch")
        return

    parent = element.getparent()
    if parent is None:
        raise ValueError("provider coordinate parent missing")
    if resource_kind == "provider_chapter":
        if expanded_name(parent.tag) != {"namespace_uri": None, "local_name": "book"}:
            raise ValueError("provider chapter parent mismatch")
        if set(coordinate) != {"book", "chapter"}:
            raise ValueError("provider chapter coordinate shape mismatch")
        if coordinate.get("chapter") != element.get("n"):
            raise ValueError("provider chapter coordinate does not match n")
        return

    if resource_kind == "provider_verse":
        if expanded_name(parent.tag) != {"namespace_uri": None, "local_name": "c"}:
            raise ValueError("provider verse parent mismatch")
        if set(coordinate) != {"book", "chapter", "verse"}:
            raise ValueError("provider verse coordinate shape mismatch")
        if coordinate.get("chapter") != parent.get("n"):
            raise ValueError("provider verse chapter coordinate does not match parent n")
        if coordinate.get("verse") != element.get("n"):
            raise ValueError("provider verse coordinate does not match n")
        return

    if resource_kind == "provider_word":
        if expanded_name(parent.tag) != {"namespace_uri": None, "local_name": "v"}:
            raise ValueError("provider word parent mismatch")
        if set(coordinate) != {
            "book",
            "chapter",
            "verse",
            "provider_word_position",
        }:
            raise ValueError("provider word coordinate shape mismatch")
        chapter = parent.getparent()
        if chapter is None or expanded_name(chapter.tag) != {"namespace_uri": None, "local_name": "c"}:
            raise ValueError("provider word chapter parent mismatch")
        if coordinate.get("chapter") != chapter.get("n"):
            raise ValueError("provider word chapter coordinate does not match parent n")
        if coordinate.get("verse") != parent.get("n"):
            raise ValueError("provider word verse coordinate does not match parent n")
        word_siblings = [
            child
            for child in element_children(parent)
            if expanded_name(child.tag) == {"namespace_uri": None, "local_name": "w"}
        ]
        expected_position = word_siblings.index(element) + 1 if element in word_siblings else None
        if coordinate.get("provider_word_position") != expected_position:
            raise ValueError("provider word position does not match sibling position")
        return

    raise ValueError("unknown provider resource kind")


def validate_projection(
    projection: dict[str, Any],
    root: etree._Element,
    owner: dict[str, Any] | None,
    registered_context: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if owner is not None:
        projection_keys = {
            "schema_version",
            "provider_context",
            "generic_owner_candidate",
            "summary",
            "resources",
            "source_text_included",
            "accepted_structure_claimed",
            "authority_boundary",
        }
        require_exact_keys(projection, projection_keys, "provider projection")
    elif "candidate" in projection:
        # Direct contract tests and callers may pass the complete C payload.
        require_exact_keys(projection, C_TOP_LEVEL_KEYS, "C payload")
    else:
        projection_keys = {
            "schema_version",
            "provider_context",
            "summary",
            "resources",
            "source_text_included",
            "generic_xml_owner_claimed",
            "intrinsic_or_cross_corpus_word_ids_claimed",
            "accepted_structure_claimed",
            "authority_boundary",
        }
        require_exact_keys(projection, projection_keys, "provider projection")
    require_exact_keys(projection.get("provider_context"), PROVIDER_CONTEXT_KEYS, "provider context")
    if owner is not None and projection.get("generic_owner_candidate") != "B":
        raise ValueError("generic projection owner must be B")
    if registered_context is not None and projection.get("provider_context") != registered_context:
        raise ValueError("provider context differs from registered selection")
    if not isinstance(projection.get("resources"), list):
        raise ValueError("provider projection resources must be a list")
    require_exact_keys(projection.get("summary"), C_SUMMARY_KEYS, "provider projection summary")
    expected = expected_provider_records(root)
    expected_keys = {
        provider_path_key(resource_kind, path)
        for resource_kind, path in expected
    }
    actual_keys = [
        provider_path_key(record["resource_kind"], record["source_element_path"])
        for record in projection["resources"]
    ]
    actual_key_set = set(actual_keys)
    duplicate_keys = {
        key for key, count in Counter(actual_keys).items() if count > 1
    }
    missing_keys = expected_keys - actual_key_set
    unexpected_keys = actual_key_set - expected_keys
    if duplicate_keys or missing_keys or unexpected_keys:
        raise ValueError(
            "provider projection is incomplete or duplicated: "
            f"duplicates={len(duplicate_keys)} "
            f"missing={len(missing_keys)} unexpected={len(unexpected_keys)}"
        )

    resource_ids = [record.get("resource_id") for record in projection["resources"]]
    if any(not isinstance(resource_id, str) or not resource_id for resource_id in resource_ids):
        raise ValueError("resource IDs are missing")
    duplicate_resource_ids = {
        resource_id
        for resource_id, count in Counter(resource_ids).items()
        if count > 1
    }
    if duplicate_resource_ids:
        raise ValueError("resource IDs must be unique")

    owner_by_id = (
        {resource["resource_id"]: resource for resource in owner["resources"]}
        if owner is not None
        else {}
    )
    expected_local_names = {
        "provider_book": "book",
        "provider_chapter": "c",
        "provider_verse": "v",
        "provider_word": "w",
    }
    cited_generic = 0
    validated_records: list[dict[str, Any]] = []
    for record in projection["resources"]:
        require_exact_keys(record, PROJECTION_RECORD_KEYS, "provider projection resource")
        element = resolve_path(root, record["source_element_path"])
        if expanded_name(element.tag) != {
            "namespace_uri": None,
            "local_name": expected_local_names[record["resource_kind"]],
        }:
            raise ValueError("provider record resolves to wrong source element")
        validate_provider_coordinate(record, element, projection)
        generic_ref = record["generic_resource_ref"]
        if owner is None:
            if generic_ref is not None:
                raise ValueError("primary C unexpectedly cites absent B")
        else:
            if generic_ref not in owner_by_id:
                raise ValueError("projection generic ref missing from B")
            if owner_by_id[generic_ref]["locator"]["path"] != record["source_element_path"]:
                raise ValueError("projection path differs from cited B path")
            cited_generic += 1
        validated_records.append(record)

    verse_records = [
        record
        for record in validated_records
        if record["resource_kind"] == "provider_verse"
    ]
    word_records = [
        record
        for record in validated_records
        if record["resource_kind"] == "provider_word"
    ]
    word_counts_by_verse = [
        sum(
            1
            for word in word_records
            if all(
                word["provider_coordinate"].get(key)
                == verse["provider_coordinate"].get(key)
                for key in ("book", "chapter", "verse")
            )
        )
        for verse in verse_records
    ]
    expected_summary = {
        "resource_count": len(validated_records),
        "verse_count": len(verse_records),
        "word_count": len(word_records),
        "word_counts_by_verse": word_counts_by_verse,
    }
    if projection.get("summary") != expected_summary:
        raise ValueError("provider projection summary mismatch")
    return {
        "resource_count": len(projection["resources"]),
        "resolved_exactly_once": len(actual_key_set),
        "expected_resource_count": len(expected_keys),
        "unique_resource_count": len(actual_key_set),
        "unique_resource_id_count": len(resource_ids),
        "generic_refs_verified": cited_generic,
        "path_failures": 0,
        "summary": expected_summary,
    }


def validate_candidate(
    payload: dict[str, Any],
    source: bytes,
    registered_context: dict[str, Any] | None = None,
    expected_candidate: str | None = None,
    expected_input_id: str | None = None,
    expected_lab_id: str | None = None,
    expected_schema_version: str | None = None,
) -> dict[str, Any]:
    candidate = payload.get("candidate") if isinstance(payload, dict) else None
    if expected_candidate is not None and candidate != expected_candidate:
        raise ValueError("payload candidate differs from requested candidate")
    if expected_lab_id is not None and payload.get("lab_id") != expected_lab_id:
        raise ValueError("payload lab ID mismatch")
    if expected_schema_version is not None and payload.get("schema_version") != expected_schema_version:
        raise ValueError("payload schema version mismatch")
    root = strict_parse(source)
    if candidate == "A":
        require_exact_keys(payload, A_TOP_LEVEL_KEYS, "A payload")
        require_exact_keys(payload.get("file_binding"), FILE_BINDING_KEYS, "A file binding")
        require_exact_keys(payload.get("parser_posture"), PARSER_POSTURE_KEYS, "A parser posture")
        validate_file_binding(payload, source, "A", expected_input_id)
        if payload["element_return_supported"] is not False:
            raise ValueError("A overclaims element return")
        if payload["intrinsic_ids_claimed"] is not False or payload["source_text_included"] is not False:
            raise ValueError("A payload claim posture mismatch")
        if payload["scope"] != "one opaque XML document resource":
            raise ValueError("A scope mismatch")
        resources = payload.get("resources")
        if resources != [
            {
                "resource_id": "xml-document-000001",
                "resource_kind": "xml_document",
                "locator": {"whole_file": True},
            }
        ]:
            raise ValueError("A resource shape mismatch")
        return {
            "document_fixity_match": True,
            "resource_count": len(payload["resources"]),
            "element_return_supported": False,
        }
    if candidate == "B":
        require_exact_keys(payload, B_TOP_LEVEL_KEYS, "B payload")
        require_exact_keys(payload.get("file_binding"), FILE_BINDING_KEYS, "B file binding")
        require_exact_keys(payload.get("parser_posture"), PARSER_POSTURE_KEYS, "B parser posture")
        validate_file_binding(payload, source, "B", expected_input_id)
        validate_b_claim_posture(payload)
        return validate_b(payload, root)
    if candidate == "C":
        require_exact_keys(payload, C_TOP_LEVEL_KEYS, "C payload")
        require_exact_keys(payload.get("file_binding"), FILE_BINDING_KEYS, "C file binding")
        require_exact_keys(payload.get("parser_posture"), PARSER_POSTURE_KEYS, "C parser posture")
        validate_file_binding(payload, source, "C", expected_input_id)
        if any(
            payload[key] is not False
            for key in (
                "source_text_included",
                "generic_xml_owner_claimed",
                "intrinsic_or_cross_corpus_word_ids_claimed",
                "accepted_structure_claimed",
            )
        ):
            raise ValueError("C claim posture mismatch")
        result = validate_projection(payload, root, None, registered_context)
        result["generic_owner_supported"] = False
        return result
    if candidate == "BC":
        require_exact_keys(payload, BC_TOP_LEVEL_KEYS, "BC payload")
        if payload["source_text_included"] is not False:
            raise ValueError("BC claim posture mismatch")
        owner = payload.get("owner")
        projection = payload.get("projection")
        if not isinstance(owner, dict) or not isinstance(projection, dict):
            raise ValueError("BC owner/projection missing")
        require_exact_keys(owner, B_TOP_LEVEL_KEYS, "BC owner")
        require_exact_keys(owner.get("file_binding"), FILE_BINDING_KEYS, "BC owner file binding")
        require_exact_keys(owner.get("parser_posture"), PARSER_POSTURE_KEYS, "BC owner parser posture")
        if owner.get("candidate") != "B":
            raise ValueError("BC owner candidate mismatch")
        if expected_lab_id is not None and owner.get("lab_id") != expected_lab_id:
            raise ValueError("BC owner lab ID mismatch")
        if owner.get("schema_version") != EXPECTED_SCHEMA_VERSIONS["B"]:
            raise ValueError("BC owner schema version mismatch")
        validate_file_binding(owner, source, "BC owner", expected_input_id)
        validate_b_claim_posture(owner, "BC owner")
        owner_result = validate_b(owner, root)
        projection_result = validate_projection(
            projection,
            root,
            owner,
            registered_context,
        )
        return {"owner": owner_result, "projection": projection_result}
    raise ValueError("unknown candidate")


def fixture_bytes(manifest: dict[str, Any], fixture_id: str) -> bytes:
    matches = [fixture for fixture in manifest["fixtures"] if fixture["id"] == fixture_id]
    if len(matches) != 1:
        raise ValueError("fixture lookup failed")
    return matches[0]["xml"].encode("utf-8")


def write_receipt(value: Any) -> None:
    temporary = RECEIPT_PATH.with_name(RECEIPT_PATH.name + ".tmp")
    temporary.write_bytes(canonical_bytes(value))
    os.chmod(temporary, 0o644)
    temporary.replace(RECEIPT_PATH)


def main() -> int:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    started = time.perf_counter()
    checks: list[dict[str, Any]] = []
    errors: list[dict[str, str]] = []

    for fixture in manifest["fixtures"]:
        fixture_id = fixture["id"]
        candidates = ["A", "B"]
        if fixture_id == "PC1-uxlc-shape":
            candidates.extend(["C", "BC"])
        source = fixture_bytes(manifest, fixture_id)
        for candidate in candidates:
            path = PUBLIC_ROOT / candidate.lower() / f"{fixture_id}.json"
            try:
                payload = json.loads(path.read_text(encoding="utf-8"))
                checks.append(
                    {
                        "selection_kind": "fixture",
                        "selection_id": fixture_id,
                        "candidate": candidate,
                        "output_sha256": sha256_bytes(path.read_bytes()),
                        "result": validate_candidate(
                            payload,
                            source,
                            expected_candidate=candidate,
                            expected_input_id=fixture_id,
                            expected_lab_id=manifest["lab_id"],
                            expected_schema_version=EXPECTED_SCHEMA_VERSIONS[candidate],
                        ),
                    }
                )
            except (OSError, json.JSONDecodeError, KeyError, TypeError, ValueError) as exc:
                errors.append(
                    {
                        "selection_kind": "fixture",
                        "selection_id": fixture_id,
                        "candidate": candidate,
                        "error_class": type(exc).__name__,
                    }
                )

    private_root = Path(manifest["private_output_root"])
    for source_id, source_record in manifest["exact_sources"].items():
        source_path = Path(source_record["path"])
        if not source_path.is_file():
            continue
        source = source_path.read_bytes()
        for candidate in ("A", "B", "C", "BC"):
            path = private_root / "outputs" / source_id / "run-1" / f"{candidate.lower()}.json"
            try:
                payload = json.loads(path.read_text(encoding="utf-8"))
                checks.append(
                    {
                        "selection_kind": "source",
                        "selection_id": source_id,
                        "candidate": candidate,
                        "output_sha256": sha256_bytes(path.read_bytes()),
                        "result": validate_candidate(
                            payload,
                            source,
                            manifest["provider_context"],
                            expected_candidate=candidate,
                            expected_input_id=source_id,
                            expected_lab_id=manifest["lab_id"],
                            expected_schema_version=EXPECTED_SCHEMA_VERSIONS[candidate],
                        ),
                    }
                )
            except (OSError, json.JSONDecodeError, KeyError, TypeError, ValueError) as exc:
                errors.append(
                    {
                        "selection_kind": "source",
                        "selection_id": source_id,
                        "candidate": candidate,
                        "error_class": type(exc).__name__,
                    }
                )

    receipt = {
        "schema_version": "tos_generic_xml_resource_inventory_independent_consumer_receipt_v1",
        "lab_id": manifest["lab_id"],
        "consumer_imports_builder": False,
        "consumer_reads_sealed_manifest": False,
        "check_count": len(checks),
        "error_count": len(errors),
        "checks": checks,
        "errors": errors,
        "all_paths_and_metadata_return": len(errors) == 0,
        "wall_seconds": time.perf_counter() - started,
        "max_rss_kib": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
        "timing_scope": "independent consumer checks and receipt assembly",
        "source_text_included": False,
        "authority_boundary": "independent source-return mechanics only; no source-text, linguistic, translation, semantic, graph, canon, rights or publication authority",
    }
    write_receipt(receipt)
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
