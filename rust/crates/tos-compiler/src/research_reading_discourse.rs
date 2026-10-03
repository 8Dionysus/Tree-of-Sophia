//! Native, source-preserving quotation and reporting-voice candidates.
//!
//! Offsets are Unicode codepoint offsets into unchanged source contexts. This
//! module ports the v1 candidate mechanics only; it makes no accepted speaker
//! claims and keeps unresolved quotation/voice ambiguities visible.
use crate::research_execution::ResearchExecution;
use regex::{Regex, RegexBuilder};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

type R<T> = Result<T, String>;
const METHOD: &str = "zarathustra-reading-workbench-v1";
const JOIN_MARKERS: [char; 3] = ['¬', '\u{00ad}', '-'];
const QUOTE_PAIRS: [(char, char); 6] = [
    ('„', '“'),
    ('‚', '‘'),
    ('«', '»'),
    ('“', '”'),
    ('‘', '’'),
    ('"', '"'),
];

fn tick_span(root: &ResearchExecution, start: usize, end: usize) -> R<()> {
    root.tick(end.saturating_sub(start) as u64)
}

fn required<'a>(value: &'a Value, key: &str) -> R<&'a Value> {
    value.get(key).ok_or_else(|| format!("missing {key}"))
}

fn string<'a>(value: &'a Value, key: &str) -> R<&'a str> {
    required(value, key)?
        .as_str()
        .ok_or_else(|| format!("{key} must be a string"))
}

fn optional_string<'a>(value: &'a Value, key: &str) -> R<Option<&'a str>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a string")),
    }
}

fn offset(value: &Value, key: &str) -> R<usize> {
    required(value, key)?
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("{key} must be a nonnegative integer"))
}

fn object_array<'a>(value: &'a Value, key: &str) -> R<&'a [Value]> {
    match value.get(key) {
        None => Ok(&[]),
        Some(v) => v
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| format!("{key} must be an array")),
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|n| n != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn defaulted(value: &Value, key: &str, fallback: Value) -> Value {
    value.get(key).cloned().unwrap_or(fallback)
}

fn python_whitespace(ch: char) -> bool {
    let mut encoded = [0u8; 4];
    let scalar = ch.encode_utf8(&mut encoded);
    tos_foundation::python_strip_unicode16_v1(scalar, 1).is_ok_and(|stripped| stripped.is_empty())
}

fn sha256(text: &str) -> String {
    crate::research_parallel_lexical::hash(text.as_bytes())
}

fn identity(kind: &str, parts: &[String]) -> String {
    let source = format!("{}\n{}", METHOD, parts.join("\n"));
    let digest = sha256(&source);
    format!("tos.{kind}.sid-{}", &digest[..32])
}

fn id2(kind: &str, first: &str, second: usize) -> String {
    identity(kind, &[first.to_owned(), second.to_string()])
}

fn id3(kind: &str, first: &str, second: usize, third: usize) -> String {
    identity(
        kind,
        &[first.to_owned(), second.to_string(), third.to_string()],
    )
}

struct SourceText<'a> {
    text: &'a str,
    chars: Vec<char>,
    byte_offsets: Vec<usize>,
}

impl<'a> SourceText<'a> {
    fn new(root: &ResearchExecution, text: &'a str) -> R<Self> {
        let mut chars = Vec::new();
        let mut byte_offsets = Vec::new();
        for (byte, ch) in text.char_indices() {
            root.tick(1)?;
            chars.push(ch);
            byte_offsets.push(byte);
        }
        byte_offsets.push(text.len());
        root.check()?;
        Ok(Self {
            text,
            chars,
            byte_offsets,
        })
    }

    fn len(&self) -> usize {
        self.chars.len()
    }

    fn slice(&self, start: usize, end: usize) -> R<&'a str> {
        if start > end || end > self.len() {
            return Err("source codepoint span outside context".into());
        }
        Ok(&self.text[self.byte_offsets[start]..self.byte_offsets[end]])
    }

    fn chars_string(&self, start: usize, end: usize) -> R<String> {
        if start > end || end > self.len() {
            return Err("source codepoint span outside context".into());
        }
        Ok(self.chars[start..end].iter().collect())
    }

    fn char_at(&self, offset: usize) -> Option<char> {
        self.chars.get(offset).copied()
    }

    fn byte_to_codepoint(&self, byte: usize) -> R<usize> {
        self.byte_offsets
            .binary_search(&byte)
            .map_err(|_| "regex returned a non-codepoint source boundary".into())
    }
}

#[derive(Clone)]
struct Policy {
    role: Value,
    mode: Value,
    status: Value,
    basis: Value,
    evidence: Value,
    performed_role: Value,
    modality: Value,
    overrides: Vec<Value>,
    voice_rules: Vec<Value>,
    marker_rules: Vec<Value>,
}

