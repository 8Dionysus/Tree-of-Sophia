//! Owner-selected private PostgreSQL projection. This is a data operation;
//! software installation neither creates a source domain nor selects its data.

use crate::{
    DurableError, DurablePgCoordinator, DurableProjectionDocument, DurableProjectionExtractor,
    DurableProjectionLimits, DurableProjectionProfile, DurableProjectionSource, DurableResult,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, RelativePath};
use tos_query::{AbortProbe, AbortReason};
use tos_segment_store::{SegmentLimits, SegmentStore};

pub const HELP: &str = r#"usage: tos-native-owner-command source-projection --selection ABS_JSON < REQUEST_JSON
Private connection: TOS_SOURCE_PG_URL, PostgreSQL 16, numeric TCP loopback or a canonical absolute Unix-socket directory.
Selection for text projections: tos_source_projection_selection_v1; existing
domain, exact source contract digest, consumer, private STO root and store limits.
Navigation requests use a protected tos_local_source_navigation_owner_v1 config
with the retained V2 model/proof, selected consumer/profile and explicit limits.
Requests: init, apply, bootstrap-text, bootstrap-navigation, advance-navigation, status, queue-age, search, search-text, exact-source. Init installs derived tables only.
Queue-age reports one selected consumer's oldest unprocessed sequence and persisted server-clock age; legacy timestamps remain unknown.
Apply resumes the durable outbox cursor; search uses selected text token matching.
Navigation uses the protected source-navigation owner selection and its explicit resource limits.
Bootstrap request: {"operation":"bootstrap-navigation","max_batches":256}; max_batches is optional, 1..256.
Result: tos_source_navigation_bootstrap_result_v2; repeat the same request until completed and cleanup_complete.
Advance request: {"operation":"advance-navigation","max_events":N}; unsupported events remain pending.
Profiles: tos_source_selected_text_v1 (direct fields),
tos_source_selected_text_v2 (also bound Agent source-copy HumanForms).
Source admission, external rights and public activation remain owner operations.
These profiles do not replace the full knowledge API or export readers.
"#;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    schema_version: String,
    domain: String,
    source_contract_sha256: String,
    #[serde(default)]
    consumer: Option<String>,
    #[serde(default)]
    consumer_id: Option<String>,
    profile: String,
    store_root: String,
    store_limits: StoreLimits,
    #[serde(default)]
    bootstrap: Option<TextBootstrapSelection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextBootstrapSelection {
    max_source_work_bytes: u64,
    max_call_work_bytes: u64,
    max_json_visits: usize,
    max_json_state_bytes: usize,
    timeout_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreLimits {
    max_segment_bytes: u64,
    max_frame_bytes: u64,
    max_frames: u32,
    max_journal_bytes: usize,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    Init,
    Status,
    QueueAge,
    BootstrapNavigation {
        #[serde(default = "default_initial_batches")]
        max_batches: usize,
    },
    BootstrapText {
        max_sources: usize,
    },
    AdvanceNavigation {
        max_events: usize,
    },
    Apply {
        max_events: usize,
    },
    Search {
        query: String,
        limit: usize,
    },
    SearchText {
        query: String,
        limit: usize,
        #[serde(default)]
        document_types: Vec<String>,
        #[serde(default)]
        source_paths: Vec<String>,
        #[serde(default)]
        cursor: Option<String>,
    },
    ExactSource {
        source_path: String,
        revision: u64,
    },
}

fn default_initial_batches() -> usize {
    256
}

impl Selection {
    fn selected_consumer(&self) -> Result<&str, &'static str> {
        if let (Some(legacy), Some(selected)) =
            (self.consumer.as_deref(), self.consumer_id.as_deref())
            && legacy != selected
        {
            return Err("projection consumer selectors differ");
        }
        let consumer = self
            .consumer_id
            .as_deref()
            .or(self.consumer.as_deref())
            .ok_or("projection consumer_id required")?;
        if consumer.is_empty() || consumer.len() > 256 {
            return Err("unsupported projection consumer_id");
        }
        Ok(consumer)
    }
}

/// Typed result returned by the selected PG source-text owner.  The caller
/// may render only `Current`; every other state remains explicit, including
/// semantic QRY rejection codes.
pub enum SourceTextSearchResult {
    Current {
        through_seq: u64,
        page: tos_query::source_text_search::SourceTextSearchPage,
        disclosure: SourceTextDisclosureGuard,
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
        code: tos_query::source_text_search::SourceTextSearchErrorCode,
        reason: &'static str,
    },
}

