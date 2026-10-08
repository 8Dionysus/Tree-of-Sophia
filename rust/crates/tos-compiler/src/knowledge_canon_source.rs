//! Authored canon/candidate source-cut front edge for the existing raw adapters.
//! It reads only manifest-selected current members, never an ambient checkout.
//! Cold planning freezes raw output witnesses in a genuine source-file custody
//! stage before final-stage creation. Rendering and compatibility preparation
//! compare independent target receipts; neither operation grants canon admission.
use crate::knowledge_canon_prepare::{CANDIDATE_PROFILE, CANON_PROFILE, framed, required, text};
use crate::knowledge_stage::{InputCollectionReceipt, InputRow, KnowledgeStage, WritePhase};
use crate::{Error, QueryVocabulary, Result, SourceBinding};
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonNumberKind, JsonValue,
    RelativePath, SourceRevision, canonical_bytes_v1, parse_json,
};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};
use tos_validation::source_cut::CutSchemaExecutor;

const NODE_SCHEMA: &str = "ToS/contracts/tos-node-contract.schema.json";
const FORM_SET_SCHEMA: &str = "ToS/contracts/human-form-set.schema.json";
/// Exact raw-file custody recipe for this source family, independent from the
/// catalog producer's source-witness namespace and the derived native inputs.
pub const CANON_SOURCE_CUSTODY: &str = "canon-source-custody";
pub const CANON_SOURCE_FILES_ROLE: &str = "authored-canon-candidate-files";
pub const CANON_SOURCE_FILES_PROFILE: &str = "tos.canon-source.source-files.v1";
pub const CANON_SOURCE_CONTRACTS_ROLE: &str = "canon-source-contracts";
pub const CANON_SOURCE_CONTRACTS_PROFILE: &str = "tos.canon-source.contracts.v1";
#[derive(Clone, Copy, Debug)]
pub struct CanonSourceLimits {
    pub max_manifest_members: u64,
    pub max_selected_members: u64,
    pub max_nodes: u64,
    pub max_packs: u64,
    pub max_edges: u64,
    pub max_source_bytes: usize,
    pub max_raw_row_bytes: usize,
    pub max_csv_fields: usize,
    pub max_csv_record_bytes: usize,
    pub max_forms: usize,
    pub max_forms_output_bytes: usize,
    pub max_page_rows: usize,
    pub max_page_bytes: usize,
    pub max_work_bytes: u64,
}
impl CanonSourceLimits {
    fn validate(self) -> Result<()> {
        if self.max_manifest_members == 0
            || self.max_selected_members == 0
            || self.max_nodes == 0
            || self.max_packs == 0
            || self.max_edges == 0
            || self.max_source_bytes == 0
            || self.max_source_bytes > 8 * 1024 * 1024
            || self.max_raw_row_bytes == 0
            || self.max_raw_row_bytes > 8 * 1024 * 1024
            || self.max_csv_fields == 0
            || self.max_csv_fields > 1024
            || self.max_csv_record_bytes == 0
            || self.max_csv_record_bytes > self.max_source_bytes
            || self.max_forms == 0
            || self.max_forms > 256
            || self.max_forms_output_bytes == 0
            || self.max_forms_output_bytes > 262144
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_page_bytes == 0
            || self.max_page_bytes > 64 * 1024 * 1024
            || self
                .max_page_rows
                .checked_mul(self.max_raw_row_bytes)
                .is_none_or(|n| n > self.max_page_bytes)
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("canon source limits"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct CanonSourceCollection {
    pub source_graph: String,
    pub input_role: String,
    pub adapter_profile: String,
    pub collection: String,
    pub count: u64,
    pub root_sha256: String,
}
#[derive(Clone, Debug)]
pub struct CanonSourceReceipt {
    pub source_revision: String,
    pub job_source_cut: String,
    /// Exact current manifest metadata EOF, independent from projected rows.
    /// Unrelated current member contents were not read by this source family.
    pub manifest_members: u64,
    pub manifest_membership_root_sha256: String,
    pub selected_members_read: u64,
    pub selected_source_root_sha256: String,
    pub nodes: u64,
    pub packs: u64,
    pub edges: u64,
    pub collections: Vec<CanonSourceCollection>,
    pub work_bytes: u64,
    pub current_members_only: bool,
    pub final_graph_rows_written: bool,
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Invalid("canon source cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("canon source deadline"));
    }
    Ok(())
}
struct Work {
    bytes: u64,
    selected: u64,
    source_root: Digest256Hasher,
}
impl Work {
    fn charge(&mut self, n: usize, l: CanonSourceLimits) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(n as u64)
            .ok_or(Error::Budget("canon source work"))?;
        if self.bytes > l.max_work_bytes {
            return Err(Error::Budget("canon source work"));
        }
        Ok(())
    }
    fn member(&mut self, path: &str, raw: &[u8], l: CanonSourceLimits) -> Result<()> {
        self.charge(raw.len(), l)?;
        self.selected = self
            .selected
            .checked_add(1)
            .ok_or(Error::Budget("canon selected members"))?;
        if self.selected > l.max_selected_members {
            return Err(Error::Budget("canon selected members"));
        }
        framed(&mut self.source_root, path);
        self.source_root.update(&(raw.len() as u64).to_be_bytes());
        self.source_root.update(Digest256::of_bytes(raw).as_bytes());
        Ok(())
    }
}
fn json_limits(max: usize) -> Result<JsonLimits> {
    JsonLimits::new(max, 96, 1_000_000, 4096).map_err(|_| Error::Budget("canon source JSON"))
}
fn parse(raw: &[u8], max: usize) -> Result<JsonValue> {
    if raw.len() > max {
        return Err(Error::Budget("canon source JSON bytes"));
    }
    Ok(
        parse_json(raw, JsonMode::PublishedStrict, json_limits(max)?)
            .map_err(|e| Error::Source(e.to_string()))?
            .into_root(),
    )
}
fn encode(v: &JsonValue, max: usize) -> Result<Vec<u8>> {
    canonical_bytes_v1(v, CanonicalProfile::SourceRecordDigestV1, json_limits(max)?)
        .map_err(|e| Error::Source(e.to_string()))
}
fn decoded(v: &JsonValue, max: usize) -> Result<Value> {
    serde_json::from_slice(&encode(v, max)?).map_err(|_| {
        Error::Source("canon source representation cannot be decoded as serde JSON".into())
    })
}
fn encode_value(v: &Value, max: usize) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(v).map_err(|_| Error::Invalid("canon projected JSON"))?;
    if raw.len() > max {
        return Err(Error::Budget("canon projected JSON bytes"));
    }
    encode(&parse(&raw, max)?, max)
}
fn schema(
    executor: &mut impl CutSchemaExecutor,
    path: &str,
    raw: &[u8],
    contract: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    if !executor
        .check(path, raw, contract, deadline, cancelled)
        .map_err(|e| Error::Source(format!("canon selected schema: {e:?}")))?
    {
        return Err(Error::Invalid("canon selected source schema"));
    }
    Ok(())
}
// Exact Decimal comparison used by parse_node_json: 1e-5 and 0.00001 share
// one coefficient/exponent, while a rounded decimal does not pass silently.
fn decimal_identity(raw: &str) -> Result<(bool, String, i64)> {
    let negative = raw.starts_with('-');
    let raw = raw.strip_prefix('-').unwrap_or(raw);
    let (base, exp) = raw.split_once(['e', 'E']).unwrap_or((raw, "0"));
    let exponent: i64 = exp
        .parse()
        .map_err(|_| Error::Invalid("canon decimal exponent"))?;
    let fractional = base.split_once('.').map_or(0, |(_, f)| f.len());
    let digits = base.chars().filter(|c| *c != '.').collect::<String>();
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Ok((false, "0".into(), 0));
    }
    let trimmed = digits.trim_end_matches('0');
    let zeros = digits.len() - trimmed.len();
    let exponent = exponent
        .checked_sub(fractional as i64)
        .and_then(|n| n.checked_add(zeros as i64))
        .ok_or(Error::Invalid("canon decimal exponent"))?;
    Ok((negative, trimmed.into(), exponent))
}
fn exact_node_numbers(v: &JsonValue, max: usize) -> Result<()> {
    match v {
        JsonValue::Number(n) if n.kind == JsonNumberKind::Float => {
            let canonical = encode(v, max)?;
            let converted = std::str::from_utf8(&canonical)
                .map_err(|_| Error::Invalid("canon numeric UTF-8"))?;
            if decimal_identity(&n.lexeme)? != decimal_identity(converted)? {
                return Err(Error::Invalid("canon node decimal loses precision"));
            }
        }
        JsonValue::Array(a) => {
            for v in a {
                exact_node_numbers(v, max)?;
            }
        }
        JsonValue::Object(a) => {
            for (_, v) in a {
                exact_node_numbers(v, max)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn node_field_consistency(v: &Value) -> Result<()> {
    let id = required(v, "node_id")?;
    let kind = required(v, "node_type")?;
    if !id.starts_with(&format!("tos.{kind}.")) {
        return Err(Error::Invalid("canon node family/type"));
    }
    if v.get("relations").is_some()
        && v.get("lineage_relations").is_some()
        && v["relations"] != v["lineage_relations"]
    {
        return Err(Error::Invalid("canon relation alias disagreement"));
    }
    let mut languages = BTreeSet::new();
    let mut spine: Option<Vec<String>> = None;
    for witness in v
        .get("language_witnesses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !languages.insert(required(witness, "language")?) {
            return Err(Error::Invalid("canon duplicate language witness"));
        }
        let segments = witness
            .get("segments")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("canon witness segments"))?
            .iter()
            .map(|s| required(s, "segment_id").map(str::to_owned))
            .collect::<Result<Vec<_>>>()?;
        if segments.iter().collect::<BTreeSet<_>>().len() != segments.len() {
            return Err(Error::Invalid("canon repeated witness segment"));
        }
        if spine.as_ref().is_some_and(|s| s != &segments) {
            return Err(Error::Invalid("canon shared segment spine"));
        }
        spine = Some(segments);
    }
    for tension in v
        .get("translation_tensions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !spine.as_ref().is_some_and(|s| {
            s.iter()
                .any(|id| Some(id.as_str()) == text(tension.get("segment_id")))
        }) {
            return Err(Error::Invalid("canon translation tension segment"));
        }
    }
    Ok(())
}
/// The authored node validator and compiler share exact numeric and cross-field
/// rules. Schema evaluation remains with the caller's selected source schema;
/// this check does not admit the object to canon or bind a filesystem path.
pub fn validate_authored_node_mechanics(raw: &[u8], max_bytes: usize) -> Result<String> {
    if max_bytes == 0 || max_bytes > 8 * 1024 * 1024 {
        return Err(Error::Budget("canon node validator bytes"));
    }
    let source = parse(raw, max_bytes)?;
    exact_node_numbers(&source, max_bytes)?;
    let value = decoded(&source, max_bytes)?;
    node_field_consistency(&value)?;
    Ok(required(&value, "node_id")?.to_owned())
}
fn node_consistency(v: &Value, path: &str) -> Result<()> {
    node_field_consistency(v)?;
    let id = required(v, "node_id")?;
    let kind = required(v, "node_type")?;
    if v.get("schema_version").and_then(Value::as_str) == Some("tos_canonical_node_v1") {
        let parts = path.split('/').collect::<Vec<_>>();
        let slug = id.rsplit('.').next().unwrap_or(id);
        if parts.len() < 4
            || parts[0] != "ToS"
            || parts[1] != "canon"
            || parts[2] != kind
            || parts.last() != Some(&"node.json")
            || (parts[parts.len() - 2] != slug
                && !(kind == "source" && parts[parts.len() - 2].starts_with(&format!("{slug}-"))))
        {
            return Err(Error::Invalid("canon native source path binding"));
        }
    }
    Ok(())
}
fn canonical_label(v: &Value) -> String {
    for key in [
        "canonical_label",
        "preferred_label",
        "label",
        "title",
        "name",
    ] {
        if let Some(s) = text(v.get(key)) {
            return s.into();
        }
    }
    text(v.get("node_id"))
        .map(|id| id.rsplit('.').next().unwrap_or(id).replace(['-', '_'], " "))
        .unwrap_or_else(|| "unnamed-node".into())
}
fn insert(
    stage: &mut KnowledgeStage<'_>,
    graph: &str,
    collection: &str,
    id: &str,
    raw: &[u8],
    work: &mut Work,
    l: CanonSourceLimits,
) -> Result<()> {
    if id.is_empty() || id.len() > 4096 || id.contains('\0') || raw.len() > l.max_raw_row_bytes {
        return Err(Error::Budget("canon source raw row"));
    }
    work.charge(raw.len(), l)?;
    stage.charge_materialized(
        1,
        (raw.len() + id.len() + graph.len() + collection.len() + 40) as u64,
    )?;
    stage.with_connection(WritePhase::Normalized, |db| {
        db.execute(
            "INSERT INTO knowledge_canon_source_rows VALUES(?1,?2,?3,?4,?5)",
            params![
                graph,
                collection,
                id,
                raw,
                Digest256::of_bytes(raw).as_bytes().as_slice()
            ],
        )?;
        Ok(())
    })
}
/// Python csv.DictReader(strict=True), with logical records, blank-record
/// skipping and exact null missing cells. Quoted CR/LF and doubled quotes are
/// data; duplicate/empty headers and surplus unnamed cells refuse.
pub(crate) fn csv_records<F>(
    raw: &[u8],
    l: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut emit: F,
) -> Result<()>
where
    F: FnMut(Vec<String>) -> Result<()>,
{
    csv_records_with_spans(
        raw,
        CsvReadLimits {
            max_fields: l.max_csv_fields,
            max_record_bytes: l.max_csv_record_bytes,
        },
        deadline,
        cancelled,
        |row, _, _| {
            emit(row)?;
            Ok(true)
        },
    )
}

