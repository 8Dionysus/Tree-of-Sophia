//! Bounded source-text search over the private durable projection.
//!
//! PostgreSQL supplies only a candidate seek.  The QRY owner retains Unicode16
//! normalization, field semantics, final recheck, ranking and continuation
//! validation.  This module deliberately keeps the currentness/rights lock
//! alive until QRY has consumed the complete bounded candidate set.

use std::time::Instant;

use postgres::GenericClient;
use tos_query::source_text_search::{MAX_QUERY_RAW_UTF8_BYTES, SourceTextSearchErrorCode};
use tos_query::{AbortProbe, AbortReason};

use super::{
    DurableError, DurableProjectionDocument, DurableProjectionLimits, DurableProjectionProfile,
    DurableResult, ProjectionReadGate, as_u64, read_projection_gate, validate_identifier,
    validate_relative_path,
};

/// Consumer-local source-text normalizer/index profile.  The profile is put on
/// a consumer only when that row is created at cursor zero.  Existing rows with
/// a NULL profile must be replayed through a new consumer.
pub(crate) const SOURCE_TEXT_SEARCH_PROFILE: &str = "tos_source_text_search_v1";
pub(crate) const SOURCE_TEXT_SEARCH_PROFILE_VERSION: i32 = 1;

const MAX_SOURCE_TEXT_FILTERS: usize = 100;
const MAX_SOURCE_TEXT_PAGE_RESULTS: usize = 100;
// The shared QRY request admits at most 4096 Unicode code points and uses the
// UTF-8 worst case as its early byte guard.  Keep the durable owner at the
// same physical ceiling; a caller-configured lower owner limit is reported as
// an explicit Budget outcome before semantic normalization.
// These are the shared QRY hard ceilings.  Keep the adapter's derived budget
// below them rather than handing a large projection event to QRY only to have
// its own validator reject the whole page.
const QRY_MAX_SOURCE_TEXT_CODE_POINTS: usize = 16_000_000;
const QRY_MAX_SOURCE_TEXT_BYTES: usize = 64 * 1024 * 1024;
const QRY_MAX_SOURCE_TEXT_WORK_STEPS: u64 = 100_000_000;
// FND normalization is deliberately charged more heavily than raw UTF-8
// bytes: Unicode16 lower/case-fold work can inspect several code units per
// input byte, especially for non-ASCII text. The cap remains finite and is
// derived from the admitted candidate payload rather than the page size.
const SOURCE_TEXT_WORK_PER_BYTE: u64 = 64;
const SOURCE_TEXT_WORK_PER_DOCUMENT: u64 = 2_048;
/// Candidate admission is deliberately wider than one response page so the
/// shared QRY kernel can rank, retain `limit + 1`, and issue a continuation.
/// The projection event ceiling remains the owner-configurable lower bound;
/// this hard ceiling prevents an unbounded search read.
const MAX_SOURCE_TEXT_CANDIDATES: usize = 256;
const MAX_SOURCE_TEXT_RESPONSE_BYTES: usize = 1 << 20;
const MAX_SOURCE_TEXT_EXCERPT_CODE_POINTS: usize = 512;
const SOURCE_TEXT_SEPARATOR: &str = "\u{001f}";

/// The projection-side representation of one FND-normalized document.  The
/// raw fields stay in the projection for disclosure; these values exist only
/// to make the candidate index deterministic and explicit.
pub(crate) struct NormalizedSourceTextFields {
    pub identity: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub body: String,
    pub all: String,
}

/// A status-aware result keeps Pending/Stale distinct from an empty successful
/// page. Semantic QRY errors retain their error code instead of collapsing
/// into a generic durable refusal.
pub(crate) enum SourceTextProjectionOutcome {
    Current {
        through_seq: u64,
        rights_version: u64,
        audit_generation: u64,
        page: tos_query::source_text_search::SourceTextSearchPage,
    },
    Pending {
        through_seq: u64,
        head_seq: u64,
    },
    Stale {
        through_seq: Option<u64>,
        head_seq: u64,
        reason: &'static str,
    },
    Budget {
        through_seq: u64,
        head_seq: u64,
        reason: &'static str,
    },
    Rejected {
        through_seq: u64,
        head_seq: u64,
        code: SourceTextSearchErrorCode,
        reason: &'static str,
    },
}

#[derive(Clone, Debug)]
struct Candidate {
    source_path: String,
    document_id: String,
    source_revision: u64,
    document_type: String,
    identity_bytes: u64,
    title_bytes: u64,
    aliases_bytes: u64,
    body_bytes: u64,
}