/// Owner-held PostgreSQL disclosure fence for one current source-text page.
///
/// The guard owns a live transaction whose `FOR SHARE` locks cover the domain,
/// audit fence and selected consumer.  A source rights revoke or head advance
/// therefore waits for the final transport recheck. Candidate reads and this
/// guard use the same transaction, so there is no unlocked hand-off window;
/// the captured cut still rejects a revoke/regrant that changed the binding.
/// The recheck delegates to the shared projection gate, whose observed-state
/// fields cover rule, schema, contract and source-generation changes without
/// maintaining a second raw-column binding snapshot here.
pub struct SourceTextDisclosureGuard {
    client: Option<postgres::Client>,
    domain: String,
    consumer_id: String,
    profile: DurableProjectionProfile,
    through_seq: u64,
    rights_version: u64,
    audit_generation: u64,
}

impl SourceTextDisclosureGuard {
    /// Recheck the exact currentness and rights binding immediately before the
    /// transport flush. The transaction remains open after this call.
    pub fn recheck(&mut self) -> DurableResult<()> {
        let client = self
            .client
            .as_mut()
            .ok_or(DurableError::Refused("source-text disclosure guard closed"))?;
        crate::durable_adapter::durable_projection::recheck_source_text_disclosure(
            client,
            &self.domain,
            &self.consumer_id,
            &self.profile,
            crate::durable_adapter::durable_projection::ProjectionDisclosureCut {
                through_seq: self.through_seq,
                rights_version: self.rights_version,
                audit_generation: self.audit_generation,
            },
        )
    }
}

impl Drop for SourceTextDisclosureGuard {
    fn drop(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.batch_execute("ROLLBACK");
        }
    }
}

struct CliAbortProbe<'a> {
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl AbortProbe for CliAbortProbe<'_> {
    fn reason(&self) -> Option<AbortReason> {
        if self.cancelled.load(Ordering::Relaxed) {
            Some(AbortReason::Cancelled)
        } else if Instant::now() >= self.deadline {
            Some(AbortReason::DeadlineExceeded)
        } else {
            None
        }
    }
}

/// Pure declared source fields; the worker supplies their exact source locator.
pub struct SourceTextExtractor;

impl DurableProjectionExtractor for SourceTextExtractor {
    fn extract(
        &self,
        source_path: &str,
        _revision: u64,
        exact_source_bytes: &[u8],
    ) -> DurableResult<Vec<DurableProjectionDocument>> {
        crate::source_search_projection::project_source_text(source_path, exact_source_bytes)
            .map(projection_documents)
            .map_err(crate::DurableError::Invalid)
    }
}

/// The explicit v2 profile also resolves current Agent source-copy forms from
/// the exact sibling metadata supplied by the worker at the same event cut.
pub struct SourceTextExtractorV2;

impl DurableProjectionExtractor for SourceTextExtractorV2 {
    fn extract(
        &self,
        source_path: &str,
        _revision: u64,
        exact_source_bytes: &[u8],
    ) -> DurableResult<Vec<DurableProjectionDocument>> {
        crate::source_search_projection::project_source_text_v2(
            source_path,
            exact_source_bytes,
            None,
        )
        .map(projection_documents)
        .map_err(crate::DurableError::Invalid)
    }

    fn affected_source_paths(&self, changed_path: &str) -> DurableResult<Vec<String>> {
        Ok(
            crate::source_search_projection::agent_human_forms_path_for_metadata(changed_path)
                .into_iter()
                .collect(),
        )
    }

    fn required_source_paths(
        &self,
        root_path: &str,
        _revision: u64,
        _exact_source_bytes: &[u8],
    ) -> DurableResult<Vec<String>> {
        Ok(
            crate::source_search_projection::agent_metadata_path_for_human_forms(root_path)
                .into_iter()
                .collect(),
        )
    }

    fn extract_with_sources(
        &self,
        source_path: &str,
        _revision: u64,
        exact_source_bytes: &[u8],
        selected_inputs: &[DurableProjectionSource],
    ) -> DurableResult<Vec<DurableProjectionDocument>> {
        let expected =
            crate::source_search_projection::agent_metadata_path_for_human_forms(source_path);
        if selected_inputs.len() > 1
            || selected_inputs.iter().any(|input| {
                Some(input.source_path.as_str()) != expected.as_deref() || input.revision == 0
            })
        {
            return Err(crate::DurableError::Invalid(
                "source-copy projection selected sibling differs",
            ));
        }
        let sibling = selected_inputs.first().map(|input| {
            (
                input.source_path.as_str(),
                input.exact_source_bytes.as_slice(),
            )
        });
        crate::source_search_projection::project_source_text_v2(
            source_path,
            exact_source_bytes,
            sibling,
        )
        .map(projection_documents)
        .map_err(crate::DurableError::Invalid)
    }
}

