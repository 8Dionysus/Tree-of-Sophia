"""Typed lexical normalization of reported registry fields; no owner admission.

The caller retains raw values and precise source locators. This module never
opens links, evaluates formulas, infers rights or identifies a work by title.
"""

from __future__ import annotations

import datetime as dt
import json
import re
from pathlib import Path
from urllib.parse import urlsplit


def load_profile(path: str | Path) -> dict:
    """Load an explicit adapter profile, rejecting ambiguous field definitions."""
    def unique_keys(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"Duplicate profile key: {key}")
            result[key] = value
        return result

    profile = json.loads(Path(path).read_text(encoding="utf-8"),
                         object_pairs_hook=unique_keys)
    if profile.get("version") != 1:
        raise ValueError("Unsupported registry normalization profile version")
    for kind in ("registry", "gaps"):
        if not isinstance(profile.get(kind), dict):
            raise ValueError(f"Missing field mapping for {kind}")
        for source, rule in profile[kind].items():
            if not source or not rule.get("target") or not rule.get("value_type"):
                raise ValueError(f"Incomplete field mapping: {kind}.{source}")
    return profile


def _result(value, status="normalized", unparsed=None, issues=None):
    return {"value": value, "normalization_status": status,
            "unparsed_fragments": unparsed or [], "issues": issues or []}


def _split_multivalue(text):
    """Split list separators outside balanced qualifiers; retain their text."""
    brackets = {"(": ")", "[": "]", "{": "}", "（": "）"}
    stack, fragments, start = [], [], 0
    for index, character in enumerate(text):
        if character in brackets:
            stack.append(brackets[character])
        elif stack and character == stack[-1]:
            stack.pop()
        elif not stack and character in ";|\n\r":
            fragments.append(text[start:index])
            start = index + 1
    fragments.append(text[start:])
    return fragments


def _lexical(text, vocabulary, multi=False):
    fragments = _split_multivalue(text) if multi else [text]
    values, unknown = [], []
    for fragment in fragments:
        fragment = fragment.strip()
        if not fragment:
            continue
        normalized = vocabulary.get(fragment.casefold())
        if normalized is None:
            unknown.append(fragment)
        else:
            values.append(normalized)
    value = values if multi else (values[0] if values else None)
    return _result(value, "partial" if unknown and values else
                   "reported_unparsed" if unknown else "normalized", unknown)


def _qualified_terms(text, vocabulary, code_key):
    """Recognize exact terms, retaining qualifiers and unmatched source text."""
    terms, unknown = [], []
    for fragment in _split_multivalue(text):
        fragment = fragment.strip()
        if not fragment:
            continue
        # A comma only separates a list if EVERY part is an exact known term.
        comma_parts = [p.strip() for p in fragment.split(",")]
        parts = (comma_parts if len(comma_parts) > 1 and
                 all(p.casefold() in vocabulary for p in comma_parts)
                 else [fragment])
        for part in parts:
            value = vocabulary.get(part.casefold())
            qualifiers = []
            if value is None:
                match = re.fullmatch(r"(.+?)\s*(\([^\n]+\)|[—–]\s*.+)", part)
                if match:
                    value = vocabulary.get(match[1].strip().casefold())
                    if value is not None:
                        qualifiers = [match[2].strip()]
            if value is None:
                unknown.append(part)
            else:
                facets = value.copy() if isinstance(value, dict) else {code_key: value}
                terms.append({**facets, "reported_fragment": part,
                              "qualifiers": [*facets.get("qualifiers", []), *qualifiers]})
    return _result(terms, "partial" if terms and unknown else
                   "reported_unparsed" if unknown else "normalized", unknown)


def _links(text):
    links, unknown, previous = [], [], 0
    # Source spans refer to the supplied field string, not a rewritten URL.
    # A semicolon inside a URL is valid. It is only a list separator when
    # followed by another URL or at the end of a whitespace-delimited token.
    pattern = r"https?://[^\s<>\"\u201c\u201d]+"
    for match in re.finditer(pattern, text, re.IGNORECASE):
        prefix = text[previous:match.start()].strip(" \t\r\n;|,")
        if prefix:
            unknown.append(prefix)
        token = match[0]
        pieces = re.split(r";(?=https?://)", token, flags=re.IGNORECASE)
        offset = match.start()
        for piece in pieces:
            url = piece.rstrip(";|,")
            # Trim prose punctuation only when it cannot be balanced by URL.
            while url.endswith(")") and url.count(")") > url.count("("):
                url = url[:-1]
            try:
                valid = bool(urlsplit(url).hostname)
            except ValueError:
                valid = False
            if valid:
                links.append({"url": url, "source_span": [offset, offset + len(url)]})
            else:
                unknown.append(piece)
            offset += len(piece) + 1
        previous = match.end()
    suffix = text[previous:].strip(" \t\r\n;|,")
    if suffix:
        unknown.append(suffix)
    return _result(links, "partial" if links and unknown else
                   "reported_unparsed" if unknown else "normalized", unknown)


