//! Native port of `scripts/zarathustra_recurring_formulas.py` v1.
//!
//! The caller supplies the complete source-spine context and surface rows.
//! Returned memberships retain private exact text; public projections must
//! strip it at their owning boundary. Formula identities describe a derived
//! candidate snapshot, never authored semantic identity.
use crate::research_execution::ResearchExecution;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use tos_foundation::{python_strip_unicode16_v1, Digest256};

type R<T> = Result<T, String>;
const METHOD_VERSION: &str = "zarathustra-recurring-formulas-v1";
const MIN_TOKENS: usize = 4;
const MAX_TOKENS: usize = 32;
const MIN_OCCURRENCES: usize = 2;

#[derive(Clone)]
struct Context {
    reference: String,
    language: String,
    witness_order: i64,
    part: i64,
    reading_ref: String,
    unit_kind: String,
    exact_text: String,
    char_boundaries: Vec<usize>,
}

#[derive(Clone)]
struct Surface {
    context_ref: String,
    id: String,
    start: usize,
    end: usize,
    kind: String,
    exact_text: String,
    normalized_text: String,
    language: Option<Value>,
    part: Option<Value>,
    exact_sha256: Option<Value>,
    normalized_sha256: Option<Value>,
    sentence_id: Option<Value>,
}

#[derive(Clone)]
struct Token {
    context_ref: String,
    surface_id: String,
    start: usize,
    end: usize,
    exact_text: String,
    normalized_text: String,
    sentence_id: Option<Value>,
}

#[derive(Clone)]
struct Span {
    context_ref: String,
    start: usize,
    end: usize,
    exact_text: String,
    exact_sha256: String,
}

type Position = (usize, usize);
type CandidateKey = (String, Vec<String>);

fn string_field<'a>(value: &'a Value, key: &str) -> R<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or non-string {key}"))
}

fn python_int(value: &Value) -> Option<i64> {
    match value {
        Value::Bool(value) => Some(if *value { 1 } else { 0 }),
        Value::Number(number) => number.as_i64().or_else(|| {
            number
                .as_u64()
                .and_then(|value| i64::try_from(value).ok())
                .or_else(|| {
                    number.as_f64().and_then(|value| {
                        (value.is_finite() && value >= i64::MIN as f64 && value < i64::MAX as f64)
                            .then_some(value.trunc() as i64)
                    })
                })
        }),
        Value::String(value) => value.trim().parse().ok(),
        _ => None,
    }
}

fn integer_field(value: &Value, key: &str) -> R<i64> {
    value
        .get(key)
        .and_then(python_int)
        .ok_or_else(|| format!("missing or non-integer {key}"))
}

fn offset_field(value: &Value, key: &str) -> R<usize> {
    value
        .get(key)
        .and_then(python_int)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("missing or invalid {key}"))
}

fn digest(text: &str) -> String {
    Digest256::of_bytes(text.as_bytes()).to_hex()
}

/// Match `json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'))`.
/// All v1 identity inputs contain only JSON strings, integers, nulls, booleans,
/// arrays and objects; recursive key sorting is independent of Map features.
fn canonical(value: &Value) -> R<String> {
    fn write(value: &Value, out: &mut String) -> R<()> {
        match value {
            Value::Null => out.push_str("null"),
            Value::Bool(false) => out.push_str("false"),
            Value::Bool(true) => out.push_str("true"),
            Value::Number(number) => out.push_str(&number.to_string()),
            Value::String(text) => {
                out.push_str(&serde_json::to_string(text).map_err(|error| error.to_string())?)
            }
            Value::Array(values) => {
                out.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    write(value, out)?;
                }
                out.push(']');
            }
            Value::Object(object) => {
                out.push('{');
                let mut keys: Vec<_> = object.keys().collect();
                keys.sort_unstable();
                for (index, key) in keys.into_iter().enumerate() {
                    if index != 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(key).map_err(|error| error.to_string())?);
                    out.push(':');
                    write(
                        object.get(key).ok_or("canonical object key disappeared")?,
                        out,
                    )?;
                }
                out.push('}');
            }
        }
        Ok(())
    }

    let mut out = String::new();
    write(value, &mut out)?;
    Ok(out)
}

