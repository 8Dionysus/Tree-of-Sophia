use tos_foundation::{
    JsonLimits, JsonMode, JsonValue, parse_json, python_lower_unicode16_v1,
    python_strip_unicode16_v1,
};

const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_REQUEST_BYTES: usize = 1_048_576;
const MAX_QUERY_POINTS: usize = 1_048_576;
const MAX_LOWER_POINTS: usize = 2_097_152;
const MAX_LOWER_BYTES: usize = 8_388_608;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchMode {
    Indexed,
    Compressed,
}

impl SearchMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Indexed => "indexed",
            Self::Compressed => "compressed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchSelectionErrorCode {
    InvalidInput,
    InvalidMode,
    InvalidCapability,
    ModeUnavailable,
    QueryTooShort,
    NoEligibleMode,
    EnginesUnavailable,
    UnicodeBudgetExceeded,
}

impl SearchSelectionErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidMode => "invalid_mode",
            Self::InvalidCapability => "invalid_capability",
            Self::ModeUnavailable => "mode_unavailable",
            Self::QueryTooShort => "query_too_short",
            Self::NoEligibleMode => "no_eligible_mode",
            Self::EnginesUnavailable => "engines_unavailable",
            Self::UnicodeBudgetExceeded => "unicode_budget_exceeded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchSelectionError {
    pub code: SearchSelectionErrorCode,
    /// Present only for an explicit mode rejected below its advertised floor.
    pub minimum: Option<u64>,
}

impl SearchSelectionError {
    const fn new(code: SearchSelectionErrorCode) -> Self {
        Self {
            code,
            minimum: None,
        }
    }
    const fn with_minimum(code: SearchSelectionErrorCode, minimum: u64) -> Self {
        Self {
            code,
            minimum: Some(minimum),
        }
    }
}

fn field<'a>(object: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    object.object_get(name)
}

fn mode_descriptor<'a>(capabilities: &'a JsonValue, mode: SearchMode) -> Option<&'a JsonValue> {
    let modes = field(capabilities, "modes")?;
    let descriptor = field(modes, mode.as_str())?;
    descriptor.as_object().map(|_| descriptor)
}

fn available(capabilities: &JsonValue, mode: SearchMode) -> bool {
    mode_descriptor(capabilities, mode)
        .and_then(|descriptor| field(descriptor, "available"))
        .and_then(JsonValue::as_bool)
        == Some(true)
}

fn minimum(capabilities: &JsonValue, mode: SearchMode) -> Result<u64, SearchSelectionError> {
    let value = mode_descriptor(capabilities, mode)
        .and_then(|descriptor| field(descriptor, "min_normalized_query_code_points"));
    let Some(value) = value else { return Ok(1) };
    if value.is_null() {
        return Ok(1);
    }
    // JS Number.isSafeInteger accepts 1.0 and 1e0. Lexemes are parsed only
    // for this compatibility check, never used for source-number identity.
    let number = match value {
        JsonValue::Number(number) => number.lexeme.parse::<f64>().ok(),
        _ => None,
    };
    match number {
        Some(number)
            if number.is_finite()
                && number.fract() == 0.0
                && number >= 1.0
                && number <= MAX_SAFE_JS_INTEGER as f64 =>
        {
            Ok(number as u64)
        }
        _ => Err(SearchSelectionError::new(
            SearchSelectionErrorCode::InvalidCapability,
        )),
    }
}

fn eligible(
    capabilities: &JsonValue,
    mode: SearchMode,
    query_points: Option<usize>,
) -> Result<bool, SearchSelectionError> {
    if !available(capabilities, mode) {
        return Ok(false);
    }
    let floor = minimum(capabilities, mode)?;
    Ok(query_points.is_none_or(|length| u64::try_from(length).is_ok_and(|length| length >= floor)))
}