fn policy_for(
    root: &ResearchExecution,
    context: &Value,
    source: &SourceText<'_>,
    policies: &Value,
    context_order: &HashMap<String, usize>,
) -> R<Policy> {
    let reading = string(context, "reading_ref")?;
    let context_ref = string(context, "context_unit_ref")?;
    let language = string(context, "language")?;
    let chapters = object_array(policies, "chapters")?;
    let mut chapter = None;
    for candidate in chapters {
        root.tick(1)?;
        if string(candidate, "reading_ref")? == reading {
            chapter = Some(candidate);
            break;
        }
    }
    let empty = Value::Null;
    let chapter = chapter.unwrap_or(&empty);
    let baseline_role = defaulted(chapter, "baseline_role", json!("unresolved"));
    let mut out = Policy {
        role: baseline_role.clone(),
        mode: defaulted(chapter, "baseline_mode", json!("unresolved")),
        status: defaulted(chapter, "status", json!("ambiguous")),
        basis: json!("chapter_context_policy_candidate"),
        evidence: defaulted(chapter, "evidence_context_refs", json!([])),
        performed_role: Value::Null,
        modality: Value::Null,
        overrides: Vec::new(),
        voice_rules: Vec::new(),
        marker_rules: Vec::new(),
    };

    for rule in object_array(chapter, "marker_overrides")? {
        root.tick(1)?;
        if string(rule, "context_ref")? == context_ref {
            out.marker_rules.push(rule.clone());
        }
    }

    for rule in object_array(chapter, "overrides")? {
        root.tick(1)?;
        if rule.get("language").is_some_and(|rule_language| {
            rule_language != context.get("language").unwrap_or(&Value::Null)
        }) {
            continue;
        }
        // dict.get("context_ref", dict.get("context_unit_ref")) honors an
        // explicitly present null context_ref rather than falling through.
        let reference = if let Some(reference) = rule.get("context_ref") {
            Some(reference)
        } else {
            rule.get("context_unit_ref")
        };
        let mut applies =
            reference.is_some_and(|v| v == context.get("context_unit_ref").unwrap_or(&Value::Null));
        let start_ref = rule.get("start_context_ref");
        let end_ref = rule.get("end_context_ref");
        if start_ref.is_some_and(truthy) && end_ref.is_some_and(truthy) {
            let start_ref = start_ref.and_then(Value::as_str);
            let end_ref = end_ref.and_then(Value::as_str);
            let here = context_order.get(context_ref).copied();
            let start = start_ref.and_then(|ref_| context_order.get(ref_).copied());
            let end = end_ref.and_then(|ref_| context_order.get(ref_).copied());
            applies = matches!((start, end, here), (Some(start), Some(end), Some(here)) if start <= here && here <= end);
        }
        if !applies {
            continue;
        }
        if rule.get("scope").and_then(Value::as_str) == Some("quoted_voice")
            && !rule
                .as_object()
                .is_some_and(|object| object.contains_key("start_offset"))
        {
            out.voice_rules.push(rule.clone());
            continue;
        }
        if rule
            .as_object()
            .is_some_and(|object| object.contains_key("start_offset"))
        {
            let start = offset(rule, "start_offset")?;
            let end = offset(rule, "end_offset")?;
            if start >= end || end > source.len() {
                return Err("voice span outside source context".into());
            }
            tick_span(root, start, end)?;
            if sha256(source.slice(start, end)?) != string(rule, "exact_sha256")? {
                return Err("voice span override source drift".into());
            }
            out.overrides.push(rule.clone());
        } else {
            if reference.is_some_and(truthy)
                && rule.get("exact_sha256") != context.get("exact_sha256")
            {
                return Err("voice context override source drift".into());
            }
            out.role = defaulted(rule, "role", out.role.clone());
            out.mode = defaulted(rule, "mode", out.mode.clone());
            out.status = defaulted(rule, "status", json!("proposed"));
            out.basis = defaulted(rule, "reason", json!("source_visible_context_override"));
            let evidence_ref = if reference.is_some_and(truthy) {
                reference.cloned().unwrap_or(Value::Null)
            } else {
                required(rule, "start_context_ref")?.clone()
            };
            out.evidence = json!([evidence_ref]);
            out.performed_role = defaulted(rule, "performed_role", Value::Null);
            out.modality = defaulted(rule, "modality", Value::Null);
        }
    }
    Ok(out)
}

#[derive(Clone)]
struct CuePattern {
    role: &'static str,
    direction: &'static str,
    regex: Regex,
}

const DE_SUBJECTS: &[(&str, &str)] = &[
    ("right_king", r"(?:der\s+)?König\s+zur\s+Rechten"),
    ("left_king", r"(?:der\s+)?König\s+zur\s+Linken"),
    (
        "conscientious_one",
        r"(?:der\s+)?(?:Gewissenhafte(?:\s+des\s+Geistes)?|Getretene|Blutende|Gefragte)",
    ),
    (
        "voluntary_beggar",
        r"(?:der\s+)?(?:freiwillige\s+Bettler|Berg-Prediger|Friedfertige)",
    ),
    (
        "ugliest_man",
        r"(?:der\s+)?(?:hässlichste\s+Mensch|Unaussprechliche)",
    ),
    (
        "magician",
        r"(?:(?:der|dieser)\s+)?(?:(?:alte|kluge)\s+)?Zauberer",
    ),
    ("pope", r"(?:der\s+)?(?:alte\s+)?Papst"),
    (
        "shadow",
        r"(?:der\s+)?(?:Wanderer\s+und\s+Schatten|Schatten|Wanderer)",
    ),
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
    (
        "wisdom",
        r"(?:(?:meine|die|seine)\s+)?(?:(?:wilde|lachende|weise)\s+)?Weisheit",
    ),
    ("solitude", r"(?:die\s+)?Einsamkeit"),
    ("soul", r"(?:(?:meine|die|seine)\s+)?Seele"),
    ("crowd", r"(?:das\s+)?(?:Volk|die\s+Menge)"),
    ("herd", r"(?:die\s+)?Heerde"),
    ("self", r"ich"),
    (
        "unresolved",
        r"er|sie|es|dieser|jener|der\s+Andere|der\s+andre\s+König",
    ),
];

const RU_SUBJECTS: &[(&str, &str)] = &[
    (
        "right_king",
        r"(?:правый\s+король|король\s+(?:направо|справа|по\s+правую\s+(?:руку|сторону)))",
    ),
    (
        "left_king",
        r"(?:левый\s+король|король\s+(?:налево|слева|по\s+левую\s+(?:руку|сторону)))",
    ),
    (
        "conscientious_one",
        r"(?:добросовестный|совестливый|совестный)(?:\s+духом[ъь]?)?|растоптанный|пострадавший",
    ),
    (
        "voluntary_beggar",
        r"добровольный\s+нищий|миролюбивый|горный\s+проповедник[ъь]?",
    ),
    (
        "ugliest_man",
        r"самый\s+безобразный\s+человек[ъь]?|безобразнейший|невыразимый",
    ),
    (
        "magician",
        r"(?:(?:старый|хитрый)\s+)?(?:чародей|волшебник[ъь]?)",
    ),
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
];

const DE_VERB: &str = r"(?:sprach(?:en)?|spricht|sagt(?:e|en)?|antwortet(?:e|en)?|entgegnet(?:e|en)?|erwidert(?:e|en)?|ruft|rief(?:en)?|redet(?:e|en)?|flüstert(?:e|en)?|schrie(?:n|en)?|schreit|sang(?:en)?|fragt(?:e|en)?|raunt(?:e|en)?|knurrt(?:e|en)?|murmelt(?:e|en)?|brummt(?:e|en)?|dacht(?:e|en)|denkt)";
const RU_VERB: &str = r"(?:говорил[аи]?|говорит[ъь]?|сказал[аи]?|сказали|ответил[аи]?|отвечал[аи]?|воскликнул[аи]?|крикнул[аи]?|кричал[аи]?|спросил[аи]?|шептал[аи]?|прошептал[аи]?|подумал[аи]?|думал[аи]?|пел[аи]?|возразил[аи]?|восклицал[аи]?)[ъь]?";
const DE_MODIFIERS: &str = r"(?:(?:aber|nun|also|endlich|hier|da|abermals|nochmals|weiter|leise|zornig|traurig|lachend|heftig|unwillig|bitter|verächtlich|ihm|mir|ihr|dann|so|erheitert|erschreckt)\s+){0,4}";
const RU_MODIFIERS: &str = r"(?:(?:же|тут[ъь]?|здесь|ему|ей|мне|он[ъь]?|снова|наконец[ъь]?|еще|тогда|тихо|громко|печально|сердито|опять|усмехаясь)\s+){0,3}";

static DE_PATTERNS: OnceLock<Result<Vec<CuePattern>, String>> = OnceLock::new();
static RU_PATTERNS: OnceLock<Result<Vec<CuePattern>, String>> = OnceLock::new();
static MARKER_REGEX: OnceLock<Regex> = OnceLock::new();
static RUST_WORD_CHAR: OnceLock<Regex> = OnceLock::new();