/// Normalize one published document through the shared QRY/FND rule.  A
/// failure is a refused projection event: publishing an unsearchable partial
/// row would make the consumer falsely current.
pub(crate) fn normalize_document(
    document: &DurableProjectionDocument,
    limits: DurableProjectionLimits,
) -> DurableResult<NormalizedSourceTextFields> {
    let normalize = |value: &str, label: &'static str, input_cap: usize, output_cap: usize| {
        tos_query::source_text_search::normalize_source_text(
            value,
            input_cap,
            output_cap,
            output_cap.saturating_mul(4),
        )
        .map_err(|_| DurableError::Refused(label))
    };
    let text_cap = limits.max_document_bytes.max(1);
    let identity = normalize(
        &document.identity,
        "source-text identity normalization exceeded budget",
        text_cap,
        text_cap,
    )?;
    let title = normalize(
        &document.title,
        "source-text title normalization exceeded budget",
        text_cap,
        text_cap,
    )?;
    let mut aliases = Vec::with_capacity(document.aliases.len());
    for alias in &document.aliases {
        aliases.push(normalize(
            alias,
            "source-text alias normalization exceeded budget",
            text_cap,
            text_cap,
        )?);
    }
    let body = normalize(
        &document.body,
        "source-text body normalization exceeded budget",
        text_cap,
        text_cap,
    )?;
    let aliases_text = aliases.join(SOURCE_TEXT_SEPARATOR);
    let all_input = [
        identity.as_str(),
        title.as_str(),
        aliases_text.as_str(),
        body.as_str(),
    ]
    .join(SOURCE_TEXT_SEPARATOR);
    let all = normalize(
        &all_input,
        "source-text concatenated normalization exceeded budget",
        text_cap.saturating_mul(4),
        limits.max_document_bytes.saturating_mul(4),
    )?;
    Ok(NormalizedSourceTextFields {
        identity,
        title,
        aliases,
        body,
        all,
    })
}