def extract_reported_links(text):
    """Return source-addressed URL mentions, without access or rights claims."""
    return _links(text)["value"]


def _checked_date(raw, text):
    if isinstance(raw, dt.datetime):
        if raw.time() != dt.time(0) or raw.tzinfo is not None:
            return _result({"value": raw.isoformat(), "precision": "datetime"})
        raw = raw.date()
    if isinstance(raw, dt.date):
        return _result({"value": raw.isoformat(), "precision": "day"})
    if re.fullmatch(r"\d{4}-\d{2}-\d{2}", text):
        try:
            date = dt.date.fromisoformat(text)
        except ValueError:
            return _result(None, "reported_unparsed", [text], ["invalid_calendar_date"])
        return _result({"value": date.isoformat(), "precision": "day"})
    return _result(None, "reported_unparsed", [text], ["date_precision_unresolved"])


def _roman(text):
    table = {"I": 1, "V": 5, "X": 10, "L": 50, "C": 100}
    total, previous = 0, 0
    for char in text[::-1]:
        value = table[char]
        total += -value if value < previous else value
        previous = value
    # Canonical validation avoids accepting arbitrary Roman-looking strings.
    if not re.fullmatch(r"X{0,3}(IX|IV|V?I{0,3})", text):
        return None
    return total or None


def _temporal(text):
    statement = {"reported_statement": text, "expressions": []}
    if text.casefold() in ("не установлена", "unknown", "not established", "undated"):
        statement["reported_date_status"] = "not_established"
        return _result(statement)

    approximate = r"(?P<approx>c\.?\s*|ca\.?\s*|ок\.?\s*)?"
    era_pattern = r"BCE|BC|CE|AD|до\s+н\.\s*э\.|н\.\s*э\."

    def era_name(value):
        if not value:
            return "unspecified"
        return "BCE" if value.upper() in ("BC", "BCE") or value.casefold().startswith("до") else "CE"

    if re.fullmatch(r"\d{1,4}", text) and 1 <= int(text) <= 9999:
        statement["expressions"].append({"year": int(text), "precision": "year",
                                         "era": "unspecified", "scope": "unspecified"})
        return _result(statement)
    year = re.fullmatch(
        approximate + r"(?P<start>\d{1,4})(?:\s*[–—-]\s*(?P<end>\d{1,4}))?"
        + rf"(?:\s*(?P<era>{era_pattern})|\s*гг?\.)?", text, re.IGNORECASE)
    if year and int(year["start"]) > 0 and (not year["end"] or int(year["end"]) > 0):
        expression = {"year_start": int(year["start"]),
                      "year_end": int(year["end"] or year["start"]),
                      "precision": "year", "era": era_name(year["era"]),
                      "approximate": bool(year["approx"]), "scope": "unspecified"}
        statement["expressions"].append(expression)
        return _result(statement)

    millennium = re.fullmatch(
        approximate + r"(?P<number>first|second|third|\d{1,2}(?:st|nd|rd|th)?)"
        + rf"\s+millennium(?:\s*(?P<era>{era_pattern}))?", text, re.IGNORECASE)
    if millennium:
        ordinal = millennium["number"].casefold()
        number = {"first": 1, "second": 2, "third": 3}.get(ordinal)
        if number is None:
            number = int(re.match(r"\d+", ordinal)[0])
        if number > 0:
            statement["expressions"].append({"millennium": number, "precision": "millennium",
                                             "era": era_name(millennium["era"]),
                                             "approximate": bool(millennium["approx"]),
                                             "scope": "unspecified"})
            return _result(statement)
    # Limit interpretation to one standalone expression; preserve composite
    # work/witness/edition timelines as unparsed statements for later review.
    match = re.fullmatch(
        approximate + r"(?:(?P<start_part>early|mid|middle|late)\s+)?"
        r"(?P<start>\d{1,2}|[IVX]+)(?:st|nd|rd|th)?"
        r"(?:\s*[–—-]\s*(?:(?P<end_part>early|mid|middle|late)\s+)?"
        r"(?P<end>\d{1,2}|[IVX]+)(?:st|nd|rd|th)?)?"
        r"\s*(?:centur(?:y|ies)|c\.|в\.|вв\.)"
        rf"(?:\s*(?P<era>{era_pattern}))?",
        text, re.IGNORECASE)
    if match:
        def century(value):
            return int(value) if value.isdigit() else _roman(value.upper())

        start = century(match["start"])
        end = century(match["end"] or match["start"])
        if start and end:
            statement["expressions"].append({"century_start": start, "century_end": end,
                                             "precision": "century", "era": era_name(match["era"]),
                                             "approximate": bool(match["approx"]),
                                             "scope": "unspecified"})
            if match["start_part"]:
                statement["expressions"][-1]["start_part"] = match["start_part"].casefold()
            if match["end_part"]:
                statement["expressions"][-1]["end_part"] = match["end_part"].casefold()
            return _result(statement)
    return _result(statement, "reported_unparsed", [text])


