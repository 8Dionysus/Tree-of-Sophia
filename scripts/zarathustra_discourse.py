"""Source-preserving quotation and voice candidates; no accepted speaker claims.

Offsets always address the unchanged context text (Unicode code points). A
quotation stack belongs to a reading, not to a sentence or a whole volume.
Expected closers take precedence over the same glyph used as another opener.
"""
from __future__ import annotations

import hashlib
import re
from collections import Counter, defaultdict


METHOD = "zarathustra-reading-workbench-v1"
PAIRS = {"„": "“", "‚": "‘", "«": "»", "“": "”", "‘": "’", '"': '"'}
MARKERS = re.compile('[„“”‚‘’«»"]')


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def identity(kind, *parts):
    return f"tos.{kind}.sid-" + sha(METHOD + "\n" + "\n".join(map(str, parts)))[:32]


def policy_for(context, policies, context_order):
    """Apply explicit source-bound ranges, then individual context policies."""
    chapters = policies.get("chapters", [])
    chapter = next((p for p in chapters if p["reading_ref"] == context["reading_ref"]), {})
    out = {"role": chapter.get("baseline_role", "unresolved"),
           "mode": chapter.get("baseline_mode", "unresolved"),
           "status": chapter.get("status", "ambiguous"),
           "basis": "chapter_context_policy_candidate",
           "evidence": chapter.get("evidence_context_refs", []), "overrides": [], "voice_rules": [],
           "frame_role": chapter.get("baseline_role", "unresolved"), "performed_role": None, "modality": None,
           "marker_rules": [r for r in chapter.get("marker_overrides", []) if r["context_ref"] == context["context_unit_ref"]]}
    for rule in chapter.get("overrides", []):
        if rule.get("language", context["language"]) != context["language"]:
            continue
        ref = rule.get("context_ref", rule.get("context_unit_ref"))
        applies = ref == context["context_unit_ref"]
        if rule.get("start_context_ref") and rule.get("end_context_ref"):
            start = context_order.get(rule["start_context_ref"])
            end = context_order.get(rule["end_context_ref"])
            here = context_order[context["context_unit_ref"]]
            applies = start is not None and end is not None and start <= here <= end
        if not applies:
            continue
        if rule.get("scope") == "quoted_voice" and "start_offset" not in rule:
            out["voice_rules"].append(rule)
            continue
        if "start_offset" in rule:
            start, end = rule["start_offset"], rule["end_offset"]
            if not 0 <= start < end <= len(context["exact_text"]):
                raise ValueError("voice span outside source context")
            if sha(context["exact_text"][start:end]) != rule["exact_sha256"]:
                raise ValueError("voice span override source drift")
            out["overrides"].append(rule)
        else:
            if ref and rule.get("exact_sha256") != context["exact_sha256"]:
                raise ValueError("voice context override source drift")
            out.update(role=rule.get("role", out["role"]), mode=rule.get("mode", out["mode"]),
                       status=rule.get("status", "proposed"), basis=rule.get("reason", "source_visible_context_override"),
                       evidence=[ref or rule["start_context_ref"]])
            out["performed_role"] = rule.get("performed_role")
            out["modality"] = rule.get("modality")
    return out


def quote_action(marker, stack, *, leading=False, language="de", closing_position=False):
    """Paragraph re-entry marks do not create a new nesting level.

    Russian » at a paragraph's left margin is a historical continuation mark,
    not a close of the carried quotation. With no open quotation it is left
    unresolved: absence of an opener is evidence, never silently repaired.
    """
    if leading and stack:
        if marker == stack[-1]["opener"] or (language == "ru" and marker == "»"):
            return "continuation"
    if stack and marker == stack[-1]["closer"]:
        return "close"
    if any(level["closer"] == marker for level in stack[:-1]):
        return "recover_ancestor_close"
    if marker == "“" and closing_position and not stack:
        return "unmatched_close"
    if marker in PAIRS:
        return "open"
    return "unmatched_close"


def ocr_like_guillemet(text, offset, language):
    """A source-visible ambiguity, NOT a correction of » into final hard sign.

    Historical Russian extraction sometimes emits e.g. любят» выдавать. This
    lexical position cannot silently close a speech stack. Terminal/delimited
    quotes are deliberately excluded from this conservative rule.
    """
    if language != "ru" or text[offset] != "»" or offset == 0:
        return False
    if re.match(r"[А-Яа-яѣѢіІ]", text[offset-1:offset]) and re.match(r"[А-Яа-яѣѢіІ]", text[offset+1:offset+2]):
        return True
    return bool(re.search(r"[бвгджзклмнпрстфхцчшщ]$", text[:offset], re.I)
                and re.match(r"\s+[а-яѣі]", text[offset+1:]))