fn identifier(kind: &str, binding: Value) -> R<String> {
    let bytes = canonical(&json!([METHOD_VERSION, binding]))?;
    let id_digest = Digest256::of_bytes(bytes.as_bytes()).to_hex();
    Ok(format!("tos.{kind}.sid-{}", &id_digest[..32]))
}

fn boundaries(text: &str) -> Vec<usize> {
    let mut result: Vec<usize> = text.char_indices().map(|(byte, _)| byte).collect();
    result.push(text.len());
    result
}

fn slice_chars<'a>(text: &'a str, index: &[usize], start: usize, end: usize) -> R<&'a str> {
    let start_byte = *index
        .get(start)
        .ok_or("source offset exceeds Unicode text")?;
    let end_byte = *index.get(end).ok_or("source offset exceeds Unicode text")?;
    text.get(start_byte..end_byte)
        .ok_or_else(|| "source offsets do not align with Unicode code points".into())
}

fn optional_digest_matches(actual: Option<&Value>, expected: &str) -> bool {
    actual.is_none_or(|value| value.as_str() == Some(expected))
}

fn python_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|n| n != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn sentence_value(token: &Token) -> Option<&Value> {
    token.sentence_id.as_ref().filter(|value| !value.is_null())
}

fn distinct_sentences(tokens: &[Token]) -> Vec<Option<Value>> {
    let mut values = Vec::new();
    for token in tokens {
        let value = sentence_value(token).cloned();
        if !values.contains(&value) {
            values.push(value);
        }
    }
    values
}

fn lexical(kind: &str) -> bool {
    kind == "word" || kind == "number"
}

