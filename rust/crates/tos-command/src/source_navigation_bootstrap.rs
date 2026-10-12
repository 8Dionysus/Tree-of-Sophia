//! Bounded capture of the complete selected source-navigation model for the
//! PostgreSQL projection's first current-index bootstrap.
//!
//! This only captures exact carriers from a `VerifiedKnowledgeModel` inside
//! its owner lease. It does not grant source admission, rights, or disclosure.

use std::collections::BTreeSet;
use std::time::Instant;

use rusqlite::{Connection, params};
use std::sync::atomic::AtomicBool;
use tos_compiler::{NavigationHeaderClaim, VerifiedKnowledgeModel};
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, JsonMode, RelativePath, parse_json};
use tos_segment_store::SegmentStore;

use crate::DurableNavigationBootstrapApplyV1;
use crate::durable_adapter::durable_projection::{
    DurableNavigationBootstrapOwnerSourceV1, DurableNavigationBootstrapPackageV1,
    DurableNavigationBootstrapRowKindV1, DurableNavigationBootstrapRowV1,
    DurableNavigationEventLimits, DurableNavigationOriginalRightWitnessV1,
    DurableProjectionProfile,
};
use crate::source_creation::ManagedSerializedCreation;
use crate::source_creation_store::CreationFilesystem;
use crate::source_current_cut::ManagedCurrentSourceGeneration;
use crate::source_managed_selection::{
    ManagedAgentSelectedParent, ManagedSelectedProof, ManagedSelectionError, ManagedSelectionResult,
};
use crate::{DurableError, DurablePgCoordinator, DurableResult};

const MAX_BOOTSTRAP_ROWS: usize = 65_536;
const MAX_BOOTSTRAP_ROW_BYTES: usize = 4 * 1024 * 1024;
const MAX_BOOTSTRAP_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const MAX_BOOTSTRAP_PATH_BYTES: usize = 4096;
const MAX_BOOTSTRAP_ID_BYTES: usize = 256;

/// Captured exact selected carriers. The caller obtains this value only within
/// `ManagedAgentSelectedParent::with_current_model`, while its current hold is
/// live, then resolves member metadata and seals the package before publish.
pub struct SourceNavigationBootstrapCaptureV1 {
    header: NavigationHeaderClaim,
    rows: Vec<DurableNavigationBootstrapRowV1>,
    /// Exact original rights bytes in authenticated ordinal order. Projected
    /// rights rows are separate carriers and cannot prove this receipt root.
    original_rights: Vec<(u64, Vec<u8>)>,
    owner_source_paths: BTreeSet<String>,
}

impl SourceNavigationBootstrapCaptureV1 {
    /// Internal owner/dependency paths that must be checked against the same
    /// selected managed generation before the capture can be sealed.
    pub fn owner_source_paths(&self) -> &BTreeSet<String> {
        &self.owner_source_paths
    }
}

