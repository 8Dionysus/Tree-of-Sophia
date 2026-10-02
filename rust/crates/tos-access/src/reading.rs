//! Maintained local reading adapter. The QRY engine owns source/fixity/grouping
//! semantics; transport parsing and the capability envelope stay with access.
use crate::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence, PreparedPacket,
};
use std::{io::Write, sync::Arc};
use tos_foundation::{
    JsonLimits, JsonMode, JsonNumberKind, JsonString, JsonValue, emit_python_compact_json,
    parse_json,
};
use tos_query::{
    AbortProbe,
    reading_search::{
        ExplicitReadingRoots, ReadingSearchBudget, ReadingSearchError, ReadingSearchErrorCode,
        ReadingSearchRequest, ReadingSearchResult, ReadingSoftware,
    },
};

pub const OPERATION_ID: &str = "tos_zarathustra_reading_search";
pub const MCP_TOOL: &str = "tos_zarathustra_reading_search";
pub const HTTP_PATH: &str = "/api/zarathustra/reading";
fn invalid() -> AccessError {
    AccessError::new(
        AccessErrorCode::InvalidRequest,
        "invalid reading-search arguments",
    )
}
fn query_error(e: ReadingSearchError) -> AccessError {
    let code = match e.code {
        ReadingSearchErrorCode::InvalidRequest => AccessErrorCode::InvalidRequest,
        ReadingSearchErrorCode::Unavailable | ReadingSearchErrorCode::Unsupported => {
            AccessErrorCode::Unavailable
        }
        ReadingSearchErrorCode::CorruptSelectedCarrier => AccessErrorCode::CorruptSelectedCarrier,
        ReadingSearchErrorCode::BudgetExceeded => AccessErrorCode::BudgetExceeded,
        ReadingSearchErrorCode::Cancelled => AccessErrorCode::Cancelled,
        ReadingSearchErrorCode::DeadlineExceeded => AccessErrorCode::DeadlineExceeded,
    };
    AccessError::new(code, e.message)
}
fn bounded_limit(value: &JsonValue) -> Result<usize, AccessError> {
    let JsonValue::Number(number) = value else {
        return Err(invalid());
    };
    if number.kind != JsonNumberKind::Int {
        return Err(invalid());
    }
    let digits = number.lexeme.strip_prefix('-').unwrap_or(&number.lexeme);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    if number.lexeme.starts_with('-') {
        return Ok(0);
    }
    let digits = digits.trim_start_matches('0');
    if digits.len() > 2 {
        Ok(100)
    } else {
        Ok(digits.parse::<usize>().unwrap_or(0).min(100))
    }
}
pub fn from_arguments(args: &JsonValue) -> Result<ReadingSearchRequest, AccessError> {
    let fields = args.as_object().ok_or_else(invalid)?;
    if fields.iter().any(|(key, _)| {
        !key.as_str().is_some_and(|key| {
            [
                "query",
                "language",
                "limit",
                "include_semantic_neighbors",
                "group_by",
            ]
            .contains(&key)
        })
    }) {
        return Err(invalid());
    }
    let query = args
        .object_get("query")
        .and_then(JsonValue::as_str)
        .ok_or_else(invalid)?;
    let language = match args.object_get("language") {
        None => "ru",
        Some(v) => v.as_str().ok_or_else(invalid)?,
    };
    let limit = args
        .object_get("limit")
        .map(bounded_limit)
        .transpose()?
        .unwrap_or(20);
    let include_semantic_neighbors = args
        .object_get("include_semantic_neighbors")
        .map(|v| v.as_bool().ok_or_else(invalid))
        .transpose()?
        .unwrap_or(false);
    let group_by = match args.object_get("group_by") {
        None | Some(JsonValue::Null) => vec!["speaker".into(), "formula".into()],
        Some(v) => v
            .as_array()
            .ok_or_else(invalid)?
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or_else(invalid))
            .collect::<Result<Vec<_>, _>>()?,
    };
    let request = ReadingSearchRequest {
        query: query.into(),
        language: language.into(),
        limit,
        include_semantic_neighbors,
        group_by,
        request_ref: None,
    };
    tos_query::reading_search::normalize_reading_request(&request).map_err(query_error)
}
fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn limits(cap: usize) -> JsonLimits {
    JsonLimits {
        max_bytes: cap,
        max_depth: 96,
        max_visits: 300_000,
        max_integer_digits: 4096,
    }
}
fn capability(result: JsonValue, provider_ref: &str, reason: Option<&str>) -> JsonValue {
    let available = !matches!(result, JsonValue::Null);
    object(vec![
        ("schema", text("tos_zarathustra_reading_capability_v1")),
        ("available", JsonValue::Bool(available)),
        ("reason", reason.map(text).unwrap_or(JsonValue::Null)),
        ("provider_ref", text(provider_ref)),
        (
            "publication_posture",
            text(if available {
                "local_full_tree_only"
            } else {
                "excluded_from_public_bundle"
            }),
        ),
        ("result", result),
        ("task", JsonValue::Null),
        (
            "authority",
            object(vec![
                ("source_owner", text("Tree-of-Sophia")),
                ("access_plane_is_source", JsonValue::Bool(false)),
                ("is_semantic_truth", JsonValue::Bool(false)),
                ("writes_to_tree", JsonValue::Bool(false)),
                ("reviewed", JsonValue::Bool(false)),
                ("canon", JsonValue::Bool(false)),
            ]),
        ),
    ])
}
struct CapabilityFence<'hold> {
    outer: Option<Box<dyn DisclosureFence + 'hold>>,
    probe: Arc<dyn AbortProbe>,
}
impl DisclosureFence for CapabilityFence<'_> {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.probe)?;
        if let Some(outer) = self.outer.as_mut() {
            outer.recheck()?
        }
        crate::knowledge::check_abort(&self.probe)
    }
}
pub(crate) fn unavailable_packet(
    probe: Arc<dyn AbortProbe>,
) -> Result<PreparedPacket<'static>, AccessError> {
    let body = emit_python_compact_json(
        &capability(
            JsonValue::Null,
            tos_query::reading_search::READING_PROVIDER_REF,
            Some("local reading data root is not selected"),
        ),
        limits(1_048_576),
    )
    .map_err(|_| {
        AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "reading capability byte budget exceeded",
        )
    })?;
    let mut fence = CapabilityFence { outer: None, probe };
    fence.recheck()?;
    Ok(PreparedPacket {
        body,
        fence: Box::new(fence),
    })
}
struct ReadingFence<'hold> {
    result: ReadingSearchResult,
    outer: Box<dyn DisclosureFence + 'hold>,
    probe: Arc<dyn AbortProbe>,
}
impl DisclosureFence for ReadingFence<'_> {
    fn recheck(&mut self) -> Result<(), AccessError> {
        self.outer.recheck()?;
        self.result
            .recheck(self.probe.as_ref())
            .map_err(query_error)?;
        self.outer.recheck()
    }
}
/// The supplied outer fence is the existing selected data-holder observation.
/// It grants no source, rights or publication authority and remains through flush.
pub fn prepare<'hold>(
    roots: &ExplicitReadingRoots,
    software: &ReadingSoftware,
    request: &ReadingSearchRequest,
    mut budget: ReadingSearchBudget,
    packet_cap: usize,
    provider_ref: &str,
    outer: Box<dyn DisclosureFence + 'hold>,
    probe: Arc<dyn AbortProbe>,
) -> Result<PreparedPacket<'hold>, AccessError> {
    let raw_cap = packet_cap
        .checked_sub(512)
        .filter(|v| *v > 0)
        .ok_or_else(|| {
            AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "reading capability framing budget unavailable",
            )
        })?;
    budget.max_response_bytes = budget.max_response_bytes.min(raw_cap);
    let mut result = match tos_query::reading_search::execute_reading_search(
        roots,
        software,
        request,
        budget,
        Arc::clone(&probe),
    ) {
        Ok(result) => result,
        Err(e) if e.code == ReadingSearchErrorCode::Unavailable => {
            let body = emit_python_compact_json(
                &capability(JsonValue::Null, provider_ref, Some(e.message)),
                limits(packet_cap),
            )
            .map_err(|_| {
                AccessError::new(
                    AccessErrorCode::BudgetExceeded,
                    "reading capability response byte budget exceeded",
                )
            })?;
            let mut fence = CapabilityFence {
                outer: Some(outer),
                probe,
            };
            fence.recheck()?;
            return Ok(PreparedPacket {
                body,
                fence: Box::new(fence),
            });
        }
        Err(e) => return Err(query_error(e)),
    };
    result.recheck(probe.as_ref()).map_err(query_error)?;
    let raw = parse_json(&result.body, JsonMode::PublishedStrict, limits(packet_cap))
        .map_err(|_| {
            AccessError::new(
                AccessErrorCode::CorruptSelectedCarrier,
                "reading result JSON invalid",
            )
        })?
        .into_root();
    let body = emit_python_compact_json(&capability(raw, provider_ref, None), limits(packet_cap))
        .map_err(|_| {
        AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "reading capability response byte budget exceeded",
        )
    })?;
    result.body.clear();
    result.body.shrink_to_fit();
    let mut fence = ReadingFence {
        result,
        outer,
        probe,
    };
    fence.recheck()?;
    Ok(PreparedPacket {
        body,
        fence: Box::new(fence),
    })
}
pub(crate) fn run_cli(
    args: &[String],
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let mut fields = Vec::new();
    let mut at = 1;
    while at < args.len() {
        let option = args[at].as_str();
        if option == "--include-semantic-neighbors" {
            fields.retain(|(k, _)| *k != "include_semantic_neighbors");
            fields.push(("include_semantic_neighbors", JsonValue::Bool(true)));
            at += 1;
            continue;
        }
        let Some(value) = args.get(at + 1) else {
            let _ = writeln!(stderr, "missing value for {option}");
            return 2;
        };
        let (key, value) = match option {
            "--query" => ("query", text(value)),
            "--language" if ["de", "ru", "en"].contains(&value.as_str()) => {
                ("language", text(value))
            }
            "--limit" => {
                let n = JsonValue::Number(tos_foundation::JsonNumber {
                    kind: JsonNumberKind::Int,
                    lexeme: value.clone(),
                });
                if bounded_limit(&n).is_err() {
                    let _ = writeln!(stderr, "invalid reading limit");
                    return 2;
                }
                ("limit", n)
            }
            "--group-by" => (
                "group_by",
                JsonValue::Array(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(text)
                        .collect(),
                ),
            ),
            _ => {
                let _ = writeln!(stderr, "unsupported reading option: {option}");
                return 2;
            }
        };
        fields.retain(|(k, _)| *k != key);
        fields.push((key, value));
        at += 2;
    }
    let result = from_arguments(&object(fields)).and_then(|request| {
        crate::checked_execute(profile.deadline_probe(), |probe| {
            executor.reading_search(request, probe)
        })
    });
    match result {
        Ok(packet) => crate::cli::write_packet(packet, profile, stdout, stderr),
        Err(e) => {
            let _ = writeln!(stderr, "{}: {}", e.code_str(), e.message);
            if e.code == AccessErrorCode::Unavailable {
                3
            } else if e.code == AccessErrorCode::InvalidRequest {
                2
            } else {
                1
            }
        }
    }
}

