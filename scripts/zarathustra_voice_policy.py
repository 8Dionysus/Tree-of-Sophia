"""Conservative, offset-preserving reporting cues for the Zarathustra reader.

This is not coreference or a syntactic parser. Explicit grammatical-neighbour
subjects are returned as candidates; pronouns stay unresolved. Reading-local
policies must resolve 'self', 'old_man', 'saint' and pronouns, never a global
nearest-name substitution. Source offsets always address the supplied string.
"""

from __future__ import annotations

import re
from typing import Any


_JOIN = re.compile(r"[¬\u00ad-]\s*\n\s*")
_SPACE = re.compile(r"\s+")


def _search_surface(text: str) -> tuple[str, list[int]]:
    """Make a lookup surface while preserving every retained codepoint address."""
    removed: set[int] = set()
    for match in _JOIN.finditer(text):
        removed.update(range(match.start(), match.end()))
    chars: list[str] = []
    positions: list[int] = []
    for i, char in enumerate(text):
        if i in removed:
            continue
        char = {"ſ": "s", "ѣ": "е", "Ѣ": "Е", "і": "и", "І": "И", "ѵ": "и"}.get(char, char)
        if char.isspace():
            if chars and chars[-1] == " ":
                continue
            char = " "
        chars.append(char)
        positions.append(i)
    return "".join(chars), positions


_DE_SUBJECTS = (
    ("right_king", r"(?:der\s+)?König\s+zur\s+Rechten"),
    ("left_king", r"(?:der\s+)?König\s+zur\s+Linken"),
    ("conscientious_one", r"(?:der\s+)?(?:Gewissenhafte(?:\s+des\s+Geistes)?|Getretene|Blutende|Gefragte)"),
    ("voluntary_beggar", r"(?:der\s+)?(?:freiwillige\s+Bettler|Berg-Prediger|Friedfertige)"),
    ("ugliest_man", r"(?:der\s+)?(?:hässlichste\s+Mensch|Unaussprechliche)"),
    ("magician", r"(?:(?:der|dieser)\s+)?(?:(?:alte|kluge)\s+)?Zauberer"),
    ("pope", r"(?:der\s+)?(?:alte\s+)?Papst"),
    ("shadow", r"(?:der\s+)?(?:Wanderer\s+und\s+Schatten|Schatten|Wanderer)"),
    ("soothsayer", r"(?:der\s+)?(?:alte\s+)?Wahrsager"),
    ("animals", r"(?:(?:die|seine|meine)\s+)?Thiere"),
    ("old_woman", r"(?:das\s+)?alte\s+Weiblein"),
    ("stillest_hour", r"(?:meine\s+)?stillste\s+Stunde"),
    ("zarathustra", r"Zarathustra"),
    ("dwarf", r"(?:der\s+)?(?:Zwerg|Geist\s+der\s+Schwere)"),
    ("sage", r"(?:der\s+)?Weise"),
    ("saint", r"(?:der\s+)?Heilige"),
    ("old_man", r"(?:der\s+)?(?:Greis|Alte|alte\s+Mann)"),
    ("youth", r"(?:der\s+)?Jüngling"),
    ("disciples", r"(?:seine|die|meine)\s+Jünger"),
    ("disciple", r"(?:der\s+)?Jünger"),
    ("hunchback", r"(?:der\s+)?Bucklichte"),
    ("fire_dog", r"(?:der\s+)?Feuerhund"),
    ("adder", r"(?:die\s+)?Natter"),
    ("ass", r"(?:der\s+)?Esel"),
    ("life", r"(?:das\s+)?Leben"),
    ("wisdom", r"(?:(?:meine|die|seine)\s+)?(?:(?:wilde|lachende|weise)\s+)?Weisheit"),
    ("solitude", r"(?:die\s+)?Einsamkeit"),
    ("soul", r"(?:(?:meine|die|seine)\s+)?Seele"),
    ("crowd", r"(?:das\s+)?(?:Volk|die\s+Menge)"),
    ("herd", r"(?:die\s+)?Heerde"),
    ("self", r"ich"),
    ("unresolved", r"er|sie|es|dieser|jener|der\s+Andere|der\s+andre\s+König"),
)