/// Copy the original detached header and complete selected node/edge/rights
/// tables under the verified model pin. All SQL projections are length-gated
/// before allocating carrier bytes; any omitted, malformed, or oversized row
/// fails closed instead of producing a partial baseline.
pub fn capture_source_navigation_bootstrap_v1(
    model: &VerifiedKnowledgeModel<'_>,
    limits: DurableNavigationEventLimits,
) -> DurableResult<SourceNavigationBootstrapCaptureV1> {
    let limits = checked_limits(limits)?;
    require_source_navigation_profile(&model.selection().model_abi, model.selection_profile())?;
    model
        .check_pin()
        .map_err(|_| DurableError::Corrupt("selected navigation model pin"))?;
    let receipt = model
        .navigation_original_receipt()
        .map_err(|_| DurableError::Corrupt("selected navigation receipt"))?;
    let page = model
        .navigation_original_page_under_caller_budget(
            None,
            1,
            limits.max_row_bytes,
            limits.max_total_bytes,
        )
        .map_err(|_| DurableError::Corrupt("selected navigation header"))?;
    if page.rows.len() != 1 || page.rows[0].0 != -1 {
        return Err(DurableError::Corrupt("selected navigation header row"));
    }
    let header_raw = page.rows.into_iter().next().unwrap().1;
    let expected_header = receipt.header_sha256.clone();
    let header_digest = Digest256::of_bytes(&header_raw);
    if Digest256::from_hex(&expected_header).ok() != Some(header_digest) {
        return Err(DurableError::Conflict(
            "selected navigation header digest differs",
        ));
    }
    let header = NavigationHeaderClaim {
        raw_json: header_raw,
        expected_sha256: expected_header,
    };

    let rights_budget = limits
        .max_total_bytes
        .checked_sub(header.raw_json.len() as u64)
        .ok_or(DurableError::Refused("navigation bootstrap byte budget"))?;
    let original_rights = capture_original_rights(model, receipt.rights, limits, rights_budget)?;
    let rights_refs = original_rights
        .iter()
        .map(|(_, raw)| raw.as_slice())
        .collect::<Vec<_>>();
    if tos_compiler::navigation_original_rights_root(&rights_refs) != receipt.rights_root_sha256 {
        return Err(DurableError::Conflict(
            "selected original rights root differs",
        ));
    }
    let mut rows = Vec::new();
    let mut owner_source_paths = BTreeSet::new();
    let mut charged_bytes = header.raw_json.len() as u64;
    for (_, raw) in &original_rights {
        charged_bytes = charged_bytes
            .checked_add(raw.len() as u64)
            .ok_or(DurableError::Refused("navigation bootstrap byte budget"))?;
    }
    if charged_bytes > limits.max_total_bytes {
        return Err(DurableError::Refused("navigation bootstrap byte budget"));
    }
    let mut source_row_count = 0u64;
    for (kind, table, key, expected_count, visibility) in [
        (
            DurableNavigationBootstrapRowKindV1::Node,
            "nodes",
            "node_id",
            Some(receipt.nodes),
            true,
        ),
        (
            DurableNavigationBootstrapRowKindV1::Edge,
            "edges",
            "edge_id",
            Some(receipt.edges),
            true,
        ),
        (
            DurableNavigationBootstrapRowKindV1::Right,
            "rights",
            "rights_id",
            None,
            false,
        ),
    ] {
        let observed = capture_carrier_table(
            model.connection(),
            table,
            key,
            kind,
            visibility,
            limits,
            &mut rows,
            &mut owner_source_paths,
            &mut charged_bytes,
            &mut source_row_count,
        )?;
        if expected_count.is_some_and(|expected| observed != expected) {
            return Err(DurableError::Conflict(
                "selected navigation carrier count differs from receipt",
            ));
        }
    }
    if rows.is_empty() || owner_source_paths.is_empty() {
        return Err(DurableError::Conflict(
            "selected navigation owner rows absent",
        ));
    }
    if navigation_input_root(&rows, DurableNavigationBootstrapRowKindV1::Node)?
        != Digest256::from_hex(&receipt.node_input_root_sha256)
            .map_err(|_| DurableError::Corrupt("selected navigation node root"))?
        || navigation_input_root(&rows, DurableNavigationBootstrapRowKindV1::Edge)?
            != Digest256::from_hex(&receipt.edge_input_root_sha256)
                .map_err(|_| DurableError::Corrupt("selected navigation edge root"))?
    {
        return Err(DurableError::Conflict(
            "selected navigation input roots differ",
        ));
    }
    model
        .check_pin()
        .map_err(|_| DurableError::Corrupt("selected navigation model pin changed"))?;
    Ok(SourceNavigationBootstrapCaptureV1 {
        header,
        rows,
        original_rights,
        owner_source_paths,
    })
}