struct InitialSourceTextExtractor {
    version: u32,
    parse: crate::source_search_projection::SourceSearchParseLimits,
}

impl DurableProjectionExtractor for InitialSourceTextExtractor {
    fn extract(
        &self,
        path: &str,
        revision: u64,
        raw: &[u8],
    ) -> DurableResult<Vec<DurableProjectionDocument>> {
        self.extract_with_sources(path, revision, raw, &[])
    }

    fn required_source_paths(
        &self,
        path: &str,
        revision: u64,
        raw: &[u8],
    ) -> DurableResult<Vec<String>> {
        if self.version == 2 {
            SourceTextExtractorV2.required_source_paths(path, revision, raw)
        } else {
            Ok(Vec::new())
        }
    }

    fn extract_with_sources(
        &self,
        path: &str,
        _revision: u64,
        raw: &[u8],
        inputs: &[DurableProjectionSource],
    ) -> DurableResult<Vec<DurableProjectionDocument>> {
        let result = if self.version == 2 {
            let expected =
                crate::source_search_projection::agent_metadata_path_for_human_forms(path);
            if inputs.len() > 1
                || inputs.iter().any(|input| {
                    Some(input.source_path.as_str()) != expected.as_deref() || input.revision == 0
                })
            {
                return Err(DurableError::Invalid(
                    "source-copy projection selected sibling differs",
                ));
            }
            crate::source_search_projection::project_source_text_v2_with_parse_limits(
                path,
                raw,
                inputs.first().map(|input| {
                    (
                        input.source_path.as_str(),
                        input.exact_source_bytes.as_slice(),
                    )
                }),
                self.parse,
            )
        } else {
            if !inputs.is_empty() {
                return Err(DurableError::Invalid(
                    "v1 initial projection has companion inputs",
                ));
            }
            crate::source_search_projection::project_source_text_with_parse_limits(
                path, raw, self.parse,
            )
        };
        result
            .map(projection_documents)
            .map_err(DurableError::Invalid)
    }
}

fn projection_documents(
    documents: Vec<crate::source_search_projection::SourceSearchDocument>,
) -> Vec<DurableProjectionDocument> {
    documents
        .into_iter()
        .map(|document| DurableProjectionDocument {
            document_id: document.id.clone(),
            identity: document.id,
            document_type: document.kind,
            title: document.title,
            body: document.body,
            aliases: document.aliases,
            dependency_paths: document.dependencies,
        })
        .collect()
}

fn read_request_bytes(input: impl Read) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    input
        .take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| "request read failed")?;
    if bytes.len() > 65_536 {
        return Err("request too large");
    }
    Ok(bytes)
}

fn read_request(input: impl Read) -> Result<Request, &'static str> {
    let bytes = read_request_bytes(input)?;
    serde_json::from_slice(&bytes).map_err(|_| "invalid projection request")
}