fn compile_patterns(
    subjects: &[(&'static str, &'static str)],
    verb: &str,
    modifiers: &str,
) -> R<Vec<CuePattern>> {
    let mut output = Vec::new();
    for &(role, subject) in subjects {
        for (direction, expression) in [
            (
                "subject_after_verb",
                format!(r"\b(?P<verb>{verb})\s+{modifiers}(?P<subject>{subject})\b"),
            ),
            (
                "subject_before_verb",
                format!(r"\b(?P<subject>{subject})\s+{modifiers}(?P<verb>{verb})\b"),
            ),
        ] {
            let regex = RegexBuilder::new(&expression)
                .case_insensitive(true)
                .build()
                .map_err(|error| format!("invalid reporting-cue pattern: {error}"))?;
            output.push(CuePattern {
                role,
                direction,
                regex,
            });
        }
    }
    Ok(output)
}

fn search_patterns(language: &str) -> R<Option<&'static [CuePattern]>> {
    let (cache, subjects, verb, modifiers) = match language {
        "de" => (&DE_PATTERNS, DE_SUBJECTS, DE_VERB, DE_MODIFIERS),
        "ru" => (&RU_PATTERNS, RU_SUBJECTS, RU_VERB, RU_MODIFIERS),
        _ => return Ok(None),
    };
    let patterns = cache.get_or_init(|| compile_patterns(subjects, verb, modifiers));
    patterns
        .as_ref()
        .map(|patterns| Some(patterns.as_slice()))
        .map_err(|error| error.clone())
}

struct SearchSurface {
    text: String,
    positions: Vec<usize>,
    byte_offsets: Vec<usize>,
}

fn search_surface(root: &ResearchExecution, source: &SourceText<'_>) -> R<SearchSurface> {
    let rust_word =
        RUST_WORD_CHAR.get_or_init(|| Regex::new(r"\A\w\z").expect("static Rust word pattern"));
    let mut removed = vec![false; source.len()];
    let mut i = 0;
    while i < source.len() {
        root.tick(1)?;
        if !JOIN_MARKERS.contains(&source.chars[i]) {
            i += 1;
            continue;
        }
        let mut end = i + 1;
        let mut has_newline = false;
        while end < source.len() && python_whitespace(source.chars[end]) {
            root.tick(1)?;
            has_newline |= source.chars[end] == '\n';
            end += 1;
        }
        // Python's greedy `\s*\n\s*` spans the full contiguous whitespace
        // run containing at least one newline (the literal newline is the
        // final newline selected by the greedy first `\s*`).
        if !has_newline {
            i += 1;
            continue;
        }
        for item in &mut removed[i..end] {
            root.tick(1)?;
            *item = true;
        }
        i = end;
    }

    let mut text = String::new();
    let mut positions = Vec::new();
    for (i, original) in source.chars.iter().copied().enumerate() {
        root.tick(1)?;
        if removed[i] {
            continue;
        }
        let mut ch = match original {
            'ſ' => 's',
            'İ' | 'ı' => 'i',
            'K' => 'k',
            'ѣ' => 'е',
            'Ѣ' => 'Е',
            'і' => 'и',
            'І' => 'И',
            'ѵ' => 'и',
            other => other,
        };
        // Python re's Unicode \w is alphanumeric plus underscore. The Rust
        // regex crate also treats marks/connectors/join controls as word
        // characters and may omit Python's non-decimal numeric categories.
        // Normalize only those one-scalar boundary classes so the preserved
        // \b operators follow Python's predicate without shifting offsets.
        let mut encoded = [0u8; 4];
        let char_text = ch.encode_utf8(&mut encoded);
        let python_word_char = tos_foundation::python_word_unicode16_v1(ch);
        let rust_word_char = rust_word.is_match(char_text);
        if python_word_char && !rust_word_char {
            ch = 'ა';
        } else if rust_word_char && !python_word_char {
            ch = '\u{e000}';
        }
        if python_whitespace(ch) {
            if text.ends_with(' ') {
                continue;
            }
            ch = ' ';
        }
        text.push(ch);
        positions.push(i);
    }
    let mut byte_offsets = Vec::new();
    for (byte, _) in text.char_indices() {
        root.tick(1)?;
        byte_offsets.push(byte);
    }
    byte_offsets.push(text.len());
    Ok(SearchSurface {
        text,
        positions,
        byte_offsets,
    })
}

fn surface_index(surface: &SearchSurface, byte: usize) -> R<usize> {
    surface
        .byte_offsets
        .binary_search(&byte)
        .map_err(|_| "reporting regex returned a non-codepoint boundary".into())
}

