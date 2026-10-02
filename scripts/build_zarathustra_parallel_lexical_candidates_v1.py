#!/usr/bin/env python3
"""Native-only Parallel producer entry; retained Python consumer helpers.

These exact helpers remain executable compatibility for Python Concept and
Eternal Concept. The full prior producer recipe is archived separately and
must never be imported or executed as a maintained fallback.
"""
from __future__ import annotations
import os
import shutil
import hashlib
import importlib.util
import json
import re
import stat
import sys
import unicodedata
from collections import Counter
from pathlib import Path
from typing import Any

COMMAND = "zarathustra-parallel-lexical-candidates-v1"

REPO = Path(__file__).resolve().parents[1]


WORK = Path("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra")


ALIGN = WORK / "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1"


RU_TECH = WORK / "technical-markup/antonovsky-1911-structural-paragraph-v2"


RU_BUILDER = Path("scripts/build_antonovsky_1911_structural_paragraph_v2.py")


LETTER = "A-Za-zÄÖÜäöüßẞА-Яа-яЁёІіЇїѢѣѲѳѴѵ"


TOKEN_RE = re.compile(rf"[{LETTER}]+(?:[’'][{LETTER}]+)*")


LINE_JOIN_RE = re.compile(rf"([{LETTER}]{{2,}})[-¬]\s*\n\s*([{LETTER}]{{2,}})")


SPACED_RE = re.compile(rf"(?<![{LETTER}])((?:[{LETTER}]\s+){{2,}}[{LETTER}])(?![{LETTER}])")


RU_FOLD = str.maketrans({"ѣ": "е", "і": "и", "ї": "и", "ѳ": "ф", "ѵ": "и"})


class BuildError(RuntimeError):
    pass


def h(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads((REPO / path).read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise BuildError(f"object required: {path}")
    return value


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in (REPO / path).read_text(encoding="utf-8").splitlines()]


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


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    # The dedicated native research parser accepts separate option/value tokens.
    args = [token for arg in args for token in (
        ["--source-root", arg.split("=", 1)[1]]
        if arg.startswith("--source-root=") else [arg]
    )]
    native = os.environ.get("TOS_NATIVE_PREPARED_CONSUMER_BIN") or shutil.which("tos")
    if not native or not Path(native).is_absolute():
        print("error: select installed tos through TOS_NATIVE_PREPARED_CONSUMER_BIN or PATH", file=sys.stderr)
        return 1
    command = [native, COMMAND]
    if args not in (["--help"], ["-h"]) and not any(
        arg == "--source-root" or arg.startswith("--source-root=") for arg in args
    ):
        command += ["--source-root", str(REPO)]
    command += args
    try:
        os.execv(native, command)
    except OSError as exc:
        print(f"error: cannot execute native tos: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