/// Value-returning compatibility API for non-disclosing projection operations.
/// Search-text is intentionally refused here because a returned `Value` cannot
/// retain its disclosure transaction through the caller's final flush.
pub fn run(selection_path: &Path, input: impl Read) -> Result<Value, String> {
    let cancelled = AtomicBool::new(false);
    let selection_path = crate::source_text_owner::normalized_absolute(
        selection_path
            .to_str()
            .ok_or("selection path must be UTF-8")?,
    )
    .map_err(|_| "selection path must be absolute")?;
    let request = read_request(input)?;
    if let Request::AdvanceNavigation { max_events } = &request {
        if !(1..=256).contains(max_events) {
            return Err("max_events must be 1..256".into());
        }
        return crate::source_navigation_producer::run(&selection_path, *max_events, &cancelled);
    }
    if let Request::BootstrapNavigation { max_batches } = &request {
        if !(1..=256).contains(max_batches) {
            return Err("max_batches must be 1..256".into());
        }
        return crate::source_navigation_producer::run_bootstrap(
            &selection_path,
            *max_batches,
            &cancelled,
        );
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let selection_bytes = crate::source_text_owner::read_absolute(
        &selection_path,
        rustix::process::geteuid().as_raw(),
        true,
        65_536,
        deadline,
        &cancelled,
    )
    .map_err(|_| "private selection is absent, unsafe or changed")?;
    let selection: Selection =
        serde_json::from_slice(&selection_bytes).map_err(|_| "invalid projection selection")?;
    let version = match selection.profile.as_str() {
        crate::source_search_projection::SOURCE_SEARCH_PROFILE => 1,
        crate::source_search_projection::SOURCE_SEARCH_PROFILE_V2 => 2,
        _ => return Err("unsupported projection profile".into()),
    };
    if selection.schema_version != "tos_source_projection_selection_v1"
        || selection.domain.is_empty()
        || selection.domain.len() > 1024
    {
        return Err("unsupported projection selection".into());
    }
    let consumer_id = selection.selected_consumer()?;
    let source_contract = Digest256::from_hex(&selection.source_contract_sha256)
        .map_err(|_| "invalid source contract digest")?;
    let profile = DurableProjectionProfile {
        id: selection.profile.clone(),
        version,
        source_contract,
    };
    let client = connect_selected_source_pg16()?;
    let mut db = DurablePgCoordinator::from_configured_client(client);
    match request {
        Request::Init => {
            db.init_projection_schema()
                .map_err(|_| "projection schema initialization refused")?;
            Ok(
                json!({"schema_version":"tos_source_projection_result_v1", "operation":"init", "profile":profile.id, "initialized":true}),
            )
        }
        Request::Search { query, limit } => {
            let result = db
                .search_projection(
                    &selection.domain,
                    consumer_id,
                    &profile,
                    &query,
                    limit,
                    Duration::from_secs(15),
                )
                .map_err(|_| "projection search refused")?;
            Ok(
                json!({"schema_version":"tos_source_projection_search_v1", "profile":profile.id, "result":result}),
            )
        }
        Request::SearchText { .. } => {
            Err("source-text search requires the streaming writer route".into())
        }
        Request::Status => {
            let result = db
                .projection_status(
                    &selection.domain,
                    consumer_id,
                    &profile,
                    Duration::from_secs(15),
                )
                .map_err(|_| "projection status refused")?;
            Ok(
                json!({"schema_version":"tos_source_projection_status_v1", "profile":profile.id, "result":result}),
            )
        }
        Request::QueueAge => {
            let result = db
                .projection_queue_age(
                    &selection.domain,
                    consumer_id,
                    &profile,
                    Duration::from_secs(15),
                )
                .map_err(|_| "projection queue-age status refused")?;
            Ok(json!({
                "schema_version":"tos_source_projection_queue_age_v1",
                "profile":profile.id,
                "result":result
            }))
        }
        Request::AdvanceNavigation { .. } => {
            Err("advance-navigation requires its protected source-navigation route".into())
        }
        Request::BootstrapNavigation { .. } => {
            Err("bootstrap-navigation requires its protected source-navigation route".into())
        }
        Request::BootstrapText { max_sources } => {
            let initial = selection
                .bootstrap
                .as_ref()
                .ok_or("bootstrap-text requires explicit initial resource selection")?;
            if !(1..=256).contains(&max_sources)
                || initial.max_source_work_bytes == 0
                || initial.max_source_work_bytes > 512 * 1024 * 1024
                || initial.max_call_work_bytes < initial.max_source_work_bytes
                || initial.max_call_work_bytes > 4 * 1024 * 1024 * 1024
                || !(1..=300_000).contains(&initial.timeout_ms)
            {
                return Err("invalid initial text resource selection".into());
            }
            let parse = crate::source_search_projection::SourceSearchParseLimits {
                max_json_visits: initial.max_json_visits,
                max_json_state_bytes: initial.max_json_state_bytes,
            }
            .validate()?;
            let store = open_store(&selection)?;
            let result = db
                .bootstrap_projection_sources(
                    &store,
                    &selection.domain,
                    consumer_id,
                    &profile,
                    &InitialSourceTextExtractor { version, parse },
                    crate::DurableProjectionBootstrapLimits {
                        max_sources,
                        max_call_bytes: initial.max_call_work_bytes,
                        source: DurableProjectionLimits {
                            max_event_bytes: initial.max_source_work_bytes,
                            max_dependencies: if version == 2 { 2 } else { 1 },
                            statement_timeout: Duration::from_millis(initial.timeout_ms),
                            ..Default::default()
                        },
                    },
                )
                .map_err(|_| "initial text bootstrap refused; committed progress retained")?;
            Ok(
                json!({"schema_version":"tos_source_text_bootstrap_progress_v1", "profile":profile.id, "result":result}),
            )
        }
        Request::Apply { max_events } => {
            if !(1..=256).contains(&max_events) {
                return Err("max_events must be 1..256".into());
            }
            let store = open_store(&selection)?;
            let limits = DurableProjectionLimits {
                max_events,
                max_dependencies: if version == 2 { 2 } else { 1 },
                ..Default::default()
            };
            let extractor: &dyn DurableProjectionExtractor = if version == 2 {
                &SourceTextExtractorV2
            } else {
                &SourceTextExtractor
            };
            let result = db
                .apply_projection_batch(
                    &store,
                    &selection.domain,
                    consumer_id,
                    &profile,
                    extractor,
                    limits,
                )
                .map_err(|_| "projection update refused; committed progress retained")?;
            Ok(
                json!({"schema_version":"tos_source_projection_progress_v1", "profile":profile.id, "result":result}),
            )
        }
        Request::ExactSource {
            source_path,
            revision,
        } => {
            RelativePath::parse(&source_path).map_err(|_| "invalid source path")?;
            if revision == 0 {
                return Err("source revision must be positive".into());
            }
            let store = open_store(&selection)?;
            let (_, bytes, _, work) = db
                .read_committed_source_selected(
                    &store,
                    &selection.domain,
                    &source_path,
                    Some(revision),
                    1_048_576,
                    deadline,
                    &cancelled,
                    |row| {
                        if row.get::<_, String>("contract_digest") != source_contract.to_hex() {
                            return Err(crate::DurableError::Conflict(
                                "selected exact source contract differs",
                            ));
                        }
                        Ok(())
                    },
                )
                .map_err(|_| "exact source read refused")?;
            Ok(json!({
                "schema_version":"tos_source_projection_exact_v1",
                "domain":selection.domain, "source_path":source_path, "revision":revision,
                "bytes":bytes.len(), "sha256":Digest256::of_bytes(&bytes).to_hex(),
                "source_read_work":{
                    "custody_metadata_bytes_read":work.custody_metadata_bytes_read,
                    "segment_bytes_read":work.segment_bytes_read,
                    "committed_member_rows_examined":work.committed_member_rows_examined
                },
                "content_base64":base64::engine::general_purpose::STANDARD.encode(&bytes)
            }))
        }
    }
}

/// Native owner entry point. Search-text uses this writer route so its
/// disclosure guard remains alive through page serialization, the actual
/// write, flush, and a final binding check. The legacy `run` value API refuses
/// that operation instead of returning an unfenced page.
pub fn run_to_writer(
    selection_path: &Path,
    input: impl Read,
    output: &mut impl Write,
) -> Result<(), String> {
    let request_bytes = read_request_bytes(input)?;
    let request: Request =
        serde_json::from_slice(&request_bytes).map_err(|_| "invalid projection request")?;
    if matches!(&request, Request::SearchText { .. }) {
        let Request::SearchText {
            query,
            limit,
            document_types,
            source_paths,
            cursor,
        } = request
        else {
            unreachable!("request variant changed after search-text match");
        };
        return run_source_text_search_to_writer(
            selection_path,
            tos_query::source_text_search::SourceTextSearchRequest {
                query,
                limit,
                document_types,
                source_paths,
                cursor,
            },
            output,
        )
        .map_err(String::from);
    }
    let value = run(selection_path, Cursor::new(request_bytes))?;
    write_projection_value(output, &value).map_err(String::from)
}

fn run_source_text_search_to_writer(
    selection_path: &Path,
    request: tos_query::source_text_search::SourceTextSearchRequest,
    output: &mut impl Write,
) -> Result<(), &'static str> {
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let probe = CliAbortProbe {
        deadline,
        cancelled: &cancelled,
    };
    let result = run_source_text_search(selection_path, request, deadline, &probe)
        .map_err(|_| "source-text search refused")?;
    match result {
        SourceTextSearchResult::Current {
            through_seq,
            page,
            mut disclosure,
        } => {
            let page_bytes = page
                .render_json(1 << 20, &probe)
                .map_err(|_| "invalid source-text page")?;
            let page: Value =
                serde_json::from_slice(&page_bytes).map_err(|_| "invalid source-text page")?;
            let profile_id = page
                .get("selection")
                .and_then(|selection| selection.get("profile_id"))
                .and_then(Value::as_str)
                .unwrap_or("source-text");
            let value = json!({
                "schema_version":"tos_source_text_search_v1",
                "operation":"search-text",
                "status":"current",
                "profile":profile_id,
                "through_seq":through_seq,
                "result":page
            });
            write_fenced_projection_value(output, &value, &mut disclosure, &probe)
        }
        SourceTextSearchResult::Pending {
            through_seq,
            head_seq,
        } => write_projection_value(
            output,
            &json!({
                "schema_version":"tos_source_text_search_v1",
                "operation":"search-text",
                "status":"pending",
                "through_seq":through_seq,
                "head_seq":head_seq
            }),
        ),
        SourceTextSearchResult::Stale {
            through_seq,
            head_seq,
            reason,
        } => write_projection_value(
            output,
            &json!({
                "schema_version":"tos_source_text_search_v1",
                "operation":"search-text",
                "status":"stale",
                "through_seq":through_seq,
                "head_seq":head_seq,
                "reason":reason
            }),
        ),
        SourceTextSearchResult::Budget {
            through_seq,
            head_seq,
            reason,
        } => write_projection_value(
            output,
            &json!({
                "schema_version":"tos_source_text_search_v1",
                "operation":"search-text",
                "status":"budget",
                "through_seq":through_seq,
                "head_seq":head_seq,
                "reason":reason
            }),
        ),
        SourceTextSearchResult::Rejected {
            through_seq,
            head_seq,
            code,
            reason,
        } => write_projection_value(
            output,
            &json!({
                "schema_version":"tos_source_text_search_v1",
                "operation":"search-text",
                "status":"error",
                "through_seq":through_seq,
                "head_seq":head_seq,
                "error_code":code.as_str(),
                "reason":reason
            }),
        ),
    }
}