/// Conservative, offset-preserving reporting cues, following the Python v1
/// ordered subject and direction pattern passes.
fn find_reporting_cues(
    root: &ResearchExecution,
    source: &SourceText<'_>,
    language: &str,
    ref_: &str,
) -> R<Vec<Value>> {
    let Some(patterns) = search_patterns(language)? else {
        return Ok(Vec::new());
    };
    let surface = search_surface(root, source)?;
    let mut found: Vec<Value> = Vec::new();
    for pattern in patterns {
        root.tick(surface.positions.len() as u64)?;
        for captures in pattern.regex.captures_iter(&surface.text) {
            root.tick(1)?;
            let whole = captures.get(0).ok_or("reporting cue missing full match")?;
            let verb = captures.name("verb").ok_or("reporting cue missing verb")?;
            let subject = captures
                .name("subject")
                .ok_or("reporting cue missing subject")?;
            let start_char = surface_index(&surface, whole.start())?;
            let end_char = surface_index(&surface, whole.end())?;
            let verb_start = surface_index(&surface, verb.start())?;
            let verb_end = surface_index(&surface, verb.end())?;
            let subject_start = surface_index(&surface, subject.start())?;
            let subject_end = surface_index(&surface, subject.end())?;
            let start = *surface
                .positions
                .get(start_char)
                .ok_or("empty reporting cue")?;
            let end = surface
                .positions
                .get(end_char.saturating_sub(1))
                .copied()
                .ok_or("empty reporting cue")?
                + 1;
            let verb_start = *surface
                .positions
                .get(verb_start)
                .ok_or("empty reporting verb")?;
            let verb_end = surface
                .positions
                .get(verb_end.saturating_sub(1))
                .copied()
                .ok_or("empty reporting verb")?
                + 1;
            let subject_start = *surface
                .positions
                .get(subject_start)
                .ok_or("empty reporting subject")?;
            let subject_end = surface
                .positions
                .get(subject_end.saturating_sub(1))
                .copied()
                .ok_or("empty reporting subject")?
                + 1;
            let status = if matches!(pattern.role, "unresolved" | "self" | "old_man" | "saint") {
                "ambiguous"
            } else {
                "proposed"
            };
            found.push(json!({
                "start":start,"end":end,"verb_start":verb_start,"verb_end":verb_end,
                "subject_start":subject_start,"subject_end":subject_end,"role":pattern.role,
                "status":status,"direction":pattern.direction,
                "method":"explicit_subject_adjacent_to_reporting_verb_v1",
                "evidence_ref":id3("reporting-cue",ref_,start,end)
            }));
        }
    }

    root.tick(found.len() as u64)?;
    found.sort_by(|left, right| {
        let lstart = offset(left, "verb_start").unwrap_or(usize::MAX);
        let rstart = offset(right, "verb_start").unwrap_or(usize::MAX);
        let llen = offset(left, "end")
            .unwrap_or(0)
            .saturating_sub(offset(left, "start").unwrap_or(0));
        let rlen = offset(right, "end")
            .unwrap_or(0)
            .saturating_sub(offset(right, "start").unwrap_or(0));
        lstart
            .cmp(&rstart)
            .then_with(|| rlen.cmp(&llen))
            .then_with(|| left["role"].as_str().cmp(&right["role"].as_str()))
    });
    let mut result: Vec<Value> = Vec::new();
    for row in found {
        root.tick(result.len() as u64 + 1)?;
        let verb_start = offset(&row, "verb_start")?;
        let start = offset(&row, "start")?;
        let end = offset(&row, "end")?;
        let mut covered = false;
        for old in &result {
            root.tick(1)?;
            if offset(old, "verb_start")? == verb_start
                && offset(old, "start")? <= start
                && end <= offset(old, "end")?
            {
                covered = true;
                break;
            }
        }
        if !covered {
            result.push(row);
        }
    }
    root.tick(result.len() as u64)?;
    result.sort_by(|left, right| {
        offset(left, "start")
            .unwrap_or(usize::MAX)
            .cmp(&offset(right, "start").unwrap_or(usize::MAX))
            .then_with(|| {
                offset(left, "end")
                    .unwrap_or(usize::MAX)
                    .cmp(&offset(right, "end").unwrap_or(usize::MAX))
            })
            .then_with(|| left["role"].as_str().cmp(&right["role"].as_str()))
    });
    Ok(result)
}

#[derive(Clone)]
struct State {
    role: Value,
    status: Value,
    mode: Value,
    basis: Value,
    evidence: Value,
    performed_role: Value,
    modality: Value,
    utterer: Option<Value>,
    turn: Option<Value>,
}

impl State {
    fn baseline(policy: &Policy) -> Self {
        Self {
            role: policy.role.clone(),
            status: policy.status.clone(),
            mode: policy.mode.clone(),
            basis: policy.basis.clone(),
            evidence: policy.evidence.clone(),
            performed_role: policy.performed_role.clone(),
            modality: policy.modality.clone(),
            utterer: None,
            turn: None,
        }
    }

    fn unquoted(role: Value, status: Value, mode: Value, basis: Value, evidence: Value) -> Self {
        Self {
            role,
            status,
            mode,
            basis,
            evidence,
            performed_role: Value::Null,
            modality: Value::Null,
            utterer: None,
            turn: None,
        }
    }

    fn quote_state(&self, role: Value, status: Value, basis: Value, evidence: Value) -> Self {
        let mode = if basis == json!("frame_utterer_only_quoted_voice_unresolved") {
            "quotation"
        } else {
            "quoted_speech"
        };
        Self {
            role,
            status,
            mode: json!(mode),
            basis,
            evidence,
            performed_role: self.performed_role.clone(),
            modality: self.modality.clone(),
            utterer: None,
            turn: None,
        }
    }
}

struct QuoteLevel {
    opener: char,
    closer: char,
    state: State,
    event_index: usize,
}

fn quote_action(
    marker: char,
    stack: &[QuoteLevel],
    leading: bool,
    language: &str,
    closing_position: bool,
) -> &'static str {
    if leading
        && stack
            .last()
            .is_some_and(|level| marker == level.opener || (language == "ru" && marker == '»'))
    {
        return "continuation";
    }
    if stack.last().is_some_and(|level| level.closer == marker) {
        return "close";
    }
    if stack[..stack.len().saturating_sub(1)]
        .iter()
        .any(|level| level.closer == marker)
    {
        return "recover_ancestor_close";
    }
    if marker == '“' && closing_position && stack.is_empty() {
        return "unmatched_close";
    }
    if QUOTE_PAIRS.iter().any(|(opener, _)| *opener == marker) {
        return "open";
    }
    "unmatched_close"
}

fn ocr_like_guillemet(
    root: &ResearchExecution,
    source: &SourceText<'_>,
    offset: usize,
    language: &str,
) -> R<bool> {
    if language != "ru" || source.char_at(offset) != Some('»') || offset == 0 {
        return Ok(false);
    }
    let russian_letter = |ch: char| matches!(ch, 'А'..='Я' | 'а'..='я' | 'ѣ' | 'Ѣ' | 'і' | 'І');
    if russian_letter(source.chars[offset - 1])
        && source.char_at(offset + 1).is_some_and(russian_letter)
    {
        return Ok(true);
    }
    let consonant = |ch: char| {
        matches!(
            ch,
            'б' | 'в'
                | 'г'
                | 'д'
                | 'ж'
                | 'з'
                | 'к'
                | 'л'
                | 'м'
                | 'н'
                | 'п'
                | 'р'
                | 'с'
                | 'т'
                | 'ф'
                | 'х'
                | 'ц'
                | 'ч'
                | 'ш'
                | 'щ'
                | 'Б'
                | 'В'
                | 'Г'
                | 'Д'
                | 'Ж'
                | 'З'
                | 'К'
                | 'Л'
                | 'М'
                | 'Н'
                | 'П'
                | 'Р'
                | 'С'
                | 'Т'
                | 'Ф'
                | 'Х'
                | 'Ц'
                | 'Ч'
                | 'Ш'
                | 'Щ'
        )
    };
    if !consonant(source.chars[offset - 1]) {
        return Ok(false);
    }
    let mut i = offset + 1;
    if i >= source.len() || !python_whitespace(source.chars[i]) {
        return Ok(false);
    }
    while i < source.len() && python_whitespace(source.chars[i]) {
        root.tick(1)?;
        i += 1;
    }
    Ok(i < source.len() && matches!(source.chars[i], 'а'..='я' | 'ѣ' | 'і'))
}

fn ends_with_reporting_punctuation(
    root: &ResearchExecution,
    text: &[char],
    start: usize,
    end: usize,
) -> R<bool> {
    let mut cursor = end;
    while cursor > start && python_whitespace(text[cursor - 1]) {
        root.tick(1)?;
        cursor -= 1;
    }
    root.tick(1)?;
    Ok(cursor > start && matches!(text[cursor - 1], ',' | '!' | '?' | '—' | '–'))
}