def voice_for_open(text, offset, markers, cues, pending, parent, policy):
    for rule in policy["overrides"]:
        if rule["start_offset"] <= offset < rule["end_offset"]:
            return rule["role"], rule.get("status", "proposed"), "source_visible_span_override", policy["evidence"]
    if policy["voice_rules"]:
        rule = policy["voice_rules"][-1]
        return rule["role"], rule.get("status", "proposed"), "source_visible_quoted_voice_policy", [rule["start_context_ref"], rule["end_context_ref"]]
    prev = max((m.start() for m in markers if m.start() < offset), default=-1)
    next_marker = min((m.start() for m in markers if m.start() > offset), default=len(text))
    before = [c for c in cues if prev < c["end"] <= offset and offset-c["end"] < 180]
    inside = [c for c in cues if offset < c["start"] < min(next_marker, offset+220)
              and re.search(r"[,!?—–]\s*$", text[offset+1:c["start"]])]
    after = [c for c in cues if next_marker < c["start"] < next_marker+100]
    candidates = []
    if before and ":" in text[before[-1]["end"]:offset]:
        candidates = before[-1:]
    elif inside:
        candidates = inside[:1]
    elif after and not re.search(r'[.!?„«]', text[next_marker+1:after[0]["start"]]):
        candidates = after[:1]
    elif pending:
        candidates = [pending]
    if candidates:
        cue = candidates[0]
        role = cue["role"]
        if role in {"first_person", "self"}:
            role = parent["role"]
        return role, cue.get("status", "proposed"), "local_reporting_cue", [cue["evidence_ref"]]
    # A quotation of a term is still uttered by its frame speaker. Its embedded
    # voice is NOT thereby resolved; both the ambiguity and the utterer survive.
    return parent["role"], "ambiguous", "frame_utterer_only_quoted_voice_unresolved", policy["evidence"]