/// Explicit local data roots plus compiled software; an observational pin
/// holder, with no source-rights or publication grant.
pub struct ReadingLocalExecutor {
    selected: Arc<ReadingRoot>,
    analysis: Arc<ReadingRoot>,
    reading_budget: ReadingSearchBudget,
}
struct ReadingRoot {
    path: std::path::PathBuf,
    directory: std::fs::File,
    dev: u64,
    ino: u64,
}
impl ReadingLocalExecutor {
    pub fn open(path: std::path::PathBuf) -> Result<Self, AccessError> {
        let selected = Arc::new(ReadingRoot::open(path)?);
        Ok(Self {
            analysis: Arc::clone(&selected),
            selected,
            reading_budget: ReadingSearchBudget::local_default(),
        })
    }

    /// Select source and immutable reading outputs independently. Neither root
    /// selects software or supplies a publication/right grant.
    pub fn open_roots(
        source: std::path::PathBuf,
        analysis: std::path::PathBuf,
    ) -> Result<Self, AccessError> {
        Self::open_roots_with_budget(source, analysis, ReadingSearchBudget::local_default())
    }

    pub fn open_roots_with_budget(
        source: std::path::PathBuf,
        analysis: std::path::PathBuf,
        reading_budget: ReadingSearchBudget,
    ) -> Result<Self, AccessError> {
        if reading_budget.max_file_bytes == 0
            || reading_budget.max_total_file_bytes == 0
            || reading_budget.max_file_bytes > reading_budget.max_total_file_bytes
        {
            return Err(invalid());
        }
        Ok(Self {
            selected: Arc::new(ReadingRoot::open(source)?),
            analysis: Arc::new(ReadingRoot::open(analysis)?),
            reading_budget,
        })
    }
}
impl ReadingRoot {
    fn open(path: std::path::PathBuf) -> Result<Self, AccessError> {
        use std::os::unix::fs::MetadataExt;
        if !path.is_absolute() {
            return Err(invalid());
        }
        let directory = tos_fd_open::open_absolute_directory(&path).map_err(|_| {
            AccessError::new(
                AccessErrorCode::Unavailable,
                "selected reading data root unavailable",
            )
        })?;
        let m = directory.metadata().map_err(|_| {
            AccessError::new(
                AccessErrorCode::Unavailable,
                "selected reading data root unavailable",
            )
        })?;
        Ok(Self {
            path,
            directory,
            dev: m.dev(),
            ino: m.ino(),
        })
    }
}
struct RootFence(Arc<ReadingRoot>);
impl DisclosureFence for RootFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        use std::os::unix::fs::MetadataExt;
        let named = tos_fd_open::open_absolute_directory(&self.0.path).map_err(|_| {
            AccessError::new(AccessErrorCode::StaleSelection, "reading data root changed")
        })?;
        for m in [self.0.directory.metadata(), named.metadata()] {
            let m = m.map_err(|_| {
                AccessError::new(AccessErrorCode::StaleSelection, "reading data root changed")
            })?;
            if (m.dev(), m.ino()) != (self.0.dev, self.0.ino) {
                return Err(AccessError::new(
                    AccessErrorCode::StaleSelection,
                    "reading data root changed",
                ));
            }
        }
        Ok(())
    }
}
struct ReadingRootsFence {
    source: RootFence,
    analysis: RootFence,
}
impl DisclosureFence for ReadingRootsFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        self.source.recheck()?;
        self.analysis.recheck()
    }
}
impl AccessExecutor for ReadingLocalExecutor {
    fn concept_search(
        &self,
        request: tos_query::reading_search::ConceptSearchRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        let roots = ExplicitReadingRoots {
            source_root: self.selected.path.clone(),
            analysis_root: self.selected.path.clone(),
        };
        let mut result = tos_query::reading_search::execute_concept_search(
            &roots,
            &ReadingSoftware::embedded(),
            &request,
            ReadingSearchBudget::local_default(),
            Arc::clone(&probe),
        )
        .map_err(query_error)?;
        let body = std::mem::take(&mut result.body);
        let mut fence = ReadingFence {
            result,
            outer: Box::new(RootFence(Arc::clone(&self.selected))),
            probe,
        };
        fence.recheck()?;
        Ok(PreparedPacket {
            body,
            fence: Box::new(fence),
        })
    }