/// Execute source-text search against an already-open owner transaction.
/// The caller retains that transaction after this returns `Current`, so the
/// same audit/domain/consumer locks cover candidate reads, page construction,
/// and the final disclosure flush.
pub(crate) fn search_source_text_in_transaction<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    request: tos_query::source_text_search::SourceTextSearchRequest,
    limits: DurableProjectionLimits,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> DurableResult<SourceTextProjectionOutcome> {
    limits.validate()?;
    profile.validate()?;
    validate_identifier(domain, super::MAX_DOMAIN_BYTES, "projection domain")?;
    validate_identifier(
        consumer_id,
        super::MAX_CONSUMER_ID_BYTES,
        "projection consumer ID",
    )?;
    if Instant::now() >= deadline {
        return Err(DurableError::Refused(
            "source-text search deadline exceeded",
        ));
    }
    if probe.reason().is_some() {
        return Err(DurableError::Refused("source-text search aborted"));
    }
    if request.query.contains('\0') {
        return Err(DurableError::Invalid("invalid source-text request"));
    }
    let query_byte_budget = limits.max_query_bytes.min(MAX_QUERY_RAW_UTF8_BYTES);
    if request.query.len() > query_byte_budget {
        return Ok(SourceTextProjectionOutcome::Budget {
            through_seq: 0,
            head_seq: 0,
            reason: "source-text query exceeds owner byte budget",
        });
    }
    let normalized_request = match request.normalize() {
        Ok(request) => request,
        Err(error) => return Ok(semantic_outcome(0, 0, error)),
    };
    validate_request(&request, limits)?;

    let (through_seq, rights_version, audit_generation, indexed_profile, indexed_version) =
        match read_projection_gate(tx, domain, consumer_id, profile) {
            Ok(gate) => match gate {
                ProjectionReadGate::Current {
                    through_seq,
                    rights_version,
                    audit_generation,
                    source_text_search_profile,
                    source_text_search_profile_version,
                } => (
                    through_seq,
                    rights_version,
                    audit_generation,
                    source_text_search_profile,
                    source_text_search_profile_version,
                ),
                ProjectionReadGate::Pending {
                    through_seq,
                    head_seq,
                } => {
                    return Ok(SourceTextProjectionOutcome::Pending {
                        through_seq,
                        head_seq,
                    });
                }
                ProjectionReadGate::Stale {
                    through_seq,
                    head_seq,
                    reason,
                } => {
                    return Ok(SourceTextProjectionOutcome::Stale {
                        through_seq,
                        head_seq,
                        reason,
                    });
                }
            },
            Err(error) if is_statement_timeout(&error) => {
                return Ok(SourceTextProjectionOutcome::Budget {
                    through_seq: 0,
                    head_seq: 0,
                    reason: "source-text SQL statement timeout",
                });
            }
            Err(error) => return Err(error),
        };
    let head_seq = through_seq;
    if indexed_profile.as_deref() != Some(SOURCE_TEXT_SEARCH_PROFILE)
        || indexed_version != Some(SOURCE_TEXT_SEARCH_PROFILE_VERSION)
    {
        return Ok(SourceTextProjectionOutcome::Stale {
            through_seq: Some(through_seq),
            head_seq,
            reason: "source-text index profile requires a new consumer replay",
        });
    }
    if let Some(outcome) = abort_outcome(through_seq, head_seq, deadline, probe) {
        return Ok(outcome);
    }

    let normalized_query = normalized_request.query().to_owned();
    let like_pattern = escaped_like_pattern(&normalized_query);
    let candidate_bound = limits.max_event_documents.min(MAX_SOURCE_TEXT_CANDIDATES);
    let candidate_limit = candidate_bound
        .checked_add(1)
        .ok_or(DurableError::Invalid("source-text candidate limit"))?;
    let candidate_limit = i64::try_from(candidate_limit)
        .map_err(|_| DurableError::Invalid("source-text candidate limit"))?;
    let rows = match tx.query(
        "SELECT source_path,document_id,source_revision,document_type,
                octet_length(identity)::bigint AS identity_bytes,
                octet_length(title)::bigint AS title_bytes,
                octet_length(aliases_text)::bigint AS aliases_bytes,
                octet_length(body)::bigint AS body_bytes
         FROM cmd2_projection_document
         WHERE domain=$1 AND consumer_id=$2 AND source_text_norm IS NOT NULL
           AND source_text_norm LIKE $3 ESCAPE E'\\\\'
           AND (cardinality($4::text[])=0 OR document_type = ANY($4::text[]))
           AND (cardinality($5::text[])=0 OR source_path = ANY($5::text[]))
         ORDER BY source_path COLLATE \"C\",document_id COLLATE \"C\"
         LIMIT $6",
        &[
            &domain,
            &consumer_id,
            &like_pattern,
            &normalized_request.document_types(),
            &normalized_request.source_paths(),
            &candidate_limit,
        ],
    ) {
        Ok(rows) => rows,
        Err(error) if is_statement_timeout_pg(&error) => {
            return Ok(SourceTextProjectionOutcome::Budget {
                through_seq,
                head_seq,
                reason: "source-text SQL statement timeout",
            });
        }
        Err(error) => return Err(error.into()),
    };
    let complete = rows.len() <= candidate_bound;
    if !complete {
        return Ok(SourceTextProjectionOutcome::Budget {
            through_seq,
            head_seq,
            reason: "source-text candidate set exceeds bounded candidate budget",
        });
    }
    let mut candidates = Vec::with_capacity(rows.len());
    let mut text_bytes = 0u64;
    for row in rows {
        if let Some(outcome) = abort_outcome(through_seq, head_seq, deadline, probe) {
            return Ok(outcome);
        }
        let source_path: String = row.get("source_path");
        let document_id: String = row.get("document_id");
        let source_revision = as_u64(row.get("source_revision"))?;
        let document_type: String = row.get("document_type");
        let custody_revision_bytes = source_revision.to_string().len() as u64;
        let identity_bytes = as_u64(row.get("identity_bytes"))?;
        let title_bytes = as_u64(row.get("title_bytes"))?;
        let aliases_bytes = as_u64(row.get("aliases_bytes"))?;
        let body_bytes = as_u64(row.get("body_bytes"))?;
        let row_bytes = identity_bytes
            .checked_add(title_bytes)
            .and_then(|n| n.checked_add(aliases_bytes))
            .and_then(|n| n.checked_add(body_bytes))
            .and_then(|n| n.checked_add(source_path.len() as u64))
            .and_then(|n| n.checked_add(document_id.len() as u64))
            .and_then(|n| n.checked_add(document_type.len() as u64))
            .and_then(|n| n.checked_add(custody_revision_bytes))
            .ok_or(DurableError::Refused("source-text byte budget overflow"))?;
        if row_bytes > limits.max_document_bytes as u64 {
            return Ok(SourceTextProjectionOutcome::Budget {
                through_seq,
                head_seq,
                reason: "source-text document exceeds byte budget",
            });
        }
        text_bytes = text_bytes
            .checked_add(row_bytes)
            .ok_or(DurableError::Refused("source-text byte budget overflow"))?;
        if text_bytes > limits.max_event_bytes {
            return Ok(SourceTextProjectionOutcome::Budget {
                through_seq,
                head_seq,
                reason: "source-text candidate bytes exceed budget",
            });
        }
        candidates.push(Candidate {
            source_path,
            document_id,
            source_revision,
            document_type,
            identity_bytes,
            title_bytes,
            aliases_bytes,
            body_bytes,
        });
    }

    let mut documents = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if let Some(outcome) = abort_outcome(through_seq, head_seq, deadline, probe) {
            return Ok(outcome);
        }
        let row = match tx.query_opt(
            "SELECT identity,title,aliases,body,
                    octet_length(identity)::bigint AS identity_bytes,
                    octet_length(title)::bigint AS title_bytes,
                    octet_length(aliases_text)::bigint AS aliases_bytes,
                    octet_length(body)::bigint AS body_bytes
             FROM cmd2_projection_document
                 WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND document_id=$4",
            &[
                &domain,
                &consumer_id,
                &candidate.source_path,
                &candidate.document_id,
            ],
        ) {
            Ok(row) => row,
            Err(error) if is_statement_timeout_pg(&error) => {
                return Ok(SourceTextProjectionOutcome::Budget {
                    through_seq,
                    head_seq,
                    reason: "source-text SQL statement timeout",
                });
            }
            Err(error) => return Err(error.into()),
        }
        .ok_or(DurableError::Corrupt(
            "source-text candidate body disappeared",
        ))?;
        let identity: String = row.get("identity");
        let title: String = row.get("title");
        let aliases: Vec<String> = row.get("aliases");
        let body: String = row.get("body");
        let identity_bytes = as_u64(row.get("identity_bytes"))?;
        let title_bytes = as_u64(row.get("title_bytes"))?;
        let aliases_bytes = as_u64(row.get("aliases_bytes"))?;
        let body_bytes = as_u64(row.get("body_bytes"))?;
        let aliases_value_bytes = aliases
            .iter()
            .map(|value| value.len() as u64)
            .try_fold(0u64, |total, value| total.checked_add(value))
            .and_then(|total| total.checked_add(aliases.len().saturating_sub(1) as u64))
            .ok_or(DurableError::Refused("source-text byte budget overflow"))?;
        if identity_bytes != candidate.identity_bytes
            || title_bytes != candidate.title_bytes
            || aliases_bytes != candidate.aliases_bytes
            || body_bytes != candidate.body_bytes
            || identity.len() as u64 != candidate.identity_bytes
            || title.len() as u64 != candidate.title_bytes
            || aliases_value_bytes != candidate.aliases_bytes
            || body.len() as u64 != candidate.body_bytes
        {
            return Err(DurableError::Corrupt("source-text candidate bytes changed"));
        }
        documents.push(tos_query::source_text_search::SourceTextDocument {
            document_type: candidate.document_type,
            source_path: candidate.source_path,
            custody_revision: candidate.source_revision.to_string(),
            document_id: candidate.document_id,
            selection_seq: through_seq,
            identity,
            title,
            aliases,
            body,
        });
    }

    let selection = tos_query::source_text_search::SourceTextSearchSelection {
        consumer_id: consumer_id.to_owned(),
        domain: domain.to_owned(),
        consumer_cut: through_seq.to_string(),
        profile_id: profile.id.clone(),
        profile_sha256: selection_profile_digest(profile),
        selection_seq: through_seq,
    };
    let budget = tos_query::source_text_search::SourceTextSearchBudget {
        max_candidates: candidate_bound,
        max_text_code_points: usize::try_from(text_bytes)
            .unwrap_or(usize::MAX)
            .min(QRY_MAX_SOURCE_TEXT_CODE_POINTS)
            .max(1),
        max_text_bytes: usize::try_from(text_bytes)
            .unwrap_or(usize::MAX)
            .saturating_mul(4)
            .saturating_add(1024)
            .min(QRY_MAX_SOURCE_TEXT_BYTES)
            .max(1),
        max_excerpt_code_points: MAX_SOURCE_TEXT_EXCERPT_CODE_POINTS,
        max_response_bytes: MAX_SOURCE_TEXT_RESPONSE_BYTES,
        max_work_steps: text_bytes
            .saturating_mul(SOURCE_TEXT_WORK_PER_BYTE)
            .saturating_add(
                u64::try_from(documents.len())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(SOURCE_TEXT_WORK_PER_DOCUMENT),
            )
            .min(QRY_MAX_SOURCE_TEXT_WORK_STEPS)
            .max(1),
    };
    let candidate_set = tos_query::source_text_search::SourceTextCandidateSet {
        complete,
        documents: &documents,
    };
    let page = match tos_query::source_text_search::execute_source_text_search(
        &request,
        &selection,
        candidate_set,
        budget,
        probe,
    ) {
        Ok(page) => page,
        Err(error) => return Ok(semantic_outcome(through_seq, head_seq, error)),
    };
    Ok(SourceTextProjectionOutcome::Current {
        through_seq,
        rights_version,
        audit_generation,
        page,
    })
}