/// Same authored CSV grammar, with its parser workspace held before Vec/String
/// growth. Any row retained beyond emit needs separate caller ownership.
pub(crate) fn csv_records_with_owned_state<F>(
    raw: &[u8],
    l: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    state: &crate::d1_public_capture::CreationState<'_>,
    mut emit: F,
) -> Result<()>
where
    F: FnMut(Vec<String>) -> Result<()>,
{
    csv_records_with_spans_owned(
        raw,
        CsvReadLimits {
            max_fields: l.max_csv_fields,
            max_record_bytes: l.max_csv_record_bytes,
        },
        deadline,
        cancelled,
        Some(state),
        |row, _, _| {
            emit(row)?;
            Ok(true)
        },
    )
}

/// Exact retained CSV row, parsed by the authored corpus parser. Byte offsets
/// preserve CR/LF and quoted records; no catalog membership or rights are granted.
pub fn read_exact_authored_csv_row(
    raw: &[u8],
    ordinal: u64,
    expected: &Value,
    max_record_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Value> {
    if ordinal == 0
        || ordinal > 9_007_199_254_740_991
        || raw.len() > 8 * 1024 * 1024
        || max_record_bytes == 0
        || max_record_bytes > 1024 * 1024
    {
        return Err(Error::Invalid("exact CSV row binding/budget"));
    }
    let expected = expected
        .as_object()
        .ok_or(Error::Invalid("exact CSV row object"))?;
    if expected.values().any(|v| !v.is_null() && !v.is_string()) {
        return Err(Error::Invalid("exact CSV cell"));
    }
    let limits = CsvReadLimits {
        max_fields: 1024,
        max_record_bytes: 8 * 1024 * 1024,
    };
    let mut columns: Option<Vec<String>> = None;
    let mut count = 0u64;
    let mut selected = None;
    csv_records_with_spans(raw, limits, deadline, cancelled, |cells, start, end| {
        if columns.is_none() {
            if cells.is_empty()
                || cells.iter().any(String::is_empty)
                || cells.iter().collect::<BTreeSet<_>>().len() != cells.len()
            {
                return Err(Error::Invalid("exact CSV unique header"));
            }
            columns = Some(cells);
            return Ok(true);
        }
        if cells.is_empty() {
            return Ok(true);
        }
        count += 1;
        let names = columns.as_ref().unwrap();
        if cells.len() > names.len() {
            return Err(Error::Invalid("exact CSV unnamed cells"));
        }
        if count != ordinal {
            return Ok(true);
        }
        if end - start > max_record_bytes {
            return Err(Error::Budget("exact CSV record bytes"));
        }
        let record: serde_json::Map<String, Value> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), cells.get(i).map_or(Value::Null, |v| json!(v))))
            .collect();
        if &record != expected {
            return Err(Error::Invalid("exact CSV retained record differs"));
        }
        let raw_record =
            std::str::from_utf8(&raw[start..end]).map_err(|_| Error::Invalid("exact CSV UTF-8"))?;
        selected = Some(
            json!({"record":record,"columns":names,"source_row":ordinal,"byte_offset":start,
            "row_bytes":end-start,"raw_record":raw_record,"raw_record_sha256":Digest256::of_bytes(&raw[start..end]).to_hex(),
            "source_file_sha256":Digest256::of_bytes(raw).to_hex()}),
        );
        Ok(false)
    })?;
    selected.ok_or(Error::Invalid("exact CSV row absent"))
}

