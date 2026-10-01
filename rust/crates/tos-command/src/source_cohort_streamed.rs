//! Additive cold Agent assessment over complete verified source membership.
//! The private SQLite index is a bounded derived lookup carrier. It never
//! grants currentness, rights, semantic admission or a selected generation.

use super::source_cohort_stream_index::{
    SourceAssessmentIndex, SourceAssessmentLimits, SourceAssessmentMember, SourceAssessmentWork,
};
use super::*;
use crate::source_revisions::ReadonlyRecordFiles;
use std::cell::RefCell;

/// Counts exact returned logical source bodies, including repeated cold reads.
/// SQL control/gates, PG scan/protocol IO and physical storage IO are not
/// observed by these counters. They are never reported as total operation IO.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StreamedColdSourceWorkV1 {
    pub derived_index: SourceAssessmentWork,
    pub original_index_length_observed: u64,
    pub original_index_allocated_observed: u64,
    pub original_bodies_returned: u64,
    pub original_body_bytes_returned: u64,
    pub original_retirement_bodies_returned: u64,
    pub original_retirement_bytes_returned: u64,
    pub current_bodies_returned: u64,
    pub current_body_bytes_returned: u64,
    pub current_projection_query_rows_returned: u64,
    pub current_and_history_projection_bytes_returned: u64,
    pub software_bodies_returned: u64,
    pub software_body_bytes_returned: u64,
    /// Bound fields sent to fresh-bootstrap SQL, not WAL or protocol IO.
    pub fresh_pg_field_bytes_attempted: u64,
    pub fresh_pg_rows_written: u64,
}

pub(super) fn set_streamed_pg_limits(
    tx: &mut Transaction<'_>,
    max_statement_ms: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    active(deadline, cancelled)?;
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_millis();
    if remaining == 0 || max_statement_ms == 0 || max_statement_ms > i32::MAX as u64 {
        return Err(DurableError::Refused("streamed SQL deadline/profile bound"));
    }
    let milliseconds = (remaining.min(max_statement_ms as u128)) as u64;
    tx.query_one(
        "SELECT set_config('statement_timeout',$1,true),set_config('lock_timeout',$2,true)",
        &[
            &format!("{milliseconds}ms"),
            &format!("{}ms", milliseconds.min(5000)),
        ],
    )?;
    active(deadline, cancelled)
}

struct StreamedSourceReadEpoch<'a, 'db> {
    tx: &'a mut Transaction<'db>,
    max_statement_ms: u64,
    index: &'a RefCell<&'a mut SourceAssessmentIndex>,
    index_work: &'a RefCell<&'a mut SourceAssessmentWork>,
    body_work: &'a mut StreamedColdSourceWorkV1,
    store: &'a SegmentStore,
    context: &'a CommandContext,
    domain: &'a str,
    // The caller holds this exact cold-root custody and audit/domain rights
    // locks for the entire read epoch, then repeats the final source fences.
    cold: &'a ColdCut,
    fresh_sink: bool,
}

