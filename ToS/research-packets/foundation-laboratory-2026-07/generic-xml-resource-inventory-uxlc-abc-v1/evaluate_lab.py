#!/usr/bin/env python3
"""Evaluate frozen gates after direct manual source/output review.

The evaluator is intentionally last. A green result is a mechanical receipt,
not a substitute for the preceding source-visible review.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from lxml import etree


LAB_DIR = Path(__file__).resolve().parent
REPO_ROOT = LAB_DIR.parents[3]
INPUT_PATH = LAB_DIR / "input-manifest.json"
SEALED_PATH = LAB_DIR / "sealed-evaluation-manifest-v2.json"
FREEZE_PATH = LAB_DIR / "freeze-receipt-v14.json"
OBSERVATIONS_PATH = LAB_DIR / "run-observations.json"
SOURCE_RECEIPT_PATH = LAB_DIR / "source-run-receipt.json"
CONSUMER_PATH = LAB_DIR / "independent-consumer-receipt.json"
MANUAL_PATH = LAB_DIR / "MANUAL_SOURCE_AND_OUTPUT_REVIEW.md"
RESULT_PATH = LAB_DIR / "comparison-result.json"
DOCTYPE_PATTERN = re.compile(br"<!DOCTYPE\s", re.IGNORECASE)
HEBREW_PATTERN = re.compile(r"[\u0590-\u05ff]")
PRIVATE_OUTPUT_MODE = 0o600
LAB_RELATIVE = LAB_DIR.relative_to(REPO_ROOT).as_posix()
METHOD_FILE_REFS = tuple(
    f"{LAB_RELATIVE}/{name}"
    for name in ("build_candidate.py", "run_experiment.py", "consume_owner.py", "evaluate_lab.py")
)
ATTRIBUTE_VALUE_PATTERN = re.compile(
    r"\s+[A-Za-z_][\w:.-]*\s*=\s*(?P<quote>['\"])(?P<value>.*?)(?P=quote)",
    re.DOTALL,
)
COMMENT_BODY_PATTERN = re.compile(r"<!--(.*?)-->", re.DOTALL)
PROCESSING_INSTRUCTION_PATTERN = re.compile(r"<\?[^\s?]+\s+(.+?)\?>", re.DOTALL)
TEXT_BETWEEN_TAGS_PATTERN = re.compile(r">([^<]*)<", re.DOTALL)
POSITIVE_AUTHORITY_PATTERN = re.compile(
    r"\b(?:accepted|canonical|promoted|admitted|authorized|approved|public\s+contract|"
    r"source[- ]text|semantic|translation|graph|canon|publication)\b",
    re.IGNORECASE,
)
AUTHORITY_NEGATION_PATTERN = re.compile(r"\b(?:no|not|never|without)\b", re.IGNORECASE)


def canonical_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_path(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


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


def expanded_name(raw_name: str) -> dict[str, str | None]:
    if raw_name.startswith("{"):
        namespace_uri, local_name = raw_name[1:].split("}", 1)
        return {"namespace_uri": namespace_uri, "local_name": local_name}
    return {"namespace_uri": None, "local_name": raw_name}


def element_children(element: etree._Element) -> list[etree._Element]:
    return [child for child in element if isinstance(child.tag, str)]


def expanded_key(element: etree._Element) -> tuple[str | None, str]:
    name = expanded_name(element.tag)
    return name["namespace_uri"], name["local_name"]  # type: ignore[return-value]


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


def topology_path_for(element: etree._Element) -> list[dict[str, Any]]:
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


def independent_structural_resources(
    root: etree._Element,
) -> list[dict[str, Any]]:
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
                "expanded_name": expanded_name(element.tag),
                "locator": {
                    "preorder": preorder,
                    "depth": len(list(element.iterancestors())),
                    "parent_resource_id": parent_resource_id,
                    "element_child_position": child_position,
                    "same_name_sibling_position": same_name_position,
                    "path": topology_path_for(element),
                },
                "element_child_count": len(element_children(element)),
                "attribute_count": len(element.attrib),
                "attribute_expanded_names": attribute_names,
            }
        )
    return resources


def independent_ordered_topology_payload(
    resources: list[dict[str, Any]],
) -> list[dict[str, Any]]:
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


def independent_unordered_shape_payload(
    root: etree._Element,
) -> list[dict[str, Any]]:
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
    return sorted(shapes, key=lambda value: topology_digest_bytes(value))


def independent_topology_summary(source: bytes) -> dict[str, Any]:
    root = strict_parse(source)
    resources = independent_structural_resources(root)
    return {
        "ordered_topology_sha256": topology_digest_object(
            independent_ordered_topology_payload(resources)
        ),
        "unordered_element_shape_sha256": topology_digest_object(
            independent_unordered_shape_payload(root)
        ),
    }


def verify_topology_summary(payload: dict[str, Any], source: bytes) -> dict[str, Any]:
    expected = independent_topology_summary(source)
    actual = payload.get("summary", {})
    return {
        "ok": all(actual.get(key) == value for key, value in expected.items()),
        "expected": expected,
        "actual": {key: actual.get(key) for key in expected},
    }


def b_output(fixture_id: str, run_number: int = 1) -> dict[str, Any]:
    return load_json(
        LAB_DIR
        / "public-synthetic"
        / f"run-{run_number}"
        / "b"
        / f"{fixture_id}.json"
    )


def candidate_output(candidate: str, fixture_id: str, run_number: int = 1) -> dict[str, Any]:
    return load_json(
        LAB_DIR
        / "public-synthetic"
        / f"run-{run_number}"
        / candidate.lower()
        / f"{fixture_id}.json"
    )


def source_output(manifest: dict[str, Any], source_id: str, candidate: str, run_number: int = 1) -> dict[str, Any]:
    return load_json(
        Path(manifest["private_output_root"])
        / "outputs"
        / source_id
        / f"run-{run_number}"
        / f"{candidate.lower()}.json"
    )


def gate(gate_id: str, passed: bool, evidence: Any) -> dict[str, Any]:
    return {"gate_id": gate_id, "passed": bool(passed), "evidence": evidence}


def verify_frozen_files(freeze: dict[str, Any]) -> tuple[bool, list[str]]:
    mismatches: list[str] = []
    for record in freeze["frozen_files"]:
        path = REPO_ROOT / record["ref"]
        if not path.is_file() or sha256_path(path) != record["sha256"]:
            mismatches.append(record["ref"])
    return not mismatches, mismatches


def verify_active_method_bindings(freeze: dict[str, Any]) -> dict[str, Any]:
    frozen = {
        record.get("ref"): record.get("sha256")
        for record in freeze.get("frozen_files", [])
        if isinstance(record, dict)
    }
    missing: list[str] = []
    mismatches: list[str] = []
    for ref in METHOD_FILE_REFS:
        path = REPO_ROOT / ref
        expected = frozen.get(ref)
        if expected is None:
            missing.append(ref)
        elif not path.is_file() or sha256_path(path) != expected:
            mismatches.append(ref)
    return {
        "ok": not missing and not mismatches,
        "method_file_count": len(METHOD_FILE_REFS),
        "missing_frozen_method_files": sorted(missing),
        "active_method_digest_mismatches": sorted(mismatches),
    }


def parse_utc_timestamp(value: Any) -> datetime:
    if not isinstance(value, str):
        raise ValueError("timestamp is not a string")
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise ValueError("timestamp has no timezone")
    return parsed.astimezone(timezone.utc)


def verify_observed_output_bindings(
    observations: dict[str, Any],
) -> dict[str, Any]:
    failures: list[str] = []
    seen: set[str] = set()
    for index, process in enumerate(observations["processes"]):
        scope = process.get("output_scope")
        ref = process.get("output_ref")
        if scope == "lab" and isinstance(ref, str):
            path = LAB_DIR / ref
        elif scope == "absolute" and isinstance(ref, str):
            path = Path(ref)
        else:
            failures.append(f"process-{index + 1}:invalid-output-reference")
            continue
        resolved = path.resolve()
        key = str(resolved)
        if key in seen:
            failures.append(f"process-{index + 1}:duplicate-output-reference")
        seen.add(key)
        if process.get("output_created") is True:
            if not path.is_file():
                failures.append(f"process-{index + 1}:missing-output")
                continue
            if process.get("output_sha256") != sha256_path(path):
                failures.append(f"process-{index + 1}:output-digest-mismatch")
            if process.get("output_bytes") != path.stat().st_size:
                failures.append(f"process-{index + 1}:output-size-mismatch")
        elif path.exists():
            failures.append(f"process-{index + 1}:unexpected-output")
    return {
        "ok": not failures,
        "checked_process_count": len(observations["processes"]),
        "failures": failures,
    }


def verify_durable_freeze_order(
    freeze: dict[str, Any], observations: dict[str, Any]
) -> tuple[bool, dict[str, Any]]:
    method_ref = freeze.get("method_freeze_ref")
    method_path = REPO_ROOT / method_ref if isinstance(method_ref, str) else Path("/")
    method_digest = sha256_path(method_path) if method_path.is_file() else None
    candidate_ref = freeze.get("candidate_output_freeze_ref")
    candidate_path = REPO_ROOT / candidate_ref if isinstance(candidate_ref, str) else Path("/")
    candidate_digest = sha256_path(candidate_path) if candidate_path.is_file() else None
    method_freeze_bindings = verify_active_method_bindings(
        load_json(method_path) if method_path.is_file() else {}
    )
    current_method_bindings = verify_active_method_bindings(freeze)
    try:
        method_time = parse_utc_timestamp(
            load_json(method_path)["frozen_at"]
        )
        output_time = parse_utc_timestamp(observations["output_run_started_at"])
        order_ok = method_time < output_time
    except (OSError, KeyError, TypeError, ValueError):
        method_time = None
        output_time = None
        order_ok = False
    digest_ok = (
        method_digest == freeze.get("method_freeze_sha256")
        and observations.get("method_freeze_ref") == method_ref
        and observations.get("method_freeze_sha256") == method_digest
    )
    candidate_digest_ok = candidate_digest == freeze.get("candidate_output_freeze_sha256")
    evidence = {
        "method_freeze_ref": method_ref,
        "method_freeze_sha256": method_digest,
        "method_freeze_digest_match": digest_ok,
        "method_frozen_at": method_time.isoformat().replace("+00:00", "Z") if method_time else None,
        "output_run_started_at": output_time.isoformat().replace("+00:00", "Z") if output_time else observations.get("output_run_started_at"),
        "durable_order": order_ok,
        "method_freeze_active_bindings": method_freeze_bindings,
        "current_freeze_active_bindings": current_method_bindings,
        "candidate_output_freeze_sha256": candidate_digest,
        "candidate_output_freeze_digest_match": candidate_digest_ok,
    }
    return (
        order_ok
        and digest_ok
        and candidate_digest_ok
        and method_freeze_bindings["ok"]
        and current_method_bindings["ok"]
    ), evidence


def _frozen_digest(freeze: dict[str, Any], ref: str) -> str | None:
    for record in freeze.get("frozen_files", []):
        if isinstance(record, dict) and record.get("ref") == ref:
            digest = record.get("sha256")
            return digest if isinstance(digest, str) else None
    return None


def verify_manual_review_binding(freeze: dict[str, Any]) -> dict[str, Any]:
    manual_ref = MANUAL_PATH.relative_to(REPO_ROOT).as_posix()
    method_ref = freeze.get("method_freeze_ref")
    method_path = REPO_ROOT / method_ref if isinstance(method_ref, str) else Path("/")
    failures: list[str] = []
    current_digest = sha256_path(MANUAL_PATH) if MANUAL_PATH.is_file() else None
    post_digest = _frozen_digest(freeze, manual_ref)
    method_freeze: dict[str, Any] = {}
    if method_path.is_file():
        try:
            method_freeze = load_json(method_path)
        except (OSError, json.JSONDecodeError):
            failures.append("method-freeze-unreadable")
    else:
        failures.append("method-freeze-missing")
    method_digest = _frozen_digest(method_freeze, manual_ref)
    if current_digest is None:
        failures.append("manual-review-missing")
    if current_digest != post_digest:
        failures.append("manual-review-post-freeze-digest-mismatch")
    if current_digest != method_digest:
        failures.append("manual-review-method-freeze-digest-mismatch")
    if freeze.get("manual_review_completed_before_method_freeze") is not True:
        failures.append("manual-review-before-method-freeze-not-asserted")
    return {
        "ok": not failures,
        "manual_review_ref": manual_ref,
        "manual_review_sha256": current_digest,
        "post_freeze_sha256": post_digest,
        "method_freeze_sha256": method_digest,
        "manual_review_completed_before_method_freeze": freeze.get(
            "manual_review_completed_before_method_freeze"
        ),
        "failures": sorted(set(failures)),
    }


def verify_replay_binding(
    manifest: dict[str, Any], source_receipt: dict[str, Any]
) -> dict[str, Any]:
    record = manifest.get("exact_sources", {}).get("replay", {})
    receipt = source_receipt.get("sources", {}).get("replay", {})
    path_value = record.get("path")
    path = Path(path_value) if isinstance(path_value, str) else Path("/")
    failures: list[str] = []
    actual_digest = sha256_path(path) if path.is_file() else None
    actual_bytes = path.stat().st_size if path.is_file() else None
    expected_digest = receipt.get("source_sha256")
    expected_bytes = receipt.get("source_bytes")
    if not path.is_file():
        failures.append("replay-file-missing")
    if not isinstance(expected_digest, str):
        failures.append("replay-receipt-digest-missing")
    elif actual_digest != expected_digest:
        failures.append("replay-receipt-digest-mismatch")
    if not isinstance(expected_bytes, int):
        failures.append("replay-receipt-size-missing")
    elif actual_bytes != expected_bytes:
        failures.append("replay-receipt-size-mismatch")
    return {
        "ok": not failures,
        "path": path_value,
        "actual_sha256": actual_digest,
        "receipt_sha256": expected_digest,
        "actual_bytes": actual_bytes,
        "receipt_bytes": expected_bytes,
        "failures": sorted(set(failures)),
    }


def exact_source_values(source_path: Path) -> list[str]:
    root = strict_parse(source_path.read_bytes())
    values: set[str] = set()
    for node in root.iter():
        for value in (node.text, node.tail):
            if value:
                stripped = value.strip()
                if stripped and (HEBREW_PATTERN.search(stripped) or len(stripped) >= 3):
                    values.add(stripped)
        if isinstance(node.tag, str):
            for value in node.attrib.values():
                stripped = value.strip()
                if stripped and (HEBREW_PATTERN.search(stripped) or len(stripped) >= 3):
                    values.add(stripped)
    return sorted(values, key=lambda value: (-len(value), value))


def exact_source_values_from_manifest(manifest: dict[str, Any]) -> list[str]:
    values: set[str] = set()
    for source in manifest["exact_sources"].values():
        source_path = Path(source["path"])
        if source_path.is_file():
            values.update(exact_source_values(source_path))
    return sorted(values, key=lambda value: (-len(value), value))


def recursive_json_strings(value: Any) -> list[str]:
    strings: list[str] = []
    if isinstance(value, dict):
        for key, nested in value.items():
            strings.append(str(key))
            # Expanded XML names are structural labels, not source text. A
            # source value such as ``Tanach`` may legitimately be the local
            # name of a synthetic fixture element and must not be treated as
            # a copied source string.
            if key not in {"local_name", "namespace_uri"}:
                strings.extend(recursive_json_strings(nested))
    elif isinstance(value, list):
        for nested in value:
            strings.extend(recursive_json_strings(nested))
    elif isinstance(value, str):
        strings.append(value)
    return strings


def conservative_malformed_fixture_strings(xml_text: str) -> tuple[list[str], bool]:
    """Extract content without treating malformed markup names as source text."""
    strings: list[str] = []
    for pattern in (
        COMMENT_BODY_PATTERN,
        PROCESSING_INSTRUCTION_PATTERN,
        ATTRIBUTE_VALUE_PATTERN,
        TEXT_BETWEEN_TAGS_PATTERN,
    ):
        for match in pattern.finditer(xml_text):
            value = match.group("value") if "value" in match.groupdict() else match.group(1)
            if value and value.strip():
                strings.append(value.strip())
    trailing = xml_text.rsplit(">", 1)[-1]
    if trailing and trailing != xml_text and trailing.strip():
        strings.append(trailing.strip())
    extraction_incomplete = (
        xml_text.count("<") != xml_text.count(">")
        or any(xml_text.count(quote) % 2 for quote in ("'", '"'))
    )
    return strings, extraction_incomplete


def fixture_content_scan(manifest: dict[str, Any]) -> dict[str, Any]:
    """Return fixture content, including a conservative malformed-XML fallback."""
    strings: list[str] = []
    fallback_fixture_ids: list[str] = []
    incomplete_fixture_ids: list[str] = []
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
    for fixture in manifest["fixtures"] + manifest["security_fixtures"]:
        fixture_id = fixture.get("id", "<unknown>")
        try:
            root = etree.fromstring(fixture["xml"].encode("utf-8"), parser=parser)
        except (etree.XMLSyntaxError, UnicodeEncodeError):
            fallback_fixture_ids.append(fixture_id)
            fallback_strings, incomplete = conservative_malformed_fixture_strings(
                fixture.get("xml", "")
            )
            strings.extend(fallback_strings)
            if incomplete:
                incomplete_fixture_ids.append(fixture_id)
            continue
        for node in root.iter():
            for value in (node.text, node.tail):
                if value and value.strip():
                    strings.append(value.strip())
            if isinstance(node.tag, str):
                for value in node.attrib.values():
                    if value.strip():
                        strings.append(value.strip())
    return {
        "strings": strings,
        "fallback_fixture_ids": sorted(set(fallback_fixture_ids)),
        "incomplete_fixture_ids": sorted(set(incomplete_fixture_ids)),
    }


def fixture_content_strings(manifest: dict[str, Any]) -> list[str]:
    """Return only text/tail/attribute content from all fixture XML payloads."""
    return fixture_content_scan(manifest)["strings"]


def source_value_hits(
    source_values: list[str],
    strings: list[str],
    *,
    include_short_substrings: bool = True,
    include_delimited_short_substrings: bool = False,
) -> list[str]:
    def delimited_short_substring(candidate: str, value: str) -> bool:
        if len(value) >= 16:
            return False
        start = candidate.find(value)
        while start >= 0:
            end = start + len(value)
            left = candidate[start - 1] if start else None
            right = candidate[end] if end < len(candidate) else None
            left_is_boundary = left is None or left in "-/:."
            right_is_boundary = right is None or right in "-/:."
            if left_is_boundary and right_is_boundary and (left is not None or right is not None):
                return True
            start = candidate.find(value, start + 1)
        return False

    hits: list[str] = []
    for value in source_values:
        if any(
            value == candidate
            or (
                value in candidate
                and (
                    include_short_substrings
                    or len(value) >= 16
                    or (
                        include_delimited_short_substrings
                        and delimited_short_substring(candidate, value)
                    )
                )
            )
            for candidate in strings
        ):
            hits.append(value)
    return hits


def tracked_lab_files() -> list[Path]:
    lab_ref = LAB_DIR.relative_to(REPO_ROOT).as_posix()
    completed = subprocess.run(
        ["git", "-C", str(REPO_ROOT), "ls-files", "-z", "--", lab_ref],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError("git ls-files failed for the laboratory directory")

    paths: list[Path] = []
    lab_root = LAB_DIR.resolve()
    for raw_path in completed.stdout.split(b"\0"):
        if not raw_path:
            continue
        try:
            relative_path = Path(raw_path.decode("utf-8"))
        except UnicodeDecodeError as error:
            raise RuntimeError("git returned a non-UTF-8 laboratory path") from error
        path = (REPO_ROOT / relative_path).resolve()
        if not _is_within(path, lab_root):
            raise RuntimeError("git returned a path outside the laboratory directory")
        paths.append(path)
    return sorted(paths, key=str)


def scan_tracked_generated_for_source_values(
    source_values: list[str],
    manifest: dict[str, Any],
    *,
    excluded_paths: set[Path] | None = None,
) -> dict[str, Any]:
    try:
        tracked_paths = tracked_lab_files()
    except RuntimeError as error:
        return {
            "ok": False,
            "leaks": [],
            "scan_errors": [str(error)],
            "tracked_file_count": 0,
            "scanned_file_count": 0,
            "excluded_file_count": 0,
            "missing_tracked_files": [],
            "undecodable_tracked_files": [],
            "manifest_control_collision_count": 0,
            "manifest_control_collision_indexes": [],
            "scan_scope": "all Git-tracked files under the laboratory directory",
        }

    excluded = {
        path.resolve()
        for path in (excluded_paths or set())
    }
    leaks: list[str] = []
    scan_errors: list[str] = []
    missing_tracked_files: list[str] = []
    undecodable_tracked_files: list[str] = []
    manifest_control_collision_indexes: set[int] = set()
    scanned_file_count = 0
    for path in tracked_paths:
        if path.resolve() in excluded:
            continue
        relative_path = path.relative_to(LAB_DIR).as_posix()
        if not path.is_file():
            missing_tracked_files.append(relative_path)
            scan_errors.append(f"missing-tracked-file:{relative_path}")
            continue

        scanned_file_count += 1
        if path == INPUT_PATH:
            try:
                payload = load_json(path)
            except (OSError, json.JSONDecodeError):
                scan_errors.append(f"invalid-json:{relative_path}")
                continue
            strings = recursive_json_strings(payload)
            hits = source_value_hits(source_values, strings)
        elif path.suffix.lower() == ".json":
            try:
                payload = load_json(path)
            except (OSError, json.JSONDecodeError):
                scan_errors.append(f"invalid-json:{relative_path}")
                continue
            strings = recursive_json_strings(payload)
            hits = source_value_hits(source_values, strings)
        else:
            try:
                text = path.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                undecodable_tracked_files.append(relative_path)
                scan_errors.append(f"undecodable-tracked-file:{relative_path}")
                continue
            except OSError:
                scan_errors.append(f"unreadable-tracked-file:{relative_path}")
                continue
            # Authored Python/Markdown control surfaces also catch short values
            # when they are embedded as explicit path/token segments. Ordinary
            # prose and identifiers remain outside that narrow short-value rule.
            hits = source_value_hits(
                source_values,
                [text],
                include_short_substrings=False,
                include_delimited_short_substrings=True,
            )

        if path == INPUT_PATH:
            # The complete manifest is inspected.  Its declared control
            # metadata may intentionally repeat short source tokens (for
            # example a book code), while fixture/security payloads are the
            # untrusted publication surface and must remain source-free.
            control = {
                key: value
                for key, value in manifest.items()
                if key not in {"fixtures", "security_fixtures"}
            }
            control_hits = set(
                source_value_hits(source_values, recursive_json_strings(control))
            )
            # XML element names are structural labels, not source content.
            # Parse fixture payloads so a source value such as ``Tanach`` in a
            # synthetic tag does not become a false privacy hit, while text,
            # tails, attributes, comments, and processing instructions remain
            # covered by the content scan.
            fixture_scan = fixture_content_scan(manifest)
            untrusted_hits = source_value_hits(source_values, fixture_scan["strings"])
            scan_errors.extend(
                f"incomplete-fixture-content-extraction:{fixture_id}"
                for fixture_id in fixture_scan["incomplete_fixture_ids"]
            )
            manifest_control_collision_indexes.update(
                source_values.index(value) + 1 for value in control_hits
            )
            hits = untrusted_hits
        for value in hits:
            index = source_values.index(value) + 1
            leaks.append(f"{relative_path}:source-value-{index}")
    leaks = sorted(set(leaks))
    scan_errors = sorted(set(scan_errors))
    return {
        "ok": not leaks and not scan_errors,
        "leaks": leaks,
        "scan_errors": scan_errors,
        "tracked_file_count": len(tracked_paths),
        "scanned_file_count": scanned_file_count,
        "excluded_file_count": len(excluded & {path.resolve() for path in tracked_paths}),
        "missing_tracked_files": sorted(missing_tracked_files),
        "undecodable_tracked_files": sorted(undecodable_tracked_files),
        "manifest_control_collision_count": len(manifest_control_collision_indexes),
        "manifest_control_collision_indexes": sorted(manifest_control_collision_indexes),
        "scan_scope": "all Git-tracked files under the laboratory directory",
        "scanned_manifest": True,
    }


def verify_consumer_output_bindings(
    observations: dict[str, Any], consumer: dict[str, Any]
) -> dict[str, Any]:
    observed: dict[tuple[str, str, str], list[str]] = defaultdict(list)
    for process in observations["processes"]:
        if process.get("exit_code") == 0 and process.get("output_sha256"):
            key = (
                process["candidate"],
                process["selection_kind"],
                process["selection_id"],
            )
            observed[key].append(process["output_sha256"])
    observed_keys = set(observed)
    checks = consumer.get("checks", [])
    consumer_keys = [
        (check.get("candidate"), check.get("selection_kind"), check.get("selection_id"))
        for check in checks
    ]
    consumer_key_set = set(consumer_keys)
    duplicate_consumer_keys = {
        key for key, count in Counter(consumer_keys).items() if count > 1
    }

    def key_label(key: tuple[str, str, str]) -> str:
        return ":".join("" if part is None else str(part) for part in key)

    missing_keys = observed_keys - consumer_key_set
    unexpected_keys = consumer_key_set - observed_keys
    failures: list[str] = []
    matched = 0
    failures.extend(
        f"{key_label(key)}:missing-consumer-check"
        for key in missing_keys
    )
    failures.extend(
        f"{key_label(key)}:unexpected-consumer-check"
        for key in unexpected_keys
    )
    failures.extend(
        f"{key_label(key)}:duplicate-consumer-check"
        for key in duplicate_consumer_keys
    )
    for check, key in zip(checks, consumer_keys):
        hashes = observed.get(key, [])
        if not hashes:
            failures.append(f"{key_label(key)}:missing-observation")
        elif check.get("output_sha256") not in hashes:
            failures.append(f"{key_label(key)}:hash-mismatch")
        else:
            matched += 1
    return {
        "ok": not failures,
        "observed_unique_selection_count": len(observed_keys),
        "consumer_check_count": len(checks),
        "consumer_unique_selection_count": len(consumer_key_set),
        "missing_selection_keys": sorted(key_label(key) for key in missing_keys),
        "unexpected_selection_keys": sorted(key_label(key) for key in unexpected_keys),
        "duplicate_consumer_selection_keys": sorted(
            key_label(key) for key in duplicate_consumer_keys
        ),
        "matched_check_count": matched,
        "failures": sorted(set(failures)),
    }


def gitignored(path: Path) -> bool:
    completed = subprocess.run(
        ["git", "-C", "/srv/AbyssOS/Tree-of-Sophia", "check-ignore", "-q", str(path)],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return completed.returncode == 0


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def verify_private_output_posture(
    observations: dict[str, Any], manifest: dict[str, Any]
) -> dict[str, Any]:
    private_root = Path(manifest["private_output_root"]).resolve()
    source_processes = [
        process
        for process in observations["processes"]
        if process.get("selection_kind") == "source"
    ]
    private_files: set[Path] = set()
    if private_root.is_dir():
        private_files.update(
            path.resolve()
            for path in private_root.rglob("*")
            if path.is_file()
        )

    failures: list[str] = []
    root_ignored = gitignored(private_root)
    if not private_root.is_dir():
        failures.append("private-root-missing")
    elif not root_ignored:
        failures.append("private-root-not-ignored")

    observed_private_files: set[Path] = set()
    for index, process in enumerate(source_processes, start=1):
        if process.get("output_scope") != "absolute":
            failures.append(f"source-process-{index}:not-absolute")
            continue
        output_ref = process.get("output_ref")
        if not isinstance(output_ref, str):
            failures.append(f"source-process-{index}:missing-output-reference")
            continue
        output_path = Path(output_ref).resolve()
        if not _is_within(output_path, private_root):
            failures.append(f"source-process-{index}:outside-private-root")
            continue
        observed_private_files.add(output_path)
        if not output_path.is_file():
            failures.append(f"source-process-{index}:missing-output")

    private_files.update(observed_private_files)
    file_evidence: list[dict[str, Any]] = []
    for path in sorted(private_files, key=str):
        if not _is_within(path, private_root):
            failures.append(f"file-outside-private-root:{path}")
            continue
        if not path.is_file():
            failures.append(f"file-missing:{path}")
            continue
        mode = path.stat().st_mode & 0o777
        ignored = gitignored(path)
        file_evidence.append(
            {
                "path": str(path),
                "mode": f"{mode:04o}",
                "gitignored": ignored,
            }
        )
        if mode != PRIVATE_OUTPUT_MODE:
            failures.append(f"file-mode-{mode:04o}:{path}")
        if not ignored:
            failures.append(f"file-not-ignored:{path}")

    applicable = bool(source_processes or private_files or private_root.exists())
    return {
        "ok": applicable and not failures,
        "applicable": applicable,
        "private_root": str(private_root),
        "private_root_mode": (
            f"{private_root.stat().st_mode & 0o777:04o}"
            if private_root.is_dir()
            else None
        ),
        "private_root_gitignored": root_ignored,
        "checked_source_process_count": len(source_processes),
        "checked_private_file_count": len(file_evidence),
        "files": file_evidence,
        "failures": sorted(set(failures)),
    }


def recompute_provider_summary(payload: dict[str, Any]) -> dict[str, Any] | None:
    projection = payload.get("projection") if payload.get("candidate") == "BC" else payload
    if not isinstance(projection, dict) or not isinstance(projection.get("resources"), list):
        return None
    resources = projection["resources"]
    verse_records = [
        record
        for record in resources
        if isinstance(record, dict) and record.get("resource_kind") == "provider_verse"
    ]
    word_records = [
        record
        for record in resources
        if isinstance(record, dict) and record.get("resource_kind") == "provider_word"
    ]
    if any(
        not isinstance(record, dict)
        or not isinstance(record.get("provider_coordinate"), dict)
        for record in verse_records + word_records
    ):
        return None
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
    return {
        "resource_count": len(resources),
        "verse_count": len(verse_records),
        "word_count": len(word_records),
        "word_counts_by_verse": word_counts_by_verse,
    }


def verify_provider_projection_summary(payload: dict[str, Any]) -> dict[str, Any]:
    projection = payload.get("projection") if payload.get("candidate") == "BC" else payload
    actual = projection.get("summary", {}) if isinstance(projection, dict) else {}
    expected = recompute_provider_summary(payload)
    return {
        "ok": expected is not None and actual == expected,
        "expected": expected,
        "actual": (
            {key: actual.get(key) for key in ("resource_count", "verse_count", "word_count", "word_counts_by_verse")}
            if isinstance(actual, dict)
            else actual
        ),
    }


def verify_source_topology_receipt(
    manifest: dict[str, Any], source_receipt: dict[str, Any], source_id: str
) -> dict[str, Any]:
    source_path = Path(manifest["exact_sources"][source_id]["path"])
    receipt = source_receipt.get("sources", {}).get(source_id, {})
    receipt_candidates = receipt.get("candidates", {})
    source = source_path.read_bytes() if source_path.is_file() else b""
    b_payload = source_output(manifest, source_id, "B")
    bc_payload = source_output(manifest, source_id, "BC")
    b_check = verify_topology_summary(b_payload, source)
    bc_check = verify_topology_summary(bc_payload["owner"], source)
    expected = b_check["expected"]
    receipt_checks = {
        "B": {
            key: receipt_candidates.get("B", {}).get(key) == value
            for key, value in expected.items()
        },
        "BC": {
            key: receipt_candidates.get("BC", {}).get(
                "ordered_topology_sha256" if key == "ordered_topology_sha256" else "unordered_element_shape_sha256"
            ) == value
            for key, value in expected.items()
        },
    }
    return {
        "ok": b_check["ok"] and bc_check["ok"] and all(
            all(checks.values()) for checks in receipt_checks.values()
        ),
        "B": b_check,
        "BC_owner": bc_check,
        "receipt_matches": receipt_checks,
    }


def verify_source_provider_receipt(
    manifest: dict[str, Any], source_receipt: dict[str, Any], source_id: str
) -> dict[str, Any]:
    receipt = source_receipt.get("sources", {}).get(source_id, {})
    receipt_candidates = receipt.get("candidates", {})
    c_payload = source_output(manifest, source_id, "C")
    bc_payload = source_output(manifest, source_id, "BC")
    c_check = verify_provider_projection_summary(c_payload)
    bc_check = verify_provider_projection_summary(bc_payload)
    c_expected = c_check["expected"] or {}
    bc_expected = bc_check["expected"] or {}
    c_receipt = receipt_candidates.get("C", {})
    bc_receipt = receipt_candidates.get("BC", {})
    receipt_matches = {
        "C": {
            key: c_receipt.get(key) == value
            for key, value in c_expected.items()
        },
        "BC": {
            "projection_resource_count": bc_receipt.get("projection_resource_count") == bc_expected.get("resource_count"),
            "verse_count": bc_receipt.get("verse_count") == bc_expected.get("verse_count"),
            "word_count": bc_receipt.get("word_count") == bc_expected.get("word_count"),
            "word_counts_by_verse": bc_receipt.get("word_counts_by_verse") == bc_expected.get("word_counts_by_verse"),
        },
    }
    return {
        "ok": c_check["ok"] and bc_check["ok"] and all(
            all(checks.values()) for checks in receipt_matches.values()
        ),
        "C": c_check,
        "BC": bc_check,
        "receipt_matches": receipt_matches,
    }


def non_authoritative_boundary_is_valid(value: Any) -> bool:
    if not isinstance(value, str) or not value.strip():
        return False
    clauses = re.split(r"[.;]", value.casefold())
    if not any(AUTHORITY_NEGATION_PATTERN.search(clause) for clause in clauses):
        return False
    for clause in clauses:
        if POSITIVE_AUTHORITY_PATTERN.search(clause) and not AUTHORITY_NEGATION_PATTERN.search(clause):
            return False
    return True


def verify_contract_controls(manifest: dict[str, Any]) -> dict[str, Any]:
    control = manifest.get("public_contract_control", {})
    failures: list[str] = []
    for key in ("schema_ref", "builder_ref"):
        ref = control.get(key)
        digest_key = f"{key.replace('_ref', '')}_sha256"
        path = REPO_ROOT / ref if isinstance(ref, str) else Path("/")
        if not path.is_file() or sha256_path(path) != control.get(digest_key):
            failures.append(f"{key}-digest-mismatch")

    no_fit_ref = control.get("no_fit_ref")
    no_fit_digest = control.get("no_fit_sha256")
    if no_fit_ref is None and no_fit_digest is None:
        no_fit_status = "not_declared_as_lab_gate"
    elif isinstance(no_fit_ref, str) and isinstance(no_fit_digest, str):
        no_fit_path = REPO_ROOT / no_fit_ref
        if not no_fit_path.is_file() or sha256_path(no_fit_path) != no_fit_digest:
            failures.append("no-fit-digest-mismatch")
        no_fit_status = "validated"
    else:
        failures.append("no-fit-control-incomplete")
        no_fit_status = "incomplete"
    return {
        "ok": not failures,
        "contract_hash_match": not any(
            failure.endswith("digest-mismatch")
            for failure in failures
            if not failure.startswith("no-fit")
        ),
        "no_fit_status": no_fit_status,
        "failures": sorted(set(failures)),
    }


def collect_recursive_keys(value: Any) -> list[str]:
    keys: list[str] = []
    if isinstance(value, dict):
        for key, nested in value.items():
            keys.append(key)
            keys.extend(collect_recursive_keys(nested))
    elif isinstance(value, list):
        for nested in value:
            keys.extend(collect_recursive_keys(nested))
    return keys


def c14n_change_summary(left_path: Path, right_path: Path) -> dict[str, Any]:
    left_root = strict_parse(left_path.read_bytes())
    right_root = strict_parse(right_path.read_bytes())
    left_elements = [element for element in left_root.iter() if isinstance(element.tag, str)]
    right_elements = [element for element in right_root.iter() if isinstance(element.tag, str)]
    if len(left_elements) != len(right_elements):
        return {
            "comparable_element_count": False,
            "changed_element_count": None,
            "changed_preorders": [],
            "provider_content_subtree_equal": False,
        }
    changed: list[int] = []
    for position, (left, right) in enumerate(zip(left_elements, right_elements), start=1):
        left_bytes = etree.tostring(left, method="c14n", exclusive=False, with_comments=False)
        right_bytes = etree.tostring(right, method="c14n", exclusive=False, with_comments=False)
        if left_bytes != right_bytes:
            changed.append(position)
    left_content = [
        element
        for element in left_root
        if isinstance(element.tag, str) and element.tag == "tanach"
    ]
    right_content = [
        element
        for element in right_root
        if isinstance(element.tag, str) and element.tag == "tanach"
    ]
    content_equal = False
    if len(left_content) == len(right_content) == 1:
        content_equal = etree.tostring(
            left_content[0], method="c14n", exclusive=False, with_comments=False
        ) == etree.tostring(
            right_content[0], method="c14n", exclusive=False, with_comments=False
        )
    return {
        "comparable_element_count": True,
        "changed_element_count": len(changed),
        "changed_preorders": changed,
        "provider_content_subtree_equal": content_equal,
        "content_hashes_published": False,
    }


def main() -> int:
    manifest = load_json(INPUT_PATH)
    sealed = load_json(SEALED_PATH)
    freeze = load_json(FREEZE_PATH)
    observations = load_json(OBSERVATIONS_PATH)
    source_receipt = load_json(SOURCE_RECEIPT_PATH)
    consumer = load_json(CONSUMER_PATH)
    gates: list[dict[str, Any]] = []

    durable_freeze_ok, durable_freeze_evidence = verify_durable_freeze_order(freeze, observations)
    gates.append(
        gate(
            "G1-freeze-before-output",
            durable_freeze_ok,
            durable_freeze_evidence,
        )
    )

    frozen_ok, frozen_mismatches = verify_frozen_files(freeze)
    gates.append(
        gate(
            "G2-input-digests-match",
            frozen_ok,
            {"mismatches": frozen_mismatches, "frozen_file_count": len(freeze["frozen_files"])},
        )
    )

    selected_path = Path(manifest["exact_sources"]["selected"]["path"])
    source_values = exact_source_values_from_manifest(manifest)
    fixture_payload = {
        "fixtures": manifest["fixtures"],
        "security_fixtures": manifest["security_fixtures"],
    }
    fixture_text = json.dumps(fixture_payload, ensure_ascii=False)
    fixture_scan = fixture_content_scan(manifest)
    fixture_content = fixture_scan["strings"]
    fixture_leaks = [
        f"source-value-{index + 1}"
        for index, value in enumerate(source_values)
        if value in fixture_content
    ]
    gates.append(
        gate(
            "G3-synthetic-fixtures-source-free",
            not fixture_leaks
            and not fixture_scan["incomplete_fixture_ids"]
            and HEBREW_PATTERN.search(fixture_text) is None,
            {
                "source_value_matches": fixture_leaks,
                "hebrew_codepoints_present": HEBREW_PATTERN.search(fixture_text) is not None,
                "malformed_fixture_fallback_ids": fixture_scan["fallback_fixture_ids"],
                "incomplete_fixture_content_ids": fixture_scan["incomplete_fixture_ids"],
            },
        )
    )

    selected_record = manifest["exact_sources"]["selected"]
    private_output_posture = verify_private_output_posture(observations, manifest)
    g4 = (
        sha256_path(selected_path) == selected_record["sha256"]
        and selected_path.stat().st_size == selected_record["byte_size"]
        and f"{selected_path.stat().st_mode & 0o777:04o}" == selected_record["expected_mode"]
        and gitignored(selected_path)
        and private_output_posture["ok"]
    )
    gates.append(
        gate(
            "G4-local-source-fixity-mode-ignore",
            g4,
            {
                "sha256_match": sha256_path(selected_path) == selected_record["sha256"],
                "byte_size_match": selected_path.stat().st_size == selected_record["byte_size"],
                "mode": f"{selected_path.stat().st_mode & 0o777:04o}",
                "gitignored": gitignored(selected_path),
                "private_output_posture": private_output_posture,
            },
        )
    )

    b_p1 = b_output("P1-no-namespace")
    gates.append(
        gate(
            "G5-parser-posture-explicit",
            all(b_p1["parser_posture"].get(key) == value for key, value in manifest["parser_posture"].items()),
            b_p1["parser_posture"],
        )
    )

    security_ids = {fixture["id"] for fixture in manifest["security_fixtures"]}
    security_processes = [
        process
        for process in observations["processes"]
        if process["selection_id"] in security_ids
    ]
    g6 = len(security_processes) == 16 and all(
        process["exit_code"] == 2 and process["output_created"] is False
        for process in security_processes
    )
    gates.append(
        gate(
            "G6-security-negatives-no-output",
            g6,
            {
                "process_count": len(security_processes),
                "failed_closed_count": sum(
                    1
                    for process in security_processes
                    if process["exit_code"] == 2 and process["output_created"] is False
                ),
            },
        )
    )

    positive_processes = [
        process
        for process in observations["processes"]
        if process["exit_code"] == 0
    ]
    deterministic_pairs: dict[tuple[str, str, str], list[str]] = {}
    for process in positive_processes:
        key = (process["candidate"], process["selection_kind"], process["selection_id"])
        deterministic_pairs.setdefault(key, []).append(process["output_sha256"])
    pair_failures = [
        ":".join(key)
        for key, digests in deterministic_pairs.items()
        if len(digests) != 2 or len(set(digests)) != 1
    ]
    observed_output_binding = verify_observed_output_bindings(observations)
    consumer_output_binding = verify_consumer_output_bindings(observations, consumer)
    gates.append(
        gate(
            "G7-independent-build-byte-equality",
            not pair_failures and observed_output_binding["ok"] and consumer_output_binding["ok"],
            {
                "pair_count": len(deterministic_pairs),
                "failures": sorted(pair_failures),
                "observed_output_binding": observed_output_binding,
                "consumer_output_binding": consumer_output_binding,
            },
        )
    )

    gates.append(
        gate(
            "G8-b-paths-resolve-exactly",
            consumer["all_paths_and_metadata_return"] and consumer["error_count"] == 0 and consumer_output_binding["ok"],
            {"check_count": consumer["check_count"], "error_count": consumer["error_count"], "output_binding": consumer_output_binding},
        )
    )

    p2a = b_output("P2-prefix-a")
    p2b = b_output("P2-prefix-b")
    fixture_bytes_by_id = {
        fixture["id"]: fixture["xml"].encode("utf-8")
        for fixture in manifest["fixtures"]
    }
    p2a_topology = verify_topology_summary(p2a, fixture_bytes_by_id["P2-prefix-a"])
    p2b_topology = verify_topology_summary(p2b, fixture_bytes_by_id["P2-prefix-b"])
    g9 = (
        p2a_topology["ok"]
        and p2b_topology["ok"]
        and p2a["summary"]["ordered_topology_sha256"] == p2b["summary"]["ordered_topology_sha256"]
        and p2a["file_binding"]["sha256"] != p2b["file_binding"]["sha256"]
    )
    gates.append(
        gate(
            "G9-prefix-independent-expanded-names",
            g9,
            {
                "ordered_topology_equal": p2a["summary"]["ordered_topology_sha256"]
                == p2b["summary"]["ordered_topology_sha256"],
                "exact_file_equal": False,
                "p2a_summary_binding": p2a_topology,
                "p2b_summary_binding": p2b_topology,
            },
        )
    )

    p4 = b_output("P4-duplicate-siblings")
    p4_items = [resource for resource in p4["resources"] if resource["expanded_name"]["local_name"] == "item"]
    p5 = b_output("P5-interleaved-siblings")
    p5_children = [resource for resource in p5["resources"] if resource["locator"]["depth"] == 1]
    p5_items = [resource for resource in p5_children if resource["expanded_name"]["local_name"] == "item"]
    g10 = (
        [resource["locator"]["same_name_sibling_position"] for resource in p4_items] == [1, 2, 3]
        and [resource["locator"]["element_child_position"] for resource in p5_children] == [1, 2, 3]
        and [resource["locator"]["same_name_sibling_position"] for resource in p5_items] == [1, 2]
    )
    gates.append(gate("G10-duplicate-and-interleaved-order", g10, {"p4_same_name": [resource["locator"]["same_name_sibling_position"] for resource in p4_items], "p5_all_child": [resource["locator"]["element_child_position"] for resource in p5_children], "p5_item_same_name": [resource["locator"]["same_name_sibling_position"] for resource in p5_items]}))

    comparison_results: dict[str, Any] = {}
    g11 = True
    g12 = True
    for group in manifest["comparison_groups"]:
        left = b_output(group["left"])
        right = b_output(group["right"])
        left_topology = verify_topology_summary(left, fixture_bytes_by_id[group["left"]])
        right_topology = verify_topology_summary(right, fixture_bytes_by_id[group["right"]])
        exact_equal = left["file_binding"]["sha256"] == right["file_binding"]["sha256"]
        unordered_equal = left["summary"]["unordered_element_shape_sha256"] == right["summary"]["unordered_element_shape_sha256"]
        ordered_equal = left["summary"]["ordered_topology_sha256"] == right["summary"]["ordered_topology_sha256"]
        comparison_results[group["id"]] = {
            "exact_file_equal": exact_equal,
            "unordered_shape_equal": unordered_equal,
            "ordered_topology_equal": ordered_equal,
            "left_summary_binding": left_topology,
            "right_summary_binding": right_topology,
        }
        g11 = g11 and left_topology["ok"] and right_topology["ok"] and not exact_equal
        g12 = g12 and left_topology["ok"] and right_topology["ok"] and unordered_equal
    g12 = g12 and comparison_results["sibling-reorder"]["ordered_topology_equal"] is False
    gates.append(gate("G11-exact-file-change-preserved", g11, comparison_results))
    gates.append(gate("G12-no-topology-content-equivalence", g12, {"comparison_groups": comparison_results, "content_equality_claimed": False}))

    bc = candidate_output("BC", "PC1-uxlc-shape")
    all_projection_refs = all(record["generic_resource_ref"] for record in bc["projection"]["resources"])
    bc_summary_binding = verify_provider_projection_summary(bc)
    gates.append(gate("G13-projection-cites-b", all_projection_refs and bc_summary_binding["ok"], {"projection_count": len(bc["projection"]["resources"]), "cited_count": sum(1 for record in bc["projection"]["resources"] if record["generic_resource_ref"]), "summary_binding": bc_summary_binding}))

    c_positive = candidate_output("C", "PC1-uxlc-shape")
    c_summary_binding = verify_provider_projection_summary(c_positive)
    c_generic_negative = [process for process in observations["processes"] if process["candidate"] == "C" and process["selection_id"] == "P1-no-namespace" and process["selection_kind"] == "fixture" and process["exit_code"] != 0]
    expected_pc1_count = sealed["positive_expectations"]["PC1-uxlc-shape"]["candidate_c_resource_count"]
    g14 = c_summary_binding["ok"] and c_positive["summary"]["resource_count"] == expected_pc1_count and c_positive["generic_xml_owner_claimed"] is False and len(c_generic_negative) == 1 and c_generic_negative[0]["output_created"] is False
    gates.append(gate("G14-c-derived-pass-primary-generic-fail", g14, {"provider_shape_resources": c_positive["summary"]["resource_count"], "genericity_negative_count": len(c_generic_negative), "generic_owner_claimed": c_positive["generic_xml_owner_claimed"], "summary_binding": c_summary_binding}))

    a_p1 = candidate_output("A", "P1-no-namespace")
    gates.append(gate("G15-a-capture-pass-element-return-fail", len(a_p1["resources"]) == 1 and a_p1["element_return_supported"] is False, {"resource_count": len(a_p1["resources"]), "element_return_supported": a_p1["element_return_supported"]}))

    leak_scan = scan_tracked_generated_for_source_values(
        source_values,
        manifest,
        excluded_paths={RESULT_PATH},
    )
    gates.append(
        gate(
            "G16-no-source-values-in-tracked-output",
            leak_scan["ok"],
            {"source_value_control_count": len(source_values), **leak_scan},
        )
    )

    source_b = source_output(manifest, "selected", "B")
    source_bc = source_output(manifest, "selected", "BC")
    forbidden_resource_keys = {"content_fingerprint", "label_fingerprint", "content_sha256", "text_sha256"}
    resource_keys = set(collect_recursive_keys(source_b["resources"])) | set(collect_recursive_keys(source_bc["owner"]["resources"]))
    g17 = not (forbidden_resource_keys & resource_keys) and source_b["element_content_fingerprints_included"] is False
    gates.append(gate("G17-no-source-element-content-fingerprints", g17, {"forbidden_keys_found": sorted(forbidden_resource_keys & resource_keys), "declared_included": source_b["element_content_fingerprints_included"]}))

    true_tei = b_output("P10-true-tei")
    lookalike = b_output("P11-tei-lookalike")
    g18 = true_tei["resources"][0]["expanded_name"]["namespace_uri"] == "http://www.tei-c.org/ns/1.0" and lookalike["resources"][1]["expanded_name"]["namespace_uri"] is None and not true_tei["tei_classification_claimed"] and not lookalike["tei_classification_claimed"]
    gates.append(gate("G18-no-tei-lookalike-classification", g18, {"true_tei_namespace_observed": True, "lookalike_namespace_is_null": True, "generic_owner_claims_tei": False}))

    source_c = source_output(manifest, "selected", "C")
    gates.append(gate("G19-no-intrinsic-or-accepted-word-id", source_c["intrinsic_or_cross_corpus_word_ids_claimed"] is False and source_c["accepted_structure_claimed"] is False, {"intrinsic_or_cross_corpus_word_ids_claimed": source_c["intrinsic_or_cross_corpus_word_ids_claimed"], "accepted_structure_claimed": source_c["accepted_structure_claimed"]}))

    control = manifest["public_contract_control"]
    item_dir = REPO_ROOT / "ToS/source-witnesses/works/digital-biblical-corpora/unicode-xml-leningrad-codex/expressions/he-uxlc-2-5-full-accents/editions/uxlc-2-5-build-27-6/items/tanach-us-server-xml-prov23-1-3-20260830t160419z"
    forbidden_admissions = [item_dir / name for name in ("item.json", "item.manifest.json", "resource-inventory.json")]
    contract_control = verify_contract_controls(manifest)
    g20 = contract_control["ok"] and not any(path.exists() for path in forbidden_admissions)
    gates.append(gate("G20-no-public-or-downstream-admission", g20, {**contract_control, "forbidden_admission_files_present": [path.name for path in forbidden_admissions if path.exists()]}))

    gates.append(gate("G21-independent-source-return", consumer["consumer_imports_builder"] is False and consumer["consumer_reads_sealed_manifest"] is False and consumer["all_paths_and_metadata_return"] and consumer_output_binding["ok"], {"consumer_imports_builder": consumer["consumer_imports_builder"], "consumer_reads_sealed_manifest": consumer["consumer_reads_sealed_manifest"], "checks": consumer["check_count"], "errors": consumer["error_count"], "output_binding": consumer_output_binding}))

    correction: dict[str, Any]
    if "replay" in source_receipt["sources"]:
        selected = source_receipt["sources"]["selected"]
        replay = source_receipt["sources"]["replay"]
        replay_binding = verify_replay_binding(manifest, source_receipt)
        selected_topology_receipt = verify_source_topology_receipt(
            manifest, source_receipt, "selected"
        )
        replay_topology_receipt = verify_source_topology_receipt(
            manifest, source_receipt, "replay"
        )
        selected_provider_receipt = verify_source_provider_receipt(
            manifest, source_receipt, "selected"
        )
        replay_provider_receipt = verify_source_provider_receipt(
            manifest, source_receipt, "replay"
        )
        exact_equal = selected["source_sha256"] == replay["source_sha256"]
        topology_equal = (
            selected_topology_receipt["B"]["expected"]
            == replay_topology_receipt["B"]["expected"]
        )
        provider_projection_equal = (
            selected_provider_receipt["C"]["expected"]
            == replay_provider_receipt["C"]["expected"]
        )
        correction = {
            "live_replay_available": True,
            "exact_file_equal": exact_equal,
            "generic_ordered_topology_equal": topology_equal,
            "provider_projection_shape_equal": provider_projection_equal,
            "replay_binding": replay_binding,
            "selected_topology_receipt": selected_topology_receipt,
            "replay_topology_receipt": replay_topology_receipt,
            "selected_provider_receipt": selected_provider_receipt,
            "replay_provider_receipt": replay_provider_receipt,
            "private_c14n_change_summary": c14n_change_summary(Path(manifest["exact_sources"]["selected"]["path"]), Path(manifest["exact_sources"]["replay"]["path"])),
            "acceptance_inferred": False,
        }
        g22 = (
            replay_binding["ok"]
            and exact_equal is False
            and selected_topology_receipt["ok"]
            and replay_topology_receipt["ok"]
            and selected_provider_receipt["ok"]
            and replay_provider_receipt["ok"]
            and topology_equal
            and provider_projection_equal
            and correction["private_c14n_change_summary"]["provider_content_subtree_equal"]
        )
    else:
        correction = {
            "live_replay_available": False,
            "live_replay_inconclusive": True,
            "synthetic_dynamic_envelope_exact_equal": comparison_results["dynamic-envelope"]["exact_file_equal"],
            "synthetic_dynamic_envelope_topology_equal": comparison_results["dynamic-envelope"]["ordered_topology_equal"],
            "prior_real_dynamic_capture_evidence_ref": "A04_UXLC_PROVERBS_23_1_3_EXACT_RESPONSE_AND_GENERIC_XML_NO_FIT_2026-08-30.md",
            "acceptance_inferred": False,
        }
        g22 = comparison_results["dynamic-envelope"]["exact_file_equal"] is False and comparison_results["dynamic-envelope"]["ordered_topology_equal"] is True
    gates.append(gate("G22-correction-layers-distinguished", g22, correction))

    manual_text = MANUAL_PATH.read_text(encoding="utf-8") if MANUAL_PATH.is_file() else ""
    manual_required_markers = ["Direct source reopen", "Representative B return", "Tracked-output source-value inspection", "Manual verdict before evaluator"]
    missing_manual_markers = [marker for marker in manual_required_markers if marker not in manual_text]
    manual_binding = verify_manual_review_binding(freeze)
    gates.append(gate("G23-manual-review-before-evaluator", MANUAL_PATH.is_file() and not missing_manual_markers and manual_binding["ok"], {"manual_review_sha256": sha256_path(MANUAL_PATH) if MANUAL_PATH.is_file() else None, "missing_markers": missing_manual_markers, "binding": manual_binding}))

    metrics_present = all(process["wall_seconds"] is not None and process["max_rss_kib"] is not None and (process["output_created"] is False or ("output_bytes" in process and "output_sha256" in process)) for process in observations["processes"])
    consumer_metrics_present = consumer.get("wall_seconds") is not None and consumer.get("max_rss_kib") is not None
    gates.append(gate("G24-separated-quality-cost-speed-metrics", metrics_present and consumer_metrics_present and source_receipt["direct_monetary_cost"] == 0, {"process_count": observations["process_count"], "wall_and_rss_measured": metrics_present, "independent_consumer_wall_and_rss_measured": consumer_metrics_present, "consumer_wall_seconds": consumer.get("wall_seconds"), "consumer_max_rss_kib": consumer.get("max_rss_kib"), "output_bytes_separate": True, "direct_monetary_cost": source_receipt["direct_monetary_cost"], "human_burden_in_manual_review": True}))

    authority_payloads = {
        "A": a_p1,
        "B": b_p1,
        "C": c_positive,
        "BC": bc,
        "BC-owner": bc.get("owner"),
        "BC-projection": bc.get("projection"),
        "source-receipt": source_receipt,
        "consumer": consumer,
    }
    authority_checks = {
        label: non_authoritative_boundary_is_valid(payload.get("authority_boundary"))
        if isinstance(payload, dict)
        else False
        for label, payload in authority_payloads.items()
    }
    authority_boundaries_present = all(authority_checks.values())
    gates.append(gate("G25-machine-green-no-authority", authority_boundaries_present, {"authority_boundaries_present": authority_boundaries_present, "authority_boundary_checks": authority_checks, "machine_result_is_acceptance": False}))

    gate_ids = [item["gate_id"] for item in gates]
    registered_gate_ids = sealed["hard_gates"]
    all_registered = gate_ids == registered_gate_ids
    all_pass = all(item["passed"] for item in gates) and all_registered
    result = {
        "schema_version": "tos_generic_xml_resource_inventory_lab_comparison_result_v1",
        "lab_id": manifest["lab_id"],
        "registered_gate_order_match": all_registered,
        "gate_count": len(gates),
        "passed_gate_count": sum(1 for item in gates if item["passed"]),
        "all_hard_gates_pass": all_pass,
        "gates": gates,
        "comparison_groups": comparison_results,
        "correction_replay": correction,
        "mechanical_preference": {
            "generic_owner": "B" if all_pass else None,
            "capture_view": "A" if all_pass else None,
            "provider_projection": "C over B" if all_pass else None,
            "candidate_c_as_generic_owner": "reject" if all_pass else "unresolved",
        },
        "public_contract_promoted": False,
        "uxlc_item_admitted": False,
        "source_text_accepted": False,
        "language_or_translation_reviewed": False,
        "semantic_or_graph_state_created": False,
        "publication_authorized": False,
        "authority_boundary": "mechanical candidate comparison after manual source/output review; no public contract, source-text, language, translation, semantic, graph, canon, rights or publication authority",
    }
    temporary = RESULT_PATH.with_name(RESULT_PATH.name + ".tmp")
    temporary.write_bytes(canonical_bytes(result))
    os.chmod(temporary, 0o644)
    temporary.replace(RESULT_PATH)

    # Materialize the result before the final privacy scan so G16 covers the
    # complete current tracked laboratory tree, including its own receipt.
    final_leak_scan = scan_tracked_generated_for_source_values(source_values, manifest)
    final_g16 = next(item for item in gates if item["gate_id"] == "G16-no-source-values-in-tracked-output")
    final_g16["passed"] = final_leak_scan["ok"]
    final_g16["evidence"] = {
        "source_value_control_count": len(source_values),
        **final_leak_scan,
    }
    all_pass = all(item["passed"] for item in gates) and all_registered
    result["passed_gate_count"] = sum(1 for item in gates if item["passed"])
    result["all_hard_gates_pass"] = all_pass
    result["mechanical_preference"] = {
        "generic_owner": "B" if all_pass else None,
        "capture_view": "A" if all_pass else None,
        "provider_projection": "C over B" if all_pass else None,
        "candidate_c_as_generic_owner": "reject" if all_pass else "unresolved",
    }
    temporary.write_bytes(canonical_bytes(result))
    os.chmod(temporary, 0o644)
    temporary.replace(RESULT_PATH)
    return 0 if all_pass else 1


if __name__ == "__main__":
    raise SystemExit(main())