fn write_projection_value(output: &mut impl Write, value: &Value) -> Result<(), &'static str> {
    serde_json::to_writer(&mut *output, value).map_err(|_| "projection output write failed")?;
    output
        .write_all(b"\n")
        .map_err(|_| "projection output write failed")?;
    output.flush().map_err(|_| "projection output flush failed")
}

fn write_fenced_projection_value(
    output: &mut impl Write,
    value: &Value,
    disclosure: &mut SourceTextDisclosureGuard,
    probe: &dyn AbortProbe,
) -> Result<(), &'static str> {
    let bytes = serde_json::to_vec(value).map_err(|_| "projection output encode failed")?;
    check_cli_probe(probe)?;
    disclosure
        .recheck()
        .map_err(|_| "source-text disclosure binding changed")?;
    check_cli_probe(probe)?;
    output
        .write_all(&bytes)
        .map_err(|_| "projection output write failed")?;
    output
        .write_all(b"\n")
        .map_err(|_| "projection output write failed")?;
    check_cli_probe(probe)?;
    disclosure
        .recheck()
        .map_err(|_| "source-text disclosure binding changed")?;
    check_cli_probe(probe)?;
    output
        .flush()
        .map_err(|_| "projection output flush failed")?;
    disclosure
        .recheck()
        .map_err(|_| "source-text disclosure binding changed")
}

