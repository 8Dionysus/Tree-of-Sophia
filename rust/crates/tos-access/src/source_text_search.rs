//! Transport binding for an explicitly selected private PG source-text consumer.
//!
//! The browser, CLI and MCP requests contain search terms and filters only.
//! This selected server-side path owns the PG selector; it never accepts a
//! domain, consumer, profile, or connection string from a request.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tos_foundation::{JsonNumber, JsonNumberKind, JsonString, JsonValue};
use tos_query::{AbortProbe, AbortReason, source_text_search::SourceTextSearchRequest};

use crate::{AccessError, AccessErrorCode, AccessProfile, DisclosureFence, PreparedPacket};

#[derive(Clone, Debug)]
pub struct SelectedPgSourceTextSearch {
    selection_path: PathBuf,
}

impl SelectedPgSourceTextSearch {
    /// Bind one explicit absolute owner selection. Relative paths are rejected
    /// so selection can never depend on the process working directory.
    pub fn new(selection_path: &Path) -> Result<Self, AccessError> {
        if !selection_path.is_absolute() {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "source-text selection requires an absolute path",
            ));
        }
        Ok(Self {
            selection_path: selection_path.to_owned(),
        })
    }

    pub fn execute(
        &self,
        request_json: &[u8],
        profile: AccessProfile,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        check_probe(probe.as_ref())?;
        if request_json.len() > profile.max_request_bytes {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "source-text request exceeds byte budget",
            ));
        }
        let request = SourceTextSearchRequest::from_json(request_json, profile.json_limits())
            .map_err(|error| match error.code {
                tos_query::source_text_search::SourceTextSearchErrorCode::BudgetExceeded => {
                    AccessError::new(
                        AccessErrorCode::BudgetExceeded,
                        "source-text request exceeds budget",
                    )
                }
                _ => AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "invalid source-text search request",
                ),
            })?;
        self.execute_request(request, profile, probe)
    }

    pub fn execute_request(
        &self,
        request: SourceTextSearchRequest,
        profile: AccessProfile,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        check_probe(probe.as_ref())?;
        let now = Instant::now();
        let timeout = profile
            .query_timeout
            .unwrap_or(Duration::from_secs(15))
            .min(Duration::from_secs(60));
        let deadline = now.checked_add(timeout).ok_or_else(|| {
            AccessError::new(
                AccessErrorCode::DeadlineExceeded,
                "source-text search deadline unavailable",
            )
        })?;

        let outcome = tos_command::source_projection_cli::run_source_text_search(
            &self.selection_path,
            request,
            deadline,
            probe.as_ref(),
        )
        .map_err(map_durable_error)?;
        check_probe(probe.as_ref())?;
        let (body, disclosure) = match outcome {
            tos_command::source_projection_cli::SourceTextSearchResult::Current {
                page,
                disclosure,
                ..
            } => (
                page.render_json(profile.max_response_bytes, probe.as_ref())
                    .map_err(|error| map_qry_error(error.code))?,
                disclosure,
            ),
            tos_command::source_projection_cli::SourceTextSearchResult::Pending { .. } => {
                return Err(AccessError::new(
                    AccessErrorCode::PublicationPending,
                    "selected source-text consumer is pending",
                ));
            }
            tos_command::source_projection_cli::SourceTextSearchResult::Stale { .. } => {
                return Err(AccessError::new(
                    AccessErrorCode::StaleSelection,
                    "selected source-text consumer is stale",
                ));
            }
            tos_command::source_projection_cli::SourceTextSearchResult::Budget { .. } => {
                return Err(AccessError::new(
                    AccessErrorCode::BudgetExceeded,
                    "source-text candidate set exceeds the page budget",
                ));
            }
            tos_command::source_projection_cli::SourceTextSearchResult::Rejected {
                code, ..
            } => return Err(map_qry_error(code)),
        };
        Ok(PreparedPacket {
            body,
            fence: Box::new(SourceTextSearchFence { disclosure, probe }),
        })
    }
}