def normalize_record_values(kind: str, raw_values: dict[str, object],
                            profile: dict) -> dict:
    """Normalize every supplied field with an explicit adapter mapping.

    Unknown fields fail closed; unknown lexical values remain visible.
    Empty fields are included. Multiple aliases sharing a target remain
    separate reported fields rather than overwriting one another.
    """
    if kind not in ("registry", "gaps") or kind not in profile:
        raise ValueError(f"Unsupported registry record kind: {kind}")
    unknown = sorted(set(raw_values) - set(profile[kind]))
    if unknown:
        raise ValueError(f"Unmapped {kind} fields: {unknown!r}")
    fields, issues = [], []
    vocabulary = profile.get("vocabulary", {})
    for source, raw in raw_values.items():
        rule = profile[kind][source]
        typ = rule["value_type"]
        field = {"source_field": source, "target": rule["target"], "value_type": typ}
        text = raw.strip() if isinstance(raw, str) else str(raw)
        if raw is None or (isinstance(raw, str) and not text):
            result = _result(None, "empty")
        elif not isinstance(raw, (str, int, float, bool, dt.date)):
            raise ValueError(f"Unsupported raw field type: {source}={type(raw).__name__}")
        elif typ == "date":
            result = _checked_date(raw, text)
        elif typ == "temporal_statement":
            result = _temporal(text)
        elif typ in ("access_modes", "use_tags"):
            result = _lexical(text, vocabulary[typ], multi=True)
        elif typ in ("object_kind", "relevance", "coverage", "confidence", "priority"):
            result = _lexical(text, vocabulary[typ])
        elif typ == "languages":
            result = _qualified_terms(text, vocabulary[typ], "language_code")
        elif typ == "formats":
            result = _qualified_terms(text, vocabulary[typ], "format")
        elif typ == "links":
            result = _links(raw if isinstance(raw, str) else text)
        elif typ == "reported_status":
            # Lexical labels are not state-machine transitions or verification.
            fragments = [p.strip() for p in _split_multivalue(text) if p.strip()]
            labels = [p for p in fragments if re.fullmatch(r"[A-Za-z][A-Za-z0-9_-]*", p)]
            unmatched = [p for p in fragments if p not in labels]
            result = _result({"reported_statement": text, "labels": labels},
                             "partial" if labels and unmatched else
                             "reported_unparsed" if unmatched else "normalized", unmatched)
        elif typ == "record_references":
            fragments = [p.strip() for p in re.split(r"[,;|\n\r]+", text) if p.strip()]
            references = [p for p in fragments if re.fullmatch(
                profile.get("record_reference_pattern", r"[A-Za-z0-9][A-Za-z0-9._-]*"), p)]
            unmatched = [p for p in fragments if p not in references]
            result = _result(references, "partial" if references and unmatched else
                             "reported_unparsed" if unmatched else "normalized", unmatched)
        elif typ == "text":
            result = _result(text)
        else:
            raise ValueError(f"Unsupported value type: {typ}")
        field.update(result)
        fields.append(field)
        for issue in result["issues"]:
            issues.append({"source_field": source, "issue": issue})
    return {"reported_fields": fields, "issues": issues}