def build_discourse(contexts, sentences, policies, find_reporting_cues):
    sentence_map = defaultdict(list)
    for row in sentences:
        sentence_map[row["context_unit_ref"]].append(row)
    ordered = sorted(contexts, key=lambda c: (c["language"], c["witness_order"]))
    context_order = {c["context_unit_ref"]: i for i, c in enumerate(ordered)}
    segments, events, gaps = [], [], []
    stack = []
    previous_reading = None
    pending = None
    active_unquoted = None

    def finish_reading():
        for level in stack:
            level["event"]["match_status"] = "unclosed_at_reading_end"
            gaps.append({"kind": "unclosed_quote", "context_unit_ref": level["event"]["context_unit_ref"],
                         "event_ref": level["event"]["event_id"], "status": "ambiguous"})
        stack.clear()

    for context in ordered:
        ref, text = context["context_unit_ref"], context["exact_text"]
        reading = context["language"], context["reading_ref"]
        if reading != previous_reading:
            finish_reading()
            pending, active_unquoted = None, None
            previous_reading = reading
        policy = policy_for(context, policies, context_order)
        baseline = {"role": policy["role"], "status": policy["status"], "mode": policy["mode"],
                    "basis": policy["basis"], "evidence": policy["evidence"],
                    "performed_role": policy["performed_role"], "modality": policy["modality"]}
        if active_unquoted and policy["basis"] == "chapter_context_policy_candidate":
            baseline = active_unquoted.copy()
        elif policy["basis"] != "chapter_context_policy_candidate":
            active_unquoted = None
        cues = find_reporting_cues(text, context["language"])
        for cue in cues:
            if not 0 <= cue["start"] < cue["end"] <= len(text):
                raise ValueError("reporting cue outside context")
            cue["evidence_ref"] = identity("reporting-cue", ref, cue["start"], cue["end"])
        markers = list(MARKERS.finditer(text))
        cuts = {0, len(text)}
        for sentence in sentence_map[ref]:
            cuts.update([sentence["start_offset"], sentence["end_offset"]])
        for cue in cues:
            cuts.update([cue["start"], cue["end"]])
        for rule in policy["overrides"]:
            cuts.update([rule["start_offset"], rule["end_offset"]])
        for match in markers:
            cuts.update([match.start(), match.end()])
        marker_by_offset = {m.start(): m for m in markers}
        closing_state = None
        for start, end in zip(sorted(cuts), sorted(cuts)[1:]):
            if end <= start:
                continue
            event = None
            if start in marker_by_offset:
                marker = text[start]
                leading = not text[:start].strip(" \n\t—–-") and bool(text[end:].strip())
                if context["unit_kind"] == "verse_line" and "\n" in text[:start]:
                    leading = leading or not text[:start].rsplit("\n", 1)[-1].strip()
                closing_position = end == len(text) or text[end].isspace() or text[end] in '.,;:!?—–'
                action = quote_action(marker, stack, leading=leading, language=context["language"],
                                      closing_position=closing_position)
                if marker == "’" and (not stack or stack[-1]["closer"] != marker):
                    action = "literal_apostrophe_candidate"
                competing_ocr_action = False
                if ocr_like_guillemet(text, start, context["language"]):
                    if action in {"close", "recover_ancestor_close"} and text[end:end+1].isspace():
                        # A real quoted word can also end in a consonant. Keep
                        # the paired boundary as primary and record competition.
                        competing_ocr_action = True
                    else:
                        action = "lexical_ocr_candidate"
                for rule in policy["marker_rules"]:
                    if rule["offset"] == start:
                        if rule["context_exact_sha256"] != context["exact_sha256"] or rule["marker_codepoint"] != f"U+{ord(marker):04X}":
                            raise ValueError("marker policy source drift")
                        action = rule["action"]
                parent = stack[-1] if stack else baseline
                event = {"event_id": identity("quote-boundary", ref, start), "context_unit_ref": ref,
                         "language": context["language"], "reading_ref": context["reading_ref"],
                         "offset": start, "marker_codepoint": f"U+{ord(marker):04X}", "action": action,
                         "depth_before": len(stack), "match_status": "proposed", "paired_event_ref": None}
                events.append(event)
                if competing_ocr_action:
                    event["alternative_action"] = "lexical_ocr_candidate"
                    gaps.append({"kind": "paired_quote_or_historical_letter_ambiguity", "event_ref": event["event_id"],
                                 "context_unit_ref": ref, "status": "ambiguous"})
                if action == "open":
                    role, status, basis, evidence = voice_for_open(text, start, markers, cues, pending, parent, policy)
                    utterer = role if not stack and parent["mode"] == "narration" else parent.get("utterer", parent["role"])
                    if policy["voice_rules"]:
                        utterer = policy["voice_rules"][-1].get("utterer_role", utterer)
                    level = {"opener": marker, "closer": PAIRS[marker], "role": role, "status": status,
                             "mode": "quoted_speech" if basis != "frame_utterer_only_quoted_voice_unresolved" else "quotation",
                             "utterer": utterer, "basis": basis, "evidence": evidence,
                             "modality": policy["voice_rules"][-1].get("modality") if policy["voice_rules"] else parent.get("modality"),
                             "performed_role": parent.get("performed_role"),
                             "event": event, "turn": identity("speech-turn", ref, start)}
                    stack.append(level)
                    pending = None
                elif action in {"close", "recover_ancestor_close"}:
                    while stack and stack[-1]["closer"] != marker:
                        discarded = stack.pop()
                        discarded["event"]["match_status"] = "unclosed_before_ancestor_close"
                        gaps.append({"kind": "interrupted_nested_quote_scope", "event_ref": discarded["event"]["event_id"],
                                     "context_unit_ref": ref, "status": "ambiguous"})
                    closing_state = stack.pop()
                    opening = closing_state["event"]
                    opening["match_status"] = event["match_status"] = "paired"
                    opening["paired_event_ref"] = event["event_id"]
                    event["paired_event_ref"] = opening["event_id"]
                elif action == "unmatched_close":
                    event["match_status"] = "unmatched"
                    gaps.append({"kind": "unmatched_quote_marker", "event_ref": event["event_id"],
                                 "context_unit_ref": ref, "status": "ambiguous"})
                elif action in {"lexical_ocr_candidate", "nonstructural_candidate"}:
                    event["match_status"] = "ambiguous_nonstructural_marker"
                    gaps.append({"kind": "guillemet_or_historical_letter_ambiguity", "event_ref": event["event_id"],
                                 "context_unit_ref": ref, "status": "ambiguous"})
                event["depth_after"] = len(stack)
            state = closing_state or (stack[-1] if stack else baseline)
            depth = len(stack) + bool(closing_state)
            closing_state = None
            state = state.copy()
            if policy["voice_rules"] and depth:
                rule = policy["voice_rules"][-1]
                state.update(role=rule["role"], utterer=rule.get("utterer_role", baseline["role"]),
                             status=rule.get("status", "proposed"), basis="source_visible_quoted_voice_policy",
                             modality=rule.get("modality"), evidence=[rule["start_context_ref"], rule["end_context_ref"]])
            containing_cues = [c for c in cues if c["start"] <= start and end <= c["end"]]
            if containing_cues:
                cue = containing_cues[0]
                next_marker = min((m.start() for m in markers if m.start() >= cue["end"]), default=len(text))
                introduction = ":" in text[cue["end"]:next_marker]
                interpolation = bool(re.search(r"[,!?—–]\s*$", text[:cue["start"]]))
                frame = state
                if stack and interpolation and not introduction:
                    frame = stack[-2] if len(stack) > 1 else baseline
                state = dict(frame, mode="reporting_clause", basis="reporting_clause_in_enclosing_frame",
                             evidence=[containing_cues[0]["evidence_ref"]])
            for rule in policy["overrides"]:
                if rule["start_offset"] <= start and end <= rule["end_offset"]:
                    state.update(role=rule["role"], mode=rule.get("mode", state["mode"]),
                                 status=rule.get("status", "proposed"), basis=rule.get("reason", "source_visible_span_override"))
            sentence_ids = [s["sentence_id"] for s in sentence_map[ref]
                            if s["start_offset"] <= start and end <= s["end_offset"]]
            if len(sentence_ids) != 1:
                raise ValueError(f"discourse slice lacks one containing source sentence: {ref}:{start}:{end}")
            seg = {"segment_id": identity("discourse-segment", ref, start, end),
                   "context_unit_ref": ref, "sentence_unit_ref": sentence_ids[0],
                   "language": context["language"], "part": context["part"], "reading_ref": context["reading_ref"],
                   "start_offset": start, "end_offset": end, "exact_text": text[start:end],
                   "exact_sha256": sha(text[start:end]), "speaker_role": state["role"],
                   "speaker_status": state["status"], "speaker_candidates": [],
                   "evidence_refs": state["evidence"], "kind": state["mode"], "quote_depth": int(depth),
                   "utterer_role": state.get("utterer", state["role"]), "attribution_basis": state["basis"],
                   "performed_role": state.get("performed_role"), "modality": state.get("modality"),
                   "speech_turn_id": state.get("turn"), "accepted": False}
            if state["basis"] == "frame_utterer_only_quoted_voice_unresolved":
                seg["speaker_candidates"] = [state["role"], "unresolved_quoted_voice"]
            segments.append(seg)
        # Explicit speech introductions can carry over without quotation marks.
        # An authored range override beats this cue-local fallback.
        colon = text.rstrip().endswith(":")
        pending = cues[-1] if colon and cues else None
        if pending and not stack:
            role = pending["role"] if pending["role"] not in {"first_person", "self"} else baseline["role"]
            active_unquoted = {"role": role, "status": "ambiguous", "mode": "unquoted_speech_candidate",
                               "basis": "carried_reporting_intro_candidate", "evidence": [pending["evidence_ref"]]}
    finish_reading()
    unclosed_turns = {identity("speech-turn", e["context_unit_ref"], e["offset"]) for e in events
                      if e["match_status"] in {"unclosed_at_reading_end", "unclosed_before_ancestor_close"}}
    for seg in segments:
        if seg["speech_turn_id"] in unclosed_turns and seg["attribution_basis"] != "source_visible_span_override":
            seg["speaker_status"] = "ambiguous"
            seg["attribution_basis"] += ":unclosed_quote_scope"
    # Consecutive spans form turn candidates; no connection across readings or
    # intervening different voices. A quote's ID is preserved across paragraphs.
    previous = None
    for seg in segments:
        key = (seg["language"], seg["reading_ref"], seg["speaker_role"], seg["kind"], seg["quote_depth"])
        if seg["speech_turn_id"] is None:
            seg["speech_turn_id"] = previous[1] if previous and previous[0] == key else identity("speech-turn", seg["segment_id"])
        previous = key, seg["speech_turn_id"]
    return segments, events, gaps


