//! Owner-selected private PostgreSQL projection. This is a data operation;
//! software installation neither creates a source domain nor selects its data.

use crate::{
    DurablePgCoordinator, DurableProjectionDocument, DurableProjectionExtractor,
    DurableProjectionLimits, DurableProjectionProfile, DurableProjectionSource, DurableResult,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, RelativePath};
use tos_segment_store::{SegmentLimits, SegmentStore};

pub const HELP: &str =
    "usage: tos-native-owner-command source-projection --selection ABS_JSON < REQUEST_JSON
Private connection: TOS_SOURCE_PG_URL, PostgreSQL 16, numeric TCP loopback.
Selection: tos_source_projection_selection_v1; existing domain, exact source
contract digest, consumer, private STO root and explicit store limits.
Requests: init, apply, status, search, exact-source. Init installs derived tables only.
Apply resumes the durable outbox cursor; search uses selected text token matching.
Profiles: tos_source_selected_text_v1 (direct fields),
tos_source_selected_text_v2 (also bound Agent source-copy HumanForms).
Source admission, external rights and public activation remain owner operations.
These profiles do not replace the full knowledge API or export readers.
";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    schema_version: String,
    domain: String,
    source_contract_sha256: String,
    consumer: String,
    profile: String,
    store_root: String,
    store_limits: StoreLimits,
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
    Apply { max_events: usize },
    Search { query: String, limit: usize },
    ExactSource { source_path: String, revision: u64 },
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

fn read_request(input: impl Read) -> Result<Request, &'static str> {
    let mut bytes = Vec::new();
    input
        .take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| "request read failed")?;
    if bytes.len() > 65_536 {
        return Err("request too large");
    }
    serde_json::from_slice(&bytes).map_err(|_| "invalid projection request")
}

pub fn run(selection_path: &Path, input: impl Read) -> Result<Value, &'static str> {
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let selection_path = crate::source_text_owner::normalized_absolute(
        selection_path
            .to_str()
            .ok_or("selection path must be UTF-8")?,
    )
    .map_err(|_| "selection path must be absolute")?;
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
        _ => return Err("unsupported projection profile"),
    };
    if selection.schema_version != "tos_source_projection_selection_v1"
        || selection.domain.is_empty()
        || selection.domain.len() > 1024
        || selection.consumer.is_empty()
        || selection.consumer.len() > 256
    {
        return Err("unsupported projection selection");
    }
    let source_contract = Digest256::from_hex(&selection.source_contract_sha256)
        .map_err(|_| "invalid source contract digest")?;
    let profile = DurableProjectionProfile {
        id: selection.profile.clone(),
        version,
        source_contract,
    };
    let request = read_request(input)?;
    let mut client = connect_selected_source_pg16()?;
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
                    &selection.consumer,
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
        Request::Status => {
            let result = db
                .projection_status(
                    &selection.domain,
                    &selection.consumer,
                    &profile,
                    Duration::from_secs(15),
                )
                .map_err(|_| "projection status refused")?;
            Ok(
                json!({"schema_version":"tos_source_projection_status_v1", "profile":profile.id, "result":result}),
            )
        }
        Request::Apply { max_events } => {
            if !(1..=256).contains(&max_events) {
                return Err("max_events must be 1..256");
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
                    &selection.consumer,
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
                return Err("source revision must be positive");
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