/// Select only the page adapter's advertised bounded mode. Input is a JSON
/// object with `capabilities`, optional `requested_mode`, and optional `query`.
/// An omitted query means the caller is asking only about mode availability.
/// A continuation must supply its retained `requested_mode` in this request.
///
/// This function uses the request-last-wins profile for adapter input. It
/// rejects escaped lone surrogates instead of silently replacing Unicode.
pub fn select_knowledge_search_mode_v1(raw: &[u8]) -> Result<SearchMode, SearchSelectionError> {
    let limits = JsonLimits {
        max_bytes: MAX_REQUEST_BYTES,
        ..JsonLimits::default()
    };
    let document = parse_json(raw, JsonMode::RequestLastWins, limits)
        .map_err(|_| SearchSelectionError::new(SearchSelectionErrorCode::InvalidInput))?;
    let request = document.root();
    if request.as_object().is_none() {
        return Err(SearchSelectionError::new(
            SearchSelectionErrorCode::InvalidInput,
        ));
    }
    let capabilities = field(request, "capabilities")
        .ok_or_else(|| SearchSelectionError::new(SearchSelectionErrorCode::InvalidInput))?;
    let requested = match field(request, "requested_mode") {
        None => None,
        Some(value) => match value.as_str() {
            Some("indexed") => Some(SearchMode::Indexed),
            Some("compressed") => Some(SearchMode::Compressed),
            _ => {
                return Err(SearchSelectionError::new(
                    SearchSelectionErrorCode::InvalidMode,
                ));
            }
        },
    };
    let query_points = match field(request, "query") {
        None => None,
        Some(value) => {
            let query = value
                .as_str()
                .ok_or_else(|| SearchSelectionError::new(SearchSelectionErrorCode::InvalidInput))?;
            let stripped = python_strip_unicode16_v1(query, MAX_QUERY_POINTS).map_err(|_| {
                SearchSelectionError::new(SearchSelectionErrorCode::UnicodeBudgetExceeded)
            })?;
            let lowered = python_lower_unicode16_v1(
                stripped,
                MAX_QUERY_POINTS,
                MAX_LOWER_POINTS,
                MAX_LOWER_BYTES,
            )
            .map_err(|_| {
                SearchSelectionError::new(SearchSelectionErrorCode::UnicodeBudgetExceeded)
            })?;
            Some(lowered.chars().count())
        }
    };
    if let Some(mode) = requested {
        if !available(capabilities, mode) {
            return Err(SearchSelectionError::new(
                SearchSelectionErrorCode::ModeUnavailable,
            ));
        }
        if !eligible(capabilities, mode, query_points)? {
            return Err(SearchSelectionError::with_minimum(
                SearchSelectionErrorCode::QueryTooShort,
                minimum(capabilities, mode)?,
            ));
        }
        return Ok(mode);
    }
    for mode in [SearchMode::Indexed, SearchMode::Compressed] {
        if eligible(capabilities, mode, query_points)? {
            return Ok(mode);
        }
    }
    if available(capabilities, SearchMode::Indexed)
        || available(capabilities, SearchMode::Compressed)
    {
        Err(SearchSelectionError::new(
            SearchSelectionErrorCode::NoEligibleMode,
        ))
    } else {
        Err(SearchSelectionError::new(
            SearchSelectionErrorCode::EnginesUnavailable,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select(raw: &str) -> Result<SearchMode, SearchSelectionError> {
        select_knowledge_search_mode_v1(raw.as_bytes())
    }

    #[test]
    fn advertised_preference_and_explicit_continuation() {
        let caps = r#"{"modes":{"indexed":{"available":true,"min_normalized_query_code_points":3},"compressed":{"available":true}}}"#;
        let request = format!(r#"{{"capabilities":{caps},"query":"道"}}"#);
        assert_eq!(select(&request), Ok(SearchMode::Compressed));
        let request =
            format!(r#"{{"capabilities":{caps},"requested_mode":"indexed","query":"fate"}}"#);
        assert_eq!(select(&request), Ok(SearchMode::Indexed));
        let request =
            format!(r#"{{"capabilities":{caps},"requested_mode":"compressed","query":"fate"}}"#);
        assert_eq!(select(&request), Ok(SearchMode::Compressed));
    }

    #[test]
    fn unicode_16_strip_lower_and_scalar_count() {
        let caps =
            r#"{"modes":{"indexed":{"available":true,"min_normalized_query_code_points":2}}}"#;
        let dotted_i = format!(r#"{{"capabilities":{caps},"query":"  İ  "}}"#);
        assert_eq!(select(&dotted_i), Ok(SearchMode::Indexed));
        let emoji = format!(r#"{{"capabilities":{caps},"query":"😀"}}"#);
        assert_eq!(
            select(&emoji).unwrap_err().code,
            SearchSelectionErrorCode::NoEligibleMode
        );
    }

    #[test]
    fn no_legacy_fallback_and_malformed_capability_refusal() {
        let none = r#"{"capabilities":{"modes":{"indexed":{"available":false},"compressed":{"available":false}}},"query":"fate"}"#;
        assert_eq!(
            select(none).unwrap_err().code,
            SearchSelectionErrorCode::EnginesUnavailable
        );
        let short = r#"{"capabilities":{"modes":{"indexed":{"available":true,"min_normalized_query_code_points":3}}},"requested_mode":"indexed","query":"道"}"#;
        assert_eq!(
            select(short),
            Err(SearchSelectionError::with_minimum(
                SearchSelectionErrorCode::QueryTooShort,
                3
            ))
        );
        let bad = r#"{"capabilities":{"modes":{"indexed":{"available":true,"min_normalized_query_code_points":true}}},"query":"fate"}"#;
        assert_eq!(
            select(bad).unwrap_err().code,
            SearchSelectionErrorCode::InvalidCapability
        );
        let unavailable = r#"{"capabilities":{"modes":{"indexed":{"available":false,"min_normalized_query_code_points":true}}},"requested_mode":"indexed","query":"fate"}"#;
        assert_eq!(
            select(unavailable).unwrap_err().code,
            SearchSelectionErrorCode::ModeUnavailable
        );
    }

    #[test]
    fn request_last_wins_and_invalid_surrogate_refusal() {
        let changed = r#"{"capabilities":{"modes":{"indexed":{"available":false}}},"capabilities":{"modes":{"indexed":{"available":true}}},"query":"fate"}"#;
        assert_eq!(select(changed), Ok(SearchMode::Indexed));
        let invalid =
            r#"{"capabilities":{"modes":{"indexed":{"available":true}}},"query":"\ud800"}"#;
        assert_eq!(
            select(invalid).unwrap_err().code,
            SearchSelectionErrorCode::InvalidInput
        );
    }
}