fn validated_streams(
    root: &ResearchExecution,
    contexts: &[Value],
    surfaces: &[Value],
) -> R<(
    Vec<Vec<Token>>,
    Vec<Context>,
    BTreeMap<String, usize>,
    Value,
)> {
    let mut parsed_contexts = Vec::with_capacity(contexts.len());
    let mut by_ref = BTreeMap::new();
    let mut witness_orders = BTreeSet::new();
    for (index, value) in contexts.iter().enumerate() {
        root.check()?;
        root.tick(1)?;
        let reference = string_field(value, "context_unit_ref")?.to_owned();
        if by_ref.insert(reference.clone(), index).is_some() {
            return Err(format!("duplicate context reference: {reference}"));
        }
        let language = string_field(value, "language")?.to_owned();
        let witness_order = integer_field(value, "witness_order")?;
        if !witness_orders.insert((language.clone(), witness_order)) {
            return Err(format!(
                "duplicate witness order: {language}:{witness_order}"
            ));
        }
        let part = integer_field(value, "part")?;
        let reading_ref = string_field(value, "reading_ref")?.to_owned();
        let unit_kind = string_field(value, "unit_kind")?.to_owned();
        let exact_text = string_field(value, "exact_text")?.to_owned();
        let exact_hash = digest(&exact_text);
        if !optional_digest_matches(value.get("exact_sha256"), &exact_hash) {
            return Err(format!("context text digest mismatch: {reference}"));
        }
        parsed_contexts.push(Context {
            reference,
            language,
            witness_order,
            part,
            reading_ref,
            unit_kind,
            char_boundaries: boundaries(&exact_text),
            exact_text,
        });
    }

    let mut grouped: BTreeMap<String, Vec<Surface>> = BTreeMap::new();
    let mut seen_surface_ids = BTreeSet::new();
    for value in surfaces {
        root.check()?;
        root.tick(1)?;
        let context_ref = string_field(value, "context_unit_ref")?.to_owned();
        let id = string_field(value, "surface_unit_id")?.to_owned();
        if !by_ref.contains_key(&context_ref) {
            return Err(format!("surface has unknown context: {id}"));
        }
        if !seen_surface_ids.insert(id.clone()) {
            return Err(format!("duplicate surface reference: {id}"));
        }
        grouped
            .entry(context_ref.clone())
            .or_default()
            .push(Surface {
                context_ref,
                id,
                start: offset_field(value, "start_offset")?,
                end: offset_field(value, "end_offset")?,
                kind: string_field(value, "surface_kind")?.to_owned(),
                exact_text: string_field(value, "exact_text")?.to_owned(),
                normalized_text: value
                    .get("normalized_text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                language: value.get("language").cloned(),
                part: value.get("part").cloned(),
                exact_sha256: value.get("exact_sha256").cloned(),
                normalized_sha256: value.get("normalized_sha256").cloned(),
                sentence_id: value.get("sentence_id").cloned(),
            });
    }

    let mut order: Vec<usize> = (0..parsed_contexts.len()).collect();
    order.sort_by(|left, right| {
        let left = &parsed_contexts[*left];
        let right = &parsed_contexts[*right];
        (&left.language, left.witness_order, &left.reference).cmp(&(
            &right.language,
            right.witness_order,
            &right.reference,
        ))
    });

    let mut streams: Vec<Vec<Token>> = Vec::new();
    let mut current_stream: Option<usize> = None;
    let mut previous_context: Option<usize> = None;
    let mut lexical_count = 0usize;
    let mut contexts_with_lexical = 0usize;
    let mut verse_joins = 0usize;
    for context_index in order {
        root.check()?;
        let context = &parsed_contexts[context_index];
        let mut units = grouped.remove(&context.reference).unwrap_or_default();
        root.tick(units.len() as u64)?;
        units.sort_by_key(|surface| surface.start);
        let mut end = 0usize;
        let mut tokens = Vec::new();
        for surface in units {
            root.check()?;
            root.tick(1)?;
            let char_len = context.char_boundaries.len() - 1;
            if surface.start != end || surface.end <= surface.start || surface.end > char_len {
                return Err(format!(
                    "surface coverage gap, overlap, or invalid extent: {}",
                    context.reference
                ));
            }
            if surface
                .language
                .as_ref()
                .is_some_and(|language| language.as_str() != Some(context.language.as_str()))
            {
                return Err(format!(
                    "surface/context language mismatch: {}",
                    context.reference
                ));
            }
            if let Some(part) = surface.part.as_ref() {
                if python_int(part) != Some(context.part) {
                    return Err(format!(
                        "surface/context part mismatch: {}",
                        context.reference
                    ));
                }
            }
            let exact = slice_chars(
                &context.exact_text,
                &context.char_boundaries,
                surface.start,
                surface.end,
            )?;
            if surface.exact_text != exact {
                return Err(format!("surface text differs from context: {}", surface.id));
            }
            let exact_hash = digest(exact);
            if !optional_digest_matches(surface.exact_sha256.as_ref(), &exact_hash) {
                return Err(format!("surface text digest mismatch: {}", surface.id));
            }
            end = surface.end;
            if !lexical(&surface.kind) {
                continue;
            }
            if surface.normalized_text.is_empty() {
                return Err(format!("empty lexical normalization: {}", surface.id));
            }
            let normalized_hash = digest(&surface.normalized_text);
            if !optional_digest_matches(surface.normalized_sha256.as_ref(), &normalized_hash) {
                return Err(format!("normalized text digest mismatch: {}", surface.id));
            }
            tokens.push(Token {
                context_ref: surface.context_ref,
                surface_id: surface.id,
                start: surface.start,
                end: surface.end,
                exact_text: surface.exact_text,
                normalized_text: surface.normalized_text,
                sentence_id: surface.sentence_id,
            });
        }
        if end != context.char_boundaries.len() - 1 {
            return Err(format!(
                "incomplete surface coverage: {}",
                context.reference
            ));
        }
        if !tokens.is_empty() {
            contexts_with_lexical += 1;
        }
        lexical_count += tokens.len();

        let join_verse = previous_context.is_some_and(|previous_index| {
            let previous = &parsed_contexts[previous_index];
            current_stream.is_some()
                && !tokens.is_empty()
                && previous.unit_kind == "verse_line"
                && context.unit_kind == "verse_line"
                && previous.language == context.language
                && previous.part == context.part
                && previous.reading_ref == context.reading_ref
                && previous.witness_order.checked_add(1) == Some(context.witness_order)
        });
        if join_verse {
            streams[current_stream.expect("join requires an existing stream")].extend(tokens);
            verse_joins += 1;
        } else if !tokens.is_empty() {
            streams.push(tokens);
            current_stream = Some(streams.len() - 1);
        } else {
            current_stream = None;
        }
        previous_context = Some(context_index);
    }

    let mut languages = BTreeSet::new();
    let mut readings: BTreeMap<String, BTreeSet<(i64, String)>> = BTreeMap::new();
    let mut technical: BTreeMap<String, BTreeSet<(i64, String)>> = BTreeMap::new();
    for context in &parsed_contexts {
        root.tick(1)?;
        languages.insert(context.language.clone());
        let target = if context.reading_ref.ends_with(".unscoped-technical") {
            &mut technical
        } else {
            &mut readings
        };
        target
            .entry(context.language.clone())
            .or_default()
            .insert((context.part, context.reading_ref.clone()));
    }
    let readings_by_language: BTreeMap<String, usize> = languages
        .iter()
        .map(|language| {
            (
                language.clone(),
                readings.get(language).map_or(0, BTreeSet::len),
            )
        })
        .collect();
    let technical_by_language: BTreeMap<String, usize> = languages
        .iter()
        .map(|language| {
            (
                language.clone(),
                technical.get(language).map_or(0, BTreeSet::len),
            )
        })
        .collect();
    let coverage = json!({
        "input_contexts": contexts.len(),
        "input_surface_units": surfaces.len(),
        "lexical_surface_units": lexical_count,
        "contexts_with_lexical_units": contexts_with_lexical,
        "lexical_streams": streams.len(),
        "adjacent_verse_context_joins": verse_joins,
        "source_surface_reconstruction_exact": true,
        "readings_by_language": readings_by_language,
        "unscoped_technical_groups_by_language": technical_by_language,
    });
    Ok((streams, parsed_contexts, by_ref, coverage))
}

fn independent_count(root: &ResearchExecution, positions: &[Position], length: usize) -> R<usize> {
    let mut ordered = positions.to_vec();
    ordered.sort_unstable();
    let mut count = 0usize;
    let mut ends: BTreeMap<usize, usize> = BTreeMap::new();
    for (stream, start) in ordered {
        root.tick(1)?;
        if start >= ends.get(&stream).copied().unwrap_or(0) {
            count += 1;
            ends.insert(
                stream,
                start.checked_add(length).ok_or("formula extent overflow")?,
            );
        }
    }
    Ok(count)
}

fn shared_extension(
    root: &ResearchExecution,
    streams: &[Vec<Token>],
    positions: &[Position],
    length: usize,
    left: bool,
) -> R<bool> {
    let mut value: Option<&str> = None;
    for &(stream_index, start) in positions {
        root.tick(1)?;
        let offset = if left {
            match start.checked_sub(1) {
                Some(offset) => offset,
                None => return Ok(false),
            }
        } else {
            start.checked_add(length).ok_or("formula extent overflow")?
        };
        let Some(token) = streams
            .get(stream_index)
            .and_then(|stream| stream.get(offset))
        else {
            return Ok(false);
        };
        match value {
            None => value = Some(&token.normalized_text),
            Some(existing) if existing != token.normalized_text.as_str() => return Ok(false),
            _ => {}
        }
    }
    Ok(value.is_some())
}

fn source_spans(
    root: &ResearchExecution,
    tokens: &[Token],
    by_ref: &BTreeMap<String, usize>,
    contexts: &[Context],
) -> R<Vec<Span>> {
    let mut spans: Vec<Span> = Vec::new();
    for token in tokens {
        root.tick(1)?;
        if let Some(previous) = spans
            .last_mut()
            .filter(|span| span.context_ref == token.context_ref)
        {
            previous.end = token.end;
        } else {
            spans.push(Span {
                context_ref: token.context_ref.clone(),
                start: token.start,
                end: token.end,
                exact_text: String::new(),
                exact_sha256: String::new(),
            });
        }
    }
    for span in &mut spans {
        root.check()?;
        let index = *by_ref
            .get(&span.context_ref)
            .ok_or_else(|| format!("formula context disappeared: {}", span.context_ref))?;
        let context = &contexts[index];
        span.exact_text = slice_chars(
            &context.exact_text,
            &context.char_boundaries,
            span.start,
            span.end,
        )?
        .to_owned();
        span.exact_sha256 = digest(&span.exact_text);
    }
    Ok(spans)
}

fn python_letter(text: &str) -> bool {
    static LETTER: OnceLock<Regex> = OnceLock::new();
    LETTER
        .get_or_init(|| Regex::new(r"\A\p{L}\z").expect("valid Unicode letter expression"))
        .is_match(text)
}

fn quality_flags(
    root: &ResearchExecution,
    tokens: &[Token],
    by_ref: &BTreeMap<String, usize>,
    contexts: &[Context],
) -> R<Vec<String>> {
    let mut longest = 0usize;
    let mut run = 0usize;
    let mut previous: Option<&Token> = None;
    for token in tokens {
        root.tick(1)?;
        if token.exact_text.chars().count() != 1 || !python_letter(&token.exact_text) {
            run = 0;
            previous = None;
            continue;
        }
        let mut contiguous = false;
        if let Some(previous_token) =
            previous.filter(|previous| previous.context_ref == token.context_ref)
        {
            let index = *by_ref
                .get(&token.context_ref)
                .ok_or_else(|| format!("formula context disappeared: {}", token.context_ref))?;
            let context = &contexts[index];
            let separator = slice_chars(
                &context.exact_text,
                &context.char_boundaries,
                previous_token.end,
                token.start,
            )?;
            contiguous = !separator.is_empty()
                && python_strip_unicode16_v1(separator, separator.chars().count())
                    .map_err(|error| error.to_string())?
                    .is_empty();
        }
        run = if contiguous { run + 1 } else { 1 };
        longest = longest.max(run);
        previous = Some(token);
    }
    let mut flags = Vec::new();
    if longest >= 3 {
        flags.push("suspected_letter_spacing_not_word_sequence".to_owned());
    }
    if distinct_sentences(tokens).len() > 1 {
        flags.push("crosses_sentence_boundary".to_owned());
    }
    Ok(flags)
}

fn positions_binding(
    streams: &[Vec<Token>],
    positions: &[Position],
    length: usize,
) -> R<Vec<Vec<String>>> {
    let mut ordered = positions.to_vec();
    ordered.sort_unstable();
    ordered
        .into_iter()
        .map(|(stream_index, start)| {
            streams
                .get(stream_index)
                .and_then(|stream| stream.get(start..start + length))
                .ok_or_else(|| "formula position exceeds lexical stream".to_owned())
                .map(|tokens| {
                    tokens
                        .iter()
                        .map(|token| token.surface_id.clone())
                        .collect()
                })
        })
        .collect()
}

fn sentence_refs(tokens: &[Token]) -> Vec<Value> {
    let mut refs = Vec::new();
    for token in tokens {
        if let Some(value) = token
            .sentence_id
            .as_ref()
            .filter(|value| python_truthy(value))
        {
            if !refs.contains(value) {
                refs.push(value.clone());
            }
        }
    }
    refs
}

fn count_values<'a>(values: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value.to_owned()).or_insert(0) += 1;
    }
    counts
}

