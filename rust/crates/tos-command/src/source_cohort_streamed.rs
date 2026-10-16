//! Additive cold mixed-public-metadata assessment over complete verified source membership.
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
    pub predicate_completion_rows_updated: u64,
    pub predicate_completion_batches: u64,
    pub completed_assessments_reused: u64,
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
    max_member_bytes: u64,
    store: &'a SegmentStore,
    context: &'a CommandContext,
    domain: &'a str,
    // The caller holds this exact cold-root custody and audit/domain rights
    // locks for the entire read epoch, then repeats the final source fences.
    cold: &'a ColdCut,
    fresh_sink: bool,
    rebuild_projections: bool,
    public_text_packages: &'a [super::CurrentPublicTextPackageV1],
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
            if max_bytes == 0
                || self.max_member_bytes == 0
                || self.max_member_bytes > usize::MAX as u64
            {
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
            // The caller supplies a hard maximum for this read; the selected
            // source profile independently caps every authored member. A
            // caller may request a wider ceiling than the cut, but neither
            // bound is relaxed for bytes actually returned.
            let effective_max_bytes = (max_bytes as u64).min(self.max_member_bytes);
            if member.size_bytes > effective_max_bytes {
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
                .read_selected(receipt, effective_max_bytes, &mut raw)?;
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
pub(super) struct VerifiedStreamedOriginalContentV1 {
    revision: SourceRevision,
    membership: SourceMembershipV1,
    pub(super) authored_members: u64,
}

pub(super) fn verify_original_content(
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
        || max_member_bytes > usize::MAX as u64
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

// Resource admission for the fields returned by the binary current-row cursor.
// A bytea projection is not its doubled hexadecimal JSON transport encoding.
// This aggregate bounds the input; cold membership and exact lineage below
// remain responsible for integrity and source authority.
fn admit_streamed_current_metadata(
    tx: &mut Transaction<'_>,
    max_statement_ms: u64,
    domain: &str,
    expected_members: u64,
    limits: SourceAssessmentLimits,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    if expected_members > limits.max_rows
        || max_member_bytes == 0
        || max_member_bytes > usize::MAX as u64
    {
        return Err(DurableError::Refused(
            "streamed current input preflight bounds",
        ));
    }
    let scan_rows = expected_members
        .checked_add(1)
        .ok_or(DurableError::Refused("streamed current input row bound"))?;
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    let admitted = tx.query_one(
        "SELECT count(*),coalesce(max(content_length),0),
         coalesce(max(greatest(octet_length(domain),octet_length(subject),octet_length(profile_id),octet_length(profile_version),octet_length(content_digest),octet_length(custody_domain_digest),octet_length(segment_digest),octet_length(frame_digest),octet_length(sto_receipt_id),octet_length(durability_class),dependency_key_bytes)),0)::bigint,
         coalesce(max(greatest(coalesce(octet_length(inventory_projection),0),octet_length(prepare_id),octet_length(store_id),octet_length(custody_domain),octet_length(pin_id),dependency_bytes)),0)::bigint,
         coalesce(sum(octet_length(domain)::bigint+octet_length(subject)+octet_length(profile_id)+octet_length(profile_version)+octet_length(content_digest)+octet_length(custody_domain_digest)+octet_length(segment_digest)+octet_length(frame_digest)+octet_length(sto_receipt_id)+octet_length(durability_class)+octet_length(prepare_id)+octet_length(store_id)+octet_length(custody_domain)+octet_length(pin_id)+coalesce(octet_length(inventory_projection),0)+dependency_bytes+76),0)::bigint
         FROM (SELECT * FROM cmd2_current WHERE domain=$1 LIMIT $2) c
         CROSS JOIN LATERAL (SELECT coalesce(sum(octet_length(d)::bigint+4),0)::bigint AS dependency_bytes,coalesce(max(octet_length(d)),0) AS dependency_key_bytes FROM unnest(c.source_dependencies) d) dependencies",
        &[&domain, &as_i64(scan_rows)?],
    )?;
    // 76 counts the fixed-width integer fields, including the joined attempt
    // fence, conservatively retaining four bytes for an absent source mode.
    if as_u64(admitted.get(0))? != expected_members
        || as_u64(admitted.get(1))? > max_member_bytes
        || as_u64(admitted.get(2))? > limits.max_key_bytes as u64
        || as_u64(admitted.get(3))? > limits.max_value_bytes as u64
        || as_u64(admitted.get(4))? > limits.max_logical_bytes
    {
        return Err(DurableError::Refused(
            "streamed current input preflight bounds",
        ));
    }
    Ok(())
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
    max_member_bytes: u64,
    index_work: &mut SourceAssessmentWork,
    body_work: &mut StreamedColdSourceWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    active(deadline, cancelled)?;
    cold.audited_root.require_store(store)?;
    admit_streamed_current_metadata(
        tx,
        max_statement_ms,
        domain,
        cold.current_members,
        limits,
        max_member_bytes,
        deadline,
        cancelled,
    )?;
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
            let original_metadata = if let Some(member) = &original_member {
                originals = originals
                    .checked_add(1)
                    .ok_or(DurableError::Refused("streamed original count overflow"))?;
                let has_dependencies = original
                    .has_dependency_index(original_content.revision, &path)
                    .map_err(|_| {
                        DurableError::Refused("streamed original dependency key lookup")
                    })?;
                let mut dependencies = Vec::new();
                let mut bytes = 0usize;
                let mut after = None;
                while let Some(target) = original
                    .dependency_after(original_content.revision, &path, after.as_ref())
                    .map_err(|_| {
                        DurableError::Refused("streamed original dependency target lookup")
                    })?
                {
                    active(deadline, cancelled)?;
                    bytes = bytes
                        .checked_add(target.as_str().len() + std::mem::size_of::<String>())
                        .filter(|value| *value <= limits.max_value_bytes)
                        .ok_or(DurableError::Refused(
                            "streamed original dependency byte bound",
                        ))?;
                    dependencies.push(target.as_str().to_owned());
                    after = Some(target);
                }
                if !has_dependencies && !dependencies.is_empty() {
                    return Err(DurableError::Corrupt(
                        "streamed original dependency presence differs",
                    ));
                }
                Some(SourceFileMetadata {
                    mode: member.mode,
                    dependencies: has_dependencies.then_some(dependencies),
                })
            } else {
                None
            };
            if receipt.binding().profile_id == ORIGINAL {
                // Verified ORIGINAL content stays exact, including carrier metadata.
                if original_member.as_ref().is_none_or(|member| {
                    member.sha256 != metadata.sha256 || member.size_bytes != metadata.size_bytes
                }) || original_metadata.as_ref() != Some(&carrier)
                {
                    return Err(DurableError::Corrupt("streamed original carrier changed"));
                }
            } else {
                // One current/initial pair at a time. The same lineage validator
                // as ordinary cold restore verifies revisions and all native writers.
                let original_raw = original_member
                    .as_ref()
                    .map(|_| {
                        original
                            .read_member(
                                original_content.revision,
                                &path,
                                max_member_bytes,
                                deadline,
                                cancelled,
                            )
                            .map_err(|_| DurableError::Corrupt("streamed original lineage bytes"))
                    })
                    .transpose()?;
                if let Some(member) = &original_raw {
                    ManagedSourceWorkV1::charge(&mut body_work.original_bodies_returned, 1)?;
                    ManagedSourceWorkV1::charge(
                        &mut body_work.original_body_bytes_returned,
                        member.raw.len() as u64,
                    )?;
                }
                let mut raw = Vec::new();
                store.read_selected(receipt, max_member_bytes, &mut raw)?;
                ManagedSourceWorkV1::charge(&mut body_work.current_bodies_returned, 1)?;
                ManagedSourceWorkV1::charge(
                    &mut body_work.current_body_bytes_returned,
                    raw.len() as u64,
                )?;
                set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
                recovery::check_current_source_lineage(
                    tx,
                    domain,
                    &row,
                    &raw,
                    original_raw.as_ref().map(|member| member.raw.as_slice()),
                    original_metadata.as_ref(),
                )?;
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
    if ![
        "metadata",
        "claim",
        "event",
        "anchor",
        "form",
        "sign-candidate",
        "path",
    ]
    .contains(&kind)
    {
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

struct CompleteMixedIdentityPassV1 {
    members: u64,
}

fn current_claim_rows(
    file: &SourceFile,
) -> cmd::SourceCommandResult<BTreeMap<String, tos_foundation::JsonValue>> {
    if file.raw.len() > 1_048_576 {
        return Err(cmd::SourceCommandError::Invalid("Claim stream budget"));
    }
    let mut rows = BTreeMap::new();
    for line in file.raw.split(|byte| *byte == b'\n') {
        if crate::source_claims::python_bytes_blank(line) {
            continue;
        }
        let record = cmd::parse(line)?;
        let id = cmd::text(&record, "claim_id")?.to_owned();
        if rows.insert(id, record).is_some() {
            return Err(cmd::SourceCommandError::Conflict(
                "duplicate Claim identity",
            ));
        }
    }
    Ok(rows)
}

fn is_current_mixed_claim_stream_path(path: &str) -> bool {
    crate::source_creation_store::is_growth_claim_stream_path(path)
        || (path.starts_with("ToS/source-witnesses/relations/")
            && path.ends_with(&format!("/{}", crate::source_claims::CLAIM_STREAM))
            && path.split('/').count() == 5)
}

fn stage_current_mixed_identities(
    epoch: &mut StreamedSourceReadEpoch<'_, '_>,
    profile: &crate::source_claims::AgentInventoryAssessmentProfile,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<CompleteMixedIdentityPassV1> {
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
            .read(
                &member.path,
                usize::try_from(epoch.max_member_bytes)
                    .map_err(|_| DurableError::Refused("streamed member byte bound"))?,
                deadline,
                cancelled,
            )
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
        if crate::source_claims::is_imported_legacy_claim_path(&member.path) {
            for id in crate::source_claims::imported_legacy_claim_identity_rows(&file)
                .map_err(source_error)?
            {
                let mut index = index.borrow_mut();
                let mut work = work.borrow_mut();
                index.add_identity(SourceIdentityKind::Claim, &id, &member.path, &mut **work)?;
                index_owned_key(
                    &mut **index,
                    "claim",
                    &id,
                    &member.path,
                    &mut **work,
                    deadline,
                    cancelled,
                )?;
            }
            after = Some(member.path);
            count = count
                .checked_add(1)
                .ok_or(DurableError::Refused("streamed first pass count overflow"))?;
            continue;
        }
        if crate::source_claims::is_historical_claim_stream_path(&member.path) {
            for id in
                crate::source_claims::historical_claim_identity_rows(&file).map_err(source_error)?
            {
                let mut index = index.borrow_mut();
                let mut work = work.borrow_mut();
                index.add_identity(SourceIdentityKind::Claim, &id, &member.path, &mut **work)?;
                index_owned_key(
                    &mut **index,
                    "claim",
                    &id,
                    &member.path,
                    &mut **work,
                    deadline,
                    cancelled,
                )?;
            }
            after = Some(member.path);
            count = count
                .checked_add(1)
                .ok_or(DurableError::Refused("streamed first pass count overflow"))?;
            continue;
        }
        if is_current_mixed_claim_stream_path(&member.path)
            || super::is_item_claim_stream(&member.path)
        {
            for (id, _) in current_claim_rows(&file).map_err(source_error)? {
                let mut index = index.borrow_mut();
                let mut work = work.borrow_mut();
                index.add_identity(SourceIdentityKind::Claim, &id, &member.path, &mut **work)?;
                index_owned_key(
                    &mut **index,
                    "claim",
                    &id,
                    &member.path,
                    &mut **work,
                    deadline,
                    cancelled,
                )?;
            }
            after = Some(member.path);
            count = count
                .checked_add(1)
                .ok_or(DurableError::Refused("streamed first pass count overflow"))?;
            continue;
        }
        crate::source_claims::stage_agent_inventory_member(
            profile,
            &file,
            |kind, id, path| {
                let (typed, name) = match kind {
                    AgentInventoryIdentityKind::Metadata => {
                        (SourceIdentityKind::Metadata, "metadata")
                    }
                    AgentInventoryIdentityKind::Form => (SourceIdentityKind::Form, "form"),
                    AgentInventoryIdentityKind::Claim => (SourceIdentityKind::Claim, "claim"),
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
        if member.path.starts_with("ToS/source-witnesses/") && member.path.ends_with("/sign.json") {
            let candidate =
                crate::source_sign::pg::candidate_id_from_current_bytes(&member.path, &file.raw)
                    .map_err(source_error)?;
            let mut index = index.borrow_mut();
            let mut work = work.borrow_mut();
            index.add_identity(
                SourceIdentityKind::SignCandidate,
                &candidate,
                &member.path,
                &mut **work,
            )?;
            index_owned_key(
                &mut **index,
                "sign-candidate",
                &candidate,
                &member.path,
                &mut **work,
                deadline,
                cancelled,
            )?;
        }
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
    Ok(CompleteMixedIdentityPassV1 { members: count })
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

fn add_current_member(
    epoch: &mut StreamedSourceReadEpoch<'_, '_>,
    members: &mut BTreeMap<String, SourceFile>,
    path: &str,
    total_bytes: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    if members.contains_key(path) {
        return Ok(());
    }
    if members.len() >= cmd::SELECTED_SOURCE_MAX_FILES {
        return Err(DurableError::Refused(
            "streamed Claim closure member count bound",
        ));
    }
    let raw = epoch
        .read(
            path,
            cmd::SELECTED_SOURCE_MAX_MEMBER_BYTES,
            deadline,
            cancelled,
        )
        .map_err(source_error)?;
    *total_bytes = total_bytes
        .checked_add(raw.len())
        .filter(|bytes| *bytes <= cmd::SELECTED_SOURCE_MAX_BYTES)
        .ok_or(DurableError::Refused("streamed Claim closure byte bound"))?;
    let path_value = RelativePath::parse(path)
        .map_err(|_| DurableError::Corrupt("streamed Claim closure path"))?;
    members.insert(
        path.to_owned(),
        SourceFile {
            path: path_value,
            raw,
        },
    );
    Ok(())
}

fn historical_owner_direct_names(claim_ids: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    let mut names = BTreeSet::from([
        "historical-claims.jsonl".to_owned(),
        "claim-revision-history.json".to_owned(),
        "source-create-request.json".to_owned(),
        "source-create-receipt.json".to_owned(),
        "source-create-environment.json".to_owned(),
        "source-create-provenance.jsonl".to_owned(),
    ]);
    for id in claim_ids {
        let hash = Digest256::of_bytes(id.as_bytes()).to_hex();
        let form = format!("historical-claims.{hash}.human-forms.json");
        names.insert(form.clone());
        names.insert(format!(".{form}.writer.lock"));
    }
    names
}

fn selected_claim_archive_paths(history_files: &[SourceFile]) -> DurableResult<BTreeSet<String>> {
    let mut archives = BTreeSet::new();
    for history_file in history_files {
        let history = cmd::parse(&history_file.raw).map_err(source_error)?;
        let receipts = cmd::array(&history, "receipts").map_err(source_error)?;
        for receipt in receipts {
            if let Some(archive) = receipt.object_get("archive_path").and_then(|v| v.as_str()) {
                RelativePath::parse(archive)
                    .map_err(|_| DurableError::Corrupt("streamed Claim archive path"))?;
                if !archive.starts_with("ToS/source-witnesses/.record-revisions/") {
                    return Err(DurableError::Corrupt(
                        "streamed Claim archive is outside retained owner storage",
                    ));
                }
                archives.insert(archive.to_owned());
            }
        }
    }
    if archives.len() > 128 {
        return Err(DurableError::Refused("streamed Claim archive count bound"));
    }
    Ok(archives)
}

/// Keep the additional contracts read by each maintained Claim owner in the
/// same bounded selected context as that owner. Current Global Claim capture
/// validation and historical Claim validation have different, explicit
/// closures; Item and growth Claim routes keep their own schemas.
fn claim_context_required_contract_paths(stream_path: &str) -> Vec<String> {
    if crate::source_claims::is_historical_claim_stream_path(stream_path) {
        crate::source_native_cli::pg_historical_claim_required_contract_paths()
    } else if stream_path.starts_with("ToS/source-witnesses/relations/")
        && is_current_mixed_claim_stream_path(stream_path)
    {
        crate::source_claims::pg_claim_capture_required_contract_paths()
    } else {
        Vec::new()
    }
}

/// Construct one bounded current owner package for a selected Claim stream.
/// It reads only direct members of that owner home, exact retained archive
/// references, the already-validated profile, and the selected owner
/// contract closure. Sibling historical records and recursive payloads never
/// enter this context.
fn current_claim_context(
    epoch: &mut StreamedSourceReadEpoch<'_, '_>,
    stream_path: &str,
    profile: &crate::source_claims::AgentInventoryAssessmentProfile,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<CommandContext> {
    let mut selected = BTreeMap::new();
    let mut total_bytes = 0usize;
    let mut homes = BTreeSet::new();
    let home = stream_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(DurableError::Corrupt("streamed Claim owner home"))?;
    homes.insert(home.to_owned());
    if super::is_item_claim_stream(stream_path) {
        let (edition_home, _) = home
            .rsplit_once("/items/")
            .ok_or(DurableError::Corrupt("streamed Item parent Edition home"))?;
        homes.insert(edition_home.to_owned());
    }

    for file in crate::source_claims::pg_inventory::claim_profile_source_files(profile) {
        total_bytes = total_bytes
            .checked_add(file.raw.len())
            .filter(|bytes| *bytes <= cmd::SELECTED_SOURCE_MAX_BYTES)
            .ok_or(DurableError::Refused("streamed Claim profile byte bound"))?;
        selected.insert(file.path.as_str().to_owned(), file);
    }

    let historical = crate::source_claims::is_historical_claim_stream_path(stream_path);
    let historical_direct_names = if historical {
        let raw = epoch
            .read(stream_path, 1_048_576, deadline, cancelled)
            .map_err(source_error)?;
        let path = RelativePath::parse(stream_path)
            .map_err(|_| DurableError::Corrupt("streamed historical Claim path"))?;
        let stream = SourceFile { path, raw };
        historical_owner_direct_names(
            crate::source_claims::historical_claim_identity_rows(&stream).map_err(source_error)?,
        )
    } else {
        BTreeSet::new()
    };

    let mut selected_stream = false;
    for directory in homes {
        let children = epoch
            .list_directory(&directory, deadline, cancelled)
            .map_err(source_error)?;
        if children.len() > 1024 {
            return Err(DurableError::Refused(
                "streamed Claim owner direct-member count bound",
            ));
        }
        for (name, is_directory) in children {
            active(deadline, cancelled)?;
            if is_directory {
                continue;
            }
            let path = format!("{directory}/{name}");
            if path == stream_path {
                selected_stream = true;
            }
            if historical && directory == home {
                if !historical_direct_names.contains(&name) {
                    continue;
                }
            }
            add_current_member(
                epoch,
                &mut selected,
                &path,
                &mut total_bytes,
                deadline,
                cancelled,
            )?;
        }
    }
    if !selected_stream {
        return Err(DurableError::Corrupt("streamed Claim owner stream absent"));
    }

    // Add only contracts declared by the selected current or historical
    // Claim owner. The ordinary capture schema is needed by check_claim_capture
    // even though it is outside the generic Agent profile closure.
    for path in claim_context_required_contract_paths(stream_path) {
        add_current_member(
            epoch,
            &mut selected,
            &path,
            &mut total_bytes,
            deadline,
            cancelled,
        )?;
    }

    // Retained archive reads are driven only by manifests named in the exact
    // selected package histories. No archive directory is traversed.
    let history_files = selected
        .values()
        .filter(|file| {
            matches!(
                file.path.as_str().rsplit('/').next(),
                Some("claim-revision-history.json" | "source-revision-history.json")
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let archives = selected_claim_archive_paths(&history_files)?;
    for archive in archives {
        let manifest_path = format!("{archive}/manifest.json");
        add_current_member(
            epoch,
            &mut selected,
            &manifest_path,
            &mut total_bytes,
            deadline,
            cancelled,
        )?;
        let manifest_file = selected.get(&manifest_path).ok_or(DurableError::Corrupt(
            "streamed Claim archive manifest absent",
        ))?;
        let manifest = cmd::parse(&manifest_file.raw).map_err(source_error)?;
        let files = cmd::field(&manifest, "files")
            .map_err(source_error)?
            .as_object()
            .ok_or(DurableError::Corrupt(
                "streamed Claim archive manifest files",
            ))?;
        for (_, binding) in files {
            let blob = cmd::text(binding, "blob").map_err(source_error)?;
            if blob.is_empty() || blob.contains('/') || RelativePath::parse(blob).is_err() {
                return Err(DurableError::Corrupt("streamed Claim archive blob path"));
            }
            let blob_path = format!("{archive}/{blob}");
            add_current_member(
                epoch,
                &mut selected,
                &blob_path,
                &mut total_bytes,
                deadline,
                cancelled,
            )?;
        }
    }

    let authored = selected.into_values().collect::<Vec<_>>();

    let mut files = authored;
    files.extend(
        epoch
            .context
            .files
            .iter()
            .filter(|file| !file.path.as_str().starts_with("ToS/"))
            .cloned(),
    );
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let context = CommandContext {
        base_revision: epoch.context.base_revision,
        configuration_raw: epoch.context.configuration_raw.clone(),
        request_raw: epoch.context.request_raw.clone(),
        recorded_at: epoch.context.recorded_at.clone(),
        effective_uid: epoch.context.effective_uid,
        files,
    };
    context.check().map_err(source_error)?;
    active(deadline, cancelled)?;
    Ok(context)
}

fn streamed_evidence_projection(
    location: &str,
    id_field: &str,
    rows: Vec<cmd::SourceCommandResult<crate::source_claims::AgentInventoryEvidenceRow>>,
) -> DurableResult<tos_foundation::JsonValue> {
    let mut entries = BTreeMap::new();
    let mut decoded = Vec::new();
    for row in rows {
        let row = row.map_err(source_error)?;
        let payload = cmd::parse(&row.canonical_payload).map_err(source_error)?;
        if row.physical_line == 0
            || cmd::canonical(&payload).map_err(source_error)? != row.canonical_payload
            || cmd::text(&payload, id_field).map_err(source_error)? != row.id
            || cmd::record_digest(&payload).map_err(source_error)? != row.source_sha256
        {
            return Err(DurableError::Corrupt(
                "streamed Claim projection evidence differs",
            ));
        }
        decoded.push((row, payload));
    }
    decoded.sort_by(|left, right| left.0.id.as_bytes().cmp(right.0.id.as_bytes()));
    let mut previous: Option<String> = None;
    for (row, payload) in decoded {
        if previous
            .as_ref()
            .is_some_and(|id| id.as_bytes() >= row.id.as_bytes())
        {
            return Err(DurableError::Corrupt(
                "streamed Claim projection evidence identity duplicate",
            ));
        }
        let value = cmd::object(vec![
            ("payload", payload),
            ("source_ref", cmd::string(location)),
            ("source_line", cmd::number(row.physical_line)),
            ("source_sha256", cmd::string(&row.source_sha256.to_hex())),
        ]);
        if entries.insert(row.id.clone(), value).is_some() {
            return Err(DurableError::Corrupt(
                "streamed Claim projection evidence identity duplicate",
            ));
        }
        previous = Some(row.id);
    }
    Ok(tos_foundation::JsonValue::Object(
        entries
            .into_iter()
            .map(|(id, value)| (tos_foundation::JsonString::from_utf8(&id), value))
            .collect(),
    ))
}

fn render_current_claim_projection(
    epoch: &mut StreamedSourceReadEpoch<'_, '_>,
    profile: &crate::source_claims::AgentInventoryAssessmentProfile,
    context: &CommandContext,
    source_profiles: &tos_foundation::JsonValue,
    file: &SourceFile,
    worker: &mut CutWorkerSchemaExecutor,
    limits: tos_validation::item_rules::ItemLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Vec<u8>> {
    let location = file.path.as_str();
    let historical_stream = crate::source_claims::is_historical_claim_stream_path(location);
    let historical_form_stream = crate::source_claims::historical_claim_form_stream_path(location);
    let is_stream = location.ends_with(&format!("/{}", crate::source_claims::CLAIM_STREAM))
        || historical_stream;
    let owner_path = if historical_stream {
        location.to_owned()
    } else if let Some(stream) = historical_form_stream {
        stream
    } else if is_stream {
        location.to_owned()
    } else {
        let parent = location
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .ok_or(DurableError::Corrupt("streamed Claim form parent"))?;
        format!("{parent}/{}", crate::source_claims::CLAIM_STREAM)
    };
    let owner_file = context
        .files
        .iter()
        .find(|selected| selected.path.as_str() == owner_path)
        .ok_or(DurableError::Corrupt("streamed Claim owner stream absent"))?;
    let owner = if crate::source_claims::is_historical_claim_stream_path(&owner_path) {
        crate::source_claims::pg_inventory::historical_claim_owner_contribution_from_profile(
            context, owner_file, profile, worker, deadline, cancelled,
        )
        .map_err(source_error)?
    } else if super::is_item_claim_stream(&owner_path) {
        super::item_claim_owner_contribution(context, &owner_path, deadline, cancelled)?
    } else {
        crate::source_claims::pg_inventory::claim_owner_contribution(
            context, owner_file, worker, deadline, cancelled,
        )
        .map_err(source_error)?
    };

    let mut form = tos_foundation::JsonValue::Null;
    if !is_stream {
        form = crate::source_claims::pg_inventory::claim_form_contribution_with_validated_profile(
            context, file, profile, worker, deadline, cancelled,
        )
        .map_err(source_error)?;
    }

    let dummy = SourceFile {
        path: RelativePath::parse("ToS/doctrine/.streamed-claim-projection-base")
            .map_err(|_| DurableError::Corrupt("streamed Claim projection basis path"))?,
        raw: Vec::new(),
    };
    let base =
        crate::source_claims::render_agent_inventory_member(
            profile,
            &dummy,
            epoch,
            std::iter::empty::<
                cmd::SourceCommandResult<crate::source_claims::AgentInventoryEvidenceRow>,
            >(),
            std::iter::empty::<
                cmd::SourceCommandResult<crate::source_claims::AgentInventoryEvidenceRow>,
            >(),
            |_| Ok(false),
            worker,
            limits,
            deadline,
            cancelled,
        )
        .map_err(source_error)?;
    let mut projection = cmd::parse(&base).map_err(source_error)?;
    cmd::set(&mut projection, "path", cmd::string(location)).map_err(source_error)?;
    cmd::set(&mut projection, "source_profiles", source_profiles.clone()).map_err(source_error)?;
    cmd::set(
        &mut projection,
        "raw_sha256",
        cmd::string(&Digest256::of_bytes(&file.raw).to_hex()),
    )
    .map_err(source_error)?;
    let events = evidence_for_member(
        &mut **epoch.index.borrow_mut(),
        &mut **epoch.index_work.borrow_mut(),
        super::source_cohort_stream_index::SourceEvidenceKind::Event,
        location,
        1_048_576,
        deadline,
        cancelled,
    )?;
    let anchors = evidence_for_member(
        &mut **epoch.index.borrow_mut(),
        &mut **epoch.index_work.borrow_mut(),
        super::source_cohort_stream_index::SourceEvidenceKind::Anchor,
        location,
        1_048_576,
        deadline,
        cancelled,
    )?;
    cmd::set(
        &mut projection,
        "events",
        streamed_evidence_projection(location, "event_id", events)?,
    )
    .map_err(source_error)?;
    cmd::set(
        &mut projection,
        "anchors",
        streamed_evidence_projection(location, "anchor_id", anchors)?,
    )
    .map_err(source_error)?;
    cmd::set(&mut projection, "form", form).map_err(source_error)?;
    cmd::set(
        &mut projection,
        "claim_owner",
        if is_stream {
            owner
        } else {
            tos_foundation::JsonValue::Null
        },
    )
    .map_err(source_error)?;
    let raw = cmd::canonical(&projection).map_err(source_error)?;
    if raw.len() > 1_048_576 {
        return Err(DurableError::Refused(
            "streamed Claim member projection bound",
        ));
    }
    Ok(raw)
}

struct CompleteMixedProjectionPassV1 {
    members: u64,
    // Exact ordered mixed projection transcript, not the addressed root.
    projection_digest: Digest256,
    // Compact identity rows come from the same exact current-member pass.
    // Large Text/provenance bodies do not survive their individual parse.
    text_identity_candidates: crate::source_text_identity_index::CurrentTextIdentityRows,
    // Public semantic annotation packets from that same full source pass.
    annotation_identity_candidates: BTreeMap<String, Vec<u8>>,
    // Exact Claim streams and shared-schema paths from that same bounded pass
    // support cold equality verification. Shared schemas use empty byte
    // sentinels because only their paths, not their contents, are indexed.
    claim_dependency_candidates: BTreeMap<String, Vec<u8>>,
    // Current Item records and manifests needed to verify the File assignment
    // index without a second source read.
    item_file_candidates: BTreeMap<String, Vec<u8>>,
    // Exact selected publication control and journal members from this same
    // bounded source pass; unrelated source bodies are never retained here.
    legacy_publication_members: BTreeMap<String, Vec<u8>>,
}

fn source_inventory_contribution_projection(
    path: &str,
    source_raw: &[u8],
    contribution: &tos_foundation::JsonValue,
) -> DurableResult<Vec<u8>> {
    // Growth and Item owner adapters return this exact seven-field semantic
    // contribution. It is not yet the ten-field persisted mixed projection.
    cmd::exact_keys(
        contribution,
        &[
            "schema_version",
            "records",
            "source_profiles",
            "events",
            "anchors",
            "form",
            "claim_owner",
        ],
    )
    .map_err(source_error)?;
    if cmd::text(contribution, "schema_version").map_err(source_error)?
        != "tos_pg_source_inventory_member_contribution_v1"
    {
        return Err(DurableError::Corrupt(
            "streamed owner source contribution schema differs",
        ));
    }
    super::mixed_projection_envelope(
        path,
        source_raw,
        cmd::field(contribution, "records")
            .map_err(source_error)?
            .clone(),
        cmd::field(contribution, "source_profiles")
            .map_err(source_error)?
            .clone(),
        cmd::field(contribution, "events")
            .map_err(source_error)?
            .clone(),
        cmd::field(contribution, "anchors")
            .map_err(source_error)?
            .clone(),
        cmd::field(contribution, "form")
            .map_err(source_error)?
            .clone(),
        cmd::field(contribution, "claim_owner")
            .map_err(source_error)?
            .clone(),
        tos_foundation::JsonValue::Null,
    )
}

fn render_current_mixed_projections(
    epoch: &mut StreamedSourceReadEpoch<'_, '_>,
    profile: &crate::source_claims::AgentInventoryAssessmentProfile,
    first: CompleteMixedIdentityPassV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: tos_validation::item_rules::ItemLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<CompleteMixedProjectionPassV1> {
    use super::source_cohort_stream_index::{AssessmentProjectionRow, SourceEvidenceKind};
    active(deadline, cancelled)?;
    if first.members != epoch.cold.current_members {
        return Err(DurableError::Corrupt(
            "streamed identity pass basis differs",
        ));
    }
    let mut after = None::<String>;
    let mut count = 0u64;
    let mut claim_context = None::<CommandContext>;
    let mut claim_context_path = None::<String>;
    let mut claim_source_profiles = None::<tos_foundation::JsonValue>;
    let mut text_identity_candidates =
        crate::source_text_identity_index::CurrentTextIdentityRows::default();
    let mut annotation_identity_candidates = BTreeMap::new();
    let mut annotation_identity_candidate_bytes = 0usize;
    let mut claim_dependency_candidates = BTreeMap::new();
    let mut claim_dependency_candidate_streams = 0usize;
    let mut claim_dependency_candidate_bytes = 0usize;
    let mut claim_dependency_candidate_schema_paths = 0usize;
    let mut item_file_candidates = BTreeMap::new();
    let mut legacy_publication_members = BTreeMap::new();
    let mut legacy_publication_bytes = 0usize;
    let mut item_manifest_count = 0usize;
    let mut item_manifest_bytes = 0usize;
    let mut item_record_count = 0usize;
    let mut item_record_bytes = 0usize;
    let mut item_rights_count = 0usize;
    let mut item_rights_bytes = 0usize;
    let mut public_text_fingerprints = BTreeMap::new();
    let mut public_text_contributions = BTreeMap::new();
    let mut public_text_contribution_bytes = 0usize;
    const MAX_RETAINED_PUBLIC_TEXT_CONTRIBUTION_BYTES: usize = 67_108_864;
    let public_text_member_limit = usize::try_from(epoch.max_member_bytes)
        .map_err(|_| DurableError::Refused("streamed member byte bound"))?;
    for package in epoch.public_text_packages {
        active(deadline, cancelled)?;
        let mut package_files = BTreeMap::new();
        let mut package_body_bytes = 0usize;
        for name in crate::source_public_text_entry::FILES {
            let path = format!("{}/{name}", package.package_home);
            let raw = epoch
                .read(&path, public_text_member_limit, deadline, cancelled)
                .map_err(source_error)?;
            package_body_bytes = package_body_bytes
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= cmd::SELECTED_SOURCE_MAX_BYTES)
                .ok_or(DurableError::Refused(
                    "streamed Public Text current package byte bound",
                ))?;
            let fingerprint = (Digest256::of_bytes(&raw), raw.len() as u64);
            if package_files.insert(path.clone(), raw).is_some()
                || public_text_fingerprints.insert(path, fingerprint).is_some()
                || public_text_fingerprints.len() as u64 > epoch.cold.current_members
            {
                return Err(DurableError::Corrupt(
                    "duplicate streamed Public Text package member",
                ));
            }
        }
        let contributions = super::regenerate_current_public_text_contributions(
            &package_files,
            std::slice::from_ref(package),
        )?;
        drop(package_files);
        for (path, owner) in contributions {
            let owner_bytes = cmd::canonical(&owner).map_err(source_error)?;
            if owner_bytes.len() > 1_048_576 {
                return Err(DurableError::Refused(
                    "streamed Public Text owner contribution member bound",
                ));
            }
            public_text_contribution_bytes = public_text_contribution_bytes
                .checked_add(owner_bytes.len())
                .filter(|bytes| *bytes <= MAX_RETAINED_PUBLIC_TEXT_CONTRIBUTION_BYTES)
                .ok_or(DurableError::Refused(
                    "streamed Public Text owner contribution package bound",
                ))?;
            if public_text_contributions.insert(path, owner).is_some() {
                return Err(DurableError::Corrupt(
                    "duplicate streamed Public Text owner contribution",
                ));
            }
        }
    }
    let mut transcript = Digest256Hasher::new();
    part(
        &mut transcript,
        b"tos-managed-mixed-source-inventory-coverage-v1",
    );
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
            .read(
                &member.path,
                usize::try_from(epoch.max_member_bytes)
                    .map_err(|_| DurableError::Refused("streamed member byte bound"))?,
                deadline,
                cancelled,
            )
            .map_err(source_error)?;
        if member.path == "ToS/source-witnesses/.metadata-publication.json"
            || member
                .path
                .starts_with("ToS/source-witnesses/.metadata-transactions/")
        {
            if legacy_publication_members.len() >= 4096 {
                return Err(DurableError::Refused(
                    "streamed legacy publication member count bound",
                ));
            }
            legacy_publication_bytes = legacy_publication_bytes
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= 33_554_432)
                .ok_or(DurableError::Refused(
                    "streamed legacy publication byte bound",
                ))?;
            if legacy_publication_members
                .insert(member.path.clone(), raw.clone())
                .is_some()
            {
                return Err(DurableError::Corrupt(
                    "duplicate streamed legacy publication member",
                ));
            }
        }
        let file = SourceFile { path, raw };
        let index = epoch.index;
        let work = epoch.index_work;
        let location = member.path.as_str();
        let basename = location.rsplit('/').next().unwrap_or(location);
        let historical_claim_stream =
            crate::source_claims::is_historical_claim_stream_path(location);
        let historical_claim_form_stream =
            crate::source_claims::historical_claim_form_stream_path(location);
        let claim_stream = is_current_mixed_claim_stream_path(location)
            || super::is_item_claim_stream(location)
            || historical_claim_stream;
        let imported_legacy_claim_stream =
            crate::source_claims::is_imported_legacy_claim_path(location);
        let claim_form =
            basename.starts_with("source-claims.") && basename.ends_with(".human-forms.json");
        let claim_owner_stream = if claim_stream || imported_legacy_claim_stream {
            Some(location.to_owned())
        } else if let Some(stream) = historical_claim_form_stream {
            Some(stream)
        } else if claim_form {
            location
                .rsplit_once('/')
                .map(|(parent, _)| format!("{parent}/{}", crate::source_claims::CLAIM_STREAM))
                .filter(|path| {
                    is_current_mixed_claim_stream_path(path) || super::is_item_claim_stream(path)
                })
        } else {
            None
        };
        let mut growth_contributions = None;
        let mut is_global_claim_owner = false;
        if let Some(stream_path) = claim_owner_stream.as_deref() {
            if claim_context_path.as_deref() != Some(stream_path) {
                claim_context = Some(current_claim_context(
                    epoch,
                    stream_path,
                    profile,
                    deadline,
                    cancelled,
                )?);
                claim_context_path = Some(stream_path.to_owned());
            }
            let context = claim_context
                .as_ref()
                .ok_or(DurableError::Corrupt("streamed Claim context absent"))?;
            growth_contributions =
                crate::source_creation_store::current_pg_growth_owner_contributions(
                    context,
                    stream_path,
                )?;
            is_global_claim_owner =
                !crate::source_claims::is_historical_claim_stream_path(stream_path)
                    && super::is_global_claim_stream(context, stream_path)?;
        }
        let is_item_claim_owner = claim_stream && super::is_item_claim_stream(location);
        let is_item_claim_form = claim_form
            && claim_context.as_ref().is_some_and(|context| {
                super::item_claim_form_owner_stream(context, location).is_some()
            });
        let is_historical_claim_owner = claim_owner_stream
            .as_deref()
            .is_some_and(crate::source_claims::is_historical_claim_stream_path);
        let growth_member = growth_contributions
            .as_ref()
            .and_then(|contributions| contributions.get(location));
        let mut projection = if let Some(contribution) = growth_member {
            source_inventory_contribution_projection(location, &file.raw, contribution)?
        } else if is_item_claim_owner {
            let context = claim_context
                .as_ref()
                .ok_or(DurableError::Corrupt("streamed Item Claim context absent"))?;
            let owner =
                super::item_claim_owner_contribution(context, location, deadline, cancelled)?;
            let contribution = super::item_claim_current_inventory_contribution(owner, profile)?;
            source_inventory_contribution_projection(location, &file.raw, &contribution)?
        } else if is_item_claim_form || is_global_claim_owner {
            if claim_source_profiles.is_none() {
                claim_source_profiles = Some(
                    crate::source_claims::pg_inventory::
                        claim_profile_sources_from_validated_profile(profile)
                    .map_err(source_error)?,
                );
            }
            render_current_claim_projection(
                epoch,
                profile,
                claim_context
                    .as_ref()
                    .ok_or(DurableError::Corrupt("streamed Claim context absent"))?,
                claim_source_profiles
                    .as_ref()
                    .ok_or(DurableError::Corrupt("streamed Claim profile absent"))?,
                &file,
                worker,
                limits,
                deadline,
                cancelled,
            )?
        } else if is_historical_claim_owner {
            if claim_source_profiles.is_none() {
                claim_source_profiles = Some(
                    crate::source_claims::pg_inventory::
                        claim_profile_sources_from_validated_profile(profile)
                    .map_err(source_error)?,
                );
            }
            render_current_claim_projection(
                epoch,
                profile,
                claim_context
                    .as_ref()
                    .ok_or(DurableError::Corrupt("streamed Claim context absent"))?,
                claim_source_profiles
                    .as_ref()
                    .ok_or(DurableError::Corrupt("streamed Claim profile absent"))?,
                &file,
                worker,
                limits,
                deadline,
                cancelled,
            )?
        } else {
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
            crate::source_claims::render_agent_inventory_member(
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
            .map_err(source_error)?
        };
        if imported_legacy_claim_stream {
            let context = claim_context.as_ref().ok_or(DurableError::Corrupt(
                "streamed imported Claim context absent",
            ))?;
            let mut value = cmd::parse(&projection).map_err(source_error)?;
            cmd::set(
                &mut value,
                "claim_owner",
                crate::source_claims::pg_inventory::imported_legacy_claim_owner_contribution(
                    context, &file,
                )
                .map_err(source_error)?,
            )
            .map_err(source_error)?;
            projection = cmd::canonical(&value).map_err(source_error)?;
        }
        if let Some(owner) = public_text_contributions.get(&member.path) {
            let fingerprint = (Digest256::of_bytes(&file.raw), file.raw.len() as u64);
            if public_text_fingerprints.get(&member.path) != Some(&fingerprint) {
                return Err(DurableError::Corrupt(
                    "streamed Public Text member reread differs",
                ));
            }
            let mut value =
                super::addressed_inventory::AddressedMixedInventoryV1::checked_projection(
                    &member.path,
                    &projection,
                    &Digest256::of_bytes(&file.raw).to_hex(),
                )?;
            let existing = value
                .object_get("public_text_owner")
                .ok_or(DurableError::Corrupt(
                    "streamed Public Text owner field absent",
                ))?;
            if existing != &tos_foundation::JsonValue::Null
                && !super::public_text_owner_contributions_match(existing, owner)?
            {
                return Err(DurableError::Corrupt(
                    "streamed Public Text generic owner contribution conflicts",
                ));
            }
            cmd::set(&mut value, "public_text_owner", owner.clone()).map_err(source_error)?;
            projection = cmd::canonical(&value).map_err(source_error)?;
            super::addressed_inventory::AddressedMixedInventoryV1::checked_projection(
                &member.path,
                &projection,
                &Digest256::of_bytes(&file.raw).to_hex(),
            )?;
        }
        if projection.len() > 1_048_576 {
            return Err(DurableError::Refused(
                "streamed canonical member projection bound",
            ));
        }
        if crate::source_claim_dependency_index::is_claim_navigation_source_member(&member.path) {
            let candidate =
                if crate::source_claim_dependency_index::is_claim_stream_path(&member.path) {
                    claim_dependency_candidate_streams = claim_dependency_candidate_streams
                        .checked_add(1)
                        .filter(|count| {
                            *count <= crate::source_claim_dependency_index::MAX_CLAIM_STREAMS
                        })
                        .ok_or(DurableError::Refused(
                            "streamed Claim dependency stream count bound exceeded",
                        ))?;
                    if file.raw.len() > crate::source_claim_dependency_index::MAX_STREAM_BYTES {
                        return Err(DurableError::Refused(
                            "streamed Claim dependency stream byte bound exceeded",
                        ));
                    }
                    claim_dependency_candidate_bytes = claim_dependency_candidate_bytes
                        .checked_add(file.raw.len())
                        .filter(|bytes| {
                            *bytes <= crate::source_claim_dependency_index::MAX_TOTAL_CLAIM_BYTES
                        })
                        .ok_or(DurableError::Refused(
                            "streamed Claim dependency aggregate byte bound exceeded",
                        ))?;
                    file.raw.clone()
                } else {
                    claim_dependency_candidate_schema_paths =
                        claim_dependency_candidate_schema_paths
                            .checked_add(1)
                            .filter(|count| {
                                *count
                                    <= crate::source_claim_dependency_index::MAX_SHARED_SCHEMA_PATHS
                            })
                            .ok_or(DurableError::Refused(
                                "streamed Claim navigation shared schema path bound exceeded",
                            ))?;
                    if member.path.len() > crate::source_claim_dependency_index::MAX_PATH_BYTES {
                        return Err(DurableError::Refused(
                            "streamed Claim navigation shared schema path byte bound exceeded",
                        ));
                    }
                    Vec::new()
                };
            if claim_dependency_candidates
                .insert(member.path.clone(), candidate)
                .is_some()
            {
                return Err(DurableError::Corrupt(
                    "duplicate streamed Claim navigation source path",
                ));
            }
            if claim_stream && growth_contributions.is_some() {
                let context = claim_context.as_ref().ok_or(DurableError::Corrupt(
                    "streamed Growth Claim context absent",
                ))?;
                let owner = growth_contributions
                    .as_ref()
                    .and_then(|contributions| contributions.get(location))
                    .and_then(|contribution| contribution.object_get("claim_owner"))
                    .ok_or(DurableError::Corrupt("streamed Growth Claim owner absent"))?;
                let direct_members = owner
                    .object_get("direct_members")
                    .and_then(tos_foundation::JsonValue::as_array)
                    .ok_or(DurableError::Corrupt(
                        "streamed Growth Claim owner members absent",
                    ))?;
                for row in direct_members {
                    let role = row
                        .object_get("role")
                        .and_then(tos_foundation::JsonValue::as_str)
                        .ok_or(DurableError::Corrupt(
                            "streamed Growth Claim owner role absent",
                        ))?;
                    if !matches!(
                        role,
                        "creation-request"
                            | "creation-environment"
                            | "creation-event"
                            | "creation-receipt"
                            | "revision-history"
                    ) {
                        continue;
                    }
                    let owner_path = row
                        .object_get("ref")
                        .and_then(tos_foundation::JsonValue::as_str)
                        .ok_or(DurableError::Corrupt(
                            "streamed Growth Claim owner ref absent",
                        ))?;
                    let raw = context
                        .files
                        .iter()
                        .find(|source| source.path.as_str() == owner_path)
                        .map(|source| source.raw.clone())
                        .ok_or(DurableError::Corrupt(
                            "streamed Growth Claim owner input absent",
                        ))?;
                    if claim_dependency_candidates.contains_key(owner_path) {
                        continue;
                    }
                    claim_dependency_candidate_bytes = claim_dependency_candidate_bytes
                        .checked_add(raw.len())
                        .filter(|bytes| {
                            *bytes <= crate::source_claim_dependency_index::MAX_TOTAL_CLAIM_BYTES
                        })
                        .ok_or(DurableError::Refused(
                            "streamed Claim owner input aggregate byte bound exceeded",
                        ))?;
                    claim_dependency_candidates.insert(owner_path.to_owned(), raw);
                }
            }
        }
        if crate::source_item_file_index::is_candidate_manifest_path(&member.path)
            || crate::source_item_file_index::is_candidate_item_record_path(&member.path)
            || crate::source_item_file_index::is_candidate_item_rights_path(&member.path)
        {
            if crate::source_item_file_index::is_candidate_manifest_path(&member.path) {
                item_manifest_count = item_manifest_count
                    .checked_add(1)
                    .filter(|count| *count <= crate::source_item_file_index::MAX_MANIFEST_COUNT)
                    .ok_or(DurableError::Refused(
                        "streamed Item manifest count bound exceeded",
                    ))?;
                if file.raw.len() > crate::source_item_file_index::MAX_MANIFEST_BYTES {
                    return Err(DurableError::Refused(
                        "streamed Item manifest byte bound exceeded",
                    ));
                }
                item_manifest_bytes = item_manifest_bytes
                    .checked_add(file.raw.len())
                    .filter(|bytes| {
                        *bytes <= crate::source_item_file_index::MAX_TOTAL_MANIFEST_BYTES
                    })
                    .ok_or(DurableError::Refused(
                        "streamed Item manifest aggregate bound exceeded",
                    ))?;
            } else if crate::source_item_file_index::is_candidate_item_record_path(&member.path) {
                item_record_count = item_record_count
                    .checked_add(1)
                    .filter(|count| *count <= crate::source_item_file_index::MAX_MANIFEST_COUNT)
                    .ok_or(DurableError::Refused(
                        "streamed Item record count bound exceeded",
                    ))?;
                item_record_bytes = item_record_bytes
                    .checked_add(file.raw.len())
                    .filter(|bytes| {
                        *bytes <= crate::source_item_file_index::MAX_TOTAL_MANIFEST_BYTES
                    })
                    .ok_or(DurableError::Refused(
                        "streamed Item record aggregate bound exceeded",
                    ))?;
            } else {
                item_rights_count = item_rights_count
                    .checked_add(1)
                    .filter(|count| *count <= crate::source_item_file_index::MAX_MANIFEST_COUNT)
                    .ok_or(DurableError::Refused(
                        "streamed Item rights count bound exceeded",
                    ))?;
                if file.raw.len() > crate::source_item_file_index::MAX_MANIFEST_BYTES {
                    return Err(DurableError::Refused(
                        "streamed Item rights byte bound exceeded",
                    ));
                }
                item_rights_bytes = item_rights_bytes
                    .checked_add(file.raw.len())
                    .filter(|bytes| {
                        *bytes <= crate::source_item_file_index::MAX_TOTAL_MANIFEST_BYTES
                    })
                    .ok_or(DurableError::Refused(
                        "streamed Item rights aggregate bound exceeded",
                    ))?;
            }
            if item_file_candidates
                .insert(member.path.clone(), file.raw.clone())
                .is_some()
            {
                return Err(DurableError::Corrupt(
                    "duplicate streamed Item File candidate path",
                ));
            }
        }
        if crate::source_annotation_identity_index::is_packet(&member.path) {
            if annotation_identity_candidates.len()
                >= crate::source_annotation_identity_index::MAX_FILES
            {
                return Err(DurableError::Refused(
                    "streamed annotation identity candidate file bound",
                ));
            }
            annotation_identity_candidate_bytes = annotation_identity_candidate_bytes
                .checked_add(file.raw.len())
                .filter(|bytes| *bytes <= crate::source_annotation_identity_index::MAX_BYTES)
                .ok_or(DurableError::Refused(
                    "streamed annotation identity candidate byte bound",
                ))?;
            if annotation_identity_candidates
                .insert(member.path.clone(), file.raw.clone())
                .is_some()
            {
                return Err(DurableError::Corrupt(
                    "duplicate streamed annotation identity candidate path",
                ));
            }
        }
        let basename = member.path.rsplit('/').next().unwrap_or(&member.path);
        if member.path.starts_with("ToS/")
            && !member.path.starts_with("ToS/source-witnesses/owner-local/")
            && crate::source_text_identity::is_candidate(basename)
        {
            text_identity_candidates.add_member(&member.path, &file.raw)?;
        }

        if epoch.fresh_sink {
            // SQL profile_id is TEXT; retain the original byte identity elsewhere.
            let original_profile = std::str::from_utf8(ORIGINAL)
                .map_err(|_| DurableError::Corrupt("original source profile is not UTF-8"))?;
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
                    &[&epoch.domain, &member.path, &projection, &original_profile],
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
            "SELECT CASE WHEN octet_length(c.inventory_projection)<=1048576 THEN c.inventory_projection ELSE NULL END,octet_length(c.inventory_projection),CASE WHEN p.effective_seq IS NULL THEN CASE WHEN octet_length(h.inventory_projection)<=1048576 THEN h.inventory_projection ELSE NULL END WHEN p.source_revision=h.revision AND p.content_digest=h.content_digest AND p.definition_digest=d.source_definition_digest THEN CASE WHEN octet_length(p.inventory_projection)<=1048576 THEN p.inventory_projection ELSE NULL END ELSE NULL END,CASE WHEN p.effective_seq IS NULL THEN octet_length(h.inventory_projection) WHEN p.source_revision=h.revision AND p.content_digest=h.content_digest AND p.definition_digest=d.source_definition_digest THEN octet_length(p.inventory_projection) ELSE NULL END FROM cmd2_current c JOIN cmd2_domain d ON d.domain=c.domain LEFT JOIN cmd2_history h ON h.domain=c.domain AND h.subject=c.subject AND h.revision=c.revision LEFT JOIN LATERAL (SELECT source_revision,content_digest,definition_digest,inventory_projection,effective_seq FROM cmd2_source_inventory_projection WHERE domain=c.domain AND subject=c.subject AND effective_seq>=h.commit_seq AND effective_seq<=d.head_seq ORDER BY effective_seq DESC LIMIT 1) p ON TRUE WHERE c.domain=$1 AND c.subject=$2",
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
        if epoch.rebuild_projections {
            set_streamed_pg_limits(epoch.tx, epoch.max_statement_ms, deadline, cancelled)?;
            let current = epoch.tx.query_one(
                "SELECT c.*,$3::bytea AS retained_inventory_projection,i.inventory_projection AS indexed_inventory_projection FROM cmd2_current c LEFT JOIN cmd2_source_index i ON i.domain=c.domain AND i.kind='path' AND i.token=c.subject AND i.path=c.subject WHERE c.domain=$1 AND c.subject=$2",
                &[&epoch.domain, &member.path, &retained],
            )?;
            let indexed: Option<Vec<u8>> = current.get("indexed_inventory_projection");
            recovery::refresh_current_projection(
                epoch.tx,
                epoch.domain,
                &current,
                &projection,
                indexed.as_deref() == Some(projection.as_slice()),
                epoch.cold.through_commit_seq,
                definition(),
            )?;
        } else if returned
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
    let projected = CompleteMixedProjectionPassV1 {
        members: count,
        projection_digest: transcript.finalize(),
        text_identity_candidates,
        annotation_identity_candidates,
        claim_dependency_candidates,
        item_file_candidates,
        legacy_publication_members,
    };
    // Predicate expectations are part of source assessment input. Stage them
    // while the index is still open, before closing semantic source inputs.
    let mut index_work = epoch.index_work.borrow_mut();
    stage_current_owner_predicate_expectations(
        &mut **epoch.index.borrow_mut(),
        &mut **index_work,
        &projected,
        epoch.fresh_sink,
        deadline,
        cancelled,
    )?;
    epoch
        .index
        .borrow_mut()
        .finish_source_inputs(&mut **index_work)?;
    active(deadline, cancelled)?;
    Ok(projected)
}

struct CompleteStreamedMixedSourceAssessmentV1 {
    members: u64,
    projection_digest: Digest256,
    publication_import: Option<crate::source_work_transaction::LegacySelectedPublicationImportV1>,
}

fn add_expected_owned_predicate(
    index: &mut SourceAssessmentIndex,
    work: &mut SourceAssessmentWork,
    kind: &str,
    scope: &str,
    token: &str,
    version: &str,
) -> DurableResult<()> {
    if !super::valid_owned_predicate_shape(OWNER, kind, scope, token) {
        return Err(DurableError::Corrupt(
            "streamed expected predicate owner definition differs",
        ));
    }
    index.add_predicate_row(
        &super::source_cohort_stream_index::AssessmentPredicateRow {
            kind: kind.to_owned(),
            owner: OWNER.to_owned(),
            scope: scope.to_owned(),
            token: token.to_owned(),
            definition_version: version.to_owned(),
        },
        work,
    )
}

/// Seed the streamed expected-predicate spool from current source-derived
/// owner tokens before source inputs are closed. Valid retained absence keys
/// are observed separately during comparison; they do not replace these
/// current source-derived expectations.
fn stage_current_owner_predicate_expectations(
    index: &mut SourceAssessmentIndex,
    work: &mut SourceAssessmentWork,
    projected: &CompleteMixedProjectionPassV1,
    fresh_sink: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    if fresh_sink {
        // Owner-local rows are intentionally rebuilt only after comparison on
        // an empty bootstrap target. The generic expected predicates were
        // already staged from the complete current source members.
        return Ok(());
    }
    let version = definition().to_hex();
    for token in projected.text_identity_candidates.tokens() {
        add_expected_owned_predicate(
            index,
            work,
            "unique",
            crate::source_text_identity_index::PREDICATE_SCOPE,
            &token,
            &version,
        )?;
    }
    for token in crate::source_annotation_identity_index::current_predicate_tokens(
        &projected.annotation_identity_candidates,
    )? {
        add_expected_owned_predicate(
            index,
            work,
            "unique",
            crate::source_annotation_identity_index::PREDICATE_SCOPE,
            &token,
            &version,
        )?;
    }
    let (claim_tokens, navigation_tokens) =
        crate::source_claim_dependency_index::current_predicate_tokens(
            &projected.claim_dependency_candidates,
            definition(),
        )?;
    for token in claim_tokens {
        add_expected_owned_predicate(
            index,
            work,
            "range",
            crate::source_claim_dependency_index::PREDICATE_SCOPE,
            &token,
            &version,
        )?;
    }
    for token in navigation_tokens {
        add_expected_owned_predicate(
            index,
            work,
            "range",
            crate::source_claim_dependency_index::NAVIGATION_REFERENCE_SCOPE,
            &token,
            &version,
        )?;
    }
    let [item_file_tokens, item_rights_tokens, item_edition_tokens] =
        crate::source_item_file_index::current_predicate_tokens(
            &projected.item_file_candidates,
            definition(),
        )?;
    for (scope, tokens) in [
        (
            crate::source_item_file_index::PREDICATE_SCOPE,
            item_file_tokens,
        ),
        (
            crate::source_item_file_index::RIGHTS_PREDICATE_SCOPE,
            item_rights_tokens,
        ),
        (
            crate::source_item_file_index::EDITION_PREDICATE_SCOPE,
            item_edition_tokens,
        ),
    ] {
        for token in tokens {
            add_expected_owned_predicate(index, work, "range", scope, &token, &version)?;
        }
    }
    // source-publication/current is optional at this point: its exact shape is
    // checked with every retained row, while ensure_source_publication_bootstrap_state
    // owns its lineage validation and may seed it after this comparison.
    active(deadline, cancelled)
}

fn compare_current_owner_indexes(
    tx: &mut Transaction<'_>,
    max_statement_ms: u64,
    domain: &str,
    index: &mut SourceAssessmentIndex,
    work: &mut SourceAssessmentWork,
    original: &tos_source_store::StreamedCorpusCutReaderV1,
    verified_original: &VerifiedStreamedOriginalContentV1,
    projected: &CompleteMixedProjectionPassV1,
    source_index_rows: u64,
    owned_predicate_rows: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<CompleteStreamedMixedSourceAssessmentV1> {
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
                || !super::valid_owned_predicate_shape(&owner, &kind, &scope, &token)
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
    Ok(CompleteStreamedMixedSourceAssessmentV1 {
        members: projected.members,
        projection_digest: projected.projection_digest,
        publication_import: None,
    })
}

// The descriptor is built only after complete source-owned semantic/index
// comparison. The private witness is not a currentness or publication grant;
// the coordinator must still hold and repeat its domain/audit/rights gates.
fn build_assessed_inventory(
    store: &SegmentStore,
    assessment: &CompleteStreamedMixedSourceAssessmentV1,
    index: &mut SourceAssessmentIndex,
    index_work: &mut SourceAssessmentWork,
    limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    tree_work: &mut tos_segment_store::AuthenticatedTreeWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    owner_check: &dyn Fn() -> DurableResult<()>,
) -> DurableResult<super::addressed_inventory::AddressedMixedInventoryV1> {
    owner_check()?;
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
    let mut tree_owner_check = || owner_check().is_ok();
    let result = super::addressed_inventory::AddressedMixedInventoryV1::build_from_assessed_projections_with_callback(
        store,
        projections,
        assessment.members,
        limits,
        deadline,
        cancelled,
        tree_work,
        &mut tree_owner_check,
    );
    owner_check()?;
    result
}

// Completion and its transcript commit atomically before addressed packing.
// Reuse is restricted to the unchanged initial bootstrap: the caller holds
// its domain/audit/rights fence and checked ORIGINAL bytes and software inputs.
// Verify the full retained projection transcript without rerunning source
// semantics or schema children. This never promotes an incomplete seed.
fn reuse_completed_bootstrap_assessment(
    tx: &mut Transaction<'_>,
    store: &SegmentStore,
    domain: &str,
    expected_members: u64,
    expected_projection: Digest256,
    max_statement_ms: u64,
    limits: SourceAssessmentLimits,
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    tree_work: &mut tos_segment_store::AuthenticatedTreeWorkV1,
    body_work: &mut StreamedColdSourceWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    owner_check: &dyn Fn() -> DurableResult<()>,
) -> DurableResult<super::addressed_inventory::AddressedMixedInventoryV1> {
    active(deadline, cancelled)?;
    owner_check()?;
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    let admission = tx.query_one(
        "SELECT count(*),coalesce(sum(octet_length(inventory_projection)),0),coalesce(max(octet_length(inventory_projection)),0),count(*) FILTER(WHERE inventory_projection IS NULL OR revision<>1 OR profile_id<>$2) FROM cmd2_current WHERE domain=$1",
        &[&domain, &std::str::from_utf8(ORIGINAL).map_err(|_| DurableError::Corrupt("bootstrap original profile"))?],
    )?;
    if as_u64(admission.get(0))? != expected_members
        || expected_members > limits.max_rows
        || as_u64(admission.get(1))? > limits.max_logical_bytes
        || admission.get::<_, i32>(2) as usize > limits.max_value_bytes
        || admission.get::<_, i64>(3) != 0
    {
        return Err(DurableError::Corrupt(
            "completed bootstrap projection coverage differs",
        ));
    }
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    let incomplete = tx.query_one(
        "SELECT count(*) FROM cmd2_predicate WHERE domain=$1 AND owner=$2 AND definition_version=$3 AND NOT complete",
        &[&domain, &OWNER, &definition().to_hex()],
    )?;
    if incomplete.get::<_, i64>(0) != 0 {
        return Err(DurableError::Corrupt(
            "completed bootstrap predicate is incomplete",
        ));
    }
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    let cap = i32::try_from(limits.max_value_bytes)
        .map_err(|_| DurableError::Refused("completed bootstrap projection byte bound"))?;
    let mut rows = super::addressed_successor::bounded_pg_rows(
        tx,
        "SELECT c.subject,CASE WHEN octet_length(c.inventory_projection)<=$2 THEN c.inventory_projection ELSE NULL END AS current_projection,CASE WHEN octet_length(h.inventory_projection)<=$2 THEN h.inventory_projection ELSE NULL END AS history_projection,CASE WHEN octet_length(i.inventory_projection)<=$2 THEN i.inventory_projection ELSE NULL END AS indexed_projection FROM cmd2_current c LEFT JOIN cmd2_history h ON h.domain=c.domain AND h.subject=c.subject AND h.revision=c.revision AND h.content_digest=c.content_digest LEFT JOIN cmd2_source_index i ON i.domain=c.domain AND i.kind='path' AND i.token=c.subject AND i.path=c.subject AND i.definition_digest=$3 WHERE c.domain=$1 ORDER BY c.subject COLLATE \"C\"",
        &[&domain, &cap, &definition().to_hex()],
    )?;
    let mut transcript = Digest256Hasher::new();
    part(
        &mut transcript,
        b"tos-managed-mixed-source-inventory-coverage-v1",
    );
    let mut count = 0u64;
    let mut ended = false;
    let projections = std::iter::from_fn(|| {
        if ended {
            return None;
        }
        let next = (|| -> DurableResult<Option<(String, Vec<u8>)>> {
            active(deadline, cancelled)?;
            let Some(row) = rows.next().transpose()? else {
                return Ok(None);
            };
            let path: String = row.get("subject");
            let current: Option<Vec<u8>> = row.get("current_projection");
            let history: Option<Vec<u8>> = row.get("history_projection");
            let indexed: Option<Vec<u8>> = row.get("indexed_projection");
            ManagedSourceWorkV1::charge(&mut body_work.current_projection_query_rows_returned, 1)?;
            ManagedSourceWorkV1::charge(
                &mut body_work.current_and_history_projection_bytes_returned,
                current.as_ref().map_or(0, |v| v.len() as u64)
                    + history.as_ref().map_or(0, |v| v.len() as u64),
            )?;
            let current = current.ok_or(DurableError::Corrupt(
                "completed bootstrap projection is absent",
            ))?;
            if history.as_deref() != Some(current.as_slice())
                || indexed.as_deref() != Some(current.as_slice())
            {
                return Err(DurableError::Corrupt(
                    "completed bootstrap retained projection differs",
                ));
            }
            part(&mut transcript, path.as_bytes());
            part(&mut transcript, Digest256::of_bytes(&current).as_bytes());
            count = count.checked_add(1).ok_or(DurableError::Refused(
                "completed bootstrap row count overflow",
            ))?;
            Ok(Some((path, current)))
        })();
        match next {
            Ok(Some(value)) => Some(Ok(value)),
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
    let mut tree_owner_check = || owner_check().is_ok();
    let inventory = super::addressed_inventory::AddressedMixedInventoryV1::build_from_assessed_projections_with_callback(
        store, projections, expected_members, tree_limits, deadline, cancelled, tree_work, &mut tree_owner_check,
    )?;
    drop(rows);
    if count != expected_members || transcript.finalize() != expected_projection {
        return Err(DurableError::Corrupt(
            "completed bootstrap projection transcript differs",
        ));
    }
    owner_check()?;
    ManagedSourceWorkV1::charge(&mut body_work.completed_assessments_reused, 1)?;
    Ok(inventory)
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
    // Count the fields consumed by the binary PG cursors. row_to_json doubles
    // bytea values as hex and adds an unrelated transport envelope; neither is
    // part of a projection value or of this index's logical-byte contract.
    for (position, (table, fields, keys, value)) in [
        (
            "cmd2_source_index",
            "octet_length(domain)::bigint+octet_length(kind)+octet_length(token)+octet_length(path)+octet_length(definition_digest)+coalesce(octet_length(inventory_projection),0)",
            "greatest(octet_length(domain),octet_length(kind),octet_length(token),octet_length(path),octet_length(definition_digest))",
            "coalesce(octet_length(inventory_projection),0)",
        ),
        (
            "cmd2_predicate",
            "octet_length(domain)::bigint+octet_length(kind)+octet_length(owner)+octet_length(scope)+octet_length(token)+octet_length(definition_version)+8+1",
            "greatest(octet_length(domain),octet_length(kind),octet_length(owner),octet_length(scope),octet_length(token),octet_length(definition_version))",
            "0",
        ),
    ]
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
            "SELECT count(*),coalesce(max({keys}),0)::bigint,coalesce(sum({fields}),0)::bigint,coalesce(max({value}),0)::bigint FROM (SELECT * FROM {table} WHERE domain=$1 LIMIT $2) t"
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
        if total_rows > limits.max_rows {
            return Err(DurableError::Refused(
                "streamed owner metadata preflight row bound",
            ));
        }
        if total_bytes > limits.max_logical_bytes {
            return Err(DurableError::Refused(
                "streamed owner metadata preflight logical byte bound",
            ));
        }
        if as_u64(row.get(1))? > limits.max_key_bytes as u64 {
            return Err(DurableError::Refused(
                "streamed owner metadata preflight key byte bound",
            ));
        }
        if as_u64(row.get(3))? > limits.max_value_bytes as u64 {
            return Err(DurableError::Refused(
                "streamed owner metadata preflight projection byte bound",
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
        // The complete semantic seal already exists. These rows and the
        // domain's final readiness commit in this same held transaction; no
        // reader can observe an intermediate grant. Avoid inserting false and
        // then rewriting every fresh predicate through the row audit trigger.
        let written = tx.execute("INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete) VALUES($1,$2,$3,$4,$5,$6,0,true)",
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
    rebuild_projections: bool,
    store: &SegmentStore,
    domain: &str,
    cold: &ColdCut,
    original: &tos_source_store::StreamedCorpusCutReaderV1,
    original_content: &VerifiedStreamedOriginalContentV1,
    context: &CommandContext,
    index: &mut SourceAssessmentIndex,
    limits: SourceAssessmentLimits,
    max_member_bytes: u64,
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
    owner_check: &dyn Fn() -> DurableResult<()>,
) -> DurableResult<(
    CompleteStreamedMixedSourceAssessmentV1,
    super::addressed_inventory::AddressedMixedInventoryV1,
)> {
    let public_text_packages = super::current_public_text_packages(
        tx,
        domain,
        cold.current_members,
        max_statement_ms,
        deadline,
        cancelled,
    )?;
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
        max_member_bytes,
        index_work,
        body_work,
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
            rebuild_projections,
            index: &index_cell,
            index_work: &work_cell,
            body_work,
            max_member_bytes,
            store,
            context,
            domain,
            cold,
            public_text_packages: &public_text_packages,
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
        let first = stage_current_mixed_identities(&mut epoch, &profile, deadline, cancelled)?;
        render_current_mixed_projections(
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
    let mut assessment = compare_current_owner_indexes(
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
    assessment.publication_import = super::legacy_selected_publication_import_from_members(
        &projected.legacy_publication_members,
        domain,
        store.store_id(),
        original_content.revision,
        original_content.membership,
        cold.through_commit_seq,
    )?;
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    if fresh_sink {
        crate::source_text_identity_index::rebuild_current_rows(
            tx,
            domain,
            &projected.text_identity_candidates,
            definition(),
        )?;
    } else {
        crate::source_text_identity_index::verify_rows_for_reassessment(
            tx,
            domain,
            &projected.text_identity_candidates,
            definition(),
        )?;
    }
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    if fresh_sink {
        crate::source_annotation_identity_index::rebuild_current(
            tx,
            domain,
            &projected.annotation_identity_candidates,
            definition(),
        )?;
    } else {
        crate::source_annotation_identity_index::verify_current(
            tx,
            domain,
            &projected.annotation_identity_candidates,
            definition(),
        )?;
    }
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    if fresh_sink {
        crate::source_claim_dependency_index::rebuild_current(
            tx,
            domain,
            &projected.claim_dependency_candidates,
            definition(),
            cold.through_commit_seq,
        )?;
    } else {
        crate::source_claim_dependency_index::verify_for_reassessment(
            tx,
            domain,
            &projected.claim_dependency_candidates,
            definition(),
        )?;
    }
    set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    if fresh_sink {
        crate::source_item_file_index::rebuild_current(
            tx,
            domain,
            &projected.item_file_candidates,
            definition(),
        )?;
    } else {
        crate::source_item_file_index::verify_for_reassessment(
            tx,
            domain,
            &projected.item_file_candidates,
            definition(),
        )?;
    }
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
        owner_check,
    )?;
    active(deadline, cancelled)?;
    Ok((assessment, inventory))
}

// Complete only retained incomplete predicates. A bounded keyset UPDATE avoids
// one enormous trigger queue and repeated audit-fence churn. Every update still
// runs the actual audit trigger under the same held source transaction.
fn complete_owned_predicates_streamed(
    tx: &mut Transaction<'_>,
    max_statement_ms: u64,
    domain: &str,
    definition: &str,
    work: &mut StreamedColdSourceWorkV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    owner_check: &dyn Fn() -> DurableResult<()>,
) -> DurableResult<()> {
    let mut after = (String::new(), String::new(), String::new());
    loop {
        active(deadline, cancelled)?;
        owner_check()?;
        set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let rows = tx.query(
            "WITH page AS MATERIALIZED (
                SELECT kind,scope,token FROM cmd2_predicate
                WHERE domain=$1 AND owner=$2 AND definition_version=$3 AND NOT complete
                  AND (kind COLLATE \"C\",scope COLLATE \"C\",token COLLATE \"C\") > ($4,$5,$6)
                ORDER BY kind COLLATE \"C\",scope COLLATE \"C\",token COLLATE \"C\" LIMIT 256
             ) UPDATE cmd2_predicate p SET complete=true FROM page
             WHERE p.domain=$1 AND p.owner=$2 AND p.definition_version=$3 AND NOT p.complete
               AND p.kind=page.kind AND p.scope=page.scope AND p.token=page.token
             RETURNING p.kind,p.scope,p.token",
            &[&domain, &OWNER, &definition, &after.0, &after.1, &after.2],
        )?;
        if rows.is_empty() {
            return Ok(());
        }
        if rows.len() > 256 {
            return Err(DurableError::Corrupt(
                "predicate completion page exceeds bound",
            ));
        }
        ManagedSourceWorkV1::charge(
            &mut work.predicate_completion_rows_updated,
            rows.len() as u64,
        )?;
        ManagedSourceWorkV1::charge(&mut work.predicate_completion_batches, 1)?;
        let next = rows
            .iter()
            .map(|r| {
                (
                    r.get::<_, String>(0),
                    r.get::<_, String>(1),
                    r.get::<_, String>(2),
                )
            })
            .max()
            .ok_or(DurableError::Corrupt(
                "predicate completion page has no cursor",
            ))?;
        if next <= after {
            return Err(DurableError::Corrupt(
                "predicate completion cursor did not advance",
            ));
        }
        after = next;
    }
}

/// Explicit derived-index limits; these do not enlarge compatibility admission
/// or grant authority. The workspace and generation profile retain their own
/// independent physical limits.
#[derive(Clone, Copy, Debug)]
pub struct StreamedColdSourceLimitsV1 {
    /// Per-source-member read ceiling for this streamed operation. Legacy
    /// callers set the historical 8 MiB profile explicitly; streamed owners
    /// may select a larger bounded member envelope for real source cuts.
    pub max_member_bytes: u64,
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
        if self.max_member_bytes == 0 || self.max_member_bytes > usize::MAX as u64 {
            return Err(DurableError::Refused("streamed source member byte bound"));
        }
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
        let owner_cancelled = std::sync::Arc::clone(&cancelled);
        let owner_check = move || active(deadline, &owner_cancelled);
        self.select_current_source_generation_addressed_streaming_controlled(
            store,
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
            &owner_check,
        )
    }

    pub(crate) fn select_current_source_generation_addressed_streaming_controlled(
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
        owner_check: &dyn Fn() -> DurableResult<()>,
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
            owner_check,
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
        owner_check: &dyn Fn() -> DurableResult<()>,
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
            owner_check,
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
        owner_check: &dyn Fn() -> DurableResult<()>,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        let mut index_work = SourceAssessmentWork::default();
        let result =
            (|| -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
                active(deadline, &cancelled)?;
                owner_check()?;
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
                owner_check()?;
                let original_content = verify_original_content(
                    original,
                    initial_revision,
                    initial_membership,
                    source_limits.max_member_bytes,
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
                owner_check()?;
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
                        != observed.completed_projection.is_some()
                        || row.get::<_, Option<i64>>("source_epoch") != Some(1)
                        || row.get::<_, i64>("rights_version") != 0
                        || row.get::<_, i64>("rule_version") != 0
                        || row.get::<_, String>("contract_digest") != observed.contract.to_hex()
                        || row
                            .get::<_, Option<String>>("selected_generation_digest")
                            .is_some()
                        || row.get::<_, Option<String>>("source_projection_digest")
                            != observed.completed_projection.map(|digest| digest.to_hex())
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
                let rebuild_projections = seed.is_none()
                    && row
                        .get::<_, Option<String>>("source_projection_digest")
                        .is_none();
                if rebuild_projections
                    && row
                        .get::<_, Option<String>>("selected_generation_digest")
                        .is_some()
                {
                    return Err(DurableError::Conflict(
                        "invalidated streamed source retains a selected descriptor",
                    ));
                }
                let completed_projection = seed
                    .as_ref()
                    .and_then(|seed| seed.observation().completed_projection);
                let (assessment, inventory) = if let Some(projection_digest) = completed_projection
                {
                    let inventory = reuse_completed_bootstrap_assessment(
                        &mut tx,
                        store,
                        domain,
                        before.current_members,
                        projection_digest,
                        generation_profile.max_sql_statement_ms,
                        limits,
                        tree_limits,
                        tree_work,
                        body_work,
                        deadline,
                        &cancelled,
                        owner_check,
                    )?;
                    (
                        CompleteStreamedMixedSourceAssessmentV1 {
                            members: before.current_members,
                            projection_digest,
                            publication_import: None,
                        },
                        inventory,
                    )
                } else {
                    let mut index = SourceAssessmentIndex::open(
                        workspace,
                        limits,
                        deadline,
                        std::sync::Arc::clone(&cancelled),
                        &mut index_work,
                    )?;
                    let (assessment, inventory) = assess_held_current_source(
                        &mut tx,
                        generation_profile.max_sql_statement_ms,
                        seed.is_some(),
                        rebuild_projections,
                        store,
                        domain,
                        &before,
                        original,
                        &original_content,
                        context,
                        &mut index,
                        limits,
                        source_limits.max_member_bytes,
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
                        owner_check,
                    )?;
                    super::ensure_source_publication_bootstrap_state(
                        &mut tx,
                        domain,
                        store.store_id(),
                        initial_revision,
                        initial_membership,
                        before.through_commit_seq,
                        cohort.definition,
                        assessment.publication_import.as_ref(),
                    )?;
                    // Locks remain held. Completion includes empty and absent predicates;
                    // only existing owned definitions are re-enabled, never invented grants.
                    active(deadline, &cancelled)?;
                    owner_check()?;
                    set_streamed_pg_limits(
                        &mut tx,
                        generation_profile.max_sql_statement_ms,
                        deadline,
                        &cancelled,
                    )?;
                    complete_owned_predicates_streamed(
                        &mut tx,
                        generation_profile.max_sql_statement_ms,
                        domain,
                        &cohort.definition.to_hex(),
                        body_work,
                        deadline,
                        &cancelled,
                        owner_check,
                    )?;
                    owner_check()?;
                    set_streamed_pg_limits(
                        &mut tx,
                        generation_profile.max_sql_statement_ms,
                        deadline,
                        &cancelled,
                    )?;
                    tx.execute("UPDATE cmd2_domain SET source_complete=true,source_generation=head_seq,source_projection_digest=$2,selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL WHERE domain=$1",
            &[&domain, &assessment.projection_digest.to_hex()])?;
                    (assessment, inventory)
                };
                active(deadline, &cancelled)?;
                owner_check()?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                owner_check()?;
                let completed_audit = lock_audit_fence(&mut tx, domain)?;
                owner_check()?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                tx.commit()?;
                owner_check()?;
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
                owner_check()?;
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
                owner_check()?;
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
                owner_check()?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                let mut tree_owner_check = || owner_check().is_ok();
                let metadata =
                    super::addressed_successor::build_addressed_metadata_tree_with_callback(
                        &mut tx,
                        store,
                        domain,
                        tree_limits,
                        deadline,
                        &cancelled,
                        &mut tree_owner_check,
                    )?;
                owner_check()?;
                let (history, current) =
                    super::addressed_successor::build_addressed_membership_trees_with_callback(
                        store,
                        &verified.installation(store),
                        tree_limits,
                        deadline,
                        &cancelled,
                        &mut tree_owner_check,
                    )?;
                owner_check()?;
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
                owner_check()?;
                set_streamed_pg_limits(
                    &mut tx,
                    generation_profile.max_sql_statement_ms,
                    deadline,
                    &cancelled,
                )?;
                tx.commit()?;
                owner_check()?;
                let candidate = self.install_addressed_selection(
                    store,
                    cut,
                    VerifiedLogFrontier(verified.log_frontier.0.clone()),
                    tree_limits,
                    deadline,
                    &cancelled,
                )?;
                owner_check()?;
                let selected = self.select_addressed_selection_controlled_with_callback(
                    store,
                    candidate,
                    &cohort,
                    None,
                    Some(generation_profile.max_sql_statement_ms),
                    deadline,
                    &cancelled,
                    Some(owner_check),
                )?;
                owner_check()?;
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

#[cfg(test)]
mod predicate_expectation_tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    #[test]
    fn current_predicates_are_staged_before_source_inputs_close() {
        let temporary = tempfile::tempdir().expect("temporary source workspace");
        let workspace = PrivateGenerationWorkspace::open(
            temporary.path(),
            ColdWorkspaceLimits {
                max_scratch_written_bytes: 1_000_000,
                max_run_bytes: 4096,
                max_runs: 2,
                merge_fan_in: 2,
                max_rows: 128,
                max_key_bytes: 4096,
            },
        )
        .expect("private source workspace");
        let limits = SourceAssessmentLimits {
            max_rows: 128,
            max_logical_bytes: 1_000_000,
            max_key_bytes: 4096,
            max_value_bytes: 65_536,
            max_placement_bytes: 65_536,
            max_sqlite_file_bytes: 1_000_000,
            max_vm_steps: 1_000_000,
        };
        let mut work = SourceAssessmentWork::default();
        let mut index = SourceAssessmentIndex::open(
            &workspace,
            limits,
            Instant::now() + Duration::from_secs(20),
            Arc::new(AtomicBool::new(false)),
            &mut work,
        )
        .expect("source assessment index");
        let projected = CompleteMixedProjectionPassV1 {
            members: 0,
            projection_digest: Digest256::of_bytes(b"empty projected source"),
            text_identity_candidates:
                crate::source_text_identity_index::CurrentTextIdentityRows::default(),
            annotation_identity_candidates: BTreeMap::new(),
            claim_dependency_candidates: BTreeMap::new(),
            item_file_candidates: BTreeMap::new(),
            legacy_publication_members: BTreeMap::new(),
        };

        stage_current_owner_predicate_expectations(
            &mut index,
            &mut work,
            &projected,
            false,
            Instant::now() + Duration::from_secs(20),
            &AtomicBool::new(false),
        )
        .expect("stage current owner predicates while open");
        let (_, navigation_tokens) =
            crate::source_claim_dependency_index::current_predicate_tokens(
                &projected.claim_dependency_candidates,
                definition(),
            )
            .expect("current Claim navigation marker");
        assert_eq!(navigation_tokens.len(), 1);

        index
            .finish_source_inputs(&mut work)
            .expect("close source inputs");
        assert!(
            stage_current_owner_predicate_expectations(
                &mut index,
                &mut work,
                &projected,
                false,
                Instant::now() + Duration::from_secs(20),
                &AtomicBool::new(false),
            )
            .is_err()
        );

        let version = definition().to_hex();
        // This optional lineage row is validated by the dedicated publication
        // gate and may appear as a valid retained owner row without a current
        // source-derived expectation.
        assert!(super::super::valid_owned_predicate_shape(
            OWNER,
            "unique",
            "source-publication",
            "current",
        ));
        index
            .observe_predicate_row(
                &super::super::source_cohort_stream_index::AssessmentPredicateRow {
                    kind: "unique".to_owned(),
                    owner: OWNER.to_owned(),
                    scope: "source-publication".to_owned(),
                    token: "current".to_owned(),
                    definition_version: version.clone(),
                },
                &mut work,
            )
            .expect("observe optional publication lineage row");
        assert!(index.finish_predicate_comparison(&mut work).is_err());

        index
            .observe_predicate_row(
                &super::super::source_cohort_stream_index::AssessmentPredicateRow {
                    kind: "range".to_owned(),
                    owner: OWNER.to_owned(),
                    scope: crate::source_claim_dependency_index::NAVIGATION_REFERENCE_SCOPE
                        .to_owned(),
                    token: navigation_tokens
                        .first()
                        .expect("navigation completeness token")
                        .clone(),
                    definition_version: version,
                },
                &mut work,
            )
            .expect("observe required current navigation marker");
        index
            .finish_predicate_comparison(&mut work)
            .expect("require all current source predicates");
    }
}

#[cfg(test)]
mod bounded_claim_context_tests {
    use super::*;

    #[test]
    fn ordinary_global_claim_context_includes_its_capture_schema_closure() {
        let ordinary = claim_context_required_contract_paths(
            "ToS/source-witnesses/relations/formation-research/source-claims.jsonl",
        );
        let capture = crate::source_claims::pg_claim_capture_required_contract_paths();
        assert_eq!(ordinary, capture);
        assert_eq!(
            ordinary,
            vec!["ToS/contracts/provenance-event-v2.schema.json".to_owned()]
        );

        let historical = claim_context_required_contract_paths(
            "ToS/source-witnesses/history/friedrich-nietzsche/jenseits-1886-commission/historical-claims.jsonl",
        );
        assert!(historical.contains(&ordinary[0]));
        assert!(historical.len() > ordinary.len());

        assert!(
            claim_context_required_contract_paths(
                "ToS/source-witnesses/items/example/source-claims.jsonl",
            )
            .is_empty()
        );
    }

    #[test]
    fn bounded_historical_claim_context_selects_only_exact_package_and_referenced_archive() {
        let claim_id = "tos.claim.synthetic".to_owned();
        let direct = historical_owner_direct_names([claim_id.clone()]);
        let form_hash = Digest256::of_bytes(claim_id.as_bytes()).to_hex();
        assert!(direct.contains("historical-claims.jsonl"));
        assert!(direct.contains(&format!("historical-claims.{form_hash}.human-forms.json")));
        assert!(!direct.contains("historical-event.json"));
        assert!(!direct.contains("historical-event.human-forms.json"));
        assert!(!direct.contains("source-revision-history.json"));

        let retained = "ToS/source-witnesses/.record-revisions/selected-history";
        let unrelated = "ToS/source-witnesses/.record-revisions/unrelated-history";
        let history = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_claim_revision_history_v1"),
            ),
            (
                "source_path",
                cmd::string("ToS/source-witnesses/history/example/historical-claims.jsonl"),
            ),
            (
                "receipts",
                tos_foundation::JsonValue::Array(vec![cmd::object(vec![(
                    "archive_path",
                    cmd::string(retained),
                )])]),
            ),
        ]);
        let history_file = SourceFile {
            path: RelativePath::parse(
                "ToS/source-witnesses/history/example/claim-revision-history.json",
            )
            .expect("history path"),
            raw: cmd::canonical(&history).expect("canonical history"),
        };
        let selected = selected_claim_archive_paths(&[history_file]).expect("selected archives");
        assert_eq!(selected, BTreeSet::from([retained.to_owned()]));
        assert!(!selected.contains(unrelated));
    }
}

#[cfg(test)]
mod owner_metadata_budget_tests {
    use super::*;

    #[test]
    #[ignore = "requires an explicitly selected disposable PostgreSQL 16 database"]
    fn postgres_contract_predicate_completion_is_bounded_and_fenced() {
        let url = std::env::var("TOS_METADATA_TEST_PG_URL").expect("disposable PG URL required");
        let mut db = DurablePgCoordinator::connect(&url).unwrap();
        db.initialize_empty_schema().unwrap();
        let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
        let mut tx = client.transaction().unwrap();
        let domain = "bounded-predicate-completion";
        tx.execute(
            "INSERT INTO cmd2_domain(domain,contract_digest) VALUES($1,repeat('a',64))",
            &[&domain],
        )
        .unwrap();
        let version = "b".repeat(64);
        let started = Instant::now();
        // Use the real row audit triggers; each source-owned insert is a
        // separate statement, as in the actual initial import.
        for i in 0..8193 {
            tx.execute("INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version) VALUES($1,'unique',$2,'scope',$3,$4)",
                &[&domain, &OWNER, &format!("token-{i:05}"), &version]).unwrap();
        }
        for (owner, definition, token, complete) in [
            (OWNER, version.as_str(), "already-complete", true),
            ("other-owner", version.as_str(), "other-owner", false),
            (OWNER, "other-definition", "other-definition", false),
        ] {
            tx.execute("INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,complete) VALUES($1,'unique',$2,'scope',$3,$4,$5)",
                &[&domain, &owner, &token, &definition, &complete]).unwrap();
        }
        let seeded_ms = started.elapsed().as_millis();
        let before: i64 = tx
            .query_one(
                "SELECT generation FROM cmd2_audit_fence WHERE domain=$1",
                &[&domain],
            )
            .unwrap()
            .get(0);
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + std::time::Duration::from_secs(90);
        let mut work = StreamedColdSourceWorkV1::default();
        let started = Instant::now();
        complete_owned_predicates_streamed(
            &mut tx,
            5000,
            domain,
            &version,
            &mut work,
            deadline,
            &cancelled,
            &|| Ok(()),
        )
        .unwrap();
        let completed_ms = started.elapsed().as_millis();
        assert_eq!(work.predicate_completion_rows_updated, 8193);
        assert_eq!(work.predicate_completion_batches, 33);
        let after: i64 = tx
            .query_one(
                "SELECT generation FROM cmd2_audit_fence WHERE domain=$1",
                &[&domain],
            )
            .unwrap()
            .get(0);
        assert_eq!(
            after - before,
            8193,
            "every changed row retained its audit fence"
        );
        assert_eq!(
            tx.query_one(
                "SELECT count(*) FROM cmd2_predicate WHERE domain=$1 AND NOT complete",
                &[&domain]
            )
            .unwrap()
            .get::<_, i64>(0),
            2
        );
        let before_work = work;
        complete_owned_predicates_streamed(
            &mut tx,
            5000,
            domain,
            &version,
            &mut work,
            deadline,
            &cancelled,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(
            work, before_work,
            "ready predicates are not rewritten on replay"
        );
        cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(
            complete_owned_predicates_streamed(
                &mut tx,
                5000,
                domain,
                &version,
                &mut work,
                deadline,
                &cancelled,
                &|| Ok(())
            )
            .is_err()
        );
        eprintln!(
            "predicate completion measured rows=8193 batches=33 seed_ms={seeded_ms} completion_ms={completed_ms}; all row audit fences preserved"
        );
        tx.rollback().unwrap();
        assert_eq!(
            client
                .query_one(
                    "SELECT count(*) FROM cmd2_predicate WHERE domain=$1",
                    &[&domain]
                )
                .unwrap()
                .get::<_, i64>(0),
            0,
            "outer source transaction still owns rollback"
        );
    }

    #[test]
    #[ignore = "requires an explicitly selected disposable PostgreSQL 16 database"]
    fn postgres_contract_owner_metadata_logical_fields() {
        let url = std::env::var("TOS_METADATA_TEST_PG_URL").expect("disposable PG URL required");
        let mut client = postgres::Client::connect(&url, postgres::NoTls).expect("connect PG");
        let mut tx = client.transaction().unwrap();
        tx.batch_execute(
            "CREATE TEMP TABLE cmd2_source_index (domain text,kind text,token text,path text,definition_digest char(64),inventory_projection bytea) ON COMMIT DROP;
             CREATE TEMP TABLE cmd2_predicate (domain text,kind text,owner text,scope text,token text,definition_version text,generation bigint,complete boolean) ON COMMIT DROP;",
        ).unwrap();
        let domain = "metadata-budget-test";
        let value = vec![b'X'; 1_048_576];
        tx.execute(
            "INSERT INTO cmd2_source_index VALUES($1,'path','ToS/a','ToS/a',repeat('a',64),$2)",
            &[&domain, &value],
        )
        .unwrap();
        tx.execute("INSERT INTO cmd2_predicate VALUES($1,'identity',$2,'scope','token',repeat('a',64),0,false)", &[&domain,&OWNER]).unwrap();
        let transport: i32 = tx
            .query_one(
                "SELECT octet_length(row_to_json(t)::text) FROM cmd2_source_index t",
                &[],
            )
            .unwrap()
            .get(0);
        assert!(transport as usize > value.len() * 2);
        let limits = SourceAssessmentLimits {
            max_rows: 2,
            max_logical_bytes: 4_194_304,
            max_key_bytes: 4096,
            max_value_bytes: value.len(),
            max_placement_bytes: 16_777_216,
            max_sqlite_file_bytes: 134_217_728,
            max_vm_steps: 200_000_000,
        };
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + std::time::Duration::from_secs(20);
        let check = |tx: &mut Transaction<'_>, bounds| {
            admit_streamed_owner_metadata(tx, 5000, domain, bounds, deadline, &cancelled)
        };
        assert_eq!(check(&mut tx, limits).unwrap(), (1, 1));
        // Reject actual value, combined count and aggregate work overruns.
        assert!(
            check(
                &mut tx,
                SourceAssessmentLimits {
                    max_value_bytes: value.len() - 1,
                    ..limits
                }
            )
            .is_err()
        );
        assert!(
            check(
                &mut tx,
                SourceAssessmentLimits {
                    max_rows: 1,
                    ..limits
                }
            )
            .is_err()
        );
        assert!(
            check(
                &mut tx,
                SourceAssessmentLimits {
                    max_logical_bytes: value.len() as u64,
                    ..limits
                }
            )
            .is_err()
        );
        // Key bounds are UTF-8 bytes, including predicates not owned by us.
        tx.execute(
            "UPDATE cmd2_predicate SET token=$1,owner='other-owner'",
            &[&"я".repeat(2049)],
        )
        .unwrap();
        assert!(check(&mut tx, limits).is_err());
        tx.execute(
            "UPDATE cmd2_predicate SET token='token',owner=$1",
            &[&OWNER],
        )
        .unwrap();
        // An unrelated domain does not consume this cut's bounds.
        tx.execute("INSERT INTO cmd2_source_index VALUES('other','path','ToS/b','ToS/b',repeat('b',64),$1)", &[&vec![0u8; value.len()+1]]).unwrap();
        assert_eq!(check(&mut tx, limits).unwrap(), (1, 1));
        // The current-source cursor has the same logical-field contract.
        // Its binary projection fits even when row_to_json doubles it past
        // the value limit. Exercise the production aggregate and all bounds.
        tx.batch_execute(
            "CREATE TEMP TABLE cmd2_current (domain text,subject text,content_length bigint,profile_id text,profile_version text,content_digest text,custody_domain_digest text,segment_digest text,frame_digest text,sto_receipt_id text,durability_class text,prepare_id bytea,store_id bytea,custody_domain bytea,pin_id bytea,inventory_projection bytea,source_dependencies text[]) ON COMMIT DROP;"
        ).unwrap();
        tx.execute(
            "INSERT INTO cmd2_current VALUES($1,'ToS/a',7,'original','1',repeat('a',64),repeat('b',64),repeat('c',64),repeat('d',64),repeat('e',64),'durable',decode(repeat('00',32),'hex'),decode(repeat('00',16),'hex'),decode('00','hex'),decode(repeat('00',16),'hex'),$2,ARRAY['ToS/dependency'])",
            &[&domain, &value],
        ).unwrap();
        assert!(
            tx.query_one(
                "SELECT octet_length(row_to_json(c)::text) FROM cmd2_current c",
                &[]
            )
            .unwrap()
            .get::<_, i32>(0) as usize
                > value.len() * 2
        );
        let check_current = |tx: &mut Transaction<'_>, bounds, expected, member_bytes| {
            admit_streamed_current_metadata(
                tx,
                5000,
                domain,
                expected,
                bounds,
                member_bytes,
                deadline,
                &cancelled,
            )
        };
        check_current(&mut tx, limits, 1, 7).unwrap();
        assert!(
            check_current(
                &mut tx,
                SourceAssessmentLimits {
                    max_value_bytes: value.len() - 1,
                    ..limits
                },
                1,
                7
            )
            .is_err()
        );
        assert!(
            check_current(
                &mut tx,
                SourceAssessmentLimits {
                    max_logical_bytes: value.len() as u64,
                    ..limits
                },
                1,
                7
            )
            .is_err()
        );
        assert!(
            check_current(
                &mut tx,
                SourceAssessmentLimits {
                    max_rows: 0,
                    ..limits
                },
                1,
                7
            )
            .is_err()
        );
        assert!(check_current(&mut tx, limits, 0, 7).is_err());
        assert!(check_current(&mut tx, limits, 1, 6).is_err());
        tx.execute(
            "UPDATE cmd2_current SET source_dependencies=ARRAY[$1]",
            &[&"я".repeat(2049)],
        )
        .unwrap();
        assert!(check_current(&mut tx, limits, 1, 7).is_err());
        tx.execute("UPDATE cmd2_current SET source_dependencies=ARRAY(SELECT repeat('x',4096) FROM generate_series(1,256))", &[]).unwrap();
        assert!(check_current(&mut tx, limits, 1, 7).is_err());
        tx.execute(
            "UPDATE cmd2_current SET source_dependencies=ARRAY['ToS/dependency']",
            &[],
        )
        .unwrap();
        tx.execute("INSERT INTO cmd2_current SELECT 'other',subject,content_length,profile_id,profile_version,content_digest,custody_domain_digest,segment_digest,frame_digest,sto_receipt_id,durability_class,prepare_id,store_id,custody_domain,pin_id,$1,source_dependencies FROM cmd2_current", &[&vec![0u8;value.len()+1]]).unwrap();
        check_current(&mut tx, limits, 1, 7).unwrap();
        tx.rollback().unwrap();
    }
}

#[cfg(test)]
mod owner_contribution_projection_tests {
    use super::*;

    fn contribution() -> tos_foundation::JsonValue {
        cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_pg_source_inventory_member_contribution_v1"),
            ),
            (
                "records",
                cmd::object(
                    crate::source_claims::MIXED_INVENTORY_RECORD_KINDS
                        .iter()
                        .map(|kind| (*kind, tos_foundation::JsonValue::Array(Vec::new())))
                        .collect(),
                ),
            ),
            ("source_profiles", cmd::object(Vec::new())),
            ("events", cmd::object(Vec::new())),
            ("anchors", cmd::object(Vec::new())),
            ("form", tos_foundation::JsonValue::Null),
            ("claim_owner", tos_foundation::JsonValue::Null),
        ])
    }

    #[test]
    fn supported_owner_contribution_is_wrapped_and_unknown_fields_stay_refused() {
        let path = "ToS/source-witnesses/works/example/source-create-request.json";
        let source = b"{}";
        source_inventory_contribution_projection(path, source, &contribution())
            .expect("supported exact owner contribution");

        let mut extended = contribution();
        cmd::set(
            &mut extended,
            "unrecognized_owner_field",
            tos_foundation::JsonValue::Null,
        )
        .expect("add unknown field");
        assert!(source_inventory_contribution_projection(path, source, &extended).is_err());
    }
}
