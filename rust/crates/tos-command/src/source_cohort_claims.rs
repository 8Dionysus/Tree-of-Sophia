//! Complete current authored inventory for Global Claims PG operations.
//!
//! Claims grounding is a whole-source contract.  This reader deliberately
//! seals that complete view inside one existing bounded PG read epoch rather
//! than treating a set of addressed reads as a complete inventory.

use super::revision::{SourceRevisionReadSession, SourceRevisionReadSet};
use super::{
    IndexRows, PreparedPgSourceRevision, SourceRevisionWrite, revision_form_ids, revision_indexes,
};
use crate::source_command::{
    self as cmd, CommandContext, PreparedCommand, SELECTED_SOURCE_MAX_BYTES,
    SELECTED_SOURCE_MAX_FILES, SELECTED_SOURCE_MAX_MEMBER_BYTES, SourceFile,
};
use crate::{DurableError, DurableResult, source_claims};
use postgres::Transaction;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};

/// Opaque complete authored cut, produced only by recursive current reads.
/// A caller cannot substitute an addressed-record list for this inventory.
#[derive(Clone, Debug)]
pub(crate) struct PgCompleteClaimCut {
    base_revision: SourceRevision,
    identity: super::revision::PgRecordRevisionIdentity,
    members: Vec<SourceFile>,
    directories: BTreeSet<String>,
    total_bytes: u64,
}

impl PgCompleteClaimCut {
    pub(crate) fn members(&self) -> &[SourceFile] {
        &self.members
    }

    pub(crate) fn base_revision(&self) -> SourceRevision {
        self.base_revision
    }

    pub(crate) fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub(crate) fn identity(&self) -> super::revision::PgRecordRevisionIdentity {
        self.identity
    }

    pub(crate) fn verify_context(
        &self,
        revision: SourceRevision,
        files: &[SourceFile],
    ) -> DurableResult<()> {
        let authored = files
            .iter()
            .filter(|file| file.path.as_str().starts_with("ToS/"))
            .cloned()
            .collect::<Vec<_>>();
        if revision != self.base_revision || authored != self.members {
            return Err(DurableError::Conflict(
                "Global Claims context differs from complete PG source cut",
            ));
        }
        Ok(())
    }

    /// Bind the owner package back to the exact exhaustive file and directory
    /// observations collected before planning. The extra source-home reads
    /// make additions/deletions conflict at the same serializable boundary.
    pub(crate) fn verify_readset(&self, readset: &SourceRevisionReadSet) -> DurableResult<()> {
        if readset.selection.identity() != self.identity
            || readset.selection.cohort.initial_revision != self.base_revision
            || readset.observations.len() != self.members.len()
        {
            return Err(DurableError::Conflict(
                "Global Claims complete cut readset size differs",
            ));
        }
        for file in &self.members {
            let Some(observation) = readset.observations.get(file.path.as_str()) else {
                return Err(DurableError::Conflict(
                    "Global Claims complete cut member was not observed",
                ));
            };
            if observation.member.raw != file.raw || observation.member.path != file.path {
                return Err(DurableError::Conflict(
                    "Global Claims complete cut member observation differs",
                ));
            }
        }
        for directory in &self.directories {
            let observed = readset.reads.reads.iter().any(|read| {
                matches!(read, crate::PredicateRead::Generation { predicate, .. }
                    if predicate.kind == crate::PredicateKind::Range
                        && predicate.owner == super::OWNER
                        && predicate.scope == "source-home"
                        && predicate.token == *directory
                        && predicate.definition_version == readset.selection.cohort.definition.to_hex())
            });
            if !observed {
                return Err(DurableError::Conflict(
                    "Global Claims directory generation was not observed",
                ));
            }
        }
        Ok(())
    }
}

