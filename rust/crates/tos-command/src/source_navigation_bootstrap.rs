//! Bounded capture of the complete selected source-navigation model for the
//! PostgreSQL projection's first current-index bootstrap.
//!
//! This only captures exact carriers from a `VerifiedKnowledgeModel` inside
//! its owner lease. It does not grant source admission, rights, or disclosure.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

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

/// Copy the original detached header and authenticated original node/edge/rights
/// members under the verified model pin. All SQL projections are length-gated
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
    // The complete capture budget spans many pages. This request contains only
    // the header and must fit the original-row reader's single-page contract.
    let header_budget = limits.max_total_bytes.min(limits.max_row_bytes as u64);
    let page = model
        .navigation_original_page_under_caller_budget(
            None,
            1,
            header_budget as usize,
            header_budget,
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
    let mut work_bytes = 0u64;
    let mut node_visibility = BTreeMap::new();
    for (kind, collection, expected_count) in [
        (
            DurableNavigationBootstrapRowKindV1::Node,
            "nodes",
            receipt.nodes,
        ),
        (
            DurableNavigationBootstrapRowKindV1::Edge,
            "edges",
            receipt.edges,
        ),
    ] {
        let mut after = None;
        let mut observed = 0u64;
        loop {
            let page = model
                .navigation_original_members_under_caller_budget(
                    collection,
                    after.as_deref(),
                    1,
                    limits.max_row_bytes as u64,
                )
                .map_err(|_| DurableError::Corrupt("selected navigation original membership"))?;
            let Some(member) = page.rows.into_iter().next() else {
                break;
            };
            if after.as_ref().is_some_and(|id| member.id <= *id)
                || member.collection != collection
                || observed >= expected_count
            {
                return Err(DurableError::Corrupt(
                    "selected navigation original member order/count",
                ));
            }
            let original =
                tos_compiler::managed_agent_producer::managed_navigation_original_member_bytes(
                    model,
                    collection,
                    &member.id,
                    tos_compiler::NavigationOriginalLimits {
                        max_rows: limits.max_current_rows as u64,
                        max_row_bytes: limits.max_row_bytes,
                        max_total_bytes: limits.max_total_bytes.saturating_sub(work_bytes),
                    },
                )
                .map_err(|_| DurableError::Corrupt("selected navigation exact original carrier"))?
                .ok_or(DurableError::Corrupt(
                    "selected navigation original member absent",
                ))?;
            work_bytes = work_bytes
                .checked_add(original.work_bytes)
                .filter(|bytes| *bytes <= limits.max_total_bytes)
                .ok_or(DurableError::Refused(
                    "navigation bootstrap original work budget",
                ))?;
            capture_original_carrier(
                kind,
                member.id.clone(),
                original.raw,
                limits,
                &mut rows,
                &mut owner_source_paths,
                &mut charged_bytes,
                &mut node_visibility,
            )?;
            observed += 1;
            after = Some(member.id);
        }
        if observed != expected_count {
            return Err(DurableError::Conflict(
                "selected navigation carrier count differs from receipt",
            ));
        }
    }
    // Original rights retain ordinal duplicates for their receipt. The current
    // projection uses one identical carrier per rights ID, as the source reader does.
    for (id, raw) in projected_original_rights(&original_rights, limits)? {
        capture_original_carrier(
            DurableNavigationBootstrapRowKindV1::Right,
            id,
            raw,
            limits,
            &mut rows,
            &mut owner_source_paths,
            &mut charged_bytes,
            &mut node_visibility,
        )?;
    }
    bind_historical_record_owners(&mut rows, limits)?;
    expand_claim_contributions(&mut rows, limits, &mut charged_bytes)?;
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

pub(crate) fn capture_original_rights(
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
                1,
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

pub(crate) fn projected_original_rights(
    original_rights: &[(u64, Vec<u8>)],
    limits: DurableNavigationEventLimits,
) -> DurableResult<BTreeMap<String, Vec<u8>>> {
    let mut projected = BTreeMap::<String, Vec<u8>>::new();
    for (_, raw) in original_rights {
        let value = parse_carrier(raw, limits)?;
        let id = value
            .object_get("rights_id")
            .and_then(|v| v.as_str())
            .filter(|id| !id.is_empty() && id.len() <= MAX_BOOTSTRAP_ID_BYTES)
            .ok_or(DurableError::Corrupt("selected original rights identity"))?
            .to_owned();
        if projected
            .insert(id, raw.clone())
            .is_some_and(|prior| prior != *raw)
        {
            return Err(DurableError::Conflict(
                "selected original rights ID bytes differ",
            ));
        }
    }
    Ok(projected)
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
    if !tos_foundation::is_managed_knowledge_model_abi(model_abi)
        || selection_profile != tos_query::SOURCE_NAVIGATION_SELECTION_PROFILE
    {
        return Err(DurableError::Refused(
            "selected navigation model profile unsupported",
        ));
    }
    Ok(())
}

fn parse_carrier(
    raw: &[u8],
    limits: DurableNavigationEventLimits,
) -> DurableResult<tos_foundation::JsonValue> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: limits.max_row_bytes,
            max_depth: 64,
            max_visits: 300_000,
            max_integer_digits: 4_300,
        },
    )
    .map(|document| document.into_root())
    .map_err(|_| DurableError::Corrupt("selected navigation carrier JSON"))
}

