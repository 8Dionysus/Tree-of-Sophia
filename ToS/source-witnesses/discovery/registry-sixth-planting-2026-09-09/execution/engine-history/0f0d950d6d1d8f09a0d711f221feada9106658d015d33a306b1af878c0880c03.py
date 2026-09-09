#!/usr/bin/env python3
"""Acquire the reviewed registry file list into immutable local source Items."""
from __future__ import annotations

import argparse
import copy
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import time
from urllib.parse import urlparse
from urllib.request import Request, urlopen
import xml.etree.ElementTree as ET

from jsonschema import Draft202012Validator

from build_source_resource_inventories import build_inventory

ROOT = Path(__file__).resolve().parents[1]
SOURCE = "ToS/source-witnesses"
TOPOLOGY = f"{SOURCE}/relations/provenance.jsonl"
TOPOLOGY_EVENT = "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31"
ALLOWED_REPOSITORIES = {"openscriptures/morphhb", "oraec/corpus_raw_data", "suttacentral/bilara-data", "PerseusDL/canonical-greekLit", "PerseusDL/canonical-latinLit"}
NO_ACQUISITION = {"downloaded": False, "acquired_at": None, "byte_size": None, "sha256": None, "event_ref": None}


def utcnow() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def json_bytes(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(json_bytes(value))


def append_jsonl(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n")


def write_jsonl_record(path: Path, value: dict) -> None:
    """Write the owner's one-record JSONL batch without pretty-printing it."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")


def manifest_fixity(manifest: dict) -> str:
    return "".join(f"{entry['sha256']}  {entry['relative_path']}\n" for entry in manifest["payload_files"])


def rights_with_file_scopes(prepared_rights: dict, manifest: dict) -> dict:
    """Bind observed Files and keep explicitly public-domain layers distinct."""
    rights = copy.deepcopy(prepared_rights)
    rights["scope_refs"] = list(dict.fromkeys([*rights["scope_refs"], *(entry["file_id"] for entry in manifest["payload_files"])]))
    for layer in rights.get("layer_assessments", []):
        if layer.get("assessment_status") == "public_domain_reviewed":
            layer["rights_statement_uri"] = "https://creativecommons.org/publicdomain/mark/1.0/"
    return rights


def safe_path(root: Path, ref: str) -> Path:
    if not isinstance(ref, str) or not ref or "\\" in ref or "\0" in ref:
        raise ValueError("invalid repository path")
    parts = PurePosixPath(ref)
    if parts.is_absolute() or ".." in parts.parts or str(parts) != ref:
        raise ValueError(f"unsafe repository path: {ref}")
    path = root.joinpath(*parts.parts)
    for ancestor in (path, *path.parents):
        if ancestor == root.parent:
            break
        if ancestor.is_symlink():
            raise ValueError(f"symlink in source path: {ref}")
    path.resolve().relative_to(root.resolve())
    return path


def validate_json(value: dict, schema_name: str, root: Path) -> None:
    schema = json.loads((root / "ToS/contracts" / (schema_name + ".schema.json")).read_text())
    Draft202012Validator(schema).validate(value)


def check_file(body: bytes, entry: dict) -> str:
    if len(body) != entry["byte_size"]:
        raise ValueError(f"byte-size mismatch: {entry['basename']}")
    blob = hashlib.sha1(b"blob " + str(len(body)).encode() + b"\0" + body).hexdigest()
    if blob != entry["git_blob_sha1"]:
        raise ValueError(f"Git blob digest mismatch: {entry['basename']}")
    return sha256(body)


def strict_json(body: bytes) -> object:
    def pairs(items: list[tuple[str, object]]) -> dict:
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError(f"duplicate JSON key: {key}")
            result[key] = value
        return result
    def nonstandard(value: str) -> None:
        raise ValueError(f"nonstandard JSON numeric constant: {value}")
    return json.loads(body, object_pairs_hook=pairs, parse_constant=nonstandard)


def load_preparation(root: Path, path: Path, *, allow_unbound: bool = False) -> tuple[dict, dict[str, dict]]:
    manifest = json.loads(path.read_bytes())
    if manifest.get("schema_version") != "tos_registry_first_planting_preparation_v1" or manifest.get("status") != "prepared-not-acquired":
        raise ValueError("not a reviewed registry acquisition preparation")
    package_path = safe_path(root, manifest["prepared_packages_ref"])
    if sha256(package_path.read_bytes()) != manifest["prepared_packages_sha256"]:
        raise ValueError("prepared source package digest mismatch")
    packages = {}
    for line in package_path.read_text().splitlines():
        package = json.loads(line)
        if package["target_slug"] in packages:
            raise ValueError("duplicate prepared target")
        packages[package["target_slug"]] = package
        for record in package["records"].values():
            validate_json(record, "corpus-record", root)
        validate_json(package["rights"], "rights-record", root)
        for claim in package["claims"]:
            validate_json(claim["record"], "claim-packet", root)
    snapshot_ref = manifest.get("source_registry_snapshot_ref")
    if not snapshot_ref and not allow_unbound:
        raise ValueError("normalized source-registry snapshot is not bound")
    if snapshot_ref:
        snapshot_path = safe_path(root, snapshot_ref)
        if sha256(snapshot_path.read_bytes()) != manifest["source_registry_snapshot_sha256"]:
            raise ValueError("normalized source-registry snapshot digest mismatch")
    for observation in manifest["metadata_observations"]:
        body = safe_path(root, observation["retained_ref"]).read_bytes()
        if len(body) != observation["retained_byte_size"] or sha256(body) != observation["retained_sha256"]:
            raise ValueError("upstream evidence snapshot fixity mismatch")
    target_slugs, all_destinations = set(), set()
    for target in manifest["targets"]:
        slug = target["slug"]
        if slug in target_slugs or not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", slug):
            raise ValueError("duplicate or invalid acquisition target")
        target_slugs.add(slug)
        if target["repository"] not in ALLOWED_REPOSITORIES or not re.fullmatch(r"[a-f0-9]{40}", target["pin"]):
            raise ValueError("unapproved repository or unpinned version")
        package = packages[slug]
        operation_date(target)
        work_slug = target.get("work_slug", slug)
        if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", work_slug):
            raise ValueError("invalid existing Work path identity")
        validate_work_extension(root, target, package)
        if target.get("metadata_evidence_refs") is not None:
            selected_metadata_observations(manifest, target)
        for ref in package["records"]:
            safe_path(root, ref)
            if not ref.startswith(f"{SOURCE}/works/{target['family']}/{work_slug}/"):
                raise ValueError("prepared source record leaves its Work territory")
        for entry in target["files"]:
            upstream = PurePosixPath(entry["upstream_path"])
            if upstream.is_absolute() or ".." in upstream.parts or str(upstream) != entry["upstream_path"]:
                raise ValueError("unsafe upstream file path")
            if entry["basename"] != upstream.name or not re.fullmatch(r"[a-f0-9]{40}", entry["git_blob_sha1"]):
                raise ValueError("invalid upstream file identity")
            expected = f"https://raw.githubusercontent.com/{target['repository']}/{target['pin']}/{upstream}"
            if entry["url"] != expected or not 0 < entry["byte_size"] <= 8_000_000:
                raise ValueError("unbound source URL or oversized source file")
            destination = f"{target['paths']['item_root']}/payload/{entry['basename']}"
            safe_path(root, destination)
            if destination in all_destinations:
                raise ValueError("duplicate payload destination")
            all_destinations.add(destination)
    if target_slugs != set(packages):
        raise ValueError("manifest and source package target sets differ")
    if manifest["totals"]["payload_files"] != len(all_destinations) or manifest["totals"]["payload_bytes"] != sum(t["byte_size"] for t in manifest["targets"]):
        raise ValueError("manifest total closure differs")
    return manifest, packages


def operation_date(target: dict) -> str:
    """Keep legacy operation identifiers stable while naming later intakes."""
    value = target.get("operation_date", "2026-09-08")
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", value):
        raise ValueError("invalid acquisition operation date")
    datetime.strptime(value, "%Y-%m-%d")
    return value


def validate_work_extension(root: Path, target: dict, package: dict) -> tuple[str, bytes] | None:
    """An existing Work may only gain the exact prepared Expression claims.

    The retained preimage binds its identity and every prior assertion. This is
    not a general source-record replacement mechanism.
    """
    binding = package.get("existing_work")
    if binding is None:
        return None
    work_ref = target["paths"]["work"]
    if binding.get("record_ref") != work_ref:
        raise ValueError("existing Work binding names another record")
    before = safe_path(root, binding["preimage_ref"]).read_bytes()
    if sha256(before) != binding["sha256"]:
        raise ValueError("existing Work preimage digest mismatch")
    old = json.loads(before)
    if old.get("record_type") != "work" or old.get("record_id") != target["ids"]["work"]:
        raise ValueError("existing Work identity differs from the prepared target")
    incoming = [claim["record"] for claim in package["claims"]
        if claim["record"].get("subject_ref") == old["record_id"]
        and claim["record"].get("predicate") == "has_expression"]
    new_expression_ids = {record["record_id"] for record in package["records"].values()
        if record.get("record_type") == "expression" and record.get("work_ref") == old["record_id"]}
    if not incoming or {claim["object"] for claim in incoming} != new_expression_ids:
        raise ValueError("Work extension does not close over its new Expressions")
    prior_refs = old.get("expression_claim_refs", [])
    additions = [claim["claim_id"] for claim in incoming]
    if len(set(prior_refs + additions)) != len(prior_refs) + len(additions):
        raise ValueError("Work extension repeats an existing or prepared claim")
    expected = copy.deepcopy(old)
    expected["expression_claim_refs"] = prior_refs + additions
    expected["record_version"] = old["record_version"] + 1
    if package["records"].get(work_ref) != expected:
        raise ValueError("Work extension changes fields outside additive Expression closure")
    return work_ref, before


def check_preparation_receipt(root: Path, manifest_path: Path, receipt_path: Path) -> dict:
    receipt = json.loads(receipt_path.read_bytes())
    required = ("manifest_sha256", "commit", "runtime_session_id", "checkpoint_review_ref", "passed_checks")
    if any(not receipt.get(field) for field in required):
        raise ValueError("preparation receipt lacks checkpoint evidence")
    if receipt["manifest_sha256"] != sha256(manifest_path.read_bytes()):
        raise ValueError("preparation receipt does not bind this manifest")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    if receipt["commit"] != commit:
        raise ValueError("preparation receipt does not bind the current repository commit")
    if not isinstance(receipt["passed_checks"], list) or any(not isinstance(item, str) or not item for item in receipt["passed_checks"]):
        raise ValueError("preparation receipt passed_checks must name the completed checks")
    if receipt.get("status", "passed") != "passed":
        raise ValueError("preparation checkpoint is not passed")
    return receipt


def transfer(root: Path, target: dict, entry: dict, log: Path) -> tuple[bytes, dict]:
    destination = safe_path(root, f"{target['paths']['item_root']}/payload/{entry['basename']}")
    relative = destination.relative_to(root).as_posix()
    ignored = subprocess.run(["git", "check-ignore", "--quiet", "--", relative], cwd=root).returncode == 0
    tracked = subprocess.run(["git", "ls-files", "--error-unmatch", "--", relative], cwd=root, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    if not ignored or tracked:
        raise ValueError("source bytes must use an untracked, ignored Item payload path")
    if destination.exists():
        body = destination.read_bytes()
        digest = check_file(body, entry)
        previous = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
        matching = [row for row in previous if row.get("status") == "completed" and row.get("destination_ref") == relative and row.get("sha256") == digest]
        if not matching:
            raise ValueError("existing matching payload lacks this operation's acquisition receipt")
        return body, matching[-1]
    started, tick = utcnow(), time.monotonic()
    row = {"target_slug": target["slug"], "url": entry["url"], "destination_ref": relative,
           "started_at": started, "expected_byte_size": entry["byte_size"], "expected_git_blob_sha1": entry["git_blob_sha1"]}
    append_jsonl(log, {**row, "status": "started"})
    try:
        request = Request(entry["url"], headers={"User-Agent": "Tree-of-Sophia-source-acquisition"})
        with urlopen(request, timeout=45) as response:
            if urlparse(response.url).hostname != "raw.githubusercontent.com":
                raise ValueError("source transfer redirected outside the pinned raw provider")
            body = response.read(entry["byte_size"] + 1)
            row["http_status"], row["final_url"] = response.status, response.url
        digest = check_file(body, entry)
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open("xb") as stream:
            stream.write(body)
        destination.chmod(0o444)
        row.update(status="completed", ended_at=utcnow(), elapsed_seconds=time.monotonic() - tick, byte_size=len(body), sha256=digest)
        append_jsonl(log, row)
        return body, row
    except Exception as error:
        append_jsonl(log, {**row, "status": "failed", "ended_at": utcnow(), "elapsed_seconds": time.monotonic() - tick,
                           "error_type": type(error).__name__, "error": str(error)})
        raise


def tei_division_addresses(edition: ET.Element, milestone_unit: str | None = None,
                           repeated_milestones: list[dict] | None = None) -> list[str]:
    """Qualify local division numbers by their source-supplied ancestors."""
    addresses: list[str] = []
    occurrences: dict[tuple[tuple[str, str], ...], int] = {}
    seen: set[tuple[tuple[str, str], ...]] = set()
    def visit(node: ET.Element, parents: tuple[tuple[str, str], ...]) -> None:
        for child in node:
            address = parents
            if child.tag == "{http://www.tei-c.org/ns/1.0}div":
                label = child.get("subtype") or child.get("type")
                number = child.get("n")
                if not label or not number:
                    raise ValueError("Perseus text division lacks a supplied type or number")
                address += ((label, number),)
                if address in seen:
                    raise ValueError("Perseus qualified division address is duplicated")
                seen.add(address)
                addresses.append(json.dumps(address, ensure_ascii=False, separators=(",", ":")))
            if milestone_unit and child.tag == "{http://www.tei-c.org/ns/1.0}milestone" and child.get("unit") == milestone_unit:
                number = child.get("n")
                if not number:
                    raise ValueError("Perseus citation milestone lacks a supplied number")
                marker = parents + ((milestone_unit, number),)
                # Milestones are physical markers, not unique passage IDs.
                # Preserve repeated supplied numbering with an occurrence suffix.
                occurrences[marker] = occurrences.get(marker, 0) + 1
                physical = marker + (("marker_occurrence", str(occurrences[marker])),)
                addresses.append(json.dumps(physical, ensure_ascii=False, separators=(",", ":")))
            visit(child, address)
    visit(edition, ())
    if repeated_milestones is not None:
        repeated_milestones.extend({"source_address": list(marker), "occurrences": count}
                                   for marker, count in occurrences.items() if count > 1)
    return addresses


def inspect_payloads(target: dict, bodies: list[tuple[dict, bytes]]) -> dict:
    report = {"target_slug": target["slug"], "file_count": len(bodies), "byte_size": sum(len(body) for _, body in bodies),
              "source_bytes_changed": False, "textual_acceptance": False, "files": []}
    coverage = target["coverage"]
    if coverage["kind"] in {"perseus-tei-work", "perseus-tei-translation", "perseus-tei-latin-work"}:
        translated = coverage["kind"] == "perseus-tei-translation"
        latin = coverage["kind"] == "perseus-tei-latin-work"
        if latin and (target.get("language") != "la" or target.get("expression_role", "source_language") != "source_language"):
            raise ValueError("Latin edition profile requires its explicit language and source role")
        if translated and (target.get("language") != "en" or target.get("expression_role") != "translation"):
            raise ValueError("English translation profile requires its explicit language and role")
        if len(bodies) != 1:
            raise ValueError("Perseus work requires exactly one prepared TEI file")
        entry, body = bodies[0]
        if "reviewed_body_prefix_sha256" in coverage:
            length = coverage.get("reviewed_body_prefix_bytes")
            if not isinstance(length, int) or not 0 < length <= len(body) or sha256(body[:length]) != coverage["reviewed_body_prefix_sha256"]:
                raise ValueError("source opening differs from the reviewed language evidence")
        marker = b"</teiHeader>"
        if marker not in body or sha256(body.split(marker, 1)[0] + marker) != coverage["header_prefix_sha256"]:
            raise ValueError("Perseus header differs from the reviewed metadata prefix")
        ns = "{http://www.tei-c.org/ns/1.0}"
        xml = ET.fromstring(body)
        text_body = xml.find(ns + "text/" + ns + "body")
        if xml.tag != ns + "TEI" or text_body is None:
            raise ValueError("Perseus file lacks the expected TEI body")
        editions = [node for node in text_body.iter(ns + "div") if node.get("type") in {"edition", "translation"}]
        expected_kind = "translation" if translated else "edition"
        allowed_languages = {"en", "eng"} if translated else {"la", "lat"} if latin else {"grc"}
        identity = editions[0].get("n") if len(editions) == 1 else None
        if latin or (translated and "identity_anchor" in coverage):
            anchor = coverage.get("identity_anchor")
            body_identity = text_body.get("{http://www.w3.org/XML/1998/namespace}base")
            if anchor not in {"edition_n", "body_xml_base"}:
                raise ValueError("Perseus identity anchor must name its reviewed XML carrier")
            if any(value is not None and value != coverage["cts_urn"] for value in (identity, body_identity)):
                raise ValueError("Perseus source identity carriers conflict")
            if anchor == "body_xml_base":
                identity = body_identity
        if len(editions) != 1 or editions[0].get("type") != expected_kind or identity != coverage["cts_urn"] or editions[0].get("{http://www.w3.org/XML/1998/namespace}lang") not in allowed_languages:
            raise ValueError("Perseus edition identity or source language differs")
        sections = [node.get("n") for node in editions[0].iter(ns + "div") if node.get("subtype") == "section"]
        text = "".join(editions[0].itertext())
        greek = sum("\u0370" <= char <= "\u03ff" or "\u1f00" <= char <= "\u1fff" for char in text)
        count = sum(char.isascii() and char.isalpha() for char in text) if translated or latin else greek
        count_field = "latin_letter_count" if translated or latin else "greek_character_count"
        if coverage.get("citation_scope") in {"hierarchical_divisions", "hierarchical_divisions_and_section_milestones"}:
            milestones = coverage["citation_scope"] == "hierarchical_divisions_and_section_milestones"
            repetitions: list[dict] = []
            addresses = tei_division_addresses(editions[0], "section" if milestones else None, repetitions)
            if not addresses or count < 1000:
                raise ValueError("Perseus qualified divisions or nonempty language-profile text check failed")
            report["files"].append({"basename": entry["basename"], "cts_urn": coverage["cts_urn"],
                "division_count": len(addresses), "first_division": addresses[0], "last_division": addresses[-1],
                "division_addresses_sha256": sha256(("\n".join(addresses)+"\n").encode()),
                "address_scope": ("source-supplied division chains and occurrence-qualified section markers; repeated numbering is retained, not corrected; no CTS service resolution asserted" if milestones else "source-supplied division type/number chain; no CTS service resolution asserted"),
                count_field: count})
            if milestones:
                report["files"][-1]["repeated_section_markers"] = repetitions
        else:
            if not sections or len(sections) != len(set(sections)) or count < 1000:
                raise ValueError("Perseus section identity or nonempty language-profile text check failed")
            report["files"].append({"basename": entry["basename"], "cts_urn": coverage["cts_urn"], "section_count": len(sections), "first_section": sections[0], "last_section": sections[-1], count_field: count})
        report["coverage_limit"] = "Complete pinned supplied file; no independent critical-edition or missing-passage judgment."
    elif coverage["kind"] == "osis-book":
        namespace = "{http://www.bibletechnologies.net/2003/OSIS/namespace}"
        entry, body = bodies[0]
        xml = ET.fromstring(body)
        if xml.tag != namespace + "osis":
            raise ValueError("source is not the expected OSIS XML")
        chapters = [node.get("osisID") for node in xml.iter(namespace + "chapter") if node.get("osisID")]
        expected = [f"{coverage['book']}.{number}" for number in range(1, coverage["chapter_count"] + 1)]
        verses = [node.get("osisID") for node in xml.iter(namespace + "verse") if node.get("osisID")]
        words = list(xml.iter(namespace + "w"))
        if chapters != expected or not verses or not words or not any((node.text or "").strip() for node in words):
            raise ValueError("OSIS source book/chapter/nonempty-word coverage mismatch")
        if any(not address.startswith(coverage["book"] + ".") for address in verses) or len(set(verses)) != len(verses):
            raise ValueError("OSIS verse identity is duplicated or belongs to another book")
        report["files"].append({"basename": entry["basename"], "chapter_ids": chapters, "verse_count": len(verses), "word_count": len(words), "first_verse": verses[0], "last_verse": verses[-1]})
    elif coverage["kind"] == "bilara-root":
        all_segment_ids = set()
        for entry, body in bodies:
            parsed = strict_json(body)
            if not isinstance(parsed, dict) or not parsed or any(not isinstance(value, str) for value in parsed.values()):
                raise ValueError("Bilara root is not a nonempty segment-to-string mapping")
            uid = entry["basename"].removesuffix("_root-pli-ms.json")
            valid_uids = {uid}
            range_match = re.fullmatch(r"dhp(\d+)-(\d+)", uid)
            if range_match:
                valid_uids.update(f"dhp{number}" for number in range(int(range_match[1]), int(range_match[2]) + 1))
            if not any(value.strip() for value in parsed.values()) or any(key.split(":", 1)[0] not in valid_uids or ":" not in key for key in parsed):
                raise ValueError("Bilara root segment identity or nonempty-text check failed")
            if all_segment_ids.intersection(parsed):
                raise ValueError("Bilara segment is duplicated across files")
            all_segment_ids.update(parsed)
            report["files"].append({"basename": entry["basename"], "uid": uid, "segment_count": len(parsed), "nonempty_segment_count": sum(bool(value.strip()) for value in parsed.values())})
        uid = coverage["uid"]
        uids = [row["uid"] for row in report["files"]]
        if len(bodies) != coverage["file_count"]:
            raise ValueError("Bilara exact file coverage failed")
        if uid in {"kp", "iti"}:
            expected = {f"{uid}{number}" for number in range(1, coverage["file_count"] + 1)}
            if set(uids) != expected:
                raise ValueError("Bilara consecutive work-unit coverage failed")
        elif uid == "dhp":
            verse_numbers = []
            for value in uids:
                match = re.fullmatch(r"dhp(\d+)-(\d+)", value)
                if not match:
                    raise ValueError("unrecognized Dhammapada range filename")
                verse_numbers.extend(range(int(match[1]), int(match[2]) + 1))
            if sorted(verse_numbers) != list(range(1, 424)):
                raise ValueError("Dhammapada file-range coverage is not exactly 1–423")
        elif uid == "ud":
            if set(uids) != {f"ud{vagga}.{number}" for vagga in range(1, 9) for number in range(1, 11)}:
                raise ValueError("Udāna file coverage is not exactly 8 × 10")
        elif uid == "snp":
            counts = [sum(value.startswith(f"snp{vagga}.") for value in uids) for vagga in range(1, 6)]
            if counts != [12, 14, 12, 16, 19]:
                raise ValueError("Suttanipāta five-vagga supplied file coverage differs")
        elif uids != [uid]:
            raise ValueError("single-sutta identity differs")
        report["unique_segment_count"] = len(all_segment_ids)
    elif coverage["kind"] == "oraec-composition":
        entry, body = bodies[0]
        parsed = strict_json(body)
        if not isinstance(parsed, (dict, list)) or not parsed:
            raise ValueError("ORAEC JSON bundle is empty or not structured")
        components = {"egy": {}, "de": {}}
        source_keys = {"writtenform", "transliteration", "transcription", "hiero", "hieroglyphs", "hieroglyphic"}
        translated_keys = {"translation", "germantranslation", "translationde", "gloss", "wordtranslation", "cotexttranslation"}
        def visit(value: object, pointer: str = "") -> None:
            if isinstance(value, dict):
                for key, child in value.items():
                    child_pointer = pointer + "/" + key.replace("~", "~0").replace("/", "~1")
                    normalized = re.sub(r"[^a-z]", "", key.lower())
                    language = "egy" if normalized in source_keys else "de" if normalized in translated_keys else None
                    if language:
                        def component_strings(part: object, at: str) -> None:
                            if isinstance(part, str) and part.strip():
                                pattern = re.sub(r"/\d+(?=/|$)", "/*", at)
                                components[language][pattern] = components[language].get(pattern, 0) + 1
                            elif isinstance(part, dict):
                                for name, nested in part.items():
                                    component_strings(nested, at + "/" + name.replace("~", "~0").replace("/", "~1"))
                            elif isinstance(part, list):
                                for index, nested in enumerate(part):
                                    component_strings(nested, at + "/" + str(index))
                        component_strings(child, child_pointer)
                    visit(child, child_pointer)
            elif isinstance(value, list):
                for index, child in enumerate(value):
                    visit(child, pointer + "/" + str(index))
        visit(parsed)
        if not all(components.values()):
            raise ValueError("ORAEC bundle requires observed Egyptian written-form and separate German translation/gloss fields; inspect format before source installation")
        report["files"].append({"basename": entry["basename"], "root_type": type(parsed).__name__, "root_member_count": len(parsed)})
        report["component_witnesses"] = [{"expression_ref": target["ids"]["expression" if language == "egy" else "translation_expression"],
            "language": language, "file_id": "tos.file.sha256." + sha256(body), "file_sha256": sha256(body),
            "selectors": [{"json_pointer_pattern": pointer, "nonempty_string_count": count} for pointer, count in sorted(patterns.items())],
            "selection_posture": "observed supplied fields; no extraction or text acceptance"} for language, patterns in components.items()]
    else:
        raise ValueError("unknown prepared coverage control")
    return report


def event(event_id: str, event_type: str, started: str, ended: str, inputs: list[dict], outputs: list[dict], *, name: str, configuration: dict, rights_ref: str, receipts: list[str]) -> dict:
    return {"schema_version": "tos_provenance_event_v1", "event_id": event_id, "event_type": event_type,
        "started_at": started, "ended_at": ended, "agent_refs": ["model:codex", "software:acquire-registry-sources"],
        "inputs": inputs, "outputs": outputs, "method": {"maker_type": "mixed", "name": name, "version": "1",
        "artifact_digest": sha256(Path(__file__).read_bytes()), "runtime": sys.version.split()[0], "configuration": configuration},
        "status": "completed_with_warnings", "warnings": ["Exact local custody and mechanical observations only; no textual acceptance, human review, semantics, canon or publication."],
        "receipt_refs": receipts, "rights_basis_ref": rights_ref, "event_version": 1, "supersedes_event_ref": None}


def install_target(root: Path, manifest_path: Path, preparation: dict, target: dict, package: dict) -> dict:
    evidence_root = manifest_path.parent
    log = evidence_root / "acquisition-transfers.jsonl"
    manifest_ref = manifest_path.relative_to(root).as_posix()
    package_ref = preparation["prepared_packages_ref"]
    item_root = safe_path(root, target["paths"]["item_root"])
    item_manifest_path = item_root / "item.manifest.json"
    if (item_root / "item.json").exists():
        return verify_target(root, target)
    extension = validate_work_extension(root, target, package)
    for ref in package["records"]:
        if safe_path(root, ref).exists():
            if extension is None or ref != extension[0] or safe_path(root, ref).read_bytes() != extension[1]:
                raise ValueError(f"refusing replacement of an existing source record: {ref}")
        elif extension is not None and ref == extension[0]:
            raise ValueError("the Work being extended is no longer present")
    started = utcnow()
    bodies, transfers = [], []
    for entry in target["files"]:
        body, receipt = transfer(root, target, entry, log)
        bodies.append((entry, body))
        transfers.append(receipt)
    observations = inspect_payloads(target, bodies)
    if extension is not None and safe_path(root, extension[0]).read_bytes() != extension[1]:
        raise ValueError("existing Work changed during acquisition; refusing to overwrite")
    ended = utcnow()
    records = copy.deepcopy(package["records"])
    component_ref = f"{target['paths']['item_root']}/component-witnesses.json"
    if observations.get("component_witnesses"):
        write_json(safe_path(root, component_ref), {"schema_version": "tos_observed_bundle_components_v1", "item_ref": target["ids"]["item"], "components": observations["component_witnesses"]})
        for key in ("expression", "translation_expression"):
            records[target["paths"][key]]["source_refs"].append(component_ref)
    for ref, record in records.items():
        if extension is not None and ref == extension[0]:
            continue
        if preparation["source_registry_snapshot_ref"] not in record["source_refs"]:
            record["source_refs"].append(preparation["source_registry_snapshot_ref"])
    item_manifest = copy.deepcopy(package["manifest_fields"])
    item_manifest["source_record_refs"].append(preparation["source_registry_snapshot_ref"])
    item_manifest["payload_files"] = [{"file_id": "tos.file.sha256." + sha256(body), "relative_path": "payload/" + entry["basename"],
        "original_basename": entry["basename"], "media_type": entry["media_type"], "byte_size": len(body), "sha256": sha256(body), "fixity_verified_at": ended} for entry, body in bodies]
    validate_json(item_manifest, "source-item-manifest", root)
    write_json(item_manifest_path, item_manifest)
    (item_root / "fixity.sha256").write_text(manifest_fixity(item_manifest), encoding="utf-8")
    rights_path = item_root / "rights.json"
    acquired_rights = rights_with_file_scopes(package["rights"], item_manifest)
    validate_json(acquired_rights, "rights-record", root)
    write_json(rights_path, acquired_rights)
    write_json(item_root / "forensic-observations.json", observations)
    inventory = build_inventory(repo_root=root, manifest_path=item_manifest_path, payload_source_root=root / SOURCE, event_date=ended[:10])
    if inventory is None:
        raise ValueError("resource inventory could not inspect every local payload")
    validate_json(inventory, "source-resource-inventory", root)
    write_json(item_root / "resource-inventory.json", inventory)
    (item_root / "forensic-report.md").write_text(f"# Exact local source intake — {target['title']}\n\nInspected: {ended}\n\n"
        f"Version: `{target['repository']}@{target['pin']}`.\n\n"
        f"The {len(bodies)} original files ({observations['byte_size']} bytes) match the prepared Git blob identities and sizes. "
        "The Item manifest records computed SHA-256. The files were opened and parsed locally; source bytes and code-point order were preserved.\n\n"
        "`forensic-observations.json` records the exact observed format and target-specific mechanical coverage. "
        "`resource-inventory.json` contains the owner's text-free enumeration."
        + ("`component-witnesses.json` identifies the separate Egyptian and German components in the same immutable JSON file.\n\n" if observations.get("component_witnesses") else "\n\n")
        + "\n".join("- " + limit for limit in target["limits"]) + "\n\n"
        + target["responsibility"] + "\n\n"
        + "Rights are positive, layer-specific provider/license assessments for local acquisition. Visibility remains local-only. No ancient authorship, philological accuracy, translation acceptance, semantic admission, canon or publication is established.\n", encoding="utf-8")
    for ref, record in records.items():
        validate_json(record, "corpus-record", root)
        write_json(safe_path(root, ref), record)
    inputs = [{"ref": manifest_ref, "role": "reviewed-exact-version-file-list", "sha256": sha256(manifest_path.read_bytes())},
              {"ref": package_ref, "role": "prepared-source-and-rights-records", "sha256": preparation["prepared_packages_sha256"]}]
    payload_outputs = [{"ref": entry["file_id"], "role": "immutable-acquired-source-file", "sha256": entry["sha256"]} for entry in item_manifest["payload_files"]]
    rights_ref = rights_path.relative_to(root).as_posix()
    receipts = [f"{target['paths']['item_root']}/forensic-report.md", log.relative_to(root).as_posix()]
    events = [event(item_manifest["acquisition_event_ref"], "acquisition", min(row["started_at"] for row in transfers), max(row["ended_at"] for row in transfers), inputs, payload_outputs,
        name="pinned-upstream-immutable-acquisition", configuration={"source_urls": [entry["url"] for entry in target["files"]], "byte_identity": "exact Git blob SHA-1 and local SHA-256", "source_bytes_changed": False}, rights_ref=rights_ref, receipts=receipts),
        event(f"tos.event.rights-assessment.registry-{operation_date(target).replace('-', '')}.{target['slug']}", "rights_assessment", package["rights"]["assessed_at"], ended, inputs,
            [{"ref": rights_ref, "role": "layer-separated-license-assessment", "sha256": sha256(rights_path.read_bytes())}], name="prepared-provider-license-scope-assessment",
            configuration={"jurisdictions_reviewed": ["MX"], "publication_authority": False, "human_legal_review_performed": False,
                "acquired_rights_transformations": ["Bind every computed File ID to the assessed Item scope.", "Use the Public Domain Mark URI for layers explicitly assessed from provider public-domain statements; retain the separate digital-object license."],
                "prepared_rights_template_unchanged": True, "file_scope_binding_performed_at": ended}, rights_ref=rights_ref, receipts=[rights_ref]),
        event(inventory["provenance_event_ref"], "forensic_inspection", started, ended, payload_outputs,
            [{"ref": item_manifest["resource_inventory_ref"], "role": "tracked_text_free_resource_inventory", "sha256": sha256((item_root / "resource-inventory.json").read_bytes())}],
            name="source-resource-inventory", configuration={"profiles": sorted({entry["profile"] for entry in inventory["files"]}), "source_text_included": False}, rights_ref=rights_ref, receipts=receipts)]
    if extension is not None:
        work_ref, before = extension
        events.append(event(f"tos.event.annotation.registry-{operation_date(target).replace('-', '')}.{target['slug']}.work-extension",
            "annotation", started, ended,
            [{"ref": package["existing_work"]["preimage_ref"], "role": "exact-prior-work-record", "sha256": sha256(before)}, *inputs],
            [{"ref": work_ref, "role": "existing-work-with-additive-expression-claim", "sha256": sha256(safe_path(root, work_ref).read_bytes())}],
            name="guarded-existing-work-expression-extension", configuration={"allowed_fields": ["expression_claim_refs", "record_version"],
                "prior_claims_preserved": True, "no_new_work_created": True}, rights_ref=rights_ref, receipts=receipts))
    for value in events:
        validate_json(value, "provenance-event", root)
    (item_root / "provenance.jsonl").write_text("".join(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n" for value in events), encoding="utf-8")
    for claim in package["claims"]:
        path = safe_path(root, claim["path"])
        existing = [json.loads(line) for line in path.read_text().splitlines()]
        matches = [value for value in existing if value["claim_id"] == claim["record"]["claim_id"]]
        if matches and matches != [claim["record"]]:
            raise ValueError("existing bibliographic claim differs from the prepared record")
        if not matches:
            append_jsonl(path, claim["record"])
    refresh_topology(root, evidence_root, ended)
    write_discovery(root, manifest_path, preparation, target, transfers, events[0])
    return verify_target(root, target)


def refresh_topology(root: Path, evidence_root: Path, ended: str) -> None:
    path = root / TOPOLOGY
    before = path.read_bytes()
    history = evidence_root / "topology-before" / (sha256(before) + ".json")
    if not history.exists():
        history.parent.mkdir(parents=True, exist_ok=True)
        history.write_bytes(before)
    batch = json.loads(before)
    if batch["event_id"] != TOPOLOGY_EVENT:
        raise ValueError("unexpected bibliographic topology owner event")
    mapping = [("work-expression/work-expression-claims.jsonl", "work_expression_claims_materialized", "unreviewed-work-expression-topology-claims"),
               ("expression-edition/expression-edition-claims.jsonl", "expression_edition_claims_materialized", "unreviewed-expression-edition-topology-claims"),
               ("edition-item/edition-item-claims.jsonl", "edition_item_claims_materialized", "unreviewed-edition-item-topology-claims")]
    required_inputs = {}
    outputs = []
    for relative, count_field, role in mapping:
        ref = f"{SOURCE}/relations/{relative}"
        data = (root / ref).read_bytes()
        claims = [json.loads(line) for line in data.decode().splitlines()]
        batch["method"]["configuration"][count_field] = len(claims)
        outputs.append({"ref": ref, "role": role, "sha256": sha256(data)})
        for claim in claims:
            for evidence_ref in claim["evidence_refs"]:
                evidence_path = safe_path(root, evidence_ref)
                kind = "item-embodiment-manifest" if evidence_path.name == "item.manifest.json" else evidence_path.stem + "-topology-record"
                required_inputs[evidence_ref] = {"ref": evidence_ref, "role": "declared-" + kind, "sha256": sha256(evidence_path.read_bytes())}
    batch["inputs"], batch["outputs"], batch["ended_at"] = [required_inputs[key] for key in sorted(required_inputs)], outputs, ended
    batch["event_version"] += 1
    write_jsonl_record(path, batch)


def selected_metadata_observations(preparation: dict, target: dict) -> list[dict]:
    refs = target.get("metadata_evidence_refs")
    if refs is None:
        return [value for value in preparation["metadata_observations"] if (target["provider"] in value["retained_ref"] or target["provider"] == "bilara" and "suttacentral" in value["retained_ref"])]
    if not refs or len(refs) != len(set(refs)):
        raise ValueError("target metadata evidence refs are empty or duplicated")
    matching = [value for value in preparation["metadata_observations"] if value["retained_ref"] in refs]
    if len(matching) != len(refs) or {value["retained_ref"] for value in matching} != set(refs):
        raise ValueError("target metadata evidence is not exactly bound by the preparation")
    return matching


def write_discovery(root: Path, manifest_path: Path, preparation: dict, target: dict, transfers: list[dict], acquisition: dict) -> None:
    slug = target["slug"]
    day = operation_date(target)
    run_ref = f"{SOURCE}/discovery/runs/registry-{slug}.{day}.v1.json"
    event_id = f"tos.event.discovery.registry-{day.replace('-', '')}.{slug}"
    matching = selected_metadata_observations(preparation, target)
    channels, selected = [], []
    for index, observation in enumerate(matching, 1):
        channels.append({"channel_id": f"channel-{slug}-metadata-{index}", "sequence": index, "channel_type": "specialized-scholarly-project", "role": "originating-record",
            "source_name": target["repository"] + " upstream metadata/license evidence", "endpoint_url": observation["url"], "interface_type": "api" if "api.github.com" in observation["url"] else "web", "interface_version": "pinned repository metadata or dated official page",
            "exact_query": "GET " + observation["url"], "queried_at": observation["started_at"], "elapsed_seconds": observation["elapsed_seconds"], "result_order_preserved": True,
            "results": [{"result_id": f"tos-discovery-result.registry-{slug}-metadata-{index}", "rank": 1, "title_as_displayed": Path(observation["retained_ref"]).name,
                "result_url": observation["url"], "originating_record_url": observation["url"], "identifiers": [], "available_formats": ["metadata/license evidence"],
                "declared_rights": {"statement": "Separate exact-provider statements are retained in the source rights record.", "scope": "unknown", "evidence_url": observation["url"], "tos_conclusion": "evidence-only-not-a-rights-conclusion"},
                "availability": "metadata-only", "machine_interface": "api" if "api.github.com" in observation["url"] else "html", "decision": "needs-reconciliation",
                "rationale": "The originating project is the first applicable source for this born-digital version. This retained metadata establishes only its stated identity/license evidence; no ancient authorship or accepted text follows.",
                "acquisition": NO_ACQUISITION, "snapshot": {"state": "captured", "format": "static-snapshot", "sha256": observation["retained_sha256"], "reason": observation["retained_ref"]}}]})
    for index, transfer_row in enumerate(transfers, len(channels) + 1):
        result_id = f"tos-discovery-result.registry-{slug}-file-{index}"
        selected.append(result_id)
        channels.append({"channel_id": f"channel-{slug}-file-{index}", "sequence": index, "channel_type": "specialized-scholarly-project", "role": "digital-object-record",
            "source_name": target["repository"], "endpoint_url": transfer_row["url"], "interface_type": "bulk", "interface_version": target["pin"], "exact_query": "GET " + transfer_row["url"],
            "queried_at": transfer_row["started_at"], "elapsed_seconds": transfer_row["elapsed_seconds"], "result_order_preserved": True,
            "results": [{"result_id": result_id, "rank": 1, "title_as_displayed": Path(transfer_row["destination_ref"]).name, "result_url": transfer_row["url"], "originating_record_url": f"https://github.com/{target['repository']}/tree/{target['pin']}",
                "identifiers": [{"scheme": "Git commit", "value": target["pin"]}, {"scheme": "Git blob SHA-1", "value": transfer_row["expected_git_blob_sha1"]}],
                "available_formats": sorted({entry["media_type"] for entry in target["files"]}), "declared_rights": {"statement": "Provider license evidence is assessed separately in the exact Item rights record.", "scope": "digital-object", "evidence_url": matching[-1]["url"], "tos_conclusion": "evidence-only-not-a-rights-conclusion"},
                "availability": "open-download", "machine_interface": "bulk-data", "decision": "select", "rationale": "Exact prepared version, size and Git blob matched; local SHA-256 and parsing/coverage were observed.",
                "acquisition": {"downloaded": True, "acquired_at": transfer_row["ended_at"], "byte_size": transfer_row["byte_size"], "sha256": transfer_row["sha256"], "event_ref": acquisition["event_id"]},
                "snapshot": {"state": "not-needed", "format": None, "sha256": None, "reason": "The immutable acquired File is separately retained in ignored local custody and described by its Item manifest."}}]})
    run = {"$schema": "https://tree-of-sophia.local/ToS/contracts/material-discovery-record.schema.json", "schema_version": "tos_material_discovery_record_v1",
        "discovery_id": f"tos.discovery.registry-{slug}.{day}.v1", "protocol_ref": f"{SOURCE}/discovery/DISCOVERY_PROTOCOL.md",
        "target": {"target_kind": "expression", "known_tos_refs": list(target["ids"].values()), "description": target["version_description"], "required_properties": target["limits"] + ["exact pinned provider version and immutable local file identity"], "acceptable_substitutions": [], "languages": [target["language"]] + (["de"] if target["provider"] == "oraec" else []), "formats": sorted({entry["media_type"] for entry in target["files"]}), "purpose_ref": manifest_path.relative_to(root).as_posix()},
        "channels": channels, "channel_comparison": [{"channel_id": channel["channel_id"], "completeness": "adequate", "metadata_precision": "strong", "rights_clarity": "adequate", "machine_interface_quality": "strong", "human_minutes": 0, "machine_seconds": channel["elapsed_seconds"], "notes": "Transport time is measured. No human time or human review is claimed. Completeness is limited to the frozen exact version; source/rights judgment remains separate."} for channel in channels],
        "selected_result_ids": selected, "rejected_result_ids": [], "rights_inference_from_availability_prohibited": True, "general_web_search_is_last_resort": True, "technical_access_bypass_used": False,
        "maker": {"maker_type": "mixed", "agent_ref": "model:codex"}, "started_at": min(channel["queried_at"] for channel in channels), "ended_at": acquisition["ended_at"], "status": "reconciled", "provenance_event_refs": [event_id, acquisition["event_id"]], "record_version": 1, "supersedes_discovery_ref": None}
    validate_json(run, "material-discovery-record", root)
    write_json(root / run_ref, run)
    provenance = event(event_id, "discovery", run["started_at"], run["ended_at"], [{"ref": manifest_path.relative_to(root).as_posix(), "role": "frozen-registry-version-target", "sha256": sha256(manifest_path.read_bytes())}],
        [{"ref": run_ref, "role": "ordered-source-discovery-and-acquisition-receipt", "sha256": sha256((root / run_ref).read_bytes())}], name="ordered-exact-provider-source-route",
        configuration={"general_web_search_performed": False, "earlier_research_queries_not_reconstructed": True, "originating_project_first_applicable": True}, rights_ref=f"{target['paths']['item_root']}/rights.json", receipts=[run_ref])
    append_jsonl(root / SOURCE / "discovery/provenance.jsonl", provenance)


def verify_target(root: Path, target: dict) -> dict:
    item_root = safe_path(root, target["paths"]["item_root"])
    manifest = json.loads((item_root / "item.manifest.json").read_bytes())
    validate_json(manifest, "source-item-manifest", root)
    if manifest["item_id"] != target["ids"]["item"] or len(manifest["payload_files"]) != len(target["files"]):
        raise ValueError("installed Item manifest identity differs")
    indexed = {entry["original_basename"]: entry for entry in manifest["payload_files"]}
    bodies = []
    for expected in target["files"]:
        body = safe_path(root, f"{target['paths']['item_root']}/payload/{expected['basename']}").read_bytes()
        digest = check_file(body, expected)
        entry = indexed[expected["basename"]]
        if entry["sha256"] != digest or entry["file_id"] != "tos.file.sha256." + digest or entry["byte_size"] != len(body) or entry["relative_path"] != "payload/" + expected["basename"]:
            raise ValueError("installed File identity differs")
        bodies.append((expected, body))
    inspect_payloads(target, bodies)
    for field in ("rights_ref", "provenance_ref", "forensic_report_ref", "resource_inventory_ref"):
        if not safe_path(root, manifest[field]).is_file():
            raise ValueError(f"installed Item companion missing: {field}")
    if (item_root / "fixity.sha256").read_text(encoding="utf-8") != manifest_fixity(manifest):
        raise ValueError("installed fixity companion differs from the exact manifest")
    rights = json.loads(safe_path(root, manifest["rights_ref"]).read_bytes())
    validate_json(rights, "rights-record", root)
    if not {manifest["item_id"], *(entry["file_id"] for entry in manifest["payload_files"])} <= set(rights["scope_refs"]):
        raise ValueError("installed rights scope does not cover the Item and every File")
    for key in target["ids"]:
        if not safe_path(root, target["paths"][key]).is_file():
            raise ValueError("installed corpus identity record missing")
    return {"target_slug": target["slug"], "status": "local-payload-and-owner-package-verified", "item_id": target["ids"]["item"], "item_manifest_ref": f"{target['paths']['item_root']}/item.manifest.json", "files": len(bodies), "bytes": sum(len(body) for _, body in bodies), "textual_acceptance": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["verify-preparation", "acquire", "verify-local"])
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--preparation-receipt", type=Path)
    parser.add_argument("--target", action="append", default=[])
    parser.add_argument("--allow-unbound-registry", action="store_true", help="Preparation inspection only; never accepted by acquire")
    args = parser.parse_args()
    manifest_path = args.manifest.resolve()
    manifest_path.relative_to(ROOT)
    preparation, packages = load_preparation(ROOT, manifest_path, allow_unbound=args.command == "verify-preparation" and args.allow_unbound_registry)
    targets = [target for target in preparation["targets"] if not args.target or target["slug"] in args.target]
    if args.target and {target["slug"] for target in targets} != set(args.target):
        raise ValueError("unknown requested acquisition target")
    if args.command == "verify-preparation":
        print(json.dumps({"status": "preparation-verified", "manifest_sha256": sha256(manifest_path.read_bytes()), "totals": preparation["totals"], "registry_bound": bool(preparation.get("source_registry_snapshot_ref")), "payloads_downloaded": False}))
        return 0
    if args.command == "acquire":
        if not args.preparation_receipt:
            raise ValueError("acquire requires the completed preparation checkpoint receipt")
        check_preparation_receipt(ROOT, manifest_path, args.preparation_receipt)
    for target in targets:
        result = install_target(ROOT, manifest_path, preparation, target, packages[target["slug"]]) if args.command == "acquire" else verify_target(ROOT, target)
        print(json.dumps(result, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, KeyError, OSError) as error:
        print(f"registry acquisition stopped: {error}", file=sys.stderr)
        raise SystemExit(1)