fn validate_request(
    request: &tos_query::source_text_search::SourceTextSearchRequest,
    limits: DurableProjectionLimits,
) -> DurableResult<()> {
    if request.query.contains('\0')
        || !(1..=MAX_SOURCE_TEXT_PAGE_RESULTS).contains(&request.limit)
        || request.limit > limits.max_results
        || request.document_types.len() > MAX_SOURCE_TEXT_FILTERS
        || request.source_paths.len() > MAX_SOURCE_TEXT_FILTERS
    {
        return Err(DurableError::Invalid("invalid source-text request"));
    }
    for value in &request.document_types {
        validate_identifier(
            value,
            super::MAX_PROJECTION_PATH_BYTES,
            "source-text filter",
        )?;
    }
    for value in &request.source_paths {
        validate_relative_path(value)?;
    }
    if let Some(cursor) = request.cursor.as_deref() {
        validate_identifier(cursor, 64 * 1024, "source-text cursor")?;
    }
    Ok(())
}

fn semantic_outcome(
    through_seq: u64,
    head_seq: u64,
    error: tos_query::source_text_search::SourceTextSearchError,
) -> SourceTextProjectionOutcome {
    match error.code {
        SourceTextSearchErrorCode::BudgetExceeded => SourceTextProjectionOutcome::Budget {
            through_seq,
            head_seq,
            reason: error.message,
        },
        code => SourceTextProjectionOutcome::Rejected {
            through_seq,
            head_seq,
            code,
            reason: error.message,
        },
    }
}