def validate_partition(contexts, segments):
    """Independent conservation check, not linguistic acceptance."""
    grouped = defaultdict(list)
    known_contexts = {c["context_unit_ref"] for c in contexts}
    seen_ids = set()
    for seg in segments:
        if seg["context_unit_ref"] not in known_contexts:
            raise ValueError("discourse orphan context")
        if seg.get("segment_id") in seen_ids:
            raise ValueError("duplicate discourse segment identity")
        if "segment_id" in seg:
            seen_ids.add(seg["segment_id"])
        grouped[seg["context_unit_ref"]].append(seg)
    for context in contexts:
        text, cursor = context["exact_text"], 0
        for seg in sorted(grouped[context["context_unit_ref"]], key=lambda s: s["start_offset"]):
            if seg["start_offset"] != cursor or seg["end_offset"] <= cursor:
                raise ValueError("discourse gap or overlap")
            exact = text[seg["start_offset"]:seg["end_offset"]]
            if exact != seg["exact_text"] or sha(exact) != seg["exact_sha256"]:
                raise ValueError("discourse exact source return mismatch")
            cursor = seg["end_offset"]
        if cursor != len(text):
            raise ValueError("discourse did not conserve complete context")
    return {"contexts_checked": len(contexts), "codepoints_checked": sum(len(c["exact_text"]) for c in contexts),
            "segments_checked": len(segments), "exact_partition": True,
            "speaker_status_counts": dict(Counter(s["speaker_status"] for s in segments))}