fn cue_contains_forbidden_intro(
    root: &ResearchExecution,
    source: &SourceText<'_>,
    start: usize,
    end: usize,
) -> R<bool> {
    tick_span(root, start, end)?;
    Ok(source.chars[start..end]
        .iter()
        .any(|ch| matches!(ch, '.' | '!' | '?' | '„' | '«')))
}

fn voice_for_open(
    root: &ResearchExecution,
    source: &SourceText<'_>,
    position: usize,
    markers: &[usize],
    cues: &[Value],
    pending: Option<&Value>,
    parent: &State,
    policy: &Policy,
) -> R<(Value, Value, Value, Value)> {
    for rule in &policy.overrides {
        root.tick(1)?;
        if offset(rule, "start_offset")? <= position && position < offset(rule, "end_offset")? {
            return Ok((
                required(rule, "role")?.clone(),
                defaulted(rule, "status", json!("proposed")),
                json!("source_visible_span_override"),
                policy.evidence.clone(),
            ));
        }
    }
    if let Some(rule) = policy.voice_rules.last() {
        let evidence = json!([
            required(rule, "start_context_ref")?,
            required(rule, "end_context_ref")?
        ]);
        return Ok((
            required(rule, "role")?.clone(),
            defaulted(rule, "status", json!("proposed")),
            json!("source_visible_quoted_voice_policy"),
            evidence,
        ));
    }

    let previous_marker = markers
        .iter()
        .copied()
        .filter(|marker| *marker < position)
        .max()
        .unwrap_or(0);
    let next_marker = markers
        .iter()
        .copied()
        .filter(|marker| *marker > position)
        .min()
        .unwrap_or(source.len());
    let mut before = Vec::new();
    let mut inside = Vec::new();
    let mut after = Vec::new();
    root.tick((markers.len() as u64).saturating_mul(3))?;
    for cue in cues {
        root.tick(1)?;
        let cue_start = offset(cue, "start")?;
        let cue_end = offset(cue, "end")?;
        if cue_end <= position
            && cue_end > previous_marker
            && position.saturating_sub(cue_end) < 180
        {
            before.push(cue);
        }
        if cue_start > position
            && cue_start < next_marker
            && cue_start < position.saturating_add(220)
            && ends_with_reporting_punctuation(root, &source.chars, position + 1, cue_start)?
        {
            inside.push(cue);
        }
        if cue_start > next_marker && cue_start < next_marker.saturating_add(100) {
            after.push(cue);
        }
    }
    let before_with_colon = if let Some(candidate) = before.last() {
        let cue_end = offset(candidate, "end")?;
        tick_span(root, cue_end, position)?;
        source
            .slice(cue_end, position)?
            .contains(':')
            .then_some(*candidate)
    } else {
        None
    };
    let selected = if before_with_colon.is_some() {
        before_with_colon
    } else if let Some(candidate) = inside.first() {
        Some(*candidate)
    } else if let Some(candidate) = after.first() {
        let cue_start = offset(candidate, "start")?;
        let scan_start = next_marker.saturating_add(1).min(source.len());
        if cue_contains_forbidden_intro(root, source, scan_start, cue_start)? {
            pending
        } else {
            Some(*candidate)
        }
    } else {
        pending
    };
    if let Some(cue) = selected {
        let cue_role = required(cue, "role")?.clone();
        let role = if matches!(cue_role.as_str(), Some("first_person" | "self")) {
            parent.role.clone()
        } else {
            cue_role
        };
        return Ok((
            role,
            defaulted(cue, "status", json!("proposed")),
            json!("local_reporting_cue"),
            json!([required(cue, "evidence_ref")?]),
        ));
    }
    Ok((
        parent.role.clone(),
        json!("ambiguous"),
        json!("frame_utterer_only_quoted_voice_unresolved"),
        policy.evidence.clone(),
    ))
}

fn baseline_state(policy: &Policy, active_unquoted: &Option<State>) -> State {
    if policy.basis == json!("chapter_context_policy_candidate") {
        if let Some(active) = active_unquoted {
            return active.clone();
        }
    }
    State::baseline(policy)
}

fn set_quote_unclosed(
    root: &ResearchExecution,
    events: &mut [Value],
    gaps: &mut Vec<Value>,
    stack: &mut Vec<QuoteLevel>,
    match_status: &str,
    gap_kind: &str,
    context_ref: Option<&str>,
) -> R<()> {
    for level in stack.iter() {
        root.tick(1)?;
        events[level.event_index]["match_status"] = json!(match_status);
        let mut gap = json!({
            "kind":gap_kind,
            "context_unit_ref":events[level.event_index]["context_unit_ref"],
            "event_ref":events[level.event_index]["event_id"],
            "status":"ambiguous"
        });
        if let Some(context_ref) = context_ref {
            gap["context_unit_ref"] = json!(context_ref);
        }
        gaps.push(gap);
    }
    stack.clear();
    Ok(())
}