fn abort_outcome(
    through_seq: u64,
    head_seq: u64,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> Option<SourceTextProjectionOutcome> {
    let (code, reason) = match probe.reason() {
        Some(AbortReason::Cancelled) => (
            SourceTextSearchErrorCode::Cancelled,
            "source-text search cancelled",
        ),
        Some(AbortReason::DeadlineExceeded) => (
            SourceTextSearchErrorCode::DeadlineExceeded,
            "source-text search deadline exceeded",
        ),
        None if Instant::now() >= deadline => (
            SourceTextSearchErrorCode::DeadlineExceeded,
            "source-text search deadline exceeded",
        ),
        None => return None,
    };
    Some(SourceTextProjectionOutcome::Rejected {
        through_seq,
        head_seq,
        code,
        reason,
    })
}

fn selection_profile_digest(profile: &DurableProjectionProfile) -> tos_foundation::Digest256 {
    let descriptor = format!(
        "{}\0{}\0{}\0{}\0{}",
        tos_query::source_text_search::SOURCE_TEXT_SEARCH_OPERATION,
        profile.id,
        profile.version,
        profile.source_contract.to_hex(),
        SOURCE_TEXT_SEARCH_PROFILE_VERSION,
    );
    tos_foundation::Digest256::of_bytes(descriptor.as_bytes())
}

fn is_statement_timeout(error: &DurableError) -> bool {
    matches!(
        error,
        DurableError::Database(error)
            if matches!(
                error.code(),
                Some(&postgres::error::SqlState::QUERY_CANCELED)
                    | Some(&postgres::error::SqlState::LOCK_NOT_AVAILABLE)
            )
    )
}

fn is_statement_timeout_pg(error: &postgres::Error) -> bool {
    matches!(
        error.code(),
        Some(&postgres::error::SqlState::QUERY_CANCELED)
            | Some(&postgres::error::SqlState::LOCK_NOT_AVAILABLE)
    )
}

fn escaped_like_pattern(value: &str) -> String {
    let mut pattern = String::with_capacity(value.len().saturating_add(2));
    pattern.push('%');
    for character in value.chars() {
        match character {
            '\\' => pattern.push_str("\\\\"),
            '%' => pattern.push_str("\\%"),
            '_' => pattern.push_str("\\_"),
            _ => pattern.push(character),
        }
    }
    pattern.push('%');
    pattern
}
