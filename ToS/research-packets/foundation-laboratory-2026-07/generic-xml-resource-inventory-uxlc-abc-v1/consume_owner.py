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
    payload: dict[str, Any], source: bytes, label: str
) -> None:
    binding = payload.get("file_binding")
    if not isinstance(binding, dict):
        raise ValueError(f"{label} file binding missing")
    if binding.get("sha256") != sha256_bytes(source):
        raise ValueError(f"{label} file binding mismatch")
    if binding.get("byte_size") != len(source):
        raise ValueError(f"{label} byte-size binding mismatch")


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
    elements = [element for element in root.iter() if isinstance(element.tag, str)]
    if len(resources) != len(elements):
        raise ValueError("resource/element count mismatch")
    element_to_resource = {element: resource for element, resource in zip(elements, resources)}
    for expected_preorder, resource in enumerate(resources, start=1):
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
    return {
        "resource_count": len(resources),
        "resolved_exactly_once": len(resources),
        "path_failures": 0,
        "metadata_mismatches": 0,
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
) -> dict[str, Any]:
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
    for record in projection["resources"]:
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
    return {
        "resource_count": len(projection["resources"]),
        "resolved_exactly_once": len(actual_key_set),
        "expected_resource_count": len(expected_keys),
        "unique_resource_count": len(actual_key_set),
        "generic_refs_verified": cited_generic,
        "path_failures": 0,
    }


def validate_candidate(payload: dict[str, Any], source: bytes) -> dict[str, Any]:
    root = strict_parse(source)
    candidate = payload["candidate"]
    if candidate == "A":
        validate_file_binding(payload, source, "A")
        if payload["element_return_supported"] is not False:
            raise ValueError("A overclaims element return")
        return {
            "document_fixity_match": True,
            "resource_count": len(payload["resources"]),
            "element_return_supported": False,
        }
    if candidate == "B":
        validate_file_binding(payload, source, "B")
        return validate_b(payload, root)
    if candidate == "C":
        validate_file_binding(payload, source, "C")
        result = validate_projection(payload, root, None)
        result["generic_owner_supported"] = False
        return result
    if candidate == "BC":
        validate_file_binding(payload["owner"], source, "BC owner")
        owner_result = validate_b(payload["owner"], root)
        projection_result = validate_projection(payload["projection"], root, payload["owner"])
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
                        "result": validate_candidate(payload, source),
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
                        "result": validate_candidate(payload, source),
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