struct CsvReadLimits {
    max_fields: usize,
    max_record_bytes: usize,
}

fn csv_records_with_spans<F>(
    raw: &[u8],
    l: CsvReadLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut emit: F,
) -> Result<()>
where
    F: FnMut(Vec<String>, usize, usize) -> Result<bool>,
{
    csv_records_with_spans_owned(raw, l, deadline, cancelled, None, emit)
}
fn csv_records_with_spans_owned<F>(
    raw: &[u8],
    l: CsvReadLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    state: Option<&crate::d1_public_capture::CreationState<'_>>,
    mut emit: F,
) -> Result<()>
where
    F: FnMut(Vec<String>, usize, usize) -> Result<bool>,
{
    let _workspace = if let Some(state) = state {
        // Pinned Vec growth overlaps old/new buffers. Cells are moved into
        // Strings without a new allocation; all cell payloads sum to at most
        // the original record-byte cap, with minimum capacity eight per cell.
        let cells = l.max_fields.max(4);
        let upper = l
            .max_record_bytes
            .checked_mul(4)
            .and_then(|n| n.checked_add(cells.checked_mul(8)?))
            .and_then(|n| n.checked_add(cells.checked_mul(4 * std::mem::size_of::<String>())?))
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<Vec<String>>())
            })
            .ok_or(Error::Budget("owned canon CSV workspace"))?;
        state.charge_work(
            raw.len()
                .checked_mul(3)
                .ok_or(Error::Budget("owned canon CSV work"))?,
        )?;
        Some(state.hold(upper)?)
    } else {
        None
    };
    let text = std::str::from_utf8(raw).map_err(|_| Error::Invalid("canon relation CSV UTF-8"))?;
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut record_start = 0;
    let mut field = Vec::new();
    let mut record = Vec::new();
    let mut started = false;
    let mut quoted = false;
    let mut closed = false;
    let mut record_bytes = 0usize;
    while i < bytes.len() {
        check(deadline, cancelled)?;
        if let Some(state) = state {
            state.active()?;
        }
        let b = bytes[i];
        i += 1;
        record_bytes = record_bytes
            .checked_add(1)
            .ok_or(Error::Budget("canon CSV record"))?;
        if record_bytes > l.max_record_bytes {
            return Err(Error::Budget("canon CSV record"));
        }
        if quoted {
            if b == b'"' {
                if i < bytes.len() && bytes[i] == b'"' {
                    field.push(b'"');
                    i += 1;
                    record_bytes += 1;
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                field.push(b);
            }
            continue;
        }
        if closed && !matches!(b, b',' | b'\r' | b'\n') {
            return Err(Error::Invalid("canon strict CSV closing quote"));
        }
        match b {
            b'"' if field.is_empty() => {
                quoted = true;
                started = true;
            }
            b',' => {
                record.push(
                    String::from_utf8(std::mem::take(&mut field))
                        .map_err(|_| Error::Invalid("canon CSV cell UTF-8"))?,
                );
                if record.len() >= l.max_fields {
                    return Err(Error::Budget("canon CSV fields"));
                }
                started = true;
                closed = false;
            }
            b'\r' | b'\n' => {
                if b == b'\r' && i < bytes.len() && bytes[i] == b'\n' {
                    i += 1;
                }
                if started || !field.is_empty() || !record.is_empty() {
                    record.push(
                        String::from_utf8(std::mem::take(&mut field))
                            .map_err(|_| Error::Invalid("canon CSV cell UTF-8"))?,
                    );
                    if !emit(std::mem::take(&mut record), record_start, i)? {
                        return Ok(());
                    }
                } else {
                    if !emit(Vec::new(), record_start, i)? {
                        return Ok(());
                    }
                }
                started = false;
                closed = false;
                record_bytes = 0;
                record_start = i;
            }
            _ => {
                field.push(b);
                started = true;
            }
        }
    }
    if quoted {
        return Err(Error::Invalid("canon truncated quoted CSV field"));
    }
    if started || !field.is_empty() || !record.is_empty() {
        record.push(String::from_utf8(field).map_err(|_| Error::Invalid("canon CSV cell UTF-8"))?);
        emit(record, record_start, i)?;
    }
    Ok(())
}
fn forms<F>(
    cut: &CorpusCutReader,
    source: &JsonValue,
    source_value: &Value,
    path: &str,
    executor: &mut impl CutSchemaExecutor,
    work: &mut Work,
    l: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    materialize: &mut F,
) -> Result<Option<(String, String, JsonValue)>>
where
    F: FnMut(&Value, &Value, usize) -> Result<Vec<Value>>,
{
    let form_path = format!(
        "{}/node.human-forms.json",
        path.rsplit_once('/')
            .ok_or(Error::Invalid("canon source path"))?
            .0
    );
    let relative = RelativePath::parse(&form_path).map_err(|e| Error::Source(e.to_string()))?;
    if cut.current().member(&relative).is_none() {
        return Ok(None);
    }
    if source_value.get("schema_version").and_then(Value::as_str) != Some("tos_canonical_node_v1") {
        return Err(Error::Invalid("canon forms require explicit native schema"));
    }
    let selected = cut
        .read_member(
            cut.current().revision(),
            &relative,
            l.max_source_bytes as u64,
            deadline,
            cancelled,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
    work.member(&form_path, &selected.raw, l)?;
    schema(
        executor,
        &form_path,
        &selected.raw,
        FORM_SET_SCHEMA,
        deadline,
        cancelled,
    )?;
    let set = parse(&selected.raw, l.max_source_bytes)?;
    let set_value = decoded(&set, l.max_source_bytes)?;
    let declared = set
        .object_get("forms")
        .and_then(JsonValue::as_array)
        .ok_or(Error::Invalid("canon current forms"))?;
    let prior = set
        .object_get("prior_forms")
        .and_then(JsonValue::as_array)
        .ok_or(Error::Invalid("canon prior forms"))?;
    if declared.len() > l.max_forms || prior.len() > 256 {
        return Err(Error::Budget("canon source forms count"));
    }
    if declared.iter().chain(prior).any(|v| {
        v.object_get("content")
            .and_then(|v| v.object_get("kind"))
            .and_then(JsonValue::as_str)
            != Some("source-copy")
    }) {
        return Err(Error::Invalid("canon noncopy HumanForm"));
    }
    if required(&set_value["subject"], "id")? != required(source_value, "node_id")? {
        return Err(Error::Invalid("canon HumanForm subject identity"));
    }
    let output = materialize(source_value, &set_value, l.max_forms_output_bytes)?;
    check(deadline, cancelled)?;
    if output.len() != declared.len() {
        return Err(Error::Invalid("canon form materialization coverage"));
    }
    let subject = json!({"id":required(source_value,"node_id")?,"version":source_value["record_version"],"digest":Digest256::of_bytes(&encode(source,l.max_source_bytes)?).to_prefixed()});
    for (input, packet) in declared.iter().zip(&output) {
        let form = decoded(input, l.max_source_bytes)?;
        let expected = json!({"id":required(&form,"form_id")?,"version":form["form_version"],"digest":Digest256::of_bytes(&encode(input,l.max_source_bytes)?).to_prefixed()});
        if packet["schema_version"] != "tos_human_form_materialization_v1"
            || packet["form"] != expected
            || packet["subject"] != subject
            || packet["performs_semantic_assessment"] != false
            || !packet.get("admission").is_some_and(Value::is_null)
        {
            return Err(Error::Invalid("canon source form output binding"));
        }
    }
    let packets = parse(
        &encode_value(&json!(output), l.max_forms_output_bytes)?,
        l.max_forms_output_bytes,
    )?;
    let bytes = encode(&packets, l.max_forms_output_bytes)?;
    work.charge(bytes.len(), l)?;
    Ok(Some((
        form_path,
        Digest256::of_bytes(&selected.raw).to_hex(),
        packets,
    )))
}
fn node_row<F>(
    cut: &CorpusCutReader,
    path: &str,
    raw: &[u8],
    executor: &mut impl CutSchemaExecutor,
    work: &mut Work,
    l: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    materialize: &mut F,
) -> Result<(String, Vec<u8>)>
where
    F: FnMut(&Value, &Value, usize) -> Result<Vec<Value>>,
{
    let source = parse(raw, l.max_source_bytes)?;
    exact_node_numbers(&source, l.max_source_bytes)?;
    schema(executor, path, raw, NODE_SCHEMA, deadline, cancelled)?;
    let value = decoded(&source, l.max_source_bytes)?;
    node_consistency(&value, path)?;
    let id = required(&value, "node_id")?.to_owned();
    let kind = required(&value, "node_type")?;
    let native =
        value.get("schema_version").and_then(Value::as_str) == Some("tos_canonical_node_v1");
    let route = path
        .split_once("/friedrich-nietzsche/")
        .map(|(_, tail)| tail.strip_suffix("/node.json").unwrap_or(tail));
    let mut row = json!({"node_id":id,"node_type":kind,"label":canonical_label(&value),"owner_branch":"ToS/canon","authority_layer":"canon","source_path":path,"source_sha256":Digest256::of_bytes(raw).to_hex(),"route_hint":route,"properties":null});
    if native {
        row["source_record_sha256"] =
            json!(Digest256::of_bytes(&encode(&source, l.max_source_bytes)?).to_hex());
    }
    let rendered = forms(
        cut,
        &source,
        &value,
        path,
        executor,
        work,
        l,
        deadline,
        cancelled,
        materialize,
    )?;
    if let Some((ref form_path, ref sha, _)) = rendered {
        row["human_forms_source_ref"] = json!(form_path);
        row["human_forms_source_sha256"] = json!(sha);
        row["human_forms"] = Value::Null;
    }
    let mut projected = parse(
        &serde_json::to_vec(&row).map_err(|_| Error::Invalid("canon raw node outer"))?,
        l.max_raw_row_bytes,
    )?;
    let entries = match &mut projected {
        JsonValue::Object(v) => v,
        _ => return Err(Error::Invalid("canon raw node object")),
    };
    for (key, v) in entries {
        if key.as_str() == Some("properties") {
            *v = source.clone();
        }
        if key.as_str() == Some("human_forms") {
            *v = rendered
                .as_ref()
                .ok_or(Error::Invalid("canon source forms absent"))?
                .2
                .clone();
        }
    }
    Ok((id, encode(&projected, l.max_raw_row_bytes)?))
}
fn derive<F>(
    stage: &mut KnowledgeStage<'_>,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    vocabulary: &QueryVocabulary,
    executor: &mut impl CutSchemaExecutor,
    l: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    materialize: &mut F,
    initial_work_bytes: u64,
) -> Result<CanonSourceReceipt>
where
    F: FnMut(&Value, &Value, usize) -> Result<Vec<Value>>,
{
    l.validate()?;
    check(deadline, cancelled)?;
    let profiles = [CANON_PROFILE, CANDIDATE_PROFILE];
    let mut sources = Vec::new();
    for profile in profiles {
        let found = vocabulary
            .sources
            .iter()
            .filter(|s| s.adapter_profile == profile)
            .collect::<Vec<_>>();
        if found.len() != 1 {
            return Err(Error::Invalid("canon authored source profile selection"));
        }
        sources.push(found[0]);
    }
    let revision = cut.current().revision();
    let manifest = cut
        .stream(revision)
        .map_err(|e| Error::Source(e.to_string()))?
        .expectation();
    if revision != expected_revision || manifest != expected_membership {
        return Err(Error::Invalid("canon independent selected source cut"));
    }
    if manifest.count > l.max_manifest_members {
        return Err(Error::Budget("canon source manifest members"));
    }
    stage.create_preparation_tables(crate::knowledge_stage::preparation_schema!(
            table r#"knowledge_canon_source_rows(source_graph TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,payload BLOB NOT NULL,payload_sha256 BLOB NOT NULL CHECK(length(payload_sha256)=32),PRIMARY KEY(source_graph,collection,id)) WITHOUT ROWID"#
        ))?;
    let mut work = Work {
        bytes: initial_work_bytes,
        selected: 0,
        source_root: Digest256Hasher::new(),
    };
    work.source_root.update(b"tos-canon-source-selected-v1\0");
    work.source_root.update(revision.0.as_bytes());
    let mut manifest_seen = 0u64;
    let mut nodes = 0u64;
    let mut packs = 0u64;
    let mut edges = 0u64;
    let mut global_csv_order = 0u64;
    for metadata in cut.current().members() {
        check(deadline, cancelled)?;
        manifest_seen += 1;
        let path = metadata.path.as_str();
        if path.starts_with("ToS/")
            && path.ends_with("/node.json")
            && !path.starts_with("ToS/canon/")
        {
            return Err(Error::Invalid("canon unsupported authored node owner"));
        }
        let canonical_node = path.starts_with("ToS/canon/") && path.ends_with("/node.json");
        // Python fallback edge IDs count every tracked ToS CSV logical row,
        // in these registered owners. An unfamiliar owner requires another adapter
        // and refuses rather than silently changing this count universe.
        let csv = path.starts_with("ToS/")
            && path.ends_with("/edges.csv")
            && !path.split('/').any(|p| p == "payload");
        if !canonical_node && !csv {
            continue;
        }
        let member = cut
            .read_member(
                revision,
                &metadata.path,
                l.max_source_bytes as u64,
                deadline,
                cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))?;
        work.member(path, &member.raw, l)?;
        if canonical_node {
            nodes = nodes
                .checked_add(1)
                .ok_or(Error::Budget("canon source nodes"))?;
            if nodes > l.max_nodes {
                return Err(Error::Budget("canon source nodes"));
            }
            let (id, row) = node_row(
                cut,
                path,
                &member.raw,
                executor,
                &mut work,
                l,
                deadline,
                cancelled,
                materialize,
            )?;
            insert(
                stage,
                &sources[0].source_graph_id,
                "nodes",
                &id,
                &row,
                &mut work,
                l,
            )?;
            continue;
        }
        let pack = path
            .strip_suffix("/edges.csv")
            .and_then(|p| p.strip_prefix("ToS/"))
            .ok_or(Error::Invalid("canon relation pack route"))?;
        let branch = path
            .split('/')
            .nth(1)
            .ok_or(Error::Invalid("canon relation owner"))?;
        let selected = match branch {
            "canon" => Some(sources[0]),
            "candidate-intake" => Some(sources[1]),
            _ => return Err(Error::Invalid("canon unsupported relation pack owner")),
        };
        let mut columns: Option<Vec<String>> = None;
        let mut ordinal = 0u64;
        let file_sha = Digest256::of_bytes(&member.raw).to_hex();
        csv_records(&member.raw, l, deadline, cancelled, |cells| {
            if columns.is_none() {
                if cells.is_empty()
                    || cells.iter().any(String::is_empty)
                    || cells.iter().collect::<BTreeSet<_>>().len() != cells.len()
                {
                    return Err(Error::Invalid("canon CSV empty/duplicate headers"));
                }
                columns = Some(cells);
                return Ok(());
            }
            if cells.is_empty() {
                return Ok(());
            }
            let header = columns.as_ref().expect("selected header");
            if cells.len() > header.len() {
                return Err(Error::Invalid("canon CSV unnamed surplus cells"));
            }
            ordinal = ordinal
                .checked_add(1)
                .ok_or(Error::Budget("canon CSV logical ordinal"))?;
            global_csv_order = global_csv_order
                .checked_add(1)
                .ok_or(Error::Budget("canon global CSV ordinal"))?;
            if global_csv_order > l.max_edges {
                return Err(Error::Budget("canon source total CSV rows"));
            }
            let mut row = serde_json::Map::new();
            for (i, key) in header.iter().enumerate() {
                row.insert(
                    key.clone(),
                    cells.get(i).map(|v| json!(v)).unwrap_or(Value::Null),
                );
            }
            let row = Value::Object(row);
            let Some(selected) = selected else {
                return Ok(());
            };
            edges = edges
                .checked_add(1)
                .ok_or(Error::Budget("canon source edges"))?;
            let edge = text(row.get("edge_id"))
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{pack}:{global_csv_order}"));
            // CSV cells are copied without whitespace normalization. Python's
            // `str(row.get(key) or fallback)` retains a nonempty cell verbatim.
            let cell = |key: &str| {
                row.get(key)
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
                    .unwrap_or("")
            };
            let edge = row
                .get("edge_id")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .unwrap_or(&edge);
            let output = json!({"edge_id":edge,"pack_id":pack,"from_id":cell("from_id"),"predicate_id":cell("predicate_id"),"to_id":cell("to_id"),"owner_branch":format!("ToS/{branch}"),"authority_layer":if branch=="canon"{"canon"}else{"candidate_intake"},"layer":cell("layer"),"status":row.get("status").and_then(Value::as_str).filter(|v|!v.is_empty()).unwrap_or(if branch=="canon"{"canon"}else{"unmarked"}),"properties":{"source_record":row,"source_row":ordinal,"source_file_sha256":file_sha}});
            let bytes = encode_value(&output, l.max_raw_row_bytes)?;
            insert(
                stage,
                &selected.source_graph_id,
                "relation_edges",
                &format!("{pack}:{}", edge.trim()),
                &bytes,
                &mut work,
                l,
            )
        })?;
        let columns = columns.ok_or(Error::Invalid("canon CSV missing header"))?;
        if let Some(selected) = selected {
            packs = packs
                .checked_add(1)
                .ok_or(Error::Budget("canon source packs"))?;
            if packs > l.max_packs {
                return Err(Error::Budget("canon source packs"));
            }
            let route = pack.split_once('/').map_or(pack, |(_, tail)| tail);
            let output = json!({"pack_id":pack,"path":path,"route_hint":route,"owner_branch":format!("ToS/{branch}"),"authority_layer":if branch=="canon"{"canon"}else{"candidate_intake"},"edge_count":ordinal,"columns":columns,"sha256":file_sha});
            insert(
                stage,
                &selected.source_graph_id,
                "relation_packs",
                pack,
                &encode_value(&output, l.max_raw_row_bytes)?,
                &mut work,
                l,
            )?;
        }
    }
    if manifest_seen != manifest.count {
        return Err(Error::Invalid("canon current manifest EOF"));
    }
    let mut collections = Vec::new();
    for selected in &sources {
        let names: &[&str] = if selected.adapter_profile == CANON_PROFILE {
            &["nodes", "relation_packs", "relation_edges"]
        } else {
            &["relation_packs", "relation_edges"]
        };
        for &collection in names {
            let key = CanonSourceCollection {
                source_graph: selected.source_graph_id.clone(),
                input_role: selected.input_role.clone(),
                adapter_profile: selected.adapter_profile.clone(),
                collection: collection.into(),
                count: 0,
                root_sha256: String::new(),
            };
            let (count, digest) = visit_rows(
                stage,
                &key,
                l,
                deadline,
                cancelled,
                &mut work.bytes,
                |_, _| Ok(()),
            )?;
            collections.push(CanonSourceCollection {
                source_graph: selected.source_graph_id.clone(),
                input_role: selected.input_role.clone(),
                adapter_profile: selected.adapter_profile.clone(),
                collection: collection.into(),
                count,
                root_sha256: digest,
            });
        }
    }
    check(deadline, cancelled)?;
    Ok(CanonSourceReceipt {
        source_revision: revision.0.to_hex(),
        job_source_cut: stage.exact_receipt()?.binding.source_cut.clone(),
        manifest_members: manifest.count,
        manifest_membership_root_sha256: manifest.digest.to_hex(),
        selected_members_read: work.selected,
        selected_source_root_sha256: work.source_root.finalize().to_hex(),
        nodes,
        packs,
        edges,
        collections,
        work_bytes: work.bytes,
        current_members_only: true,
        final_graph_rows_written: false,
    })
}
/// A mechanical handoff frozen after full selected source derivation. The
/// private fields prevent a caller from substituting roots or custody binding.
/// The planner owns the SQL rows until render succeeds or the stage is dropped.
#[derive(Clone, Debug)]
pub struct CanonSourcePlan {
    receipt: CanonSourceReceipt,
    planner_binding: Value,
    source_inputs: Vec<InputCollectionReceipt>,
    selected_revision: SourceRevision,
    selected_membership: SourceMembershipV1,
}
impl CanonSourcePlan {
    pub fn receipt(&self) -> &CanonSourceReceipt {
        &self.receipt
    }
}
fn binding_snapshot(b: &SourceBinding) -> Value {
    json!({"owner_profile":b.owner_profile,"source_cut":b.source_cut,
        "through_commit_seq":b.through_commit_seq,"membership_root":b.membership_root,
        "index_generation":b.index_generation,"route_map_version":b.route_map_version,
        "reader_abi":b.reader_abi,"projection_root_sha256":b.projection_root_sha256,
        "complete":b.complete})
}
fn collection_snapshot(c: &InputCollectionReceipt) -> Value {
    json!({"source_graph":c.source_graph,"collection":c.collection,
        "input_role":c.input_role,"adapter_profile":c.adapter_profile,
        "count":c.expected_count,"root":c.expected_root_sha256})
}
fn charge_bytes(work: &mut u64, n: usize, l: CanonSourceLimits) -> Result<()> {
    *work = work
        .checked_add(n as u64)
        .ok_or(Error::Budget("canon source work"))?;
    if *work > l.max_work_bytes {
        return Err(Error::Budget("canon source work"));
    }
    Ok(())
}
fn selected_cut(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    l: CanonSourceLimits,
) -> Result<()> {
    if cut.current().revision() != revision
        || cut
            .stream(revision)
            .map_err(|e| Error::Source(e.to_string()))?
            .expectation()
            != membership
    {
        return Err(Error::Invalid("canon independent selected source cut"));
    }
    if membership.count > l.max_manifest_members {
        return Err(Error::Budget("canon source manifest members"));
    }
    Ok(())
}
// A real selected-source custody stage has independent raw-file receipts. Its
// membership_root may cover a different owner universe; per-file manifest
// equality and explicit current membership expectation perform this binding.
fn custody_source_path(path: &str) -> bool {
    (path.starts_with("ToS/canon/")
        && (path.ends_with("/node.json") || path.ends_with("/node.human-forms.json")))
        || ((path.starts_with("ToS/canon/") || path.starts_with("ToS/candidate-intake/"))
            && path.ends_with("/edges.csv")
            && !path.split('/').any(|p| p == "payload"))
}
fn source_custody(
    stage: &KnowledgeStage<'_>,
    cut: &CorpusCutReader,
    l: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
) -> Result<Vec<InputCollectionReceipt>> {
    if stage
        .exact_receipt()?
        .collections
        .iter()
        .filter(|c| c.source_graph == CANON_SOURCE_CUSTODY)
        .count()
        != 2
    {
        return Err(Error::Invalid("canon source custody collection closure"));
    }
    let mut inputs = Vec::new();
    for (name, role, profile) in [
        (
            "source-files",
            CANON_SOURCE_FILES_ROLE,
            CANON_SOURCE_FILES_PROFILE,
        ),
        (
            "contracts",
            CANON_SOURCE_CONTRACTS_ROLE,
            CANON_SOURCE_CONTRACTS_PROFILE,
        ),
    ] {
        let entries = stage
            .exact_receipt()?
            .collections
            .iter()
            .filter(|c| {
                c.source_graph == CANON_SOURCE_CUSTODY
                    && c.collection == name
                    && c.input_role == role
                    && c.adapter_profile == profile
            })
            .collect::<Vec<_>>();
        if entries.len() != 1 {
            return Err(Error::Invalid("canon source custody registration"));
        }
        let entry = entries[0];
        if entry.expected_count > l.max_manifest_members {
            return Err(Error::Budget("canon custody file count"));
        }
        let mut count = 0u64;
        let mut root = Digest256Hasher::new();
        let mut after = None;
        loop {
            check(deadline, cancelled)?;
            let page = stage.scan_input(&entry.source_graph, name, after.as_deref(), 1)?;
            for row in page.rows {
                if row.payload.len() > l.max_source_bytes {
                    return Err(Error::Budget("canon custody file bytes"));
                }
                charge_bytes(work, row.payload.len(), l)?;
                if (name == "source-files" && !custody_source_path(&row.id))
                    || (name == "contracts" && row.id != NODE_SCHEMA && row.id != FORM_SET_SCHEMA)
                {
                    return Err(Error::Invalid("canon source custody recipe path"));
                }
                let path =
                    RelativePath::parse(&row.id).map_err(|e| Error::Source(e.to_string()))?;
                let metadata = cut
                    .current()
                    .member(&path)
                    .ok_or(Error::Invalid("canon custody file outside current cut"))?;
                let digest = Digest256::of_bytes(&row.payload);
                if metadata.size_bytes != row.payload.len() as u64 || metadata.sha256 != digest {
                    return Err(Error::Invalid("canon custody current file digest/size"));
                }
                count = count
                    .checked_add(1)
                    .ok_or(Error::Budget("canon custody file count"))?;
                if count > l.max_manifest_members {
                    return Err(Error::Budget("canon custody file count"));
                }
                framed(&mut root, &row.id);
                root.update(digest.as_bytes());
            }
            after = page.next_id;
            if after.is_none() {
                break;
            }
        }
        if count != entry.expected_count || root.finalize().to_hex() != entry.expected_root_sha256 {
            return Err(Error::Invalid("canon independent custody root/count"));
        }
        inputs.push(entry.clone());
    }
    let mut seen = 0u64;
    for metadata in cut.current().members() {
        check(deadline, cancelled)?;
        seen = seen
            .checked_add(1)
            .ok_or(Error::Budget("canon manifest EOF"))?;
        if seen > l.max_manifest_members {
            return Err(Error::Budget("canon manifest EOF"));
        }
        let path = metadata.path.as_str();
        let source = custody_source_path(path);
        let contract = path == NODE_SCHEMA || path == FORM_SET_SCHEMA;
        if source || contract {
            let entry = &inputs[usize::from(contract)];
            let row = stage
                .raw_by_id(&entry.source_graph, &entry.collection, path)?
                .ok_or(Error::Invalid(
                    "canon required current custody file missing",
                ))?;
            charge_bytes(work, row.payload.len(), l)?;
        }
    }
    let expected = cut
        .stream(cut.current().revision())
        .map_err(|e| Error::Source(e.to_string()))?
        .expectation();
    if seen != expected.count {
        return Err(Error::Invalid("canon custody manifest EOF"));
    }
    for path in [NODE_SCHEMA, FORM_SET_SCHEMA] {
        check(deadline, cancelled)?;
        let row = stage
            .raw_by_id(&inputs[1].source_graph, &inputs[1].collection, path)?
            .ok_or(Error::Invalid("canon selected schema custody missing"))?;
        charge_bytes(work, row.payload.len(), l)?;
    }
    Ok(inputs)
}
fn visit_rows<F>(
    stage: &mut KnowledgeStage<'_>,
    collection: &CanonSourceCollection,
    l: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut u64,
    mut sink: F,
) -> Result<(u64, String)>
where
    F: FnMut(&mut KnowledgeStage<'_>, &[(String, Vec<u8>, Vec<u8>)]) -> Result<()>,
{
    let mut after: Option<String> = None;
    let mut count = 0u64;
    let mut root = Digest256Hasher::new();
    loop {
        check(deadline, cancelled)?;
        let batch: Vec<(String,Vec<u8>,Vec<u8>)> = stage.with_connection(WritePhase::Sort, |db| {
            let mut query = db.prepare("SELECT id,CASE WHEN length(payload)<=?4 THEN payload ELSE NULL END,payload_sha256 FROM knowledge_canon_source_rows WHERE source_graph=?1 AND collection=?2 AND (?3 IS NULL OR id>?3) ORDER BY id LIMIT ?5")?;
            let rows = query.query_map(params![collection.source_graph,collection.collection,after,
                l.max_raw_row_bytes as i64,l.max_page_rows as i64], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
            rows.collect::<std::result::Result<Vec<_>,_>>().map_err(Error::from)
        })?;
        if batch.is_empty() {
            break;
        }
        for (id, raw, sha) in &batch {
            check(deadline, cancelled)?;
            if sha.as_slice() != Digest256::of_bytes(&raw).as_bytes() {
                return Err(Error::Invalid("canon source disk row digest"));
            }
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("canon source raw rows"))?;
            let cap = match collection.collection.as_str() {
                "nodes" => l.max_nodes,
                "relation_packs" => l.max_packs,
                "relation_edges" => l.max_edges,
                _ => return Err(Error::Invalid("canon plan collection")),
            };
            if count > cap {
                return Err(Error::Budget("canon source raw rows"));
            }
            framed(&mut root, &id);
            root.update(&sha);
            charge_bytes(work, raw.len(), l)?;
        }
        sink(stage, &batch)?;
        after = batch.last().map(|(id, _, _)| id.clone());
    }
    Ok((count, root.finalize().to_hex()))
}
fn ingest_canon_page(
    target: &mut KnowledgeStage<'_>,
    collection: &CanonSourceCollection,
    page: &[(String, Vec<u8>, Vec<u8>)],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let (max_rows, max_bytes) = target.input_batch_limits();
    let mut next = 0;
    while next < page.len() {
        let mut rows = Vec::new();
        let mut bytes = 0u64;
        while next < page.len() && rows.len() < max_rows {
            check(deadline, cancelled)?;
            let (id, raw, _) = &page[next];
            let row_bytes = raw.len() as u64;
            if row_bytes > max_bytes {
                if rows.is_empty() {
                    target.ingest_input(InputRow {
                        source_graph: &collection.source_graph,
                        collection: &collection.collection,
                        id,
                        payload: raw,
                    })?;
                    next += 1;
                }
                break;
            }
            if bytes + row_bytes > max_bytes {
                break;
            }
            bytes += row_bytes;
            rows.push(InputRow {
                source_graph: &collection.source_graph,
                collection: &collection.collection,
                id,
                payload: raw,
            });
            next += 1;
        }
        if !rows.is_empty() {
            target.ingest_input_batch(&rows)?;
        }
        check(deadline, cancelled)?;
    }
    Ok(())
}
fn match_target(stage: &KnowledgeStage<'_>, receipt: &CanonSourceReceipt) -> Result<()> {
    for c in &receipt.collections {
        let entries = stage
            .exact_receipt()?
            .collections
            .iter()
            .filter(|r| r.source_graph == c.source_graph && r.collection == c.collection)
            .collect::<Vec<_>>();
        if entries.len() != 1
            || entries[0].input_role != c.input_role
            || entries[0].adapter_profile != c.adapter_profile
            || entries[0].expected_count != c.count
            || entries[0].expected_root_sha256 != c.root_sha256
        {
            return Err(Error::Invalid(
                "canon source independent raw registration/root/count",
            ));
        }
    }
    for graph in receipt
        .collections
        .iter()
        .map(|c| &c.source_graph)
        .collect::<BTreeSet<_>>()
    {
        if stage
            .exact_receipt()?
            .collections
            .iter()
            .filter(|c| &c.source_graph == graph)
            .count()
            != receipt
                .collections
                .iter()
                .filter(|c| &c.source_graph == graph)
                .count()
        {
            return Err(Error::Invalid("canon source raw collection coverage"));
        }
    }
    Ok(())
}
fn clear_plan(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.with_connection(WritePhase::Finalize, |db| {
        db.execute_batch("DROP TABLE knowledge_canon_source_rows")?;
        Ok(())
    })
}
/// Derive exact native raw collections before creating the final stage. The
/// supplied planner must hold the exact CANON_SOURCE_CUSTODY raw-file recipe;
/// output counts/roots are derived mechanical witnesses, never admission.
pub fn plan_canon_source_inputs<F>(
    planner: &mut KnowledgeStage<'_>,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    vocabulary: &QueryVocabulary,
    executor: &mut impl CutSchemaExecutor,
    limits: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut materialize_forms: F,
) -> Result<CanonSourcePlan>
where
    F: FnMut(&Value, &Value, usize) -> Result<Vec<Value>>,
{
    let result = (|| {
        limits.validate()?;
        check(deadline, cancelled)?;
        selected_cut(cut, expected_revision, expected_membership, limits)?;
        let mut work = 0;
        let source_inputs = source_custody(planner, cut, limits, deadline, cancelled, &mut work)?;
        let receipt = derive(
            planner,
            cut,
            expected_revision,
            expected_membership,
            vocabulary,
            executor,
            limits,
            deadline,
            cancelled,
            &mut materialize_forms,
            work,
        )?;
        Ok(CanonSourcePlan {
            receipt,
            planner_binding: binding_snapshot(&planner.exact_receipt()?.binding),
            source_inputs,
            selected_revision: expected_revision,
            selected_membership: expected_membership,
        })
    })();
    if result.is_err() {
        planner.poison();
    }
    result
}
/// Transfer a frozen plan into an independently created native stage. All
/// custody and raw roots are checked before any target row is ingested. The
/// current cut is explicit; a retained cut cannot stand in for current custody.
pub fn render_canon_source_plan(
    planner: &mut KnowledgeStage<'_>,
    plan: &CanonSourcePlan,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    target: &mut KnowledgeStage<'_>,
    limits: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CanonSourceReceipt> {
    let result = (|| {
        limits.validate()?;
        check(deadline, cancelled)?;
        selected_cut(cut, expected_revision, expected_membership, limits)?;
        if plan.selected_revision != expected_revision
            || plan.selected_membership != expected_membership
            || binding_snapshot(&planner.exact_receipt()?.binding) != plan.planner_binding
            || plan.receipt.source_revision != expected_revision.0.to_hex()
            || plan.receipt.manifest_members != expected_membership.count
            || plan.receipt.manifest_membership_root_sha256 != expected_membership.digest.to_hex()
        {
            return Err(Error::Invalid("canon frozen source plan binding"));
        }
        let mut target_binding = binding_snapshot(&target.exact_receipt()?.binding);
        // Each stage's projection digest binds its own independent raw input
        // transport. All source-owner identity/currentness fields must agree.
        target_binding["projection_root_sha256"] =
            plan.planner_binding["projection_root_sha256"].clone();
        if target_binding != plan.planner_binding {
            return Err(Error::Invalid("canon target source binding"));
        }
        match_target(target, &plan.receipt)?;
        let mut work = plan.receipt.work_bytes;
        let inputs = source_custody(planner, cut, limits, deadline, cancelled, &mut work)?;
        if inputs.len() != plan.source_inputs.len()
            || inputs
                .iter()
                .zip(&plan.source_inputs)
                .any(|(a, b)| collection_snapshot(a) != collection_snapshot(b))
        {
            return Err(Error::Invalid("canon frozen source custody receipts"));
        }
        for c in &plan.receipt.collections {
            let (count, root) = visit_rows(
                planner,
                c,
                limits,
                deadline,
                cancelled,
                &mut work,
                |_, _| Ok(()),
            )?;
            if count != c.count || root != c.root_sha256 {
                return Err(Error::Invalid("canon frozen raw plan root/count"));
            }
        }
        for c in &plan.receipt.collections {
            let (count, root) = visit_rows(
                planner,
                c,
                limits,
                deadline,
                cancelled,
                &mut work,
                |_, page| ingest_canon_page(target, c, page, deadline, cancelled),
            )?;
            if count != c.count || root != c.root_sha256 {
                return Err(Error::Invalid("canon rendered raw plan root/count"));
            }
        }
        clear_plan(planner)?;
        let mut receipt = plan.receipt.clone();
        receipt.work_bytes = work;
        Ok(receipt)
    })();
    if result.is_err() {
        planner.poison();
        target.poison();
    }
    result
}
/// Actual native CMD adapter injects only its pure source-copy materializer.
/// `CutWorkerSchemaExecutor::from_cut` and all source/member/resource custody
/// remain owned by the caller. Independent raw job receipts must already be
/// registered; this function cannot rewrite that selected input contract.
pub fn prepare_canon_source_inputs<F>(
    stage: &mut KnowledgeStage<'_>,
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    vocabulary: &QueryVocabulary,
    executor: &mut impl CutSchemaExecutor,
    limits: CanonSourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut materialize_forms: F,
) -> Result<CanonSourceReceipt>
where
    F: FnMut(&Value, &Value, usize) -> Result<Vec<Value>>,
{
    let result = (|| {
        let mut receipt = derive(
            stage,
            cut,
            expected_revision,
            expected_membership,
            vocabulary,
            executor,
            limits,
            deadline,
            cancelled,
            &mut materialize_forms,
            0,
        )?;
        match_target(stage, &receipt)?;
        let mut work = receipt.work_bytes;
        for collection in &receipt.collections {
            let (count, root) = visit_rows(
                stage,
                collection,
                limits,
                deadline,
                cancelled,
                &mut work,
                |stage, page| ingest_canon_page(stage, collection, page, deadline, cancelled),
            )?;
            if count != collection.count || root != collection.root_sha256 {
                return Err(Error::Invalid("canon rendered raw plan root/count"));
            }
        }
        clear_plan(stage)?;
        receipt.work_bytes = work;
        Ok(receipt)
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    // Expected logical records were independently observed with CPython
    // csv.reader/DictReader(strict=True). This protects genuine source-format
    // boundaries, rather than comparing the projection to itself.
    #[test]
    fn python_oracle_canon_source_conversion_boundaries() {
        let l = CanonSourceLimits {
            max_manifest_members: 16,
            max_selected_members: 16,
            max_nodes: 8,
            max_packs: 8,
            max_edges: 16,
            max_source_bytes: 8192,
            max_raw_row_bytes: 8192,
            max_csv_fields: 16,
            max_csv_record_bytes: 4096,
            max_forms: 16,
            max_forms_output_bytes: 65536,
            max_page_rows: 1,
            max_page_bytes: 8192,
            max_work_bytes: 65536,
        };
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        let raw = b"a,b,c\r\n\"one\r\ntwo\",\"quote\"\"word\",\r\n\r\nx\r\n,\r\n";
        let mut records = Vec::new();
        csv_records(raw, l, deadline, &cancelled, |row| {
            records.push(row);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            records,
            vec![
                vec!["a", "b", "c"],
                vec!["one\r\ntwo", "quote\"word", ""],
                vec![],
                vec!["x"],
                vec!["", ""]
            ]
        );
        let mut literal = Vec::new();
        csv_records(
            b"a,b\nword\"literal,\"quoted\"\n",
            l,
            deadline,
            &cancelled,
            |r| {
                literal.push(r);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(literal[1], vec!["word\"literal", "quoted"]);
        for raw in [
            b"a,b\n\"unfinished".as_slice(),
            b"a,b\n\"done\"x,y\n".as_slice(),
        ] {
            assert!(csv_records(raw, l, deadline, &cancelled, |_| Ok(())).is_err());
        }
        let exact = parse(br#"{"x":1e-05,"y":-0.0,"z":0.1}"#, 8192).unwrap();
        exact_node_numbers(&exact, 8192).unwrap();
        let lossy = parse(br#"{"x":0.100000000000000005}"#, 8192).unwrap();
        assert!(exact_node_numbers(&lossy, 8192).is_err());
        cancelled.store(true, Ordering::Relaxed);
        assert!(csv_records(raw, l, deadline, &cancelled, |_| Ok(())).is_err());
    }
}

#[cfg(test)]
mod exact_csv_read_tests {
    use super::*;
    #[test]
    fn exact_csv_spans_preserve_quoted_newlines_and_missing_cells() {
        let raw = b"edge_id,label,note\r\n\r\ne1,\"alpha\r\nbeta\",\"a\"\"b\"\r\ne2,last\r\n";
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + std::time::Duration::from_secs(1);
        let first = read_exact_authored_csv_row(
            raw,
            1,
            &json!({"edge_id":"e1","label":"alpha\r\nbeta","note":"a\"b"}),
            1024,
            deadline,
            &cancelled,
        )
        .unwrap();
        let start = first["byte_offset"].as_u64().unwrap() as usize;
        let len = first["row_bytes"].as_u64().unwrap() as usize;
        assert_eq!(
            first["raw_record"],
            std::str::from_utf8(&raw[start..start + len]).unwrap()
        );
        assert_eq!(first["raw_record"], "e1,\"alpha\r\nbeta\",\"a\"\"b\"\r\n");
        let second = read_exact_authored_csv_row(
            raw,
            2,
            &json!({"edge_id":"e2","label":"last","note":null}),
            1024,
            deadline,
            &cancelled,
        )
        .unwrap();
        assert_eq!(second["raw_record"], "e2,last\r\n");
        assert!(
            read_exact_authored_csv_row(
                raw,
                2,
                &json!({"edge_id":"e2","label":"last","note":""}),
                1024,
                deadline,
                &cancelled
            )
            .is_err()
        );
        assert!(
            read_exact_authored_csv_row(
                raw,
                1,
                &json!({"edge_id":"e1","label":"alpha\r\nbeta","note":"a\"b"}),
                4,
                deadline,
                &cancelled
            )
            .is_err()
        );
    }
}