/// Read every authored file and every immediate-child range under `ToS` in
/// lexicographic order, with the same hard limits as the Claim owner cut.
pub(crate) fn read_pg_complete_claim_cut(
    session: &mut SourceRevisionReadSession<'_>,
    reserved_members: usize,
    reserved_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<PgCompleteClaimCut> {
    let member_limit = SELECTED_SOURCE_MAX_FILES
        .checked_sub(reserved_members)
        .filter(|limit| *limit > 0)
        .ok_or(DurableError::Refused(
            "Global Claims complete member budget exceeded",
        ))?;
    let authored_byte_limit = (SELECTED_SOURCE_MAX_BYTES as u64)
        .checked_sub(reserved_bytes)
        .ok_or(DurableError::Refused(
            "Global Claims complete byte budget exceeded",
        ))?;
    // Empty directories do not consume the owner member budget. Keep a
    // separate hard ceiling so a valid small member cut is not refused just
    // because its authored tree contains more directory nodes than files.
    let directory_limit = SELECTED_SOURCE_MAX_FILES
        .checked_add(1)
        .ok_or(DurableError::Refused(
            "Global Claims complete directory budget exceeded",
        ))?;
    let base_revision = session.selection().cohort.initial_revision;
    let identity = session.selection().identity();
    let mut pending = vec![
        RelativePath::parse("ToS")
            .map_err(|_| DurableError::Corrupt("Global Claims root path invalid"))?,
    ];
    let mut directories = BTreeSet::new();
    let mut files = BTreeMap::new();
    let mut total_bytes = 0u64;
    while let Some(parent) = pending.pop() {
        if !directories.insert(parent.as_str().to_owned()) {
            return Err(DurableError::Corrupt(
                "Global Claims directory visited more than once",
            ));
        }
        if directories.len() > directory_limit {
            return Err(DurableError::Refused(
                "Global Claims complete directory budget exceeded",
            ));
        }
        let children = session.list_immediate_children(&parent, deadline, cancelled)?;
        // Reverse the sorted range so stack traversal remains path ordered.
        for (name, is_directory) in children.into_iter().rev() {
            let path = RelativePath::parse(&format!("{}/{name}", parent.as_str()))
                .map_err(|_| DurableError::Refused("Global Claims path is invalid"))?;
            if is_directory {
                pending.push(path);
                continue;
            }
            if files.len() >= member_limit {
                return Err(DurableError::Refused(
                    "Global Claims complete member budget exceeded",
                ));
            }
            crate::source_claims::validate_pg_complete_member(&path)
                .map_err(|_| DurableError::Refused("Global Claims member is not admissible"))?;
            let remaining =
                authored_byte_limit
                    .checked_sub(total_bytes)
                    .ok_or(DurableError::Refused(
                        "Global Claims complete byte budget exceeded",
                    ))?;
            let member = session.read_current_member(
                &path,
                (SELECTED_SOURCE_MAX_MEMBER_BYTES as u64)
                    .min(max_member_bytes)
                    .min(remaining),
                deadline,
                cancelled,
            )?;
            total_bytes = total_bytes
                .checked_add(member.raw.len() as u64)
                .filter(|bytes| *bytes <= authored_byte_limit)
                .ok_or(DurableError::Refused(
                    "Global Claims complete byte budget exceeded",
                ))?;
            if files
                .insert(
                    member.path.as_str().to_owned(),
                    SourceFile {
                        path: member.path,
                        raw: member.raw,
                    },
                )
                .is_some()
            {
                return Err(DurableError::Corrupt(
                    "Global Claims complete path duplicated",
                ));
            }
        }
    }
    let members = files.into_values().collect::<Vec<_>>();
    if members
        .iter()
        .any(|file| !file.path.as_str().starts_with("ToS/"))
    {
        return Err(DurableError::Corrupt(
            "Global Claims complete inventory escaped authored namespace",
        ));
    }
    Ok(PgCompleteClaimCut {
        base_revision,
        identity,
        members,
        directories,
        total_bytes,
    })
}

/// Owner-prepared Global Claim package entering the shared atomic revision
/// writer. The owner still constructs and validates every byte; this type
/// only seals the exact source epoch, family tag and durable index effect.
pub(crate) struct PreparedPgClaimCommand {
    context: CommandContext,
    proposal: PreparedCommand,
    cut: PgCompleteClaimCut,
    family_tag: String,
    semantic_indexes: IndexRows,
    owner_contributions: BTreeMap<String, tos_foundation::JsonValue>,
}

impl PreparedPgClaimCommand {
    pub(crate) fn from_revision(
        context: CommandContext,
        proposal: PreparedCommand,
        cut: PgCompleteClaimCut,
        owner_contributions: BTreeMap<String, tos_foundation::JsonValue>,
    ) -> DurableResult<Self> {
        Self::new(
            context,
            proposal,
            cut,
            IndexRows::new(),
            owner_contributions,
        )
    }

    pub(crate) fn from_creation(
        serialized: &source_claims::SerializedClaimCreation,
        cut: PgCompleteClaimCut,
        owner_contributions: BTreeMap<String, tos_foundation::JsonValue>,
    ) -> DurableResult<Self> {
        let context = serialized.context().clone();
        let proposal = serialized.command().clone();
        if proposal.operation != "claims.create" {
            return Err(DurableError::Refused(
                "PG Claim creation package has another operation",
            ));
        }
        let config = cmd::parse(&context.configuration_raw).map_err(DurableError::Source)?;
        let source_path = cmd::text(&config, "source_path").map_err(DurableError::Source)?;
        let source_path = RelativePath::parse(source_path)
            .map_err(|_| DurableError::Invalid("PG Claim source path"))?;
        let semantic_indexes =
            source_claims::pg_claim_creation_index_rows(&source_path, serialized.files())
                .map_err(DurableError::Source)?
                .into_iter()
                .collect();
        let package_paths = serialized
            .files()
            .keys()
            .map(|name| format!("{}/{name}", serialized.home().as_str()))
            .collect::<BTreeSet<_>>();
        let change_paths = proposal
            .changes
            .iter()
            .map(|change| change.path.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        let exact_new_package = package_paths == change_paths
            && proposal.changes.len() == serialized.files().len()
            && proposal.changes.iter().all(|change| {
                change.before.is_none()
                    && change.after.as_ref()
                        == serialized.files().get(
                            change
                                .path
                                .as_str()
                                .rsplit_once('/')
                                .map(|(_, name)| name)
                                .unwrap_or(""),
                        )
            });
        let exact_replay = proposal.replayed && proposal.changes.is_empty();
        if !exact_new_package && !exact_replay {
            return Err(DurableError::Conflict(
                "PG Claim creation package is not the exact owner five-file closure",
            ));
        }
        Self::new(
            context,
            proposal,
            cut,
            semantic_indexes,
            owner_contributions,
        )
    }

    fn new(
        context: CommandContext,
        proposal: PreparedCommand,
        cut: PgCompleteClaimCut,
        semantic_indexes: IndexRows,
        owner_contributions: BTreeMap<String, tos_foundation::JsonValue>,
    ) -> DurableResult<Self> {
        cut.verify_context(context.base_revision, &context.files)?;
        let family_tag = source_claims::pg_family_tag(&context).map_err(DurableError::Source)?;
        if proposal.base_revision != context.base_revision
            || !["claims.create", "claim.revise"].contains(&proposal.operation.as_str())
            || semantic_indexes
                .iter()
                .any(|(kind, _, _)| !["claim", "event", "anchor", "form"].contains(&kind.as_str()))
            || owner_contributions.keys().any(|path| {
                !proposal
                    .changes
                    .iter()
                    .any(|change| change.path.as_str() == path)
            })
            || (proposal.operation == "claims.create") != !semantic_indexes.is_empty()
        {
            return Err(DurableError::Conflict(
                "PG Claim package base, operation or index closure differs",
            ));
        }
        Ok(Self {
            context,
            proposal,
            cut,
            family_tag,
            semantic_indexes,
            owner_contributions,
        })
    }
}

impl PreparedPgSourceRevision for PreparedPgClaimCommand {
    fn context(&self) -> &CommandContext {
        &self.context
    }

    fn proposal(&self) -> &PreparedCommand {
        &self.proposal
    }

    fn identity(&self) -> super::revision::PgRecordRevisionIdentity {
        self.cut.identity()
    }

    fn family_tag(&self) -> &str {
        &self.family_tag
    }

    fn inventory_owner_contributions(
        &self,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<BTreeMap<String, tos_foundation::JsonValue>> {
        if writes.iter().any(|write| {
            self.proposal
                .changes
                .iter()
                .find(|change| change.path.as_str() == write.subject)
                .is_none_or(|change| change.after.as_deref() != Some(write.exact_bytes.as_slice()))
        }) {
            return Err(DurableError::Conflict(
                "PG Claim owner projections differ from exact writes",
            ));
        }
        Ok(self.owner_contributions.clone())
    }

    fn legacy_family_encoding(&self) -> bool {
        false
    }

    fn validate_readset(&self, readset: &SourceRevisionReadSet) -> DurableResult<()> {
        self.cut.verify_readset(readset)?;
        let expected = source_claims::pg_family_tag(&self.context).map_err(DurableError::Source)?;
        if self.identity()
            != super::revision::PgRecordRevisionIdentity::from_selection(&readset.selection)
            || self.family_tag != expected
            || self.proposal.base_revision != readset.selection.cohort.initial_revision
            || self.context.base_revision != readset.selection.cohort.initial_revision
            || !["claims.create", "claim.revise"].contains(&self.proposal.operation.as_str())
        {
            return Err(DurableError::Conflict(
                "PG Claim semantic preparation epoch or family differs",
            ));
        }
        if self.proposal.operation == "claims.create" && !self.proposal.replayed {
            for (_, token, _) in &self.semantic_indexes {
                for kind in ["metadata", "claim", "event", "anchor", "form"] {
                    if !readset.reads.reads.iter().any(|read| {
                        matches!(read, crate::PredicateRead::Generation { predicate, .. }
                            if predicate.kind == crate::PredicateKind::Unique
                                && predicate.owner == super::OWNER
                                && predicate.scope == kind
                                && predicate.token == *token
                                && predicate.definition_version == readset.selection.cohort.definition.to_hex())
                    }) {
                        return Err(DurableError::Conflict(
                            "PG Claim unique identity generation was not observed",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn work_revision_evidence(
        &self,
        _writes: &[SourceRevisionWrite],
    ) -> DurableResult<Option<crate::source_pg_work_revision::CommitEvidence>> {
        Ok(None)
    }

    fn check_indexes(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<IndexRows> {
        check_claim_write_indexes(
            tx,
            domain,
            writes,
            &self.context,
            &self.proposal.operation,
            &self.semantic_indexes,
        )
    }
}

fn check_claim_write_indexes(
    tx: &mut Transaction<'_>,
    domain: &str,
    writes: &[SourceRevisionWrite],
    context: &CommandContext,
    operation: &str,
    semantic_indexes: &IndexRows,
) -> DurableResult<IndexRows> {
    let mut indexes = revision_indexes(writes);
    indexes.extend(semantic_indexes.iter().cloned());
    if !["claims.create", "claim.revise"].contains(&operation) {
        return Err(DurableError::Conflict(
            "PG Claim writer owner family differs",
        ));
    }
    let config = cmd::parse(&context.configuration_raw).map_err(DurableError::Source)?;
    let source_path = cmd::text(&config, "source_path").map_err(DurableError::Source)?;
    let source_path = RelativePath::parse(source_path)
        .map_err(|_| DurableError::Invalid("PG Claim source path"))?;
    let source_path = source_path.as_str();
    let parent = source_path
        .rsplit_once('/')
        .ok_or(DurableError::Invalid("PG Claim source parent"))?
        .0;
    let history_path = format!("{}/claim-revision-history.json", parent);
    let claim_config = cmd::parse(&context.configuration_raw).map_err(DurableError::Source)?;
    let form_path = claim_config
        .object_get("claim_id")
        .and_then(tos_foundation::JsonValue::as_str)
        .map(|claim_id| {
            format!(
                "{}/source-claims.{}.human-forms.json",
                parent,
                Digest256::of_bytes(claim_id.as_bytes())
                    .to_prefixed()
                    .trim_start_matches("sha256:")
            )
        });
    let archive_members = if operation == "claim.revise" {
        if form_path.is_none() {
            return Err(DurableError::Invalid("PG Claim revision identity"));
        }
        Some(claim_revision_archive_members(
            context,
            writes,
            source_path,
            &history_path,
        )?)
    } else {
        None
    };
    if operation == "claims.create" {
        let expected = BTreeSet::from([
            source_path.to_owned(),
            format!("{parent}/source-create-request.json"),
            format!("{parent}/source-create-environment.json"),
            format!("{parent}/source-create-provenance.jsonl"),
            format!("{parent}/source-create-receipt.json"),
        ]);
        let inserted = writes
            .iter()
            .filter(|write| write.expected_predecessor.is_none())
            .map(|write| write.subject.clone())
            .collect::<BTreeSet<_>>();
        if inserted != expected || writes.len() != expected.len() {
            return Err(DurableError::Refused(
                "PG Claim creation writes are not the exact owner five-file package",
            ));
        }
    } else if !writes.iter().any(|write| write.subject == source_path)
        || !writes.iter().any(|write| write.subject == history_path)
    {
        return Err(DurableError::Refused(
            "PG Claim revision omits its source or retained-history successor",
        ));
    }
    for write in writes {
        let paths = tx.query(
            "SELECT path FROM cmd2_source_index WHERE domain=$1 AND kind='path' AND token=$2",
            &[&domain, &write.subject],
        )?;
        if write.expected_predecessor.is_some() {
            if operation == "claim.revise"
                && write.subject != source_path
                && form_path.as_deref() != Some(write.subject.as_str())
                && write.subject != history_path
            {
                return Err(DurableError::Refused(
                    "PG Claim revision changes a path outside its exact owner package",
                ));
            }
            if paths.len() != 1 || paths[0].get::<_, String>("path") != write.subject {
                return Err(DurableError::Conflict(
                    "PG Claim predecessor path index is absent or ambiguous",
                ));
            }
        } else {
            let allowed = if operation == "claims.create" {
                true
            } else {
                write.subject == history_path
                    || form_path.as_deref() == Some(write.subject.as_str())
                    || archive_members
                        .as_ref()
                        .is_some_and(|members| members.contains(&write.subject))
            };
            if !allowed || !paths.is_empty() {
                return Err(DurableError::Refused(
                    "PG Claim insertion is outside its owner history or retained package",
                ));
            }
            if operation == "claim.revise"
                && archive_members
                    .as_ref()
                    .is_some_and(|members| members.contains(&write.subject))
                && write.subject.ends_with(".blob")
            {
                let digest = write
                    .subject
                    .rsplit_once('/')
                    .and_then(|(_, name)| name.strip_suffix(".blob"))
                    .ok_or(DurableError::Corrupt(
                        "PG Claim archive blob insertion path is malformed",
                    ))?;
                if Digest256::of_bytes(&write.exact_bytes).to_hex() != digest {
                    return Err(DurableError::Corrupt(
                        "PG Claim archive blob bytes differ from their content address",
                    ));
                }
            }
        }
        let rows = tx.query(
            "SELECT kind,token,path FROM cmd2_source_index
             WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\" AND kind<>'path'
             ORDER BY kind COLLATE \"C\",token COLLATE \"C\" LIMIT 513",
            &[&domain, &write.subject],
        )?;
        if rows.len() > 512 {
            return Err(DurableError::Refused(
                "PG Claim semantic index rows exceed the maintained package bound",
            ));
        }
        let stored_claims = rows
            .iter()
            .filter(|row| row.get::<_, String>("kind") == "claim")
            .map(|row| row.get::<_, String>("token"))
            .collect::<BTreeSet<_>>();
        let stored_forms = rows
            .iter()
            .filter(|row| row.get::<_, String>("kind") == "form")
            .map(|row| row.get::<_, String>("token"))
            .collect::<BTreeSet<_>>();
        if rows.iter().any(|row| {
            row.get::<_, String>("path") != write.subject
                || !["claim", "form", "event", "anchor"]
                    .contains(&row.get::<_, String>("kind").as_str())
        }) {
            return Err(DurableError::Refused(
                "PG Claim semantic transition includes another owner index",
            ));
        }
        if stored_claims.len() > 32
            || stored_forms.len() > 288
            || (!stored_claims.is_empty()
                && source_claims::pg_claim_stream_ids(&write.exact_bytes)
                    .map_err(DurableError::Source)?
                    != stored_claims)
            || (!stored_forms.is_empty() && revision_form_ids(&write.exact_bytes)? != stored_forms)
        {
            return Err(DurableError::Conflict(
                "PG Claim revision changes a stable claim or form identity",
            ));
        }
    }
    if operation == "claims.create" {
        for (kind, token, path) in semantic_indexes {
            if !["claim", "event", "anchor", "form"].contains(&kind.as_str())
                || !writes.iter().any(|write| write.subject == *path)
                || tx
                    .query_opt(
                        "SELECT 1 FROM cmd2_source_index
                         WHERE domain=$1 AND kind=ANY($2) AND token=$3 LIMIT 1",
                        &[
                            &domain,
                            &vec!["metadata", "claim", "event", "anchor", "form"],
                            &token,
                        ],
                    )?
                    .is_some()
            {
                return Err(DurableError::Conflict(
                    "PG Claim new semantic identity is occupied or unbound",
                ));
            }
        }
    } else if writes.iter().any(|write| {
        write.expected_predecessor.is_none()
            && write.subject != history_path
            && form_path.as_deref() != Some(write.subject.as_str())
            && !archive_members
                .as_ref()
                .is_some_and(|members| members.contains(&write.subject))
    }) {
        return Err(DurableError::Refused(
            "PG Claim revision inserts a path outside its exact owner archive",
        ));
    } else if !semantic_indexes.is_empty() {
        return Err(DurableError::Corrupt(
            "PG Claim revision carries new semantic identities",
        ));
    }
    Ok(indexes)
}

/// Return only the predecessor archive members named by the exact owner
/// history receipt and its manifest. This keeps a Claim revision from using
/// its insertion allowance to add unrelated source-witness files.
fn claim_revision_archive_members(
    context: &CommandContext,
    writes: &[SourceRevisionWrite],
    source_path: &str,
    history_path: &str,
) -> DurableResult<BTreeSet<String>> {
    let history_write = writes
        .iter()
        .find(|write| write.subject == history_path)
        .ok_or(DurableError::Refused(
            "PG Claim revision history successor is absent",
        ))?;
    let history = cmd::parse(&history_write.exact_bytes).map_err(DurableError::Source)?;
    let receipts = cmd::array(&history, "receipts").map_err(DurableError::Source)?;
    let receipt = receipts.last().ok_or(DurableError::Corrupt(
        "PG Claim revision history has no receipt",
    ))?;
    let previous_source = cmd::field(receipt, "previous_source").map_err(DurableError::Source)?;
    let previous_id = cmd::text(previous_source, "id").map_err(DurableError::Source)?;
    let previous_revision =
        cmd::text(receipt, "previous_revision").map_err(DurableError::Source)?;
    let previous_revision = previous_revision
        .strip_prefix("sha256:")
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or(DurableError::Corrupt(
            "PG Claim archive predecessor revision is malformed",
        ))?;
    let expected_root = format!(
        "ToS/source-witnesses/.record-revisions/{}-{previous_revision}",
        Digest256::of_bytes(previous_id.as_bytes())
            .to_prefixed()
            .trim_start_matches("sha256:")
    );
    let archive_root = cmd::text(receipt, "archive_path").map_err(DurableError::Source)?;
    if archive_root != expected_root {
        return Err(DurableError::Corrupt(
            "PG Claim archive receipt path differs from its predecessor",
        ));
    }
    let manifest_path = format!("{archive_root}/manifest.json");
    let manifest_raw = writes
        .iter()
        .find(|write| write.subject == manifest_path)
        .map(|write| write.exact_bytes.as_slice())
        .or_else(|| {
            context
                .files
                .iter()
                .find(|file| file.path.as_str() == manifest_path)
                .map(|file| file.raw.as_slice())
        })
        .ok_or(DurableError::Refused(
            "PG Claim predecessor archive manifest is not selected",
        ))?;
    let manifest = cmd::parse(manifest_raw).map_err(DurableError::Source)?;
    if cmd::text(&manifest, "schema_version").map_err(DurableError::Source)?
        != "tos_source_package_archive_v1"
        || cmd::text(&manifest, "source_path").map_err(DurableError::Source)? != source_path
        || cmd::text(&manifest, "revision").map_err(DurableError::Source)?
            != cmd::text(receipt, "previous_revision").map_err(DurableError::Source)?
        || cmd::field(&manifest, "source").map_err(DurableError::Source)? != previous_source
    {
        return Err(DurableError::Corrupt(
            "PG Claim predecessor archive manifest differs from its receipt",
        ));
    }
    let files = cmd::field(&manifest, "files")
        .map_err(DurableError::Source)?
        .as_object()
        .ok_or(DurableError::Corrupt(
            "PG Claim predecessor archive file map is malformed",
        ))?;
    let mut members = BTreeSet::from([manifest_path]);
    let mut blobs = BTreeSet::new();
    for (_, binding) in files {
        let blob = cmd::text(binding, "blob").map_err(DurableError::Source)?;
        let digest = blob
            .strip_suffix(".blob")
            .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(DurableError::Corrupt(
                "PG Claim predecessor archive blob name is malformed",
            ))?;
        if !blobs.insert(digest.to_owned()) {
            continue;
        }
        members.insert(format!("{archive_root}/{blob}"));
    }
    Ok(members)
}

/// Derive the complete durable index binding used by replay verification.
/// Creation contributes Claim/event/anchor/form identities; revision carries
/// only new retained-archive path rows.
pub(crate) fn expected_revision_indexes(
    family_tag: &str,
    writes: &[SourceRevisionWrite],
) -> DurableResult<IndexRows> {
    let mut indexes = revision_indexes(writes);
    if family_tag.ends_with(":claims.create") {
        let source_write = writes
            .iter()
            .find(|write| write.subject.ends_with("/source-claims.jsonl"))
            .ok_or(DurableError::Corrupt(
                "PG Claim source stream absent on replay",
            ))?;
        let source_path = RelativePath::parse(&source_write.subject)
            .map_err(|_| DurableError::Corrupt("PG Claim replay source path"))?;
        let parent = source_write
            .subject
            .rsplit_once('/')
            .ok_or(DurableError::Corrupt("PG Claim replay source parent"))?
            .0;
        let files = writes
            .iter()
            .filter_map(|write| {
                write
                    .subject
                    .strip_prefix(&format!("{parent}/"))
                    .filter(|name| !name.contains('/'))
                    .map(|name| (name.to_owned(), write.exact_bytes.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        indexes.extend(
            source_claims::pg_claim_creation_index_rows(&source_path, &files)
                .map_err(DurableError::Source)?,
        );
    }
    Ok(indexes)
}