fn check_cli_probe(probe: &dyn AbortProbe) -> Result<(), &'static str> {
    match probe.reason() {
        None => Ok(()),
        Some(AbortReason::Cancelled) => Err("source-text search cancelled"),
        Some(AbortReason::DeadlineExceeded) => Err("source-text search deadline exceeded"),
    }
}

/// Typed source-text access for native owners.  The caller supplies the
/// protected selection path, already parsed QRY request, original deadline and
/// cooperative abort probe.  Currentness/rights and the PG transaction remain
/// inside the durable adapter; a Current result carries the typed page while
/// Pending/Stale/Budget stay explicit for the caller to map.
pub fn run_source_text_search(
    selection_path: &Path,
    request: tos_query::source_text_search::SourceTextSearchRequest,
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> DurableResult<SourceTextSearchResult> {
    check_source_text_access(deadline, probe)?;
    let selection_path = crate::source_text_owner::normalized_absolute(
        selection_path
            .to_str()
            .ok_or(DurableError::Invalid("selection path must be UTF-8"))?,
    )
    .map_err(DurableError::Source)?;
    let cancelled = AtomicBool::new(false);
    let selection_bytes = crate::source_text_owner::read_absolute(
        &selection_path,
        rustix::process::geteuid().as_raw(),
        true,
        65_536,
        deadline,
        &cancelled,
    )
    .map_err(DurableError::Source)?;
    check_source_text_access(deadline, probe)?;
    let selection: Selection = serde_json::from_slice(&selection_bytes)
        .map_err(|_| DurableError::Invalid("invalid projection selection"))?;
    let version = match selection.profile.as_str() {
        crate::source_search_projection::SOURCE_SEARCH_PROFILE => 1,
        crate::source_search_projection::SOURCE_SEARCH_PROFILE_V2 => 2,
        _ => return Err(DurableError::Invalid("unsupported projection profile")),
    };
    if selection.schema_version != "tos_source_projection_selection_v1"
        || selection.domain.is_empty()
        || selection.domain.len() > 1024
    {
        return Err(DurableError::Invalid("unsupported projection selection"));
    }
    let consumer_id = selection
        .selected_consumer()
        .map_err(DurableError::Invalid)?
        .to_owned();
    let domain = selection.domain.clone();
    let source_contract = Digest256::from_hex(&selection.source_contract_sha256)
        .map_err(|_| DurableError::Invalid("invalid source contract digest"))?;
    let profile = DurableProjectionProfile {
        id: selection.profile,
        version,
        source_contract,
    };
    let mut client = connect_selected_source_pg16_until(deadline, probe)?;
    client.batch_execute("BEGIN")?;
    // Candidate reads, QRY page construction, and disclosure transport all
    // share this one owner-held transaction.  In particular, there is no
    // unlocked client hand-off between the source-text query and the final
    // rights/currentness recheck.
    let outcome = (|| -> DurableResult<_> {
        check_source_text_access(deadline, probe)?;
        let remaining =
            deadline
                .checked_duration_since(Instant::now())
                .ok_or(DurableError::Refused(
                    "source-text search deadline exceeded",
                ))?;
        let statement_ms = remaining.as_millis().clamp(1, 15_000);
        let lock_ms = statement_ms.min(5_000);
        client.batch_execute(&format!(
            "SET LOCAL statement_timeout='{}ms'; \
             SET LOCAL lock_timeout='{}ms'; \
             SET LOCAL idle_in_transaction_session_timeout='0ms'",
            statement_ms, lock_ms
        ))?;
        let mut limits = DurableProjectionLimits::default();
        limits.max_query_bytes = tos_query::source_text_search::MAX_QUERY_RAW_UTF8_BYTES;
        limits.max_results = 100;
        limits.statement_timeout = remaining.min(Duration::from_secs(15));
        crate::durable_adapter::durable_projection::durable_source_text_search::search_source_text_in_transaction(
            &mut client,
            &domain,
            &consumer_id,
            &profile,
            request,
            limits,
            deadline,
            probe,
        )
    })();
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = client.batch_execute("ROLLBACK");
            return Err(error);
        }
    };
    Ok(match outcome {
        crate::durable_adapter::durable_projection::durable_source_text_search::SourceTextProjectionOutcome::Current {
            through_seq,
            rights_version,
            audit_generation,
            page,
        } => {
            let mut disclosure = SourceTextDisclosureGuard {
                client: Some(client),
                domain: domain.clone(),
                consumer_id: consumer_id.clone(),
                profile: profile.clone(),
                through_seq,
                rights_version,
                audit_generation,
            };
            if let Err(error) = disclosure.recheck() {
                if is_source_text_sql_timeout(&error) {
                    drop(disclosure);
                    let abort = probe.reason().or_else(|| {
                        (Instant::now() >= deadline).then_some(AbortReason::DeadlineExceeded)
                    });
                    return Ok(match abort {
                        Some(AbortReason::Cancelled) => SourceTextSearchResult::Rejected {
                            through_seq,
                            head_seq: through_seq,
                            code: tos_query::source_text_search::SourceTextSearchErrorCode::
                                Cancelled,
                            reason: "source-text search cancelled",
                        },
                        Some(AbortReason::DeadlineExceeded) => SourceTextSearchResult::Rejected {
                            through_seq,
                            head_seq: through_seq,
                            code: tos_query::source_text_search::SourceTextSearchErrorCode::
                                DeadlineExceeded,
                            reason: "source-text search deadline exceeded",
                        },
                        None => SourceTextSearchResult::Budget {
                            through_seq,
                            head_seq: through_seq,
                            reason: "source-text SQL statement timeout",
                        },
                    });
                }
                return Err(error);
            }
            SourceTextSearchResult::Current {
                through_seq,
                page,
                disclosure,
            }
        }
        crate::durable_adapter::durable_projection::durable_source_text_search::SourceTextProjectionOutcome::Pending {
            through_seq,
            head_seq,
        } => {
            let _ = client.batch_execute("ROLLBACK");
            SourceTextSearchResult::Pending { through_seq, head_seq }
        }
        crate::durable_adapter::durable_projection::durable_source_text_search::SourceTextProjectionOutcome::Stale {
            through_seq,
            head_seq,
            reason,
        } => {
            let _ = client.batch_execute("ROLLBACK");
            SourceTextSearchResult::Stale { through_seq, head_seq, reason }
        }
        crate::durable_adapter::durable_projection::durable_source_text_search::SourceTextProjectionOutcome::Budget {
            through_seq,
            head_seq,
            reason,
        } => {
            let _ = client.batch_execute("ROLLBACK");
            SourceTextSearchResult::Budget { through_seq, head_seq, reason }
        }
        crate::durable_adapter::durable_projection::durable_source_text_search::SourceTextProjectionOutcome::Rejected {
            through_seq,
            head_seq,
            code,
            reason,
        } => {
            let _ = client.batch_execute("ROLLBACK");
            SourceTextSearchResult::Rejected { through_seq, head_seq, code, reason }
        }
    })
}