/// Resolve every internal row/dependency path against the selected managed
/// generation. This provides exact digest/size observations for the sealed
/// helper package; it does not read payload bytes or issue disclosure rights.
pub fn resolve_source_navigation_bootstrap_owners_v1(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    generation: &ManagedCurrentSourceGeneration,
    paths: &BTreeSet<String>,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Vec<DurableNavigationBootstrapOwnerSourceV1>> {
    let limits = checked_limits(limits)?;
    if paths.is_empty() || paths.len() > limits.max_dependencies {
        return Err(DurableError::Refused(
            "navigation bootstrap owner source budget",
        ));
    }
    let mut owners = Vec::with_capacity(paths.len());
    for raw_path in paths {
        let path = checked_path(raw_path)?;
        let member = generation
            .member(coordinator, store, &path, deadline, cancelled)
            .map_err(DurableError::Source)?
            .ok_or(DurableError::Conflict(
                "navigation bootstrap source path is outside selected generation",
            ))?;
        owners.push(DurableNavigationBootstrapOwnerSourceV1 {
            source_path: path.into_string(),
            sha256: member.sha256,
            size_bytes: member.size_bytes,
        });
    }
    Ok(owners)
}

/// Seal a captured baseline only after its every referenced source member has
/// been resolved in the same managed selected generation.
pub fn seal_source_navigation_bootstrap_v1<P: ManagedSelectedProof>(
    domain: &str,
    selected: &ManagedAgentSelectedParent<P>,
    profile: &DurableProjectionProfile,
    capture: SourceNavigationBootstrapCaptureV1,
    owner_sources: Vec<DurableNavigationBootstrapOwnerSourceV1>,
    limits: DurableNavigationEventLimits,
) -> DurableResult<DurableNavigationBootstrapPackageV1> {
    require_source_navigation_profile(
        &selected.selection_expectation().model_abi,
        selected.selection_profile(),
    )?;
    DurableNavigationBootstrapPackageV1::checked_from_selected(
        domain,
        selected,
        profile,
        &capture.header,
        capture.rows,
        capture
            .original_rights
            .into_iter()
            .map(|(ordinal, raw)| DurableNavigationOriginalRightWitnessV1 { ordinal, raw })
            .collect(),
        owner_sources,
        limits,
    )
}

/// Capture the complete selected source-navigation baseline under the real
/// managed-model hold, resolve its exact source-member witnesses, then publish
/// only if the CMD head and managed source binding still match that capture.
/// This is a private projection bootstrap; it does not authorize a query or
/// disclose carriers.
#[allow(clippy::too_many_arguments)]
pub fn publish_current_source_navigation_bootstrap_v1<P: ManagedSelectedProof>(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    filesystem: &CreationFilesystem,
    package: &ManagedSerializedCreation,
    selected: &ManagedAgentSelectedParent<P>,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ManagedSelectionResult<DurableNavigationBootstrapApplyV1> {
    let capture = selected.with_current_model(
        coordinator,
        store,
        filesystem,
        package,
        contract,
        rule_version,
        rights_version,
        job_id,
        job_fence,
        deadline,
        cancelled,
        |model, lease| {
            lease
                .require_managed_basis(selected.source_proof())
                .map_err(ManagedSelectionError::Source)?;
            capture_source_navigation_bootstrap_v1(model, limits)
                .map_err(ManagedSelectionError::Durable)
        },
    )?;
    let owner_sources = resolve_source_navigation_bootstrap_owners_v1(
        coordinator,
        store,
        selected.generation(),
        capture.owner_source_paths(),
        limits,
        deadline,
        cancelled,
    )
    .map_err(ManagedSelectionError::Durable)?;
    let sealed = seal_source_navigation_bootstrap_v1(
        domain,
        selected,
        profile,
        capture,
        owner_sources,
        limits,
    )
    .map_err(ManagedSelectionError::Durable)?;
    coordinator
        .publish_navigation_bootstrap_v1(
            store,
            domain,
            consumer_id,
            profile,
            selected,
            &sealed,
            limits,
        )
        .map_err(ManagedSelectionError::Durable)
}

fn capture_original_rights(
    model: &VerifiedKnowledgeModel<'_>,
    expected_count: u64,
    limits: DurableNavigationEventLimits,
    max_total_bytes: u64,
) -> DurableResult<Vec<(u64, Vec<u8>)>> {
    if expected_count > limits.max_current_rows as u64 {
        return Err(DurableError::Refused("selected original rights row budget"));
    }
    if expected_count == 0 {
        return Ok(Vec::new());
    }
    let mut rights = Vec::with_capacity(expected_count as usize);
    let mut after = Some(-1);
    let mut bytes = 0u64;
    loop {
        let remaining = max_total_bytes
            .checked_sub(bytes)
            .ok_or(DurableError::Refused(
                "selected original rights byte budget",
            ))?;
        if remaining == 0 {
            return Err(DurableError::Refused(
                "selected original rights byte budget",
            ));
        }
        let page_byte_budget = remaining.min(limits.max_row_bytes as u64);
        let page = model
            .navigation_original_page_under_caller_budget(
                after,
                256.min(limits.max_current_rows),
                limits.max_row_bytes.min(page_byte_budget as usize),
                page_byte_budget,
            )
            .map_err(|_| DurableError::Corrupt("selected original rights page"))?;
        if page.rows.is_empty() {
            break;
        }
        for (ordinal, raw) in &page.rows {
            if *ordinal < 0 || *ordinal as u64 != rights.len() as u64 {
                return Err(DurableError::Corrupt("selected original rights ordinal"));
            }
            bytes = bytes
                .checked_add(raw.len() as u64)
                .ok_or(DurableError::Refused(
                    "selected original rights byte budget",
                ))?;
            if raw.len() > limits.max_row_bytes || bytes > max_total_bytes {
                return Err(DurableError::Refused(
                    "selected original rights byte budget",
                ));
            }
            rights.push((*ordinal as u64, raw.clone()));
            if rights.len() > expected_count as usize {
                return Err(DurableError::Corrupt("selected original rights count"));
            }
        }
        after = page.rows.last().map(|(ordinal, _)| *ordinal);
        if page.next_ordinal.is_none() {
            break;
        }
    }
    if rights.len() as u64 != expected_count {
        return Err(DurableError::Conflict(
            "selected original rights count differs",
        ));
    }
    Ok(rights)
}

fn navigation_input_root(
    rows: &[DurableNavigationBootstrapRowV1],
    kind: DurableNavigationBootstrapRowKindV1,
) -> DurableResult<Digest256> {
    let mut carriers = std::collections::BTreeMap::<String, Digest256>::new();
    for row in rows.iter().filter(|row| row.kind == kind) {
        let digest = Digest256::of_bytes(&row.raw);
        if carriers
            .insert(row.id.clone(), digest)
            .is_some_and(|previous| previous != digest)
        {
            return Err(DurableError::Conflict(
                "shared navigation carrier input differs",
            ));
        }
    }
    let mut hasher = Digest256Hasher::new();
    for (id, digest) in carriers {
        hasher.update(&(id.len() as u64).to_be_bytes());
        hasher.update(id.as_bytes());
        hasher.update(digest.as_bytes());
    }
    Ok(hasher.finalize())
}

fn checked_limits(
    limits: DurableNavigationEventLimits,
) -> DurableResult<DurableNavigationEventLimits> {
    if limits.max_current_rows == 0
        || limits.max_current_rows > MAX_BOOTSTRAP_ROWS
        || limits.max_dependencies == 0
        || limits.max_row_bytes == 0
        || limits.max_row_bytes > MAX_BOOTSTRAP_ROW_BYTES
        || limits.max_total_bytes == 0
        || limits.max_total_bytes > MAX_BOOTSTRAP_TOTAL_BYTES
    {
        return Err(DurableError::Refused("navigation bootstrap capture limits"));
    }
    Ok(limits)
}

fn require_source_navigation_profile(
    model_abi: &str,
    selection_profile: &str,
) -> DurableResult<()> {
    if model_abi != tos_compiler::KNOWLEDGE_MANAGED_MODEL_ABI
        || selection_profile != tos_query::SOURCE_NAVIGATION_SELECTION_PROFILE
    {
        return Err(DurableError::Refused(
            "selected navigation model profile unsupported",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn capture_carrier_table(
    db: &Connection,
    table: &str,
    key: &str,
    kind: DurableNavigationBootstrapRowKindV1,
    has_visibility: bool,
    limits: DurableNavigationEventLimits,
    output: &mut Vec<DurableNavigationBootstrapRowV1>,
    owner_source_paths: &mut BTreeSet<String>,
    charged_bytes: &mut u64,
    source_row_count: &mut u64,
) -> DurableResult<u64> {
    let visibility = if has_visibility { "visible" } else { "NULL" };
    let sql = format!(
        "SELECT CASE WHEN length(CAST({key} AS BLOB)) BETWEEN 1 AND ?1 THEN {key} END,
                {visibility},
                CASE WHEN carrier_size BETWEEN 0 AND ?2
                           AND length(carrier)<=?2 THEN carrier END,
                CASE WHEN length(CAST(carrier_sha256 AS BLOB))=64 THEN carrier_sha256 END
         FROM {table} ORDER BY {key} LIMIT ?3"
    );
    let read_cap = limits
        .max_current_rows
        .checked_add(1)
        .ok_or(DurableError::Refused("navigation bootstrap row count"))?;
    let read_cap = i64::try_from(read_cap)
        .map_err(|_| DurableError::Refused("navigation bootstrap row count"))?;
    let row_cap = i64::try_from(limits.max_row_bytes)
        .map_err(|_| DurableError::Refused("navigation bootstrap row bytes"))?;
    let mut statement = db
        .prepare(&sql)
        .map_err(|_| DurableError::Corrupt("selected navigation table"))?;
    let mut cursor = statement
        .query(params![MAX_BOOTSTRAP_ID_BYTES as i64, row_cap, read_cap])
        .map_err(|_| DurableError::Corrupt("selected navigation table query"))?;
    let mut observed = 0u64;
    while let Some(row) = cursor
        .next()
        .map_err(|_| DurableError::Corrupt("selected navigation table row"))?
    {
        observed = observed
            .checked_add(1)
            .ok_or(DurableError::Refused("navigation bootstrap row count"))?;
        if observed as usize > limits.max_current_rows
            || *source_row_count as usize >= limits.max_current_rows
        {
            return Err(DurableError::Refused("navigation bootstrap row budget"));
        }
        *source_row_count += 1;
        let id: Option<String> = row
            .get(0)
            .map_err(|_| DurableError::Corrupt("selected navigation carrier ID"))?;
        let id = id.ok_or(DurableError::Corrupt("selected navigation carrier ID size"))?;
        if id.len() > MAX_BOOTSTRAP_ID_BYTES {
            return Err(DurableError::Refused(
                "navigation bootstrap carrier ID size",
            ));
        }
        let visible_raw: Option<i64> = row
            .get(1)
            .map_err(|_| DurableError::Corrupt("selected navigation visibility"))?;
        let visible = if has_visibility {
            match visible_raw {
                Some(0) => Some(false),
                Some(1) => Some(true),
                _ => return Err(DurableError::Corrupt("selected navigation visibility")),
            }
        } else {
            None
        };
        let raw: Option<Vec<u8>> = row
            .get(2)
            .map_err(|_| DurableError::Corrupt("selected navigation carrier"))?;
        let raw = raw.ok_or(DurableError::Refused(
            "selected navigation carrier byte cap",
        ))?;
        let sha: Option<String> = row
            .get(3)
            .map_err(|_| DurableError::Corrupt("selected navigation carrier digest"))?;
        let sha = sha.ok_or(DurableError::Corrupt(
            "selected navigation carrier digest size",
        ))?;
        if Digest256::from_hex(&sha).ok() != Some(Digest256::of_bytes(&raw)) {
            return Err(DurableError::Corrupt("selected navigation carrier digest"));
        }
        let value = parse_json(
            &raw,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: limits.max_row_bytes,
                max_depth: 64,
                max_visits: 300_000,
                max_integer_digits: 4_300,
            },
        )
        .map_err(|_| DurableError::Corrupt("selected navigation carrier JSON"))?;
        let id_field = match kind {
            DurableNavigationBootstrapRowKindV1::Node => "node_id",
            DurableNavigationBootstrapRowKindV1::Edge => "edge_id",
            DurableNavigationBootstrapRowKindV1::Right => "rights_id",
            DurableNavigationBootstrapRowKindV1::Diagnostic => {
                return Err(DurableError::Invalid("unexpected diagnostic carrier table"));
            }
        };
        if value.get(id_field).and_then(|value| value.as_str()) != Some(id.as_str()) {
            return Err(DurableError::Corrupt("selected navigation carrier key"));
        }
        let mut refs = BTreeSet::new();
        let mut explicit_owner = None;
        if let Some(path) = value.get("source_ref").and_then(|value| value.as_str()) {
            let path = checked_path(path)?.into_string();
            explicit_owner = Some(path.clone());
            refs.insert(path);
        }
        if let Some(path) = value.get("path").and_then(|value| value.as_str()) {
            let path = checked_path(path)?.into_string();
            explicit_owner.get_or_insert_with(|| path.clone());
            refs.insert(path);
        }
        if let Some(paths) = value.get("source_refs").and_then(|value| value.as_array()) {
            for path in paths {
                let path = path
                    .as_str()
                    .ok_or(DurableError::Corrupt("selected navigation source ref"))?;
                if has_uri_scheme(path) {
                    continue;
                }
                if let Ok(path) = RelativePath::parse(path) {
                    refs.insert(path.into_string());
                } else {
                    return Err(DurableError::Corrupt("selected navigation source ref"));
                }
            }
        }
        if refs.is_empty() {
            return Err(DurableError::Conflict(
                "selected navigation row has no source owner",
            ));
        }
        let owner_record_id = value
            .get("record_id")
            .and_then(|value| value.as_str())
            .or_else(|| {
                value
                    .pointer("/record/record_id")
                    .and_then(|value| value.as_str())
            })
            .or_else(|| {
                value
                    .pointer("/source_record/record_id")
                    .and_then(|value| value.as_str())
            })
            .filter(|record_id| !record_id.is_empty() && record_id.len() <= MAX_BOOTSTRAP_ID_BYTES)
            .unwrap_or(&id)
            .to_owned();
        // `source_refs` are dependency evidence and may include citations;
        // they are not row ownership. A carrier without an explicit producer
        // path is assigned only when it has exactly one internal dependency.
        let owner_path = match explicit_owner {
            Some(owner_path) => owner_path,
            None if refs.len() == 1 => refs.iter().next().unwrap().clone(),
            None => {
                return Err(DurableError::Conflict(
                    "selected navigation row ownership is ambiguous",
                ));
            }
        };
        owner_source_paths.extend(refs.iter().cloned());
        if owner_source_paths.len() > limits.max_dependencies {
            return Err(DurableError::Refused(
                "navigation bootstrap dependency source budget",
            ));
        }
        {
            *charged_bytes = charged_bytes
                .checked_add(raw.len() as u64)
                .ok_or(DurableError::Refused("navigation bootstrap byte budget"))?;
            if *charged_bytes > limits.max_total_bytes {
                return Err(DurableError::Refused("navigation bootstrap byte budget"));
            }
            if output.len() >= limits.max_current_rows {
                return Err(DurableError::Refused("navigation bootstrap row budget"));
            }
            output.push(DurableNavigationBootstrapRowV1 {
                kind,
                id: id.clone(),
                owner_source_path: owner_path,
                owner_record_id: owner_record_id.clone(),
                visible,
                raw: raw.clone(),
            });
        }
    }
    Ok(observed)
}

fn checked_path(value: &str) -> DurableResult<RelativePath> {
    if value.len() > MAX_BOOTSTRAP_PATH_BYTES {
        return Err(DurableError::Refused(
            "navigation bootstrap source path size",
        ));
    }
    RelativePath::parse(value)
        .map_err(|_| DurableError::Invalid("navigation bootstrap source path"))
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic() || (index > 0 && b"+-.").contains(&byte)
        })
}