impl ReadonlyRecordFiles for StreamedSourceReadEpoch<'_, '_> {
    fn read(
        &mut self,
        name: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> cmd::SourceCommandResult<Vec<u8>> {
        let mut operation = || -> DurableResult<Vec<u8>> {
            active(deadline, cancelled)?;
            self.cold.audited_root.require_store(self.store)?;
            let path = RelativePath::parse(name)
                .map_err(|_| DurableError::Invalid("streamed source member path"))?;
            if max_bytes == 0 || max_bytes > 8_388_608 {
                return Err(DurableError::Refused(
                    "streamed source per-member byte bound",
                ));
            }
            if !name.starts_with("ToS/") {
                // Software inputs were verified against their independent
                // capture before this epoch; never fall back to a filesystem.
                let raw = self.context.file(&path).map_err(source_error)?.ok_or(
                    DurableError::Refused("streamed selected software member absent"),
                )?;
                if raw.len() > max_bytes {
                    return Err(DurableError::Refused("streamed software member byte bound"));
                }
                ManagedSourceWorkV1::charge(&mut self.body_work.software_bodies_returned, 1)?;
                ManagedSourceWorkV1::charge(
                    &mut self.body_work.software_body_bytes_returned,
                    raw.len() as u64,
                )?;
                return Ok(raw.to_vec());
            }
            let member = self
                .index
                .borrow_mut()
                .member(name, &mut **self.index_work.borrow_mut())?
                .ok_or(DurableError::Refused(
                    "streamed selected source member absent",
                ))?;
            if member.size_bytes > max_bytes as u64 {
                return Err(DurableError::Refused("streamed current member byte bound"));
            }
            let encoded = self
                .index
                .borrow_mut()
                .lookup_current_placement(name, &mut **self.index_work.borrow_mut())?
                .ok_or(DurableError::Corrupt(
                    "streamed verified current placement absent",
                ))?;
            let selected = tos_segment_store::decode_placement_tree_row(&encoded, 1_048_576)?;
            set_streamed_pg_limits(self.tx, self.max_statement_ms, deadline, cancelled)?;
            let row = self.tx.query_one(
                "SELECT c.*,a.attempt_fence AS source_attempt_fence FROM cmd2_current c JOIN cmd2_attempt a USING(domain,prepare_id) WHERE c.domain=$1 AND c.subject=$2",
                &[&self.domain, &name],
            )?;
            let metadata = selected_source_metadata(&row, &selected, self.domain, &path)?;
            let carrier = row_source_metadata(&row)?;
            if metadata.sha256.as_bytes() != &member.sha256
                || metadata.size_bytes != member.size_bytes
                || metadata.mode != member.mode
                || carrier.dependencies != member.dependencies
            {
                return Err(DurableError::Corrupt(
                    "streamed source carrier lookup differs",
                ));
            }
            let prepare: Vec<u8> = row.get("prepare_id");
            let receipts = match self.store.recover_attempt_fenced(
                &prepare,
                as_u64(row.get("source_attempt_fence"))?,
                0,
            )? {
                Some(AttemptRecovery::Sealed { receipts }) => receipts,
                _ => {
                    return Err(DurableError::Corrupt(
                        "streamed current source intent absent",
                    ));
                }
            };
            let receipt = receipts
                .iter()
                .find(|receipt| receipt.receipt_id() == selected.placement.receipt_id())
                .ok_or(DurableError::Corrupt("streamed current receipt absent"))?;
            check_history_locator(
                &row,
                receipt,
                self.domain,
                name,
                as_u64(row.get("revision"))?,
            )?;
            if receipt.placement() != selected.placement {
                return Err(DurableError::Corrupt("streamed current placement differs"));
            }
            let mut raw = Vec::new();
            self.store
                .read_selected(receipt, max_bytes as u64, &mut raw)?;
            // Preserve successful returned work even if subsequent semantic or
            // deadline verification refuses the operation.
            ManagedSourceWorkV1::charge(&mut self.body_work.current_bodies_returned, 1)?;
            ManagedSourceWorkV1::charge(
                &mut self.body_work.current_body_bytes_returned,
                raw.len() as u64,
            )?;
            active(deadline, cancelled)?;
            if Digest256::of_bytes(&raw).as_bytes() != &member.sha256
                || raw.len() as u64 != member.size_bytes
            {
                return Err(DurableError::Corrupt(
                    "streamed current source bytes differ",
                ));
            }
            Ok(raw)
        };
        operation().map_err(crate::source_current_cut::durable)
    }

    fn list_directory(
        &mut self,
        name: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> cmd::SourceCommandResult<Vec<(String, bool)>> {
        active(deadline, cancelled).map_err(crate::source_current_cut::durable)?;
        if !name.starts_with("ToS/") {
            return Err(cmd::SourceCommandError::Unsupported(
                "streamed software directory listing",
            ));
        }
        let children = self
            .index
            .borrow_mut()
            .list_immediate_children(
                name,
                cmd::SELECTED_SOURCE_MAX_FILES,
                deadline,
                cancelled,
                &mut **self.index_work.borrow_mut(),
            )
            .map_err(crate::source_current_cut::durable)?;
        active(deadline, cancelled).map_err(crate::source_current_cut::durable)?;
        Ok(children
            .into_iter()
            .map(|child| (child.name, child.is_directory))
            .collect())
    }
}

/// Complete content fixity is a prerequisite only. This private witness grants
/// no semantic assessment, current PG selection, rights or generation.
struct VerifiedStreamedOriginalContentV1 {
    revision: SourceRevision,
    membership: SourceMembershipV1,
    authored_members: u64,
}

fn verify_original_content(
    original: &tos_source_store::StreamedCorpusCutReaderV1,
    revision: SourceRevision,
    membership: SourceMembershipV1,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
    work: &mut StreamedColdSourceWorkV1,
) -> DurableResult<VerifiedStreamedOriginalContentV1> {
    active(deadline, cancelled)?;
    let current = original
        .revision_at(0)
        .map_err(|_| DurableError::Refused("streamed original current revision"))?
        .ok_or(DurableError::Corrupt("streamed original current absent"))?;
    if current.revision != revision
        || current.membership != membership
        || max_member_bytes == 0
        || max_member_bytes > 8_388_608
    {
        return Err(DurableError::Conflict(
            "streamed original selection differs",
        ));
    }
    let mut ordinal = 0u64;
    let mut authored_members = 0u64;
    while let Some(selected) = original
        .revision_at(ordinal)
        .map_err(|_| DurableError::Refused("streamed original retained revision"))?
    {
        active(deadline, cancelled)?;
        let mut stream = original
            .stream_with_member_limit(selected.revision, max_member_bytes)
            .map_err(|_| DurableError::Refused("streamed original membership cursor"))?;
        while let Some(member) = stream
            .next_member(deadline, cancelled)
            .map_err(|_| DurableError::Corrupt("streamed original content verification"))?
        {
            if ordinal == 0 && member.path.as_str().starts_with("ToS/") {
                authored_members = authored_members
                    .checked_add(1)
                    .ok_or(DurableError::Refused(
                        "streamed original authored count overflow",
                    ))?;
            }
            ManagedSourceWorkV1::charge(&mut work.original_bodies_returned, 1)?;
            ManagedSourceWorkV1::charge(
                &mut work.original_body_bytes_returned,
                member.raw.len() as u64,
            )?;
            if member.size_bytes > max_member_bytes {
                return Err(DurableError::Refused("streamed original member bound"));
            }
        }
        if stream.coverage() != Some(selected.membership) {
            return Err(DurableError::Corrupt(
                "streamed original incomplete content coverage",
            ));
        }
        for retirement in 0..selected.retirement_count {
            active(deadline, cancelled)?;
            let retired = original
                .read_retirement(
                    selected.revision,
                    retirement,
                    max_member_bytes,
                    deadline,
                    cancelled,
                )
                .map_err(|_| DurableError::Corrupt("streamed original retirement content"))?;
            ManagedSourceWorkV1::charge(&mut work.original_retirement_bodies_returned, 2)?;
            ManagedSourceWorkV1::charge(
                &mut work.original_retirement_bytes_returned,
                retired.raw.len() as u64,
            )?;
            ManagedSourceWorkV1::charge(
                &mut work.original_retirement_bytes_returned,
                retired.event_raw.len() as u64,
            )?;
        }
        ordinal = ordinal.checked_add(1).ok_or(DurableError::Refused(
            "streamed original revision ordinal overflow",
        ))?;
    }
    active(deadline, cancelled)?;
    Ok(VerifiedStreamedOriginalContentV1 {
        revision,
        membership,
        authored_members,
    })
}

/// Populate only from the exact already cold-verified membership cursor and
/// the caller's held RR source/audit/domain/rights snapshot. No row list or
/// placement map survives the next iteration. The derived index is not a gate.
fn index_current_source(
    tx: &mut Transaction<'_>,
    max_statement_ms: u64,
    store: &SegmentStore,
    domain: &str,
    cold: &ColdCut,
    original: &tos_source_store::StreamedCorpusCutReaderV1,
    original_content: &VerifiedStreamedOriginalContentV1,
    index: &mut SourceAssessmentIndex,
    limits: SourceAssessmentLimits,
    index_work: &mut SourceAssessmentWork,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    active(deadline, cancelled)?;
    cold.audited_root.require_store(store)?;
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    let admitted = tx.query_one(
        "SELECT count(*),coalesce(sum(content_length),0)::bigint,coalesce(max(content_length),0),coalesce(max(octet_length(inventory_projection)),0)::bigint,coalesce(max(octet_length(row_to_json(c)::text)),0)::bigint,coalesce(sum(octet_length(row_to_json(c)::text)),0)::bigint FROM cmd2_current c WHERE domain=$1",
        &[&domain],
    )?;
    if as_u64(admitted.get(0))? != cold.current_members
        || cold.current_members > limits.max_rows
        || as_u64(admitted.get(1))? > limits.max_logical_bytes
        || as_u64(admitted.get(2))? > 8_388_608
        || as_u64(admitted.get(3))? > limits.max_value_bytes as u64
        || as_u64(admitted.get(4))? > limits.max_value_bytes as u64
        || as_u64(admitted.get(5))? > limits.max_logical_bytes
    {
        return Err(DurableError::Refused(
            "streamed current input preflight bounds",
        ));
    }
    let installation = cold.installation(store);
    let mut cursor = installation.cursor(GenerationNamespaceV1::Current)?;
    let mut count = 0u64;
    let mut originals = 0u64;
    let mut after_length = 0i32;
    let mut after_path = String::new();
    loop {
        active(deadline, cancelled)?;
        let parameters: &[&(dyn postgres::types::ToSql + Sync)] =
            &[&domain, &after_length, &after_path, &COLD_SOURCE_PAGE_ROWS];
        set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let page = tx.query(
            "SELECT c.*,a.attempt_fence AS source_attempt_fence FROM cmd2_current c JOIN cmd2_attempt a USING(domain,prepare_id) WHERE c.domain=$1 AND (octet_length(c.subject),c.subject COLLATE \"C\") > ($2,$3 COLLATE \"C\") ORDER BY octet_length(c.subject),c.subject COLLATE \"C\" LIMIT $4",
            parameters,
        )?;
        if page.is_empty() {
            break;
        }
        for row in page {
            active(deadline, cancelled)?;
            let selected = cursor
                .next_row()?
                .ok_or(DurableError::Corrupt("streamed current cursor ended early"))?;
            let subject: String = row.get("subject");
            let length = i32::try_from(subject.len())
                .map_err(|_| DurableError::Corrupt("streamed current path byte length"))?;
            if (length, subject.as_bytes()) <= (after_length, after_path.as_bytes())
                || count >= cold.current_members
            {
                return Err(DurableError::Corrupt(
                    "streamed current keyset order/count differs",
                ));
            }
            after_length = length;
            after_path.clone_from(&subject);
            let path = RelativePath::parse(&subject)
                .map_err(|_| DurableError::Corrupt("streamed current path"))?;
            let metadata = selected_source_metadata(&row, &selected, domain, &path)?;
            let carrier = row_source_metadata(&row)?;
            let prepare: Vec<u8> = row.get("prepare_id");
            let receipts = match store.recover_attempt_fenced(
                &prepare,
                as_u64(row.get("source_attempt_fence"))?,
                0,
            )? {
                Some(AttemptRecovery::Sealed { receipts }) => receipts,
                _ => {
                    return Err(DurableError::Corrupt(
                        "streamed current sealed intent absent",
                    ));
                }
            };
            let receipt = receipts
                .iter()
                .find(|receipt| receipt.receipt_id() == selected.placement.receipt_id())
                .ok_or(DurableError::Corrupt(
                    "streamed current selected receipt absent",
                ))?;
            check_history_locator(
                &row,
                receipt,
                domain,
                &subject,
                as_u64(row.get("revision"))?,
            )?;
            if receipt.placement() != selected.placement {
                return Err(DurableError::Corrupt(
                    "streamed current receipt placement differs",
                ));
            }
            let original_member = original
                .member(original_content.revision, &path)
                .map_err(|_| DurableError::Refused("streamed original member lookup"))?;
            if let Some(original_member) = original_member {
                // A complete content EOF witness preceded this lookup; unchanged
                // bytes retain their original source fixity/profile, not a new tag.
                if original_member.sha256 != metadata.sha256
                    || original_member.size_bytes != metadata.size_bytes
                    || original_member.mode != metadata.mode
                    || receipt.binding().profile_id != ORIGINAL
                {
                    return Err(DurableError::Corrupt("streamed original carrier changed"));
                }
                let has_dependencies = original
                    .has_dependency_index(original_content.revision, &path)
                    .map_err(|_| {
                        DurableError::Refused("streamed original dependency key lookup")
                    })?;
                if has_dependencies != carrier.dependencies.is_some() {
                    return Err(DurableError::Corrupt(
                        "streamed original dependency presence differs",
                    ));
                }
                let mut after: Option<RelativePath> = None;
                let mut position = 0usize;
                while let Some(target) = original
                    .dependency_after(original_content.revision, &path, after.as_ref())
                    .map_err(|_| {
                        DurableError::Refused("streamed original dependency target lookup")
                    })?
                {
                    active(deadline, cancelled)?;
                    if carrier
                        .dependencies
                        .as_ref()
                        .and_then(|values| values.get(position))
                        .map(String::as_str)
                        != Some(target.as_str())
                    {
                        return Err(DurableError::Corrupt(
                            "streamed original dependencies differ",
                        ));
                    }
                    position = position
                        .checked_add(1)
                        .ok_or(DurableError::Refused("streamed dependency count overflow"))?;
                    after = Some(target);
                }
                if carrier
                    .dependencies
                    .as_ref()
                    .is_some_and(|values| values.len() != position)
                {
                    return Err(DurableError::Corrupt(
                        "streamed original dependency EOF differs",
                    ));
                }
                originals = originals
                    .checked_add(1)
                    .ok_or(DurableError::Refused("streamed original count overflow"))?;
            } else if receipt.binding().profile_id != CREATION
                || carrier.mode != 0o644
                || carrier.dependencies.is_none()
            {
                return Err(DurableError::Corrupt(
                    "streamed extra member outside controlled Agent writer",
                ));
            }
            index.add_member(
                SourceAssessmentMember {
                    path: &subject,
                    sha256: *metadata.sha256.as_bytes(),
                    size_bytes: metadata.size_bytes,
                    mode: metadata.mode,
                    dependencies: carrier.dependencies.as_deref(),
                },
                index_work,
            )?;
            let encoded = tos_segment_store::encode_placement_tree_row(
                &selected,
                limits.max_placement_bytes,
            )?;
            index.add_current_placement(&subject, &encoded, index_work)?;
            count = count
                .checked_add(1)
                .ok_or(DurableError::Refused("streamed current count overflow"))?;
        }
    }
    if cursor.next_row()?.is_some() || count != cold.current_members {
        return Err(DurableError::Corrupt(
            "streamed current PG/cursor EOF differs",
        ));
    }
    cursor.finish()?;
    if originals != original_content.authored_members {
        return Err(DurableError::Corrupt(
            "streamed original bootstrap membership missing",
        ));
    }
    index.finish_member_inputs(index_work)?;
    active(deadline, cancelled)
}

// Exact streamed form of existing index_rows/predicates: a stable owner key,
// its unique absence predicate, all source-home ancestors and inventory gate.
fn index_owned_key(
    index: &mut SourceAssessmentIndex,
    kind: &str,
    token: &str,
    path: &str,
    work: &mut SourceAssessmentWork,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    use super::source_cohort_stream_index::{AssessmentIndexRow, AssessmentPredicateRow};
    active(deadline, cancelled)?;
    if !["metadata", "claim", "event", "anchor", "form", "path"].contains(&kind) {
        return Err(DurableError::Refused("streamed owner index kind"));
    }
    let version = definition().to_hex();
    index.add_index_row(
        &AssessmentIndexRow {
            kind: kind.into(),
            token: token.into(),
            path: path.into(),
            definition_digest: version.clone(),
        },
        work,
    )?;
    let predicate = |kind: &str, scope: &str, token: &str| AssessmentPredicateRow {
        kind: kind.into(),
        owner: OWNER.into(),
        scope: scope.into(),
        token: token.into(),
        definition_version: version.clone(),
    };
    index.add_predicate_row(&predicate("unique", kind, token), work)?;
    index.add_predicate_row(&predicate("range", "source-inventory", "all"), work)?;
    let mut rest = path;
    while let Some((parent, _)) = rest.rsplit_once('/') {
        active(deadline, cancelled)?;
        index.add_predicate_row(&predicate("range", "source-home", parent), work)?;
        rest = parent;
    }
    active(deadline, cancelled)
}

struct CompleteAgentIdentityPassV1 {
    members: u64,
}

fn stage_current_agent_identities(
    epoch: &mut StreamedSourceReadEpoch<'_, '_>,
    profile: &crate::source_claims::AgentInventoryAssessmentProfile,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<CompleteAgentIdentityPassV1> {
    use super::source_cohort_stream_index::{SourceEvidenceKind, SourceIdentityKind};
    use crate::source_claims::{AgentInventoryEvidenceKind, AgentInventoryIdentityKind};
    let mut after = None::<String>;
    let mut count = 0u64;
    loop {
        active(deadline, cancelled)?;
        let next = epoch
            .index
            .borrow_mut()
            .member_after(after.as_deref(), &mut **epoch.index_work.borrow_mut())?;
        let Some(member) = next else {
            break;
        };
        let path = RelativePath::parse(&member.path)
            .map_err(|_| DurableError::Corrupt("streamed staged path"))?;
        let raw = epoch
            .read(&member.path, 8_388_608, deadline, cancelled)
            .map_err(source_error)?;
        let file = SourceFile { path, raw };
        let index = epoch.index;
        let work = epoch.index_work;
        index_owned_key(
            &mut **index.borrow_mut(),
            "path",
            &member.path,
            &member.path,
            &mut **work.borrow_mut(),
            deadline,
            cancelled,
        )?;
        crate::source_claims::stage_agent_inventory_member(
            profile,
            &file,
            |kind, id, path| {
                let (typed, name) = match kind {
                    AgentInventoryIdentityKind::Metadata => {
                        (SourceIdentityKind::Metadata, "metadata")
                    }
                    AgentInventoryIdentityKind::Form => (SourceIdentityKind::Form, "form"),
                };
                let mut index = index.borrow_mut();
                let mut work = work.borrow_mut();
                index
                    .add_identity(typed, id, path, &mut **work)
                    .map_err(crate::source_current_cut::durable)?;
                index_owned_key(
                    &mut **index,
                    name,
                    id,
                    path,
                    &mut **work,
                    deadline,
                    cancelled,
                )
                .map_err(crate::source_current_cut::durable)
            },
            |kind, id, path, line, sha, payload| {
                let (evidence, identity, name) = match kind {
                    AgentInventoryEvidenceKind::Event => (
                        SourceEvidenceKind::Event,
                        SourceIdentityKind::Event,
                        "event",
                    ),
                    AgentInventoryEvidenceKind::Anchor => (
                        SourceEvidenceKind::Anchor,
                        SourceIdentityKind::Anchor,
                        "anchor",
                    ),
                };
                let mut index = index.borrow_mut();
                let mut work = work.borrow_mut();
                index
                    .add_identity(identity, id, path, &mut **work)
                    .map_err(crate::source_current_cut::durable)?;
                index
                    .add_evidence_row(evidence, path, id, line, sha, payload, &mut **work)
                    .map_err(crate::source_current_cut::durable)?;
                index_owned_key(
                    &mut **index,
                    name,
                    id,
                    path,
                    &mut **work,
                    deadline,
                    cancelled,
                )
                .map_err(crate::source_current_cut::durable)
            },
        )
        .map_err(source_error)?;
        after = Some(member.path);
        count = count
            .checked_add(1)
            .ok_or(DurableError::Refused("streamed first pass count overflow"))?;
    }
    if count != epoch.cold.current_members {
        return Err(DurableError::Corrupt(
            "streamed complete identity pass EOF differs",
        ));
    }
    active(deadline, cancelled)?;
    Ok(CompleteAgentIdentityPassV1 { members: count })
}

fn evidence_for_member(
    index: &mut SourceAssessmentIndex,
    work: &mut SourceAssessmentWork,
    kind: super::source_cohort_stream_index::SourceEvidenceKind,
    path: &str,
    byte_limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Vec<cmd::SourceCommandResult<crate::source_claims::AgentInventoryEvidenceRow>>> {
    let mut bytes = 0usize;
    let mut output = Vec::new();
    index.for_each_evidence(
        kind,
        path,
        |row, _work| {
            active(deadline, cancelled)?;
            bytes = bytes
                .checked_add(row.canonical_payload.len())
                .and_then(|n| n.checked_add(row.id.len()))
                .and_then(|n| n.checked_add(48))
                .filter(|n| *n <= byte_limit)
                .ok_or(DurableError::Refused(
                    "streamed one-member evidence fan-in bound",
                ))?;
            output
                .try_reserve(1)
                .map_err(|_| DurableError::Refused("streamed evidence allocation"))?;
            output.push(Ok(crate::source_claims::AgentInventoryEvidenceRow {
                id: row.id,
                physical_line: row.physical_line,
                source_sha256: Digest256::from_bytes(row.source_sha256),
                canonical_payload: row.canonical_payload,
            }));
            Ok(())
        },
        work,
    )?;
    active(deadline, cancelled)?;
    Ok(output)
}

struct CompleteAgentProjectionPassV1 {
    members: u64,
    // Exact original ordered V1 projection transcript, not an addressed root.
    projection_digest: Digest256,
}

fn render_current_agent_projections(
    epoch: &mut StreamedSourceReadEpoch<'_, '_>,
    profile: &crate::source_claims::AgentInventoryAssessmentProfile,
    first: CompleteAgentIdentityPassV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: tos_validation::item_rules::ItemLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<CompleteAgentProjectionPassV1> {
    use super::source_cohort_stream_index::{AssessmentProjectionRow, SourceEvidenceKind};
    active(deadline, cancelled)?;
    if first.members != epoch.cold.current_members {
        return Err(DurableError::Corrupt(
            "streamed identity pass basis differs",
        ));
    }
    let mut after = None::<String>;
    let mut count = 0u64;
    let mut transcript = Digest256Hasher::new();
    part(&mut transcript, b"tos-managed-agent-inventory-coverage-v1");
    loop {
        active(deadline, cancelled)?;
        let next = epoch
            .index
            .borrow_mut()
            .member_after(after.as_deref(), &mut **epoch.index_work.borrow_mut())?;
        let Some(member) = next else {
            break;
        };
        let path = RelativePath::parse(&member.path)
            .map_err(|_| DurableError::Corrupt("streamed projected path"))?;
        let raw = epoch
            .read(&member.path, 8_388_608, deadline, cancelled)
            .map_err(source_error)?;
        let file = SourceFile { path, raw };
        let index = epoch.index;
        let work = epoch.index_work;
        let events = evidence_for_member(
            &mut **index.borrow_mut(),
            &mut **work.borrow_mut(),
            SourceEvidenceKind::Event,
            &member.path,
            1_048_576,
            deadline,
            cancelled,
        )?;
        let anchors = evidence_for_member(
            &mut **index.borrow_mut(),
            &mut **work.borrow_mut(),
            SourceEvidenceKind::Anchor,
            &member.path,
            1_048_576,
            deadline,
            cancelled,
        )?;
        let projection = crate::source_claims::render_agent_inventory_member(
            profile,
            &file,
            epoch,
            events,
            anchors,
            |path| {
                index
                    .borrow_mut()
                    .contains_member(path, &mut **work.borrow_mut())
                    .map_err(crate::source_current_cut::durable)
            },
            worker,
            limits,
            deadline,
            cancelled,
        )
        .map_err(source_error)?;
        if projection.len() > 1_048_576 {
            return Err(DurableError::Refused(
                "streamed canonical member projection bound",
            ));
        }
        if epoch.fresh_sink {
            for table in ["cmd2_current", "cmd2_history"] {
                set_streamed_pg_limits(epoch.tx, epoch.max_statement_ms, deadline, cancelled)?;
                ManagedSourceWorkV1::charge(
                    &mut epoch.body_work.fresh_pg_field_bytes_attempted,
                    (epoch.domain.len() + member.path.len() + projection.len() + ORIGINAL.len())
                        as u64,
                )?;
                let query = format!(
                    "UPDATE {table} SET inventory_projection=$3 WHERE domain=$1 AND subject=$2 AND revision=1 AND profile_id=$4 AND inventory_projection IS NULL"
                );
                let written = epoch.tx.execute(
                    &query,
                    &[&epoch.domain, &member.path, &projection, &ORIGINAL],
                )?;
                ManagedSourceWorkV1::charge(&mut epoch.body_work.fresh_pg_rows_written, written)?;
                if written != 1 {
                    return Err(DurableError::Corrupt(
                        "fresh streamed projection target differs",
                    ));
                }
            }
        }
        set_streamed_pg_limits(epoch.tx, epoch.max_statement_ms, deadline, cancelled)?;
        let returned = epoch.tx.query_one(
            "SELECT CASE WHEN octet_length(c.inventory_projection)<=1048576 THEN c.inventory_projection ELSE NULL END,octet_length(c.inventory_projection),CASE WHEN octet_length(h.inventory_projection)<=1048576 THEN h.inventory_projection ELSE NULL END,octet_length(h.inventory_projection) FROM cmd2_current c LEFT JOIN cmd2_history h USING(domain,subject,revision) WHERE c.domain=$1 AND c.subject=$2",
            &[&epoch.domain, &member.path],
        )?;
        let live: Option<Vec<u8>> = returned.get(0);
        let retained: Option<Vec<u8>> = returned.get(2);
        ManagedSourceWorkV1::charge(
            &mut epoch.body_work.current_projection_query_rows_returned,
            1,
        )?;
        ManagedSourceWorkV1::charge(
            &mut epoch
                .body_work
                .current_and_history_projection_bytes_returned,
            live.as_ref().map_or(0, |raw| raw.len() as u64),
        )?;
        ManagedSourceWorkV1::charge(
            &mut epoch
                .body_work
                .current_and_history_projection_bytes_returned,
            retained.as_ref().map_or(0, |raw| raw.len() as u64),
        )?;
        if returned
            .get::<_, Option<i32>>(1)
            .is_none_or(|n| n < 0 || n as usize != projection.len())
            || returned
                .get::<_, Option<i32>>(3)
                .is_none_or(|n| n < 0 || n as usize != projection.len())
            || live.as_deref() != Some(projection.as_slice())
            || retained.as_deref() != Some(projection.as_slice())
        {
            return Err(DurableError::Corrupt(
                "streamed current/retained projection differs from source",
            ));
        }
        part(&mut transcript, member.path.as_bytes());
        part(&mut transcript, Digest256::of_bytes(&projection).as_bytes());
        index.borrow_mut().add_projection(
            &AssessmentProjectionRow {
                path: member.path.clone(),
                value: Some(projection),
            },
            &mut **work.borrow_mut(),
        )?;
        after = Some(member.path);
        count = count.checked_add(1).ok_or(DurableError::Refused(
            "streamed projection pass count overflow",
        ))?;
    }
    if count != first.members {
        return Err(DurableError::Corrupt(
            "streamed complete projection pass EOF differs",
        ));
    }
    epoch
        .index
        .borrow_mut()
        .finish_source_inputs(&mut **epoch.index_work.borrow_mut())?;
    active(deadline, cancelled)?;
    Ok(CompleteAgentProjectionPassV1 {
        members: count,
        projection_digest: transcript.finalize(),
    })
}

struct CompleteStreamedOwnerAssessmentV1 {
    members: u64,
    projection_digest: Digest256,
}

fn compare_current_owner_indexes(
    tx: &mut Transaction<'_>,
    max_statement_ms: u64,
    domain: &str,
    index: &mut SourceAssessmentIndex,
    work: &mut SourceAssessmentWork,
    original: &tos_source_store::StreamedCorpusCutReaderV1,
    verified_original: &VerifiedStreamedOriginalContentV1,
    projected: &CompleteAgentProjectionPassV1,
    source_index_rows: u64,
    owned_predicate_rows: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<CompleteStreamedOwnerAssessmentV1> {
    use super::source_cohort_stream_index::{
        AssessmentIndexRow, AssessmentPredicateRow, AssessmentProjectionRow,
    };
    active(deadline, cancelled)?;
    let version = definition().to_hex();
    let mut projection_rows = 0u64;
    cold_source_key_rows_controlled(
        tx,
        Some(max_statement_ms),
        domain,
        false,
        source_index_rows,
        deadline,
        cancelled,
        |row| {
            let kind: String = row.get("kind");
            let path: String = row.get("path");
            let definition_digest: String = row.get("definition_digest");
            if definition_digest != version {
                return Err(DurableError::Corrupt(
                    "streamed current owner index definition differs",
                ));
            }
            index.observe_index_row(
                &AssessmentIndexRow {
                    kind: kind.clone(),
                    token: row.get("token"),
                    path: path.clone(),
                    definition_digest,
                },
                work,
            )?;
            let projection: Option<Vec<u8>> = row.get("inventory_projection");
            if kind == "path" {
                index.observe_projection(
                    &AssessmentProjectionRow {
                        path,
                        value: projection,
                    },
                    work,
                )?;
                projection_rows = projection_rows.checked_add(1).ok_or(DurableError::Refused(
                    "streamed index projection count overflow",
                ))?;
            } else if projection.is_some() {
                return Err(DurableError::Corrupt(
                    "streamed auxiliary index projection unexpected",
                ));
            }
            Ok(())
        },
    )?;
    if projection_rows != projected.members {
        return Err(DurableError::Corrupt(
            "streamed owner projection membership differs",
        ));
    }
    index.finish_index_comparison(work)?;
    index.finish_projection_comparison(work)?;
    cold_source_key_rows_controlled(
        tx,
        Some(max_statement_ms),
        domain,
        true,
        owned_predicate_rows,
        deadline,
        cancelled,
        |row| {
            let kind: String = row.get("kind");
            let scope: String = row.get("scope");
            let token: String = row.get("token");
            let owner: String = row.get("owner");
            let definition_version: String = row.get("definition_version");
            if owner != OWNER
                || definition_version != version
                || !["unique", "range"].contains(&kind.as_str())
                || (kind == "range"
                    && !["source-home", "source-inventory"].contains(&scope.as_str()))
                || (scope == "source-inventory" && token != "all")
                || (kind == "unique"
                    && !["metadata", "claim", "event", "anchor", "form", "path"]
                        .contains(&scope.as_str()))
            {
                return Err(DurableError::Corrupt(
                    "streamed stored predicate owner definition differs",
                ));
            }
            index.observe_predicate_row(
                &AssessmentPredicateRow {
                    kind,
                    owner,
                    scope,
                    token,
                    definition_version,
                },
                work,
            )
        },
    )?;
    index.finish_predicate_comparison(work)?;
    // Declared manifest identity and regenerated owner identity remain separate
    // layers: every declared claim resolves to its exact source-owned path.
    let mut after = None::<String>;
    while let Some((id, path)) = original
        .identity_after(verified_original.revision, after.as_deref())
        .map_err(|_| DurableError::Refused("streamed original identity claim cursor"))?
    {
        active(deadline, cancelled)?;
        if index.identity_path(&id, work)?.as_deref() != Some(path.as_str()) {
            return Err(DurableError::Refused(
                "manifest identity lacks exact maintained owner meaning",
            ));
        }
        after = Some(id);
    }
    active(deadline, cancelled)?;
    Ok(CompleteStreamedOwnerAssessmentV1 {
        members: projected.members,
        projection_digest: projected.projection_digest,
    })
}

// The descriptor is built only after complete source-owned semantic/index
// comparison. The private witness is not a currentness or publication grant;
// the coordinator must still hold and repeat its domain/audit/rights gates.
fn build_assessed_inventory(
    store: &SegmentStore,
    assessment: &CompleteStreamedOwnerAssessmentV1,
    index: &mut SourceAssessmentIndex,
    index_work: &mut SourceAssessmentWork,
    limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    tree_work: &mut tos_segment_store::AuthenticatedTreeWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<super::addressed_inventory::AddressedInventoryV2> {
    let mut after = None::<String>;
    let mut ended = false;
    let projections = std::iter::from_fn(|| {
        if ended {
            return None;
        }
        let input = (|| -> DurableResult<Option<(String, Vec<u8>)>> {
            active(deadline, cancelled)?;
            let Some(row) = index.projection_after(after.as_deref(), index_work)? else {
                return Ok(None);
            };
            let raw = row.value.ok_or(DurableError::Corrupt(
                "complete streamed assessment projection absent",
            ))?;
            after = Some(row.path.clone());
            active(deadline, cancelled)?;
            Ok(Some((row.path, raw)))
        })();
        match input {
            Ok(Some(row)) => Some(Ok(row)),
            Ok(None) => {
                ended = true;
                None
            }
            Err(error) => {
                ended = true;
                Some(Err(error))
            }
        }
    });
    super::addressed_inventory::AddressedInventoryV2::build_from_assessed_projections(
        store,
        projections,
        assessment.members,
        limits,
        deadline,
        cancelled,
        tree_work,
    )
}

// Aggregate admission is a resource boundary only. Exact rows are subsequently
// consumed by the existing bounded PG page cursors and compared to regenerated
// semantics; the aggregate cannot grant completeness.
fn admit_streamed_owner_metadata(
    tx: &mut Transaction<'_>,
    max_statement_ms: u64,
    domain: &str,
    limits: SourceAssessmentLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(u64, u64)> {
    let mut total_rows = 0u64;
    let mut total_bytes = 0u64;
    let mut counts = [0u64; 2];
    for (position, table) in ["cmd2_source_index", "cmd2_predicate"]
        .into_iter()
        .enumerate()
    {
        active(deadline, cancelled)?;
        let scan_rows = limits
            .max_rows
            .checked_sub(total_rows)
            .and_then(|n| n.checked_add(1))
            .ok_or(DurableError::Refused("streamed owner metadata row bound"))?;
        let query = format!(
            "SELECT count(*),coalesce(max(octet_length(row_to_json(t)::text)),0)::bigint,coalesce(sum(octet_length(row_to_json(t)::text)),0)::bigint FROM (SELECT * FROM {table} WHERE domain=$1 LIMIT $2) t"
        );
        set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let row = tx.query_one(&query, &[&domain, &as_i64(scan_rows)?])?;
        counts[position] = as_u64(row.get(0))?;
        total_rows = total_rows
            .checked_add(counts[position])
            .ok_or(DurableError::Refused(
                "streamed owner metadata row overflow",
            ))?;
        total_bytes = total_bytes
            .checked_add(as_u64(row.get(2))?)
            .ok_or(DurableError::Refused(
                "streamed owner metadata byte overflow",
            ))?;
        if total_rows > limits.max_rows
            || total_bytes > limits.max_logical_bytes
            || as_u64(row.get(1))? > limits.max_value_bytes as u64
        {
            return Err(DurableError::Refused(
                "streamed owner metadata preflight bounds",
            ));
        }
    }
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    let owned = as_u64(
        tx.query_one(
            "SELECT count(*) FROM cmd2_predicate WHERE domain=$1 AND owner=$2",
            &[&domain, &OWNER],
        )?
        .get(0),
    )?;
    active(deadline, cancelled)?;
    Ok((counts[0], owned))
}

// Only called after the fresh seed's held fence and complete semantic seal.
// Expected rows are derived from real source validators; PG readback still
// runs through the same strict comparison as ordinary cold selection.
fn populate_fresh_owner_rows(
    tx: &mut Transaction<'_>,
    max_statement_ms: u64,
    domain: &str,
    index: &mut SourceAssessmentIndex,
    index_work: &mut SourceAssessmentWork,
    body_work: &mut StreamedColdSourceWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    let mut after: Option<(String, String)> = None;
    while let Some(row) = index.expected_index_after(
        after.as_ref().map(|key| (key.0.as_str(), key.1.as_str())),
        index_work,
    )? {
        set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let bytes = [
            domain,
            &row.kind,
            &row.token,
            &row.path,
            &row.definition_digest,
        ]
        .iter()
        .try_fold(0u64, |sum, value| sum.checked_add(value.len() as u64))
        .ok_or(DurableError::Refused("fresh index bound fields overflow"))?;
        ManagedSourceWorkV1::charge(&mut body_work.fresh_pg_field_bytes_attempted, bytes)?;
        let written = tx.execute("INSERT INTO cmd2_source_index(domain,kind,token,path,definition_digest) VALUES($1,$2,$3,$4,$5)",
            &[&domain, &row.kind, &row.token, &row.path, &row.definition_digest])?;
        ManagedSourceWorkV1::charge(&mut body_work.fresh_pg_rows_written, written)?;
        if written != 1 {
            return Err(DurableError::Corrupt("fresh source index insert differs"));
        }
        after = Some((row.kind, row.token));
    }
    let mut after: Option<(String, String, String, String)> = None;
    while let Some(row) = index.expected_predicate_after(
        after.as_ref().map(|key| {
            (
                key.0.as_str(),
                key.1.as_str(),
                key.2.as_str(),
                key.3.as_str(),
            )
        }),
        index_work,
    )? {
        if row.owner != OWNER {
            return Err(DurableError::Corrupt("fresh predicate owner differs"));
        }
        set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let bytes = [
            domain,
            &row.kind,
            &row.owner,
            &row.scope,
            &row.token,
            &row.definition_version,
        ]
        .iter()
        .try_fold(0u64, |sum, value| sum.checked_add(value.len() as u64))
        .ok_or(DurableError::Refused(
            "fresh predicate bound fields overflow",
        ))?;
        ManagedSourceWorkV1::charge(&mut body_work.fresh_pg_field_bytes_attempted, bytes)?;
        let written = tx.execute("INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete) VALUES($1,$2,$3,$4,$5,$6,0,false)",
            &[&domain,&row.kind,&row.owner,&row.scope,&row.token,&row.definition_version])?;
        ManagedSourceWorkV1::charge(&mut body_work.fresh_pg_rows_written, written)?;
        if written != 1 {
            return Err(DurableError::Corrupt(
                "fresh source predicate insert differs",
            ));
        }
        after = Some((row.kind, row.owner, row.scope, row.token));
    }
    let mut after = None;
    while let Some(row) = index.projection_after(after.as_deref(), index_work)? {
        let projection = row
            .value
            .ok_or(DurableError::Corrupt("fresh expected projection absent"))?;
        set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        ManagedSourceWorkV1::charge(
            &mut body_work.fresh_pg_field_bytes_attempted,
            (domain.len() + row.path.len() + projection.len() + 64) as u64,
        )?;
        let written = tx.execute("UPDATE cmd2_source_index SET inventory_projection=$3 WHERE domain=$1 AND kind='path' AND token=$2 AND path=$2 AND definition_digest=$4 AND inventory_projection IS NULL",
            &[&domain,&row.path,&projection,&definition().to_hex()])?;
        ManagedSourceWorkV1::charge(&mut body_work.fresh_pg_rows_written, written)?;
        if written != 1 {
            return Err(DurableError::Corrupt(
                "fresh projection index target differs",
            ));
        }
        after = Some(row.path);
    }
    active(deadline, cancelled)
}

// This is the actual ordered assessment kernel for the additive coordinator
// endpoint. The caller retains the exact cold-root pin and the RR audit/domain
// rights locks throughout this function and repeats them before publication.
fn assess_held_current_source(
    tx: &mut Transaction<'_>,
    max_statement_ms: u64,
    fresh_sink: bool,
    store: &SegmentStore,
    domain: &str,
    cold: &ColdCut,
    original: &tos_source_store::StreamedCorpusCutReaderV1,
    original_content: &VerifiedStreamedOriginalContentV1,
    context: &CommandContext,
    index: &mut SourceAssessmentIndex,
    limits: SourceAssessmentLimits,
    index_work: &mut SourceAssessmentWork,
    body_work: &mut StreamedColdSourceWorkV1,
    worker: &mut CutWorkerSchemaExecutor,
    effective_uid: u64,
    schema_source_path: &str,
    item_limits: tos_validation::item_rules::ItemLimits,
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    tree_work: &mut tos_segment_store::AuthenticatedTreeWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(
    CompleteStreamedOwnerAssessmentV1,
    super::addressed_inventory::AddressedInventoryV2,
)> {
    let (mut source_rows, mut predicate_rows) =
        admit_streamed_owner_metadata(tx, max_statement_ms, domain, limits, deadline, cancelled)?;
    if fresh_sink && (source_rows != 0 || predicate_rows != 0) {
        return Err(DurableError::Conflict(
            "fresh streamed owner rows already populated",
        ));
    }
    index_current_source(
        tx,
        max_statement_ms,
        store,
        domain,
        cold,
        original,
        original_content,
        index,
        limits,
        index_work,
        deadline,
        cancelled,
    )?;
    let projected = {
        let index_cell = RefCell::new(&mut *index);
        let work_cell = RefCell::new(&mut *index_work);
        let mut epoch = StreamedSourceReadEpoch {
            tx,
            max_statement_ms,
            fresh_sink,
            index: &index_cell,
            index_work: &work_cell,
            body_work,
            store,
            context,
            domain,
            cold,
        };
        let profile = crate::source_claims::prepare_agent_inventory_assessment_profile(
            &mut epoch,
            original_content.revision,
            effective_uid,
            schema_source_path,
            worker,
            deadline,
            cancelled,
        )
        .map_err(source_error)?;
        let first = stage_current_agent_identities(&mut epoch, &profile, deadline, cancelled)?;
        render_current_agent_projections(
            &mut epoch,
            &profile,
            first,
            worker,
            item_limits,
            deadline,
            cancelled,
        )?
    };
    if fresh_sink {
        populate_fresh_owner_rows(
            tx,
            max_statement_ms,
            domain,
            index,
            index_work,
            body_work,
            deadline,
            cancelled,
        )?;
        (source_rows, predicate_rows) = admit_streamed_owner_metadata(
            tx,
            max_statement_ms,
            domain,
            limits,
            deadline,
            cancelled,
        )?;
    }
    let assessment = compare_current_owner_indexes(
        tx,
        max_statement_ms,
        domain,
        index,
        index_work,
        original,
        original_content,
        &projected,
        source_rows,
        predicate_rows,
        deadline,
        cancelled,
    )?;
    // A successful semantic pass cannot outlive a failed validator child.
    finish_worker(worker, deadline, cancelled)?;
    let inventory = build_assessed_inventory(
        store,
        &assessment,
        index,
        index_work,
        tree_limits,
        tree_work,
        deadline,
        cancelled,
    )?;
    active(deadline, cancelled)?;
    Ok((assessment, inventory))
}

/// Explicit derived-index limits; these do not enlarge compatibility admission
/// or grant authority. The workspace and generation profile retain their own
/// independent physical limits.
#[derive(Clone, Copy, Debug)]
pub struct StreamedColdSourceLimitsV1 {
    pub max_rows: u64,
    pub max_logical_bytes: u64,
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
    pub max_placement_bytes: usize,
    pub max_sqlite_file_bytes: u64,
    pub max_vm_steps: u64,
}
impl StreamedColdSourceLimitsV1 {
    pub(super) fn index_limits(self) -> DurableResult<SourceAssessmentLimits> {
        SourceAssessmentLimits {
            max_rows: self.max_rows,
            max_logical_bytes: self.max_logical_bytes,
            max_key_bytes: self.max_key_bytes,
            max_value_bytes: self.max_value_bytes,
            max_placement_bytes: self.max_placement_bytes,
            max_sqlite_file_bytes: self.max_sqlite_file_bytes,
            max_vm_steps: self.max_vm_steps,
        }
        .validate()
    }
}

impl DurablePgCoordinator {
    /// Additive current Agent cold assessment. Exact V1 ORIGINAL is retained;
    /// current bytes are regenerated and checked without complete Rust maps.
    /// The returned addressed handle reaches the existing managed model gates,
    /// not public corpus admission or an independent rights grant.
    pub fn select_current_source_generation_addressed_streaming(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        original: &tos_source_store::StreamedCorpusCutReaderV1,
        initial_revision: SourceRevision,
        initial_membership: SourceMembershipV1,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        schema_source_path: &str,
        workspace: &super::PrivateGenerationWorkspace,
        generation_profile: super::StreamedGenerationProfile,
        source_limits: StreamedColdSourceLimitsV1,
        item_limits: tos_validation::item_rules::ItemLimits,
        tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
        body_work: &mut StreamedColdSourceWorkV1,
        tree_work: &mut tos_segment_store::AuthenticatedTreeWorkV1,
        deadline: Instant,
        cancelled: std::sync::Arc<AtomicBool>,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        self.complete_streamed_source_generation(
            store,
            None,
            domain,
            original,
            initial_revision,
            initial_membership,
            context,
            software,
            components,
            worker,
            schema_source_path,
            workspace,
            generation_profile,
            source_limits,
            item_limits,
            tree_limits,
            body_work,
            tree_work,
            deadline,
            cancelled,
        )
    }

    pub(super) fn complete_fresh_source_bootstrap_streamed(
        &mut self,
        store: &SegmentStore,
        seed: super::source_cohort_bootstrap_streamed::FreshSourceBootstrapV1,
        original: &tos_source_store::StreamedCorpusCutReaderV1,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        schema_source_path: &str,
        workspace: &PrivateGenerationWorkspace,
        generation_profile: StreamedGenerationProfile,
        source_limits: StreamedColdSourceLimitsV1,
        item_limits: tos_validation::item_rules::ItemLimits,
        tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
        body_work: &mut StreamedColdSourceWorkV1,
        tree_work: &mut tos_segment_store::AuthenticatedTreeWorkV1,
        deadline: Instant,
        cancelled: std::sync::Arc<AtomicBool>,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        let observed = seed.observation();
        let domain = observed.domain.clone();
        let revision = observed.revision;
        let membership = observed.membership;
        self.complete_streamed_source_generation(
            store,
            Some(seed),
            &domain,
            original,
            revision,
            membership,
            context,
            software,
            components,
            worker,
            schema_source_path,
            workspace,
            generation_profile,
            source_limits,
            item_limits,
            tree_limits,
            body_work,
            tree_work,
            deadline,
            cancelled,
        )
    }

    fn complete_streamed_source_generation(
        &mut self,
        store: &SegmentStore,
        seed: Option<super::source_cohort_bootstrap_streamed::FreshSourceBootstrapV1>,
        domain: &str,
        original: &tos_source_store::StreamedCorpusCutReaderV1,
        initial_revision: SourceRevision,
        initial_membership: SourceMembershipV1,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        schema_source_path: &str,
        workspace: &super::PrivateGenerationWorkspace,
        generation_profile: super::StreamedGenerationProfile,
        source_limits: StreamedColdSourceLimitsV1,
        item_limits: tos_validation::item_rules::ItemLimits,
        tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
        body_work: &mut StreamedColdSourceWorkV1,
        tree_work: &mut tos_segment_store::AuthenticatedTreeWorkV1,
        deadline: Instant,
        cancelled: std::sync::Arc<AtomicBool>,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        let mut index_work = SourceAssessmentWork::default();
        let result =
            (|| -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
                active(deadline, &cancelled)?;
                let limits = source_limits.index_limits()?;
                let generation_profile = generation_profile.validate(workspace)?;
                if context.base_revision != initial_revision
                    || store.custody_domain() != domain.as_bytes()
                {
                    return Err(DurableError::Conflict(
                        "streamed selected source context differs",
                    ));
                }
                context
                    .check_selected_software_inputs(software, components, deadline, &cancelled)
                    .map_err(source_error)?;
                let original_content = verify_original_content(
                    original,
                    initial_revision,
                    initial_membership,
                    8_388_608,
                    deadline,
                    &cancelled,
                    body_work,
                )?;
                let before = self.cold_verify_cut_streamed(
                    store,
                    domain,
                    workspace,
                    generation_profile,
                    deadline,
                    &cancelled,
                )?;
                if let Some(seed) = seed.as_ref() {
                    let observed = seed.observation();
                    if observed.domain != domain
                        || observed.store_id != store.store_id()
                        || observed.revision != initial_revision
                        || observed.membership != initial_membership
                        || observed.audit != before.audit_generation
                        || observed.head != before.through_commit_seq
                        || observed.authored_members != before.current_members
                        || before.historical_members != before.current_members
                    {
                        return Err(DurableError::Conflict(
                            "fresh streamed seed/cold fence differs",
                        ));
                    }
                }
                let mut index = SourceAssessmentIndex::open(
                    workspace,
                    limits,
                    deadline,
                    std::sync::Arc::clone(&cancelled),
                    &mut index_work,
                )?;
                let mut tx = self
                    .client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .start()?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                tx.batch_execute("SET LOCAL work_mem='4MB'")?;
                tx.query_one(
                    "SELECT set_config('temp_file_limit',$1,true)",
                    &[&format!(
                        "{}kB",
                        generation_profile.max_pg_temp_bytes / 1024
                    )],
                )?;
                if lock_audit_fence(&mut tx, domain)? != before.audit_generation {
                    return Err(DurableError::Conflict("streamed source audit changed"));
                }
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                let row = tx.query_one(
                    "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
                    &[&domain],
                )?;
                let cohort = ManagedSourceCohort {
                    domain: domain.into(),
                    store_id: store.store_id(),
                    initial_revision,
                    initial_membership,
                    epoch: as_u64(
                        row.get::<_, Option<i64>>("source_epoch")
                            .ok_or(DurableError::Corrupt("streamed source epoch absent"))?,
                    )?,
                    definition: definition(),
                    generation: before.through_commit_seq,
                };
                cohort_matches(&row, &cohort, false)?;
                if let Some(seed) = seed.as_ref() {
                    let observed = seed.observation();
                    if row.get::<_, bool>("source_complete")
                        || row.get::<_, Option<i64>>("source_epoch") != Some(1)
                        || row.get::<_, i64>("rights_version") != 0
                        || row.get::<_, i64>("rule_version") != 0
                        || row.get::<_, String>("contract_digest") != observed.contract.to_hex()
                        || row
                            .get::<_, Option<String>>("selected_generation_digest")
                            .is_some()
                        || row
                            .get::<_, Option<String>>("source_projection_digest")
                            .is_some()
                    {
                        return Err(DurableError::Conflict("fresh streamed seed header differs"));
                    }
                }

                if !row.get::<_, bool>("rights_allowed")
                    || as_u64(row.get("head_seq"))? != before.through_commit_seq
                    || {
                        set_streamed_pg_limits(
                            &mut tx,
                            generation_profile.max_sql_statement_ms,
                            deadline,
                            &cancelled,
                        )?;
                        database_oid(&mut tx)? != before.database_oid
                    }
                {
                    return Err(DurableError::Conflict(
                        "streamed source rights/head/database changed",
                    ));
                }
                let (assessment, inventory) = assess_held_current_source(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    seed.is_some(),
                    store,
                    domain,
                    &before,
                    original,
                    &original_content,
                    context,
                    &mut index,
                    limits,
                    &mut index_work,
                    body_work,
                    worker,
                    context.effective_uid,
                    schema_source_path,
                    item_limits,
                    tree_limits,
                    tree_work,
                    deadline,
                    &cancelled,
                )?;
                // Locks remain held. Completion includes empty and absent predicates;
                // only existing owned definitions are re-enabled, never invented grants.
                active(deadline, &cancelled)?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                tx.execute("UPDATE cmd2_predicate SET complete=true WHERE domain=$1 AND owner=$2 AND definition_version=$3",
            &[&domain, &OWNER, &cohort.definition.to_hex()])?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                tx.execute("UPDATE cmd2_domain SET source_complete=true,source_generation=head_seq,source_projection_digest=$2,selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL WHERE domain=$1",
            &[&domain, &assessment.projection_digest.to_hex()])?;
                active(deadline, &cancelled)?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                let completed_audit = lock_audit_fence(&mut tx, domain)?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                tx.commit()?;
                let verified = self.cold_verify_cut_streamed(
                    store,
                    domain,
                    workspace,
                    generation_profile,
                    deadline,
                    &cancelled,
                )?;
                if verified.audit_generation != completed_audit
                    || (seed.is_none()
                        && (verified.history_membership_root != before.history_membership_root
                            || verified.current_membership_root != before.current_membership_root))
                    || verified.historical_members != before.historical_members
                    || verified.current_members != before.current_members
                    || verified.log_digest != before.log_digest
                {
                    return Err(DurableError::Conflict(
                        "streamed completed assessment raced mutation",
                    ));
                }
                let mut tx = self
                    .client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .start()?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                tx.batch_execute("SET LOCAL work_mem='4MB'")?;
                if lock_audit_fence(&mut tx, domain)? != verified.audit_generation {
                    return Err(DurableError::Conflict("streamed addressed audit changed"));
                }
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                let row = tx.query_one(
                    "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
                    &[&domain],
                )?;
                cohort_matches(&row, &cohort, true)?;
                if !row.get::<_, bool>("rights_allowed")
                    || as_u64(row.get("head_seq"))? != verified.through_commit_seq
                    || verified.through_commit_seq != before.through_commit_seq
                    || verified.current_members != assessment.members
                    || {
                        set_streamed_pg_limits(
                            &mut tx,
                            generation_profile.max_sql_statement_ms,
                            deadline,
                            &cancelled,
                        )?;
                        database_oid(&mut tx)? != verified.database_oid
                    }
                {
                    return Err(DurableError::Conflict("streamed addressed source changed"));
                }
                super::audit_delta::activate_domain_controlled(
                    &mut tx,
                    domain,
                    verified.audit_generation,
                    super::audit_delta::audit_delta_schema_digest(),
                    Some(super::audit_delta::AuditDeltaControl {
                        deadline,
                        cancelled: &cancelled,
                    }),
                )
                .map_err(super::addressed_successor::delta_error)?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                let metadata = super::addressed_successor::build_addressed_metadata_tree(
                    &mut tx,
                    store,
                    domain,
                    tree_limits,
                    deadline,
                    &cancelled,
                )?;
                let (history, current) =
                    super::addressed_successor::build_addressed_membership_trees(
                        store,
                        &verified.installation(store),
                        tree_limits,
                        deadline,
                        &cancelled,
                    )?;
                let cut = super::addressed_successor::AddressedCutV2 {
                    store_id: store.store_id(),
                    domain_digest: store.domain_digest(),
                    domain: domain.into(),
                    through_seq: verified.through_commit_seq,
                    audit_generation: verified.audit_generation,
                    database_oid: verified.database_oid,
                    schema_profile_digest: schema_profile_digest(),
                    state_profile_digest: super::audit_delta::audit_delta_schema_digest(),
                    log_digest: verified.log_digest,
                    metadata,
                    history,
                    current,
                    inventory,
                };
                active(deadline, &cancelled)?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                tx.commit()?;
                let candidate = self.install_addressed_selection(
                    store,
                    cut,
                    VerifiedLogFrontier(verified.log_frontier.0.clone()),
                    tree_limits,
                    deadline,
                    &cancelled,
                )?;
                let selected = self.select_addressed_selection_controlled(
                    store,
                    candidate,
                    &cohort,
                    None,
                    Some(generation_profile.max_sql_statement_ms),
                    deadline,
                    &cancelled,
                )?;
                Ok(
            crate::source_current_cut::ManagedCurrentSourceGeneration::from_verified_addressed(
                store, cohort, selected,
            ),
        )
            })();
        // Work is copied on both success and refusal, including operations
        // already completed before a semantic/cancellation/storage failure.
        body_work.derived_index = index_work;
        result
    }
}

impl super::PrivateGenerationWorkspace {
    /// Open the additive V1 source reader on a fresh private workspace inode.
    /// Charging the full index ceiling reserves scratch capacity before any
    /// SQLite write; it is a budget charge, not measured physical IO.
    pub fn open_streamed_source_cut(
        &self,
        source: &tos_source_store::CorpusReader,
        revision: SourceRevision,
        limits: tos_source_store::StreamedCutReadLimitsV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        work: &mut StreamedColdSourceWorkV1,
    ) -> DurableResult<tos_source_store::StreamedCorpusCutReaderV1> {
        use std::os::unix::fs::MetadataExt;
        active(deadline, cancelled)?;
        self.charge_private_bytes(limits.max_index_bytes)?;
        let file = self.new_private_file()?;
        let observation = file
            .try_clone()
            .map_err(|_| DurableError::Refused("streamed source index observation handle"))?;
        let result = source.open_source_cut_streamed(revision, limits, file, deadline, cancelled);
        let metadata = observation
            .metadata()
            .map_err(|_| DurableError::Refused("streamed source index file observation"))?;
        work.original_index_length_observed = metadata.len();
        work.original_index_allocated_observed =
            metadata
                .blocks()
                .checked_mul(512)
                .ok_or(DurableError::Refused(
                    "streamed source index allocation overflow",
                ))?;
        // StoreError.detail is a static operation/category, never raw SQL,
        // payload, path, or an underlying IO error message.
        let reader = result.map_err(|error| DurableError::Refused(error.detail))?;
        active(deadline, cancelled)?;
        Ok(reader)
    }
}
