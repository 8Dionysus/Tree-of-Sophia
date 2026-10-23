//! Bounded frozen source selections over existing owner cuts and record history.
//! This module writes nothing. Inline policy definitions describe source
//! selection only; neither a schema nor a stored record grants current access.
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{CreationFilesystem, work_transaction as tx};
use crate::source_revisions::{self, RecordVersionReadInput, RevisionFamily};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::query_context::{
    AttributedKnowledge, ChronologyVersion, Cutoff, KnowledgeSelection, SourceSnapshot,
};
use tos_foundation::{
    Digest256, ExactRecordRef, FoundationErrorCode, JsonLimits, JsonMode, JsonValue, RecordVersion,
    SourceRevision, StableId, parse_json_with_state_budget,
};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::item_rules::ItemLimits;
use tos_validation::source_cut::CutWorkerSchemaExecutor;

#[derive(Clone, Copy, Debug)]
pub struct ChronologyLimits {
    pub max_members: usize,
    pub max_context_bytes: usize,
    pub max_resolution_work_bytes: usize,
    pub max_evidence_bytes: usize,
}
impl Default for ChronologyLimits {
    fn default() -> Self {
        Self {
            max_members: 128,
            max_context_bytes: 33_554_432,
            max_resolution_work_bytes: 134_217_728,
            max_evidence_bytes: 1_048_576,
        }
    }
}
/// Verified cut/member/policy evidence. Construction alone is not persistence.
pub struct BoundChronology {
    chronology: ChronologyVersion,
    evidence: JsonValue,
}
impl BoundChronology {
    pub fn chronology(&self) -> &ChronologyVersion {
        &self.chronology
    }
    pub fn evidence(&self) -> &JsonValue {
        &self.evidence
    }
}
/// Actual owner record and origin receipt read while the existing writer mutex
/// remains held. Recheck current authority immediately before disclosure.
pub struct LoadedChronology<'a> {
    bound: BoundChronology,
    filesystem: &'a CreationFilesystem,
    context: CommandContext,
    fence: tx::WorkCorpusFence<'a>,
    owner_evidence: JsonValue,
    members: RefCell<crate::source_claims::PublicExactClaimRead>,
}
/// Private comparison failures contain no configured identity or source bytes.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ConfiguredOwnerIdentityError {
    Mismatch,
    Resource,
    CorruptRetainedConfiguration,
}
impl LoadedChronology<'_> {
    pub fn bound(&self) -> &BoundChronology {
        &self.bound
    }
    pub fn owner_evidence(&self) -> &JsonValue {
        &self.owner_evidence
    }
    /// Compare only the configuration already authenticated by this object's loader.
    /// The caller must debit the full child allowance from its existing operation
    /// budget before calling, without refund. Success is not a currentness token:
    /// retain this same Loaded and its existing rechecks through disclosure.
    pub(crate) fn compare_retained_owner_identity(
        &self,
        expected_principal: &str,
        expected_authority: &str,
        prepaid_owned_bytes: usize,
        prepaid_json_visits: usize,
    ) -> std::result::Result<(), ConfiguredOwnerIdentityError> {
        // These are debits of the parent's allowance, never a new/default budget.
        // Fixed workspace covers document/result/limits and borrowed lookup state.
        const CHILD_BYTES: usize = 262_144;
        const CHILD_VISITS: usize = 8192;
        const FIXED_BYTES: usize = 4096;
        if prepaid_owned_bytes != CHILD_BYTES
            || prepaid_json_visits != CHILD_VISITS
            || self.context.configuration_raw.len() > 65_536
            || expected_principal.len() > 1024
            || expected_authority.len() > 1024
        {
            return Err(ConfiguredOwnerIdentityError::Resource);
        }
        let parser_bytes = prepaid_owned_bytes
            .checked_sub(FIXED_BYTES)
            .ok_or(ConfiguredOwnerIdentityError::Resource)?;
        let document = parse_json_with_state_budget(
            &self.context.configuration_raw,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: 65_536,
                max_depth: 64,
                max_visits: prepaid_json_visits,
                max_integer_digits: 4096,
            },
            parser_bytes,
        )
        .map_err(|error| {
            if error.code == FoundationErrorCode::BudgetExceeded {
                ConfiguredOwnerIdentityError::Resource
            } else {
                ConfiguredOwnerIdentityError::CorruptRetainedConfiguration
            }
        })?;
        let mut remaining_visits = prepaid_json_visits
            .checked_sub(document.visits())
            .ok_or(ConfiguredOwnerIdentityError::Resource)?;
        let fields = document
            .root()
            .as_object()
            .ok_or(ConfiguredOwnerIdentityError::CorruptRetainedConfiguration)?;
        // Strict parsing rejects duplicates. Borrowed scans avoid temporary
        // object_get keys, copied identities or reparsing during either lookup.
        // Both scans share the parser's remaining visits. Charge each field,
        // including a match, before examining it; exhaustion precedes inspection.
        let mut identity = |name: &str| {
            for (key, value) in fields {
                remaining_visits = remaining_visits
                    .checked_sub(1)
                    .ok_or(ConfiguredOwnerIdentityError::Resource)?;
                if key.as_str() == Some(name) {
                    return value
                        .as_str()
                        .filter(|value| !value.is_empty())
                        .ok_or(ConfiguredOwnerIdentityError::CorruptRetainedConfiguration);
                }
            }
            Err(ConfiguredOwnerIdentityError::CorruptRetainedConfiguration)
        };
        let principal = identity("principal_id")?;
        let authority = identity("authority_ref")?;
        // The authenticated loader already checked nonblank configured strings.
        // Preserve their exact text; neither trimming nor issuer aliasing applies.
        if principal != expected_principal || authority != expected_authority {
            return Err(ConfiguredOwnerIdentityError::Mismatch);
        }
        Ok(())
    }
    pub fn recheck(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        self.filesystem
            .current_context(&self.context, deadline, cancelled)?;
        self.fence.verify(deadline, cancelled)?;
        current_bytes(self.filesystem, &self.context, deadline, cancelled)?;
        self.members
            .borrow()
            .recheck(self.filesystem, deadline, cancelled)
    }
}
fn invalid(reason: &'static str) -> SourceCommandError {
    SourceCommandError::Invalid(reason)
}
fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
        return Err(SourceCommandError::Denied(
            "Chronology deadline or cancellation",
        ));
    }
    Ok(())
}
fn limits(l: ChronologyLimits) -> SourceCommandResult<()> {
    if l.max_members == 0
        || l.max_members > 128
        || l.max_context_bytes == 0
        || l.max_context_bytes > 33_554_432
        || l.max_resolution_work_bytes == 0
        || l.max_resolution_work_bytes > 134_217_728
        || l.max_evidence_bytes == 0
        || l.max_evidence_bytes > 1_048_576
    {
        return Err(SourceCommandError::Unsupported("Chronology finite limits"));
    }
    Ok(())
}
fn context_bytes(ctx: &CommandContext, l: ChronologyLimits) -> SourceCommandResult<usize> {
    ctx.check()?;
    let count = ctx
        .files
        .iter()
        .try_fold(0usize, |n, f| n.checked_add(f.raw.len()))
        .filter(|n| *n <= l.max_context_bytes)
        .ok_or(SourceCommandError::Unsupported(
            "Chronology context byte budget",
        ))?;
    Ok(count)
}
fn exact(v: &JsonValue) -> SourceCommandResult<ExactRecordRef> {
    cmd::exact_keys(v, &["id", "version", "digest"])?;
    Ok(ExactRecordRef {
        id: StableId::parse(cmd::text(v, "id")?)
            .map_err(|_| invalid("Chronology exact identity"))?,
        version: RecordVersion::new(
            cmd::field(v, "version")?
                .as_u64()
                .ok_or(invalid("Chronology integer version"))?,
        )
        .map_err(|_| invalid("Chronology positive version"))?,
        digest: Digest256::from_prefixed(cmd::text(v, "digest")?)
            .map_err(|_| invalid("Chronology exact digest"))?,
    })
}
fn wire(r: &ExactRecordRef) -> JsonValue {
    cmd::object(vec![
        ("id", cmd::string(r.id.as_str())),
        ("version", cmd::number(r.version.get())),
        ("digest", cmd::string(&r.digest.to_prefixed())),
    ])
}
fn cutoff(v: &JsonValue) -> SourceCommandResult<Cutoff> {
    cmd::exact_keys(v, &["value", "trs"])?;
    let value = cmd::field(v, "value")?;
    if value.is_null()
        || value
            .as_str()
            .is_some_and(|s| s.trim().is_empty() || s == "Current")
        || ["type", "state", "kind"]
            .iter()
            .any(|k| value.object_get(k).and_then(JsonValue::as_str) == Some("Current"))
    {
        return Err(invalid("Chronology requires concrete cutoff"));
    }
    let trs = cmd::field(v, "trs")?;
    Ok(Cutoff {
        value: value.clone(),
        trs: if trs.is_null() {
            None
        } else {
            Some(exact(trs)?)
        },
    })
}
fn policy(v: &JsonValue, kind: &str, profile: &str) -> SourceCommandResult<ExactRecordRef> {
    cmd::exact_keys(v, &["definition", "reference", "scope"])?;
    if cmd::text(v, "scope")? != "InlineSourceDefinitionNotAuthorization" {
        return Err(invalid("Chronology policy scope"));
    }
    let d = cmd::field(v, "definition")?;
    cmd::exact_keys(
        d,
        &[
            "schema_version",
            "policy_id",
            "policy_version",
            "kind",
            "profile",
            "rules",
            "basis",
        ],
    )?;
    if cmd::text(d, "schema_version")? != "tos_chronology_source_policy_v1"
        || cmd::text(d, "kind")? != kind
        || cmd::text(d, "profile")? != profile
    {
        return Err(SourceCommandError::Unsupported(
            "Chronology source policy profile",
        ));
    }
    let rules = cmd::field(d, "rules")?;
    cmd::exact_keys(rules, &["membership", "history", "ordering"])?;
    if cmd::text(rules, "membership")? != "PresentInPinnedCut"
        || cmd::text(rules, "history")? != "ExactRevisionOnly"
        || cmd::text(rules, "ordering")? != "DeclaredSequence"
    {
        return Err(SourceCommandError::Unsupported(
            "Chronology source policy rules",
        ));
    }
    let r = exact(cmd::field(v, "reference")?)?;
    if r.id.as_str() != cmd::text(d, "policy_id")?
        || r.version.get()
            != cmd::field(d, "policy_version")?
                .as_u64()
                .ok_or(invalid("Chronology policy integer version"))?
        || r.digest != cmd::record_digest(d)?
    {
        return Err(SourceCommandError::Conflict(
            "Chronology inline policy canonical binding",
        ));
    }
    Ok(r)
}
/// Join an already owner-verified packet to actual frozen physical bytes.
/// The live catalog is never represented as a catalog of this frozen cut.
fn verify_pinned_member_packet(
    record: &JsonValue,
    provenance: &JsonValue,
    cut: &CorpusCutReader,
    l: ChronologyLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let source = cmd::field(provenance, "source")?;
    let digest = Digest256::from_prefixed(cmd::text(source, "stream_sha256")?)
        .map_err(|_| invalid("Chronology verified stream digest"))?;
    let size = cmd::field(source, "stream_bytes")?
        .as_u64()
        .ok_or(invalid("Chronology verified stream size"))?;
    let line = cmd::field(source, "line")?
        .as_u64()
        .and_then(|n| n.checked_sub(1))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(invalid("Chronology verified stream line"))?;
    if size > 8_388_608 || size > l.max_context_bytes as u64 {
        return Err(SourceCommandError::Unsupported(
            "Chronology frozen member byte budget",
        ));
    }
    let stream = cmd::text(source, "source_ref")?;
    let archive = cmd::field(source, "archive_blob_ref")?.as_str();
    for locator in std::iter::once(stream).chain(archive) {
        active(deadline, cancelled)?;
        let path = tos_foundation::RelativePath::parse(locator)
            .map_err(|_| invalid("Chronology verified stream locator"))?;
        let Some(member) = cut.current().member(&path) else {
            continue;
        };
        if member.sha256 != digest || member.size_bytes != size {
            continue;
        }
        let raw = cut
            .read_member(cut.current().revision(), &path, size, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Chronology frozen stream custody read"))?
            .raw;
        if Digest256::of_bytes(&raw) != digest || raw.len() as u64 != size {
            return Err(SourceCommandError::Conflict(
                "Chronology frozen stream bytes differ",
            ));
        }
        let row = raw
            .split(|b| *b == b'\n')
            .nth(line)
            .ok_or(invalid("Chronology frozen stream line absent"))?;
        if !cmd::same(&cmd::parse(row)?, record)? {
            return Err(SourceCommandError::Conflict(
                "Chronology frozen exact packet differs",
            ));
        }
        return Ok(cmd::object(vec![
            (
                "source_revision",
                cmd::string(&cut.current().revision().0.to_prefixed()),
            ),
            ("source_ref", cmd::string(locator)),
            ("stream_sha256", cmd::string(&digest.to_prefixed())),
            ("stream_bytes", cmd::number(size)),
            ("line", cmd::number((line as u64) + 1)),
        ]));
    }
    Err(SourceCommandError::Conflict(
        "Chronology exact packet absent from frozen stream/archive",
    ))
}
/// Binds exact public Claim revisions to the real pinned cut. `pinned` is an
/// existing selected owner context, not an arbitrary verified-status envelope.
/// Its source/software custody is repeated here. The logical snapshot name is
/// owner declared; only cut revision and actual members have physical proof.
#[allow(clippy::too_many_arguments)]
pub fn bind_chronology_record_from_cut(
    held: &LoadedChronology<'_>,
    record: &JsonValue,
    pinned: &CommandContext,
    cut: &CorpusCutReader,
    snapshot: &SourceSnapshot,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    l: ChronologyLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BoundChronology> {
    held.recheck(deadline, cancelled)?;
    let result = bind_chronology_record_inner(
        record,
        pinned,
        cut,
        snapshot,
        software,
        components,
        l,
        deadline,
        cancelled,
        |reference| {
            held.members
                .borrow_mut()
                .resolve(reference, deadline, cancelled)
        },
    )?;
    held.recheck(deadline, cancelled)?;
    Ok(result)
}
#[allow(clippy::too_many_arguments)]
fn bind_chronology_record_inner(
    record: &JsonValue,
    pinned: &CommandContext,
    cut: &CorpusCutReader,
    snapshot: &SourceSnapshot,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    l: ChronologyLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut resolve: impl FnMut(&JsonValue) -> SourceCommandResult<(JsonValue, JsonValue, &'static str)>,
) -> SourceCommandResult<BoundChronology> {
    limits(l)?;
    active(deadline, cancelled)?;
    let bytes = context_bytes(pinned, l)?;
    pinned.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    if cmd::text(record, "schema_version")? != "tos_chronology_version_record_v1"
        || cmd::text(record, "record_type")? != "chronology-version"
        || !matches!(
            cmd::text(record, "visibility")?,
            "public" | "public_metadata_only"
        )
    {
        return Err(SourceCommandError::Denied(
            "Chronology public source profile",
        ));
    }
    let reference = exact(&crate::source_forms::metadata_subject(record)?)?;
    let c = cmd::field(record, "semantic_content")?;
    cmd::exact_keys(
        c,
        &[
            "schema_version",
            "source_snapshot",
            "system_as_of",
            "attributed_knowledge",
            "source_publication_as_of",
            "selection_mode",
            "members",
            "policies",
        ],
    )?;
    if cmd::text(c, "schema_version")? != "tos_frozen_chronology_v1"
        || cmd::text(c, "selection_mode")? != "ExplicitExactMembers"
    {
        return Err(SourceCommandError::Unsupported(
            "Chronology explicit member selection profile",
        ));
    }
    let source = cmd::field(c, "source_snapshot")?;
    cmd::exact_keys(source, &["id", "revision"])?;
    let revision = SourceRevision(
        Digest256::from_prefixed(cmd::text(source, "revision")?)
            .map_err(|_| invalid("Chronology source fence"))?,
    );
    if revision != cut.current().revision()
        || revision != snapshot.reference.revision
        || cmd::text(source, "id")? != snapshot.reference.id.as_str()
    {
        return Err(SourceCommandError::Conflict(
            "Chronology pinned snapshot identity or source fence",
        ));
    }
    let policies = cmd::field(c, "policies")?;
    cmd::exact_keys(policies, &["evidence", "selection"])?;
    let evidence_policy = policy(
        cmd::field(policies, "evidence")?,
        "Evidence",
        "ExactPinnedMemberEvidence-v1",
    )?;
    let selection_policy = policy(
        cmd::field(policies, "selection")?,
        "Selection",
        "ExplicitExactMembers-v1",
    )?;
    let system_as_of = cutoff(cmd::field(c, "system_as_of")?)?;
    let a = cmd::field(c, "attributed_knowledge")?;
    let attributed_knowledge = if a.is_null() {
        None
    } else {
        cmd::exact_keys(a, &["knower", "cutoff"])?;
        Some(AttributedKnowledge {
            knower: StableId::parse(cmd::text(a, "knower")?)
                .map_err(|_| invalid("Chronology knower"))?,
            cutoff: cutoff(cmd::field(a, "cutoff")?)?,
        })
    };
    let publication = cmd::field(c, "source_publication_as_of")?;
    let source_publication_as_of = if publication.is_null() {
        None
    } else {
        Some(cutoff(publication)?)
    };
    let members = cmd::array(c, "members")?;
    if members.is_empty()
        || members.len() > l.max_members
        || bytes
            .checked_mul(members.len())
            .is_none_or(|n| n > l.max_resolution_work_bytes)
    {
        return Err(SourceCommandError::Unsupported(
            "Chronology member or resolution work budget",
        ));
    }
    let mut unique = BTreeSet::new();
    let mut refs = Vec::new();
    let mut evidence = Vec::new();
    for member in members {
        active(deadline, cancelled)?;
        let r = exact(member)?;
        if !unique.insert(r.clone()) {
            return Err(invalid("Chronology repeated member revision"));
        }
        let (record, provenance, status) = resolve(member)?;
        let frozen_source =
            verify_pinned_member_packet(&record, &provenance, cut, l, deadline, cancelled)?;
        evidence.push(cmd::object(vec![
            ("reference", member.clone()),
            ("record", record),
            ("version_status", cmd::string(status)),
            ("provenance", provenance),
            ("frozen_source", frozen_source),
        ]));
        refs.push(r);
        if cmd::canonical(&JsonValue::Array(evidence.clone()))?.len() > l.max_evidence_bytes {
            return Err(SourceCommandError::Unsupported(
                "Chronology evidence byte budget",
            ));
        }
    }
    let chronology = ChronologyVersion {
        reference,
        selection: KnowledgeSelection {
            source_snapshot: snapshot.reference.clone(),
            system_as_of,
            attributed_knowledge,
            source_publication_as_of,
        },
        evidence_policy,
        selection_policy,
        member_claim_revisions: refs,
    };
    let evidence = cmd::object(vec![
        ("scope", cmd::string("verified-pinned-cut-explicit-members")),
        ("source_revision", cmd::string(&revision.0.to_prefixed())),
        (
            "snapshot_name_scope",
            cmd::string("owner-declared-logical-label"),
        ),
        ("members", JsonValue::Array(evidence)),
        (
            "policy_scope",
            cmd::string("InlineSourceDefinitionNotAuthorization"),
        ),
        ("persisted", JsonValue::Bool(false)),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    active(deadline, cancelled)?;
    Ok(BoundChronology {
        chronology,
        evidence,
    })
}
fn current_bytes(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for f in ctx
        .files
        .iter()
        .filter(|f| f.path.as_str().starts_with("ToS/"))
    {
        active(deadline, cancelled)?;
        let (parent, name) = f
            .path
            .as_str()
            .rsplit_once('/')
            .ok_or(invalid("Chronology owner path"))?;
        let directory = tx::read_existing_parent(fs, parent)?.ok_or(
            SourceCommandError::Conflict("Chronology current selected parent absent"),
        )?;
        let raw = tx::read_at(
            &directory,
            name,
            fs.protected_uid(),
            8_388_608,
            deadline,
            cancelled,
        )?
        .ok_or(SourceCommandError::Conflict(
            "Chronology current selected file absent",
        ))?;
        if raw != f.raw {
            return Err(SourceCommandError::Conflict(
                "Chronology current selected bytes changed",
            ));
        }
    }
    Ok(())
}
/// Load an actual public owner record/retained version, with current protected
/// Unix/configuration and physical-source checks under the existing mutex.
/// No new lock is created, and no caller permission boolean is accepted.
#[allow(clippy::too_many_arguments)]
pub fn load_chronology_from_owner<'a>(
    fs: &'a CreationFilesystem,
    current: &CommandContext,
    current_cut: &CorpusCutReader,
    requested: &ExactRecordRef,
    pinned: &CommandContext,
    pinned_cut: &CorpusCutReader,
    snapshot: &SourceSnapshot,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    l: ChronologyLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<LoadedChronology<'a>> {
    limits(l)?;
    let current_size = context_bytes(current, l)?;
    current.check_from_selected_captures(current_cut, software, components, deadline, cancelled)?;
    fs.current_context(current, deadline, cancelled)?;
    let (config, family) = source_revisions::configuration(current)?;
    if !matches!(
        family,
        RevisionFamily::PublicProfile | RevisionFamily::PublicProfileScope
    ) || cmd::text(&config, "profile_type_id")? != "tos.entity.chronology-version"
        || cmd::text(&config, "record_id")? != requested.id.as_str()
        || !matches!(
            cmd::text(&cmd::parse(&current.request_raw)?, "operation")?,
            "describe" | "inspect-version"
        )
    {
        return Err(SourceCommandError::Denied(
            "Chronology existing public owner read scope",
        ));
    }
    let fence = tx::WorkCorpusFence::hold_existing(fs, deadline, cancelled)?;
    current_bytes(fs, current, deadline, cancelled)?;
    let path = cmd::text(&config, "source_path")?;
    let input = RecordVersionReadInput {
        files: &current.files,
        source_revision: current.base_revision,
        effective_uid: current.effective_uid,
        schema_source_path: path,
    };
    let item_limits = ItemLimits {
        max_member_bytes: 8_388_608,
        max_total_bytes: l.max_context_bytes as u64,
        max_state_bytes: l.max_context_bytes,
        max_issues: 32,
        deadline,
    };
    let resolved = source_revisions::resolve_record_version_readonly(
        &input,
        path,
        item_limits,
        &wire(requested),
        worker,
        deadline,
        cancelled,
    )?;
    let current_record = match resolved.version_status {
        "current" => &resolved.record,
        "historical" => resolved
            .current_record
            .as_ref()
            .ok_or(invalid("Chronology current record missing"))?,
        _ => {
            return Err(SourceCommandError::Unsupported(
                "Chronology metadata version status",
            ));
        }
    };
    for record in [&resolved.record, current_record] {
        if !matches!(
            cmd::text(record, "visibility")?,
            "public" | "public_metadata_only"
        ) {
            return Err(SourceCommandError::Denied(
                "Chronology current or historical record is not public",
            ));
        }
    }
    let mut members = crate::source_claims::PublicExactClaimRead::new(
        fs,
        current,
        current_cut,
        worker,
        deadline,
        cancelled,
    )?;
    let bound = bind_chronology_record_inner(
        &resolved.record,
        pinned,
        pinned_cut,
        snapshot,
        software,
        components,
        l,
        deadline,
        cancelled,
        |reference| members.resolve(reference, deadline, cancelled),
    )?;
    let pinned_size = context_bytes(pinned, l)?;
    let work = current_size
        .checked_add(pinned_size)
        .and_then(|n| n.checked_mul(bound.chronology.member_claim_revisions.len()));
    if work.is_none_or(|n| n > l.max_resolution_work_bytes) {
        return Err(SourceCommandError::Unsupported(
            "Chronology aggregate member resolution budget",
        ));
    }
    let parent = path
        .rsplit_once('/')
        .ok_or(invalid("Chronology owner parent"))?
        .0;
    let receipt_path =
        tos_foundation::RelativePath::parse(&format!("{parent}/source-create-receipt.json"))
            .map_err(|_| invalid("Chronology receipt path"))?;
    let receipt = cmd::parse(
        current
            .file(&receipt_path)?
            .ok_or(SourceCommandError::Conflict(
                "Chronology actual creation receipt absent",
            ))?,
    )?;
    if cmd::text(&receipt, "schema_version")? != "tos_local_source_create_receipt_v1"
        || cmd::text(&receipt, "source_path")? != path
        || cmd::field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
    {
        return Err(SourceCommandError::Conflict(
            "Chronology creation receipt scope",
        ));
    }
    let origin = source_revisions::resolve_record_version_readonly(
        &input,
        path,
        item_limits,
        cmd::field(&receipt, "source")?,
        worker,
        deadline,
        cancelled,
    )?;
    if cmd::integer(&origin.record, "record_version")? != 1 {
        return Err(invalid("Chronology creation origin must be version one"));
    }
    let base = path
        .rsplit('/')
        .next()
        .ok_or(invalid("Chronology source basename"))?;
    let binding = cmd::field(cmd::field(&receipt, "files")?, base)?;
    if cmd::field(binding, "sha256")? != cmd::field(&origin.source, "record_sha256")?
        || cmd::field(binding, "bytes")? != cmd::field(&origin.source, "record_bytes")?
    {
        return Err(SourceCommandError::Conflict(
            "Chronology creation physical origin binding",
        ));
    }
    let request_path =
        tos_foundation::RelativePath::parse(&format!("{parent}/source-create-request.json"))
            .map_err(|_| invalid("Chronology original request path"))?;
    let request = cmd::parse(
        current
            .file(&request_path)?
            .ok_or(SourceCommandError::Conflict(
                "Chronology actual creation request absent",
            ))?,
    )?;
    if cmd::text(&request, "operation")? != "source.create"
        || cmd::text(&receipt, "request_digest")? != cmd::record_digest(&request)?.to_prefixed()
        || !cmd::same(
            &crate::source_forms::metadata_subject(cmd::field(&request, "record")?)?,
            cmd::field(&receipt, "source")?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "Chronology creation request binding",
        ));
    }
    let owner_evidence = cmd::object(vec![
        ("source", resolved.source),
        ("history", resolved.history),
        ("transition", resolved.transition),
        ("creation_receipt", receipt),
        ("source_persisted", JsonValue::Bool(true)),
        (
            "scope",
            cmd::string("actual-owner-record-cut-members-origin-receipt"),
        ),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    if cmd::canonical(&owner_evidence)?.len() > l.max_evidence_bytes {
        return Err(SourceCommandError::Unsupported(
            "Chronology owner evidence byte budget",
        ));
    }
    let result = LoadedChronology {
        bound,
        filesystem: fs,
        context: current.clone(),
        fence,
        owner_evidence,
        members: RefCell::new(members),
    };
    result.recheck(deadline, cancelled)?;
    Ok(result)
}
