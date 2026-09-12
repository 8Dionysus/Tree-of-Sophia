#!/usr/bin/env python3
"""Build a private source-returnable material library for the local constructor.

This is presentation data, not a new corpus contract or semantic projection.
All 210 entries of the existing eternal-return evidence dossier are retained,
including ambiguous, excluded, and one-sided entries. Exact witness text is
read only from anchored local inputs, never embedded in this tracked builder.
The three existing EN demo translations are imported from their private packet;
every other EN view explicitly displays untranslated German.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
from xml.etree import ElementTree as ET


WORK = Path("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra")
CANDIDATE = Path("ToS/candidate-intake/zarathustra/eternal-return-concept-candidate-v1")
ALIGNMENT_DIR = WORK / "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1"
ANALYSIS = WORK / "gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/eternal-return-analysis.v1.json"
DE_CITATIONS = WORK / "technical-markup/dta-first-editions-parts-1-4-v1/citation-spine.v1.jsonl"
RU_PARAGRAPHS = WORK / "technical-markup/antonovsky-1911-structural-paragraph-v2/paragraph-spine.v2.jsonl"
RU_STRUCTURE = WORK / "technical-markup/antonovsky-1911-structural-paragraph-v2/structure-spine.v2.jsonl"
ANNOTATION = "tos.annotation.eternal-return-candidate.sid-5cbb125d0d411355a3b40aeefa71de2f"
WORK_ID = "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
ROMAN = {1: "I", 2: "II", 3: "III", 4: "IV"}
DE_YEARS = {1: 1883, 2: 1883, 3: 1884, 4: 1891}
DEMO_IDS = {
    "tos.annotation.semantic-evidence-candidate.sid-7ebaa9e0fcebf5be121f19eda32fa6ee": "moment",
    "tos.annotation.semantic-evidence-candidate.sid-22f2b9f4f255c2745006082db8c28909": "all-things",
    "tos.annotation.semantic-evidence-candidate.sid-ccdeb398711d81a0b4999a00748e39d2": "same-life",
}


def bi(ru: str, en: str) -> dict[str, str]:
    return {"ru": ru, "en": en}


# Hand-authored navigation labels, not transcriptions of the historical RU
# translation. Each key must resolve to its actual German source chapter.
CHAPTER_LABELS = {
    "p1.r1": bi("Предисловие Заратустры", "Zarathustra’s Prologue"),
    "p1.r4": bi("О потусторонниках", "On the Otherworldly"),
    "p1.r10": bi("О проповедниках смерти", "On the Preachers of Death"),
    "p1.r13": bi("О базарных мухах", "On the Flies of the Marketplace"),
    "p1.r15": bi("О друге", "On the Friend"),
    "p1.r16": bi("О тысяче и одной цели", "On a Thousand and One Goals"),
    "p1.r23": bi("О дарящей добродетели", "On the Giving Virtue"),
    "p2.r5": bi("О добродетельных", "On the Virtuous"),
    "p2.r10": bi("Танцевальная песнь", "The Dance Song"),
    "p2.r11": bi("Надгробная песнь", "The Tomb Song"),
    "p2.r15": bi("О непорочном познании", "On Immaculate Knowledge"),
    "p2.r17": bi("О поэтах", "On Poets"),
    "p2.r19": bi("Прорицатель", "The Soothsayer"),
    "p2.r20": bi("Об избавлении", "On Redemption"),
    "p2.r22": bi("Самый тихий час", "The Stillest Hour"),
    "p3.r1": bi("Странник", "The Wanderer"),
    "p3.r2": bi("О видении и загадке", "On the Vision and the Riddle"),
    "p3.r3": bi("О блаженстве против воли", "On Bliss Against One’s Will"),
    "p3.r4": bi("Перед восходом солнца", "Before Sunrise"),
    "p3.r5": bi("Об умаляющей добродетели", "On the Diminishing Virtue"),
    "p3.r8": bi("Об отступниках", "On Apostates"),
    "p3.r9": bi("Возвращение домой", "The Homecoming"),
    "p3.r10": bi("О трёх злых", "On the Three Evils"),
    "p3.r12": bi("О старых и новых скрижалях", "On Old and New Tablets"),
    "p3.r13": bi("Выздоравливающий", "The Convalescent"),
    "p3.r15": bi("Другая танцевальная песнь", "The Other Dance Song"),
    "p3.r16": bi("Семь печатей", "The Seven Seals"),
    "p4.r1": bi("Медовое приношение", "The Honey Offering"),
    "p4.r3": bi("Беседа с королями", "Conversation with the Kings"),
    "p4.r6": bi("В отставке", "Retired"),
    "p4.r9": bi("Тень", "The Shadow"),
    "p4.r10": bi("В полдень", "At Noon"),
    "p4.r15": bi("О науке", "On Science"),
    "p4.r17": bi("Пробуждение", "The Awakening"),
    "p4.r18": bi("Праздник осла", "The Ass Festival"),
    "p4.r19": bi("Песнь ночного странника", "The Night Wanderer’s Song"),
}

CLASS_NOTES = {
    "core": bi("Основной материал исследовательского досье: здесь отмечена явная формула возвращения. Это отбор для рассмотрения, а не принятое толкование.", "Core material in the research dossier: an explicit recurrence formulation was identified here. This is a selection for consideration, not an accepted interpretation."),
    "supporting": bi("Сопутствующий материал досье: фрагмент включён для чтения образов времени, круга, жизни или утверждения. Связь с понятием ещё требует рассмотрения.", "Supporting material in the dossier: the passage was included to examine images of time, the circle, life, or affirmation. Its relation to the concept remains open to review."),
    "ambiguous": bi("Неоднозначный материал: словесное соседство делает фрагмент интересным для сравнения, но само по себе не подтверждает вечное возвращение.", "Ambiguous material: verbal proximity makes the passage useful for comparison, but does not by itself establish eternal recurrence."),
    "excluded": bi("Контрольный фрагмент: в досье это местное возвращение исключено из положительных свидетельств вечного возвращения. Его сохранение помогает различать значения.", "Control passage: the dossier excludes this local return from positive evidence for eternal recurrence. Keeping it visible helps distinguish meanings."),
}

SPEAKER_LABELS = {
    "animals_eagle_and_serpent": bi("Звери — орёл и змея", "The animals — eagle and serpent"),
    "dwarf": bi("Карлик", "The dwarf"),
    "external_narrator": bi("Внешний повествователь", "External narrator"),
    "mixed_external_narrator_and_zarathustra": bi("Повествователь и Заратустра", "Narrator and Zarathustra"),
    "paratext_heading": bi("Заголовок", "Heading"),
    "spirit_of_gravity_as_dwarf_voice": bi("Дух тяжести в голосе карлика", "The spirit of gravity in the dwarf’s voice"),
    "ugliest_man": bi("Самый безобразный человек", "The ugliest man"),
    "zarathustra": bi("Заратустра", "Zarathustra"),
    "zarathustra_as_storyteller": bi("Заратустра как рассказчик", "Zarathustra as storyteller"),
    "zarathustra_midnight_song_voice": bi("Полуночная песнь Заратустры", "Zarathustra’s midnight song"),
    "zarathustra_song_voice": bi("Песенный голос Заратустры", "Zarathustra’s singing voice"),
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def rows(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def build(repo: Path, demo_path: Path) -> tuple[dict, dict]:
    def path(relative: str | Path) -> Path:
        candidate = (repo / relative).resolve()
        require(candidate.is_relative_to(repo), "Source reference escapes the source repository")
        return candidate

    def ref(label: str, relative: str | Path, pointer: str = "") -> dict:
        return {"label": label, "ref": str(path(relative)) + ("#" + pointer if pointer else "")}

    def node(identity: str, kind: str, parent: str | None, title: dict, body: dict, refs: list[dict], **more) -> dict:
        return {"id": identity, "kind": kind, "parentId": parent, "title": title, "body": body, "sourceRefs": refs, **more}

    work = load(path(WORK / "work.json"))
    require(work["record_id"] == WORK_ID, "Unexpected work identity")
    dossier = load(path(CANDIDATE / "concept-candidate.v1.json"))
    require(dossier["annotation_id"] == ANNOTATION and dossier["review_status"] == "unreviewed", "Dossier identity or review posture changed")
    require(dossier["body"]["concept_id"] is None and not dossier["body"]["graph_effect"] and not dossier["body"]["canon_effect"], "Library requires the existing research-candidate posture")
    private = load(path(ANALYSIS))
    evidence_rows = rows(path(CANDIDATE / "evidence-spine.v1.jsonl"))
    evidence = {entry["evidence_id"]: entry for entry in evidence_rows}
    private_evidence = {entry["evidence_id"]: entry for entry in private["evidence"]}
    require(len(evidence) == len(evidence_rows) == 210 and set(evidence) == set(private_evidence), "Expected all 210 unique dossier evidence units")
    for identity, entry in evidence.items():
        require(all(private_evidence[identity].get(key) == value for key, value in entry.items()), "Tracked evidence and private analysis disagree")
        require(not entry["accepted"] and not entry["graph_effect"] and not entry["canon_effect"], "Evidence posture changed")
    de_citations = {entry["unit_id"]: entry for entry in rows(path(DE_CITATIONS))}
    ru_paragraphs = {entry["paragraph_unit_id"]: entry for entry in rows(path(RU_PARAGRAPHS))}
    ru_structures = {entry["structure_unit_id"]: entry for entry in rows(path(RU_STRUCTURE))}
    speaker_entries = {entry["evidence_ref"]: entry for entry in rows(path(CANDIDATE / "review-preparation-v1/speaker-attribution-candidates.v1.jsonl"))}
    demo = load(demo_path)
    require(demo["schema"] == "tos_local_story_demo_v1" and demo["id"] == "eternal-return", "Unexpected source demo packet")
    demo_nodes = {entry["id"]: entry for entry in demo["steps"]}
    packets: dict[int, dict] = {}
    anchor_maps: dict[tuple[int, str], dict] = {}
    layer_cache: dict[Path, str] = {}
    xml_cache: dict[Path, ET.Element] = {}
    checked_anchor_count = 0
    readings = Counter(entry["reading_ref"] for entry in evidence_rows)
    chapter_keys = set(readings) - {"p2.rNone"}
    require(chapter_keys == set(CHAPTER_LABELS), "Chapter coverage changed; review presentation labels")
    part_counts = Counter(entry["part"] for entry in evidence_rows)
    class_counts = Counter(entry["evidence_class"] for entry in evidence_rows)

    for part in range(1, 5):
        packet = load(path(ALIGNMENT_DIR / f"part-{part}.translation-alignment-packet.v1.json"))
        require(packet["rights_and_visibility"]["effective_visibility"] == "local_only" and not packet["rights_and_visibility"]["publication_authorized"], "Source visibility changed; review library privacy")
        packets[part] = packet
        for side in ["source_side", "target_side"]:
            anchor_maps[(part, side)] = {entry["anchor_ref"]: (i, entry) for i, entry in enumerate(packet[side]["anchors"])}

    def source_text(part: int, side: str, anchor_id: str) -> tuple[str, list[dict], str]:
        nonlocal checked_anchor_count
        index, anchor = anchor_maps[(part, side)][anchor_id]
        layer = path(anchor["text_layer_ref"])
        if layer not in layer_cache:
            text = layer.read_text(encoding="utf-8")
            require(sha(text) == anchor["text_layer_sha256"], "Source text-layer digest mismatch")
            layer_cache[layer] = text
        else:
            require(sha(layer_cache[layer]) == anchor["text_layer_sha256"], "Conflicting source layer binding")
        selector = anchor["selector"]
        require(selector["type"] == "text_position" and selector["position_unit"] == "unicode_code_point" and selector["interval"] == "half_open", "Unsupported source selector")
        require(0 <= selector["start"] <= selector["end"] <= len(layer_cache[layer]), "Source selector is outside its layer")
        exact = layer_cache[layer][selector["start"]:selector["end"]]
        require(sha(exact) == anchor["exact_sha256"], "Exact anchor digest mismatch")
        checked_anchor_count += 1
        packet_ref = ALIGNMENT_DIR / f"part-{part}.translation-alignment-packet.v1.json"
        lang = "DE" if side == "source_side" else "RU"
        return exact, [ref(f"{lang} · {anchor_id}", packet_ref, f"/{side}/anchors/{index}")], anchor["source_return"]["locator_ref"]

    def source_heading(raw_return: str, locator: str) -> tuple[str, str]:
        file_ref = raw_return.split("#", 1)[0]
        xml_path = path(file_ref)
        if xml_path not in xml_cache:
            xml_cache[xml_path] = ET.parse(xml_path).getroot()
        xpath = "/".join("t:" + item for item in locator.split("/")[1:])
        element = xml_cache[xml_path].find(xpath, {"t": "http://www.tei-c.org/ns/1.0"})
        require(element is not None, "Chapter locator does not resolve in the German source")
        heading = element.find("{http://www.tei-c.org/ns/1.0}head")
        require(heading is not None, "Chapter has no source heading")
        return " ".join("".join(heading.itertext()).split()), file_ref

    nodes = [node("work", "work", None, bi("Так говорил Заратустра", "Thus Spoke Zarathustra"), bi(
        "Фридрих Ницше. В библиотеке — 210 выбранных фрагментов из четырёх частей книги. Открывайте главы, сравнивайте тексты и добавляйте собственные мысли к прочитанному.",
        "Friedrich Nietzsche. The library offers 210 selected passages from the book’s four parts. Open chapters, compare texts, and add your own thoughts to what you read."
    ), [ref(WORK_ID, WORK / "work.json"), ref(ANNOTATION, CANDIDATE / "concept-candidate.v1.json"), ref("210 · coverage", CANDIDATE / "coverage-receipt.v1.json")], sourceNote=bi(
        "Фридрих Ницше. Локальная библиотека объединяет 210 выбранных свидетельств из исследовательского досье вечного возвращения: 36 глав четырёх частей и один внеглавный эпиграф. Это весь отбор досье, а не полный текст книги. Названия глав переведены для навигации; точные тексты сохраняют собственные языки и источники.",
        "Friedrich Nietzsche. This local library contains all 210 selected evidence units from the eternal-return research dossier: 36 chapters across four parts and one epigraph outside the chapter structure. It is the complete dossier selection, not the complete book. Chapter titles are translated for navigation; exact texts retain their own languages and sources."
    ))]

    for part in range(1, 5):
        packet_ref = ALIGNMENT_DIR / f"part-{part}.translation-alignment-packet.v1.json"
        packet = packets[part]
        chapter_count = sum(key.startswith(f"p{part}.") for key in chapter_keys)
        nodes.append(node(f"part-{part}", "part", "work", bi(f"Часть {ROMAN[part]}", f"Part {ROMAN[part]}"), bi(
            f"В этой части для чтения открыты {chapter_count} глав. В библиотеке: {part_counts[part]} фрагментов. Выберите главу, чтобы приблизиться к тексту.",
            f"Explore {chapter_count} chapters from this part, with {part_counts[part]} passages in the library. Choose a chapter to approach the text."
        ), [ref(packet["source_side"]["expression_ref"], packet_ref, "/source_side"), ref(packet["target_side"]["expression_ref"], packet_ref, "/target_side")], sourceNote=bi(
            f"{part_counts[part]} выбранных фрагментов. Немецкое свидетельство — издание {DE_YEARS[part]} года, TEI DTA; русский слой — техническое извлечение перевода Антоновского 1911 года. Сопоставления предложены для рассмотрения и не устанавливают принятую эквивалентность переводов.",
            f"{part_counts[part]} selected passages. The German witness is the {DE_YEARS[part]} edition in DTA TEI; the Russian layer is a technical extraction of Antonovsky’s 1911 translation. Alignments are proposals for consideration, not accepted translation equivalences."
        )))

    # Chapter containment comes from the source citation spine. The dossier's
    # reading labels must agree with that structure; they never author it.
    chapter_info: dict[str, dict] = {}
    for reading in sorted(chapter_keys, key=lambda key: tuple(map(int, re.fullmatch(r"p(\d+)\.r(\d+)", key).groups()))):
        selected = [entry for entry in evidence_rows if entry["reading_ref"] == reading]
        entry = selected[0]
        part = entry["part"]
        source_units = [de_citations[unit] for row in selected for unit in row["source_paragraph_unit_refs"]]
        majors = {unit["nearest_major_unit_id"] for unit in source_units}
        require(len(majors) == 1 and None not in majors, "A presentation chapter crosses source chapter boundaries")
        chapter = de_citations[next(iter(majors))]
        reading_number = int(reading.split(".r")[1])
        # Part I's source correspondence also numbers the enclosing speeches
        # division. The reading dossier numbers its individual speeches after
        # the prologue, so their source correspondence is one higher.
        correspondence_number = reading_number + (1 if part == 1 and reading_number > 1 else 0)
        require(chapter["part_order"] == part and chapter["major_correspondence_sequence"] == correspondence_number, "Dossier reading does not match source-owned chapter")
        _, anchor = anchor_maps[(part, "source_side")][entry["source_anchor_refs"][0]]
        heading, xml_ref = source_heading(anchor["source_return"]["locator_ref"], chapter["source_locator"])
        ru_unit_refs = {ru_paragraphs[unit]["reading_unit_ref"] for row in selected for unit in row["target_paragraph_unit_refs"]}
        refs = [ref(chapter["unit_id"], DE_CITATIONS, "unit=" + chapter["unit_id"]), ref(heading, xml_ref, chapter["source_locator"])]
        for ru_unit in sorted(ru_unit_refs):
            structure = ru_structures[ru_unit]
            require(structure["part_id"] == f"part_{part}" and structure["reading_unit_ordinal_within_part"] == int(reading.split(".r")[1]), "Russian chapter correspondence differs from this presentation grouping")
            refs.append(ref(ru_unit, RU_STRUCTURE, "unit=" + ru_unit))
        identity = "chapter-" + reading
        chapter_info[reading] = {"id": identity, "unit": chapter["unit_id"]}
        nodes.append(node(identity, "chapter", f"part-{part}", CHAPTER_LABELS[reading], bi(
            f"Часть {ROMAN[part]} · фрагментов для чтения: {readings[reading]}. Откройте любой, чтобы читать и сравнивать.",
            f"Part {ROMAN[part]} · {readings[reading]} passages to explore. Open any passage to read and compare."
        ), refs, sourceNote=bi(
            f"{heading} Здесь собрано {readings[reading]} фрагментов, отобранных досье. Откройте их для чтения и сравнения. Русское и английское названия служат навигации; немецкий заголовок возвращает к конкретной главе исходного издания.",
            f"{heading} This chapter contains {readings[reading]} passages selected by the dossier. Open them for reading and comparison. Russian and English titles provide navigation; the German heading returns to the specific chapter in the source edition."
        )))

    def order(entry: dict) -> tuple:
        unit = de_citations[entry["source_paragraph_unit_refs"][0]]
        # Citation-spine insertion order preserves actual source order, including
        # headings and verse material whose kind-local ordinals can repeat.
        return entry["part"], source_order[unit["unit_id"]]

    source_order = {unit: index for index, unit in enumerate(de_citations)}
    reading_positions: Counter = Counter()
    translated_count = 0
    bilingual_count = 0
    for entry in sorted(evidence_rows, key=order):
        identity = entry["evidence_id"]
        part = entry["part"]
        reading = entry["reading_ref"]
        reading_positions[reading] += 1
        local_number = reading_positions[reading]
        raw = private_evidence[identity]
        packet = packets[part]
        mapping = next(item for item in packet["alignments"] if item["alignment_id"] == entry["alignment_ref"])
        require(mapping["status"] == entry["alignment_status"] and mapping["status"] != "accepted", "Alignment review status changed")
        exact: dict[str, str] = {}
        refs: list[dict] = []
        for lang, side, anchor_field, mapping_field in [
            ("de", "source_side", "source_anchor_refs", "ordered_source_anchor_refs"),
            ("ru", "target_side", "target_anchor_refs", "ordered_target_anchor_refs"),
        ]:
            require(mapping[mapping_field] == entry[anchor_field], "Evidence and mapping anchors disagree")
            pieces = []
            for anchor_id in entry[anchor_field]:
                piece, anchor_refs, _ = source_text(part, side, anchor_id)
                pieces.append(piece)
                refs.extend(anchor_refs)
            exact[lang] = "\n".join(pieces)
            require(sha(exact[lang]) == entry[f"{lang}_exact_sha256"] and exact[lang] == raw[f"{lang}_text"], "Evidence excerpt does not match its exact anchored bytes")
            if pieces:
                refs.append(ref(f"{lang.upper()} · SHA-256 {entry[f'{lang}_exact_sha256']}", CANDIDATE / "evidence-spine.v1.jsonl", "evidence=" + identity))
        bilingual_count += bool(exact["de"] and exact["ru"])
        de_units = [de_citations[unit] for unit in entry["source_paragraph_unit_refs"]]
        for unit in de_units:
            refs.append(ref(unit["display_citation"], DE_CITATIONS, "unit=" + unit["unit_id"]))
        for unit in entry["target_paragraph_unit_refs"]:
            refs.append(ref(ru_paragraphs[unit]["display_citation"], RU_PARAGRAPHS, "unit=" + unit))
        refs.append(ref(identity, CANDIDATE / "evidence-spine.v1.jsonl", "evidence=" + identity))
        refs.append(ref(entry["alignment_ref"], ALIGNMENT_DIR / f"part-{part}.translation-alignment-packet.v1.json", "alignment=" + entry["alignment_ref"]))
        if reading in chapter_info:
            require(all(unit["nearest_major_unit_id"] == chapter_info[reading]["unit"] for unit in de_units), "Fragment parent is not its source chapter")
            parent = chapter_info[reading]["id"]
        else:
            require(reading == "p2.rNone" and all(unit["nearest_major_unit_id"] is None for unit in de_units), "Unexpected material outside chapters")
            parent = f"part-{part}"
        short_id = DEMO_IDS.get(identity, identity)
        chapter_label = CHAPTER_LABELS.get(reading, bi("Эпиграф", "Epigraph"))
        # The Russian incipit is a bounded literal prefix of its witness, with
        # only leading/trailing display whitespace trimmed. No OCR repair or
        # historical spelling modernization belongs in a navigation title.
        incipit = exact["ru"].lstrip()
        ru_title = (incipit[:64].rstrip() + "…" if len(incipit) > 65 else incipit.rstrip()) if incipit else f"{chapter_label['ru']} · {local_number}"
        title = bi(ru_title, f"{chapter_label['en']} · {local_number}")
        quote = bi(exact["ru"] or exact["de"], exact["de"])
        note = bi(
            "Русский: точное техническое извлечение перевода Антоновского 1911 года; сохранены историческая орфография, переносы и возможные ошибки извлечения. Английский перевод отсутствует. При выборе EN показывается точный немецкий текст.",
            "English translation unavailable — showing the exact German witness. Russian text is the exact technical extraction of Antonovsky’s 1911 translation, retaining historical spelling, line breaks, and possible extraction errors."
        )
        if not exact["ru"]:
            note = bi(
                "В текущем сопоставлении нет русского фрагмента: показан точный немецкий текст. Этот пробел не доказывает отсутствия перевода в книге. Английский перевод также отсутствует.",
                "No Russian passage is present in this alignment: the exact German text is shown. This gap does not establish an omission in the translated book. English translation is also unavailable."
            )
        if identity in DEMO_IDS:
            previous = demo_nodes[short_id]
            require(previous["source"]["exact"] == exact, "Existing demo translation is bound to a different source excerpt")
            require(set(previous["quote"]) == {"ru", "en"} and all(previous["quote"].values()), "Existing bilingual demo quote is incomplete")
            quote, note = previous["quote"], previous["quoteNote"]
            title = previous["title"]
            translated_count += 1
        source_note = dict(CLASS_NOTES[entry["evidence_class"]])
        if not entry["positive_evidence_eligible"]:
            source_note = bi(source_note["ru"] + " Сопоставление одностороннее; как двуязычное подтверждение этот фрагмент не используется.", source_note["en"] + " The alignment is one-sided; this entry is not used as bilingual support.")
        if identity in DEMO_IDS:
            body = demo_nodes[short_id]["body"]
        elif reading in CHAPTER_LABELS:
            chapter_label = CHAPTER_LABELS[reading]
            body = bi(f"Из главы «{chapter_label['ru']}» · часть {ROMAN[part]}.", f"From {chapter_label['en']} · Part {ROMAN[part]}.")
        else:
            body = bi("Эпиграф ко второй части.", "Epigraph to Part II.")
        voice = speaker_entries.get(identity)
        if voice:
            base = SPEAKER_LABELS[voice["primary_role"]]
            speaker = bi(base["ru"] + " · предварительная атрибуция", base["en"] + " · provisional attribution")
            if "animals_voicing_a_hypothetical_zarathustra" in voice.get("alternative_roles", []):
                speaker = bi("Звери, в том числе передающие предполагаемые слова Заратустры · атрибуция открыта", "The animals, including their imagined words of Zarathustra · attribution remains open")
            refs.append(ref(voice["speaker_attribution_candidate_id"], CANDIDATE / "review-preparation-v1/speaker-attribution-candidates.v1.jsonl", "evidence=" + identity))
        else:
            speaker = bi("Голос не установлен в этом досье", "The speaker has not been identified in this dossier")
        nodes.append(node(short_id, "fragment", parent, title, body, refs, exact=exact, quote=quote, quoteNote=note, speaker=speaker, sourceNote=source_note))

    nodes.append(node("dossier", "dossier", "work", bi("Вечное возвращение · досье", "Eternal recurrence · dossier"), bi(
        "Исследовательский кандидат: 210 фрагментов и три открытых направления чтения — космологическое, экзистенциальное и поэтическое. Сравнивайте их опоры и развивайте собственное понимание.",
        "A research candidate: 210 passages and three open directions of reading — cosmological, existential, and poetic. Compare their evidence and develop your own understanding."
    ), [ref(ANNOTATION, CANDIDATE / "concept-candidate.v1.json"), ref("210 · coverage", CANDIDATE / "coverage-receipt.v1.json"), ref("3 · readings", CANDIDATE / "review-preparation-v1/interpretation-review-matrix.v1.json")], sourceNote=bi(
        "Исследовательский кандидат объединяет явные формулы, сопутствующие образы, неоднозначные места и исключённые контрольные примеры. Здесь открыты космологическое, экзистенциальное и поэтическое чтения. Ни одно из них не принято; concept_id не выдан, graph_effect=false и canon_effect=false. Свободные связи в вашем Древе выражают ваши собственные заметки и не меняют этот статус.",
        "This research candidate brings together explicit formulations, supporting images, ambiguous passages, and excluded controls. Cosmological, existential, and poetic readings remain open. None has been accepted; no concept_id has been issued, graph_effect=false and canon_effect=false. Free connections in your Tree express your own notes and do not change that status."
    )))

    require(bilingual_count == 206 and translated_count == 3, "Unexpected language coverage")
    identities = {item["id"] for item in nodes}
    require(len(identities) == len(nodes) == 252, "Library must contain 252 unique nodes")
    for item in nodes:
        require(item["parentId"] is None if item["id"] == "work" else item["parentId"] in identities, "Invalid structural parent")
        for field in ["title", "body", "sourceNote"] + (["quote", "quoteNote", "speaker"] if item["kind"] == "fragment" else []):
            require(set(item[field]) == {"ru", "en"} and all(item[field].values()), "Incomplete bilingual UI fields")
        for reference in item["sourceRefs"]:
            require(Path(reference["ref"].split("#", 1)[0]).is_file(), "Missing source reference")
    result = {"schema": "tos_constructor_library_v1", "rootId": "work", "nodes": nodes}
    # Canonical UTF-8 JSON: sorted object keys, no ASCII escaping or whitespace,
    # no trailing newline, and the fingerprint field itself excluded.
    result["fingerprint"] = sha(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    report = {"nodes": len(nodes), "kinds": dict(Counter(item["kind"] for item in nodes)), "evidence_units": len(evidence), "evidence_classes": dict(class_counts), "de_ru_pairs": bilingual_count, "de_only": len(evidence) - bilingual_count, "en_demo_translations": translated_count, "en_unavailable": len(evidence) - translated_count, "anchor_excerpts_checked": checked_anchor_count, "graph_effect": False, "canon_effect": False}
    return result, report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-repo", type=Path, default=Path("/srv/AbyssOS/Tree-of-Sophia"))
    parser.add_argument("--demo-packet", type=Path, default=Path("/srv/abyss-machine/storage/artifacts/tos-eternal-return-demo-20260910/demo-data.json"))
    parser.add_argument("--output", type=Path, default=Path("/srv/abyss-machine/storage/artifacts/tos-tree-constructor-20260910/library.json"))
    parser.add_argument("--check", action="store_true", help="Verify inputs and exact existing output bytes without writing")
    args = parser.parse_args()
    repo, output, demo = args.source_repo.resolve(), args.output.resolve(), args.demo_packet.resolve()
    builder_repo = Path(__file__).resolve().parents[3]
    require(output != demo, "Output must not overwrite the previous demo packet")
    for private_path in [output, demo]:
        require(not private_path.is_relative_to(repo) and not private_path.is_relative_to(builder_repo), "Private material must stay outside both source worktrees")
    data, report = build(repo, demo)
    serialized = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
    if args.check:
        require(output.read_text(encoding="utf-8") == serialized, "Existing library differs from reproducible output")
        require(output.stat().st_mode & 0o077 == 0, "Private library permissions are too broad")
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        fd = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            os.fchmod(stream.fileno(), 0o600)
            stream.write(serialized)
    print(json.dumps({"status": "checked" if args.check else "built", "output": str(output), "bytes": len(serialized.encode("utf-8")), "sha256": sha(serialized), **report}))


if __name__ == "__main__":
    main()