/// Build source-preserving discourse segment, quote-boundary, and ambiguity
/// candidates with the v1 identities and policy ordering.
pub fn build_discourse(
    root: &ResearchExecution,
    contexts: &[Value],
    sentences: &[Value],
    policies: &Value,
) -> R<(Vec<Value>, Vec<Value>, Vec<Value>)> {
    let mut sentence_map: HashMap<String, Vec<Value>> = HashMap::new();
    for row in sentences {
        root.tick(1)?;
        sentence_map
            .entry(string(row, "context_unit_ref")?.to_owned())
            .or_default()
            .push(row.clone());
    }

    let mut ordered: Vec<(String, i128, usize)> = Vec::with_capacity(contexts.len());
    for (index, context) in contexts.iter().enumerate() {
        root.tick(1)?;
        let witness_order = required(context, "witness_order")?;
        let witness_order = witness_order
            .as_i64()
            .map(i128::from)
            .or_else(|| witness_order.as_u64().map(i128::from))
            .ok_or("witness_order must be an integer")?;
        ordered.push((
            string(context, "language")?.to_owned(),
            witness_order,
            index,
        ));
    }
    root.tick(ordered.len() as u64)?;
    ordered.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    let ordered_contexts: Vec<&Value> = ordered
        .iter()
        .map(|(_, _, index)| &contexts[*index])
        .collect();

    let mut context_order = HashMap::new();
    for (index, context) in ordered_contexts.iter().enumerate() {
        root.tick(1)?;
        context_order.insert(string(context, "context_unit_ref")?.to_owned(), index);
    }

    let mut segments = Vec::new();
    let mut events = Vec::new();
    let mut gaps = Vec::new();
    let mut stack: Vec<QuoteLevel> = Vec::new();
    let mut previous_reading: Option<(String, String)> = None;
    let mut pending: Option<Value> = None;
    let mut active_unquoted: Option<State> = None;

    for context in &ordered_contexts {
        root.check()?;
        let context_ref = string(context, "context_unit_ref")?;
        let language = string(context, "language")?;
        let reading_ref = string(context, "reading_ref")?;
        let text = string(context, "exact_text")?;
        let source = SourceText::new(root, text)?;
        let reading = (language.to_owned(), reading_ref.to_owned());
        if previous_reading.as_ref() != Some(&reading) {
            set_quote_unclosed(
                root,
                &mut events,
                &mut gaps,
                &mut stack,
                "unclosed_at_reading_end",
                "unclosed_quote",
                None,
            )?;
            pending = None;
            active_unquoted = None;
            previous_reading = Some(reading);
        }

        let policy = policy_for(root, context, &source, policies, &context_order)?;
        let baseline = baseline_state(&policy, &active_unquoted);
        if policy.basis != json!("chapter_context_policy_candidate") {
            active_unquoted = None;
        }

        let cues = find_reporting_cues(root, &source, language, context_ref)?;
        for cue in &cues {
            root.tick(1)?;
            let start = offset(cue, "start")?;
            let end = offset(cue, "end")?;
            if start >= end || end > source.len() {
                return Err("reporting cue outside context".into());
            }
        }

        let marker_regex = MARKER_REGEX
            .get_or_init(|| Regex::new(r#"[„“”‚‘’«»"]"#).expect("static quote marker pattern"));
        root.tick(source.len() as u64)?;
        let mut markers = Vec::new();
        let mut marker_by_offset = BTreeMap::new();
        for found in marker_regex.find_iter(source.text) {
            root.tick(1)?;
            let start = source.byte_to_codepoint(found.start())?;
            let end = source.byte_to_codepoint(found.end())?;
            markers.push(start);
            marker_by_offset.insert(start, end);
        }

        let mut cuts = BTreeSet::new();
        cuts.insert(0);
        cuts.insert(source.len());
        if let Some(rows) = sentence_map.get(context_ref) {
            for sentence in rows {
                root.tick(1)?;
                cuts.insert(offset(sentence, "start_offset")?);
                cuts.insert(offset(sentence, "end_offset")?);
            }
        }
        for cue in &cues {
            root.tick(1)?;
            cuts.insert(offset(cue, "start")?);
            cuts.insert(offset(cue, "end")?);
        }
        for rule in &policy.overrides {
            root.tick(1)?;
            cuts.insert(offset(rule, "start_offset")?);
            cuts.insert(offset(rule, "end_offset")?);
        }
        for (start, end) in &marker_by_offset {
            root.tick(1)?;
            cuts.insert(*start);
            cuts.insert(*end);
        }
        root.tick(cuts.len() as u64)?;
        if cuts.iter().any(|cut| *cut > source.len()) {
            return Err("discourse boundary outside source context".into());
        }

        let boundaries: Vec<usize> = cuts.into_iter().collect();
        let mut closing_state: Option<State> = None;
        for pair in boundaries.windows(2) {
            root.tick(1)?;
            let start = pair[0];
            let end = pair[1];
            if end <= start {
                continue;
            }
            if start >= source.len() {
                return Err("discourse slice starts beyond source context".into());
            }
            if marker_by_offset.contains_key(&start) {
                let marker_end = *marker_by_offset
                    .get(&start)
                    .ok_or("quote marker boundary missing")?;
                let marker = source
                    .char_at(start)
                    .ok_or("quote marker codepoint missing")?;
                let mut leading = true;
                tick_span(root, 0, start)?;
                for ch in &source.chars[..start] {
                    if !matches!(ch, ' ' | '\n' | '\t' | '—' | '–' | '-') {
                        leading = false;
                        break;
                    }
                }
                if leading {
                    tick_span(root, end, source.len())?;
                    leading = source.chars[end..].iter().any(|ch| !python_whitespace(*ch));
                }
                if string(context, "unit_kind")? == "verse_line"
                    && source.chars[..start].contains(&'\n')
                {
                    if let Some(newline) = source.chars[..start].iter().rposition(|ch| *ch == '\n')
                    {
                        tick_span(root, newline + 1, start)?;
                        leading |= source.chars[newline + 1..start]
                            .iter()
                            .all(|ch| python_whitespace(*ch));
                    }
                }
                let next = source.char_at(marker_end);
                let closing_position = marker_end == source.len()
                    || next.is_some_and(python_whitespace)
                    || next.is_some_and(|ch| {
                        matches!(ch, '.' | ',' | ';' | ':' | '!' | '?' | '—' | '–')
                    });
                let mut action =
                    quote_action(marker, &stack, leading, language, closing_position).to_owned();
                if marker == '’' && stack.last().is_none_or(|level| level.closer != marker) {
                    action = "literal_apostrophe_candidate".into();
                }
                let mut competing_ocr_action = false;
                if ocr_like_guillemet(root, &source, start, language)? {
                    if matches!(action.as_str(), "close" | "recover_ancestor_close")
                        && next.is_some_and(python_whitespace)
                    {
                        competing_ocr_action = true;
                    } else {
                        action = "lexical_ocr_candidate".into();
                    }
                }
                for rule in &policy.marker_rules {
                    root.tick(1)?;
                    if offset(rule, "offset")? == start {
                        let codepoint = format!("U+{:04X}", marker as u32);
                        if rule.get("context_exact_sha256") != context.get("exact_sha256")
                            || string(rule, "marker_codepoint")? != codepoint
                        {
                            return Err("marker policy source drift".into());
                        }
                        action = string(rule, "action")?.to_owned();
                    }
                }

                let parent = stack
                    .last()
                    .map(|level| level.state.clone())
                    .unwrap_or_else(|| baseline.clone());
                root.tick(stack.len() as u64)?;
                events.push(json!({
                    "event_id":id2("quote-boundary",context_ref,start),
                    "context_unit_ref":context_ref,
                    "language":language,
                    "reading_ref":reading_ref,
                    "offset":start,
                    "marker_codepoint":format!("U+{:04X}",marker as u32),
                    "action":action,
                    "depth_before":stack.len(),
                    "match_status":"proposed",
                    "paired_event_ref":null
                }));
                let index = events.len() - 1;
                if competing_ocr_action {
                    events[index]["alternative_action"] = json!("lexical_ocr_candidate");
                    gaps.push(json!({
                        "kind":"paired_quote_or_historical_letter_ambiguity",
                        "event_ref":events[index]["event_id"],
                        "context_unit_ref":context_ref,
                        "status":"ambiguous"
                    }));
                }

                match action.as_str() {
                    "open" => {
                        let (role, status, basis, evidence) = voice_for_open(
                            root,
                            &source,
                            start,
                            &markers,
                            &cues,
                            pending.as_ref(),
                            &parent,
                            &policy,
                        )?;
                        let narrator_frame = stack.is_empty() && parent.mode == json!("narration");
                        let mut quote_state =
                            parent.quote_state(role.clone(), status, basis, evidence);
                        let inherited_utterer = if narrator_frame {
                            role
                        } else {
                            parent
                                .utterer
                                .clone()
                                .unwrap_or_else(|| parent.role.clone())
                        };
                        quote_state.utterer = Some(inherited_utterer);
                        if let Some(rule) = policy.voice_rules.last() {
                            quote_state.utterer = Some(defaulted(
                                rule,
                                "utterer_role",
                                if narrator_frame {
                                    quote_state.role.clone()
                                } else {
                                    parent
                                        .utterer
                                        .clone()
                                        .unwrap_or_else(|| parent.role.clone())
                                },
                            ));
                            quote_state.modality = defaulted(rule, "modality", Value::Null);
                        }
                        quote_state.turn = Some(json!(id2("speech-turn", context_ref, start)));
                        let closer = QUOTE_PAIRS
                            .iter()
                            .find(|(opener, _)| *opener == marker)
                            .map(|(_, closer)| *closer)
                            .ok_or("quote opener has no expected closer")?;
                        stack.push(QuoteLevel {
                            opener: marker,
                            closer,
                            state: quote_state,
                            event_index: index,
                        });
                        pending = None;
                    }
                    "close" | "recover_ancestor_close" => {
                        while stack.last().is_some_and(|level| level.closer != marker) {
                            let discarded = stack.pop().ok_or("quote recovery stack underflow")?;
                            events[discarded.event_index]["match_status"] =
                                json!("unclosed_before_ancestor_close");
                            gaps.push(json!({
                                "kind":"interrupted_nested_quote_scope",
                                "event_ref":events[discarded.event_index]["event_id"],
                                "context_unit_ref":context_ref,
                                "status":"ambiguous"
                            }));
                        }
                        let closing = stack
                            .pop()
                            .ok_or("quote close has no matching open quotation")?;
                        events[closing.event_index]["match_status"] = json!("paired");
                        events[index]["match_status"] = json!("paired");
                        let opening_id = events[closing.event_index]["event_id"].clone();
                        let closing_id = events[index]["event_id"].clone();
                        events[closing.event_index]["paired_event_ref"] = closing_id;
                        events[index]["paired_event_ref"] = opening_id;
                        closing_state = Some(closing.state);
                    }
                    "unmatched_close" => {
                        events[index]["match_status"] = json!("unmatched");
                        gaps.push(json!({
                            "kind":"unmatched_quote_marker",
                            "event_ref":events[index]["event_id"],
                            "context_unit_ref":context_ref,
                            "status":"ambiguous"
                        }));
                    }
                    "lexical_ocr_candidate" | "nonstructural_candidate" => {
                        events[index]["match_status"] = json!("ambiguous_nonstructural_marker");
                        gaps.push(json!({
                            "kind":"guillemet_or_historical_letter_ambiguity",
                            "event_ref":events[index]["event_id"],
                            "context_unit_ref":context_ref,
                            "status":"ambiguous"
                        }));
                    }
                    _ => {}
                }
                events[index]["depth_after"] = json!(stack.len());
            }

            let mut state = closing_state
                .clone()
                .or_else(|| stack.last().map(|level| level.state.clone()))
                .unwrap_or_else(|| baseline.clone());
            let depth = stack.len() + usize::from(closing_state.is_some());
            closing_state = None;

            if !policy.voice_rules.is_empty() && depth > 0 {
                let rule = policy.voice_rules.last().ok_or("voice rule stack empty")?;
                state.role = required(rule, "role")?.clone();
                state.utterer = Some(defaulted(rule, "utterer_role", baseline.role.clone()));
                state.status = defaulted(rule, "status", json!("proposed"));
                state.basis = json!("source_visible_quoted_voice_policy");
                state.modality = defaulted(rule, "modality", Value::Null);
                state.evidence = json!([
                    required(rule, "start_context_ref")?,
                    required(rule, "end_context_ref")?
                ]);
            }

            let mut containing_cue = None;
            for cue in &cues {
                root.tick(1)?;
                if offset(cue, "start")? <= start && end <= offset(cue, "end")? {
                    containing_cue = Some(cue);
                    break;
                }
            }
            if let Some(cue) = containing_cue {
                let cue_end = offset(cue, "end")?;
                root.tick(markers.len() as u64)?;
                let next_marker = markers
                    .iter()
                    .copied()
                    .filter(|marker| *marker >= cue_end)
                    .min()
                    .unwrap_or(source.len());
                tick_span(root, cue_end, next_marker)?;
                let introduction = source.slice(cue_end, next_marker)?.contains(':');
                let interpolation =
                    ends_with_reporting_punctuation(root, &source.chars, 0, offset(cue, "start")?)?;
                let frame = if !stack.is_empty() && interpolation && !introduction {
                    if stack.len() > 1 {
                        stack[stack.len() - 2].state.clone()
                    } else {
                        baseline.clone()
                    }
                } else {
                    state.clone()
                };
                state = frame;
                state.mode = json!("reporting_clause");
                state.basis = json!("reporting_clause_in_enclosing_frame");
                state.evidence = json!([required(cue, "evidence_ref")?]);
            }

            for rule in &policy.overrides {
                root.tick(1)?;
                if offset(rule, "start_offset")? <= start && end <= offset(rule, "end_offset")? {
                    state.role = required(rule, "role")?.clone();
                    state.mode = defaulted(rule, "mode", state.mode.clone());
                    state.status = defaulted(rule, "status", json!("proposed"));
                    state.basis = defaulted(rule, "reason", json!("source_visible_span_override"));
                }
            }

            let mut sentence_ids = Vec::new();
            if let Some(rows) = sentence_map.get(context_ref) {
                for sentence in rows {
                    root.tick(1)?;
                    if offset(sentence, "start_offset")? <= start
                        && end <= offset(sentence, "end_offset")?
                    {
                        sentence_ids.push(required(sentence, "sentence_id")?.clone());
                    }
                }
            }
            if sentence_ids.len() != 1 {
                return Err(format!(
                    "discourse slice lacks one containing source sentence: {context_ref}:{start}:{end}"
                ));
            }
            tick_span(root, start, end)?;
            let exact_text = source.chars_string(start, end)?;
            let segment_id = id3("discourse-segment", context_ref, start, end);
            let unresolved = state.basis == json!("frame_utterer_only_quoted_voice_unresolved");
            let segment = json!({
                "segment_id":segment_id,
                "context_unit_ref":context_ref,
                "sentence_unit_ref":sentence_ids[0],
                "language":language,
                "part":required(context,"part")?,
                "reading_ref":reading_ref,
                "start_offset":start,
                "end_offset":end,
                "exact_text":exact_text,
                "exact_sha256":sha256(source.slice(start,end)?),
                "speaker_role":state.role.clone(),
                "speaker_status":state.status.clone(),
                "speaker_candidates":if unresolved { json!([state.role,"unresolved_quoted_voice"]) } else { json!([]) },
                "evidence_refs":state.evidence.clone(),
                "kind":state.mode.clone(),
                "quote_depth":depth,
                "utterer_role":state.utterer.clone().unwrap_or_else(|| state.role.clone()),
                "attribution_basis":state.basis.clone(),
                "performed_role":state.performed_role.clone(),
                "modality":state.modality.clone(),
                "speech_turn_id":state.turn.clone(),
                "accepted":false
            });
            segments.push(segment);
        }

        let mut trimmed_end = source.len();
        while trimmed_end > 0 && python_whitespace(source.chars[trimmed_end - 1]) {
            root.tick(1)?;
            trimmed_end -= 1;
        }
        pending = if trimmed_end > 0 && source.chars[trimmed_end - 1] == ':' {
            cues.last().cloned()
        } else {
            None
        };
        if let Some(pending_cue) = pending.as_ref().filter(|_| stack.is_empty()) {
            let cue_role = required(pending_cue, "role")?.clone();
            let role = if matches!(cue_role.as_str(), Some("first_person" | "self")) {
                baseline.role.clone()
            } else {
                cue_role
            };
            active_unquoted = Some(State::unquoted(
                role,
                json!("ambiguous"),
                json!("unquoted_speech_candidate"),
                json!("carried_reporting_intro_candidate"),
                json!([required(pending_cue, "evidence_ref")?]),
            ));
        }
    }

    set_quote_unclosed(
        root,
        &mut events,
        &mut gaps,
        &mut stack,
        "unclosed_at_reading_end",
        "unclosed_quote",
        None,
    )?;
    let mut unclosed_turns = HashSet::new();
    for event in &events {
        root.tick(1)?;
        if matches!(
            event.get("match_status").and_then(Value::as_str),
            Some("unclosed_at_reading_end" | "unclosed_before_ancestor_close")
        ) {
            let event_ref = string(event, "context_unit_ref")?;
            let event_offset = offset(event, "offset")?;
            unclosed_turns.insert(id2("speech-turn", event_ref, event_offset));
        }
    }
    for segment in &mut segments {
        root.tick(1)?;
        let turn = optional_string(segment, "speech_turn_id")?;
        if turn.is_some_and(|turn| unclosed_turns.contains(turn))
            && string(segment, "attribution_basis")? != "source_visible_span_override"
        {
            segment["speaker_status"] = json!("ambiguous");
            let basis = string(segment, "attribution_basis")?.to_owned();
            segment["attribution_basis"] = json!(format!("{basis}:unclosed_quote_scope"));
        }
    }

    let mut previous: Option<(Value, Value, Value, Value, Value, Value)> = None;
    for segment in &mut segments {
        root.tick(1)?;
        let key = (
            required(segment, "language")?.clone(),
            required(segment, "reading_ref")?.clone(),
            required(segment, "speaker_role")?.clone(),
            required(segment, "kind")?.clone(),
            required(segment, "quote_depth")?.clone(),
        );
        if segment.get("speech_turn_id").is_none_or(Value::is_null) {
            let turn = match previous.as_ref() {
                Some((old_language, old_reading, old_role, old_kind, old_depth, old_turn))
                    if (old_language, old_reading, old_role, old_kind, old_depth)
                        == (&key.0, &key.1, &key.2, &key.3, &key.4) =>
                {
                    old_turn.clone()
                }
                _ => json!(identity(
                    "speech-turn",
                    &[string(segment, "segment_id")?.to_owned()]
                )),
            };
            segment["speech_turn_id"] = turn;
        }
        let turn = segment["speech_turn_id"].clone();
        previous = Some((key.0, key.1, key.2, key.3, key.4, turn));
    }
    root.check()?;
    Ok((segments, events, gaps))
}

/// Independently verify complete context conservation and return a mechanical
/// receipt. This check does not evaluate attribution, quotation, or meaning.
pub fn validate_partition(
    root: &ResearchExecution,
    contexts: &[Value],
    segments: &[Value],
) -> R<Value> {
    let mut known_contexts = HashSet::new();
    for context in contexts {
        root.tick(1)?;
        known_contexts.insert(string(context, "context_unit_ref")?.to_owned());
    }
    let mut grouped: HashMap<String, Vec<Value>> = HashMap::new();
    let mut seen_ids = HashSet::new();
    for segment in segments {
        root.tick(1)?;
        let context_ref = string(segment, "context_unit_ref")?.to_owned();
        if !known_contexts.contains(&context_ref) {
            return Err("discourse orphan context".into());
        }
        if let Some(segment_id) = segment.get("segment_id") {
            let key = match segment_id {
                Value::Array(_) | Value::Object(_) => {
                    return Err("discourse segment identity must be hashable".into());
                }
                Value::String(value) => format!("string:{value}"),
                _ => segment_id.to_string(),
            };
            if seen_ids.contains(&key) {
                return Err("duplicate discourse segment identity".into());
            }
            seen_ids.insert(key);
        }
        grouped
            .entry(context_ref)
            .or_default()
            .push(segment.clone());
    }

    let mut codepoints_checked = 0usize;
    for context in contexts {
        root.check()?;
        let context_ref = string(context, "context_unit_ref")?;
        let text = string(context, "exact_text")?;
        let source = SourceText::new(root, text)?;
        codepoints_checked = codepoints_checked
            .checked_add(source.len())
            .ok_or("discourse codepoint count overflow")?;
        let mut rows = grouped.remove(context_ref).unwrap_or_default();
        root.tick(rows.len() as u64)?;
        rows.sort_by_key(|segment| offset(segment, "start_offset").unwrap_or(usize::MAX));
        let mut cursor = 0usize;
        for segment in rows {
            root.tick(1)?;
            let start = offset(&segment, "start_offset")?;
            let end = offset(&segment, "end_offset")?;
            if start != cursor || end <= cursor {
                return Err("discourse gap or overlap".into());
            }
            tick_span(root, start, end)?;
            let exact = source.slice(start, end)?;
            if exact != string(&segment, "exact_text")?
                || sha256(exact) != string(&segment, "exact_sha256")?
            {
                return Err("discourse exact source return mismatch".into());
            }
            cursor = end;
        }
        if cursor != source.len() {
            return Err("discourse did not conserve complete context".into());
        }
    }

    let mut speaker_status_counts: BTreeMap<String, usize> = BTreeMap::new();
    for segment in segments {
        root.tick(1)?;
        let status = required(segment, "speaker_status")?;
        let key = match status {
            Value::String(value) => value.clone(),
            Value::Null => "null".into(),
            Value::Bool(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            Value::Array(_) | Value::Object(_) => {
                return Err("speaker status must be hashable".into());
            }
        };
        *speaker_status_counts.entry(key).or_default() += 1;
    }
    root.check()?;
    Ok(json!({
        "contexts_checked":contexts.len(),
        "codepoints_checked":codepoints_checked,
        "segments_checked":segments.len(),
        "exact_partition":true,
        "speaker_status_counts":speaker_status_counts
    }))
}
