#!/usr/bin/env python3
"""Freeze metadata and source-record templates, without fetching corpus bodies."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import time
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
HERE = ROOT / "ToS/source-witnesses/discovery/registry-first-planting-2026-09-08"
REL = HERE.relative_to(ROOT).as_posix()
MORPH = "3d15126fb1ef74867fc1434be1942e837932691f"
ORAEC = "b83a0ee5fae27a40d4c0a2a9a8c9c2973d45e9cd"
BILARA = "d6d54741b7f2ddfeca82f02c3f95eb3990b4e351"
SC_DATA = "36c4fddac3ef3c0cda0ffc07760a04d39b8c2eae"
SC_LICENSE = "5c7470b58bcc0bb50887d3a6ed54f4687703f176"
TOPOLOGY_EVENT = "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31"
BRANCH_PREPARATION = "ToS/philosophy/source-planting-preparation/first-wave-20260908.json"
HEADERS = {"User-Agent": "Tree-of-Sophia-source-preparation", "Accept": "application/vnd.github+json"}
BY = "https://creativecommons.org/licenses/by/4.0/"
BYSA = "https://creativecommons.org/licenses/by-sa/4.0/"
CC0 = "https://creativecommons.org/publicdomain/zero/1.0/"


def stamp() -> str:
    return datetime.now(timezone.utc).isoformat()


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()


def write(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encode(value))


def snapshot(name: str, url: str, *, selected_paths: list[str] | None = None) -> tuple[bytes, dict]:
    """Retain a small metadata/license body, or selected Git-tree metadata only."""
    evidence = HERE / "evidence"
    evidence.mkdir(exist_ok=True)
    target = evidence / name
    receipt_path = evidence / (name + ".receipt.json")
    if target.exists() or receipt_path.exists():
        if not target.exists() or not receipt_path.exists():
            raise ValueError(f"incomplete evidence pair: {name}")
        body, receipt = target.read_bytes(), json.loads(receipt_path.read_text())
        if receipt["url"] != url or receipt["retained_sha256"] != digest(body):
            raise ValueError(f"evidence identity or digest changed: {name}")
        return body, receipt
    started, tick = stamp(), time.monotonic()
    with urlopen(Request(url, headers=HEADERS), timeout=40) as response:
        body = response.read(8_000_001)
        status, final_url = response.status, response.url
    if len(body) > 8_000_000:
        raise ValueError("metadata response exceeds the bounded eight-megabyte limit")
    ended, elapsed = stamp(), time.monotonic() - tick
    response_sha256 = digest(body)
    if selected_paths is not None:
        payload = json.loads(body)
        if payload.get("truncated"):
            raise ValueError("truncated upstream Git tree")
        rows = [row for row in payload["tree"] if row["path"] in selected_paths]
        if {row["path"] for row in rows} != set(selected_paths):
            raise ValueError("selected metadata paths absent from Git tree")
        body = encode({"sha": payload["sha"], "tree": rows, "truncated": False,
                       "selection_only": True, "complete_response_sha256": response_sha256})
    receipt = {"url": url, "final_url": final_url, "http_status": status,
               "started_at": started, "ended_at": ended, "elapsed_seconds": elapsed,
               "response_sha256": response_sha256, "retained_sha256": digest(body),
               "retained_byte_size": len(body), "corpus_payload_fetched": False,
               "retained_ref": f"{REL}/evidence/{name}"}
    target.write_bytes(body)
    write(receipt_path, receipt)
    return body, receipt


def raw(repo: str, pin: str, path: str) -> str:
    return f"https://raw.githubusercontent.com/{repo}/{pin}/{path}"


def registry(original: str) -> dict:
    dossier = original.rsplit("-R", 1)[0]
    corpus = "table-ii" if dossier.startswith("T2-") else "table-i"
    folder = "2t" if corpus == "table-ii" else ("1a/1" if dossier in {"A04", "A12"} else "1a/2")
    return {"corpus": corpus, "document_id": dossier, "original_source_id": original,
            "entry_id": f"tos-registry:{corpus}/{dossier}/registry/source-{digest(original.encode())[:24]}",
            "original_file": f"{folder}/ToS_Corpus_Rights_{dossier}_registry.xlsx",
            "sheet": "registry", "row": int(original.rsplit("R", 1)[1]) + 1}


def targets() -> list[dict]:
    result = []
    for slug, title, book, chapters in [("proverbs", "Proverbs", "Prov", 31),
                                        ("job", "Job", "Job", 42),
                                        ("ecclesiastes", "Ecclesiastes (Qohelet)", "Eccl", 12)]:
        result.append(dict(slug=slug, title=title, family="hebrew-bible", provider="morphhb",
            repository="openscriptures/morphhb", pin=MORPH, language="hbo", script="Hebr",
            expression="hbo-wlc-oshb", edition=f"oshb-{MORPH[:12]}", item="git-osis-xml",
            upstream_paths=[f"wlc/{book}.xml"], registry_sources=[registry("A12-R018")],
            branch_target_slug="qohelet" if slug == "ecclesiastes" else slug,
            coverage={"kind": "osis-book", "book": book, "chapter_count": chapters},
            version_description="Westminster Leningrad Codex digital transcription with Open Scriptures Hebrew Bible lemma and morphology annotation; immutable OSHB Git snapshot.",
            responsibility="Open Scriptures Hebrew Bible Project: modern transcription/annotation distribution, not ancient authorship.",
            limits=["Alternative WLC/OSHB edition for the named work need; not a copy of Sefaria or an equivalence claim.",
                    "Original XML bytes and Hebrew character order must be preserved; no NFC normalization.",
                    "Morphological analysis and lemma assignments are modern annotations, not accepted ToS analysis."]))
    for slug, title, number, region, original in [
        ("immortality-of-writers", "Immortality of Writers", 1016, "verso 2.5–3.11", "A04-R062"),
        ("be-a-scribe", "Be a Scribe!", 3091, "verso 3.11–4.6", "A04-R078")]:
        result.append(dict(slug=slug, title=title, family="egyptian-literature", provider="oraec",
            repository="oraec/corpus_raw_data", pin=ORAEC, language="egy", script="Latn",
            expression="egy-aed-tei-oraec", edition=f"oraec-{ORAEC[:12]}", item="git-annotated-json",
            translation_expression="de-aed-tei-oraec",
            upstream_paths=[f"oraec{number}.json"], registry_sources=[registry(original)], branch_target_slug=slug,
            coverage={"kind": "oraec-composition", "oraec_id": f"oraec{number}", "physical_carrier": "pChester Beatty IV / British Museum EA 10684", "carrier_region": region},
            version_description="ORAEC mixed scholarly JSON bundle containing Egyptian written forms, hieroglyphic representations, German translation/glosses and annotations, derived from AED-TEI and the January 2018 database export; immutable ORAEC Git snapshot. The Egyptian and German expressions are separately identified within one Item.",
            responsibility="Peter Dils is the item-specific responsible scholar. The upstream README preserves the complete contributor list for the exported corpus.",
            limits=[f"This version covers the {region} composition section of EA 10684, not the entire papyrus.",
                    "The two ORAEC selections are sections of one physical carrier, not two separate papyri.",
                    "Egyptian transcription/transliteration, annotations and any included glosses remain distinct supplied layers; no new translation is created.",
                    "A digital edition section does not establish complete critical reconstruction or ancient authorship."]))
    pali = [
        ("khuddakapatha", "Khuddakapāṭha", "kp", "kn/kp/", ["A21-R050"], 9),
        ("dhammapada", "Dhammapada", "dhp", "kn/dhp/", ["A18-R010", "A21-R046"], 26),
        ("sutta-nipata", "Suttanipāta", "snp", "kn/snp/", ["A18-R009", "A21-R047"], 73),
        ("udana", "Udāna", "ud", "kn/ud/", ["A18-R011", "A21-R048"], 80),
        ("itivuttaka", "Itivuttaka", "iti", "kn/iti/", ["A18-R012", "A21-R049"], 112),
        ("samannaphala-sutta", "Sāmaññaphala Sutta (DN 2)", "dn2", "dn/dn2_root-pli-ms.json", ["A18-R015"], 1),
        ("upali-sutta", "Upāli Sutta (MN 56)", "mn56", "mn/mn56_root-pli-ms.json", ["A18-R016"], 1),
        ("sammaditthi-sutta", "Sammādiṭṭhi Sutta (MN 9)", "mn9", "mn/mn9_root-pli-ms.json", ["T2-39-R113"], 1)]
    for slug, title, uid, path, sources, count in pali:
        result.append(dict(slug=slug, title=title, family="pali-canon", provider="bilara",
            repository="suttacentral/bilara-data", pin=BILARA, language="pli", script="Latn",
            expression="pli-mahasangiti-suttacentral", edition=f"bilara-{BILARA[:12]}", item="git-root-segment-json",
            upstream_prefix="root/pli/ms/sutta/" + path, registry_sources=[registry(value) for value in sources],
            branch_target_slug="suttanipata" if slug == "sutta-nipata" else slug,
            coverage={"kind": "bilara-root", "uid": uid, "file_count": count},
            version_description="SuttaCentral Mahāsaṅgīti-derived Pāli root text, Roman script, preserved in the pinned Bilara published snapshot. Root edition metadata reports Dhamma Society Fund, Bangkok, 2010; SuttaCentral adapted structure/markup and sometimes punctuation.",
            responsibility="The Dhamma Society prepared the Mahāsaṅgīti edition; Ven. Yuttadhammo preserved and supplied its XML; SuttaCentral maintains the supplied structure, markup and punctuation. No modern actor is asserted to be the ancient author.",
            limits=["Only the root/pli/ms source layer is selected; English Sujato translations, comments, variants, parallels and HTML are separate layers.",
                    "A supplied root text does not establish a universally original reading or ToS textual acceptance.",
                    "For MN 9 the registry describes a Sujato translation and also points to the cognate root; this selects the separate root version as an antecedent to the later commentarial branch." if uid == "mn9" else "Collection/file coverage is checked within this version, not as complete coverage of the Tipiṭaka or all recensions."]))
    return result


def capture_metadata() -> tuple[dict[str, list[dict]], list[dict]]:
    receipts = []
    files: dict[str, list[dict]] = {}
    requests = [
        ("morphhb-tree.json", f"https://api.github.com/repos/openscriptures/morphhb/git/trees/{MORPH}?recursive=1", ["wlc/Prov.xml", "wlc/Job.xml", "wlc/Eccl.xml"]),
        ("oraec-tree.json", f"https://api.github.com/repos/oraec/corpus_raw_data/git/trees/{ORAEC}", ["oraec1016.json", "oraec3091.json"]),
        ("bilara-kn-tree.json", "https://api.github.com/repos/suttacentral/bilara-data/git/trees/e2c11bd977c2768a322f14a06d721a6263a72d62?recursive=1", None),
        ("bilara-dn-tree.json", "https://api.github.com/repos/suttacentral/bilara-data/git/trees/985db01adf1ce6e3371159f5960892fe3691dca8", ["dn2_root-pli-ms.json"]),
        ("bilara-mn-tree.json", "https://api.github.com/repos/suttacentral/bilara-data/git/trees/f0e956618fd089176cb797125a68a8b9ea59a3b2", ["mn9_root-pli-ms.json", "mn56_root-pli-ms.json"]),
    ]
    for name, url, selected in requests:
        body, receipt = snapshot(name, url, selected_paths=selected)
        data = json.loads(body)
        if data.get("truncated"):
            raise ValueError("truncated upstream Git tree")
        receipts.append(receipt)
        files[name] = data["tree"]
    sources = [
        ("morphhb-LICENSE.md", raw("openscriptures/morphhb", MORPH, "LICENSE.md")),
        ("morphhb-README.md", raw("openscriptures/morphhb", MORPH, "README.md")),
        ("oraec-README.md", raw("oraec/corpus_raw_data", ORAEC, "README.md")),
        ("oraec1016-metadata.html", "https://oraec.github.io/corpus/oraec1016.html"),
        ("oraec3091-metadata.html", "https://oraec.github.io/corpus/oraec3091.html"),
        ("suttacentral-licensing.json", raw("suttacentral/suttacentral", SC_LICENSE, "client/localization/elements/licensing_en.json")),
        ("suttacentral-root-edition.json", raw("suttacentral/sc-data", SC_DATA, "misc/root_edition.json")),
        ("bilara-LICENSE.md", raw("suttacentral/bilara-data", BILARA, "LICENSE.md")),
    ]
    for name, url in sources:
        _, receipt = snapshot(name, url)
        receipts.append(receipt)
    return files, receipts


def build_manifest(files: dict[str, list[dict]], receipts: list[dict]) -> dict:
    all_targets = targets()
    for target in all_targets:
        provider = target["provider"]
        if provider == "bilara":
            prefix = target["upstream_prefix"]
            district = prefix.split("/")[4]
            source_rows = files[f"bilara-{district}-tree.json"]
            rows = [{**row, "path": f"root/pli/ms/sutta/{district}/{row['path']}"} for row in source_rows if row["type"] == "blob"]
            selected = [row for row in rows if row["path"].startswith(prefix) and row["path"].endswith(".json")]
            if len(selected) != target["coverage"]["file_count"]:
                raise ValueError(f"target file count mismatch: {target['slug']}")
        else:
            selected = [row for row in files[f"{provider}-tree.json"] if row["path"] in target["upstream_paths"]]
        target["files"] = [{"upstream_path": row["path"], "basename": Path(row["path"]).name,
            "byte_size": row["size"], "git_blob_sha1": row["sha"],
            "media_type": "application/xml" if row["path"].endswith(".xml") else "application/json",
            "url": raw(target["repository"], target["pin"], row["path"])} for row in sorted(selected, key=lambda row: row["path"])]
        if len({entry["basename"] for entry in target["files"]}) != len(target["files"]):
            raise ValueError("flattened payload basename collision")
        identity = f"{target['family']}.{target['slug']}"
        exp_identity = f"{identity}.{target['expression']}"
        ed_identity = f"{exp_identity}.{target['edition']}"
        target["ids"] = {"work": f"tos.work.{identity}", "expression": f"tos.expression.{exp_identity}",
                         "edition": f"tos.edition.{ed_identity}", "item": f"tos.item.{ed_identity}.{target['item']}"}
        work = f"ToS/source-witnesses/works/{target['family']}/{target['slug']}"
        expression = f"{work}/expressions/{target['expression']}"
        edition = f"{expression}/editions/{target['edition']}"
        item = f"{edition}/items/{target['item']}"
        target["paths"] = {"work": f"{work}/work.json", "expression": f"{expression}/expression.json",
                           "edition": f"{edition}/edition.json", "item": f"{item}/item.json", "item_root": item}
        if target.get("translation_expression"):
            target["ids"]["translation_expression"] = f"tos.expression.{identity}.{target['translation_expression']}"
            target["paths"]["translation_expression"] = f"{work}/expressions/{target['translation_expression']}/expression.json"
        target["byte_size"] = sum(entry["byte_size"] for entry in target["files"])
    return {"schema_version": "tos_registry_first_planting_preparation_v1", "status": "prepared-not-acquired",
            "source_registry_snapshot_ref": None, "branch_preparation_ref": BRANCH_PREPARATION,
            "provider_pins": {"morphhb": MORPH, "oraec": ORAEC, "bilara": BILARA, "suttacentral_metadata": SC_DATA, "suttacentral_license": SC_LICENSE},
            "metadata_observations": receipts, "targets": all_targets,
            "totals": {"works": len(all_targets), "payload_files": sum(len(t["files"]) for t in all_targets), "payload_bytes": sum(t["byte_size"] for t in all_targets)},
            "authority_boundary": "Preparation only. Registry leads, upstream metadata and reviewed access evidence do not establish payload custody, textual acceptance, branch planting, semantics or canon."}


def source_refs(target: dict) -> list[str]:
    refs = [f"{REL}/manifest.json", f"{REL}/SOURCE_AND_RIGHTS_REVIEW.md",
            f"https://github.com/{target['repository']}/tree/{target['pin']}"]
    if target["provider"] == "morphhb":
        refs.extend([f"{REL}/evidence/morphhb-LICENSE.md", f"{REL}/evidence/morphhb-README.md"])
    elif target["provider"] == "oraec":
        refs.extend([f"{REL}/evidence/oraec-README.md", f"{REL}/evidence/{target['coverage']['oraec_id']}-metadata.html"])
    else:
        refs.extend([f"{REL}/evidence/suttacentral-root-edition.json", f"{REL}/evidence/suttacentral-licensing.json"])
    return refs


def rights_record(target: dict, assessed_at: str) -> dict:
    provider, ids = target["provider"], target["ids"]
    rights_id = "tos.rights." + ids["item"].removeprefix("tos.item.")
    refs = source_refs(target)
    license_uri = {"morphhb": BY, "oraec": BYSA, "bilara": CC0}[provider]
    conditions = ["Retain source credit, license link and supplied copyright/license notices.",
                  "Identify changes in any future derivative; acquired source bytes remain unchanged.",
                  "Do not impose restrictions that prevent the licensed freedoms."] if provider != "bilara" else []
    if provider == "morphhb":
        conditions.append("Original work of the Open Scriptures Hebrew Bible available at https://github.com/openscriptures/morphhb")
    if provider == "oraec":
        conditions.extend(["Retain Peter Dils as the item-specific responsible scholar and the complete exported-corpus contributor list in the upstream README.",
                           "Any shared adapted licensed material must satisfy CC BY-SA 4.0 share-alike."])
    posture = "authorized" if provider == "bilara" else "authorized_with_conditions"
    derivative = "allowed" if provider == "bilara" else "allowed_with_conditions"
    permission = ["Acquire and retain the exact openly supplied files locally.",
                  "Process the licensed digital layers, keeping input bytes and versions traceable."]
    rationale = ("Model assessment of the exact provider's positive public-domain/license statements and the applicable Creative Commons instrument for local acquisition in Mexico. "
                 "This is a license-scope assessment, not an independent legal determination of ancient authorship, copyright term in every jurisdiction, or public-release approval. "
                 "The requested operation retains source payloads locally; broader reuse permissions remain recorded with their conditions.")
    layers = []
    def layer(name: str, role: str, scope: list[str], statement: str, *, ancient: bool = False) -> None:
        layers.append({"layer_id": f"{rights_id}.layer.{name}", "layer_role": role,
            "scope_refs": scope, "assessment_status": "public_domain_reviewed" if ancient else "licensed",
            "assessment_basis": "mixed" if ancient else "license", "rights_statement_uri": license_uri,
            "license_uri": None if ancient else license_uri, "jurisdictions_reviewed": ["MX"],
            "source_refs": refs + [license_uri], "rights_holder_refs": [] if ancient else [f"https://github.com/{target['repository']}"],
            "permissions": permission, "restrictions": [] if ancient else conditions,
            "redistribution_posture": "authorized" if ancient else posture,
            "derivative_posture": "allowed" if ancient else derivative,
            "server_processing_posture": "authorized" if ancient else posture,
            "term": {"calculation_status": "not_calculated" if ancient else "not_applicable",
                     "basis": "Provider public-domain declaration; no independent jurisdictional term calculation." if ancient else "Positive Creative Commons license/waiver; no term expiry is relied on for this use.",
                     "starts_on": None, "ends_on": None,
                     "uncertainty": "No additional jurisdiction-wide conclusion is asserted."},
            "uncertainty": "The supplier's scope and authority remain the evidence boundary; no human legal review is claimed.",
            "assessed_at": assessed_at, "review_status": "unreviewed", "rationale": statement})
    if provider == "morphhb":
        layer("wlc-text", "embedded_text", [ids["expression"]], "The pinned OSHB license explicitly distinguishes the public-domain Westminster Leningrad Codex text.", ancient=True)
        layer("oshb-annotation", "annotation", [ids["item"]], "The OSHB contribution, including lemma and morphology annotation, is covered by CC BY 4.0 and the project attribution requirement.")
        layer("digital-presentation", "edition_presentation", [ids["edition"], ids["item"]], "The exact OSIS distribution is the licensed OSHB version; no facsimile, third-party translation or Sefaria material is included.")
    elif provider == "oraec":
        layer("egyptian-written-form", "transliteration", [ids["expression"]], "The modern scholarly Egyptian written-form/transliteration layer belongs to the exact JSON export licensed CC BY-SA 4.0, even though the ancient composition is older.")
        layer("german-translation", "translation", [ids["translation_expression"]], "The supplied German sentence translation and lexical glosses are modern scholarly language layers covered by the exact export's CC BY-SA 4.0 statement. They are not classified as ancient Egyptian text.")
        layer("annotation", "annotation", [ids["item"]], "Morphology, lemma, and hieroglyphic representation annotations retain the shared export license and scholarly responsibility.")
        layer("digital-bundle", "edition_presentation", [ids["edition"], ids["item"]], "The single mixed JSON bundle is retained intact; no papyrus photograph or full critical reconstruction is acquired.")
    else:
        layer("pali-root", "embedded_text", [ids["expression"]], "SuttaCentral's official licensing statement explicitly declares the Buddhist original-language texts public domain.", ancient=True)
        layer("suttacentral-contribution", "editing", [ids["edition"], ids["item"]], "SuttaCentral's own structure/markup and punctuation contributions fall under its explicit CC0 dedication. The root-edition metadata records the Mahāsaṅgīti lineage separately.")
    layer("metadata", "metadata", [ids["item"]], "Metadata and attribution retained from the same exact licensed distribution; repository API transport metadata reports identifiers and fixity without conferring content rights.")
    return {"schema_version": "tos_rights_record_v1", "rights_id": rights_id,
        "scope_refs": [ids["item"]], "assessment_status": "licensed", "rights_statement_uri": license_uri,
        "license_uri": license_uri, "jurisdictions_reviewed": ["MX"], "source_refs": refs + [license_uri],
        "layer_assessments": layers, "permissions": permission, "restrictions": conditions,
        "visibility": "local_only", "redistribution_posture": posture, "derivative_posture": derivative,
        "assessed_by": {"maker_type": "model", "agent_ref": "model:codex"}, "assessed_at": assessed_at,
        "rationale": rationale, "review_status": "unreviewed", "review_refs": [f"{REL}/SOURCE_AND_RIGHTS_REVIEW.md"],
        "access_request_ref": None, "record_version": 1, "supersedes_rights_ref": None}


def prepare_package(target: dict, assessed_at: str, *, evidence_refs: list[str] | None = None, rights_assessment: dict | None = None) -> dict:
    ids, paths, title = target["ids"], target["paths"], target["title"]
    refs = source_refs(target) if evidence_refs is None else evidence_refs
    claim_ids = {"work_expression": f"tos.claim.topology.registry-20260908.{target['slug']}.work-expression",
                 "expression_edition": f"tos.claim.topology.registry-20260908.{target['slug']}.expression-edition",
                 "edition_item": f"tos.claim.topology.registry-20260908.{target['slug']}.edition-item"}
    if "translation_expression" in ids:
        claim_ids.update({"work_translation": f"tos.claim.topology.registry-20260908.{target['slug']}.work-translation",
                          "translation_edition": f"tos.claim.topology.registry-20260908.{target['slug']}.translation-edition"})
    def record(kind: str, label: str, **fields: object) -> dict:
        return {"schema_version": "tos_corpus_record_v1", "record_type": "expression" if kind == "translation_expression" else kind,
            "record_id": ids[kind], "preferred_label": label, "field_languages": {"preferred_label": {"language": "en", "script": "Latn"}, "notes": {"language": "en", "script": "Latn"}},
            "variant_labels": [], "identity_status": "provisional", "source_refs": refs,
            "external_identifiers": [], "same_as_posture": "no_equivalence_claim", **fields,
            "record_version": 1, "supersedes_ref": None}
    expression_refs = [ids["expression"]]
    work_claim_refs = [claim_ids["work_expression"]]
    if "translation_expression" in ids:
        expression_refs.append(ids["translation_expression"])
        work_claim_refs.append(claim_ids["work_translation"])
    records = {
        paths["work"]: record("work", title, expression_claim_refs=work_claim_refs,
            responsibility_claim_refs=[], notes="Provisional work identity for the exact selected source version. " + " ".join(target["limits"]) + " No ancient author attribution is inferred from the responsibility path or modern provider."),
        paths["expression"]: record("expression", f"{title} — {target['language']} source-language layer",
            work_ref=ids["work"], language=target["language"], expression_role="source_language",
            responsibility_claim_refs=[], embodiment_claim_refs=[claim_ids["expression_edition"]],
            notes=target["version_description"] + " " + target["responsibility"] +
                (" This identity denotes only the Egyptian written-form/transliteration component within the shared mixed JSON Item. German translation/glosses have a separate Expression; exact source fields are recorded by post-acquisition forensic inspection." if target["provider"] == "oraec" else " Supplied annotation or punctuation does not establish ToS source-text acceptance.")),
        paths["edition"]: record("edition", f"{title} — {target['provider']} Git snapshot {target['pin'][:12]}",
            embodies_expression_refs=expression_refs, publication_claim_refs=[], exemplar_claim_refs=[claim_ids["edition_item"]],
            edition_statement=f"Immutable {target['repository']} commit {target['pin']}; " + target["version_description"],
            notes="Digital snapshot identity, not an ancient publication date. " + target["responsibility"]),
        paths["item"]: record("item", f"{title} — exact local {target['item']} copy", item_manifest_ref=f"{paths['item_root']}/item.manifest.json",
            notes="The Item identifies one acquired immutable digital file set from the pinned provider version. Its individual Files have content-addressed SHA-256 identities. " + " ".join(target["limits"]))}
    if "translation_expression" in ids:
        records[paths["translation_expression"]] = record("translation_expression", f"{title} — supplied German translation and gloss layer",
            work_ref=ids["work"], language="de", expression_role="translation", responsibility_claim_refs=[],
            embodiment_claim_refs=[claim_ids["translation_edition"]],
            notes="Modern German sentence translations and lexical glosses supplied with the AED-TEI/ORAEC scholarly JSON bundle. " + target["responsibility"] + " This is a distinct component of the same exact Edition and Item, not a translation generated by ToS or an accepted translation. Its exact source field selectors are recorded after acquisition.")
    claims = []
    for key, predicate, left, right, filename in [
        ("work_expression", "has_expression", "work", "expression", "work-expression/work-expression-claims.jsonl"),
        ("expression_edition", "embodied_by", "expression", "edition", "expression-edition/expression-edition-claims.jsonl"),
        ("edition_item", "exemplified_by", "edition", "item", "edition-item/edition-item-claims.jsonl"),
        *(([("work_translation", "has_expression", "work", "translation_expression", "work-expression/work-expression-claims.jsonl"),
            ("translation_edition", "embodied_by", "translation_expression", "edition", "expression-edition/expression-edition-claims.jsonl")]) if "translation_expression" in ids else [])]:
        evidence_refs = [paths[left], paths[right]]
        if right == "item":
            evidence_refs.append(f"{paths['item_root']}/item.manifest.json")
        claims.append({"path": f"ToS/source-witnesses/relations/{filename}", "record": {
            "schema_version": "tos_claim_packet_v1", "claim_id": claim_ids[key], "claim_type": "bibliographic",
            "assertion_layer": "bibliographic_assertion", "subject_ref": ids[left], "predicate": predicate, "object": ids[right],
            "evidence_refs": evidence_refs, "maker": {"maker_type": "model", "agent_ref": "model:codex"},
            "provenance_event_ref": TOPOLOGY_EVENT, "epistemic_status": "observed", "review_status": "unreviewed",
            "reviews": [], "visibility": "public_metadata_only", "claim_version": 1, "supersedes_claim_ref": None}})
    item_root = paths["item_root"]
    manifest_fields = {"$schema": "https://tree-of-sophia.local/ToS/contracts/source-item-manifest.schema.json",
        "schema_version": "tos_source_item_manifest_v1", "item_id": ids["item"], "item_kind": "born_digital",
        "embodiment_ref": ids["edition"], "storage_posture": "local_gitignored_payload",
        "acquisition_event_ref": f"tos.event.acquisition.registry-20260908.{target['slug']}",
        "rights_ref": f"{item_root}/rights.json", "provenance_ref": f"{item_root}/provenance.jsonl",
        "forensic_report_ref": f"{item_root}/forensic-report.md", "resource_inventory_ref": f"{item_root}/resource-inventory.json",
        "source_record_refs": refs, "visibility": "local_only", "manifest_version": 1, "supersedes_manifest_ref": None}
    return {"target_slug": target["slug"], "status": "prepared-not-acquired", "records": records,
            "rights": rights_record(target, assessed_at) if rights_assessment is None else rights_assessment, "claims": claims, "manifest_fields": manifest_fields,
            "fields_completed_only_after_acquisition": ["payload_files", "File SHA-256", "acquisition time", "forensic structure observations", "resource inventory", "provenance events"]}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-registry-snapshot", help="Exact normalized snapshot path, when frozen")
    args = parser.parse_args()
    files, receipts = capture_metadata()
    manifest = build_manifest(files, receipts)
    if args.source_registry_snapshot:
        path = ROOT / args.source_registry_snapshot
        if not path.is_file():
            raise ValueError("source registry snapshot does not exist")
        manifest["source_registry_snapshot_ref"] = args.source_registry_snapshot
        manifest["source_registry_snapshot_sha256"] = digest(path.read_bytes())
    assessed_at = max(receipt["ended_at"] for receipt in receipts)
    packages = [prepare_package(target, assessed_at) for target in manifest["targets"]]
    package_bytes = b"".join((json.dumps(package, ensure_ascii=False, separators=(",", ":")) + "\n").encode() for package in packages)
    (HERE / "prepared-source-packages.jsonl").write_bytes(package_bytes)
    manifest["prepared_packages_ref"] = f"{REL}/prepared-source-packages.jsonl"
    manifest["prepared_packages_sha256"] = digest(package_bytes)
    manifest["totals"]["expressions"] = sum(2 if "translation_expression" in target["ids"] else 1 for target in manifest["targets"])
    manifest["totals"]["bibliographic_claims"] = sum(len(package["claims"]) for package in packages)
    write(HERE / "manifest.json", manifest)
    print(json.dumps(manifest["totals"], ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
