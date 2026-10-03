//! Candidate-side source index and exact retirement-only transfer.
//!
//! This is the maintained `corpus_source_validation.source_index` /
//! `corpus_source_retirement` kernel. Inputs come from root's held candidate
//! custody and the complete freshly rendered owner report. Nothing here selects
//! a store revision, writes bytes, accepts source meaning, or grants rights.
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    io,
};
use tos_foundation::{
    Digest256, FoundationErrorCode, JsonLimits, JsonMode, JsonValue, RelativePath, parse_json,
    parse_json_with_state_budget,
};

pub const RETIREMENT_SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
pub const MAX_EVENT_BYTES: usize = 1_048_576;
const STRUCTURED_JSON_BYTES: u64 = 16 * 1024 * 1024;

/// All closures use ONE caller-owned cumulative byte/state/deadline budget.
/// `read` verifies length/digest against membership before returning bytes;
/// `verify_member` streams the same verification without retaining a large
/// review document. Neither may resolve a mutable checkout pathname.
pub struct CandidateInput<'a> {
    pub members: &'a BTreeMap<String, Value>,
    pub read: &'a mut dyn FnMut(&str, usize) -> io::Result<Vec<u8>>,
    pub verify_member: &'a mut dyn FnMut(&str) -> io::Result<()>,
    pub check: &'a mut dyn FnMut() -> io::Result<()>,
    pub json: JsonLimits,
    /// Per-document Foundation parser workspace, reserved independently from
    /// raw/decoded bytes and the growing retained source index.
    pub json_state_bytes: usize,
}
/// Constructed only by the caller receiving its complete sealed FND report.
/// Catalog rows are from that same fresh render, not an existing catalog file.
pub struct FreshRows {
    pub records: Vec<Value>,
    pub claims: Vec<Value>,
    pub native_semantic: BTreeMap<String, Vec<String>>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Index {
    pub identities: BTreeMap<String, String>,
    pub dependencies: BTreeMap<String, Vec<String>>,
}
#[derive(Clone, Copy)]
pub struct IndexLimits {
    pub max_edges: usize,
    pub max_state_bytes: usize,
}
/// Program-selected native schema execution. Both raw inputs are candidate
/// bound. None means valid; Some is the actual first schema diagnostic.
pub type SchemaCheck<'a> = dyn FnMut(&str, &[u8], &[u8]) -> io::Result<Option<String>> + 'a;

fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}
fn decode_budget() -> io::Error {
    io::Error::new(
        io::ErrorKind::FileTooLarge,
        "JSON decoding byte budget exceeded",
    )
}
fn field<'a>(value: &'a Value, key: &str) -> io::Result<&'a Value> {
    value
        .as_object()
        .and_then(|v| v.get(key))
        .ok_or_else(|| invalid(format!("source index input lacks {key}")))
}
fn text(value: &Value) -> io::Result<&str> {
    value
        .as_str()
        .ok_or_else(|| invalid("source index string field is invalid"))
}
fn string<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    text(field(value, key)?)
}
fn array(value: &Value) -> io::Result<&[Value]> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("source index array field is invalid"))
}
fn member_size(value: &Value) -> io::Result<u64> {
    field(value, "size_bytes")?
        .as_u64()
        .ok_or_else(|| invalid("candidate member size is invalid"))
}
impl CandidateInput<'_> {
    fn tick(&mut self) -> io::Result<()> {
        (self.check)()
    }
    fn bytes(&mut self, path: &str, cap: usize) -> io::Result<Vec<u8>> {
        self.tick()?;
        let members = self.members;
        let member = members
            .get(path)
            .ok_or_else(|| invalid("source companion is missing"))?;
        let size = member_size(member)?;
        if size > cap as u64 {
            return Err(invalid("source companion exceeds its bounded size"));
        }
        let raw = (self.read)(path, cap)?;
        self.tick()?;
        if raw.len() as u64 != size
            || Digest256::of_bytes(&raw).to_hex() != string(member, "sha256")?
        {
            return Err(invalid(
                "candidate source bytes differ from exact membership",
            ));
        }
        Ok(raw)
    }
}
struct State {
    limits: IndexLimits,
    bytes: usize,
    edges: usize,
}
impl State {
    fn new(limits: IndexLimits) -> io::Result<Self> {
        if limits.max_edges == 0
            || limits.max_edges == usize::MAX
            || limits.max_state_bytes == 0
            || limits.max_state_bytes == usize::MAX
        {
            return Err(invalid("invalid source index limits"));
        }
        Ok(Self {
            limits,
            bytes: 0,
            edges: 0,
        })
    }
    fn reserve(&mut self, bytes: usize, edge: bool) -> io::Result<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_state_bytes)
            .ok_or_else(|| invalid("source index state budget exceeded"))?;
        if edge {
            self.edges = self
                .edges
                .checked_add(1)
                .filter(|n| *n <= self.limits.max_edges)
                .ok_or_else(|| invalid("source index edge budget exceeded"))?;
        }
        Ok(())
    }
}
fn bind(
    input: &mut CandidateInput<'_>,
    index: &mut Index,
    state: &mut State,
    id: &str,
    path: &str,
) -> io::Result<()> {
    input.tick()?;
    if !input.members.contains_key(path) {
        return Err(invalid(
            "source identity is outside the admitted member set",
        ));
    }
    if let Some(old) = index.identities.get(id) {
        if old != path {
            return Err(invalid("duplicate source identity"));
        }
        return Ok(());
    }
    state.reserve(
        id.len().saturating_add(path.len()).saturating_add(96),
        false,
    )?;
    index.identities.insert(id.to_owned(), path.to_owned());
    Ok(())
}
fn edge(
    deps: &mut BTreeMap<String, BTreeSet<String>>,
    state: &mut State,
    source: &str,
    target: &str,
) -> io::Result<()> {
    if source == target || deps.get(source).is_some_and(|s| s.contains(target)) {
        return Ok(());
    }
    if !deps.contains_key(source) {
        state.reserve(source.len().saturating_add(96), false)?;
        deps.insert(source.to_owned(), BTreeSet::new());
    }
    state.reserve(target.len().saturating_add(64), true)?;
    deps.get_mut(source)
        .ok_or_else(|| invalid("dependency accumulator missing"))?
        .insert(target.to_owned());
    Ok(())
}
/// Python json.loads(bytes) observes BOM / zero-pattern UTF-16 and UTF-32.
/// Raw digest custody precedes decoding. This does not normalize authored text.
fn json_encoding(raw: &[u8], cap: usize) -> io::Result<Vec<u8>> {
    if raw.len() > cap {
        return Err(decode_budget());
    }
    let (skip, width, little) = if raw.starts_with(&[0, 0, 0xfe, 0xff]) {
        (4, 4, false)
    } else if raw.starts_with(&[0xff, 0xfe, 0, 0]) {
        (4, 4, true)
    } else if raw.starts_with(&[0xfe, 0xff]) {
        (2, 2, false)
    } else if raw.starts_with(&[0xff, 0xfe]) {
        (2, 2, true)
    } else if raw.starts_with(&[0xef, 0xbb, 0xbf]) {
        return utf8_surrogatepass(&raw[3..], cap);
    } else if raw.len() >= 4 && raw[0] == 0 && raw[1] == 0 && raw[2] == 0 {
        (0, 4, false)
    } else if raw.len() >= 4 && raw[1] == 0 && raw[2] == 0 && raw[3] == 0 {
        (0, 4, true)
    } else if raw.len() >= 2 && raw[0] == 0 {
        (0, 2, false)
    } else if raw.len() >= 2 && raw[1] == 0 {
        (0, 2, true)
    } else {
        return utf8_surrogatepass(raw, cap);
    };
    let bytes = &raw[skip..];
    if bytes.len() % width != 0 {
        return Err(invalid("invalid JSON byte encoding"));
    }
    let mut result = String::new();
    if width == 2 {
        let units = bytes.chunks_exact(2).map(|v| {
            if little {
                u16::from_le_bytes([v[0], v[1]])
            } else {
                u16::from_be_bytes([v[0], v[1]])
            }
        });
        // Python's surrogatepass retains lone literal surrogates. Represent them
        // as JSON escapes so the existing FND legacy parser retains WTF-16.
        for value in char::decode_utf16(units) {
            let extra = match &value {
                Ok(c) => c.len_utf8(),
                Err(_) => 6,
            };
            if extra > cap.saturating_sub(result.len()) {
                return Err(decode_budget());
            }
            match value {
                Ok(c) => result.push(c),
                Err(e) => result.push_str(&format!("\\u{:04x}", e.unpaired_surrogate())),
            }
        }
    } else {
        for v in bytes.chunks_exact(4) {
            let n = if little {
                u32::from_le_bytes([v[0], v[1], v[2], v[3]])
            } else {
                u32::from_be_bytes([v[0], v[1], v[2], v[3]])
            };
            let extra = char::from_u32(n).map_or(6, char::len_utf8);
            if extra > cap.saturating_sub(result.len()) {
                return Err(decode_budget());
            }
            if let Some(c) = char::from_u32(n) {
                result.push(c)
            } else if (0xd800..=0xdfff).contains(&n) {
                result.push_str(&format!("\\u{n:04x}"))
            } else {
                return Err(invalid("invalid JSON byte encoding"));
            }
        }
    }
    Ok(result.into_bytes())
}
fn utf8_surrogatepass(mut raw: &[u8], cap: usize) -> io::Result<Vec<u8>> {
    let mut result = Vec::new();
    loop {
        match std::str::from_utf8(raw) {
            Ok(_) => {
                if raw.len() > cap.saturating_sub(result.len()) {
                    return Err(decode_budget());
                }
                result.extend_from_slice(raw);
                break;
            }
            Err(error) => {
                let good = error.valid_up_to();
                if good.saturating_add(6) > cap.saturating_sub(result.len()) {
                    return Err(decode_budget());
                }
                result.extend_from_slice(&raw[..good]);
                raw = &raw[good..];
                if raw.len() < 3
                    || raw[0] != 0xed
                    || !(0xa0..=0xbf).contains(&raw[1])
                    || !(0x80..=0xbf).contains(&raw[2])
                {
                    return Err(invalid("invalid JSON UTF-8 encoding"));
                }
                let unit = ((raw[0] as u16 & 15) << 12)
                    | ((raw[1] as u16 & 63) << 6)
                    | (raw[2] as u16 & 63);
                result.extend_from_slice(format!("\\u{unit:04x}").as_bytes());
                raw = &raw[3..];
            }
        }
    }
    Ok(result)
}
fn document(
    raw: &[u8],
    mode: JsonMode,
    limits: JsonLimits,
    state_bytes: usize,
) -> io::Result<Option<JsonValue>> {
    let decoded = match json_encoding(raw, limits.max_bytes) {
        Ok(v) => v,
        Err(error) if error.kind() == io::ErrorKind::FileTooLarge => return Err(error),
        Err(_) => return Ok(None),
    };
    // Decode expansion is charged to the selected parser's byte limit.
    match parse_json_with_state_budget(&decoded, mode, limits, state_bytes) {
        Ok(v) => Ok(Some(v.into_root())),
        Err(e) if e.code == FoundationErrorCode::BudgetExceeded => Err(invalid(e)),
        Err(_) => Ok(None),
    }
}
fn strict_object(raw: &[u8], limits: JsonLimits) -> io::Result<Value> {
    let decoded = json_encoding(raw, limits.max_bytes)?;
    let value = parse_json(&decoded, JsonMode::PublishedStrict, limits)
        .map_err(|e| match e.code {
            FoundationErrorCode::DuplicateMember => {
                invalid("retirement JSON contains duplicate fields")
            }
            FoundationErrorCode::BudgetExceeded => invalid(e),
            _ => invalid("retirement input is not finite JSON"),
        })?
        .into_root();
    fn convert(v: &JsonValue) -> io::Result<Value> {
        Ok(match v {
            JsonValue::Null => Value::Null,
            JsonValue::Bool(v) => Value::Bool(*v),
            JsonValue::String(v) => Value::String(
                v.as_str()
                    .ok_or_else(|| invalid("retirement input requires scalar strings"))?
                    .to_owned(),
            ),
            JsonValue::Number(n) => {
                if n.lexeme.contains(['.', 'e', 'E'])
                    && !n.lexeme.parse::<f64>().is_ok_and(f64::is_finite)
                {
                    return Err(invalid("retirement input is not finite JSON"));
                }
                Value::Number(n.lexeme.parse().map_err(invalid)?)
            }
            JsonValue::Array(v) => Value::Array(v.iter().map(convert).collect::<io::Result<_>>()?),
            JsonValue::Object(v) => {
                let mut out = serde_json::Map::new();
                for (k, v) in v {
                    out.insert(
                        k.as_str()
                            .ok_or_else(|| invalid("retirement input requires scalar keys"))?
                            .to_owned(),
                        convert(v)?,
                    );
                }
                Value::Object(out)
            }
        })
    }
    let result = convert(&value)?;
    if !result.is_object() {
        return Err(invalid("retirement input must be a JSON object"));
    }
    Ok(result)
}
fn event_limits(input: &CandidateInput<'_>) -> JsonLimits {
    JsonLimits {
        max_bytes: MAX_EVENT_BYTES.min(input.json.max_bytes),
        ..input.json
    }
}
fn reference_path(value: &str) -> Cow<'_, str> {
    let value = value.split('#').next().unwrap_or(value);
    if let Some((path, tail)) = value.rsplit_once(':') {
        // Regex $ also permits a final LF; all other trailing whitespace stays.
        let tail = tail.strip_suffix('\n').unwrap_or(tail);
        if !tail.is_empty()
            && tail
                .chars()
                .all(tos_foundation::python_decimal_unicode16_v1)
        {
            return if value.ends_with('\n') {
                Cow::Owned(format!("{path}\n"))
            } else {
                Cow::Borrowed(path)
            };
        }
    }
    Cow::Borrowed(value)
}
fn references(
    input: &mut CandidateInput<'_>,
    index: &Index,
    deps: &mut BTreeMap<String, BTreeSet<String>>,
    state: &mut State,
    source: &str,
    value: &JsonValue,
) -> io::Result<()> {
    input.tick()?;
    match value {
        JsonValue::String(s) => {
            if let Some(s) = s.as_str() {
                if let Some(target) = index.identities.get(s) {
                    edge(deps, state, source, target)?;
                }
                if s.starts_with("ToS/") {
                    let p = reference_path(s);
                    if input.members.contains_key(p.as_ref()) {
                        edge(deps, state, source, p.as_ref())?;
                    }
                }
            } else {
                // An unpaired surrogate in a fragment must not erase a valid
                // scalar path before '#'. It cannot match a scalar identity.
                let prefix = s
                    .units()
                    .split(|u| *u == b'#' as u16)
                    .next()
                    .unwrap_or(s.units());
                if let Ok(prefix) = String::from_utf16(prefix) {
                    if prefix.starts_with("ToS/") {
                        let path = reference_path(&prefix);
                        if input.members.contains_key(path.as_ref()) {
                            edge(deps, state, source, path.as_ref())?;
                        }
                    }
                }
            }
        }
        JsonValue::Array(v) => {
            for item in v {
                references(input, index, deps, state, source, item)?;
            }
        }
        JsonValue::Object(v) => {
            for (_, item) in v {
                references(input, index, deps, state, source, item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Existing standalone kernel callers retain the original return contract.
pub fn build_index(
    input: &mut CandidateInput<'_>,
    fresh: &FreshRows,
    base: Option<&Index>,
    limits: IndexLimits,
    schemas: &mut SchemaCheck<'_>,
) -> io::Result<Index> {
    build_index_accounted(input, fresh, base, limits, schemas).map(|(index, _)| index)
}
/// Same kernel and pre-allocation ledger; return its retained logical upper
/// bound so the complete admission operation does not lose index state at the
/// phase boundary. This is not an allocator/RSS measurement.
pub fn build_index_accounted(
    input: &mut CandidateInput<'_>,
    fresh: &FreshRows,
    base: Option<&Index>,
    limits: IndexLimits,
    schemas: &mut SchemaCheck<'_>,
) -> io::Result<(Index, usize)> {
    input.tick()?;
    let mut state = State::new(limits)?;
    state.reserve(std::mem::size_of::<Index>(), false)?;
    let mut index = Index::default();
    for row in &fresh.records {
        bind(
            input,
            &mut index,
            &mut state,
            string(row, "record_id")?,
            string(row, "source_record_ref")?,
        )?;
    }
    for row in &fresh.claims {
        bind(
            input,
            &mut index,
            &mut state,
            string(row, "claim_id")?,
            string(row, "source_claim_file_ref")?,
        )?;
    }
    for (id, refs) in &fresh.native_semantic {
        input.tick()?;
        let previous = base
            .and_then(|b| b.identities.get(id))
            .filter(|p| refs.contains(p));
        let anchor = previous
            .or_else(|| refs.iter().min())
            .ok_or_else(|| invalid("native semantic identity has no version path"))?;
        for path in refs {
            if !input.members.contains_key(path) {
                return Err(invalid(
                    "source identity is outside the admitted member set",
                ));
            }
        }
        bind(input, &mut index, &mut state, id, anchor)?;
    }
    let mut schema = None;
    // Borrowed membership paths avoid a second complete path inventory.
    let members = input.members;
    for path in members.keys() {
        if path.starts_with("ToS/source-witnesses/retirements/") && path.ends_with(".json") {
            let raw = input.bytes(path, MAX_EVENT_BYTES)?;
            let row = strict_object(&raw, event_limits(input))?;
            if schema.is_none() {
                let raw = input.bytes(RETIREMENT_SCHEMA, MAX_EVENT_BYTES)?;
                strict_object(&raw, event_limits(input))?;
                schema = Some(raw);
            }
            let schema = schema
                .as_deref()
                .ok_or_else(|| invalid("retirement schema missing"))?;
            if schemas(path, &raw, schema)?.is_some()
                || string(&row, "schema_version")? != "tos_provenance_event_v1"
                || string(&row, "event_type")? != "migration"
                || string(field(&row, "method")?, "name")? != "corpus-source-retirement"
                || !string(&row, "event_id")?.starts_with("tos.event.")
            {
                return Err(invalid(
                    "source retirement record has an invalid operation or ID",
                ));
            }
            bind(
                input,
                &mut index,
                &mut state,
                string(&row, "event_id")?,
                path,
            )?;
        }
    }
    let mut deps = BTreeMap::new();
    for (path, member) in members {
        input.tick()?;
        if path.starts_with("ToS/contracts/")
            || path.starts_with("ToS/doctrine/semantic-interchange/")
        {
            continue;
        }
        if path.ends_with(".json") && member_size(member)? <= STRUCTURED_JSON_BYTES {
            let raw = input.bytes(path, input.json.max_bytes)?;
            if let Some(row) = document(
                &raw,
                JsonMode::LegacyPythonObserved,
                input.json,
                input.json_state_bytes,
            )? {
                references(input, &index, &mut deps, &mut state, path, &row)?;
            }
        } else if path.ends_with(".jsonl") {
            let raw = input.bytes(path, input.json.max_bytes)?;
            for line in raw.split(|b| *b == b'\n') {
                input.tick()?;
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                if let Some(row) = document(
                    line,
                    JsonMode::LegacyPythonObserved,
                    input.json,
                    input.json_state_bytes,
                )? {
                    references(input, &index, &mut deps, &mut state, path, &row)?;
                }
            }
        }
    }
    for (id, refs) in &fresh.native_semantic {
        let anchor = index
            .identities
            .get(id)
            .ok_or_else(|| invalid("native semantic anchor missing"))?;
        for path in refs {
            input.tick()?;
            edge(&mut deps, &mut state, anchor, path)?;
        }
    }
    // Moving strings, not cloning another complete dependency index.
    index.dependencies = deps
        .into_iter()
        .map(|(p, v)| (p, v.into_iter().collect()))
        .collect();
    input.tick()?;
    Ok((index, state.bytes))
}

pub fn validate_retirements(
    input: &mut CandidateInput<'_>,
    retirements: &[Value],
    base: Option<&Value>,
    schemas: &mut SchemaCheck<'_>,
) -> io::Result<BTreeMap<String, String>> {
    input.tick()?;
    if retirements.is_empty() {
        return Ok(BTreeMap::new());
    }
    let base = base.ok_or_else(|| invalid("source retirement requires an accepted base"))?;
    let schema = input.bytes(RETIREMENT_SCHEMA, MAX_EVENT_BYTES)?;
    strict_object(&schema, event_limits(input))?;
    let mut groups: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for row in retirements {
        input.tick()?;
        groups
            .entry(string(row, "event_ref")?)
            .or_default()
            .push(row);
    }
    let mut identities = BTreeMap::new();
    for (event_ref, rows) in groups {
        input.tick()?;
        if !event_ref.starts_with("ToS/source-witnesses/retirements/")
            || !event_ref.ends_with(".json")
        {
            return Err(invalid(
                "retirement event must use the source retirement owner path",
            ));
        }
        let raw = input.bytes(event_ref, MAX_EVENT_BYTES)?;
        let event = strict_object(&raw, event_limits(input))?;
        if let Some(issue) = schemas(event_ref, &raw, &schema)? {
            return Err(invalid(format!(
                "{event_ref}: retirement event violates provenance schema: {issue}"
            )));
        }
        let method = field(&event, "method")?;
        let id = string(&event, "event_id")?;
        if string(&event, "schema_version")? != "tos_provenance_event_v1"
            || string(&event, "event_type")? != "migration"
            || !id.starts_with("tos.event.")
            || !matches!(
                string(&event, "status")?,
                "completed" | "completed_with_warnings"
            )
            || string(method, "name")? != "corpus-source-retirement"
            || string(method, "version")? != "1"
        {
            return Err(invalid(
                "retirement event does not declare the source retirement operation",
            ));
        }
        if tos_validation::retirement_rules::observed_datetime_order(
            string(&event, "started_at")?,
            string(&event, "ended_at")?,
        )
        .map_err(|e| invalid(format!("retirement date-time invalid: {e:?}")))?
            == std::cmp::Ordering::Greater
        {
            return Err(invalid("retirement event ends before it starts"));
        }
        let config = field(method, "configuration")?;
        let keys = config
            .as_object()
            .ok_or_else(|| invalid("retirement configuration must be an object"))?;
        if keys.len() != 5
            || [
                "base_revision",
                "retirements",
                "reason",
                "review_ref",
                "review_sha256",
            ]
            .iter()
            .any(|k| !keys.contains_key(*k))
        {
            return Err(invalid(
                "retirement configuration must bind base, exact targets and owner review",
            ));
        }
        let mut targets = rows
            .iter()
            .map(|r| Ok((string(r, "path")?, string(r, "sha256")?)))
            .collect::<io::Result<Vec<_>>>()?;
        targets.sort_by(|a, b| a.0.cmp(b.0));
        let targets = targets
            .into_iter()
            .map(|(p, s)| json!({"path":p,"sha256":s}))
            .collect::<Vec<_>>();
        if field(config, "base_revision")? != field(base, "revision")?
            || field(config, "retirements")? != &json!(targets)
        {
            return Err(invalid(
                "retirement targets or accepted base differ from the source batch",
            ));
        }
        if tos_foundation::python_strip_unicode16_v1(
            string(config, "reason")?,
            input.json.max_bytes,
        )
        .map_err(invalid)?
        .is_empty()
        {
            return Err(invalid("retirement needs a source-visible reason"));
        }
        let review = string(config, "review_ref")?;
        RelativePath::parse(review).map_err(invalid)?;
        let digest = string(config, "review_sha256")?;
        if Digest256::from_hex(digest).is_err()
            || digest
                .bytes()
                .any(|b| !(b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        {
            return Err(invalid("retirement review digest is invalid"));
        }
        if !review.starts_with("ToS/review-ledger/") || review == event_ref {
            return Err(invalid(
                "retirement review must return to the source-owned review ledger",
            ));
        }
        let metadata = input
            .members
            .get(review)
            .ok_or_else(|| invalid("retirement review is missing"))?;
        if string(metadata, "sha256")? != digest || member_size(metadata)? == 0 {
            return Err(invalid(
                "retirement review is empty or has a different digest",
            ));
        }
        input.tick()?;
        (input.verify_member)(review)?;
        input.tick()?;
        let mut expected_inputs=targets.iter().map(|r|Ok(json!({"ref":string(r,"path")?,"role":"retired_source","sha256":string(r,"sha256")?}))).collect::<io::Result<Vec<_>>>()?;
        expected_inputs.push(json!({"ref":review,"role":"source_owner_review","sha256":digest}));
        if field(&event, "inputs")? != &json!(expected_inputs) {
            return Err(invalid(
                "retirement provenance inputs do not bind the exact sources and review",
            ));
        }
        if field(&event, "outputs")? != &json!([{"ref":event_ref,"role":"corpus_retirement_event"}])
        {
            return Err(invalid(
                "retirement provenance output must name this retained event",
            ));
        }
        if field(&event, "receipt_refs")? != &json!([review]) {
            return Err(invalid(
                "retirement receipt must return to its exact owner review",
            ));
        }
        if identities
            .insert(id.to_owned(), event_ref.to_owned())
            .is_some()
        {
            return Err(invalid("duplicate source retirement event ID"));
        }
    }
    input.tick()?;
    Ok(identities)
}

/// None routes a wider edit to full source validation. A surviving incoming
/// dependency or a reused accepted ID refuses instead of taking that fallback.
pub fn membership_transition(
    input: &mut CandidateInput<'_>,
    retirements: &[Value],
    base: Option<&Value>,
    event_ids: &BTreeMap<String, String>,
) -> io::Result<Option<Index>> {
    input.tick()?;
    let Some(base) = base.filter(|_| !retirements.is_empty()) else {
        return Ok(None);
    };
    let retired = retirements
        .iter()
        .map(|r| Ok(string(r, "path")?.to_owned()))
        .collect::<io::Result<BTreeSet<_>>>()?;
    if retired
        .iter()
        .any(|p| !p.starts_with("ToS/source-witnesses/"))
    {
        return Ok(None);
    }
    let events = event_ids.values().cloned().collect::<BTreeSet<_>>();
    let mut reviews = BTreeMap::new();
    for path in &events {
        let raw = input.bytes(path, MAX_EVENT_BYTES)?;
        let event = strict_object(&raw, event_limits(input))?;
        reviews.insert(
            path.clone(),
            string(
                field(field(&event, "method")?, "configuration")?,
                "review_ref",
            )?
            .to_owned(),
        );
    }
    let mut previous = BTreeMap::new();
    for entry in array(field(base, "files")?)? {
        input.tick()?;
        if previous.insert(string(entry, "path")?, entry).is_some() {
            return Err(invalid("duplicate accepted base member"));
        }
    }
    if events.iter().any(|p| previous.contains_key(p.as_str())) {
        return Ok(None);
    }
    let added = input
        .members
        .keys()
        .filter(|p| !previous.contains_key(p.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut expected = events.clone();
    expected.extend(
        reviews
            .values()
            .filter(|p| !previous.contains_key(p.as_str()))
            .cloned(),
    );
    if added != expected
        || previous
            .keys()
            .filter(|p| !input.members.contains_key(**p))
            .map(|p| (*p).to_owned())
            .collect::<BTreeSet<_>>()
            != retired
    {
        return Ok(None);
    }
    for (path, entry) in &previous {
        input.tick()?;
        if !retired.contains(*path) && input.members.get(*path) != Some(*entry) {
            return Ok(None);
        }
    }
    let mut index = Index::default();
    let old_deps = field(base, "dependencies")?
        .as_object()
        .ok_or_else(|| invalid("accepted dependencies must be an object"))?;
    for (source, targets) in old_deps {
        input.tick()?;
        if retired.contains(source) {
            continue;
        }
        let targets = array(targets)?
            .iter()
            .map(|v| Ok(text(v)?.to_owned()))
            .collect::<io::Result<Vec<_>>>()?;
        if targets.iter().any(|p| retired.contains(p)) {
            return Err(invalid(format!(
                "retirement leaves an incoming source dependency unresolved: {source}"
            )));
        }
        index.dependencies.insert(source.clone(), targets);
    }
    let old_ids = field(base, "identities")?
        .as_object()
        .ok_or_else(|| invalid("accepted identities must be an object"))?;
    for (id, path) in old_ids {
        input.tick()?;
        let path = text(path)?;
        if !retired.contains(path) {
            index.identities.insert(id.clone(), path.to_owned());
        }
    }
    for (id, path) in event_ids {
        input.tick()?;
        if old_ids.contains_key(id) {
            return Err(invalid(
                "retirement event reuses an accepted source identity",
            ));
        }
        index.identities.insert(id.clone(), path.clone());
    }
    for (event, review) in reviews {
        index.dependencies.insert(event, vec![review]);
    }
    input.tick()?;
    Ok(Some(index))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn members(raw: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Value> {
        raw.iter().map(|(p,b)|(p.clone(),json!({"path":p,"sha256":Digest256::of_bytes(b).to_hex(),"size_bytes":b.len(),"mode":420}))).collect()
    }
    fn with_input<T>(
        raw: &BTreeMap<String, Vec<u8>>,
        f: impl FnOnce(&mut CandidateInput<'_>) -> io::Result<T>,
    ) -> io::Result<T> {
        let members = members(raw);
        let mut read = |p: &str, cap: usize| {
            let b = raw.get(p).ok_or_else(|| invalid("missing fixture"))?;
            if b.len() > cap {
                return Err(invalid("fixture cap"));
            }
            Ok(b.clone())
        };
        let mut verify = |p: &str| {
            if raw.contains_key(p) {
                Ok(())
            } else {
                Err(invalid("missing fixture review"))
            }
        };
        let mut check = || Ok(());
        f(&mut CandidateInput {
            members: &members,
            read: &mut read,
            verify_member: &mut verify,
            check: &mut check,
            json: JsonLimits::default(),
            json_state_bytes: 1024 * 1024,
        })
    }
    #[test]
    fn structured_values_native_anchor_and_duplicate_conflict() {
        let a = "ToS/source-witnesses/a.json";
        let b = "ToS/source-witnesses/b.json";
        let c = "ToS/source-witnesses/c.json";
        let raw=BTreeMap::from([
            (a.to_owned(),r#"{"tos.b":"unused","nested":["tos.b","ToS/source-witnesses/c.json:١٢#anchor","ToS/source-witnesses/b.json#\ud800"],"x":"tos.b","x":"unused"}"#.as_bytes().to_vec()),
            (b.to_owned(),b"{}".to_vec()),(c.to_owned(),b"{}".to_vec()),
            ("ToS/source-witnesses/note.md".to_owned(),b"tos.b".to_vec()),
            ("ToS/contracts/test.json".to_owned(),br#"{"value":"tos.b"}"#.to_vec()),
        ]);
        let fresh = FreshRows {
            records: vec![json!({"record_id":"tos.b","source_record_ref":b})],
            claims: vec![],
            native_semantic: BTreeMap::from([(
                "tos.native".to_owned(),
                vec![a.to_owned(), c.to_owned()],
            )]),
        };
        let base = Index {
            identities: BTreeMap::from([("tos.native".to_owned(), c.to_owned())]),
            dependencies: BTreeMap::new(),
        };
        let limits = IndexLimits {
            max_edges: 32,
            max_state_bytes: 16384,
        };
        let result = with_input(&raw, |input| {
            build_index(input, &fresh, Some(&base), limits, &mut |_, _, _| {
                panic!("no retirement schema read")
            })
        })
        .unwrap();
        assert_eq!(result.identities["tos.native"], c);
        assert_eq!(result.dependencies[a], vec![b.to_owned(), c.to_owned()]);
        assert_eq!(result.dependencies[c], vec![a.to_owned()]);
        assert!(
            !result
                .dependencies
                .contains_key("ToS/source-witnesses/note.md")
        );
        assert!(!result.dependencies.contains_key("ToS/contracts/test.json"));
        // A resource refusal must abort admission, never masquerade as an
        // optional malformed document and silently omit its dependencies.
        let exhausted = with_input(&raw, |input| {
            input.json_state_bytes = 1;
            build_index(input, &fresh, Some(&base), limits, &mut |_, _, _| {
                panic!("no retirement schema read")
            })
        })
        .unwrap_err();
        assert!(exhausted.to_string().contains("JSON parser state budget"));
        let conflict = FreshRows {
            records: vec![
                json!({"record_id":"tos.b","source_record_ref":a}),
                json!({"record_id":"tos.b","source_record_ref":b}),
            ],
            claims: vec![],
            native_semantic: BTreeMap::new(),
        };
        assert!(
            with_input(&raw, |input| build_index(
                input,
                &conflict,
                None,
                limits,
                &mut |_, _, _| Ok(None)
            ))
            .unwrap_err()
            .to_string()
            .contains("duplicate source identity")
        );
        assert!(
            with_input(&raw, |input| build_index(
                input,
                &fresh,
                None,
                IndexLimits {
                    max_edges: 1,
                    ..limits
                },
                &mut |_, _, _| Ok(None)
            ))
            .is_err()
        );
    }
    #[test]
    fn python_json_and_regex_boundaries() {
        assert_eq!(reference_path("ToS/a:12#x"), "ToS/a");
        assert_eq!(reference_path("ToS/a:١٢"), "ToS/a");
        assert_eq!(reference_path("ToS/a:²"), "ToS/a:²");
        assert_eq!(reference_path("ToS/a:12\n"), "ToS/a\n");
        assert_eq!(reference_path("ToS/a:12\r"), "ToS/a:12\r");
        let source = r#"{"x":"wrong","x":"tos.correct","other":NaN}"#;
        for encoding in [
            source.as_bytes().to_vec(),
            source.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            source
                .chars()
                .flat_map(|c| (c as u32).to_be_bytes())
                .collect(),
        ] {
            let row = document(
                &encoding,
                JsonMode::LegacyPythonObserved,
                JsonLimits::default(),
                1024 * 1024,
            )
            .unwrap()
            .unwrap();
            assert_eq!(row.object_get("x").unwrap().as_str(), Some("tos.correct"));
        }
        assert!(
            strict_object(br#"{"x":1,"x":2}"#, JsonLimits::default())
                .unwrap_err()
                .to_string()
                .contains("duplicate fields")
        );
        assert!(strict_object(br#"{"x":1e999}"#, JsonLimits::default()).is_err());
        assert!(
            document(
                b"not json",
                JsonMode::LegacyPythonObserved,
                JsonLimits::default(),
                1024 * 1024,
            )
            .unwrap()
            .is_none()
        );
        let row = document(
            b"{\"x\":\"\xed\xa0\x80\",\"y\":\"tos.correct\"}",
            JsonMode::LegacyPythonObserved,
            JsonLimits::default(),
            1024 * 1024,
        )
        .unwrap()
        .unwrap();
        assert_eq!(row.object_get("y").unwrap().as_str(), Some("tos.correct"));
    }
    #[test]
    fn retirement_exact_bindings_and_narrow_transfer() {
        let old = "ToS/source-witnesses/old.md";
        let event = "ToS/source-witnesses/retirements/old.json";
        let review = "ToS/review-ledger/old.md";
        let old_sha = Digest256::of_bytes(b"old").to_hex();
        let review_sha = Digest256::of_bytes(b"review").to_hex();
        let config = json!({"base_revision":"base","retirements":[{"path":old,"sha256":old_sha}],"reason":"retained source owner review","review_ref":review,"review_sha256":review_sha});
        let event_value = json!({"schema_version":"tos_provenance_event_v1","event_type":"migration","event_id":"tos.event.retirement","status":"completed","started_at":"2026-09-30T00:00:00Z","ended_at":"2026-09-30T00:00:01Z","method":{"name":"corpus-source-retirement","version":"1","configuration":config},"inputs":[{"ref":old,"role":"retired_source","sha256":old_sha},{"ref":review,"role":"source_owner_review","sha256":review_sha}],"outputs":[{"ref":event,"role":"corpus_retirement_event"}],"receipt_refs":[review]});
        let raw = BTreeMap::from([
            (RETIREMENT_SCHEMA.to_owned(), b"{}".to_vec()),
            (review.to_owned(), b"review".to_vec()),
            (event.to_owned(), serde_json::to_vec(&event_value).unwrap()),
        ]);
        let surviving = members(&raw)[RETIREMENT_SCHEMA].clone();
        let mut base = json!({"revision":"base","files":[surviving,{"path":old,"sha256":old_sha,"size_bytes":3,"mode":420}],"identities":{"tos.old":old},"dependencies":{}});
        let retired = vec![json!({"path":old,"sha256":old_sha,"event_ref":event})];
        let ids = with_input(&raw, |input| {
            validate_retirements(input, &retired, Some(&base), &mut |_, _, schema| {
                assert_eq!(schema, b"{}");
                Ok(None)
            })
        })
        .unwrap();
        let index = with_input(&raw, |input| {
            membership_transition(input, &retired, Some(&base), &ids)
        })
        .unwrap()
        .unwrap();
        assert_eq!(
            index.identities,
            BTreeMap::from([("tos.event.retirement".to_owned(), event.to_owned())])
        );
        assert_eq!(index.dependencies[event], vec![review.to_owned()]);
        base["dependencies"] = json!({(RETIREMENT_SCHEMA):[old]});
        assert!(
            with_input(&raw, |input| membership_transition(
                input,
                &retired,
                Some(&base),
                &ids
            ))
            .unwrap_err()
            .to_string()
            .contains("incoming source dependency unresolved")
        );
        base["dependencies"] = json!({});
        base["files"][0]["mode"] = json!(384);
        assert!(
            with_input(&raw, |input| membership_transition(
                input,
                &retired,
                Some(&base),
                &ids
            ))
            .unwrap()
            .is_none()
        );
        assert!(
            with_input(&BTreeMap::new(), |input| validate_retirements(
                input,
                &[],
                None,
                &mut |_, _, _| panic!("empty retirement fastpath")
            ))
            .unwrap()
            .is_empty()
        );
        let mut broken = raw.clone();
        let mut changed = event_value;
        changed["outputs"] = json!([]);
        broken.insert(event.to_owned(), serde_json::to_vec(&changed).unwrap());
        assert!(
            with_input(&broken, |input| validate_retirements(
                input,
                &retired,
                Some(&base),
                &mut |_, _, _| Ok(None)
            ))
            .unwrap_err()
            .to_string()
            .contains("output must name")
        );
    }
}