#[allow(clippy::too_many_arguments)]
fn capture_original_carrier(
    kind: DurableNavigationBootstrapRowKindV1,
    id: String,
    raw: Vec<u8>,
    limits: DurableNavigationEventLimits,
    output: &mut Vec<DurableNavigationBootstrapRowV1>,
    owner_source_paths: &mut BTreeSet<String>,
    charged_bytes: &mut u64,
    node_visibility: &mut BTreeMap<String, bool>,
) -> DurableResult<()> {
    if id.is_empty() || id.len() > MAX_BOOTSTRAP_ID_BYTES {
        return Err(DurableError::Refused(
            "navigation bootstrap carrier ID size",
        ));
    }
    let value = parse_carrier(&raw, limits)?;
    let visible = match kind {
        DurableNavigationBootstrapRowKindV1::Node => {
            let visible = tos_compiler::visible_node(&value);
            if node_visibility.insert(id.clone(), visible).is_some() {
                return Err(DurableError::Corrupt("duplicate original navigation node"));
            }
            Some(visible)
        }
        DurableNavigationBootstrapRowKindV1::Edge => {
            let endpoint = |field| {
                value
                    .object_get(field)
                    .and_then(|v| v.as_str())
                    .and_then(|id| node_visibility.get(id))
                    .copied()
                    .ok_or(DurableError::Corrupt(
                        "navigation original edge endpoint absent",
                    ))
            };
            let from_visible = endpoint("from_id")?;
            let to_visible = endpoint("to_id")?;
            Some(from_visible && to_visible)
        }
        _ => None,
    };
    let id_field = match kind {
        DurableNavigationBootstrapRowKindV1::Node => "node_id",
        DurableNavigationBootstrapRowKindV1::Edge => "edge_id",
        DurableNavigationBootstrapRowKindV1::Right => "rights_id",
        DurableNavigationBootstrapRowKindV1::Diagnostic => {
            return Err(DurableError::Invalid("unexpected diagnostic carrier table"));
        }
    };
    if value.object_get(id_field).and_then(|value| value.as_str()) != Some(id.as_str()) {
        return Err(DurableError::Corrupt("selected navigation carrier key"));
    }
    let mut refs = BTreeSet::new();
    let mut explicit_owner = None;
    if let Some(path) = value
        .object_get("source_ref")
        .and_then(|value| value.as_str())
    {
        let path = checked_path(path)?.into_string();
        explicit_owner = Some(path.clone());
        refs.insert(path);
    }
    if let Some(path) = value.object_get("path").and_then(|value| value.as_str()) {
        let path = checked_path(path)?.into_string();
        explicit_owner.get_or_insert_with(|| path.clone());
        refs.insert(path);
    }
    let source_refs = value
        .object_get("source_refs")
        .map(|value| {
            value
                .as_array()
                .ok_or(DurableError::Corrupt("selected navigation source refs"))
        })
        .transpose()?
        .unwrap_or(&[]);
    let bindings = value
        .object_get("source_ref_bindings")
        .map(|value| {
            value
                .as_object()
                .ok_or(DurableError::Corrupt("selected navigation source bindings"))
        })
        .transpose()?
        .unwrap_or(&[]);
    let bindings = checked_source_reference_bindings(
        bindings
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
        |reference| source_refs.iter().any(|v| v.as_str() == Some(reference)),
    )?;
    {
        let paths = source_refs;
        for path in paths {
            let path = path
                .as_str()
                .ok_or(DurableError::Corrupt("selected navigation source ref"))?;
            if has_uri_scheme(path) {
                continue;
            }
            let source_path = bindings.get(path).map(String::as_str).unwrap_or(path);
            if let Ok(path) = checked_path(source_path) {
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
    let historical_record_id = value
        .object_get("properties")
        .and_then(|properties| properties.object_get("record_version_view"))
        .and_then(|view| view.object_get("record_ref"))
        .and_then(|reference| reference.object_get("id"))
        .and_then(|id| id.as_str());
    let historical_edge_owner = (value.object_get("edge_kind").and_then(|v| v.as_str())
        == Some("exact_historical_record_reference"))
    .then(|| value.object_get("from_id").and_then(|v| v.as_str()))
    .flatten();
    let owner_record_id = value
        .object_get("record_id")
        .and_then(|value| value.as_str())
        .or_else(|| {
            value
                .object_get("record")
                .and_then(|record| record.object_get("record_id"))
                .and_then(|value| value.as_str())
        })
        .or_else(|| {
            value
                .object_get("source_record")
                .and_then(|record| record.object_get("record_id"))
                .and_then(|value| value.as_str())
        })
        .or(historical_record_id)
        .or(historical_edge_owner)
        .filter(|record_id| !record_id.is_empty() && record_id.len() <= MAX_BOOTSTRAP_ID_BYTES)
        .unwrap_or(&id)
        .to_owned();
    // `source_refs` are dependency evidence and may include citations;
    // they are not row ownership. A carrier without an explicit producer
    // path is assigned only when it has exactly one internal dependency.
    // A historical reference belongs to its current record node; the archive
    // is an additional dependency, not an alternative owner of that edge.
    if explicit_owner.is_none() {
        if let Some(record_id) = historical_edge_owner {
            let record = output
                .iter()
                .find(|row| {
                    row.kind == DurableNavigationBootstrapRowKindV1::Node
                        && row.id == record_id
                        && row.owner_record_id == record_id
                })
                .ok_or(DurableError::Corrupt(
                    "navigation history source owner absent",
                ))?;
            if !refs.contains(&record.owner_source_path) {
                return Err(DurableError::Corrupt(
                    "navigation history source owner differs",
                ));
            }
            explicit_owner = Some(record.owner_source_path.clone());
        }
    }
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
            raw,
        });
    }

    Ok(())
}

