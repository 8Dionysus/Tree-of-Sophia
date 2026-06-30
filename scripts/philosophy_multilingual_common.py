#!/usr/bin/env python3
"""Shared multilingual display helpers for ToS philosophy projections."""

from __future__ import annotations

import json
import re
from functools import lru_cache
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
LEDGER_REF = "ToS/philosophy/atlas/multilingual/content-labels.json"
LEDGER_PATH = REPO_ROOT / LEDGER_REF


def content_language_contract() -> dict[str, Any]:
    ledger = load_label_ledger()
    return {
        "schema_version": "tos_multilingual_content_contract_v1",
        "source_ref": LEDGER_REF,
        "display_languages": ledger["display_languages"],
        "required_translation_languages": ledger["required_translation_languages"],
        "original_language_rule": ledger["original_language_rule"],
        "downstream_consumer_rule": ledger["downstream_consumer_rule"],
    }


@lru_cache(maxsize=1)
def load_label_ledger() -> dict[str, Any]:
    payload = json.loads(LEDGER_PATH.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{LEDGER_REF} must contain a JSON object")
    if payload.get("schema_version") != "tos_philosophy_multilingual_labels_v1":
        raise ValueError(f"{LEDGER_REF} has unexpected schema_version")
    return payload


def _label_sets() -> dict[str, Any]:
    sets = load_label_ledger().get("label_sets")
    return sets if isinstance(sets, dict) else {}


def _exact_labels() -> dict[str, dict[str, str]]:
    labels = _label_sets().get("exact_labels")
    return labels if isinstance(labels, dict) else {}


def _dossier_titles() -> dict[str, dict[str, str]]:
    labels = _label_sets().get("dossier_titles")
    return labels if isinstance(labels, dict) else {}


def _dossier_id(value: str) -> str | None:
    match = re.search(r"\b(A\d{2})\b", value)
    return match.group(1) if match else None


def _clean_dossier_prefix(value: str) -> tuple[str, bool, str | None]:
    text = value.strip()
    docx = text.lower().endswith(".docx")
    if docx:
        text = re.sub(r"\.docx$", "", text, flags=re.IGNORECASE)
    prefix = None
    for candidate in ("ToS Deep Research:", "ToS Deep Research_", "Corpus Or Prepared Source Document:"):
        if text.startswith(candidate):
            prefix = candidate
            text = text[len(candidate) :].strip()
            break
    return text, docx, prefix


def _dossier_title(value: str, language: str) -> tuple[str | None, str | None]:
    dossier_id = _dossier_id(value)
    if not dossier_id:
        return None, None
    title = _dossier_titles().get(dossier_id)
    if not isinstance(title, dict) or not title.get(language):
        return None, None
    cleaned, docx, prefix = _clean_dossier_prefix(value)
    del cleaned
    translated_prefix = ""
    if prefix == "ToS Deep Research:" or prefix == "ToS Deep Research_":
        translated_prefix = "ToS Deep Research: "
    elif prefix == "Corpus Or Prepared Source Document:":
        translated_prefix = "Corpus Or Prepared Source Document: "
    return f"{translated_prefix}{dossier_id} — {title[language]}{'.docx' if docx else ''}", "reviewed"


def _exact_label(value: str, language: str) -> tuple[str | None, str | None]:
    exact = _exact_labels().get(value)
    if isinstance(exact, dict) and exact.get(language):
        return str(exact[language]), "reviewed"
    return None, None


def _apply_english_draft_replacements(value: str) -> str:
    translated = value
    for pattern, replacement in _DRAFT_REPLACEMENTS:
        translated = pattern.sub(replacement, translated)
    translated = re.sub(r"\s+—\s+", ": ", translated)
    translated = re.sub(r"(^|[\s/—-])и(?=$|[\s/—-])", r"\1and", translated, flags=re.IGNORECASE)
    translated = re.sub(r"(^|[\s/—-])как(?=$|[\s/—-])", r"\1as", translated, flags=re.IGNORECASE)
    return re.sub(r"\s+", " ", translated).strip()


def _translate_component(value: str, language: str) -> tuple[str, str]:
    text = value.strip()
    exact, status = _exact_label(text, language)
    if exact:
        return exact, status or "reviewed"
    dossier, status = _dossier_title(text, language)
    if dossier:
        return dossier, status or "reviewed"
    if language == "en" and re.search(r"[А-Яа-яЁё]", text):
        return _apply_english_draft_replacements(text) or text, "draft"
    return text, "source"


def _compound_label(value: str, language: str) -> tuple[str | None, str | None]:
    if ":" not in value:
        return None, None
    prefix, remainder = value.split(":", 1)
    translated_prefix, prefix_status = _translate_component(prefix, language)
    translated_remainder, remainder_status = _translate_component(remainder, language)
    if translated_prefix == prefix.strip() and translated_remainder == remainder.strip():
        return None, None
    status = "draft" if "draft" in {prefix_status, remainder_status} else "reviewed"
    return f"{translated_prefix}: {translated_remainder}", status


_DRAFT_REPLACEMENTS: tuple[tuple[re.Pattern[str], str], ...] = tuple(
    (re.compile(pattern, re.IGNORECASE), replacement)
    for pattern, replacement in (
        (r"до\s*н\.?\s*э\.?", "BCE"),
        (r"н\.?\s*э\.?", "CE"),
        (r"тысячелетия|тыс\.", "millennium"),
        (r"вв\.", "centuries"),
        (r"в\.", "century"),
        (r"Западная Азия", "West Asia"),
        (r"Северная Африка", "North Africa"),
        (r"Южная Азия", "South Asia"),
        (r"Центральная Азия", "Central Asia"),
        (r"Восточная Азия", "East Asia"),
        (r"Юго-Восточная Азия", "Southeast Asia"),
        (r"Египетский|Египетская", "Egyptian"),
        (r"Египет", "Egypt"),
        (r"позднеегипетская|поздний|поздняя", "late"),
        (r"ранний|ранняя", "early"),
        (r"многоязычный", "multilingual"),
        (r"многоязычие", "multilingualism"),
        (r"письменная фиксация", "written fixation"),
        (r"письменный|письменное", "written"),
        (r"писцовая этика", "scribal ethics"),
        (r"школьная словесность", "school literature"),
        (r"мудрость", "wisdom"),
        (r"право|закон", "law"),
        (r"ритуал", "ritual"),
        (r"храмовая ученость", "temple scholarship"),
        (r"ученость", "scholarship"),
        (r"комментарий", "commentary"),
        (r"корпусы", "corpora"),
        (r"корпус", "corpus"),
        (r"знаки", "signs"),
        (r"печати", "seals"),
        (r"предел реконструкции", "limits of reconstruction"),
        (r"клинописный слой", "cuneiform layer"),
        (r"клинопись", "cuneiform"),
        (r"трехъязычие", "trilingualism"),
        (r"царские надписи", "royal inscriptions"),
        (r"надписи", "inscriptions"),
        (r"эпиграфика", "epigraphy"),
        (r"санскритские", "Sanskrit"),
        (r"палийские", "Pali"),
        (r"буддизм", "Buddhism"),
        (r"джайнизм", "Jainism"),
        (r"дисциплина", "discipline"),
        (r"шастра", "shastra"),
        (r"даршаны", "darshanas"),
        (r"канон", "canon"),
        (r"Авеста", "Avesta"),
        (r"ахеменидские", "Achaemenid"),
        (r"Маат", "Ma'at"),
        (r"Мани", "Mani"),
        (r"манихейство", "Manichaeism"),
        (r"Коптский", "Coptic"),
        (r"Эламский", "Elamite"),
        (r"Элам", "Elam"),
        (r"Индская цивилизация", "Indus Civilization"),
        (r"Хеттское|Хеттский", "Hittite"),
        (r"Левантская", "Levantine"),
        (r"иврито-арамейский", "Hebrew-Aramaic"),
        (r"документальный мир", "documentary world"),
        (r"Вторая храмовая Иудея", "Second Temple Judea"),
        (r"Шан", "Shang"),
        (r"ранний Чжоу", "Early Zhou"),
        (r"Ста школ", "Hundred Schools"),
        (r"Имперское конфуцианство", "Imperial Confucianism"),
        (r"Ведийско-брахманическая традиция", "Vedic-Brahmanical tradition"),
        (r"ранние Упанишады", "Early Upanishads"),
        (r"Шраманские традиции", "Shramana traditions"),
        (r"Тхеравада", "Theravada"),
        (r"палийского канона", "Pali Canon"),
        (r"Древний Иран", "Ancient Iran"),
        (r"Герметические", "Hermetic"),
        (r"гностические", "Gnostic"),
        (r"мандеистские", "Mandaean"),
        (r"поздней античности", "Late Antiquity"),
        (r"до ислама", "before Islam"),
    )
)


def english_label(value: str) -> tuple[str, str]:
    text = value.strip()
    exact, status = _exact_label(text, "en")
    if exact:
        return exact, status or "reviewed"
    compound, status = _compound_label(text, "en")
    if compound:
        return compound, status or "reviewed"
    dossier, status = _dossier_title(text, "en")
    if dossier:
        return dossier, status or "reviewed"
    if not re.search(r"[А-Яа-яЁё]", text):
        return text, "source"
    translated = _apply_english_draft_replacements(text)
    return translated or text, "draft"


def russian_label(value: str) -> tuple[str, str]:
    text = value.strip()
    exact, status = _exact_label(text, "ru")
    if exact:
        return exact, status or "reviewed"
    compound, status = _compound_label(text, "ru")
    if compound:
        return compound, status or "reviewed"
    dossier, status = _dossier_title(text, "ru")
    if dossier:
        return dossier, status or "reviewed"
    return text, "source"


def multilingual_label(label: str, source_ref: str, properties: dict[str, Any] | None = None) -> dict[str, Any]:
    props = properties or {}
    original = props.get("original_label") or props.get("original_title") or props.get("attested_original")
    transliteration = props.get("transliteration") or props.get("title_transliteration")
    original_language = props.get("original_language") or props.get("language")
    original_script = props.get("original_script") or props.get("script")
    if not isinstance(original, str) or not original.strip():
        original = None
    if not isinstance(transliteration, str) or not transliteration.strip():
        transliteration = None
    if not isinstance(original_language, str) or not original_language.strip():
        original_language = None
    if not isinstance(original_script, str) or not original_script.strip():
        original_script = None
    ru, ru_status = russian_label(label)
    en, en_status = english_label(label)
    original_status = "source" if original else "pending"
    if props.get("node_type") in {"domain-root", "atlas", "atlas-section", "view-section"}:
        original_status = "not_applicable"
    return {
        "schema_version": "tos_multilingual_label_v1",
        "label": {
            "original": original,
            "ru": ru,
            "en": en,
        },
        "language": {
            "original_language": original_language,
            "original_script": original_script,
            "transliteration": transliteration,
        },
        "translation_status": {
            "original": original_status,
            "ru": ru_status,
            "en": en_status,
        },
        "source_ref": source_ref,
    }