_RU_SUBJECTS = (
    ("right_king", r"(?:правый\s+король|король\s+(?:направо|справа|по\s+правую\s+(?:руку|сторону)))"),
    ("left_king", r"(?:левый\s+король|король\s+(?:налево|слева|по\s+левую\s+(?:руку|сторону)))"),
    ("conscientious_one", r"(?:добросовестный|совестливый|совестный)(?:\s+духом[ъь]?)?|растоптанный|пострадавший"),
    ("voluntary_beggar", r"добровольный\s+нищий|миролюбивый|горный\s+проповедник[ъь]?"),
    ("ugliest_man", r"самый\s+безобразный\s+человек[ъь]?|безобразнейший|невыразимый"),
    ("magician", r"(?:(?:старый|хитрый)\s+)?(?:чародей|волшебник[ъь]?)"),
    ("pope", r"(?:старый\s+)?папа"),
    ("shadow", r"странник[ъь]?\s+и\s+тень|тень|странник[ъь]?"),
    ("soothsayer", r"(?:старый\s+)?прорицатель"),
    ("animals", r"(?:(?:его|мои|свои)\s+)?звери"),
    ("old_woman", r"старуха|старушка"),
    ("zarathustra", r"Заратустра"),
    ("dwarf", r"карлик[ъь]?|дух[ъь]?\s+тяжести"),
    ("sage", r"мудрец[ъь]?"),
    ("saint", r"святой"),
    ("old_man", r"старец[ъь]?|старик[ъь]?"),
    ("youth", r"юноша"),
    ("disciples", r"(?:его\s+)?ученики"),
    ("disciple", r"ученик[ъь]?"),
    ("hunchback", r"горбатый|горбун[ъь]?"),
    ("fire_dog", r"огненный\s+пес[ъь]?"),
    ("adder", r"змея"),
    ("ass", r"осел[ъь]?"),
    ("life", r"жизнь"),
    ("wisdom", r"(?:моя\s+)?мудрость"),
    ("solitude", r"уединение|одиночество"),
    ("soul", r"(?:(?:моя|его)\s+)?душа"),
    ("crowd", r"народ[ъь]?|толпа"),
    ("self", r"я"),
    ("unresolved", r"он[ъь]?|она|оно|они|этот[ъь]?|тот[ъь]?"),
)

_VERBS = {
    "de": r"(?:sprach(?:en)?|spricht|sagt(?:e|en)?|antwortet(?:e|en)?|entgegnet(?:e|en)?|erwidert(?:e|en)?|ruft|rief(?:en)?|redet(?:e|en)?|flüstert(?:e|en)?|schrie(?:n|en)?|schreit|sang(?:en)?|fragt(?:e|en)?|raunt(?:e|en)?|knurrt(?:e|en)?|murmelt(?:e|en)?|brummt(?:e|en)?|dacht(?:e|en)|denkt)",
    "ru": r"(?:говорил[аи]?|говорит[ъь]?|сказал[аи]?|сказали|ответил[аи]?|отвечал[аи]?|воскликнул[аи]?|крикнул[аи]?|кричал[аи]?|спросил[аи]?|шептал[аи]?|прошептал[аи]?|подумал[аи]?|думал[аи]?|пел[аи]?|возразил[аи]?|восклицал[аи]?)[ъь]?",
}
_MODIFIERS = {
    "de": r"(?:(?:aber|nun|also|endlich|hier|da|abermals|nochmals|weiter|leise|zornig|traurig|lachend|heftig|unwillig|bitter|verächtlich|ihm|mir|ihr|dann|so|erheitert|erschreckt)\s+){0,4}",
    "ru": r"(?:(?:же|тут[ъь]?|здесь|ему|ей|мне|он[ъь]?|снова|наконец[ъь]?|еще|тогда|тихо|громко|печально|сердито|опять|усмехаясь)\s+){0,3}",
}


def find_reporting_cues(text: str, language: str) -> list[dict[str, Any]]:
    """Return conservative subject/reporting-verb matches with exact offsets.

    'self' and 'unresolved' MUST be resolved against reading-local discourse,
    not interpreted as Zarathustra. A cue can describe reported/hypothetical
    speech and therefore does not by itself change the current utterer.
    """
    if language not in _VERBS:
        return []
    surface, positions = _search_surface(text)
    subjects = _DE_SUBJECTS if language == "de" else _RU_SUBJECTS
    verb, mods = _VERBS[language], _MODIFIERS[language]
    found: list[dict[str, Any]] = []
    for role, subject in subjects:
        for direction, expression in (
            ("subject_after_verb", rf"\b(?P<verb>{verb})\s+{mods}(?P<subject>{subject})\b"),
            ("subject_before_verb", rf"\b(?P<subject>{subject})\s+{mods}(?P<verb>{verb})\b"),
        ):
            for match in re.finditer(expression, surface, re.IGNORECASE):
                vstart, vend = match.span("verb")
                sstart, send = match.span("subject")
                found.append({
                    "start": positions[match.start()],
                    "end": positions[match.end() - 1] + 1,
                    "verb_start": positions[vstart],
                    "verb_end": positions[vend - 1] + 1,
                    "subject_start": positions[sstart],
                    "subject_end": positions[send - 1] + 1,
                    "role": role,
                    "status": "ambiguous" if role in {"unresolved", "self", "old_man", "saint"} else "proposed",
                    "direction": direction,
                    "method": "explicit_subject_adjacent_to_reporting_verb_v1",
                })
    # Prefer full role names to embedded generic subjects; retain ambiguity
    # when different explicit candidates claim the same verb at equal span.
    found.sort(key=lambda row: (row["verb_start"], -(row["end"] - row["start"]), row["role"]))
    result: list[dict[str, Any]] = []
    for row in found:
        same_verb = [old for old in result if old["verb_start"] == row["verb_start"]]
        if any(old["start"] <= row["start"] and row["end"] <= old["end"] for old in same_verb):
            continue
        result.append(row)
    return sorted(result, key=lambda row: (row["start"], row["end"], row["role"]))