/// A version node keeps its exact archived source reference, but its view also
/// contains the current record/history provenance. It must therefore be
/// replaced with that record. Resolve ownership after all original nodes are
/// captured: historical node IDs may sort before the current record ID.
fn bind_historical_record_owners(
    rows: &mut [DurableNavigationBootstrapRowV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let current_owners = rows
        .iter()
        .filter(|row| {
            row.kind == DurableNavigationBootstrapRowKindV1::Node && row.id == row.owner_record_id
        })
        .map(|row| (row.id.clone(), row.owner_source_path.clone()))
        .collect::<BTreeMap<_, _>>();
    for row in rows
        .iter_mut()
        .filter(|row| row.kind == DurableNavigationBootstrapRowKindV1::Node)
    {
        let value = parse_carrier(&row.raw, limits)?;
        let Some(view) = value
            .object_get("properties")
            .and_then(|properties| properties.object_get("record_version_view"))
        else {
            continue;
        };
        let id = view
            .object_get("record_ref")
            .and_then(|reference| reference.object_get("id"))
            .and_then(|id| id.as_str())
            .ok_or(DurableError::Corrupt(
                "navigation history record identity absent",
            ))?;
        let owner_path = current_owners
            .get(id)
            .filter(|_| id == row.owner_record_id)
            .ok_or(DurableError::Corrupt(
                "navigation historical record owner absent",
            ))?;
        let declared_source = view
            .object_get("provenance")
            .and_then(|provenance| provenance.object_get("source"))
            .and_then(|source| source.object_get("source_ref"))
            .and_then(|path| path.as_str());
        if let Some(path) = declared_source {
            if checked_path(path)?.as_str() != owner_path {
                return Err(DurableError::Corrupt(
                    "navigation historical record source differs",
                ));
            }
        } else if row.owner_source_path != *owner_path {
            return Err(DurableError::Corrupt(
                "navigation historical archive has no record source",
            ));
        }
        row.owner_source_path = owner_path.clone();
    }
    Ok(())
}

/// Restore each reified Claim's contribution to shared carriers. PostgreSQL's
/// existing bootstrap key includes the owner pair; selecting one arbitrary
/// owner would lose surviving contributions when another Claim is replaced.
fn expand_claim_contributions(
    rows: &mut Vec<DurableNavigationBootstrapRowV1>,
    limits: DurableNavigationEventLimits,
    charged_bytes: &mut u64,
) -> DurableResult<()> {
    type Owner = (String, String);
    let mut owners = BTreeMap::<String, BTreeSet<Owner>>::new();
    let mut linked = BTreeMap::<String, String>::new();
    let mut claim_edges = BTreeMap::<String, Owner>::new();
    let local_nodes = rows
        .iter()
        .filter(|row| row.kind == DurableNavigationBootstrapRowKindV1::Node)
        .map(|row| row.id.clone())
        .collect::<BTreeSet<_>>();
    for row in rows.iter() {
        let value = parse_carrier(&row.raw, limits)?;
        let field = |key| value.object_get(key).and_then(|v| v.as_str());
        if row.kind == DurableNavigationBootstrapRowKindV1::Node {
            if field("node_kind") == Some("maker")
                && let Some(identity) = value
                    .object_get("properties")
                    .and_then(|p| p.object_get("identity_node_id"))
                    .and_then(|v| v.as_str())
            {
                linked.insert(row.id.clone(), identity.to_owned());
            }
        } else if row.kind == DurableNavigationBootstrapRowKindV1::Edge {
            if let (Some(claim), Some(path)) = (field("claim_ref"), field("source_claim_file_ref"))
            {
                let owner = (checked_path(path)?.into_string(), claim.to_owned());
                let root = format!("claim:{claim}");
                let subject_navigation = field("edge_kind") == Some("claim_subject_navigation")
                    && field("predicate_id") == Some("has_source_claim")
                    && field("to_id") == Some(root.as_str());
                if !subject_navigation && field("from_id") != Some(root.as_str()) {
                    return Err(DurableError::Corrupt("navigation Claim edge owner differs"));
                }
                owners.entry(root).or_default().insert(owner.clone());
                let target =
                    field("to_id").ok_or(DurableError::Corrupt("navigation Claim target"))?;
                // Alternative/superseded Claims retain their own cohorts.
                if !subject_navigation && !target.starts_with("claim:") {
                    owners
                        .entry(target.to_owned())
                        .or_default()
                        .insert(owner.clone());
                }
                claim_edges.insert(row.id.clone(), owner);
            }
        }
    }
    for (node, identity) in linked {
        if local_nodes.contains(&identity)
            && let Some(contributors) = owners.get(&node).cloned()
        {
            owners.entry(identity).or_default().extend(contributors);
        }
    }
    let mut expanded = Vec::new();
    for row in std::mem::take(rows) {
        let contributions = match row.kind {
            DurableNavigationBootstrapRowKindV1::Node => owners.remove(&row.id),
            DurableNavigationBootstrapRowKindV1::Edge => claim_edges
                .remove(&row.id)
                .map(|owner| BTreeSet::from([owner])),
            _ => None,
        };
        if let Some(contributions) = contributions {
            for (index, (path, id)) in contributions.into_iter().enumerate() {
                if index > 0 {
                    *charged_bytes = charged_bytes
                        .checked_add(row.raw.len() as u64)
                        .filter(|n| *n <= limits.max_total_bytes)
                        .ok_or(DurableError::Refused("navigation Claim contribution bytes"))?;
                }
                if expanded.len() >= limits.max_current_rows {
                    return Err(DurableError::Refused("navigation Claim contribution rows"));
                }
                expanded.push(DurableNavigationBootstrapRowV1 {
                    kind: row.kind,
                    id: row.id.clone(),
                    owner_source_path: path,
                    owner_record_id: id,
                    visible: row.visible,
                    raw: row.raw.clone(),
                });
            }
        } else {
            if expanded.len() >= limits.max_current_rows {
                return Err(DurableError::Refused(
                    "navigation bootstrap contribution rows",
                ));
            }
            expanded.push(row);
        }
    }
    // A maker's identity may already be represented by the same subject/object,
    // but every retained ownership target must be an actual selected carrier.
    if !owners.is_empty() || !claim_edges.is_empty() {
        return Err(DurableError::Corrupt(
            "navigation Claim contribution carrier absent",
        ));
    }
    *rows = expanded;
    Ok(())
}
fn checked_path(value: &str) -> DurableResult<RelativePath> {
    if value.len() > MAX_BOOTSTRAP_PATH_BYTES {
        return Err(DurableError::Refused(
            "navigation bootstrap source path size",
        ));
    }
    RelativePath::parse(value.split_once('#').map_or(value, |(path, _)| path))
        .map_err(|_| DurableError::Invalid("navigation bootstrap source path"))
}

/// A stable evidence reference stays in the carrier's source_refs. The source
/// producer may additionally bind it to the exact catalog member that owns the
/// referenced record. Both bootstrap capture and sealing use this contract;
/// a binding cannot add an unrelated reference, redirect a URI, or hide a path.
pub(crate) fn checked_source_reference_bindings<'a>(
    bindings: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
    has_reference: impl Fn(&str) -> bool,
) -> DurableResult<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for (reference, source) in bindings {
        let source = source.ok_or(DurableError::Invalid("navigation source binding path"))?;
        let path = checked_path(source)?;
        if reference.is_empty()
            || reference.len() > MAX_BOOTSTRAP_PATH_BYTES
            || reference.contains('/')
            || reference.contains('#')
            || reference.contains('\0')
            || has_uri_scheme(reference)
            || !has_reference(reference)
            || path.as_str() != source
            || result
                .insert(reference.to_owned(), path.into_string())
                .is_some()
        {
            return Err(DurableError::Invalid("navigation source reference binding"));
        }
    }
    Ok(result)
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic() || (index > 0 && b"+-.".contains(&byte))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn navigation_reference_binding_capture_preserves_owner_and_dependencies() {
        let limits = DurableNavigationEventLimits::default();
        let owner = "ToS/source-witnesses/example/responsibility-claims.jsonl";
        let anchor_source = "ToS/source-witnesses/example/anchors.jsonl";
        let anchor = "tos.anchor.example.title-page";
        let row = json!({
            "edge_id":"edge:claim", "from_id":"from", "to_id":"to",
            "edge_kind":"evidence_claim", "record_id":"tos.claim.example.author",
            "source_ref":owner, "source_refs":[owner,anchor,"ToS/research.md"],
            "source_ref_bindings":{anchor:anchor_source}
        });
        let capture = |value: &serde_json::Value| {
            let mut rows = Vec::new();
            let mut paths = BTreeSet::new();
            capture_original_carrier(
                DurableNavigationBootstrapRowKindV1::Edge,
                "edge:claim".into(),
                serde_json::to_vec(value).unwrap(),
                limits,
                &mut rows,
                &mut paths,
                &mut 0,
                &mut BTreeMap::from([("from".into(), true), ("to".into(), true)]),
            )?;
            Ok::<_, DurableError>((rows, paths))
        };
        let (rows, paths) = capture(&row).unwrap();
        assert_eq!(rows[0].owner_source_path, owner);
        assert_eq!(rows[0].owner_record_id, "tos.claim.example.author");
        assert_eq!(
            paths,
            BTreeSet::from([
                owner.to_owned(),
                anchor_source.to_owned(),
                "ToS/research.md".into(),
            ])
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&rows[0].raw).unwrap(),
            row
        );
        let mut missing_owner = row.clone();
        missing_owner.as_object_mut().unwrap().remove("source_ref");
        assert!(matches!(
            capture(&missing_owner),
            Err(DurableError::Conflict(_))
        ));
        let mut unrelated = row.clone();
        unrelated["source_ref_bindings"]["tos.anchor.unrelated"] = json!(anchor_source);
        assert!(capture(&unrelated).is_err());
        let mut redirected_path = row;
        redirected_path["source_ref_bindings"]["ToS/research.md"] = json!(anchor_source);
        assert!(capture(&redirected_path).is_err());
    }

    #[test]
    fn shared_claim_carriers_survive_removal_of_one_contributor() {
        let limits = DurableNavigationEventLimits::default();
        let path = "ToS/source-witnesses/claims/example/source-claims.jsonl";
        let mut rows = Vec::new();
        for (kind, id, value) in [
            (
                DurableNavigationBootstrapRowKindV1::Node,
                "claim:a",
                json!({"node_id":"claim:a"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Node,
                "claim:b",
                json!({"node_id":"claim:b"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Node,
                "evidence:shared",
                json!({"node_id":"evidence:shared","properties":{"evidence_ref":"ToS/evidence.md"}}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                "edge:a",
                json!({"edge_id":"edge:a","claim_ref":"a","source_claim_file_ref":path,"from_id":"claim:a","to_id":"evidence:shared"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                "edge:b",
                json!({"edge_id":"edge:b","claim_ref":"b","source_claim_file_ref":path,"from_id":"claim:b","to_id":"evidence:shared"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                "alternative:a",
                json!({"edge_id":"alternative:a","claim_ref":"a","source_claim_file_ref":path,"from_id":"claim:a","to_id":"claim:b"}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                "subject:b",
                json!({"edge_id":"subject:b","claim_ref":"b","source_claim_file_ref":path,
                    "edge_kind":"claim_subject_navigation","predicate_id":"has_source_claim",
                    "from_id":"tos.work.a","to_id":"claim:b"}),
            ),
        ] {
            rows.push(DurableNavigationBootstrapRowV1 {
                kind,
                id: id.into(),
                owner_source_path: path.into(),
                owner_record_id: id.into(),
                visible: Some(true),
                raw: serde_json::to_vec(&value).unwrap(),
            });
        }
        rows.push(DurableNavigationBootstrapRowV1 {
            kind: DurableNavigationBootstrapRowKindV1::Node,
            id: "tos.work.a".into(),
            owner_source_path: "ToS/source-witnesses/works/a/work.json".into(),
            owner_record_id: "tos.work.a".into(),
            visible: Some(true),
            raw: br#"{"node_id":"tos.work.a"}"#.to_vec(),
        });
        let mut bytes = rows.iter().map(|row| row.raw.len() as u64).sum();
        expand_claim_contributions(&mut rows, limits, &mut bytes).unwrap();
        assert_eq!(
            rows.iter()
                .find(|row| row.id == "tos.work.a")
                .unwrap()
                .owner_record_id,
            "tos.work.a"
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.id == "subject:b")
                .unwrap()
                .owner_record_id,
            "b"
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.id == "evidence:shared")
                .count(),
            2
        );
        assert_eq!(rows.iter().filter(|row| row.id == "claim:b").count(), 1);
        assert_eq!(
            bytes,
            rows.iter().map(|row| row.raw.len() as u64).sum::<u64>()
        );
        rows.retain(|row| row.owner_record_id != "a");
        assert_eq!(
            rows.iter()
                .map(|row| row.id.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "claim:b",
                "edge:b",
                "evidence:shared",
                "subject:b",
                "tos.work.a"
            ])
        );
        assert!(
            rows.iter()
                .filter(|row| row.id != "tos.work.a")
                .all(|row| row.owner_record_id == "b" && row.owner_source_path == path)
        );
    }

    #[test]
    fn original_history_rows_share_owner_and_fragment_refs_keep_exact_file() {
        let source = "ToS/source-witnesses/works/example/work.json";
        let archive = "ToS/source-witnesses/.record-revisions/example/previous.blob";
        let packets = [
            (
                DurableNavigationBootstrapRowKindV1::Node,
                json!({"node_id":"record-version:one","source_ref":format!("{archive}#L1"),"properties":{"record_version_view":{"record_ref":{"id":"work:example","version":1},"provenance":{"source":{"source_ref":source}}}}}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Node,
                json!({"node_id":"work:example","source_ref":source}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Edge,
                json!({"edge_id":"history:one","edge_kind":"exact_historical_record_reference","from_id":"work:example","to_id":"record-version:one","source_refs":[archive,source]}),
            ),
            (
                DurableNavigationBootstrapRowKindV1::Right,
                json!({"rights_id":"rights:example","record_id":"work:example","source_ref":source}),
            ),
        ];
        let mut rows = Vec::new();
        let mut paths = BTreeSet::new();
        let mut charged = 0;
        let mut visibility = BTreeMap::new();
        for (kind, value) in packets {
            let key = match kind {
                DurableNavigationBootstrapRowKindV1::Node => "node_id",
                DurableNavigationBootstrapRowKindV1::Edge => "edge_id",
                _ => "rights_id",
            };
            let raw = serde_json::to_vec(&value).unwrap();
            capture_original_carrier(
                kind,
                value[key].as_str().unwrap().into(),
                raw.clone(),
                DurableNavigationEventLimits::default(),
                &mut rows,
                &mut paths,
                &mut charged,
                &mut visibility,
            )
            .unwrap();
            assert_eq!(rows.last().unwrap().raw, raw);
        }
        let original_raw = rows.iter().map(|row| row.raw.clone()).collect::<Vec<_>>();
        bind_historical_record_owners(&mut rows, DurableNavigationEventLimits::default()).unwrap();
        assert_eq!(
            original_raw,
            rows.iter().map(|row| row.raw.clone()).collect::<Vec<_>>()
        );
        assert_eq!(
            paths,
            BTreeSet::from([source.to_owned(), archive.to_owned()])
        );
        assert!(rows.iter().all(|row| row.owner_record_id == "work:example" && row.owner_source_path == source));
        assert!(rows[..3].iter().all(|row| row.visible == Some(true)));
        assert_eq!(rows[3].visible, None);
        assert_eq!(
            charged,
            rows.iter().map(|row| row.raw.len() as u64).sum::<u64>()
        );
        rows[1].owner_source_path = "ToS/source-witnesses/works/another/work.json".into();
        assert!(
            bind_historical_record_owners(&mut rows, DurableNavigationEventLimits::default())
                .is_err()
        );
    }

    #[test]
    fn hidden_original_node_does_not_hide_missing_endpoint_corruption() {
        let mut rows = Vec::new();
        let mut paths = BTreeSet::new();
        let mut charged = 0;
        let mut visibility = BTreeMap::new();
        let limits = DurableNavigationEventLimits::default();
        let node = br#"{"node_id":"hidden","source_ref":"ToS/source-witnesses/works/example/work.json","properties":{"packet_id":"private"}}"#;
        capture_original_carrier(
            DurableNavigationBootstrapRowKindV1::Node,
            "hidden".into(),
            node.to_vec(),
            limits,
            &mut rows,
            &mut paths,
            &mut charged,
            &mut visibility,
        )
        .unwrap();
        assert_eq!(rows[0].visible, Some(false));
        let edge = br#"{"edge_id":"broken","from_id":"hidden","to_id":"missing","source_refs":["ToS/source-witnesses/works/example/work.json"]}"#;
        assert!(
            capture_original_carrier(
                DurableNavigationBootstrapRowKindV1::Edge,
                "broken".into(),
                edge.to_vec(),
                limits,
                &mut rows,
                &mut paths,
                &mut charged,
                &mut visibility
            )
            .is_err()
        );
        assert_eq!(rows.len(), 1);
    }
}
