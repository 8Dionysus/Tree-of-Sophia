#!/usr/bin/env python3
"""Build whole-work DE/RU lexical recurrence and candidate observations.

Tracked files are source-withholding projections. Exact strings, token order,
occurrence selectors, and readable labels stay in ignored mode-0600 artifacts.
Nothing emitted here accepts a text, lemma, translation, sign, or concept.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import os
import re
import secrets
import sqlite3
import stat
import sys
import tempfile
import unicodedata
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable


REPO = Path(__file__).resolve().parents[1]
WORK = Path("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra")
ROUTE = WORK / "lexical-indexes/dta-antonovsky-parallel-candidates-v1"
ALIGN = WORK / "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1"
DE_TECH = WORK / "technical-markup/dta-first-editions-parts-1-4-v1"
RU_TECH = WORK / "technical-markup/antonovsky-1911-structural-paragraph-v2"
PRIVATE = WORK / "gold-sets/foundation-pilot-v1/local-content/parallel-lexical-candidates-v1"
PLAN_REF = ROUTE / "plan.v1.json"
ISSUANCE_REF = ROUTE / "identity-issuance.v1.json"
DE_DB = WORK / "gold-sets/foundation-pilot-v1/local-content/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.sqlite3"
RU_BUILDER = Path("scripts/build_antonovsky_1911_structural_paragraph_v2.py")
GENERATOR_REF = Path("scripts/build_zarathustra_parallel_lexical_candidates_v1.py")

OUTPUTS = {
    "russian_recurrence": ROUTE / "russian-recurrence.v1.jsonl",
    "keywords": ROUTE / "keyword-candidates.v1.jsonl",
    "phrases": ROUTE / "repeated-sequences.v1.jsonl",
    "associations": ROUTE / "translation-surface-association-candidates.v1.jsonl",
    "coverage": ROUTE / "coverage-receipt.v1.json",
    "summary": ROUTE / "summary.v1.json",
    "provenance": ROUTE / "provenance.jsonl",
    "manifest": ROUTE / "manifest.v1.json",
}
PRIVATE_DB = PRIVATE / "antonovsky-1911-lexical-observation-v1.sqlite3"
PRIVATE_ANALYSIS = PRIVATE / "parallel-candidate-analysis.v1.json"

LETTER = "A-Za-zÄÖÜäöüßẞА-Яа-яЁёІіЇїѢѣѲѳѴѵ"
TOKEN_RE = re.compile(rf"[{LETTER}]+(?:[’'][{LETTER}]+)*")
LINE_JOIN_RE = re.compile(rf"([{LETTER}]{{2,}})[-¬]\s*\n\s*([{LETTER}]{{2,}})")
SPACED_RE = re.compile(rf"(?<![{LETTER}])((?:[{LETTER}]\s+){{2,}}[{LETTER}])(?![{LETTER}])")
RU_FOLD = str.maketrans({"ѣ": "е", "і": "и", "ї": "и", "ѳ": "ф", "ѵ": "и"})

DE_STOP = frozenset("aber alle allem allen aller alles als also am an auch auf aus bei bin bis bist da damit dann das dass dein deine dem den denn der des die dies diese doch dort du durch ein eine einem einen einer eines er es etwas für gegen gehabt ganz hat haben hier ich im in ist ja jede jedem jeden jeder jedes kann kein keine man mein meine mich mit muß nach nicht noch nun nur ob oder ohne sein seine sich sie sind so über um und uns unser unter vom von vor war waren was weil wenn werde werden wie wieder wir wird wo zu zum zur".split())
RU_STOP = frozenset("а без бы был была были было быть в во вот все всего всех вы где да для до его ее если есть еще же за и из или им их к как ко когда кто ли мне мой моя мы на над не него нее нет ни но ну о об он она они оно от по под при про с со так также там те тем то того тоже тот ты у уж уже хотя чем что чтобы эта эти это я въ къ съ онъ она они его ея ею ему ихъ мой моя мое моею мы ты вы не былъ была было были бы иль надъ подъ при чемъ что-бы".split())
DE_KEYWORD_STOP = DE_STOP | frozenset("ihr mir euch ihm dir wer dich ihn ihnen sei schon soll muss dieser diese diess jene selber".split())
RU_KEYWORD_STOP = RU_STOP | frozenset("меня себя мои тебя себе вас вам них свою этого ибо только чтоб теперь даже тогда должен своей много слишком больше более".split())


class BuildError(RuntimeError):
    pass


def jbytes(value: Any, *, pretty: bool = True) -> bytes:
    if pretty:
        text = json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True)
    else:
        text = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return (text + "\n").encode()


def jlbytes(rows: Iterable[dict[str, Any]]) -> bytes:
    return b"".join(jbytes(row, pretty=False) for row in rows)


def digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def digest_file(path: Path) -> str:
    return digest_bytes(path.read_bytes())


def h(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads((REPO / path).read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise BuildError(f"object required: {path}")
    return value


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in (REPO / path).read_text(encoding="utf-8").splitlines()]


def verify_inputs(plan: dict[str, Any]) -> None:
    for label, record in plan["inputs"].items():
        path = REPO / record["ref"]
        if not path.is_file() or digest_file(path) != record["sha256"]:
            raise BuildError(f"input drift: {label}")
    if not (REPO / DE_DB).is_file():
        raise BuildError("German lexical database is missing")


def base_key(value: str) -> str:
    return unicodedata.normalize("NFC", value).casefold()


def ru_key(value: str) -> str:
    value = base_key(value).translate(RU_FOLD)
    if len(value) > 2 and value.endswith("ъ"):
        value = value[:-1]
    return value


def preprocess(text: str, language: str) -> str:
    value = text.replace("\r\n", "\n")
    return LINE_JOIN_RE.sub(lambda m: m.group(1) + m.group(2), value)


def tokens(text: str, language: str) -> list[str]:
    key = ru_key if language == "ru" else base_key
    return [key(m.group(0)) for m in TOKEN_RE.finditer(preprocess(text, language))]


def exact_tokens(text: str) -> list[tuple[str, int, int]]:
    return [(m.group(0), m.start(), m.end()) for m in TOKEN_RE.finditer(text)]


def script_clean(value: str, language: str) -> bool:
    if language == "de":
        return not re.search(r"[А-Яа-яЁёІіЇїѢѣѲѳѴѵ]", value)
    return not re.search(r"[A-Za-zÄÖÜäöüßẞ]", value)


def quality_profile(text: str, language: str) -> dict[str, Any]:
    raw = [x[0] for x in exact_tokens(text)]
    one = sum(len(base_key(x)) == 1 for x in raw)
    mixed = sum(not script_clean(x, language) for x in raw)
    content = sum(
        len((ru_key(x) if language == "ru" else base_key(x))) >= 3
        and script_clean(x, language) for x in raw
    )
    return {
        "exact_token_count": len(raw), "one_letter_token_count": one,
        "one_letter_share": one / len(raw) if raw else 1.0,
        "spaced_letter_run_count": len(SPACED_RE.findall(text)),
        "line_join_candidate_count": len(LINE_JOIN_RE.findall(text.replace("\r\n", "\n"))),
        "mixed_script_token_count": mixed, "content_token_count": content,
        "positive_evidence_eligible": bool(raw) and one / len(raw) <= .25
        and content > 0 and mixed / len(raw) <= .1,
    }


def entropy(parts: Counter[int]) -> float:
    total = sum(parts.values())
    if not total or len(parts) < 2:
        return 0.0
    raw = -sum((n / total) * math.log(n / total) for n in parts.values())
    return raw / math.log(4)


def million(value: float) -> int:
    return round(max(-1.0, min(1.0, value)) * 1_000_000)


def slice_anchor(side: dict[str, Any], anchor_ref: str, caches: dict[str, str]) -> str:
    anchors = side["anchor_map"]
    anchor = anchors[anchor_ref]
    ref = anchor["text_layer_ref"]
    if ref not in caches:
        path = REPO / ref
        if stat.S_IMODE(path.stat().st_mode) != 0o600:
            raise BuildError(f"private text layer is not 0600: {ref}")
        caches[ref] = path.read_text(encoding="utf-8")
        if h(caches[ref]) != anchor["text_layer_sha256"]:
            raise BuildError(f"private text layer drift: {ref}")
    s = anchor["selector"]
    text = caches[ref][s["start"]:s["end"]]
    if h(text) != anchor["exact_sha256"]:
        raise BuildError(f"anchor return mismatch: {anchor_ref}")
    return text


def load_parallel() -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    units, de_units, ru_units, caches = [], [], [], {}
    spine = {x["alignment_id"]: x for x in load_jsonl(ALIGN / "alignment-spine.v1.jsonl")}
    for part in range(1, 5):
        packet = load_json(ALIGN / f"part-{part}.translation-alignment-packet.v1.json")
        source = dict(packet["source_side"])
        target = dict(packet["target_side"])
        source["anchor_map"] = {x["anchor_ref"]: x for x in source["anchors"]}
        target["anchor_map"] = {x["anchor_ref"]: x for x in target["anchors"]}
        for alignment in packet["alignments"]:
            de_texts = [slice_anchor(source, x, caches) for x in alignment["ordered_source_anchor_refs"]]
            ru_texts = [slice_anchor(target, x, caches) for x in alignment["ordered_target_anchor_refs"]]
            de_text = "\n".join(de_texts)
            ru_text = "\n".join(ru_texts)
            row = {
                "alignment_id": alignment["alignment_id"],
                "part": part,
                "reading": f"p{part}.r{spine[alignment['alignment_id']]['reading_ordinal_within_part']}",
                "status": alignment["status"],
                "shape": alignment["correspondence_shape"],
                "strict": alignment["status"] == "proposed" and alignment["correspondence_shape"] == "one_to_one" and "agrees across" in alignment["status_reason"],
                "de": tokens(de_text, "de"),
                "ru": tokens(ru_text, "ru"),
                "de_quality": quality_profile(de_text, "de"),
                "ru_quality": quality_profile(ru_text, "ru"),
            }
            paragraph_qualities = [quality_profile(x, "de") for x in de_texts] + [quality_profile(x, "ru") for x in ru_texts]
            row["candidate_universe_eligible"] = (
                row["de_quality"]["one_letter_share"] <= .5
                and row["ru_quality"]["one_letter_share"] <= .5
                and row["de_quality"]["content_token_count"] > 0
                and row["ru_quality"]["content_token_count"] > 0
            )
            row["positive_evidence_eligible"] = all(x["positive_evidence_eligible"] for x in paragraph_qualities)
            units.append(row)
            de_units.append({"part": part, "unit_id": alignment["alignment_id"], "tokens": row["de"]})
            ru_units.append({"part": part, "unit_id": alignment["alignment_id"], "tokens": row["ru"]})
    return units, de_units, ru_units, list(caches)


def import_ru_builder() -> Any:
    path = REPO / RU_BUILDER
    spec = importlib.util.spec_from_file_location("antonovsky_ru_candidate_source", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def build_ru_observations() -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    module = import_ru_builder()
    model = module.reconstruct()
    tracked = load_jsonl(RU_TECH / "logical-row-spine.v2.jsonl")
    if len(tracked) != len(model["rows"]):
        raise BuildError("Russian logical-row parity drift")
    include = {"reading_unit_heading", "cycle_heading", "prose", "verse"}
    occurrences, units = [], []
    row_text: dict[str, str] = {}
    role_counts: Counter[str] = Counter()
    for raw, bound in zip(model["rows"], tracked, strict=True):
        if h(raw.text) != bound["exact_sha256"]:
            raise BuildError("Russian row text return mismatch")
        row_text[raw.row_ref] = raw.text
        role_counts[raw.role] += 1
        if raw.role not in include or not raw.reading_ref:
            continue
        part = int(raw.reading_ref.split(".")[0].split("_")[1])
        unit_tokens = tokens(raw.text, "ru")
        units.append({"part": part, "reading": raw.reading_ref, "unit_id": bound["logical_row_unit_id"], "tokens": unit_tokens})
        for ordinal, (surface, start, end) in enumerate(exact_tokens(raw.text), 1):
            normalized = base_key(surface)
            folded = ru_key(surface)
            occurrence_id = "tos.occurrence.zarathustra-ru-ant1911.sid-" + h(
                f"{bound['logical_row_unit_id']}\n{start}\n{end}\n{h(surface)}"
            )[:32]
            occurrences.append({
                "occurrence_id": occurrence_id, "unit_id": bound["logical_row_unit_id"],
                "reading": raw.reading_ref, "part": part, "role": raw.role,
                "ordinal": ordinal, "start": start, "end": end, "surface": surface,
                "exact_sha256": h(surface), "normalized": normalized,
                "normalized_sha256": h(normalized), "analysis_key": folded,
                "analysis_key_sha256": h(folded),
            })
    if len({x["occurrence_id"] for x in occurrences}) != len(occurrences):
        raise BuildError("Russian occurrence identity collision")
    phrase_units: list[dict[str, Any]] = []
    tracked_paragraphs = load_jsonl(RU_TECH / "paragraph-spine.v2.jsonl")
    for raw, bound in zip(model["paragraphs"], tracked_paragraphs, strict=True):
        reading = raw["reading_ref"]
        part = int(reading.split(".")[0].split("_")[1])
        phrase_units.append({
            "part": part, "reading": reading, "unit_id": bound["paragraph_unit_id"],
            "tokens": tokens("\n".join(row_text[x] for x in raw["row_refs"]), "ru"),
        })
    tracked_verse = load_jsonl(RU_TECH / "verse-line-spine.v2.jsonl")
    for raw, bound in zip(model["verse_lines"], tracked_verse, strict=True):
        reading = raw["reading_ref"]
        part = int(reading.split(".")[0].split("_")[1])
        phrase_units.append({
            "part": part, "reading": reading, "unit_id": bound["verse_line_unit_id"],
            "tokens": tokens(row_text[raw["row_ref"]], "ru"),
        })
    role_by_id = {x["logical_row_unit_id"]: x["technical_role"] for x in tracked}
    phrase_units.extend(
        x for x in units
        if role_by_id.get(x["unit_id"]) in {"reading_unit_heading", "cycle_heading"}
    )
    return occurrences, units, {
        "role_counts": dict(sorted(role_counts.items())), "included_unit_count": len(units),
        "phrase_units": phrase_units,
    }


def build_ru_db(path: Path, occurrences: list[dict[str, Any]]) -> bytes:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(prefix=".ru-lexical-", suffix=".sqlite3", dir=path.parent)
    os.close(descriptor)
    temporary = Path(name)
    try:
        db = sqlite3.connect(temporary)
        db.executescript("""
        PRAGMA journal_mode=DELETE;
        CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL) WITHOUT ROWID;
        CREATE TABLE occurrences(occurrence_id TEXT PRIMARY KEY,unit_id TEXT NOT NULL,reading_ref TEXT NOT NULL,part_order INTEGER NOT NULL,role TEXT NOT NULL,token_ordinal INTEGER NOT NULL,start_offset INTEGER NOT NULL,end_offset INTEGER NOT NULL,exact_form TEXT NOT NULL,exact_form_sha256 TEXT NOT NULL,normalized_form TEXT NOT NULL,normalized_form_sha256 TEXT NOT NULL,analysis_key TEXT NOT NULL,analysis_key_sha256 TEXT NOT NULL) WITHOUT ROWID;
        CREATE INDEX occurrence_analysis_idx ON occurrences(analysis_key);
        CREATE INDEX occurrence_unit_idx ON occurrences(unit_id,token_ordinal);
        CREATE VIRTUAL TABLE unit_fts USING fts5(unit_id UNINDEXED, exact_text, normalized_text, tokenize='unicode61 remove_diacritics 0');
        """)
        db.executemany("INSERT INTO occurrences VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)", [(
            x["occurrence_id"], x["unit_id"], x["reading"], x["part"], x["role"],
            x["ordinal"], x["start"], x["end"], x["surface"], x["exact_sha256"],
            x["normalized"], x["normalized_sha256"], x["analysis_key"], x["analysis_key_sha256"]
        ) for x in occurrences])
        grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for x in occurrences:
            grouped[x["unit_id"]].append(x)
        db.executemany("INSERT INTO unit_fts VALUES(?,?,?)", [
            (unit, " ".join(x["surface"] for x in rows), " ".join(x["analysis_key"] for x in rows))
            for unit, rows in sorted(grouped.items())
        ])
        db.executemany("INSERT INTO metadata VALUES(?,?)", [
            ("authority_boundary", "private mechanical occurrence search; no text acceptance, lemma, lexeme, translation, sign, concept, or canon"),
            ("plan_sha256", digest_file(REPO / PLAN_REF)),
            ("occurrence_count", str(len(occurrences))),
        ])
        db.commit()
        db.execute("VACUUM")
        db.close()
        os.chmod(temporary, 0o600)
        payload = temporary.read_bytes()
    finally:
        temporary.unlink(missing_ok=True)
    return payload


def german_statistics() -> tuple[dict[str, Any], dict[str, Counter[int]], dict[str, set[str]], dict[str, Counter[str]]]:
    db = sqlite3.connect(REPO / DE_DB)
    rows = db.execute("""
      SELECT o.normalized_form,o.exact_form,s.part_order,o.section_resource_id,count(*)
      FROM occurrences o JOIN source_items s USING(item_ref)
      GROUP BY o.normalized_form,o.exact_form,s.part_order,o.section_resource_id
    """).fetchall()
    db.close()
    part_counts: dict[str, Counter[int]] = defaultdict(Counter)
    sections: dict[str, set[str]] = defaultdict(set)
    surfaces: dict[str, Counter[str]] = defaultdict(Counter)
    for key, surface, part, section, count in rows:
        part_counts[key][part] += count
        surfaces[key][surface] += count
        if section:
            sections[key].add(f"{part}:{section}")
    return {"token_count": sum(sum(x.values()) for x in part_counts.values()), "form_count": len(part_counts)}, part_counts, sections, surfaces


def russian_statistics(occurrences: list[dict[str, Any]]) -> tuple[dict[str, Counter[int]], dict[str, set[str]], dict[str, Counter[str]], Counter[str]]:
    part_counts: dict[str, Counter[int]] = defaultdict(Counter)
    readings: dict[str, set[str]] = defaultdict(set)
    surfaces: dict[str, Counter[str]] = defaultdict(Counter)
    fold_changes: Counter[str] = Counter()
    for x in occurrences:
        key = x["normalized"]
        part_counts[key][x["part"]] += 1
        readings[key].add(x["reading"])
        surfaces[key][x["surface"]] += 1
        if x["normalized"] != x["analysis_key"]:
            fold_changes[key] += 1
    return part_counts, readings, surfaces, fold_changes


def unit_statistics(units: list[dict[str, Any]], language: str) -> tuple[dict[str, Counter[int]], dict[str, set[str]]]:
    part_counts: dict[str, Counter[int]] = defaultdict(Counter)
    ranges: dict[str, set[str]] = defaultdict(set)
    for unit in units:
        for key in unit["tokens"]:
            if len(key) < 3 or not script_clean(key, language):
                continue
            part_counts[key][unit["part"]] += 1
            ranges[key].add(unit.get("reading", unit["unit_id"]))
    return part_counts, ranges


def keyword_rows(language: str, part_counts: dict[str, Counter[int]], ranges: dict[str, Any], stop: set[str], fold_changes: Counter[str] | None = None) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    total_parts = Counter()
    for counts in part_counts.values():
        total_parts.update(counts)
    all_total = sum(total_parts.values())
    tracked, private = [], []
    for key, counts in part_counts.items():
        total = sum(counts.values())
        if total < 5 or len(key) < 3 or key in stop:
            continue
        ent = entropy(counts)
        prominence = math.log1p(total) * (0.35 + 0.65 * ent)
        changed = 0 if fold_changes is None else fold_changes[key]
        status = "ambiguous" if changed * 2 > total else "proposed"
        common = {
            "schema_version": "tos_lexical_keyword_candidate_v1", "language": language,
            "form_key_sha256": h(key), "occurrence_count": total,
            "part_range": len(counts), "range_count": len(ranges.get(key, [])),
            "part_entropy_millionths": million(ent),
            "global_recurrence_prominence_millionths": round(prominence * 1_000_000),
            "part_occurrence_counts": [
                {"part_order": part, "occurrence_count": counts[part]}
                for part in sorted(counts)
            ],
            "status": status, "semantic_sufficiency": False,
        }
        tracked.append(common)
        private.append({**common, "analysis_key": key, "part_counts": dict(sorted(counts.items()))})
        for part in range(1, 5):
            c = counts[part]
            if c < 3:
                continue
            rest_c = total - c
            rest_n = all_total - total_parts[part]
            odds = math.log((c + .5) / (total_parts[part] - c + .5)) - math.log((rest_c + .5) / (rest_n - rest_c + .5))
            if odds <= 0:
                continue
            private[-1].setdefault("part_salience", {})[str(part)] = round(odds * math.log1p(c), 6)
    tracked.sort(key=lambda x: (x["language"], -x["global_recurrence_prominence_millionths"], x["form_key_sha256"]))
    private.sort(key=lambda x: (x["language"], -x["global_recurrence_prominence_millionths"], x["analysis_key"]))
    return tracked, private


def phrase_rows(language: str, units: list[dict[str, Any]], stop: set[str]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    counts: Counter[tuple[str, ...]] = Counter()
    parts: dict[tuple[str, ...], set[int]] = defaultdict(set)
    unit_hits: dict[tuple[str, ...], set[str]] = defaultdict(set)
    for unit in units:
        seq = unit["tokens"]
        for n in (2, 3):
            for i in range(len(seq) - n + 1):
                gram = tuple(seq[i:i+n])
                if any(len(x) < 2 or not script_clean(x, language) for x in gram):
                    continue
                if all(x in stop for x in gram):
                    continue
                counts[gram] += 1
                parts[gram].add(unit["part"])
                unit_hits[gram].add(unit["unit_id"])
    tracked, private = [], []
    for gram, count in counts.items():
        if count < 3:
            continue
        status = "proposed" if count >= 5 and len(unit_hits[gram]) >= 3 else "deferred"
        row = {
            "schema_version": "tos_lexical_repeated_sequence_candidate_v1", "language": language,
            "sequence_sha256": h("\n".join(gram)), "token_length": len(gram),
            "occurrence_count": count, "unit_range": len(unit_hits[gram]),
            "part_range": len(parts[gram]), "status": status,
            "semantic_sufficiency": False,
        }
        tracked.append(row)
        private.append({**row, "sequence": list(gram)})
    tracked.sort(key=lambda x: (x["language"], -x["occurrence_count"], x["sequence_sha256"]))
    private.sort(key=lambda x: (x["language"], -x["occurrence_count"], x["sequence"]))
    return tracked, private


def association_rows(units: list[dict[str, Any]], issuance: dict[str, str] | None) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[str]]:
    universe = [x for x in units if x["status"] == "proposed" and x["candidate_universe_eligible"]]
    positive = [x for x in units if x["status"] == "proposed" and x["positive_evidence_eligible"]]
    risk = [x for x in units if x["status"] != "proposed"]
    df_de, df_ru = Counter(), Counter()
    universe_pairs, pairs, strict_pairs, risk_pairs = Counter(), Counter(), Counter(), Counter()
    universe_evidence: dict[tuple[str, str], list[str]] = defaultdict(list)
    evidence: dict[tuple[str, str], list[str]] = defaultdict(list)
    readings: dict[tuple[str, str], Counter[str]] = defaultdict(Counter)
    shapes: dict[tuple[str, str], Counter[str]] = defaultdict(Counter)
    for row in universe:
        de = {x for x in row["de"] if len(x) >= 3 and x not in DE_STOP and script_clean(x, "de")}
        ru = {x for x in row["ru"] if len(x) >= 3 and x not in RU_STOP and script_clean(x, "ru")}
        for pair in ((a, b) for a in de for b in ru):
            universe_pairs[pair] += 1
            universe_evidence[pair].append(row["alignment_id"])
    for row in positive:
        de = {x for x in row["de"] if len(x) >= 3 and x not in DE_STOP and script_clean(x, "de")}
        ru = {x for x in row["ru"] if len(x) >= 3 and x not in RU_STOP and script_clean(x, "ru")}
        df_de.update(de); df_ru.update(ru)
        for pair in ((a, b) for a in de for b in ru):
            pairs[pair] += 1
            evidence[pair].append(row["alignment_id"])
            readings[pair][row["reading"]] += 1
            shapes[pair][row["shape"]] += 1
            if row["strict"]:
                strict_pairs[pair] += 1
    for row in risk:
        de = {x for x in row["de"] if len(x) >= 3 and x not in DE_STOP and script_clean(x, "de")}
        ru = {x for x in row["ru"] if len(x) >= 3 and x not in RU_STOP and script_clean(x, "ru")}
        risk_pairs.update((a, b) for a in de for b in ru)
    raw: list[tuple[str, str, dict[str, Any]]] = []
    n = len(positive)
    for (de, ru), universe_support in universe_pairs.items():
        if universe_support < 4:
            continue
        support = pairs[(de, ru)]
        if support and df_de[de] and df_ru[ru]:
            pdr = support / df_de[de]
            prd = support / df_ru[ru]
            dice = 2 * support / (df_de[de] + df_ru[ru])
            pmi = math.log((support * n) / (df_de[de] * df_ru[ru]))
            pmi_bits = pmi / math.log(2)
            denom = -math.log(support / n)
            npmi = 1.0 if denom == 0 else pmi / denom
            max_reading_share = max(readings[(de, ru)].values()) / support
        else:
            pdr = prd = dice = 0.0
            pmi_bits = -1_000.0
            npmi = -1.0
            max_reading_share = 1.0
        strict_support = strict_pairs[(de, ru)]
        risk_support = risk_pairs[(de, ru)]
        risk_share = risk_support / (support + risk_support) if support + risk_support else 1.0
        metrics = {
            "identity_universe_support": universe_support,
            "support": support, "source_unit_frequency": df_de[de], "target_unit_frequency": df_ru[ru],
            "strict_support": strict_support, "risk_support": risk_support,
            "source_to_target_precision_millionths": million(pdr),
            "target_to_source_precision_millionths": million(prd),
            "dice_millionths": million(dice), "npmi_millionths": million(npmi),
            "pmi_millibits": round(pmi_bits * 1000),
            "risk_share_millionths": million(risk_share),
            "reading_range": len(readings[(de, ru)]),
            "maximum_single_reading_share_millionths": million(max_reading_share),
            "correspondence_shapes": dict(sorted(shapes[(de, ru)].items())),
        }
        raw.append((de, ru, metrics))
    by_de: dict[str, list[tuple[str, str, dict[str, Any]]]] = defaultdict(list)
    by_ru: dict[str, list[tuple[str, str, dict[str, Any]]]] = defaultdict(list)
    for row in raw:
        by_de[row[0]].append(row); by_ru[row[1]].append(row)
    rank_score = lambda x: (-x[2]["dice_millionths"] * math.log1p(x[2]["support"]) * max(x[2]["pmi_millibits"], 0), -x[2]["support"], x[0], x[1])
    de_rank = {}
    ru_rank = {}
    for values in by_de.values():
        for rank, row in enumerate(sorted(values, key=rank_score), 1): de_rank[(row[0], row[1])] = rank
    for values in by_ru.values():
        for rank, row in enumerate(sorted(values, key=rank_score), 1): ru_rank[(row[0], row[1])] = rank
    eligible: list[tuple[str, str, dict[str, Any]]] = []
    for de, ru, metrics in raw:
        metrics["source_candidate_rank"] = de_rank[(de, ru)]
        metrics["target_candidate_rank"] = ru_rank[(de, ru)]
        proposed_gate = (
            metrics["support"] >= 8 and metrics["dice_millionths"] >= 200_000
            and metrics["pmi_millibits"] >= 2_000
            and metrics["source_candidate_rank"] <= 3 and metrics["target_candidate_rank"] <= 3
            and metrics["strict_support"] >= 4 and metrics["risk_share_millionths"] <= 200_000
            and metrics["maximum_single_reading_share_millionths"] <= 500_000
            and "many_to_many" not in metrics["correspondence_shapes"]
        )
        ambiguous_gate = (
            metrics["dice_millionths"] >= 100_000 and metrics["pmi_millibits"] >= 1_000
            and metrics["source_candidate_rank"] <= 5 and metrics["target_candidate_rank"] <= 5
        )
        metrics["status"] = "proposed" if proposed_gate else "ambiguous" if ambiguous_gate else "deferred"
        eligible.append((de, ru, metrics))
    eligible.sort(key=lambda x: (-x[2]["support"], -x[2]["identity_universe_support"], -x[2]["dice_millionths"], h(x[0]), h(x[1])))
    bindings = [f"{h(de)}|{h(ru)}" for de, ru, _ in eligible]
    tracked, private = [], []
    for de, ru, metrics in eligible:
        binding = f"{h(de)}|{h(ru)}"
        candidate_id = None if issuance is None else issuance[binding]
        row = {
            "schema_version": "tos_translation_surface_association_candidate_v1",
            "candidate_id": candidate_id, "source_language": "de", "target_language": "ru",
            "source_form_key_sha256": h(de), "target_form_key_sha256": h(ru),
            **metrics, "supporting_alignment_refs": evidence[(de, ru)],
            "identity_universe_alignment_refs": universe_evidence[(de, ru)],
            "candidate_kind": "translation_surface_association_candidate",
            "semantic_seed_posture": "proposal_not_lexical_equivalence_sign_or_concept",
            "accepted": False, "review_refs": [], "graph_effect": False,
        }
        tracked.append(row)
        private.append({**row, "source_form": de, "target_form": ru})
    return tracked, private, bindings


def issue(bindings: list[str]) -> dict[str, str]:
    path = REPO / ISSUANCE_REF
    if path.exists():
        raise BuildError("identity issuance exists; refusing remint")
    rows = [{"binding": x, "id": f"tos.lexical-association-candidate.sid-{secrets.token_hex(16)}"} for x in bindings]
    payload = {
        "schema_version": "tos_zarathustra_parallel_lexical_candidate_identity_issuance_v1",
        "issuance_id": "tos.identity-issuance.zarathustra-parallel-lexical-candidates-v1",
        "issued_on": "2026-09-02", "opaque_identity": True,
        "binding_is_not_identity": True, "candidate_count": len(rows), "identities": rows,
    }
    _write(path, jbytes(payload))
    return {x["binding"]: x["id"] for x in rows}


def load_issuance(bindings: list[str]) -> dict[str, str]:
    data = load_json(ISSUANCE_REF)
    rows = data["identities"]
    if set(x["binding"] for x in rows) != set(bindings) or len(rows) != len(bindings):
        raise BuildError("candidate identity binding drift")
    ids = [x["id"] for x in rows]
    if len(ids) != len(set(ids)):
        raise BuildError("candidate identity collision")
    return {x["binding"]: x["id"] for x in rows}


def _write(path: Path, payload: bytes, mode: int = 0o644) -> None:
    path = REPO / path
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    temp = Path(name)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(payload); handle.flush(); os.fsync(handle.fileno())
        os.chmod(temp, mode); os.replace(temp, path)
    finally:
        temp.unlink(missing_ok=True)


def generate(*, with_issuance: bool) -> tuple[dict[Path, bytes], dict[Path, tuple[bytes, int]], list[str], dict[str, Any]]:
    plan = load_json(PLAN_REF); verify_inputs(plan)
    parallel, de_parallel_units, _ru_parallel_units, private_layer_refs = load_parallel()
    ru_occ, ru_units, ru_meta = build_ru_observations()
    ru_phrase_units = ru_meta.pop("phrase_units")
    de_meta, de_counts, de_sections, de_surfaces = german_statistics()
    ru_exact_counts, ru_exact_readings, ru_surfaces, fold_changes = russian_statistics(ru_occ)
    ru_counts, ru_readings = unit_statistics(ru_units, "ru")
    de_kw, de_kw_private = keyword_rows("de", de_counts, de_sections, DE_KEYWORD_STOP)
    ru_kw, ru_kw_private = keyword_rows("ru", ru_counts, ru_readings, RU_KEYWORD_STOP)

    # Paragraphs cover parts I-III and prose in IV; add source-attested verse lines in IV.
    de_phrase_units = list(de_parallel_units)
    part4 = load_json(DE_TECH / "part-4.source-text-unit.v1.json")
    anchors = {x["anchor_ref"]: x for x in part4["anchors"]}
    layer = (REPO / part4["source_layer"]["text_layer_ref"]).read_text(encoding="utf-8")
    for unit in part4["units"]:
        if unit["unit_kind"] != "verse_line":
            continue
        anchor = anchors[unit["ordered_anchor_refs"][0]]; s = anchor["selector"]
        text = layer[s["start"]:s["end"]]
        if h(text) != anchor["exact_sha256"]:
            raise BuildError("German verse-line anchor drift")
        de_phrase_units.append({"part": 4, "unit_id": unit["unit_id"], "tokens": tokens(text, "de")})
    de_ph, de_ph_private = phrase_rows("de", de_phrase_units, DE_STOP)
    ru_ph, ru_ph_private = phrase_rows("ru", ru_phrase_units, RU_STOP)

    provisional, assoc_private, bindings = association_rows(parallel, None)
    issuance = load_issuance(bindings) if with_issuance else None
    associations, assoc_private, _ = association_rows(parallel, issuance)

    ru_rows = []
    for key, counts in sorted(ru_exact_counts.items(), key=lambda x: h(x[0])):
        ru_rows.append({
            "schema_version": "tos_russian_exact_form_recurrence_projection_v1",
            "normalized_form_sha256": h(key), "analysis_form_sha256": h(ru_key(key)),
            "occurrence_count": sum(counts.values()),
            "part_range": len(counts), "reading_range": len(ru_exact_readings[key]),
            "part_counts": [{"part_order": p, "occurrence_count": counts[p]} for p in sorted(counts)],
            "orthographic_folded_occurrence_count": fold_changes[key],
            "source_string_tracked": False, "semantic_sufficiency": False,
        })
    status_counts = Counter(x["status"] for x in associations)
    alignment_quality = {
        language: {
            field: sum(x[f"{language}_quality"][field] for x in parallel)
            for field in (
                "exact_token_count", "one_letter_token_count", "spaced_letter_run_count",
                "line_join_candidate_count", "mixed_script_token_count",
            )
        }
        for language in ("de", "ru")
    }
    summary = {
        "schema_version": "tos_zarathustra_parallel_lexical_candidate_summary_v1",
        "status": "completed-mechanical-candidate-observation-no-promotion",
        "parts": 4, "german_token_occurrences": de_meta["token_count"],
        "german_normalized_form_count": de_meta["form_count"],
        "russian_exact_token_occurrences": len(ru_occ), "russian_analysis_form_count": len(ru_counts),
        "russian_included_unit_count": ru_meta["included_unit_count"],
        "keyword_candidate_count": len(de_kw) + len(ru_kw),
        "keyword_candidates_by_language": {"de": len(de_kw), "ru": len(ru_kw)},
        "repeated_sequence_count": len(de_ph) + len(ru_ph),
        "repeated_sequences_by_language": {"de": len(de_ph), "ru": len(ru_ph)},
        "parallel_alignment_units": len(parallel),
        "parallel_status_counts": dict(sorted(Counter(x["status"] for x in parallel).items())),
        "quality_deferred_alignment_units": sum(not x["positive_evidence_eligible"] for x in parallel),
        "association_candidate_count": len(associations),
        "association_status_counts": dict(sorted(status_counts.items())),
        "accepted_candidate_count": 0, "review_count": 0, "graph_effect": False,
        "semantic_equivalence_asserted": False,
    }
    private_analysis = {
        "schema_version": "tos_zarathustra_parallel_lexical_private_analysis_v1",
        "source_bearing": True, "mode": "0600", "summary": summary,
        "russian_role_census": ru_meta["role_counts"],
        "alignment_quality_census": alignment_quality,
        "keywords": de_kw_private + ru_kw_private,
        "repeated_sequences": de_ph_private + ru_ph_private,
        "translation_surface_associations": assoc_private,
        "surface_variants": {
            "de": {h(k): dict(v.most_common()) for k, v in de_surfaces.items() if k in {x["analysis_key"] for x in de_kw_private}},
            "ru": {h(k): dict(v.most_common()) for k, v in ru_surfaces.items()
                   if ru_key(k) in {x["analysis_key"] for x in ru_kw_private}},
        },
        "authority_boundary": plan["authority_boundary"],
    }
    outputs: dict[Path, bytes] = {
        OUTPUTS["russian_recurrence"]: jlbytes(ru_rows),
        OUTPUTS["keywords"]: jlbytes(de_kw + ru_kw),
        OUTPUTS["phrases"]: jlbytes(de_ph + ru_ph),
        OUTPUTS["associations"]: jlbytes(associations),
        OUTPUTS["summary"]: jbytes(summary),
    }
    ru_db_bytes = build_ru_db(REPO / PRIVATE_DB, ru_occ)
    private: dict[Path, tuple[bytes, int]] = {
        PRIVATE_DB: (ru_db_bytes, 0o600), PRIVATE_ANALYSIS: (jbytes(private_analysis), 0o600),
    }
    coverage = {
        "schema_version": "tos_zarathustra_parallel_lexical_candidate_coverage_v1",
        "parts_complete": 4, "russian_logical_rows_reconstructed": sum(ru_meta["role_counts"].values()),
        "russian_included_roles": plan["scope"]["russian_included_roles"],
        "russian_exact_occurrence_count": len(ru_occ), "russian_occurrence_ids_unique": True,
        "german_existing_lexical_occurrence_count": de_meta["token_count"],
        "paragraph_alignment_units_consumed": len(parallel),
        "proposed_positive_evidence_units": sum(
            x["status"] == "proposed" and x["positive_evidence_eligible"] for x in parallel
        ),
        "quality_deferred_alignment_units": sum(not x["positive_evidence_eligible"] for x in parallel),
        "quality_deferred_proposed_units": sum(
            x["status"] == "proposed" and not x["positive_evidence_eligible"] for x in parallel
        ),
        "ambiguous_risk_units": sum(x["status"] == "ambiguous" for x in parallel),
        "deferred_risk_units": sum(x["status"] == "deferred" for x in parallel),
        "private_layer_refs_read": sorted(private_layer_refs),
        "private_outputs_mode": "0600", "tracked_source_strings": False,
        "accepted_candidate_count": 0, "semantic_equivalence_asserted": False,
    }
    outputs[OUTPUTS["coverage"]] = jbytes(coverage)
    provenance = [{
        "schema_version": "tos_provenance_event_v1",
        "event_id": "tos.event.zarathustra-parallel-lexical-candidates-v1.build",
        "event_type": "mechanical_lexical_candidate_materialization",
        "occurred_at": "2026-09-02T00:15:00-06:00", "agent_ref": "codex-internal-agents.lexical-candidate-v1",
        "software_ref": str(GENERATOR_REF), "software_sha256": digest_file(REPO / GENERATOR_REF),
        "plan_ref": str(PLAN_REF), "plan_sha256": digest_file(REPO / PLAN_REF),
        "authority_boundary": plan["authority_boundary"],
    }]
    outputs[OUTPUTS["provenance"]] = jlbytes(provenance)
    manifest = {
        "schema_version": "tos_zarathustra_parallel_lexical_candidate_manifest_v1",
        "plan_ref": str(PLAN_REF), "plan_sha256": digest_file(REPO / PLAN_REF),
        "identity_issuance_ref": str(ISSUANCE_REF),
        "identity_issuance_sha256": digest_file(REPO / ISSUANCE_REF) if (REPO / ISSUANCE_REF).exists() else None,
        "generated_outputs": {str(path): {"sha256": digest_bytes(payload), "byte_size": len(payload)} for path, payload in sorted(outputs.items(), key=lambda x: str(x[0]))},
        "private_outputs": {str(path): {"sha256": digest_bytes(payload), "byte_size": len(payload), "required_mode": "0600"} for path, (payload, _mode) in sorted(private.items(), key=lambda x: str(x[0]))},
        "source_text_included": False, "semantic_equivalence_asserted": False,
        "accepted_candidate_count": 0, "canon_effect": False,
    }
    outputs[OUTPUTS["manifest"]] = jbytes(manifest)
    return outputs, private, bindings, private_analysis


def run_build(issue_identities: bool) -> dict[str, Any]:
    outputs, private, bindings, analysis = generate(with_issuance=False)
    if issue_identities:
        issue(bindings)
    issuance = load_issuance(bindings)
    outputs, private, _bindings, analysis = generate(with_issuance=True)
    for path, payload in outputs.items():
        _write(path, payload)
    for path, (payload, mode) in private.items():
        _write(path, payload, mode)
    return analysis["summary"]


def run_check() -> dict[str, Any]:
    outputs, private, bindings, analysis = generate(with_issuance=True)
    load_issuance(bindings)
    for path, expected in outputs.items():
        actual = REPO / path
        if not actual.is_file() or actual.read_bytes() != expected:
            raise BuildError(f"tracked parity mismatch: {path}")
    for path, (expected, mode) in private.items():
        actual = REPO / path
        if not actual.is_file() or actual.read_bytes() != expected:
            raise BuildError(f"private parity mismatch: {path}")
        if stat.S_IMODE(actual.stat().st_mode) != mode:
            raise BuildError(f"private mode mismatch: {path}")
    return analysis["summary"]


def preview() -> dict[str, Any]:
    _outputs, _private, _bindings, analysis = generate(with_issuance=False)
    top = sorted(analysis["translation_surface_associations"], key=lambda x: (-x["support"], -x["dice_millionths"]))[:25]
    return {"summary": analysis["summary"], "top_associations": [
        {k: x[k] for k in ("source_form", "target_form", "support", "dice_millionths", "npmi_millionths", "status")}
        for x in top
    ]}


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
            result = run_build(args.issue_identities)
        else:
            if args.issue_identities:
                raise BuildError("--issue-identities is valid only with --build")
            result = run_check()
    except (BuildError, OSError, sqlite3.Error, json.JSONDecodeError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr); return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True)); return 0


if __name__ == "__main__":
    raise SystemExit(main())
