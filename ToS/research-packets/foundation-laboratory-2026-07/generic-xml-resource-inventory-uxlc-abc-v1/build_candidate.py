#!/usr/bin/env python3
"""Build one deterministic candidate for the generic-XML A/B/C laboratory.

The program intentionally does not read the sealed evaluation manifest. Exact
source-derived outputs are directed to the Git-ignored operator-local tree by
the runner. Public tracked inputs are synthetic.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path
from typing import Any

from lxml import etree


LAB_ID = "tos.lab.generic-xml-resource-inventory-uxlc-abc.v1"
DOCTYPE_PATTERN = re.compile(br"<!DOCTYPE\s", re.IGNORECASE)
ASCII_INTEGER = re.compile(r"^[0-9]+$")


class CandidateBuildError(RuntimeError):
    """Expected fail-closed laboratory error."""


def canonical_bytes(value: Any) -> bytes:
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


def digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def digest_object(value: Any) -> str:
    return digest_bytes(canonical_bytes(value))


def expanded_name(raw_name: str) -> dict[str, str | None]:
    if raw_name.startswith("{"):
        namespace_uri, local_name = raw_name[1:].split("}", 1)
        return {"namespace_uri": namespace_uri, "local_name": local_name}
    return {"namespace_uri": None, "local_name": raw_name}


def expanded_key(element: etree._Element) -> tuple[str | None, str]:
    name = expanded_name(element.tag)
    return name["namespace_uri"], name["local_name"]  # type: ignore[return-value]


def element_children(element: etree._Element) -> list[etree._Element]:
    return [child for child in element if isinstance(child.tag, str)]


def strict_parse(payload: bytes) -> etree._Element:
    if DOCTYPE_PATTERN.search(payload):
        raise CandidateBuildError("DOCTYPE declarations are forbidden")
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
    try:
        root = etree.fromstring(payload, parser=parser)
    except etree.XMLSyntaxError as exc:
        raise CandidateBuildError("strict XML parse failed") from exc
    # The byte-level guard above is only a fast path for UTF-8.  libxml2 has
    # already decoded the document at this point, so docinfo also catches
    # UTF-16/UTF-32 declarations whose bytes contain interleaved NULs.
    docinfo = root.getroottree().docinfo
    if docinfo.doctype or docinfo.internalDTD is not None or docinfo.externalDTD is not None:
        raise CandidateBuildError("DOCTYPE declarations are forbidden")
    return root


def parser_posture() -> dict[str, bool | str]:
    return {
        "parser": "lxml.etree.XMLParser",
        "mode": "strict_xml",
        "recover": False,
        "load_dtd": False,
        "dtd_validation": False,
        "resolve_entities": False,
        "no_network": True,
        "huge_tree": False,
        "xinclude": False,
        "reject_doctype": True,
    }


def file_binding(payload: bytes, input_id: str) -> dict[str, Any]:
    sha256 = digest_bytes(payload)
    return {
        "input_id": input_id,
        "file_id": f"tos.file.sha256.{sha256}",
        "sha256": sha256,
        "byte_size": len(payload),
        "media_type": "application/xml",
    }


def path_for(element: etree._Element) -> list[dict[str, Any]]:
    lineage = list(reversed(list(element.iterancestors()))) + [element]
    result: list[dict[str, Any]] = []
    for node in lineage:
        parent = node.getparent()
        if parent is None:
            same_name_position = 1
        else:
            siblings = [
                child
                for child in element_children(parent)
                if expanded_key(child) == expanded_key(node)
            ]
            same_name_position = siblings.index(node) + 1
        result.append(
            {
                "expanded_name": expanded_name(node.tag),
                "same_name_sibling_position": same_name_position,
            }
        )
    return result


def structural_resources(
    root: etree._Element,
) -> tuple[list[dict[str, Any]], dict[etree._Element, str]]:
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
            siblings = element_children(parent)
            child_position = siblings.index(element) + 1
            same_name_siblings = [
                sibling
                for sibling in siblings
                if expanded_key(sibling) == expanded_key(element)
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
                "resource_kind": "xml_element",
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
    return resources, resource_ids


def ordered_topology_payload(resources: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
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


def unordered_shape_payload(root: etree._Element) -> list[dict[str, Any]]:
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
    return sorted(shapes, key=lambda value: canonical_bytes(value))


def build_a(payload: bytes, input_id: str) -> dict[str, Any]:
    strict_parse(payload)
    return {
        "schema_version": "tos_lab_generic_xml_candidate_a_v1",
        "lab_id": LAB_ID,
        "candidate": "A",
        "file_binding": file_binding(payload, input_id),
        "parser_posture": parser_posture(),
        "scope": "one opaque XML document resource",
        "resources": [
            {
                "resource_id": "xml-document-000001",
                "resource_kind": "xml_document",
                "locator": {"whole_file": True},
            }
        ],
        "source_text_included": False,
        "element_return_supported": False,
        "intrinsic_ids_claimed": False,
        "authority_boundary": "exact capture view only; no element, passage, source-text, linguistic, semantic, graph, canon, rights or publication authority",
    }


def build_b_from_root(
    payload: bytes, input_id: str, root: etree._Element
) -> tuple[dict[str, Any], dict[etree._Element, str]]:
    resources, resource_ids = structural_resources(root)
    attribute_count = sum(resource["attribute_count"] for resource in resources)
    namespace_uris = sorted(
        {
            resource["expanded_name"]["namespace_uri"]
            for resource in resources
            if resource["expanded_name"]["namespace_uri"] is not None
        }
    )
    ordered_payload = ordered_topology_payload(resources)
    unordered_payload = unordered_shape_payload(root)
    result = {
        "schema_version": "tos_lab_generic_xml_candidate_b_v1",
        "lab_id": LAB_ID,
        "candidate": "B",
        "file_binding": file_binding(payload, input_id),
        "parser_posture": parser_posture(),
        "scope": {
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
        },
        "summary": {
            "resource_count": len(resources),
            "attribute_count": attribute_count,
            "max_depth": max(resource["locator"]["depth"] for resource in resources),
            "namespace_uris": namespace_uris,
            "ordered_topology_sha256": digest_object(ordered_payload),
            "unordered_element_shape_sha256": digest_object(unordered_payload),
        },
        "resources": resources,
        "source_text_included": False,
        "element_content_fingerprints_included": False,
        "intrinsic_ids_claimed": False,
        "cross_file_identity_claimed": False,
        "tei_classification_claimed": False,
        "authority_boundary": "generic exact-file element topology and source return only; no provider passage, accepted structure, source-text, linguistic, semantic, graph, canon, rights or publication authority",
    }
    return result, resource_ids


def build_b(payload: bytes, input_id: str) -> dict[str, Any]:
    root = strict_parse(payload)
    result, _ = build_b_from_root(payload, input_id, root)
    return result


def direct_children_named(element: etree._Element, local_name: str) -> list[etree._Element]:
    return [
        child
        for child in element_children(element)
        if expanded_name(child.tag) == {"namespace_uri": None, "local_name": local_name}
    ]


def exactly_one(values: list[etree._Element], label: str) -> etree._Element:
    if len(values) != 1:
        raise CandidateBuildError(f"UXLC provider shape requires one {label}")
    return values[0]


def numeric_attribute(element: etree._Element, attribute: str, label: str) -> str:
    value = element.get(attribute)
    if value is None or ASCII_INTEGER.fullmatch(value) is None:
        raise CandidateBuildError(f"UXLC provider shape requires numeric {label}")
    return value


def provider_projection(
    root: etree._Element,
    provider_context: dict[str, Any],
    generic_resource_ids: dict[etree._Element, str] | None,
) -> list[dict[str, Any]]:
    if expanded_name(root.tag) != {"namespace_uri": None, "local_name": "Tanach"}:
        raise CandidateBuildError("source is not the registered UXLC provider shape")
    tanach = exactly_one(direct_children_named(root, "tanach"), "tanach subtree")
    book = exactly_one(direct_children_named(tanach, "book"), "selected book")
    chapter = exactly_one(direct_children_named(book, "c"), "selected chapter")
    chapter_number = numeric_attribute(chapter, "n", "chapter n")
    verses = direct_children_named(chapter, "v")
    if not verses:
        raise CandidateBuildError("UXLC provider shape requires verses")

    records: list[dict[str, Any]] = []

    def append_record(
        resource_id: str,
        resource_kind: str,
        element: etree._Element,
        coordinate: dict[str, Any],
    ) -> None:
        record = {
            "resource_id": resource_id,
            "resource_kind": resource_kind,
            "provider_coordinate": coordinate,
            "source_element_path": path_for(element),
            "generic_resource_ref": (
                generic_resource_ids[element] if generic_resource_ids is not None else None
            ),
        }
        records.append(record)

    book_code = provider_context["book_code"]
    append_record(
        "uxlc-book-000001",
        "provider_book",
        book,
        {"book": book_code},
    )
    append_record(
        "uxlc-chapter-000001",
        "provider_chapter",
        chapter,
        {"book": book_code, "chapter": chapter_number},
    )
    word_serial = 0
    for verse_serial, verse in enumerate(verses, start=1):
        verse_number = numeric_attribute(verse, "n", "verse n")
        append_record(
            f"uxlc-verse-{verse_serial:06d}",
            "provider_verse",
            verse,
            {"book": book_code, "chapter": chapter_number, "verse": verse_number},
        )
        words = direct_children_named(verse, "w")
        for word_position, word in enumerate(words, start=1):
            word_serial += 1
            append_record(
                f"uxlc-word-{word_serial:06d}",
                "provider_word",
                word,
                {
                    "book": book_code,
                    "chapter": chapter_number,
                    "verse": verse_number,
                    "provider_word_position": word_position,
                },
            )
    return records


def c_context(base: dict[str, Any], input_kind: str) -> dict[str, Any]:
    if input_kind == "fixture":
        return {
            "provider": "synthetic UXLC-shape control",
            "expression": "synthetic",
            "edition": "synthetic",
            # Do not copy the exact-source provider code into tracked
            # synthetic fixtures; synthetic output must remain source-free.
            "book_code": "SYNTH",
            "selector": "synthetic",
            "projection_authority": "provider_shape_test_only",
        }
    return dict(base)


def build_c(
    payload: bytes,
    input_id: str,
    provider_context: dict[str, Any],
    input_kind: str,
) -> dict[str, Any]:
    root = strict_parse(payload)
    context = c_context(provider_context, input_kind)
    records = provider_projection(root, context, None)
    verse_records = [record for record in records if record["resource_kind"] == "provider_verse"]
    word_records = [record for record in records if record["resource_kind"] == "provider_word"]
    return {
        "schema_version": "tos_lab_generic_xml_candidate_c_v1",
        "lab_id": LAB_ID,
        "candidate": "C",
        "file_binding": file_binding(payload, input_id),
        "parser_posture": parser_posture(),
        "provider_context": context,
        "summary": {
            "resource_count": len(records),
            "verse_count": len(verse_records),
            "word_count": len(word_records),
            "word_counts_by_verse": [
                len(
                    [
                        word
                        for word in word_records
                        if word["provider_coordinate"]["verse"]
                        == verse["provider_coordinate"]["verse"]
                    ]
                )
                for verse in verse_records
            ],
        },
        "resources": records,
        "source_text_included": False,
        "generic_xml_owner_claimed": False,
        "intrinsic_or_cross_corpus_word_ids_claimed": False,
        "accepted_structure_claimed": False,
        "authority_boundary": "UXLC provider-coordinate challenger only; not a generic XML owner and no accepted source-text, word, linguistic, semantic, graph, canon, rights or publication authority",
    }


def build_bc(
    payload: bytes,
    input_id: str,
    provider_context: dict[str, Any],
    input_kind: str,
) -> dict[str, Any]:
    root = strict_parse(payload)
    owner, resource_ids = build_b_from_root(payload, input_id, root)
    context = c_context(provider_context, input_kind)
    records = provider_projection(root, context, resource_ids)
    verse_records = [record for record in records if record["resource_kind"] == "provider_verse"]
    word_records = [record for record in records if record["resource_kind"] == "provider_word"]
    projection = {
        "schema_version": "tos_lab_uxlc_provider_projection_over_generic_xml_v1",
        "provider_context": context,
        "generic_owner_candidate": "B",
        "summary": {
            "resource_count": len(records),
            "verse_count": len(verse_records),
            "word_count": len(word_records),
            "word_counts_by_verse": [
                len(
                    [
                        word
                        for word in word_records
                        if word["provider_coordinate"]["verse"]
                        == verse["provider_coordinate"]["verse"]
                    ]
                )
                for verse in verse_records
            ],
        },
        "resources": records,
        "source_text_included": False,
        "accepted_structure_claimed": False,
        "authority_boundary": "derived provider navigation only; every record returns through B and creates no source-text, linguistic, semantic, graph, canon, rights or publication authority",
    }
    return {
        "schema_version": "tos_lab_generic_xml_candidate_bc_v1",
        "lab_id": LAB_ID,
        "candidate": "BC",
        "owner": owner,
        "projection": projection,
        "source_text_included": False,
        "authority_boundary": "laboratory owner/projection composition only; no public contract or downstream acceptance",
    }


def load_input(
    manifest: dict[str, Any], fixture_id: str | None, source_id: str | None
) -> tuple[bytes, str, str]:
    if (fixture_id is None) == (source_id is None):
        raise CandidateBuildError("select exactly one fixture or source")
    if fixture_id is not None:
        all_fixtures = manifest["fixtures"] + manifest["security_fixtures"]
        matches = [fixture for fixture in all_fixtures if fixture["id"] == fixture_id]
        if len(matches) != 1:
            raise CandidateBuildError("unknown or duplicate fixture id")
        return matches[0]["xml"].encode("utf-8"), fixture_id, "fixture"
    source = manifest["exact_sources"].get(source_id)
    if source is None:
        raise CandidateBuildError("unknown source id")
    path = Path(source["path"])
    if not path.is_file():
        raise CandidateBuildError("declared local source is unavailable")
    return path.read_bytes(), source_id, "source"


def write_output(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(canonical_bytes(value))
    os.chmod(temporary, 0o600)
    temporary.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--candidate", choices=["A", "B", "C", "BC"], required=True)
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--fixture")
    selection.add_argument("--source", choices=["selected", "replay"])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    try:
        manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
        payload, input_id, input_kind = load_input(manifest, args.fixture, args.source)
        if args.candidate == "A":
            result = build_a(payload, input_id)
        elif args.candidate == "B":
            result = build_b(payload, input_id)
        elif args.candidate == "C":
            result = build_c(
                payload,
                input_id,
                manifest["provider_context"],
                input_kind,
            )
        else:
            result = build_bc(
                payload,
                input_id,
                manifest["provider_context"],
                input_kind,
            )
        write_output(args.output, result)
        return 0
    except (CandidateBuildError, json.JSONDecodeError, OSError) as exc:
        print(f"CandidateBuildError: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