/// One-shot source-text CLI: `source-text search QUERY --limit N`, with
/// repeatable exact `--document-type` and `--source-path` filters and an
/// opaque `--cursor` continuation.
pub fn run_cli(
    args: &[String],
    selected: Option<&SelectedPgSourceTextSearch>,
    profile: AccessProfile,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let usage = "usage: tos-access [--source-text-search-selection ABS_JSON] source-text search QUERY --limit N [--document-type ID] [--source-path PATH] [--cursor TOKEN]";
    if args.len() < 3 || args[0] != "source-text" || args[1] != "search" {
        let _ = writeln!(stderr, "{usage}");
        return 2;
    }
    let mut fields = vec![(
        JsonString::from_utf8("query"),
        JsonValue::String(JsonString::from_utf8(&args[2])),
    )];
    let mut document_types = Vec::new();
    let mut source_paths = Vec::new();
    let mut limit = None;
    let mut cursor = None;
    let mut at = 3;
    while at < args.len() {
        let option = args[at].as_str();
        let Some(value) = args.get(at + 1) else {
            let _ = writeln!(stderr, "missing value for {option}");
            return 2;
        };
        match option {
            "--document-type" => {
                document_types.push(JsonValue::String(JsonString::from_utf8(value)))
            }
            "--source-path" => source_paths.push(JsonValue::String(JsonString::from_utf8(value))),
            "--limit" => match value.parse::<usize>() {
                Ok(value) if (1..=100).contains(&value) => limit = Some(value),
                _ => {
                    let _ = writeln!(stderr, "invalid value for --limit");
                    return 2;
                }
            },
            "--cursor" => cursor = Some(JsonValue::String(JsonString::from_utf8(value))),
            _ => {
                let _ = writeln!(stderr, "unsupported source-text search option: {option}");
                return 2;
            }
        }
        at += 2;
    }
    let Some(limit) = limit else {
        let _ = writeln!(stderr, "--limit is required");
        return 2;
    };
    fields.push((
        JsonString::from_utf8("limit"),
        JsonValue::Number(JsonNumber {
            kind: JsonNumberKind::Int,
            lexeme: limit.to_string(),
        }),
    ));
    fields.push((
        JsonString::from_utf8("document_types"),
        JsonValue::Array(document_types),
    ));
    fields.push((
        JsonString::from_utf8("source_paths"),
        JsonValue::Array(source_paths),
    ));
    if let Some(cursor) = cursor {
        fields.push((JsonString::from_utf8("cursor"), cursor));
    }
    let request = match SourceTextSearchRequest::from_value(&JsonValue::Object(fields)) {
        Ok(request) => request,
        Err(_) => {
            let _ = writeln!(
                stderr,
                "invalid_request: invalid source-text search request"
            );
            return 2;
        }
    };
    let Some(selected) = selected else {
        let _ = writeln!(
            stderr,
            "unavailable: no durable PostgreSQL source-text consumer selected"
        );
        return 3;
    };
    match crate::common::checked_execute(profile.deadline_probe(), |probe| {
        selected.execute_request(request, profile, probe)
    }) {
        Ok(packet) => crate::cli::write_packet(packet, profile, stdout, stderr),
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            match error.code {
                AccessErrorCode::Unavailable
                | AccessErrorCode::StaleSelection
                | AccessErrorCode::PublicationPending
                | AccessErrorCode::IndexIncomplete => 3,
                AccessErrorCode::InvalidRequest => 2,
                _ => 1,
            }
        }
    }
}

struct SourceTextSearchFence {
    disclosure: tos_command::source_projection_cli::SourceTextDisclosureGuard,
    probe: Arc<dyn AbortProbe>,
}

impl DisclosureFence for SourceTextSearchFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        check_probe(self.probe.as_ref())?;
        self.disclosure.recheck().map_err(map_durable_error)?;
        check_probe(self.probe.as_ref())
    }
}

fn check_probe(probe: &dyn AbortProbe) -> Result<(), AccessError> {
    match probe.reason() {
        None => Ok(()),
        Some(AbortReason::Cancelled) => Err(AccessError::new(
            AccessErrorCode::Cancelled,
            "source-text search cancelled",
        )),
        Some(AbortReason::DeadlineExceeded) => Err(AccessError::new(
            AccessErrorCode::DeadlineExceeded,
            "source-text search deadline exceeded",
        )),
    }
}

fn map_qry_error(code: tos_query::source_text_search::SourceTextSearchErrorCode) -> AccessError {
    use tos_query::source_text_search::SourceTextSearchErrorCode as E;
    match code {
        E::InvalidRequest => AccessError::new(
            AccessErrorCode::InvalidRequest,
            "invalid source-text search request",
        ),
        E::BudgetExceeded => AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "source-text search budget exceeded",
        ),
        E::StaleCursor => AccessError::new(
            AccessErrorCode::CursorExpired,
            "source-text continuation does not match the selected consumer",
        ),
        E::CorruptCandidate => AccessError::new(
            AccessErrorCode::CorruptSelectedCarrier,
            "selected source-text candidate is invalid",
        ),
        E::Cancelled => {
            AccessError::new(AccessErrorCode::Cancelled, "source-text search cancelled")
        }
        E::DeadlineExceeded => AccessError::new(
            AccessErrorCode::DeadlineExceeded,
            "source-text search deadline exceeded",
        ),
    }
}

fn map_durable_error(error: tos_command::DurableError) -> AccessError {
    use tos_command::DurableError as E;
    match error {
        E::Invalid(_) => AccessError::new(
            AccessErrorCode::InvalidRequest,
            "selected source-text request or selection is invalid",
        ),
        E::Conflict(_) => AccessError::new(
            AccessErrorCode::StaleSelection,
            "selected source-text projection contract changed",
        ),
        E::Corrupt(_) | E::Storage(_) => AccessError::new(
            AccessErrorCode::CorruptSelectedCarrier,
            "selected source-text projection is corrupt",
        ),
        E::Database(_) | E::Source(_) | E::Refused(_) | E::Indeterminate(_) => AccessError::new(
            AccessErrorCode::Unavailable,
            "selected source-text search unavailable",
        ),
    }
}
