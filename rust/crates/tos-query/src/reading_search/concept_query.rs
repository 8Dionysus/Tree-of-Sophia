//! Direct maintained concept-search entry over the same source-bound reader.
use super::*;

#[derive(Clone, Debug)]
pub struct ConceptSearchRequest {
    pub query: String,
    pub language: String,
    pub limit: String,
    pub include_semantic_neighbors: bool,
    pub request_ref: Option<String>,
}

pub fn execute_concept_search(
    roots: &ExplicitReadingRoots,
    software: &ReadingSoftware,
    request: &ConceptSearchRequest,
    budget: ReadingSearchBudget,
    probe: Arc<dyn AbortProbe>,
) -> Result<ReadingSearchResult> {
    check(probe.as_ref())?;
    let input_bytes = request
        .query
        .len()
        .checked_add(request.language.len())
        .and_then(|n| n.checked_add(request.limit.len()))
        .and_then(|n| n.checked_add(request.request_ref.as_ref().map_or(0, String::len)))
        .ok_or_else(budget_error)?;
    if input_bytes > budget.json.max_bytes {
        return Err(budget_error());
    }
    if !["de", "ru", "en"].contains(&request.language.as_str()) {
        return Err(error(
            ReadingSearchErrorCode::InvalidRequest,
            "unsupported concept-search language",
        ));
    }
    let (limit_text, card_limit) = limit(&request.limit, budget.json.max_integer_digits)?;
    let request_ref = request
        .request_ref
        .as_deref()
        .map(|reference| {
            let path = std::path::Path::new(reference);
            if path.is_absolute() {
                path.strip_prefix(&roots.source_root)
                    .ok()
                    .and_then(std::path::Path::to_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        error(
                            ReadingSearchErrorCode::InvalidRequest,
                            "concept request must belong to selected source root",
                        )
                    })
            } else {
                Ok(reference.to_owned())
            }
        })
        .transpose()?;
    // The maintained standalone concept command permits limit=0 (coverage
    // only) and limits above the reading API's 100-card clamp. The existing
    // kernel bounds actual selected data/work; do not impose the API clamp.
    let selection = ReadingSearchRequest {
        query: request.query.clone(),
        language: request.language.clone(),
        limit: 0,
        include_semantic_neighbors: request.include_semantic_neighbors,
        group_by: Vec::new(),
        request_ref,
    };
    let mut reader = Reader::new(roots, software, budget, &probe)?;
    reader.work(
        input_bytes
            .checked_add(
                request
                    .limit
                    .len()
                    .checked_mul(64)
                    .ok_or_else(budget_error)?,
            )
            .ok_or_else(budget_error)?,
    )?;
    let (packet, _) = concept::baseline_with_limit_text(
        &mut reader,
        &request.query,
        &selection,
        &limit_text,
        card_limit,
    )?;
    let body = reader.emit(&packet)?;
    reader.finish(body)
}

// Python argparse int spelling, bounded by the existing JSON integer-digit
// profile. Saturate only the vector slice count; preserve the complete integer
// in the maintained search-result identity even above machine integer range.
pub(super) fn limit(raw: &str, max_digits: usize) -> Result<(String, usize)> {
    let invalid = || {
        error(
            ReadingSearchErrorCode::InvalidRequest,
            "concept limit must be zero or greater",
        )
    };
    let raw = raw.trim_matches(char::is_whitespace);
    let (negative, digits) = if let Some(s) = raw.strip_prefix('-') {
        (true, s)
    } else {
        (false, raw.strip_prefix('+').unwrap_or(raw))
    };
    let mut decimal = String::new();
    let mut previous_digit = false;
    for ch in digits.chars() {
        if ch == '_' && previous_digit {
            previous_digit = false;
            continue;
        }
        if !tos_foundation::python_decimal_unicode16_v1(ch) {
            return Err(invalid());
        }
        if decimal.len() >= max_digits {
            return Err(budget_error());
        }
        let mut code = ch as u32;
        let mut offset = 0;
        while code > 0
            && char::from_u32(code - 1).is_some_and(tos_foundation::python_decimal_unicode16_v1)
        {
            code -= 1;
            offset += 1;
            if offset > 64 {
                return Err(budget_error());
            }
        }
        decimal.push((b'0' + (offset % 10) as u8) as char);
        previous_digit = true;
    }
    if !previous_digit {
        return Err(invalid());
    }
    let normalized = decimal.trim_start_matches('0');
    if negative && !normalized.is_empty() {
        return Err(invalid());
    }
    let normalized = if normalized.is_empty() {
        "0"
    } else {
        normalized
    };
    let count = normalized.bytes().fold(0usize, |n, digit| {
        n.saturating_mul(10).saturating_add((digit - b'0') as usize)
    });
    Ok((normalized.to_owned(), count))
}