fn check_source_text_access(deadline: Instant, probe: &dyn AbortProbe) -> DurableResult<()> {
    if Instant::now() >= deadline {
        return Err(DurableError::Refused(
            "source-text search deadline exceeded",
        ));
    }
    if let Some(reason) = probe.reason() {
        return Err(match reason {
            AbortReason::Cancelled => DurableError::Refused("source-text search aborted"),
            AbortReason::DeadlineExceeded => {
                DurableError::Refused("source-text search deadline exceeded")
            }
        });
    }
    Ok(())
}

fn is_source_text_sql_timeout(error: &DurableError) -> bool {
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

/// Open the exact private PostgreSQL transport shared by source projection and
/// native source revision owners. Credentials remain in the process environment
/// and connection or SQL errors never include them in the public reason.
pub(crate) fn connect_selected_source_pg16() -> Result<postgres::Client, &'static str> {
    let url = std::env::var("TOS_SOURCE_PG_URL").map_err(|_| "TOS_SOURCE_PG_URL required")?;
    let mut config = crate::backup_recovery::selected_config(&url)
        .map_err(|_| "explicit local PostgreSQL selection required")?;
    config.connect_timeout(Duration::from_secs(5));
    let mut client = config
        .connect(postgres::NoTls)
        .map_err(|_| "selected PostgreSQL unavailable")?;
    client
        .batch_execute("SET statement_timeout='15s'; SET lock_timeout='5s'")
        .map_err(|_| "PostgreSQL session configuration refused")?;
    let version: i32 = client
        .query_one("SELECT current_setting('server_version_num')::integer", &[])
        .map_err(|_| "PostgreSQL version unavailable")?
        .get(0);
    if version / 10_000 != 16 {
        return Err("selected source coordinator requires PostgreSQL 16");
    }
    Ok(client)
}