/// Find recurring normalized lexical sequences. Source text remains exact in
/// spans and is never repaired, lemmatized, translated, or semantically merged.
pub fn build_formulas(
    root: &ResearchExecution,
    contexts: &[Value],
    surfaces: &[Value],
) -> R<(Vec<Value>, Vec<Value>, Vec<Value>, Value)> {
    root.check()?;
    let (streams, context_rows, by_ref, coverage) = validated_streams(root, contexts, surfaces)?;

    let mut candidates: BTreeMap<CandidateKey, Vec<Position>> = BTreeMap::new();
    for (stream_index, stream) in streams.iter().enumerate() {
        root.check()?;
        let Some(first) = stream.first() else {
            continue;
        };
        let context_index = *by_ref
            .get(&first.context_ref)
            .ok_or_else(|| format!("formula context disappeared: {}", first.context_ref))?;
        let language = context_rows[context_index].language.clone();
        if stream.len() < MIN_TOKENS {
            continue;
        }
        for start in 0..=stream.len() - MIN_TOKENS {
            root.check()?;
            root.tick(MIN_TOKENS as u64)?;
            let forms = stream[start..start + MIN_TOKENS]
                .iter()
                .map(|token| token.normalized_text.clone())
                .collect();
            candidates
                .entry((language.clone(), forms))
                .or_default()
                .push((stream_index, start));
        }
    }

    let mut retained: Vec<(String, Vec<String>, Vec<Position>, bool)> = Vec::new();
    let mut candidate_count = 0usize;
    let mut suppressed_count = 0usize;
    for length in MIN_TOKENS..=MAX_TOKENS {
        root.check()?;
        let mut next_candidates: BTreeMap<CandidateKey, Vec<Position>> = BTreeMap::new();
        for ((language, forms), positions) in std::mem::take(&mut candidates) {
            root.check()?;
            let independent = independent_count(root, &positions, length)?;
            if independent < MIN_OCCURRENCES {
                continue;
            }
            candidate_count += 1;
            let left_extension = shared_extension(root, &streams, &positions, length, true)?
                && independent_count(
                    root,
                    &positions
                        .iter()
                        .map(|&(index, start)| (index, start - 1))
                        .collect::<Vec<_>>(),
                    length + 1,
                )? == independent;
            let right_extension = shared_extension(root, &streams, &positions, length, false)?
                && independent_count(root, &positions, length + 1)? == independent;
            if !left_extension && (!right_extension || length == MAX_TOKENS) {
                retained.push((
                    language.clone(),
                    forms.clone(),
                    positions.clone(),
                    right_extension && length == MAX_TOKENS,
                ));
            } else {
                suppressed_count += 1;
            }
            if length == MAX_TOKENS {
                continue;
            }
            for &(stream_index, start) in &positions {
                root.tick(1)?;
                let stream = &streams[stream_index];
                if let Some(token) = stream.get(start + length) {
                    let mut extended = forms.clone();
                    extended.push(token.normalized_text.clone());
                    next_candidates
                        .entry((language.clone(), extended))
                        .or_default()
                        .push((stream_index, start));
                }
            }
        }
        candidates = next_candidates;
        if candidates.is_empty() {
            break;
        }
    }
    root.tick(retained.len() as u64)?;
    retained.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));

    let mut families = Vec::new();
    let mut memberships = Vec::new();
    let mut relations = Vec::new();
    for (language, forms, positions, capped) in retained {
        root.check()?;
        let occurrence_bindings = positions_binding(&streams, &positions, forms.len())?;
        let formula_id = identifier(
            "formula-candidate",
            json!([language, forms, occurrence_bindings]),
        )?;
        let mut formula_memberships = Vec::new();
        let mut exact_signatures: BTreeMap<String, String> = BTreeMap::new();
        let mut selected_positions = positions.clone();
        selected_positions.sort_unstable();
        for (stream_index, start) in selected_positions {
            root.check()?;
            root.tick(forms.len() as u64)?;
            let tokens = streams[stream_index]
                .get(start..start + forms.len())
                .ok_or_else(|| "formula position exceeds lexical stream".to_owned())?;
            let first = tokens.first().ok_or("empty formula occurrence")?;
            let context_index = *by_ref
                .get(&first.context_ref)
                .ok_or_else(|| format!("formula context disappeared: {}", first.context_ref))?;
            let context = &context_rows[context_index];
            let spans = source_spans(root, tokens, &by_ref, &context_rows)?;
            let refs: Vec<String> = tokens
                .iter()
                .map(|token| token.surface_id.clone())
                .collect();
            let occurrence_id = identifier("formula-occurrence", json!([formula_id, refs]))?;
            let exact_texts: Vec<String> =
                spans.iter().map(|span| span.exact_text.clone()).collect();
            exact_signatures.insert(occurrence_id.clone(), canonical(&json!(exact_texts))?);
            let single = spans.len() == 1;
            let flags = quality_flags(root, tokens, &by_ref, &context_rows)?;
            let deferred = flags
                .iter()
                .any(|flag| flag == "suspected_letter_spacing_not_word_sequence");
            let bundle: Vec<Value> = spans
                .iter()
                .map(|span| {
                    json!({
                        "context_unit_ref": span.context_ref,
                        "start_offset": span.start,
                        "end_offset": span.end,
                        "exact_sha256": span.exact_sha256,
                    })
                })
                .collect();
            let bundle_hash = digest(&canonical(&json!(bundle))?);
            let normalized_hash = digest(&canonical(&json!(forms))?);
            let display_text = spans
                .iter()
                .map(|span| span.exact_text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let sentence_boundary = distinct_sentences(tokens).len() > 1;
            let source_spans_value: Vec<Value> = spans
                .iter()
                .map(|span| {
                    json!({
                        "context_unit_ref": span.context_ref,
                        "start_offset": span.start,
                        "end_offset": span.end,
                        "exact_text": span.exact_text,
                        "exact_sha256": span.exact_sha256,
                    })
                })
                .collect();
            let row = json!({
                "formula_occurrence_id": occurrence_id,
                "formula_id": formula_id,
                "language": language,
                "part": context.part,
                "reading_ref": context.reading_ref,
                "reading_scope_kind": if context.reading_ref.ends_with(".unscoped-technical") { "unscoped_technical" } else { "reading" },
                "witness_order": context.witness_order,
                "context_unit_ref": first.context_ref,
                "start_offset": first.start,
                "end_offset": if single { json!(tokens.last().expect("nonempty tokens").end) } else { Value::Null },
                "surface_unit_refs": refs,
                "sentence_unit_refs": sentence_refs(tokens),
                "source_spans": source_spans_value,
                "exact_text": if single { json!(spans[0].exact_text) } else { Value::Null },
                "exact_sha256": if single { json!(spans[0].exact_sha256) } else { Value::Null },
                "source_span_bundle_sha256": bundle_hash,
                "normalized_sha256": normalized_hash,
                "display_text": display_text,
                "display_joined_across_contexts": !single,
                "crosses_sentence_boundary": sentence_boundary,
                "quality_flags": flags,
                "quality_status": if deferred { "deferred" } else { "proposed" },
                "status": if deferred { "deferred" } else { "proposed" },
            });
            formula_memberships.push(row);
        }
        formula_memberships.sort_by(|left, right| {
            (
                left["witness_order"].as_i64().unwrap_or_default(),
                left["start_offset"].as_u64().unwrap_or_default(),
                left["formula_occurrence_id"].as_str().unwrap_or_default(),
            )
                .cmp(&(
                    right["witness_order"].as_i64().unwrap_or_default(),
                    right["start_offset"].as_u64().unwrap_or_default(),
                    right["formula_occurrence_id"].as_str().unwrap_or_default(),
                ))
        });
        let occurrence_count = formula_memberships.len();
        let exact_variants = formula_memberships
            .iter()
            .filter_map(|row| row["formula_occurrence_id"].as_str())
            .filter_map(|id| exact_signatures.get(id))
            .collect::<BTreeSet<_>>()
            .len();
        let deferred_count = formula_memberships
            .iter()
            .filter(|row| row["quality_status"] == "deferred")
            .count();
        let mut reading_scopes = BTreeSet::new();
        let mut source_scopes = BTreeSet::new();
        let mut quality_flag_set = BTreeSet::new();
        let mut has_cross_context = false;
        for row in &formula_memberships {
            let part = row["part"].as_i64().unwrap_or_default();
            let reading = row["reading_ref"].as_str().unwrap_or_default().to_owned();
            source_scopes.insert((part, reading.clone()));
            if row["reading_scope_kind"] == "reading" {
                reading_scopes.insert((part, reading));
            }
            has_cross_context |= row["display_joined_across_contexts"] == true;
            if let Some(flags) = row["quality_flags"].as_array() {
                quality_flag_set.extend(flags.iter().filter_map(Value::as_str).map(str::to_owned));
            }
        }
        let match_kind = if exact_variants == 1 {
            "repeats_exact"
        } else {
            "reprises_normalized"
        };
        families.push(json!({
            "formula_id": formula_id,
            "language": language,
            "normalized_tokens": forms,
            "normalized_sha256": digest(&canonical(&json!(forms))?),
            "token_count": forms.len(),
            "occurrence_count": occurrence_count,
            "independent_occurrence_count": independent_count(root, &positions, forms.len())?,
            "reading_count": reading_scopes.len(),
            "source_scope_count": source_scopes.len(),
            "exact_variant_count": exact_variants,
            "has_cross_context_occurrences": has_cross_context,
            "right_extension_capped": capped,
            "match_kind": match_kind,
            "identity_posture": "derived_candidate_membership_snapshot",
            "quality_flags": quality_flag_set,
            "quality_deferred_occurrence_count": deferred_count,
            "quality_status": if deferred_count > 0 { "deferred" } else { "proposed" },
            "status": if deferred_count > 0 { "deferred" } else { "proposed" },
        }));
        memberships.extend(formula_memberships.iter().cloned());
        for pair in formula_memberships.windows(2) {
            root.tick(1)?;
            let source_ref = pair[0]["formula_occurrence_id"]
                .as_str()
                .ok_or("formula occurrence lacks identity")?;
            let target_ref = pair[1]["formula_occurrence_id"]
                .as_str()
                .ok_or("formula occurrence lacks identity")?;
            let source_signature = exact_signatures
                .get(source_ref)
                .ok_or("formula occurrence lacks exact signature")?;
            let target_signature = exact_signatures
                .get(target_ref)
                .ok_or("formula occurrence lacks exact signature")?;
            let relation_type = if source_signature == target_signature {
                "repeats_exact"
            } else {
                "reprises_normalized"
            };
            let relation_id = identifier(
                "formula-relation-candidate",
                json!([relation_type, source_ref, target_ref]),
            )?;
            let deferred =
                pair[0]["quality_status"] == "deferred" || pair[1]["quality_status"] == "deferred";
            relations.push(json!({
                "relation_id": relation_id,
                "relation_type": relation_type,
                "formula_id": formula_id,
                "source_occurrence_ref": source_ref,
                "target_occurrence_ref": target_ref,
                "reason_codes": ["identical_normalized_lexical_sequence", "consecutive_family_occurrences_in_witness_order"],
                "status": if deferred { "deferred" } else { "proposed" },
            }));
        }
    }

    let mut covered = BTreeSet::new();
    for row in &memberships {
        root.tick(1)?;
        if let Some(refs) = row["surface_unit_refs"].as_array() {
            covered.extend(refs.iter().filter_map(Value::as_str).map(str::to_owned));
        }
    }
    let families_by_language =
        count_values(families.iter().filter_map(|row| row["language"].as_str()));
    let match_kinds = count_values(families.iter().filter_map(|row| row["match_kind"].as_str()));
    let quality_statuses = count_values(
        families
            .iter()
            .filter_map(|row| row["quality_status"].as_str()),
    );
    let capped_count = families
        .iter()
        .filter(|row| row["right_extension_capped"] == true)
        .count();
    let receipt = json!({
        "method": METHOD_VERSION,
        "settings": { "min_tokens": MIN_TOKENS, "max_tokens": MAX_TOKENS, "min_occurrences": MIN_OCCURRENCES },
        "coverage": coverage,
        "counts": {
            "repeated_sequences_before_maximal_suppression": candidate_count,
            "suppressed_nested_sequences": suppressed_count,
            "formula_families": families.len(),
            "formula_occurrences": memberships.len(),
            "formula_relations": relations.len(),
            "lexical_units_in_retained_formulas": covered.len(),
            "length_capped_families": capped_count,
            "families_by_language": families_by_language,
            "match_kinds": match_kinds,
            "quality_statuses": quality_statuses,
        },
        "limitations": [
            "Lexical identity is measured after the existing lossy source-spine normalization; semantic equivalence requires separate contextual assessment.",
            "Punctuation and whitespace do not participate in matching; they remain exact in source spans.",
            "Matching uses the declared source-spine normalization; lemma, OCR, hyphenation, letter-spacing and synonym variants retain their source forms.",
            "Runs of three or more whitespace-separated single letters are quality-deferred; token_count is not a count of reconstructed words.",
            "Sentence-crossing repetitions are explicitly flagged, not asserted to be syntactic phrases.",
            "Paragraph and nonconsecutive-context boundaries are barriers; only adjacent verse lines may be joined.",
            "Stanza boundaries absent from the input context metadata cannot be inferred.",
            "Repeated formulas shorter than min_tokens are outside this detector's declared scope.",
            "A right_extension_capped family is only a prefix of a longer common sequence.",
            "Formula-free passages were scanned; no membership there does not assert the absence of a motif.",
            "No cross-language equivalence or altered-word near-variant relation is inferred.",
        ],
        "source_text_included": false,
        "separate_returned_rows_contain_private_source_text": true,
        "semantic_promotion": false,
        "human_review_count": 0,
        "accepted_candidate_count": 0,
    });
    root.check()?;
    Ok((families, memberships, relations, receipt))
}
