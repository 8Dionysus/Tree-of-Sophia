#!/usr/bin/env python3
"""Build candidate-only DE/RU morphology families, themes, and relations."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import os
import re
import secrets
import stat
import subprocess
import sys
import tempfile
import unicodedata
from collections import Counter, defaultdict
from itertools import combinations
from pathlib import Path
from typing import Any, Iterable


REPO = Path(__file__).resolve().parents[1]
WORK = Path("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra")
ROUTE = Path("ToS/candidate-intake/zarathustra/dta-antonovsky-morphology-themes-v1")
PREVIOUS = WORK / "lexical-indexes/dta-antonovsky-parallel-candidates-v1"
PRIVATE_PREVIOUS = WORK / "gold-sets/foundation-pilot-v1/local-content/parallel-lexical-candidates-v1/parallel-candidate-analysis.v1.json"
PRIVATE_ROOT = WORK / "gold-sets/foundation-pilot-v1/local-content/morphology-themes-v1"
PRIVATE_ANALYSIS = PRIVATE_ROOT / "morphology-theme-analysis.v1.json"
PLAN_REF = ROUTE / "plan.v1.json"
ISSUANCE_REF = ROUTE / "identity-issuance.v1.json"
PREVIOUS_BUILDER = Path("scripts/build_zarathustra_parallel_lexical_candidates_v1.py")
GENERATOR_REF = Path("scripts/build_zarathustra_morphology_theme_candidates_v1.py")

OUTPUTS = {
    "families": ROUTE / "morphological-family-candidates.v1.jsonl",
    "relations": ROUTE / "typed-relation-candidates.v1.jsonl",
    "clusters": ROUTE / "thematic-cluster-candidates.v1.jsonl",
    "summary": ROUTE / "summary.v1.json",
    "coverage": ROUTE / "coverage-receipt.v1.json",
    "provenance": ROUTE / "provenance.jsonl",
    "manifest": ROUTE / "manifest.v1.json",
}

DE_SUFFIXES = (
    ("ern", "nominal_inflection"), ("est", "verbal_inflection"),
    ("em", "nominal_inflection"), ("en", "inflection"),
    ("er", "nominal_inflection"), ("es", "nominal_inflection"),
    ("te", "verbal_inflection"), ("st", "verbal_inflection"),
    ("et", "verbal_inflection"), ("e", "inflection"),
    ("n", "inflection"), ("s", "inflection"), ("t", "verbal_inflection"),
)
RU_SUFFIXES = (
    ("иями", "nominal_inflection"), ("ями", "nominal_inflection"),
    ("ами", "nominal_inflection"), ("ого", "adjectival_inflection"),
    ("ему", "adjectival_inflection"), ("ому", "adjectival_inflection"),
    ("ими", "adjectival_inflection"), ("ыми", "adjectival_inflection"),
    ("аться", "verbal_inflection"), ("яться", "verbal_inflection"),
    ("ить", "verbal_inflection"), ("ать", "verbal_inflection"),
    ("ять", "verbal_inflection"), ("ешь", "verbal_inflection"),
    ("ишь", "verbal_inflection"), ("ете", "verbal_inflection"),
    ("ите", "verbal_inflection"), ("ут", "verbal_inflection"),
    ("ют", "verbal_inflection"), ("ат", "verbal_inflection"),
    ("ят", "verbal_inflection"), ("ла", "verbal_inflection"),
    ("ли", "verbal_inflection"), ("ло", "verbal_inflection"),
    ("ого", "nominal_inflection"), ("его", "nominal_inflection"),
    ("ому", "nominal_inflection"), ("ему", "nominal_inflection"),
    ("ой", "nominal_inflection"), ("ей", "nominal_inflection"),
    ("ий", "adjectival_inflection"), ("ый", "adjectival_inflection"),
    ("ая", "adjectival_inflection"), ("яя", "adjectival_inflection"),
    ("ую", "adjectival_inflection"), ("юю", "adjectival_inflection"),
    ("ов", "nominal_inflection"), ("ев", "nominal_inflection"),
    ("ам", "nominal_inflection"), ("ям", "nominal_inflection"),
    ("ах", "nominal_inflection"), ("ях", "nominal_inflection"),
    ("ом", "nominal_inflection"), ("ем", "nominal_inflection"),
    ("ою", "nominal_inflection"), ("ею", "nominal_inflection"),
    ("ы", "nominal_inflection"), ("и", "nominal_inflection"),
    ("а", "inflection"), ("я", "inflection"), ("у", "inflection"),
    ("ю", "inflection"), ("е", "inflection"), ("о", "inflection"),
    ("ь", "inflection"),
)
DE_FOLD = str.maketrans({"ä": "a", "ö": "o", "ü": "u", "ß": "ss"})
CYRILLIC_FORM = re.compile(r"^[а-яё]+$", re.IGNORECASE)


class BuildError(RuntimeError):
    pass


def jb(value: Any, pretty: bool = True) -> bytes:
    options = {"ensure_ascii": False, "sort_keys": True}
    text = json.dumps(value, indent=2 if pretty else None,
                      separators=None if pretty else (",", ":"), **options)
    return (text + "\n").encode()


def jlb(rows: Iterable[dict[str, Any]]) -> bytes:
    return b"".join(jb(row, False) for row in rows)


def sha_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha_file(path: Path) -> str:
    return sha_bytes(path.read_bytes())


def h(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads((REPO / path).read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise BuildError(f"object required: {path}")
    return value


def verify_inputs(plan: dict[str, Any]) -> None:
    for name, record in plan["inputs"].items():
        path = REPO / record["ref"]
        if not path.is_file() or sha_file(path) != record["sha256"]:
            raise BuildError(f"input drift: {name}")
    private = REPO / PRIVATE_PREVIOUS
    if not private.is_file() or stat.S_IMODE(private.stat().st_mode) != 0o600:
        raise BuildError("previous private lexical analysis missing or not 0600")


def fold(value: str, language: str) -> str:
    value = unicodedata.normalize("NFC", value).casefold()
    return value.translate(DE_FOLD) if language == "de" else value


def signatures(value: str, language: str) -> list[tuple[str, str, int]]:
    base = fold(value, language)
    rows = [(base, "orthographic_base", 3)]
    suffixes = DE_SUFFIXES if language == "de" else RU_SUFFIXES
    for suffix, method in suffixes:
        if base.endswith(suffix) and len(base) - len(suffix) >= 4:
            stem = base[:-len(suffix)]
            rows.append((stem, method, 2 if method != "inflection" else 1))
    # German umlaut alternation and the Russian historical fold are challenger
    # features, never lemma assignments.
    return list(dict.fromkeys(rows))


def german_case_class(form: dict[str, Any], previous: dict[str, Any]) -> str:
    variants = previous.get("surface_variants", {}).get("de", {}).get(form["form_sha256"], {})
    total = sum(variants.values())
    if not total:
        return "mixed_or_sparse"
    upper = sum(count for surface, count in variants.items() if surface[:1].isupper())
    ratio = upper / total
    if total >= 3 and ratio >= .8:
        return "noun_like_initial_upper"
    if total >= 3 and ratio <= .2:
        return "lowercase_like"
    return "mixed_or_sparse"


def hunspell_stems(forms: list[str]) -> tuple[dict[str, list[str]], dict[str, Any]]:
    command = ["hunspell", "-d", "ru_RU", "-m"]
    proc = subprocess.run(
        command, input="\n".join(forms) + "\n", text=True,
        capture_output=True, check=True, encoding="utf-8", errors="replace",
    )
    blocks = [block for block in proc.stdout.split("\n\n") if block.strip()]
    if len(blocks) != len(forms):
        raise BuildError(f"Hunspell block mismatch: {len(blocks)} != {len(forms)}")
    result = {}
    for form, block in zip(forms, blocks, strict=True):
        result[form] = sorted({x.casefold().replace("ё", "е") for x in re.findall(r"(?:^|\s)st:([^\s]+)", block)})
    aff = Path("/usr/share/hunspell/ru_RU.aff")
    dic = Path("/usr/share/hunspell/ru_RU.dic")
    if not aff.is_file() or not dic.is_file():
        raise BuildError("fixed ru_RU Hunspell dictionary is unavailable")
    return result, {
        "command": command,
        "version": subprocess.run(["hunspell", "-v"], text=True, capture_output=True, check=False).stdout.strip(),
        "dictionary_aff_sha256": sha_file(aff),
        "dictionary_dic_sha256": sha_file(dic),
        "stderr_sha256": sha_bytes(proc.stderr.encode()),
    }


def import_previous_builder() -> Any:
    path = REPO / PREVIOUS_BUILDER
    spec = importlib.util.spec_from_file_location("tos_parallel_lexical_candidate_source", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def read_previous() -> dict[str, Any]:
    return load_json(PRIVATE_PREVIOUS)


def build_surface_families(previous: dict[str, Any], plan: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]], dict[str, Any]]:
    minimum = plan["thresholds"]["minimum_form_frequency"]
    maximum = plan["thresholds"]["maximum_surface_family_size"]
    forms: dict[str, dict[str, Any]] = {}
    for row in previous["keywords"]:
        if row["occurrence_count"] < minimum:
            continue
        key = f"{row['language']}:{row['analysis_key']}"
        forms[key] = {
            "language": row["language"], "form": row["analysis_key"],
            "form_sha256": row["form_key_sha256"],
            "occurrence_count": row["occurrence_count"],
            "part_range": row["part_range"], "reading_range": row["range_count"],
        }
        if row["language"] == "de":
            forms[key]["case_class"] = german_case_class(forms[key], previous)
    groups: dict[tuple[str, str], dict[str, Any]] = {}
    for key, form in forms.items():
        for signature, method, strength in signatures(form["form"], form["language"]):
            bucket = groups.setdefault((form["language"], signature), {
                "language": form["language"], "signature": signature,
                "members": set(), "methods": Counter(), "strength": strength,
            })
            bucket["members"].add(key); bucket["methods"][method] += 1
            bucket["strength"] = max(bucket["strength"], strength)

    candidates: list[dict[str, Any]] = []
    seen_sets: set[tuple[str, tuple[str, ...]]] = set()
    member_of_multi: set[str] = set()
    for group in groups.values():
        members = sorted(group["members"])
        if len(members) < 2:
            continue
        signature_key = (group["language"], tuple(members))
        if signature_key in seen_sets:
            continue
        seen_sets.add(signature_key)
        total = sum(forms[x]["occurrence_count"] for x in members)
        if total < plan["thresholds"]["minimum_multi_form_family_frequency"]:
            continue
        status = "deferred" if len(members) > maximum else "proposed"
        method = group["methods"].most_common(1)[0][0]
        if group["language"] == "ru":
            # A suffix key is a useful competing proposal, never sufficient
            # for Russian morphology (друг/другой and поэт/поэтому are controls).
            status = "ambiguous" if len(members) <= maximum else "deferred"
        elif method == "orthographic_base" or any(forms[x]["case_class"] != "noun_like_initial_upper" for x in members):
            # Lowercase and mixed-case German stems retain Liebe/liebe and
            # Leben/leben as challengers instead of pretending POS identity.
            status = "ambiguous"
        row = {
            "binding": "surface|" + group["language"] + "|" + "|".join(sorted(forms[x]["form_sha256"] for x in members)),
            "candidate_kind": "surface_morphology_family_candidate",
            "language": group["language"], "method": method,
            "status": status, "member_keys": members,
            "member_form_sha256s": sorted(forms[x]["form_sha256"] for x in members),
            "member_count": len(members), "occurrence_count": total,
            "part_range": max(forms[x]["part_range"] for x in members),
            "accepted": False, "review_refs": [], "graph_effect": False,
        }
        candidates.append(row); member_of_multi.update(members)

    # A fixed local dictionary supplies a separate Russian proposal lane.
    # It competes with suffix families; it does not overwrite them.
    ru_keys = sorted(key for key in forms if key.startswith("ru:") and CYRILLIC_FORM.fullmatch(forms[key]["form"]))
    provider_stems, provider = hunspell_stems([forms[key]["form"] for key in ru_keys])
    provider_groups: dict[str, set[str]] = defaultdict(set)
    provider_count: Counter[str] = Counter()
    for key in ru_keys:
        for stem in provider_stems[forms[key]["form"]]:
            if len(stem) >= 3:
                provider_groups[stem].add(key); provider_count[key] += 1
    for stem, member_set in sorted(provider_groups.items()):
        members = sorted(member_set)
        total = sum(forms[x]["occurrence_count"] for x in members)
        if len(members) < 2 or total < plan["thresholds"]["minimum_multi_form_family_frequency"]:
            continue
        status = "proposed" if len(members) <= 20 and all(provider_count[x] == 1 for x in members) else "ambiguous"
        hashes = sorted(forms[x]["form_sha256"] for x in members)
        candidates.append({
            "binding": "provider|ru|" + "|".join(hashes),
            "candidate_kind": "provider_morphology_family_candidate",
            "language": "ru", "method": "hunspell_dictionary_stem",
            "status": status, "member_keys": members,
            "member_form_sha256s": hashes, "member_count": len(members),
            "occurrence_count": total,
            "part_range": max(forms[x]["part_range"] for x in members),
            "provider_stem_sha256": h(stem),
            "accepted": False, "review_refs": [], "graph_effect": False,
        })
        member_of_multi.update(members)

    # High-frequency singleton probes let translation bridges and themes retain
    # important non-inflected forms without pretending a singleton is a lemma.
    strong_forms = set()
    for assoc in previous["translation_surface_associations"]:
        if assoc["status"] == "proposed":
            strong_forms.add("de:" + assoc["source_form"])
            strong_forms.add("ru:" + assoc["target_form"])
    for key, form in sorted(forms.items()):
        if key in member_of_multi or (form["occurrence_count"] < 12 and key not in strong_forms):
            continue
        candidates.append({
            "binding": f"singleton|{form['language']}|{form['form_sha256']}",
            "candidate_kind": "singleton_surface_probe_candidate",
            "language": form["language"], "method": "singleton_recurrence_probe",
            "status": "proposed", "member_keys": [key],
            "member_form_sha256s": [form["form_sha256"]], "member_count": 1,
            "occurrence_count": form["occurrence_count"], "part_range": form["part_range"],
            "accepted": False, "review_refs": [], "graph_effect": False,
        })
    deduplicated: dict[str, dict[str, Any]] = {}
    for row in candidates:
        binding = row["binding"]
        if binding not in deduplicated:
            clean = dict(row)
            if "provider_stem_sha256" in clean:
                clean["provider_stem_sha256s"] = [clean.pop("provider_stem_sha256")]
            deduplicated[binding] = clean
            continue
        current = deduplicated[binding]
        current["status"] = "ambiguous"
        if "provider_stem_sha256" in row:
            current.setdefault("provider_stem_sha256s", []).append(row["provider_stem_sha256"])
            current["provider_stem_sha256s"] = sorted(set(current["provider_stem_sha256s"]))
    candidates = sorted(deduplicated.values(), key=lambda x: (x["language"], x["binding"]))
    return candidates, forms, provider


def memberships(families: list[dict[str, Any]]) -> dict[str, list[str]]:
    by_form: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for family in families:
        for key in family["member_keys"]:
            by_form[key].append(family)
    result = {}
    for key, rows in by_form.items():
        rank = {"proposed": 0, "ambiguous": 1, "deferred": 2}
        rows.sort(key=lambda x: (
            rank[x["status"]],
            0 if x["member_count"] > 1 else 1,
            x["member_count"], x["binding"],
        ))
        result[key] = [x["binding"] for x in rows[:2]]
    return result


def add_alignment_challengers(families: list[dict[str, Any]], forms: dict[str, dict[str, Any]], previous: dict[str, Any], plan: dict[str, Any]) -> list[dict[str, Any]]:
    member_map = memberships(families)
    strong = [
        x for x in previous["translation_surface_associations"]
        if x["status"] != "deferred" and x["strict_support"] >= 5
        and x["dice_millionths"] >= 300_000
    ]
    # Each form contributes only its strongest surviving cross-language
    # neighbour. This exposes suppletion/derivation without letting a whole
    # paragraph's contextual vocabulary become a morphology family.
    best_de: dict[str, dict[str, Any]] = {}
    best_ru: dict[str, dict[str, Any]] = {}
    score = lambda x: (
        1 if x["status"] == "proposed" else 0,
        x["strict_support"], x["dice_millionths"],
        -x["source_candidate_rank"], -x["target_candidate_rank"],
    )
    for row in strong:
        de_key, ru_key = "de:" + row["source_form"], "ru:" + row["target_form"]
        if de_key in forms and (de_key not in best_de or score(row) > score(best_de[de_key])):
            best_de[de_key] = row
        if ru_key in forms and (ru_key not in best_ru or score(row) > score(best_ru[ru_key])):
            best_ru[ru_key] = row
    evidence: dict[tuple[str, str], list[tuple[str, int]]] = defaultdict(list)
    for de_key, row in best_de.items():
        ru_key = "ru:" + row["target_form"]
        for ru_family in member_map.get(ru_key, []):
            evidence[("de", ru_family)].append((de_key, row["support"]))
    for ru_key, row in best_ru.items():
        de_key, ru_key = "de:" + row["source_form"], "ru:" + row["target_form"]
        for de_family in member_map.get(de_key, []):
            evidence[("ru", de_family)].append((ru_key, row["support"]))
    existing = {(x["language"], tuple(x["member_form_sha256s"])) for x in families}
    challengers = []
    for (language, opposite_family), rows in sorted(evidence.items()):
        unique = sorted({key for key, _support in rows if key in forms})
        if len(unique) < 2:
            continue
        support = sum(support for key, support in rows if key in unique)
        hashes = sorted(forms[x]["form_sha256"] for x in unique)
        if support < plan["thresholds"]["alignment_challenger_minimum_combined_support"]:
            continue
        if (language, tuple(hashes)) in existing:
            continue
        row = {
            "binding": "alignment-challenger|" + language + "|" + "|".join(hashes),
            "candidate_kind": "alignment_neighborhood_family_challenger",
            "language": language, "method": "shared_opposite_language_family_neighborhood",
            "status": "deferred", "member_keys": unique,
            "member_form_sha256s": hashes, "member_count": len(unique),
            "occurrence_count": sum(forms[x]["occurrence_count"] for x in unique),
            "part_range": max(forms[x]["part_range"] for x in unique),
            "alignment_support": support, "opposite_family_binding": opposite_family,
            "accepted": False, "review_refs": [], "graph_effect": False,
        }
        challengers.append(row); existing.add((language, tuple(hashes)))
    return sorted(families + challengers, key=lambda x: (x["language"], x["binding"]))


def aggregate_bridges(families: list[dict[str, Any]], previous: dict[str, Any], plan: dict[str, Any]) -> list[dict[str, Any]]:
    member_map = memberships(families)
    accum: dict[tuple[str, str], dict[str, Any]] = defaultdict(lambda: {
        "support": 0, "strict_support": 0, "proposed_sources": 0,
        "weighted_dice": 0, "source_candidate_ids": [],
    })
    for row in previous["translation_surface_associations"]:
        de_key, ru_key = "de:" + row["source_form"], "ru:" + row["target_form"]
        for de_family in member_map.get(de_key, []):
            for ru_family in member_map.get(ru_key, []):
                target = accum[(de_family, ru_family)]
                target["support"] += row["support"]
                target["strict_support"] += row["strict_support"]
                target["weighted_dice"] += row["dice_millionths"] * max(row["support"], 1)
                target["source_candidate_ids"].append(row["candidate_id"])
                target["proposed_sources"] += row["status"] == "proposed"
    result = []
    for pair, values in accum.items():
        if values["support"] < plan["thresholds"]["family_bridge_minimum_support"]:
            continue
        dice = values["weighted_dice"] // max(values["support"], 1)
        if dice < plan["thresholds"]["family_bridge_minimum_dice_millionths"]:
            continue
        status = "proposed" if values["proposed_sources"] and values["strict_support"] >= 4 else "ambiguous"
        result.append({
            "binding": "family-bridge|" + pair[0] + "|" + pair[1],
            "relation_type": "candidate_translation_neighborhood_bridge",
            "subject_binding": pair[0], "object_binding": pair[1],
            "status": status, "support": values["support"],
            "strict_support": values["strict_support"], "dice_millionths": dice,
            "source_candidate_refs": sorted(set(values["source_candidate_ids"])),
            "accepted": False, "review_refs": [], "graph_effect": False,
        })
    return sorted(result, key=lambda x: (-x["support"], x["binding"]))


def co_recurrence_relations(families: list[dict[str, Any]], forms: dict[str, dict[str, Any]], previous_builder: Any, plan: dict[str, Any]) -> list[dict[str, Any]]:
    member_map = memberships(families)
    primary = {key: bindings[0] for key, bindings in member_map.items()}
    units, _de, _ru, _layers = previous_builder.load_parallel()
    positive = [x for x in units if x["status"] == "proposed" and x["positive_evidence_eligible"]]
    result = []
    for language in ("de", "ru"):
        df, pairs = Counter(), Counter()
        for unit in positive:
            candidates = {primary.get(language + ":" + token) for token in unit[language]}
            candidates.discard(None)
            ranked = sorted(candidates, key=lambda b: (
                -next(x["occurrence_count"] for x in families if x["binding"] == b), b
            ))[:18]
            df.update(ranked); pairs.update(combinations(sorted(ranked), 2))
        rows = []
        n = len(positive)
        for (left, right), support in pairs.items():
            if support < plan["thresholds"]["co_recurrence_minimum_support"]:
                continue
            dice = 2 * support / (df[left] + df[right])
            pmi = math.log2((support * n) / (df[left] * df[right]))
            if round(dice * 1_000_000) < plan["thresholds"]["co_recurrence_minimum_dice_millionths"] or round(pmi * 1000) < plan["thresholds"]["co_recurrence_minimum_pmi_millibits"]:
                continue
            status = "proposed" if support >= 12 and dice >= .30 and pmi >= 1.0 else "ambiguous"
            rows.append({
                "binding": f"co-recurrence|{language}|{left}|{right}",
                "relation_type": "candidate_within_language_co_recurrence",
                "language": language, "subject_binding": left, "object_binding": right,
                "status": status, "support": support, "unit_count": n,
                "dice_millionths": round(dice * 1_000_000),
                "pmi_millibits": round(pmi * 1000),
                "accepted": False, "review_refs": [], "graph_effect": False,
            })
        rows.sort(key=lambda x: (-x["support"], -x["dice_millionths"], x["binding"]))
        result.extend(rows[:plan["thresholds"]["maximum_co_recurrence_relations_per_language"]])
    return result


class UnionFind:
    def __init__(self) -> None:
        self.parent: dict[str, str] = {}

    def find(self, value: str) -> str:
        self.parent.setdefault(value, value)
        if self.parent[value] != value:
            self.parent[value] = self.find(self.parent[value])
        return self.parent[value]

    def union(self, left: str, right: str) -> None:
        a, b = self.find(left), self.find(right)
        if a != b:
            self.parent[max(a, b)] = min(a, b)


def build_clusters(families: list[dict[str, Any]], bridges: list[dict[str, Any]]) -> list[dict[str, Any]]:
    by_binding = {x["binding"]: x for x in families}
    uf = UnionFind()
    for bridge in bridges:
        if bridge["status"] == "proposed":
            uf.union(bridge["subject_binding"], bridge["object_binding"])
    components: dict[str, set[str]] = defaultdict(set)
    for value in uf.parent:
        components[uf.find(value)].add(value)
    clusters = []
    for members in components.values():
        languages = Counter(by_binding[x]["language"] for x in members)
        if not languages["de"] or not languages["ru"]:
            continue
        member_bridges = [x for x in bridges if x["subject_binding"] in members and x["object_binding"] in members]
        status = "proposed" if len(members) == 2 and len(member_bridges) >= 1 else "ambiguous"
        binding = "cluster|" + "|".join(sorted(members))
        clusters.append({
            "binding": binding, "candidate_kind": "bilingual_recurrence_neighborhood_cluster_candidate",
            "status": status, "member_bindings": sorted(members),
            "member_count": len(members), "language_member_counts": dict(sorted(languages.items())),
            "bridge_count": len(member_bridges),
            "aggregate_bridge_support": sum(x["support"] for x in member_bridges),
            "accepted": False, "review_refs": [], "graph_effect": False,
            "concept_identity_asserted": False,
        })
    return sorted(clusters, key=lambda x: (-x["aggregate_bridge_support"], x["binding"]))


def relation_candidates(families: list[dict[str, Any]], bridges: list[dict[str, Any]], co_relations: list[dict[str, Any]], clusters: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rows = list(bridges) + list(co_relations)
    # Explicit form membership and competition make overlapping analyses visible.
    by_form: dict[str, list[str]] = defaultdict(list)
    for family in families:
        for form_hash in family["member_form_sha256s"]:
            by_form[form_hash].append(family["binding"])
            rows.append({
                "binding": f"membership|{form_hash}|{family['binding']}",
                "relation_type": "candidate_form_membership",
                "subject_form_sha256": form_hash, "object_binding": family["binding"],
                "status": family["status"], "support": family["occurrence_count"],
                "accepted": False, "review_refs": [], "graph_effect": False,
            })
    seen_competition = set()
    for bindings in by_form.values():
        for left, right in combinations(sorted(set(bindings)), 2):
            pair = (left, right)
            if pair in seen_competition:
                continue
            seen_competition.add(pair)
            rows.append({
                "binding": f"competition|{left}|{right}",
                "relation_type": "candidate_competes_with",
                "subject_binding": left, "object_binding": right,
                "status": "ambiguous", "support": 1,
                "accepted": False, "review_refs": [], "graph_effect": False,
            })
    family_cluster = {}
    for cluster in clusters:
        for member in cluster["member_bindings"]:
            family_cluster[member] = cluster["binding"]
            rows.append({
                "binding": f"cluster-membership|{member}|{cluster['binding']}",
                "relation_type": "candidate_cluster_membership",
                "subject_binding": member, "object_binding": cluster["binding"],
                "status": cluster["status"], "support": cluster["aggregate_bridge_support"],
                "accepted": False, "review_refs": [], "graph_effect": False,
            })
    cluster_pairs: dict[tuple[str, str], dict[str, int]] = defaultdict(lambda: {"support": 0, "relations": 0})
    for row in co_relations:
        left = family_cluster.get(row["subject_binding"]); right = family_cluster.get(row["object_binding"])
        if not left or not right or left == right:
            continue
        pair = tuple(sorted((left, right)))
        cluster_pairs[pair]["support"] += row["support"]
        cluster_pairs[pair]["relations"] += 1
    for (left, right), evidence in cluster_pairs.items():
        rows.append({
            "binding": f"cluster-co-recurrence|{left}|{right}",
            "relation_type": "candidate_cluster_co_recurrence",
            "subject_binding": left, "object_binding": right,
            "status": "ambiguous", "support": evidence["support"],
            "family_relation_count": evidence["relations"],
            "accepted": False, "review_refs": [], "graph_effect": False,
        })
    return sorted(rows, key=lambda x: (x["relation_type"], x["binding"]))


def all_identity_bindings(families: list[dict[str, Any]], clusters: list[dict[str, Any]], relations: list[dict[str, Any]]) -> list[tuple[str, str]]:
    return sorted(set(
        [("family", x["binding"]) for x in families]
        + [("cluster", x["binding"]) for x in clusters]
        + [("relation", x["binding"]) for x in relations]
    ))


def issue(bindings: list[tuple[str, str]]) -> None:
    path = REPO / ISSUANCE_REF
    if path.exists():
        raise BuildError("identity issuance exists; refusing remint")
    prefixes = {"family": "tos.annotation.morph-family-candidate.sid-", "cluster": "tos.annotation.theme-cluster-candidate.sid-", "relation": "tos.claim.typed-relation-candidate.sid-"}
    identities = [{"kind": kind, "binding": binding, "id": prefixes[kind] + secrets.token_hex(16)} for kind, binding in bindings]
    payload = {
        "schema_version": "tos_zarathustra_morphology_theme_candidate_identity_issuance_v1",
        "issuance_id": "tos.identity-issuance.zarathustra-morphology-themes-v1",
        "issued_on": "2026-09-02", "opaque_identity": True,
        "binding_is_not_identity_or_linguistic_judgment": True,
        "candidate_count": len(identities), "identities": identities,
    }
    write(path.relative_to(REPO), jb(payload))


def identity_map(bindings: list[tuple[str, str]]) -> dict[tuple[str, str], str]:
    data = load_json(ISSUANCE_REF)
    found = {(x["kind"], x["binding"]): x["id"] for x in data["identities"]}
    if set(found) != set(bindings) or len(found) != len(data["identities"]):
        raise BuildError("candidate identity binding drift")
    if len(set(found.values())) != len(found):
        raise BuildError("candidate identity collision")
    return found


def publicize(rows: list[dict[str, Any]], kind: str, ids: dict[tuple[str, str], str]) -> list[dict[str, Any]]:
    output = []
    for raw in rows:
        row = dict(raw)
        binding = row.pop("binding")
        row[f"{kind}_id"] = ids[(kind, binding)]
        for key in ("member_keys", "opposite_family_binding"):
            row.pop(key, None)
        for key in ("subject_binding", "object_binding"):
            if key in row:
                target_kind = "cluster" if row[key].startswith("cluster|") else "family"
                row[key.replace("binding", "ref")] = ids[(target_kind, row.pop(key))]
        if "member_bindings" in row:
            row["member_refs"] = [ids[("family", x)] for x in row.pop("member_bindings")]
        output.append(row)
    return output


def readable_analysis(families: list[dict[str, Any]], forms: dict[str, dict[str, Any]], clusters: list[dict[str, Any]], relations: list[dict[str, Any]], ids: dict[tuple[str, str], str], summary: dict[str, Any]) -> dict[str, Any]:
    family_labels = {}
    family_languages = {}
    private_families = []
    for family in families:
        labels = [forms[x]["form"] for x in family["member_keys"]]
        family_labels[family["binding"]] = labels
        family_languages[family["binding"]] = family["language"]
        private_families.append({**family, "family_id": ids[("family", family["binding"])], "forms": labels})
    private_clusters = []
    for cluster in clusters:
        de = [form for member in cluster["member_bindings"] for form in family_labels[member] if family_languages[member] == "de"]
        ru = [form for member in cluster["member_bindings"] for form in family_labels[member] if family_languages[member] == "ru"]
        private_clusters.append({**cluster, "cluster_id": ids[("cluster", cluster["binding"])], "display_hint": {"de": de[:8], "ru": ru[:8]}})
    examples = {
        "de_mensch": [x for x in private_families if {"mensch", "menschen"}.intersection(x["forms"])],
        "de_liebe": [x for x in private_families if {"liebe", "lieben", "liebt"}.intersection(x["forms"])],
        "ru_human": [x for x in private_families if {"человек", "людей", "люди"}.intersection(x["forms"])],
        "ru_love": [x for x in private_families if {"любовь", "люблю"}.intersection(x["forms"])],
    }
    return {
        "schema_version": "tos_zarathustra_morphology_theme_private_analysis_v1",
        "source_bearing": True, "required_mode": "0600", "summary": summary,
        "families": private_families, "clusters": private_clusters,
        "typed_relations": [{**x, "relation_id": ids[("relation", x["binding"])]} for x in relations],
        "named_probe_examples": examples,
        "authority_boundary": "readable agent proposals only; no accepted morphology, lemma, lexeme, sense, sign, concept, relation, graph fact, canon, or human review",
    }


def generate(with_identities: bool) -> tuple[dict[Path, bytes], dict[Path, tuple[bytes, int]], list[tuple[str, str]], dict[str, Any]]:
    plan = load_json(PLAN_REF); verify_inputs(plan)
    previous = read_previous(); previous_builder = import_previous_builder()
    families, forms, provider = build_surface_families(previous, plan)
    families = add_alignment_challengers(families, forms, previous, plan)
    bridges = aggregate_bridges(families, previous, plan)
    co_relations = co_recurrence_relations(families, forms, previous_builder, plan)
    clusters = build_clusters(families, bridges)
    relations = relation_candidates(families, bridges, co_relations, clusters)
    bindings = all_identity_bindings(families, clusters, relations)
    if not with_identities:
        return {}, {}, bindings, {"families": families, "clusters": clusters, "relations": relations}
    ids = identity_map(bindings)
    public_families = publicize(families, "family", ids)
    public_clusters = publicize(clusters, "cluster", ids)
    public_relations = publicize(relations, "relation", ids)
    family_status = Counter(x["status"] for x in families)
    cluster_status = Counter(x["status"] for x in clusters)
    relation_types = Counter(x["relation_type"] for x in relations)
    summary = {
        "schema_version": "tos_zarathustra_morphology_theme_candidate_summary_v1",
        "status": "completed-agent-candidate-pass-no-promotion",
        "parts": 4, "input_form_candidate_count": len(forms),
        "morphological_family_candidate_count": len(families),
        "families_by_language": dict(sorted(Counter(x["language"] for x in families).items())),
        "family_status_counts": dict(sorted(family_status.items())),
        "alignment_challenger_family_count": sum(x["candidate_kind"] == "alignment_neighborhood_family_challenger" for x in families),
        "thematic_cluster_candidate_count": len(clusters),
        "cluster_status_counts": dict(sorted(cluster_status.items())),
        "typed_relation_candidate_count": len(relations),
        "relation_type_counts": dict(sorted(relation_types.items())),
        "accepted_candidate_count": 0, "human_review_count": 0,
        "semantic_relation_asserted": False, "concept_identity_asserted": False,
        "graph_effect": False, "canon_effect": False,
    }
    private_analysis = readable_analysis(families, forms, clusters, relations, ids, summary)
    private_analysis["russian_morphology_provider"] = provider
    outputs = {
        OUTPUTS["families"]: jlb(public_families),
        OUTPUTS["relations"]: jlb(public_relations),
        OUTPUTS["clusters"]: jlb(public_clusters),
        OUTPUTS["summary"]: jb(summary),
    }
    coverage = {
        "schema_version": "tos_zarathustra_morphology_theme_candidate_coverage_v1",
        "parts_complete": 4, "languages": ["de", "ru"],
        "input_keyword_form_candidates": len(forms),
        "forms_with_family_membership": len({x for family in families for x in family["member_keys"]}),
        "positive_alignment_units_used_for_co_recurrence": load_json(PREVIOUS / "coverage-receipt.v1.json")["proposed_positive_evidence_units"],
        "german_provider_census_token_coverage": load_json(Path(plan["inputs"]["german_morphology_census_receipt"]["ref"]))["coverage"]["token_weighted_coverage"],
        "provider_output_used_as_accepted_morphology": False,
        "russian_provider": provider,
        "tracked_source_strings": False, "private_output_mode": "0600",
        "competing_memberships_preserved": True, "zero_review_refs": True,
        "accepted_candidate_count": 0, "graph_effect": False,
    }
    outputs[OUTPUTS["coverage"]] = jb(coverage)
    outputs[OUTPUTS["provenance"]] = jlb([{
        "schema_version": "tos_provenance_event_v1",
        "event_id": "tos.event.zarathustra-morphology-theme-candidates-v1.build",
        "event_type": "agent_candidate_morphology_theme_materialization",
        "occurred_at": "2026-09-02T03:00:00-06:00",
        "agent_ref": "codex-internal-agents.morphology-theme-candidate-v1",
        "software_ref": str(GENERATOR_REF), "software_sha256": sha_file(REPO / GENERATOR_REF),
        "plan_ref": str(PLAN_REF), "plan_sha256": sha_file(REPO / PLAN_REF),
        "authority_boundary": plan["authority_boundary"],
    }])
    private = {PRIVATE_ANALYSIS: (jb(private_analysis), 0o600)}
    manifest = {
        "schema_version": "tos_zarathustra_morphology_theme_candidate_manifest_v1",
        "plan_ref": str(PLAN_REF), "plan_sha256": sha_file(REPO / PLAN_REF),
        "identity_issuance_ref": str(ISSUANCE_REF), "identity_issuance_sha256": sha_file(REPO / ISSUANCE_REF),
        "generated_outputs": {str(path): {"sha256": sha_bytes(payload), "byte_size": len(payload)} for path, payload in sorted(outputs.items(), key=lambda x: str(x[0]))},
        "private_outputs": {str(path): {"sha256": sha_bytes(payload), "byte_size": len(payload), "required_mode": "0600"} for path, (payload, _mode) in private.items()},
        "source_text_included": False, "accepted_candidate_count": 0,
        "semantic_relation_asserted": False, "concept_identity_asserted": False,
        "graph_effect": False, "canon_effect": False,
    }
    outputs[OUTPUTS["manifest"]] = jb(manifest)
    return outputs, private, bindings, private_analysis


def write(path: Path, payload: bytes, mode: int = 0o644) -> None:
    target = REPO / path
    target.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix="." + target.name + ".", suffix=".tmp", dir=target.parent)
    temporary = Path(name)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(payload); handle.flush(); os.fsync(handle.fileno())
        os.chmod(temporary, mode); os.replace(temporary, target)
    finally:
        temporary.unlink(missing_ok=True)


def build(issue_identities: bool) -> dict[str, Any]:
    _outputs, _private, bindings, _analysis = generate(False)
    if issue_identities:
        issue(bindings)
    identity_map(bindings)
    outputs, private, _bindings, analysis = generate(True)
    for path, payload in outputs.items():
        write(path, payload)
    for path, (payload, mode) in private.items():
        write(path, payload, mode)
    return analysis["summary"]


def check() -> dict[str, Any]:
    outputs, private, bindings, analysis = generate(True)
    identity_map(bindings)
    for path, payload in outputs.items():
        target = REPO / path
        if not target.is_file() or target.read_bytes() != payload:
            raise BuildError(f"tracked parity mismatch: {path}")
    for path, (payload, mode) in private.items():
        target = REPO / path
        if not target.is_file() or target.read_bytes() != payload:
            raise BuildError(f"private parity mismatch: {path}")
        if stat.S_IMODE(target.stat().st_mode) != mode:
            raise BuildError(f"private mode mismatch: {path}")
    return analysis["summary"]


def preview() -> dict[str, Any]:
    _outputs, _private, bindings, analysis = generate(False)
    return {
        "identity_count": len(bindings),
        "family_count": len(analysis["families"]),
        "cluster_count": len(analysis["clusters"]),
        "relation_count": len(analysis["relations"]),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build", action="store_true")
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--preview", action="store_true")
    parser.add_argument("--issue-identities", action="store_true")
    args = parser.parse_args()
    try:
        if args.preview:
            result = preview()
        elif args.build:
            result = build(args.issue_identities)
        else:
            if args.issue_identities:
                raise BuildError("--issue-identities is valid only with --build")
            result = check()
    except (BuildError, OSError, KeyError, ValueError, json.JSONDecodeError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr); return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True)); return 0


if __name__ == "__main__":
    raise SystemExit(main())
