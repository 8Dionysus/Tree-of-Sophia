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

// Browser demand-driven observations preserve repeated/lazy JS property reads.
// Phases: 0 query, 1 availability, 2 minimum, 3 tail availability, 4 done.
pub struct BrowserSearchMode {
    requested: Option<SearchMode>,
    current: SearchMode,
    phase: u8,
    query_points: Option<usize>,
    selected: Option<SearchMode>,
    error: Option<SearchSelectionError>,
}
impl BrowserSearchMode {
    pub fn new(requested: u8) -> Self {
        let mode = match requested {
            1 => Some(SearchMode::Indexed),
            2 => Some(SearchMode::Compressed),
            _ => None,
        };
        let mut result = Self {
            requested: mode,
            current: mode.unwrap_or(SearchMode::Indexed),
            phase: 0,
            query_points: None,
            selected: None,
            error: None,
        };
        if requested > 2 {
            result.fail(SearchSelectionErrorCode::InvalidMode);
        }
        result
    }
    fn fail(&mut self, code: SearchSelectionErrorCode) {
        self.error = Some(SearchSelectionError::new(code));
        self.phase = 4;
    }
    pub fn phase(&self) -> u8 {
        self.phase
    }
    pub fn current(&self) -> SearchMode {
        self.current
    }
    pub fn selected(&self) -> Option<SearchMode> {
        self.selected
    }
    pub fn error(&self) -> Option<SearchSelectionError> {
        self.error
    }
    pub fn query(&mut self, present: bool, units: &[u16]) {
        if self.phase != 0 {
            return;
        }
        if present {
            // A lone surrogate has exactly the same strip/lower/count classes
            // as U+FFFD: one uncased, non-ignorable, non-whitespace code point.
            // This internal count carrier is never returned or sent as query.
            let text: String = char::decode_utf16(units.iter().copied())
                .map(|item| item.unwrap_or('\u{fffd}'))
                .collect();
            let input = units.len();
            let count = python_strip_unicode16_v1(&text, input)
                .and_then(|stripped| {
                    python_lower_unicode16_v1(
                        stripped,
                        input,
                        input.saturating_mul(2),
                        input.saturating_mul(6),
                    )
                })
                .map(|lowered| lowered.chars().count());
            match count {
                Ok(value) => self.query_points = Some(value),
                Err(_) => {
                    self.fail(SearchSelectionErrorCode::UnicodeBudgetExceeded);
                    return;
                }
            }
        }
        self.phase = 1;
    }
    fn query_count(&mut self, count: Option<usize>) {
        self.query_points = count;
        self.phase = 1;
    }
    fn next(&mut self) {
        if self.current == SearchMode::Indexed {
            self.current = SearchMode::Compressed;
            self.phase = 1;
        } else {
            self.current = SearchMode::Indexed;
            self.phase = 3;
        }
    }
    pub fn availability(&mut self, available: bool) {
        if self.phase == 3 {
            if available {
                self.fail(SearchSelectionErrorCode::NoEligibleMode);
            } else if self.current == SearchMode::Indexed {
                self.current = SearchMode::Compressed;
            } else {
                self.fail(SearchSelectionErrorCode::EnginesUnavailable);
            }
        } else if self.phase == 1 {
            if available {
                self.phase = 2;
            } else if self.requested.is_some() {
                self.fail(SearchSelectionErrorCode::ModeUnavailable);
            } else {
                self.next();
            }
        }
    }
    pub fn minimum(&mut self, nullish: bool, numeric: bool, value: f64) {
        if self.phase != 2 {
            return;
        }
        let floor = if nullish { 1.0 } else { value };
        if !nullish
            && (!numeric
                || !floor.is_finite()
                || floor.fract() != 0.0
                || floor < 1.0
                || floor > MAX_SAFE_JS_INTEGER as f64)
        {
            self.fail(SearchSelectionErrorCode::InvalidCapability);
            return;
        }
        let floor = floor as u64;
        if self
            .query_points
            .is_none_or(|points| u64::try_from(points).is_ok_and(|points| points >= floor))
        {
            self.selected = Some(self.current);
            self.phase = 4;
        } else if self.requested.is_some() {
            self.error = Some(SearchSelectionError::with_minimum(
                SearchSelectionErrorCode::QueryTooShort,
                floor,
            ));
            self.phase = 4;
        } else {
            self.next();
        }
    }
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
    let query = match field(request, "query") {
        None => None,
        Some(value) => {
            let query = value
                .as_str()
                .ok_or_else(|| SearchSelectionError::new(SearchSelectionErrorCode::InvalidInput))?;
            let stripped = python_strip_unicode16_v1(query, MAX_QUERY_POINTS).map_err(|_| {
                SearchSelectionError::new(SearchSelectionErrorCode::UnicodeBudgetExceeded)
            })?;
            // Preserve the portable JSON ABI's existing bounded Unicode work.
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
    let mut session = BrowserSearchMode::new(match requested {
        None => 0,
        Some(SearchMode::Indexed) => 1,
        Some(SearchMode::Compressed) => 2,
    });
    session.query_count(query);
    while session.phase() != 4 {
        match session.phase() {
            1 | 3 => session.availability(available(capabilities, session.current())),
            2 => {
                let floor = minimum(capabilities, session.current())?;
                session.minimum(false, true, floor as f64);
            }
            _ => {
                return Err(SearchSelectionError::new(
                    SearchSelectionErrorCode::InvalidInput,
                ));
            }
        }
    }
    session.selected().ok_or_else(|| {
        session.error().unwrap_or(SearchSelectionError::new(
            SearchSelectionErrorCode::InvalidInput,
        ))
    })
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