    fn word_analysis_available(&self) -> bool {
        true
    }
    fn word_analysis(
        &self,
        request: tos_query::reading_search::WordAnalysisRequest,
        candidate: Option<&[u8]>,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        let roots = ExplicitReadingRoots {
            source_root: self.selected.path.clone(),
            analysis_root: self.selected.path.clone(),
        };
        let software = ReadingSoftware::embedded();
        let budget = ReadingSearchBudget::local_default();
        let mut result = match candidate {
            Some(bytes) => tos_query::reading_search::validate_word_analysis_candidate(
                &roots,
                &software,
                &request,
                bytes,
                budget,
                Arc::clone(&probe),
            ),
            None => tos_query::reading_search::execute_word_analysis_task(
                &roots,
                &software,
                &request,
                budget,
                Arc::clone(&probe),
            ),
        }
        .map_err(query_error)?;
        let body = std::mem::take(&mut result.body);
        let mut fence = ReadingFence {
            result,
            outer: Box::new(RootFence(Arc::clone(&self.selected))),
            probe,
        };
        fence.recheck()?;
        Ok(PreparedPacket {
            body,
            fence: Box::new(fence),
        })
    }

    fn source_descend_available(&self) -> bool {
        false
    }
    fn source_descend(
        &self,
        _: crate::Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        Err(AccessError::new(
            AccessErrorCode::Unavailable,
            "selected local source operation unavailable",
        ))
    }
    fn reading_search_available(&self) -> bool {
        true
    }
    fn reading_search(
        &self,
        request: ReadingSearchRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        let roots = ExplicitReadingRoots {
            source_root: self.selected.path.clone(),
            analysis_root: self.analysis.path.clone(),
        };
        prepare(
            &roots,
            &ReadingSoftware::embedded(),
            &request,
            self.reading_budget.clone(),
            1_048_576,
            tos_query::reading_search::READING_PROVIDER_REF,
            Box::new(ReadingRootsFence {
                source: RootFence(Arc::clone(&self.selected)),
                analysis: RootFence(Arc::clone(&self.analysis)),
            }),
            probe,
        )
    }
}

/// The existing public-bundle capability excludes the local source provider.
/// Query parameters cannot convert this software posture into source access.
pub(crate) fn public_word_analysis_capability(cap: usize) -> Result<Vec<u8>, crate::AccessError> {
    let raw = r#"{"schema":"tos_zarathustra_word_analysis_capability_v1","available":false,"reason":"local source-bound word-analysis provider is excluded from the public bundle","provider_ref":"scripts/prepare_zarathustra_word_analysis_v1.py","publication_posture":"excluded_from_public_bundle","task":null,"authority":{"source_owner":"Tree-of-Sophia","access_plane_is_source":false,"is_semantic_truth":false,"writes_to_tree":false,"reviewed":false,"canon":false}}"#;
    if raw.len() > cap {
        return Err(crate::AccessError::new(
            crate::AccessErrorCode::BudgetExceeded,
            "word-analysis capability response budget",
        ));
    }
    Ok(raw.as_bytes().to_vec())
}