fn connect_selected_source_pg16_until(
    deadline: Instant,
    probe: &dyn AbortProbe,
) -> DurableResult<postgres::Client> {
    check_source_text_access(deadline, probe)?;
    let remaining =
        deadline
            .checked_duration_since(Instant::now())
            .ok_or(DurableError::Refused(
                "source-text search deadline exceeded",
            ))?;
    let connect_timeout = remaining.min(Duration::from_secs(5));
    let url = std::env::var("TOS_SOURCE_PG_URL")
        .map_err(|_| DurableError::Refused("TOS_SOURCE_PG_URL required"))?;
    let mut config = crate::backup_recovery::selected_config(&url)
        .map_err(|_| DurableError::Refused("explicit local PostgreSQL selection required"))?;
    config.connect_timeout(connect_timeout);
    let mut client = config
        .connect(postgres::NoTls)
        .map_err(|_| DurableError::Refused("selected PostgreSQL unavailable"))?;
    check_source_text_access(deadline, probe)?;
    let remaining =
        deadline
            .checked_duration_since(Instant::now())
            .ok_or(DurableError::Refused(
                "source-text search deadline exceeded",
            ))?;
    let statement_ms = remaining.as_millis().clamp(1, 15_000);
    let lock_ms = statement_ms.min(5_000);
    let session_sql = format!(
        "SET statement_timeout='{}ms'; SET lock_timeout='{}ms'",
        statement_ms, lock_ms
    );
    client
        .batch_execute(&session_sql)
        .map_err(|_| DurableError::Refused("PostgreSQL session configuration refused"))?;
    check_source_text_access(deadline, probe)?;
    let version: i32 = client
        .query_one("SELECT current_setting('server_version_num')::integer", &[])
        .map_err(|_| DurableError::Refused("PostgreSQL version unavailable"))?
        .get(0);
    if version / 10_000 != 16 {
        return Err(DurableError::Refused(
            "selected source coordinator requires PostgreSQL 16",
        ));
    }
    check_source_text_access(deadline, probe)?;
    Ok(client)
}

fn open_store(selection: &Selection) -> Result<SegmentStore, &'static str> {
    let root = crate::source_text_owner::normalized_absolute(&selection.store_root)
        .map_err(|_| "invalid private store root")?;
    SegmentStore::open_existing(
        &root,
        SegmentLimits {
            max_segment_bytes: selection.store_limits.max_segment_bytes,
            max_frame_bytes: selection.store_limits.max_frame_bytes,
            max_frames: selection.store_limits.max_frames,
            max_journal_bytes: selection.store_limits.max_journal_bytes,
        },
    )
    .map_err(|_| "selected private store refused")
}
