//! Bounded PostgreSQL/STO transport for one authored source revision.
//!
//! This is deliberately a read epoch, not a current-source generation.  It
//! reconstructs only the immutable cohort identity and the addressed members
//! needed by one owner command; it never performs the complete cold reopen or
//! creates a selected full-corpus snapshot.

use super::*;
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_revisions::ReadonlyRecordFiles;
use postgres::IsolationLevel;
use std::sync::atomic::AtomicBool;

const MAX_CHILDREN: usize = 64;
const MAX_RETAINED_STATE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CLAIM_DEPENDENCY_LOOKUPS: usize = 4_096;
const MAX_CLAIM_DEPENDENCY_ROWS: usize = 65_536;
const MAX_ITEM_FILE_LOOKUPS: usize = 4_096;
const MAX_ITEM_FILE_ROWS: usize = 65_536;
const MAX_SOURCE_IDENTITY_LOOKUPS: usize = 4_096;
const MAX_SOURCE_IDENTITY_TOKEN_BYTES: usize = 4_096;
const MAX_SIGN_CANDIDATE_TOKEN_BYTES: usize = 4_096;
const MAX_SOURCE_PUBLICATION_PACKAGE_BYTES: usize = 1_048_576;
const MAX_SOURCE_PUBLICATION_BLOB_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SOURCE_PUBLICATION_VIRTUAL_LOOKUPS: u64 = 4_096;
const MAX_SOURCE_PUBLICATION_PACKAGE_BYTES_READ: u64 = 64 * 1024 * 1024;
const MAX_SOURCE_PUBLICATION_IMPORT_BLOB_MEMBERS: usize = 4_096;
const SOURCE_PUBLICATION_CONTROL_PATH: &str = "ToS/source-witnesses/.metadata-publication.json";
const SOURCE_PUBLICATION_JOURNAL_ROOT: &str = "ToS/source-witnesses/.metadata-transactions";
const SOURCE_PUBLICATION_ROW_FIELDS: &str = "state,publication_generation,token,base_token,base_generation,transaction_id,package_digest,package_bytes,origin_kind,source_commit_seq,import_event_id,import_transition_index,import_receipt_digest,store_id,bootstrap_source_revision,bootstrap_membership_digest,bootstrap_membership_count";

/// The bounded initial Agent route treats these five identity namespaces as a
/// single uniqueness domain.  Keep the list with the PG transport so the
/// absence probes and their generation predicates cannot drift apart.
pub(crate) const PG_AGENT_INDEX_KINDS: [&str; 5] = ["metadata", "claim", "event", "anchor", "form"];

#[derive(Clone, Debug)]
pub(crate) struct SourceRevisionSelection {
    pub(crate) cohort: ManagedSourceCohort,
    pub(crate) contract: Digest256,
    pub(crate) epoch: u64,
    pub(crate) source_generation: u64,
    pub(crate) head_seq: u64,
    pub(crate) audit_generation: u64,
    pub(crate) rights_version: u64,
    pub(crate) rule_version: u64,
    pub(crate) job_id: String,
    pub(crate) job_fence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedSourcePublicationRow {
    state: String,
    publication_generation: u64,
    token: Option<String>,
    base_token: Option<String>,
    base_generation: u64,
    transaction_id: Option<String>,
    package_digest: Option<String>,
    package_bytes: Option<Vec<u8>>,
    origin_kind: String,
    source_commit_seq: Option<u64>,
    import_event_id: Option<String>,
    import_transition_index: Option<u64>,
    import_receipt_digest: Option<String>,
    store_id: [u8; 16],
    bootstrap_source_revision: String,
    bootstrap_membership_digest: String,
    bootstrap_membership_count: u64,
}

impl SelectedSourcePublicationRow {
    fn from_row(row: &postgres::Row) -> DurableResult<Self> {
        let store_id = row
            .get::<_, Vec<u8>>("store_id")
            .try_into()
            .map_err(|_| DurableError::Corrupt("source publication store identity"))?;
        Ok(Self {
            state: row.get("state"),
            publication_generation: as_u64(row.get("publication_generation"))?,
            token: row.get("token"),
            base_token: row.get("base_token"),
            base_generation: as_u64(row.get("base_generation"))?,
            transaction_id: row.get("transaction_id"),
            package_digest: row.get("package_digest"),
            package_bytes: row.get("package_bytes"),
            origin_kind: row.get("origin_kind"),
            source_commit_seq: row
                .get::<_, Option<i64>>("source_commit_seq")
                .map(as_u64)
                .transpose()?,
            import_event_id: row.get("import_event_id"),
            import_transition_index: row
                .get::<_, Option<i64>>("import_transition_index")
                .map(as_u64)
                .transpose()?,
            import_receipt_digest: row.get("import_receipt_digest"),
            store_id,
            bootstrap_source_revision: row.get("bootstrap_source_revision"),
            bootstrap_membership_digest: row.get("bootstrap_membership_digest"),
            bootstrap_membership_count: as_u64(row.get("bootstrap_membership_count"))?,
        })
    }

    fn verify_selection(&self, selection: &SourceRevisionSelection) -> DurableResult<()> {
        if self.store_id != selection.cohort.store_id
            || self.bootstrap_source_revision != selection.cohort.initial_revision.0.to_hex()
            || self.bootstrap_membership_digest
                != selection.cohort.initial_membership.digest.to_hex()
            || self.bootstrap_membership_count != selection.cohort.initial_membership.count
            || self
                .source_commit_seq
                .is_some_and(|source_commit_seq| source_commit_seq > selection.head_seq)
        {
            return Err(DurableError::Conflict(
                "source publication bootstrap or commit lineage differs",
            ));
        }
        match self.state.as_str() {
            "absent" => {
                if self.publication_generation != 0
                    || self.origin_kind != "bootstrap_absent"
                    || self.token.is_some()
                    || self.base_token.is_some()
                    || self.base_generation != 0
                    || self.transaction_id.is_some()
                    || self.package_digest.is_some()
                    || self.package_bytes.is_some()
                    || self.source_commit_seq != Some(0)
                    || self.import_event_id.is_some()
                    || self.import_transition_index.is_some()
                    || self.import_receipt_digest.is_some()
                {
                    return Err(DurableError::Corrupt(
                        "source publication absent row is not generation zero",
                    ));
                }
            }
            "present" => {
                let token = self
                    .token
                    .as_deref()
                    .ok_or(DurableError::Corrupt("source publication token absent"))?;
                let transaction_id = self.transaction_id.as_deref().ok_or(
                    DurableError::Corrupt("source publication transaction absent"),
                )?;
                let package_digest = self.package_digest.as_deref().ok_or(
                    DurableError::Corrupt("source publication package digest absent"),
                )?;
                let package_bytes = self
                    .package_bytes
                    .as_deref()
                    .ok_or(DurableError::Corrupt("source publication package absent"))?;
                if self.publication_generation == 0
                    || self.publication_generation
                        != self
                            .base_generation
                            .checked_add(2)
                            .ok_or(DurableError::Corrupt(
                                "source publication generation overflow",
                            ))?
                    || !crate::source_work_transaction::is_hash(token)
                    || !crate::source_work_transaction::is_hash(transaction_id)
                    || self.base_generation == 0 && self.base_token.as_deref().is_some()
                    || self.base_generation > 0
                        && !self
                            .base_token
                            .as_deref()
                            .is_some_and(crate::source_work_transaction::is_hash)
                    || package_bytes.is_empty()
                    || package_bytes.len() > MAX_SOURCE_PUBLICATION_PACKAGE_BYTES
                    || Digest256::from_hex(package_digest).is_err()
                    || Digest256::of_bytes(package_bytes).to_hex() != package_digest
                {
                    return Err(DurableError::Corrupt(
                        "source publication row fields or package digest differ",
                    ));
                }
                match self.origin_kind.as_str() {
                    "pg_commit"
                        if self.source_commit_seq.is_some_and(|seq| seq > 0)
                            && self.import_event_id.is_none()
                            && self.import_transition_index.is_none()
                            && self.import_receipt_digest.is_none() => {}
                    "legacy_sto_import"
                        if self.source_commit_seq.is_none()
                            && self
                                .import_event_id
                                .as_deref()
                                .is_some_and(crate::source_work_transaction::is_hash)
                            && self.import_transition_index.is_some_and(|index| index > 0)
                            && self
                                .import_receipt_digest
                                .as_deref()
                                .is_some_and(|digest| Digest256::from_hex(digest).is_ok()) => {}
                    _ => {
                        return Err(DurableError::Corrupt(
                            "source publication origin lineage differs",
                        ));
                    }
                }
            }
            _ => {
                return Err(DurableError::Corrupt("source publication state invalid"));
            }
        }
        Ok(())
    }
}

fn selected_publication_transaction_id(path: &str) -> DurableResult<String> {
    let member = path
        .strip_prefix(&format!("{SOURCE_PUBLICATION_JOURNAL_ROOT}/"))
        .ok_or(DurableError::Invalid("source publication journal path"))?;
    let (id, name) = member
        .split_once('/')
        .ok_or(DurableError::Invalid("source publication journal path"))?;
    if !matches!(name, "manifest.json" | "completion.json")
        || id.len() != 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DurableError::Invalid("source publication journal identity"));
    }
    Ok(format!("sha256:{id}"))
}

fn selected_publication_package_member(
    row: &SelectedSourcePublicationRow,
    requested_path: &str,
) -> DurableResult<Vec<u8>> {
    let package = row
        .package_bytes
        .as_deref()
        .ok_or(DurableError::Corrupt("source publication package absent"))?;
    let transaction_id = row.transaction_id.as_deref().ok_or(DurableError::Corrupt(
        "source publication transaction absent",
    ))?;
    let id = transaction_id
        .strip_prefix("sha256:")
        .ok_or(DurableError::Corrupt(
            "source publication transaction identity",
        ))?;
    let journal = format!("{SOURCE_PUBLICATION_JOURNAL_ROOT}/{id}");
    let control_raw = crate::source_work_transaction::selected_publication_virtual_member(
        package,
        SOURCE_PUBLICATION_CONTROL_PATH,
    )
    .map_err(DurableError::Source)?
    .ok_or(DurableError::Corrupt("source publication control absent"))?;
    let manifest_path = format!("{journal}/manifest.json");
    let manifest_raw = crate::source_work_transaction::selected_publication_virtual_member(
        package,
        &manifest_path,
    )
    .map_err(DurableError::Source)?
    .ok_or(DurableError::Corrupt("source publication manifest absent"))?;
    let completion_path = format!("{journal}/completion.json");
    let completion_raw = crate::source_work_transaction::selected_publication_virtual_member(
        package,
        &completion_path,
    )
    .map_err(DurableError::Source)?
    .ok_or(DurableError::Corrupt(
        "source publication completion absent",
    ))?;

    let control = cmd::parse(&control_raw).map_err(DurableError::Source)?;
    crate::source_work_transaction::state(&control).map_err(DurableError::Source)?;
    let manifest = cmd::parse(&manifest_raw).map_err(DurableError::Source)?;
    let completion = cmd::parse(&completion_raw).map_err(DurableError::Source)?;
    let base = cmd::field(&manifest, "base_publication").map_err(DurableError::Source)?;
    cmd::exact_keys(base, &["token", "generation"]).map_err(DurableError::Source)?;
    let base_token = match cmd::field(base, "token").map_err(DurableError::Source)? {
        tos_foundation::JsonValue::Null => None,
        tos_foundation::JsonValue::String(value) => Some(
            value
                .as_str()
                .ok_or(DurableError::Corrupt(
                    "source publication manifest base token",
                ))?
                .to_owned(),
        ),
        _ => {
            return Err(DurableError::Corrupt(
                "source publication manifest base token",
            ));
        }
    };
    if cmd::text(&control, "phase").map_err(DurableError::Source)? != "ready"
        || cmd::text(&control, "outcome").map_err(DurableError::Source)? != "committed"
        || cmd::text(&control, "transaction_id").map_err(DurableError::Source)? != transaction_id
        || cmd::integer(&control, "generation").map_err(DurableError::Source)?
            != row.publication_generation
        || row.token.as_deref() != Some(cmd::text(&control, "token").map_err(DurableError::Source)?)
        || cmd::text(&manifest, "transaction_id").map_err(DurableError::Source)? != transaction_id
        || cmd::integer(base, "generation").map_err(DurableError::Source)? != row.base_generation
        || base_token != row.base_token
        || cmd::text(&control, "manifest_sha256").map_err(DurableError::Source)?
            != Digest256::of_bytes(&manifest_raw).to_prefixed()
        || cmd::exact_keys(&completion, &["schema_version", "publication"])
            .map_err(DurableError::Source)
            .is_err()
        || cmd::text(&completion, "schema_version").map_err(DurableError::Source)?
            != "tos_selected_metadata_completion_v1"
        || !cmd::same(
            cmd::field(&completion, "publication").map_err(DurableError::Source)?,
            &control,
        )
        .map_err(DurableError::Source)?
    {
        return Err(DurableError::Conflict(
            "source publication package lineage differs from indexed row",
        ));
    }

    crate::source_work_transaction::selected_publication_virtual_member(package, requested_path)
        .map_err(DurableError::Source)?
        .ok_or(DurableError::Conflict(
            "source publication package member is absent",
        ))
}

fn selected_publication_import_receipt(
    tx: &mut postgres::Transaction<'_>,
    selection: &SourceRevisionSelection,
    row: &SelectedSourcePublicationRow,
) -> DurableResult<(
    crate::source_work_transaction::LegacyPublicationImportBindingV1,
    Vec<u8>,
)> {
    if row.origin_kind != "legacy_sto_import" {
        return Err(DurableError::Corrupt(
            "source publication import receipt requested for non-import row",
        ));
    }
    let event_id = row.import_event_id.as_deref().ok_or(DurableError::Corrupt(
        "source publication import event absent",
    ))?;
    let row_receipt_digest = row
        .import_receipt_digest
        .as_deref()
        .ok_or(DurableError::Corrupt(
            "source publication import receipt digest absent",
        ))?;
    let receipt = tx
        .query_opt(
            "SELECT origin_kind,import_event_id,receipt_digest,receipt_bytes,store_id,
                    bootstrap_source_revision,bootstrap_membership_digest,
                    bootstrap_membership_count,import_source_head,chain_root_digest,
                    transition_count,head_control_digest,terminal_generation,terminal_token
             FROM cmd2_source_publication_import
             WHERE domain=$1 AND import_event_id=$2 FOR SHARE",
            &[&selection.cohort.domain, &event_id],
        )?
        .ok_or(DurableError::Conflict(
            "source publication import receipt is absent",
        ))?;
    let receipt_bytes: Vec<u8> = receipt.get("receipt_bytes");
    let (binding, receipt_digest) =
        crate::source_work_transaction::parse_legacy_selected_publication_import_receipt(
            &receipt_bytes,
        )
        .map_err(DurableError::Source)?;
    let store_id: [u8; 16] = receipt
        .get::<_, Vec<u8>>("store_id")
        .try_into()
        .map_err(|_| DurableError::Corrupt("source publication import store identity"))?;
    let import_index = row.import_transition_index.ok_or(DurableError::Corrupt(
        "source publication import transition index absent",
    ))?;
    if receipt.get::<_, String>("origin_kind") != "legacy_sto_import"
        || receipt.get::<_, String>("import_event_id") != event_id
        || receipt.get::<_, String>("receipt_digest") != receipt_digest.to_hex()
        || row_receipt_digest != receipt_digest.to_hex()
        || binding.receipt_sha256 != receipt_digest
        || binding.origin_kind != "legacy_sto_import"
        || binding.import_event_id != event_id
        || binding.domain != selection.cohort.domain
        || binding.store_id != selection.cohort.store_id
        || store_id != binding.store_id
        || receipt.get::<_, String>("bootstrap_source_revision")
            != binding.bootstrap_source_revision.0.to_hex()
        || receipt.get::<_, String>("bootstrap_membership_digest")
            != binding.bootstrap_membership.digest.to_hex()
        || as_u64(receipt.get("bootstrap_membership_count"))? != binding.bootstrap_membership.count
        || binding.bootstrap_source_revision != selection.cohort.initial_revision
        || binding.bootstrap_membership != selection.cohort.initial_membership
        || binding.through_seq != as_u64(receipt.get("import_source_head"))?
        || binding.chain_root_sha256.to_hex() != receipt.get::<_, String>("chain_root_digest")
        || binding.transition_count != as_u64(receipt.get("transition_count"))?
        || binding.head_control_sha256.to_hex() != receipt.get::<_, String>("head_control_digest")
        || binding.head_generation != as_u64(receipt.get("terminal_generation"))?
        || binding.head_token != receipt.get::<_, String>("terminal_token")
        || binding.through_seq > selection.head_seq
        || binding.head_generation < row.publication_generation
        || import_index > binding.transition_count
    {
        return Err(DurableError::Conflict(
            "source publication import receipt binding differs",
        ));
    }
    Ok((binding, receipt_bytes))
}

fn verify_selected_publication_parent(
    tx: &mut postgres::Transaction<'_>,
    selection: &SourceRevisionSelection,
    child: &SelectedSourcePublicationRow,
) -> DurableResult<()> {
    if child.state == "absent" {
        return Ok(());
    }
    let row = tx.query_opt(
        &format!(
            "SELECT {SOURCE_PUBLICATION_ROW_FIELDS} FROM cmd2_source_publication_history
             WHERE domain=$1 AND publication_generation=$2 FOR SHARE"
        ),
        &[&selection.cohort.domain, &as_i64(child.base_generation)?],
    )?;
    let parent = row
        .as_ref()
        .map(SelectedSourcePublicationRow::from_row)
        .transpose()?
        .ok_or(DurableError::Conflict(
            "source publication predecessor history is absent",
        ))?;
    parent.verify_selection(selection)?;
    if parent.publication_generation != child.base_generation
        || parent.token != child.base_token
        || (child.base_generation == 0 && parent.state != "absent")
        || (child.base_generation > 0 && parent.state != "present")
    {
        return Err(DurableError::Conflict(
            "source publication predecessor history differs",
        ));
    }
    match child.origin_kind.as_str() {
        "legacy_sto_import" => {
            let child_index = child.import_transition_index.ok_or(DurableError::Corrupt(
                "source publication import transition index absent",
            ))?;
            let follows_import = parent.origin_kind == "legacy_sto_import"
                && parent.import_event_id == child.import_event_id
                && parent
                    .import_transition_index
                    .and_then(|index| index.checked_add(1))
                    == Some(child_index);
            let starts_import = parent.state == "absent" && child_index == 1;
            if !follows_import && !starts_import {
                return Err(DurableError::Conflict(
                    "source publication import transition order differs",
                ));
            }
            if follows_import {
                selected_publication_import_receipt(tx, selection, &parent)?;
            }
        }
        "pg_commit" => {
            let child_seq = child.source_commit_seq.ok_or(DurableError::Corrupt(
                "source publication commit sequence absent",
            ))?;
            let parent_seq = match parent.source_commit_seq {
                Some(parent_seq) => parent_seq,
                None if parent.origin_kind == "legacy_sto_import" => {
                    let (binding, _) = selected_publication_import_receipt(tx, selection, &parent)?;
                    if parent.publication_generation != binding.head_generation
                        || parent.token.as_deref() != Some(binding.head_token.as_str())
                        || parent.import_transition_index != Some(binding.transition_count)
                    {
                        return Err(DurableError::Conflict(
                            "source publication import predecessor is not the imported head",
                        ));
                    }
                    binding.through_seq
                }
                None => {
                    return Err(DurableError::Corrupt(
                        "source publication predecessor commit sequence absent",
                    ));
                }
            };
            if child_seq <= parent_seq {
                return Err(DurableError::Conflict(
                    "source publication commit sequence order differs",
                ));
            }
        }
        _ => {
            return Err(DurableError::Corrupt(
                "source publication predecessor origin invalid",
            ));
        }
    }
    Ok(())
}

/// Opaque identity of the bounded PostgreSQL read epoch.  This is deliberately
/// separate from `SourceRevision`: the latter names the real bootstrap source
/// revision carried by the semantic `CommandContext`, while this value binds
/// the selected PG domain/fences used for admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PgRecordRevisionIdentity {
    pub(crate) domain_digest: Digest256,
    pub(crate) store_id: [u8; 16],
    pub(crate) bootstrap_revision: SourceRevision,
    pub(crate) bootstrap_membership: SourceMembershipV1,
    pub(crate) cohort_digest: Digest256,
    pub(crate) cohort_epoch: u64,
    pub(crate) cohort_definition: Digest256,
    pub(crate) cohort_generation: u64,
    pub(crate) job_id_digest: Digest256,
    pub(crate) job_fence: u64,
    pub(crate) epoch: u64,
    pub(crate) source_generation: u64,
    pub(crate) head_seq: u64,
    pub(crate) audit_generation: u64,
    pub(crate) rights_version: u64,
    pub(crate) rule_version: u64,
    pub(crate) contract: Digest256,
}

impl PgRecordRevisionIdentity {
    pub(crate) fn from_selection(selection: &SourceRevisionSelection) -> Self {
        let mut cohort_hasher = Digest256Hasher::new();
        part(
            &mut cohort_hasher,
            b"tos-pg-record-revision-cohort-identity-v1",
        );
        part(&mut cohort_hasher, selection.cohort.domain.as_bytes());
        part(&mut cohort_hasher, &selection.cohort.store_id);
        part(
            &mut cohort_hasher,
            selection.cohort.initial_revision.0.as_bytes(),
        );
        part(
            &mut cohort_hasher,
            &selection.cohort.initial_membership.count.to_be_bytes(),
        );
        part(
            &mut cohort_hasher,
            selection.cohort.initial_membership.digest.as_bytes(),
        );
        part(&mut cohort_hasher, &selection.cohort.epoch.to_be_bytes());
        part(&mut cohort_hasher, selection.cohort.definition.as_bytes());
        part(
            &mut cohort_hasher,
            &selection.cohort.generation.to_be_bytes(),
        );
        Self {
            domain_digest: Digest256::of_bytes(selection.cohort.domain.as_bytes()),
            store_id: selection.cohort.store_id,
            bootstrap_revision: selection.cohort.initial_revision,
            bootstrap_membership: selection.cohort.initial_membership,
            cohort_digest: cohort_hasher.finalize(),
            cohort_epoch: selection.cohort.epoch,
            cohort_definition: selection.cohort.definition,
            cohort_generation: selection.cohort.generation,
            job_id_digest: Digest256::of_bytes(selection.job_id.as_bytes()),
            job_fence: selection.job_fence,
            epoch: selection.epoch,
            source_generation: selection.source_generation,
            head_seq: selection.head_seq,
            audit_generation: selection.audit_generation,
            rights_version: selection.rights_version,
            rule_version: selection.rule_version,
            contract: selection.contract,
        }
    }

    /// Digest the complete opaque source-selection identity for owner
    /// mechanics that need a stable input binding but must not manufacture a
    /// `SourceRevision` for the current PG head.
    pub(crate) fn digest(&self) -> Digest256 {
        let mut hasher = Digest256Hasher::new();
        hasher.update(b"tos-pg-record-revision-identity-v1\0");
        hasher.update(self.domain_digest.as_bytes());
        hasher.update(&self.store_id);
        hasher.update(self.bootstrap_revision.0.as_bytes());
        hasher.update(&self.bootstrap_membership.count.to_be_bytes());
        hasher.update(self.bootstrap_membership.digest.as_bytes());
        hasher.update(self.cohort_digest.as_bytes());
        hasher.update(&self.cohort_epoch.to_be_bytes());
        hasher.update(self.cohort_definition.as_bytes());
        hasher.update(&self.cohort_generation.to_be_bytes());
        hasher.update(self.job_id_digest.as_bytes());
        hasher.update(&self.job_fence.to_be_bytes());
        hasher.update(&self.epoch.to_be_bytes());
        hasher.update(&self.source_generation.to_be_bytes());
        hasher.update(&self.head_seq.to_be_bytes());
        hasher.update(&self.audit_generation.to_be_bytes());
        hasher.update(&self.rights_version.to_be_bytes());
        hasher.update(&self.rule_version.to_be_bytes());
        hasher.update(self.contract.as_bytes());
        hasher.finalize()
    }
}

impl SourceRevisionSelection {
    pub(crate) fn identity(&self) -> PgRecordRevisionIdentity {
        PgRecordRevisionIdentity::from_selection(self)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SourceRevisionObservation {
    pub(crate) member: ManagedCurrentMember,
    pub(crate) locator: Digest256,
}

pub(crate) struct SourceRevisionReadSet {
    pub(crate) selection: SourceRevisionSelection,
    pub(crate) reads: SourceReads,
    pub(crate) observations: BTreeMap<String, SourceRevisionObservation>,
    pub(crate) work: SourceRevisionReadWork,
    claim_navigation_closure: Option<SourceRevisionClaimNavigationClosure>,
}

/// Exact point-identity closure for the caller's selected tokens. The result
/// is tied to the PG epoch and can be checked against the eventual readset;
/// it carries no claim that unrelated identities were enumerated.
#[derive(Clone, Debug)]
pub(crate) struct SourceRevisionIdentityClosure {
    selection_identity: PgRecordRevisionIdentity,
    owners: BTreeMap<String, Option<(String, RelativePath)>>,
    generations: BTreeMap<(String, String), u64>,
}

impl SourceRevisionIdentityClosure {
    pub(crate) fn selection_identity(&self) -> PgRecordRevisionIdentity {
        self.selection_identity
    }

    pub(crate) fn owners(&self) -> &BTreeMap<String, Option<(String, RelativePath)>> {
        &self.owners
    }

    /// Canonical commitment to the selected point lookups and every global
    /// uniqueness generation observed for them.
    pub(crate) fn closure_root(&self) -> Digest256 {
        let mut hasher = Digest256Hasher::new();
        part(&mut hasher, b"tos-pg-source-identity-closure-v1");
        part(&mut hasher, self.selection_identity.digest().as_bytes());
        part(&mut hasher, &(self.owners.len() as u64).to_be_bytes());
        for (token, owner) in &self.owners {
            part(&mut hasher, token.as_bytes());
            if let Some((kind, path)) = owner {
                part(&mut hasher, b"present");
                part(&mut hasher, kind.as_bytes());
                part(&mut hasher, path.as_str().as_bytes());
            } else {
                part(&mut hasher, b"absent");
            }
        }
        part(&mut hasher, &(self.generations.len() as u64).to_be_bytes());
        for ((kind, token), generation) in &self.generations {
            part(&mut hasher, kind.as_bytes());
            part(&mut hasher, token.as_bytes());
            part(&mut hasher, &generation.to_be_bytes());
        }
        hasher.finalize()
    }

    /// Require the closure's exact selection identity, predicate generations,
    /// and present owner members to be part of the eventual command readset.
    pub(crate) fn verify_readset(&self, readset: &SourceRevisionReadSet) -> DurableResult<()> {
        if readset.selection.identity() != self.selection_identity
            || self.generations.len() != self.owners.len() * PG_AGENT_INDEX_KINDS.len()
        {
            return Err(DurableError::Conflict(
                "source identity closure selection or generation count differs",
            ));
        }
        for (token, owner) in &self.owners {
            for kind in PG_AGENT_INDEX_KINDS {
                let Some(generation) = self.generations.get(&(kind.to_owned(), token.clone()))
                else {
                    return Err(DurableError::Conflict(
                        "source identity closure generation is absent",
                    ));
                };
                if !readset.reads.reads.iter().any(|read| {
                    matches!(read, PredicateRead::Generation { predicate, observed_generation }
                        if predicate.kind == PredicateKind::Unique
                            && predicate.owner == OWNER
                            && predicate.scope == kind
                            && predicate.token == *token
                            && predicate.definition_version == readset.selection.cohort.definition.to_hex()
                            && observed_generation == generation)
                }) {
                    return Err(DurableError::Conflict(
                        "source identity closure predicate is absent from readset",
                    ));
                }
            }
            if let Some((_, path)) = owner {
                let Some(observation) = readset.observations.get(path.as_str()) else {
                    return Err(DurableError::Conflict(
                        "source identity closure owner member was not read",
                    ));
                };
                if observation.member.path != *path {
                    return Err(DurableError::Conflict(
                        "source identity closure owner member path differs",
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Sealed, bounded reverse-reference evidence produced from one fresh PG
/// read epoch. The constructor remains in `with_source_revision_session`,
/// after its final source/audit/rights/lease fence check.
#[derive(Clone, Debug)]
pub(crate) struct SourceRevisionClaimNavigationClosure {
    domain: String,
    selection_identity: PgRecordRevisionIdentity,
    as_of_sequence: u64,
    baseline_sequence: u64,
    held_source_head_sequence: u64,
    lookups: BTreeMap<
        (
            crate::source_claim_dependency_index::ClaimNavigationReferenceKind,
            String,
        ),
        crate::source_claim_dependency_index::ClaimNavigationReferenceObservation,
    >,
    member_facts: BTreeMap<String, SourceRevisionClaimMemberEvidence>,
    members: BTreeMap<String, SourceRevisionObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRevisionClaimMemberEvidence {
    pub(crate) path: String,
    pub(crate) digest: Digest256,
    pub(crate) size_bytes: u64,
    pub(crate) commit_seq: u64,
}

impl SourceRevisionClaimNavigationClosure {
    pub(crate) fn domain(&self) -> &str {
        &self.domain
    }

    pub(crate) fn selection_identity(&self) -> PgRecordRevisionIdentity {
        self.selection_identity
    }

    pub(crate) fn as_of_sequence(&self) -> u64 {
        self.as_of_sequence
    }

    pub(crate) fn baseline_sequence(&self) -> u64 {
        self.baseline_sequence
    }

    pub(crate) fn held_source_head_sequence(&self) -> u64 {
        self.held_source_head_sequence
    }

    /// Canonical commitment to this bounded reverse-reference closure. The
    /// readset remains the authority; this root is only a transport binding
    /// for compilers that need to seal affected-owner manifests.
    pub(crate) fn closure_root(&self) -> Digest256 {
        let mut hasher = Digest256Hasher::new();
        part(&mut hasher, b"tos-pg-claim-navigation-closure-v1");
        part(&mut hasher, self.domain.as_bytes());
        part(&mut hasher, self.selection_identity.digest().as_bytes());
        part(&mut hasher, &self.as_of_sequence.to_be_bytes());
        part(&mut hasher, &self.baseline_sequence.to_be_bytes());
        part(&mut hasher, &self.held_source_head_sequence.to_be_bytes());
        part(&mut hasher, &(self.lookups.len() as u64).to_be_bytes());
        for ((kind, endpoint), observation) in &self.lookups {
            part(&mut hasher, kind.as_str().as_bytes());
            part(&mut hasher, endpoint.as_bytes());
            part(&mut hasher, observation.predicate_token.as_bytes());
            part(&mut hasher, &observation.generation.to_be_bytes());
            part(&mut hasher, observation.completeness_token.as_bytes());
            part(
                &mut hasher,
                &observation.completeness_generation.to_be_bytes(),
            );
            part(&mut hasher, &(observation.rows.len() as u64).to_be_bytes());
            for row in &observation.rows {
                part(&mut hasher, row.claim_id.as_bytes());
                part(&mut hasher, row.path.as_bytes());
                part(&mut hasher, row.kind.as_str().as_bytes());
                part(&mut hasher, row.endpoint.as_bytes());
                part(&mut hasher, row.member_digest.as_bytes());
                part(&mut hasher, &row.size_bytes.to_be_bytes());
                part(&mut hasher, &row.commit_seq.to_be_bytes());
                part(&mut hasher, &row.valid_from_seq.to_be_bytes());
                part(
                    &mut hasher,
                    &row.valid_to_seq.unwrap_or_default().to_be_bytes(),
                );
            }
        }
        part(&mut hasher, &(self.member_facts.len() as u64).to_be_bytes());
        for (path, fact) in &self.member_facts {
            part(&mut hasher, path.as_bytes());
            part(&mut hasher, fact.digest.as_bytes());
            part(&mut hasher, &fact.size_bytes.to_be_bytes());
            part(&mut hasher, &fact.commit_seq.to_be_bytes());
        }
        part(&mut hasher, &(self.members.len() as u64).to_be_bytes());
        for (path, observation) in &self.members {
            let member = &observation.member;
            part(&mut hasher, path.as_bytes());
            part(&mut hasher, Digest256::of_bytes(&member.raw).as_bytes());
            part(&mut hasher, member.metadata.sha256.as_bytes());
            part(&mut hasher, &(member.raw.len() as u64).to_be_bytes());
            part(&mut hasher, &member.commit_seq.to_be_bytes());
        }
        hasher.finalize()
    }

    pub(crate) fn lookups(
        &self,
    ) -> &BTreeMap<
        (
            crate::source_claim_dependency_index::ClaimNavigationReferenceKind,
            String,
        ),
        crate::source_claim_dependency_index::ClaimNavigationReferenceObservation,
    > {
        &self.lookups
    }

    pub(crate) fn member_facts(&self) -> &BTreeMap<String, SourceRevisionClaimMemberEvidence> {
        &self.member_facts
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PgSourceHomeObservation {
    pub(crate) parent: RelativePath,
    pub(crate) children: Vec<(String, bool)>,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct PgSourceUniqueIndexObservation {
    pub(crate) kind: String,
    pub(crate) token: String,
    pub(crate) rows: Vec<(String, String)>,
    pub(crate) generation: u64,
}

/// Bounded source transport accounting. These counters describe work actually
/// performed by the selected PG/STO route; they are not a claim that a
/// complete source inventory was observed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SourceRevisionReadWork {
    pub(crate) exact_source_members_read: u64,
    pub(crate) exact_source_bytes_read: u64,
    /// Exact immutable source versions reopened for Work-revision replay.
    pub(crate) historical_source_members_read: u64,
    pub(crate) historical_source_bytes_read: u64,
    pub(crate) custody_metadata_bytes_read: u64,
    pub(crate) segment_bytes_read: u64,
    pub(crate) committed_member_rows_examined: u64,
    pub(crate) metadata_member_lookups: u64,
    pub(crate) directory_index_rows_examined: u64,
    pub(crate) unique_index_lookups: u64,
    pub(crate) text_identity_lookups: u64,
    pub(crate) claim_dependency_lookups: u64,
    pub(crate) claim_navigation_lookups: u64,
    pub(crate) item_file_lookups: u64,
    pub(crate) item_rights_lookups: u64,
    pub(crate) item_edition_lookups: u64,
    pub(crate) sign_candidate_lookups: u64,
    pub(crate) source_publication_virtual_lookups: u64,
    pub(crate) source_publication_package_bytes_read: u64,
}

pub(crate) struct SourceRevisionReadSession<'a> {
    coordinator: &'a mut DurablePgCoordinator,
    store: &'a SegmentStore,
    selection: SourceRevisionSelection,
    reads: SourceReads,
    observations: BTreeMap<String, SourceRevisionObservation>,
    retained_state_bytes: usize,
    predicate_reads: BTreeSet<(String, String, String)>,
    work: SourceRevisionReadWork,
    work_revision: Option<crate::source_pg_work_revision::Observation>,
    claim_navigation_lookups: BTreeMap<
        (
            crate::source_claim_dependency_index::ClaimNavigationReferenceKind,
            String,
        ),
        crate::source_claim_dependency_index::ClaimNavigationReferenceObservation,
    >,
    claim_navigation_members: BTreeMap<String, SourceRevisionObservation>,
    claim_navigation_as_of_sequence: Option<u64>,
    claim_navigation_baseline_sequence: Option<u64>,
}

impl SourceRevisionReadSet {
    pub(crate) fn selection(&self) -> &SourceRevisionSelection {
        &self.selection
    }

    pub(crate) fn reads(&self) -> &SourceReads {
        &self.reads
    }

    pub(crate) fn observations(&self) -> &BTreeMap<String, SourceRevisionObservation> {
        &self.observations
    }

    pub(crate) fn claim_navigation_closure(&self) -> Option<&SourceRevisionClaimNavigationClosure> {
        self.claim_navigation_closure.as_ref()
    }
}

impl DurablePgCoordinator {
    /// Execute one bounded source read epoch. The closure may read addressed
    /// members and immediate children only. The final fence check happens
    /// before the readset is returned, so callers cannot accidentally use a
    /// stale PG selection as an admitted source revision.
    pub(crate) fn with_source_revision_session<T>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        operation: impl FnOnce(&mut SourceRevisionReadSession<'_>) -> DurableResult<T>,
    ) -> DurableResult<(T, SourceRevisionReadSet)> {
        active(deadline, cancelled)?;
        let selection = self.open_source_revision_selection(
            store, domain, job_id, job_fence, deadline, cancelled,
        )?;
        let mut session = SourceRevisionReadSession {
            coordinator: self,
            store,
            selection: selection.clone(),
            reads: SourceReads {
                reads: vec![PredicateRead::Generation {
                    predicate: PredicateToken {
                        kind: PredicateKind::Range,
                        owner: OWNER.to_owned(),
                        scope: "source-cohort".to_owned(),
                        token: "generation".to_owned(),
                        definition_version: selection.cohort.definition.to_hex(),
                    },
                    observed_generation: selection.source_generation,
                }],
                locators: BTreeMap::new(),
                managed_original: None,
                revision: None,
            },
            observations: BTreeMap::new(),
            retained_state_bytes: 0,
            predicate_reads: BTreeSet::new(),
            work: SourceRevisionReadWork::default(),
            work_revision: None,
            claim_navigation_lookups: BTreeMap::new(),
            claim_navigation_members: BTreeMap::new(),
            claim_navigation_as_of_sequence: None,
            claim_navigation_baseline_sequence: None,
        };
        // The actual owner must attempt its exit fence even when the borrowed
        // operation refuses. Preserve the first error and every spent prefix;
        // only a successful body AND fence may expose the finalized readset.
        let result = operation(&mut session);
        let result = finish_source_revision_epoch(result, || {
            session.verify_fresh(deadline, cancelled)
        })?;
        let readset = SourceRevisionReadSet {
            selection: session.selection.clone(),
            reads: session.reads,
            observations: session.observations,
            work: session.work,
            claim_navigation_closure: if session.claim_navigation_as_of_sequence.is_none() {
                None
            } else {
                let selection_identity = session.selection.identity();
                let member_facts = session
                    .claim_navigation_members
                    .iter()
                    .map(|(path, observation)| {
                        let member = &observation.member;
                        (
                            path.clone(),
                            SourceRevisionClaimMemberEvidence {
                                path: path.clone(),
                                digest: member.metadata.sha256,
                                size_bytes: member.raw.len() as u64,
                                commit_seq: member.commit_seq,
                            },
                        )
                    })
                    .collect();
                Some(SourceRevisionClaimNavigationClosure {
                    domain: session.selection.cohort.domain.clone(),
                    selection_identity,
                    as_of_sequence: session
                        .claim_navigation_as_of_sequence
                        .ok_or(DurableError::Corrupt("Claim navigation as-of cut absent"))?,
                    baseline_sequence: session
                        .claim_navigation_baseline_sequence
                        .ok_or(DurableError::Corrupt("Claim navigation baseline absent"))?,
                    held_source_head_sequence: session.selection.head_seq,
                    lookups: session.claim_navigation_lookups,
                    member_facts,
                    members: session.claim_navigation_members,
                })
            },
        };
        Ok((result, readset))
    }

    /// Reopen only the domain/cohort descriptor needed by an incremental
    /// revision. No complete membership, index, or STO generation is scanned.
    pub(crate) fn open_source_revision_selection(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceRevisionSelection> {
        active(deadline, cancelled)?;
        if domain.is_empty()
            || store.custody_domain() != domain.as_bytes()
            || store.store_id() == [0; 16]
        {
            return Err(DurableError::Conflict(
                "source revision custody domain differs",
            ));
        }
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let row = tx.query_one(
            "SELECT d.*,f.generation AS audit_generation,f.maintenance_state
             FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain)
             WHERE d.domain=$1",
            &[&domain],
        )?;
        if row.get::<_, String>("maintenance_state") != "normal"
            || !row.get::<_, bool>("rights_allowed")
            || !row.get::<_, bool>("source_complete")
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
        {
            return Err(DurableError::Refused(
                "bounded source revision selection is not currently readable",
            ));
        }
        let initial_revision = SourceRevision(parse_hex(
            row.get::<_, Option<String>>("source_revision")
                .ok_or(DurableError::Refused("source cohort revision absent"))?,
        )?);
        let initial_membership = SourceMembershipV1 {
            count: as_u64(
                row.get::<_, Option<i64>>("source_membership_count")
                    .ok_or(DurableError::Corrupt("source membership count absent"))?,
            )?,
            digest: parse_hex(
                row.get::<_, Option<String>>("source_membership_digest")
                    .ok_or(DurableError::Corrupt("source membership digest absent"))?,
            )?,
        };
        let epoch = as_u64(
            row.get::<_, Option<i64>>("source_epoch")
                .ok_or(DurableError::Corrupt("source epoch absent"))?,
        )?;
        let definition_digest = parse_hex(
            row.get::<_, Option<String>>("source_definition_digest")
                .ok_or(DurableError::Corrupt("source definition absent"))?,
        )?;
        super::require_current_source_definition(definition_digest)?;
        let source_generation = as_u64(
            row.get::<_, Option<i64>>("source_generation")
                .ok_or(DurableError::Refused("source generation absent"))?,
        )?;
        let head_seq = as_u64(row.get("head_seq"))?;
        let audit_generation = as_u64(row.get("audit_generation"))?;
        let contract = parse_hex(row.get::<_, String>("contract_digest"))?;
        let rights_version = as_u64(row.get("rights_version"))?;
        let rule_version = as_u64(row.get("rule_version"))?;
        let job = tx
            .query_opt(
                "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
                &[&domain, &job_id],
            )?
            .ok_or(DurableError::Refused(
                "bounded source revision job lease absent",
            ))?;
        if as_u64(job.get(0))? != job_fence {
            return Err(DurableError::Refused(
                "bounded source revision job lease changed",
            ));
        }
        let cohort = ManagedSourceCohort {
            domain: domain.to_owned(),
            store_id: store.store_id(),
            initial_revision,
            initial_membership,
            epoch,
            definition: definition_digest,
            generation: source_generation,
        };
        cohort_matches(&row, &cohort, true)?;
        tx.commit()?;
        Ok(SourceRevisionSelection {
            cohort,
            contract,
            epoch,
            source_generation,
            head_seq,
            audit_generation,
            rights_version,
            rule_version,
            job_id: job_id.to_owned(),
            job_fence,
        })
    }
}

impl SourceRevisionReadSession<'_> {
    pub(crate) fn selection(&self) -> &SourceRevisionSelection {
        &self.selection
    }

    /// Snapshot the exact source/predicate reads accumulated by this held
    /// owner session. Prepared family adapters bind this value into their
    /// proposal so registration cannot accept a different same-selection cut.
    pub(crate) fn reads_snapshot(&self) -> SourceReads {
        self.reads.clone()
    }

    pub(crate) fn ready_work_recorded_at(
        &mut self,
        binding: &super::WorkRevisionRecoveryBinding,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<String>> {
        active(deadline, cancelled)?;
        let state = self.coordinator.client.query_opt(
            "SELECT state FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&binding.domain, &binding.prepare_id],
        )?;
        let Some(state) = state else {
            return Ok(None);
        };
        match state.get::<_, String>(0).as_str() {
            "committed" => return Ok(None),
            "ready" => {}
            "registered" => {
                return Err(DurableError::Indeterminate(
                    "source revision was registered but not durably sealed",
                ));
            }
            _ => {
                return Err(DurableError::Refused(
                    "source revision attempt is not replayable",
                ));
            }
        }
        let attempt = self.coordinator.recover_source_revision_attempt(
            self.store,
            &binding.domain,
            &binding.prepare_id,
            &binding.command_id,
            binding.raw_request_digest,
            binding.owner_configuration_raw_digest,
            binding.owner_configuration_canonical_digest,
            &binding.family_tag,
            true,
            &binding.job_id,
            binding.job_fence,
            Some(binding),
            deadline,
            cancelled,
        )?;
        if attempt.work_revision.is_some() {
            return Ok(None);
        }
        if attempt.epoch != self.selection.epoch
            || attempt.source_generation != self.selection.source_generation
            || attempt.contract != self.selection.contract
            || attempt.definition != self.selection.cohort.definition
            || attempt.rights_version != self.selection.rights_version
            || attempt.rule_version != self.selection.rule_version
            || attempt.job_id != self.selection.job_id
            || attempt.job_fence != self.selection.job_fence
        {
            return Err(DurableError::Conflict(
                "ready source revision selection changed",
            ));
        }
        attempt
            .recovered_work_recorded_at
            .map(Some)
            .ok_or(DurableError::Corrupt(
                "ready PG Work timestamp evidence absent",
            ))
    }

    /// Observe the required v1 selected-record journal for a Work owner.
    pub(crate) fn observe_pg_work_revision(
        &mut self,
        source_path: &str,
        record_id: &str,
        requested_command_id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<crate::source_pg_work_revision::Observation> {
        self.observe_pg_record_revision(
            source_path,
            record_id,
            requested_command_id,
            true,
            deadline,
            cancelled,
        )?
        .ok_or(DurableError::Refused("PG Work revision extension absent"))
    }

    /// Reuse the Work-compatible receipt format for every selected record
    /// owner. Selected revisions require the explicitly provisioned v1 extension
    /// so both independent and publication-bound commits replay exactly.
    pub(crate) fn observe_pg_selected_revision(
        &mut self,
        source_path: &str,
        record_id: &str,
        requested_command_id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<crate::source_pg_work_revision::Observation> {
        self.observe_pg_record_revision(
            source_path,
            record_id,
            requested_command_id,
            false,
            deadline,
            cancelled,
        )?
        .ok_or(DurableError::Refused(
            "PG selected record revision extension absent",
        ))
    }

    fn observe_pg_record_revision(
        &mut self,
        source_path: &str,
        record_id: &str,
        requested_command_id: &str,
        required: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<crate::source_pg_work_revision::Observation>> {
        active(deadline, cancelled)?;
        if self.work_revision.is_some() {
            return Err(DurableError::Conflict(
                "PG Work revision observed more than once",
            ));
        }
        let path = RelativePath::parse(source_path)
            .map_err(|_| DurableError::Invalid("PG selected revision source path"))?;
        if !path.as_str().starts_with("ToS/source-witnesses/")
            || !path.as_str().ends_with(".json")
            || path.as_str().split('/').any(|part| part.starts_with('.'))
            || record_id.is_empty()
            || record_id.len() > 256
        {
            return Err(DurableError::Refused("PG selected revision owner identity"));
        }
        let token = crate::source_pg_work_revision::predicate_token(
            &self.selection.cohort.store_id,
            source_path,
            record_id,
        );
        let domain = self.selection.cohort.domain.as_str();
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        let gate = tx.query_one(
            "SELECT rights_allowed,source_complete,source_generation,source_epoch,
                    contract_digest,source_definition_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
            || !gate.get::<_, bool>(0)
            || !gate.get::<_, bool>(1)
            || gate.get::<_, Option<i64>>(2) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(4) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(5) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "PG selected revision read epoch changed",
            ));
        }
        let extension_present: bool = tx
            .query_one(
                "SELECT to_regclass(format('%I.%I',current_schema(),$1::text)) IS NOT NULL",
                &[&"cmd2_work_revision_history_v1"],
            )?
            .get(0);
        if !extension_present {
            tx.commit()?;
            if required {
                return Err(DurableError::Refused("PG Work revision extension absent"));
            }
            return Ok(None);
        }
        crate::source_pg_work_revision::require_schema_v1(&mut tx, deadline, cancelled)?;
        let predicate = tx.query_opt(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4",
            &[
                &domain,
                &OWNER,
                &crate::source_pg_work_revision::PREDICATE_SCOPE,
                &token,
            ],
        )?;
        let generation = predicate
            .as_ref()
            .map(|row| as_u64(row.get(0)))
            .transpose()?
            .unwrap_or(0);
        if predicate.as_ref().is_some_and(|row| {
            !row.get::<_, bool>(1)
                || row.get::<_, String>(2) != self.selection.cohort.definition.to_hex()
        }) {
            return Err(DurableError::Conflict(
                "PG Work revision predicate definition differs",
            ));
        }
        let mut observation = crate::source_pg_work_revision::observe(
            &mut tx,
            domain,
            self.selection.cohort.store_id,
            source_path,
            record_id,
            generation,
            requested_command_id,
        )?;
        tx.commit()?;
        if let Some(selected) = observation.selected_refs.take() {
            let mut before = Vec::new();
            let mut after = Vec::new();
            for reference in &selected.before {
                if let Some(file) =
                    self.read_historical_work_member(reference, deadline, cancelled)?
                {
                    before.push(file);
                }
            }
            for reference in &selected.after {
                if let Some(file) =
                    self.read_historical_work_member(reference, deadline, cancelled)?
                {
                    after.push(file);
                }
            }
            observation.retained_bytes = observation
                .retained_bytes
                .checked_add(
                    before
                        .iter()
                        .chain(&after)
                        .try_fold(0usize, |total, file| {
                            total
                                .checked_add(file.raw.capacity())
                                .and_then(|bytes| bytes.checked_add(file.path.as_str().len()))
                        })
                        .ok_or(DurableError::Refused("PG Work replay state overflow"))?,
                )
                .filter(|bytes| *bytes <= MAX_RETAINED_STATE_BYTES)
                .ok_or(DurableError::Refused("PG Work replay state budget"))?;
            observation.selected = Some(crate::source_revisions::RetainedRevisionTransaction {
                transaction_id: selected.transaction_id,
                status: crate::source_revisions::RevisionTransactionStatus::Committed,
                authorization_raw: selected.authorization_raw,
                before,
                after,
            });
        }
        self.record_predicate(
            "range",
            crate::source_pg_work_revision::PREDICATE_SCOPE,
            &token,
            generation,
        );
        self.retained_state_bytes = self
            .retained_state_bytes
            .checked_add(observation.retained_bytes)
            .filter(|bytes| *bytes <= MAX_RETAINED_STATE_BYTES)
            .ok_or(DurableError::Refused(
                "PG Work revision reader state budget",
            ))?;
        self.work_revision = Some(observation.clone());
        Ok(Some(observation))
    }

    fn read_historical_work_member(
        &mut self,
        reference: &crate::source_pg_work_revision::MemberVersionRef,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<crate::source_command::SourceFile>> {
        active(deadline, cancelled)?;
        let (Some(revision), Some(expected_digest)) = (reference.revision, reference.digest) else {
            if reference.revision.is_none() && reference.digest.is_none() {
                return Ok(None);
            }
            return Err(DurableError::Corrupt(
                "PG Work historical reference binding",
            ));
        };
        let path = RelativePath::parse(&reference.path)
            .map_err(|_| DurableError::Corrupt("PG Work historical reference path"))?;
        let (row, raw, _placement, work) = self.coordinator.read_committed_source_selected(
            self.store,
            &self.selection.cohort.domain,
            path.as_str(),
            Some(revision),
            8_388_608,
            deadline,
            cancelled,
            |row| cohort_matches(row, &self.selection.cohort, true),
        )?;
        if as_u64(row.get("revision"))? != revision
            || parse_hex(row.get::<_, String>("content_digest"))? != expected_digest
            || Digest256::of_bytes(&raw) != expected_digest
        {
            return Err(DurableError::Conflict(
                "PG Work historical member reference differs",
            ));
        }
        Self::charge_work(&mut self.work.exact_source_members_read, 1)?;
        Self::charge_work(&mut self.work.exact_source_bytes_read, raw.len() as u64)?;
        Self::charge_work(&mut self.work.historical_source_members_read, 1)?;
        Self::charge_work(
            &mut self.work.historical_source_bytes_read,
            raw.len() as u64,
        )?;
        Self::charge_work(
            &mut self.work.custody_metadata_bytes_read,
            work.custody_metadata_bytes_read,
        )?;
        Self::charge_work(&mut self.work.segment_bytes_read, work.segment_bytes_read)?;
        Self::charge_work(
            &mut self.work.committed_member_rows_examined,
            work.committed_member_rows_examined,
        )?;
        Ok(Some(crate::source_command::SourceFile { path, raw }))
    }

    fn charge_work(counter: &mut u64, amount: u64) -> DurableResult<()> {
        *counter = counter
            .checked_add(amount)
            .ok_or(DurableError::Refused("bounded source read work overflow"))?;
        Ok(())
    }

    fn observe_predicate_generation(
        &mut self,
        kind: PredicateKind,
        scope: &str,
        token: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<u64> {
        active(deadline, cancelled)?;
        if scope.is_empty() || token.is_empty() {
            return Err(DurableError::Invalid(
                "bounded source predicate identity empty",
            ));
        }
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "bounded source predicate audit fence changed",
            ));
        }
        let gate = tx.query_one(
            "SELECT rights_allowed,source_complete,source_generation,source_epoch,
                    contract_digest,source_definition_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if !gate.get::<_, bool>(0)
            || !gate.get::<_, bool>(1)
            || gate.get::<_, Option<i64>>(2) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(4) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(5) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "bounded source predicate read epoch changed",
            ));
        }
        let kind_text = kind.as_str();
        let predicate = tx.query_opt(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5",
            &[
                &self.selection.cohort.domain,
                &kind_text,
                &OWNER,
                &scope,
                &token,
            ],
        )?;
        let generation = predicate
            .as_ref()
            .map(|row| as_u64(row.get(0)))
            .transpose()?
            .unwrap_or(0);
        if predicate.as_ref().is_some_and(|row| {
            !row.get::<_, bool>(1)
                || row.get::<_, String>(2) != self.selection.cohort.definition.to_hex()
        }) {
            return Err(DurableError::Conflict(
                "bounded source predicate definition differs",
            ));
        }
        tx.commit()?;
        self.record_predicate(kind_text, scope, token, generation);
        Ok(generation)
    }

    /// Record and verify that the complete source-home prefix is empty. The
    /// C-collated half-open range is an indexed existence check, not a corpus
    /// directory scan.
    pub(crate) fn observe_home_absence(
        &mut self,
        home: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgSourceHomeObservation> {
        active(deadline, cancelled)?;
        let home = home.as_str();
        if !home.starts_with("ToS/") || home.ends_with('/') {
            return Err(DurableError::Invalid("bounded source home path"));
        }
        // Bind the generation before checking absence. Reading it afterwards
        // could accept an insertion between the empty-range read and the
        // generation observation. A later mutation invalidates this predicate
        // at the common source-revision commit boundary.
        let generation = self.observe_predicate_generation(
            PredicateKind::Range,
            "source-home",
            home,
            deadline,
            cancelled,
        )?;
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "bounded source home audit fence changed",
            ));
        }
        let lower = format!("{home}/");
        let upper = format!("{home}0");
        if tx
            .query_opt(
                "SELECT subject FROM cmd2_current
                 WHERE domain=$1 AND subject=$2
                 LIMIT 1",
                &[&self.selection.cohort.domain, &home],
            )?
            .is_some()
        {
            return Err(DurableError::Conflict(
                "bounded source home is already occupied",
            ));
        }
        if tx
            .query_opt(
                "SELECT subject FROM cmd2_current
                 WHERE domain=$1
                   AND subject COLLATE \"C\" >= $2 COLLATE \"C\"
                   AND subject COLLATE \"C\" < $3 COLLATE \"C\"
                 ORDER BY subject COLLATE \"C\" LIMIT 1",
                &[&self.selection.cohort.domain, &lower, &upper],
            )?
            .is_some()
        {
            return Err(DurableError::Conflict(
                "bounded source home is already occupied",
            ));
        }
        tx.commit()?;
        Ok(PgSourceHomeObservation {
            parent: RelativePath::parse(home)
                .map_err(|_| DurableError::Invalid("bounded source home path"))?,
            children: Vec::new(),
            generation,
        })
    }

    /// Record a point unique-identity observation. Only the fixed owner index
    /// kinds are admitted by the bounded Agent-creation route.
    pub(crate) fn observe_unique_index(
        &mut self,
        kind: &str,
        token: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<PgSourceUniqueIndexObservation> {
        if !PG_AGENT_INDEX_KINDS.contains(&kind) {
            return Err(DurableError::Refused(
                "bounded source unique index kind is outside Agent profile",
            ));
        }
        if token.is_empty() {
            return Err(DurableError::Invalid("bounded source unique token empty"));
        }
        active(deadline, cancelled)?;
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "bounded source unique audit fence changed",
            ));
        }
        let kinds = PG_AGENT_INDEX_KINDS
            .iter()
            .map(|kind| (*kind).to_owned())
            .collect::<Vec<_>>();
        if tx
            .query_opt(
                "SELECT 1 FROM cmd2_source_index
                 WHERE domain=$1 AND kind=ANY($2) AND token=$3 LIMIT 1",
                &[&self.selection.cohort.domain, &kinds, &token],
            )?
            .is_some()
        {
            return Err(DurableError::Conflict(
                "bounded source unique identity is already occupied",
            ));
        }
        tx.commit()?;
        Self::charge_work(&mut self.work.unique_index_lookups, 1)?;
        // A token is unique across the five fixed namespaces.  Record every
        // namespace's generation in the readset so a later write in any one
        // of them invalidates this admission.
        let mut generation = None;
        for observed_kind in PG_AGENT_INDEX_KINDS {
            let observed = self.observe_predicate_generation(
                PredicateKind::Unique,
                observed_kind,
                token,
                deadline,
                cancelled,
            )?;
            if observed_kind == kind {
                generation = Some(observed);
            }
        }
        Ok(PgSourceUniqueIndexObservation {
            kind: kind.to_owned(),
            token: token.to_owned(),
            rows: Vec::new(),
            generation: generation.ok_or(DurableError::Corrupt(
                "bounded source unique generation missing",
            ))?,
        })
    }

    /// Resolve an exact Sign promotion candidate through its per-kind index.
    /// Unlike the five globally unique metadata identities, candidate Claims
    /// may legitimately share an ID with `kind='claim'`; this observation is
    /// fenced only by the Sign candidate namespace and its own generation.
    pub(crate) fn lookup_sign_promotion_candidate(
        &mut self,
        token: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<RelativePath>> {
        if token.is_empty() || token.len() > MAX_SIGN_CANDIDATE_TOKEN_BYTES {
            return Err(DurableError::Invalid(
                "Sign promotion candidate token bound",
            ));
        }
        active(deadline, cancelled)?;
        let domain = self.selection.cohort.domain.as_str();
        let (path, generation) = {
            let mut tx = self.coordinator.client.transaction()?;
            tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
            let fence = tx.query_one(
                "SELECT generation,maintenance_state FROM cmd2_audit_fence
                 WHERE domain=$1 FOR SHARE",
                &[&domain],
            )?;
            if fence.get::<_, String>(1) != "normal"
                || as_u64(fence.get(0))? != self.selection.audit_generation
            {
                return Err(DurableError::Conflict(
                    "Sign promotion candidate audit fence changed",
                ));
            }
            let gate = tx.query_one(
                "SELECT rights_allowed,source_complete,source_generation,source_epoch,
                        contract_digest,source_definition_digest
                 FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&domain],
            )?;
            let definition = gate
                .get::<_, Option<String>>(5)
                .ok_or(DurableError::Corrupt("source definition absent"))?;
            super::require_current_source_definition(parse_hex(definition)?)?;
            if !gate.get::<_, bool>(0)
                || !gate.get::<_, bool>(1)
                || gate.get::<_, Option<i64>>(2) != Some(as_i64(self.selection.source_generation)?)
                || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.epoch)?)
                || gate.get::<_, String>(4) != self.selection.contract.to_hex()
                || gate.get::<_, Option<String>>(5)
                    != Some(self.selection.cohort.definition.to_hex())
            {
                return Err(DurableError::Conflict(
                    "Sign promotion candidate source epoch changed",
                ));
            }
            let row = tx.query_opt(
                "SELECT path,definition_digest FROM cmd2_source_index
                 WHERE domain=$1 AND kind='sign-candidate' AND token=$2",
                &[&domain, &token],
            )?;
            let path = if let Some(row) = row {
                if row.get::<_, String>(1) != self.selection.cohort.definition.to_hex() {
                    return Err(DurableError::Conflict(
                        "Sign promotion candidate index definition changed",
                    ));
                }
                let path = row.get::<_, String>(0);
                if !path.starts_with("ToS/")
                    || path.starts_with("ToS/source-witnesses/owner-local/")
                {
                    return Err(DurableError::Corrupt(
                        "Sign promotion candidate index path is outside public ToS",
                    ));
                }
                let path = RelativePath::parse(&path)
                    .map_err(|_| DurableError::Corrupt("Sign promotion candidate index path"))?;
                if tx
                    .query_opt(
                        "SELECT 1 FROM cmd2_current WHERE domain=$1 AND subject=$2 LIMIT 1",
                        &[&domain, &path.as_str()],
                    )?
                    .is_none()
                {
                    return Err(DurableError::Corrupt(
                        "Sign promotion candidate index is not current",
                    ));
                }
                Some(path)
            } else {
                None
            };
            let predicate = tx.query_opt(
                "SELECT generation,complete,definition_version FROM cmd2_predicate
                 WHERE domain=$1 AND kind='unique' AND owner=$2
                   AND scope='sign-candidate' AND token=$3",
                &[&domain, &OWNER, &token],
            )?;
            if predicate.as_ref().is_some_and(|row| {
                !row.get::<_, bool>(1)
                    || row.get::<_, String>(2) != self.selection.cohort.definition.to_hex()
            }) {
                return Err(DurableError::Conflict(
                    "Sign promotion candidate predicate definition changed",
                ));
            }
            let generation = predicate
                .as_ref()
                .map(|row| as_u64(row.get(0)))
                .transpose()?
                .unwrap_or(0);
            active(deadline, cancelled)?;
            tx.commit()?;
            (path, generation)
        };
        Self::charge_work(&mut self.work.sign_candidate_lookups, 1)?;
        self.record_predicate("unique", "sign-candidate", token, generation);
        Ok(path)
    }

    /// Resolve one maintained identity through the current owner index and
    /// bind every namespace generation that could introduce a collision.
    /// Claim grounding uses this instead of scanning all authored files.
    pub(crate) fn lookup_source_identity(
        &mut self,
        token: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<(String, RelativePath)>> {
        let tokens = BTreeSet::from([token.to_owned()]);
        let closure = self.lookup_source_identities(&tokens, deadline, cancelled)?;
        let owner = closure.owners().get(token).cloned().flatten();
        Ok(owner)
    }

    /// Resolve only the exact identities needed by a selected owner closure.
    /// Each requested token is checked across the complete fixed uniqueness
    /// domain, including absent tokens, and all five generation predicates are
    /// bound to this readset. The caller still reads and parses each returned
    /// current member through `read_current_member`; this index is navigation
    /// and absence evidence, never semantic authority.
    pub(crate) fn lookup_source_identities(
        &mut self,
        tokens: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceRevisionIdentityClosure> {
        if tokens.len() > MAX_SOURCE_IDENTITY_LOOKUPS {
            return Err(DurableError::Refused(
                "bounded source identity lookup count exceeded",
            ));
        }
        if tokens
            .iter()
            .any(|token| token.is_empty() || token.len() > MAX_SOURCE_IDENTITY_TOKEN_BYTES)
        {
            return Err(DurableError::Invalid("bounded source identity token bound"));
        }
        if tokens.is_empty() {
            return Ok(SourceRevisionIdentityClosure {
                selection_identity: self.selection.identity(),
                owners: BTreeMap::new(),
                generations: BTreeMap::new(),
            });
        }
        active(deadline, cancelled)?;

        let kinds = PG_AGENT_INDEX_KINDS
            .iter()
            .map(|kind| (*kind).to_owned())
            .collect::<Vec<_>>();
        let token_values = tokens.iter().cloned().collect::<Vec<_>>();
        let (identities, generations) = {
            let mut tx = self.coordinator.client.transaction()?;
            tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
            let fence = tx.query_one(
                "SELECT generation,maintenance_state FROM cmd2_audit_fence
                 WHERE domain=$1 FOR SHARE",
                &[&self.selection.cohort.domain],
            )?;
            if fence.get::<_, String>(1) != "normal"
                || as_u64(fence.get(0))? != self.selection.audit_generation
            {
                return Err(DurableError::Conflict(
                    "bounded source identity batch audit fence changed",
                ));
            }
            let gate = tx.query_one(
                "SELECT rights_allowed,rights_version,source_complete,source_generation,
                        source_epoch,contract_digest,source_definition_digest
                 FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&self.selection.cohort.domain],
            )?;
            super::require_current_source_definition(parse_hex(
                gate.get::<_, Option<String>>(6)
                    .ok_or(DurableError::Corrupt("source definition absent"))?,
            )?)?;
            if !gate.get::<_, bool>(0)
                || as_u64(gate.get(1))? != self.selection.rights_version
                || !gate.get::<_, bool>(2)
                || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.source_generation)?)
                || gate.get::<_, Option<i64>>(4) != Some(as_i64(self.selection.epoch)?)
                || gate.get::<_, String>(5) != self.selection.contract.to_hex()
                || gate.get::<_, Option<String>>(6)
                    != Some(self.selection.cohort.definition.to_hex())
            {
                return Err(DurableError::Conflict(
                    "bounded source identity batch read epoch changed",
                ));
            }

            let rows = tx.query(
                "SELECT kind,token,path,definition_digest FROM cmd2_source_index
                 WHERE domain=$1 AND kind=ANY($2) AND token=ANY($3)
                 ORDER BY token COLLATE \"C\",kind COLLATE \"C\"",
                &[&self.selection.cohort.domain, &kinds, &token_values],
            )?;
            let mut identities = tokens
                .iter()
                .map(|token| (token.clone(), None))
                .collect::<BTreeMap<String, Option<(String, RelativePath)>>>();
            for row in rows {
                let kind: String = row.get("kind");
                let token: String = row.get("token");
                let raw_path: String = row.get("path");
                let definition: String = row.get("definition_digest");
                if !PG_AGENT_INDEX_KINDS.contains(&kind.as_str())
                    || !tokens.contains(&token)
                    || definition != self.selection.cohort.definition.to_hex()
                {
                    return Err(DurableError::Conflict(
                        "bounded source identity batch index definition differs",
                    ));
                }
                let path = RelativePath::parse(&raw_path)
                    .map_err(|_| DurableError::Corrupt("bounded source identity path"))?;
                if !path.as_str().starts_with("ToS/") {
                    return Err(DurableError::Refused(
                        "bounded source identity path outside authored namespace",
                    ));
                }
                let slot = identities.get_mut(&token).ok_or(DurableError::Corrupt(
                    "bounded source identity token absent",
                ))?;
                if slot.is_some() {
                    return Err(DurableError::Conflict(
                        "bounded source identity index is not globally unique",
                    ));
                }
                *slot = Some((kind, path));
            }

            let predicate_rows = tx.query(
                "SELECT scope,token,generation,complete,definition_version
                 FROM cmd2_predicate
                 WHERE domain=$1 AND kind='unique' AND owner=$2
                   AND scope=ANY($3) AND token=ANY($4)
                 ORDER BY token COLLATE \"C\",scope COLLATE \"C\"",
                &[&self.selection.cohort.domain, &OWNER, &kinds, &token_values],
            )?;
            let mut generations = BTreeMap::new();
            for row in predicate_rows {
                let scope: String = row.get("scope");
                let token: String = row.get("token");
                if !PG_AGENT_INDEX_KINDS.contains(&scope.as_str())
                    || !tokens.contains(&token)
                    || !row.get::<_, bool>("complete")
                    || row.get::<_, String>("definition_version")
                        != self.selection.cohort.definition.to_hex()
                {
                    return Err(DurableError::Conflict(
                        "bounded source identity batch predicate definition differs",
                    ));
                }
                generations.insert((scope, token), as_u64(row.get("generation"))?);
            }
            for token in tokens {
                for kind in PG_AGENT_INDEX_KINDS {
                    generations
                        .entry((kind.to_owned(), token.clone()))
                        .or_insert(0);
                }
            }
            tx.commit()?;
            (identities, generations)
        };

        Self::charge_work(&mut self.work.unique_index_lookups, tokens.len() as u64)?;
        for token in tokens {
            for kind in PG_AGENT_INDEX_KINDS {
                let generation = generations
                    .get(&(kind.to_owned(), token.clone()))
                    .copied()
                    .unwrap_or(0);
                self.record_predicate("unique", kind, &token, generation);
            }
        }
        Ok(SourceRevisionIdentityClosure {
            selection_identity: self.selection.identity(),
            owners: identities,
            generations,
        })
    }

    /// Resolve a bounded batch of public Text identity owners from the
    /// dedicated multi-owner index. The separate token generation is bound
    /// even for an empty result, so a later insertion invalidates this read.
    pub(crate) fn lookup_text_identity_owners(
        &mut self,
        tokens: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<BTreeMap<String, crate::source_text_identity::PgSourceTextIdentityObservation>>
    {
        if tokens.is_empty() {
            return Ok(BTreeMap::new());
        }
        active(deadline, cancelled)?;
        let domain = self.selection.cohort.domain.as_str();
        let (owners, generations) = {
            let mut tx = self.coordinator.client.transaction()?;
            tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
            let fence = tx.query_one(
                "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
                &[&domain],
            )?;
            if fence.get::<_, String>(1) != "normal"
                || as_u64(fence.get(0))? != self.selection.audit_generation
            {
                return Err(DurableError::Conflict("Text identity audit fence changed"));
            }
            let gate = tx.query_one(
                "SELECT rights_allowed,rights_version,source_complete,source_generation,
                        source_epoch,contract_digest,source_definition_digest
                 FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&domain],
            )?;
            let source_definition = gate
                .get::<_, Option<String>>(6)
                .ok_or(DurableError::Corrupt("source definition absent"))?;
            super::require_current_source_definition(parse_hex(source_definition)?)?;
            if !gate.get::<_, bool>(0)
                || as_u64(gate.get(1))? != self.selection.rights_version
                || !gate.get::<_, bool>(2)
                || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.source_generation)?)
                || gate.get::<_, Option<i64>>(4) != Some(as_i64(self.selection.epoch)?)
                || gate.get::<_, String>(5) != self.selection.contract.to_hex()
                || gate.get::<_, Option<String>>(6)
                    != Some(self.selection.cohort.definition.to_hex())
            {
                return Err(DurableError::Conflict(
                    "Text identity source read epoch changed",
                ));
            }
            let owners = crate::source_text_identity_index::lookup_current_owners(
                &mut tx,
                domain,
                tokens,
                self.selection.cohort.definition,
            )?;
            let token_values = tokens.iter().cloned().collect::<Vec<_>>();
            let predicate_rows = tx.query(
                "SELECT token,generation,complete,definition_version FROM cmd2_predicate
                 WHERE domain=$1 AND kind='unique' AND owner=$2 AND scope=$3 AND token=ANY($4)
                 ORDER BY token COLLATE \"C\"",
                &[
                    &domain,
                    &OWNER,
                    &crate::source_text_identity_index::PREDICATE_SCOPE,
                    &token_values,
                ],
            )?;
            let mut generations = tokens
                .iter()
                .map(|token| (token.clone(), 0u64))
                .collect::<BTreeMap<_, _>>();
            let mut seen_predicates = BTreeSet::new();
            for row in predicate_rows {
                let token: String = row.get("token");
                if !row.get::<_, bool>("complete")
                    || row.get::<_, String>("definition_version")
                        != self.selection.cohort.definition.to_hex()
                    || generations
                        .insert(token, as_u64(row.get("generation"))?)
                        .is_none()
                {
                    return Err(DurableError::Conflict(
                        "Text identity predicate definition differs",
                    ));
                }
                let token: String = row.get("token");
                if !seen_predicates.insert(token) {
                    return Err(DurableError::Corrupt(
                        "duplicate Text identity predicate row",
                    ));
                }
            }
            for token in tokens {
                let owned = owners
                    .get(token)
                    .is_some_and(|token_owners| !token_owners.is_empty());
                if owned
                    && (!seen_predicates.contains(token)
                        || generations.get(token).copied().unwrap_or(0) == 0)
                {
                    return Err(DurableError::Corrupt(
                        "owned Text identity token lacks its complete predicate generation",
                    ));
                }
            }
            tx.commit()?;
            (owners, generations)
        };
        Self::charge_work(&mut self.work.text_identity_lookups, tokens.len() as u64)?;
        let mut result = BTreeMap::new();
        for token in tokens {
            let generation = generations
                .get(token)
                .copied()
                .ok_or(DurableError::Corrupt(
                    "Text identity predicate token absent",
                ))?;
            self.record_predicate(
                "unique",
                crate::source_text_identity_index::PREDICATE_SCOPE,
                token,
                generation,
            );
            let token_owners = owners
                .get(token)
                .cloned()
                .ok_or(DurableError::Corrupt("Text identity lookup token absent"))?;
            result.insert(
                token.clone(),
                crate::source_text_identity::PgSourceTextIdentityObservation {
                    owners: token_owners,
                    generation,
                },
            );
        }
        Ok(result)
    }

    /// Read a bounded batch of typed reverse Claim edges and their Range
    /// generations in the same fenced transaction. An empty result is only
    /// point-absence evidence for this predicate; later additions advance its
    /// generation atomically with the source commit.
    pub(crate) fn lookup_claim_dependencies(
        &mut self,
        keys: &BTreeSet<(String, String, String)>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<
        BTreeMap<
            (String, String, String),
            crate::source_claim_dependency_index::ClaimDependencyObservation,
        >,
    > {
        if keys.is_empty() {
            return Ok(BTreeMap::new());
        }
        if keys.len() > MAX_CLAIM_DEPENDENCY_LOOKUPS {
            return Err(DurableError::Refused(
                "Claim dependency lookup count bound exceeded",
            ));
        }
        active(deadline, cancelled)?;
        let domain = self.selection.cohort.domain.as_str();
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "Claim dependency audit fence changed",
            ));
        }
        let gate = tx.query_one(
            "SELECT rights_allowed,rights_version,source_complete,source_generation,
                    source_epoch,contract_digest,source_definition_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        let source_definition = gate
            .get::<_, Option<String>>(6)
            .ok_or(DurableError::Corrupt("source definition absent"))?;
        super::require_current_source_definition(parse_hex(source_definition)?)?;
        if !gate.get::<_, bool>(0)
            || as_u64(gate.get(1))? != self.selection.rights_version
            || !gate.get::<_, bool>(2)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(4) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(5) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(6) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "Claim dependency source read epoch changed",
            ));
        }
        let mut total_rows = 0usize;
        let mut result = BTreeMap::new();
        for (direction, predicate, endpoint) in keys {
            active(deadline, cancelled)?;
            let direction = match direction.as_str() {
                "subject" => {
                    crate::source_claim_dependency_index::ClaimDependencyDirection::Subject
                }
                "object" => crate::source_claim_dependency_index::ClaimDependencyDirection::Object,
                _ => return Err(DurableError::Invalid("Claim dependency direction")),
            };
            let observation = crate::source_claim_dependency_index::lookup_current(
                &mut tx,
                domain,
                direction,
                predicate,
                endpoint,
                self.selection.cohort.definition,
            )?;
            total_rows = total_rows
                .checked_add(observation.rows.len())
                .filter(|count| *count <= MAX_CLAIM_DEPENDENCY_ROWS)
                .ok_or(DurableError::Refused(
                    "Claim dependency lookup aggregate row bound exceeded",
                ))?;
            result.insert(
                (
                    direction.as_str().to_owned(),
                    predicate.clone(),
                    endpoint.clone(),
                ),
                observation,
            );
        }
        active(deadline, cancelled)?;
        tx.commit()?;
        Self::charge_work(&mut self.work.claim_dependency_lookups, keys.len() as u64)?;
        for ((direction, predicate, endpoint), observation) in &result {
            let token = &observation.predicate_token;
            self.record_predicate(
                "range",
                crate::source_claim_dependency_index::PREDICATE_SCOPE,
                token,
                observation.generation,
            );
            if direction != "subject" && direction != "object" {
                return Err(DurableError::Corrupt(
                    "Claim dependency result direction differs",
                ));
            }
            let _ = (predicate, endpoint);
        }
        Ok(result)
    }

    /// Resolve exact reverse references at an authored-source cut while the
    /// readset remains bound to the session's held current source head.
    pub(crate) fn lookup_claim_navigation_references_at(
        &mut self,
        keys: &BTreeSet<(
            crate::source_claim_dependency_index::ClaimNavigationReferenceKind,
            String,
        )>,
        as_of_sequence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<
        BTreeMap<
            (
                crate::source_claim_dependency_index::ClaimNavigationReferenceKind,
                String,
            ),
            crate::source_claim_dependency_index::ClaimNavigationReferenceObservation,
        >,
    > {
        use crate::source_claim_dependency_index::{
            MAX_NAVIGATION_REFERENCE_LOOKUPS, MAX_NAVIGATION_REFERENCE_ROWS, MAX_STREAM_BYTES,
            NAVIGATION_REFERENCE_SCOPE,
        };

        if self
            .claim_navigation_as_of_sequence
            .is_some_and(|existing| existing != as_of_sequence)
        {
            return Err(DurableError::Conflict(
                "Claim navigation read session mixed as-of cuts",
            ));
        }
        let additional = keys
            .iter()
            .filter(|key| !self.claim_navigation_lookups.contains_key(*key))
            .count();
        if self
            .claim_navigation_lookups
            .len()
            .checked_add(additional)
            .filter(|count| *count <= MAX_NAVIGATION_REFERENCE_LOOKUPS)
            .is_none()
        {
            return Err(DurableError::Refused(
                "Claim navigation lookup count bound exceeded",
            ));
        }
        active(deadline, cancelled)?;
        let domain = self.selection.cohort.domain.clone();
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "Claim navigation audit fence changed",
            ));
        }
        let gate = tx.query_one(
            "SELECT rights_allowed,rights_version,source_complete,source_generation,
                    source_epoch,contract_digest,source_definition_digest,head_seq,
                    source_claim_navigation_reference_baseline_seq
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        let source_definition = gate
            .get::<_, Option<String>>(6)
            .ok_or(DurableError::Corrupt("source definition absent"))?;
        super::require_current_source_definition(parse_hex(source_definition)?)?;
        if !gate.get::<_, bool>(0)
            || as_u64(gate.get(1))? != self.selection.rights_version
            || !gate.get::<_, bool>(2)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(4) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(5) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(6) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "Claim navigation source read epoch changed",
            ));
        }
        let held_source_head_sequence = as_u64(gate.get(7))?;
        let baseline_sequence = gate
            .get::<_, Option<i64>>(8)
            .map(as_u64)
            .transpose()?
            .ok_or(DurableError::Conflict(
                "Claim navigation interval baseline absent",
            ))?;
        if held_source_head_sequence != self.selection.head_seq
            || as_of_sequence < baseline_sequence
            || as_of_sequence > held_source_head_sequence
        {
            return Err(DurableError::Conflict(
                "Claim navigation as-of cut is outside the held source interval",
            ));
        }

        let mut total_rows = self
            .claim_navigation_lookups
            .values()
            .map(|observation| observation.rows.len())
            .sum::<usize>();
        let mut fresh = BTreeMap::new();
        for (kind, endpoint) in keys {
            if self
                .claim_navigation_lookups
                .contains_key(&(*kind, endpoint.clone()))
            {
                continue;
            }
            active(deadline, cancelled)?;
            let observation =
                crate::source_claim_dependency_index::lookup_navigation_references_at(
                    &mut tx,
                    &domain,
                    *kind,
                    endpoint,
                    self.selection.cohort.definition,
                    as_of_sequence,
                    held_source_head_sequence,
                )?;
            if observation.kind != *kind || observation.endpoint != *endpoint {
                return Err(DurableError::Corrupt(
                    "Claim navigation lookup identity differs",
                ));
            }
            total_rows = total_rows
                .checked_add(observation.rows.len())
                .filter(|count| *count <= MAX_NAVIGATION_REFERENCE_ROWS)
                .ok_or(DurableError::Refused(
                    "Claim navigation lookup aggregate row bound exceeded",
                ))?;
            if fresh
                .insert((*kind, endpoint.clone()), observation)
                .is_some()
            {
                return Err(DurableError::Corrupt(
                    "duplicate Claim navigation lookup key",
                ));
            }
        }
        let (completeness_token, completeness_generation) =
            crate::source_claim_dependency_index::navigation_completeness_observation(
                &mut tx,
                &domain,
                self.selection.cohort.definition,
            )?;
        let mut member_facts = BTreeMap::<String, (Digest256, u64, u64)>::new();
        for observation in fresh.values() {
            for row in &observation.rows {
                let path = row.path.as_str();
                if !path.starts_with("ToS/source-witnesses/")
                    || path.starts_with("ToS/source-witnesses/owner-local/")
                    || !path.ends_with(&format!("/{}", crate::source_claims::CLAIM_STREAM))
                {
                    return Err(DurableError::Corrupt(
                        "Claim navigation source path is outside public streams",
                    ));
                }
                let facts = (row.member_digest, row.size_bytes, row.commit_seq);
                if member_facts
                    .insert(path.to_owned(), facts)
                    .is_some_and(|existing| existing != facts)
                {
                    return Err(DurableError::Conflict(
                        "Claim navigation rows disagree on as-of stream facts",
                    ));
                }
            }
        }
        let mut member_revisions = BTreeMap::new();
        for (path, (expected_digest, expected_size, expected_commit_seq)) in &member_facts {
            active(deadline, cancelled)?;
            let row = tx
                .query_opt(
                    "SELECT revision,commit_seq,content_digest,content_length,source_mode
                 FROM cmd2_history WHERE domain=$1 AND subject=$2 AND commit_seq<=$3
                 ORDER BY revision DESC LIMIT 1",
                    &[&domain, path, &as_i64(as_of_sequence)?],
                )?
                .ok_or(DurableError::Conflict(
                    "Claim navigation as-of source history absent",
                ))?;
            let revision = as_u64(row.get(0))?;
            let commit_seq = as_u64(row.get(1))?;
            if commit_seq != *expected_commit_seq
                || parse_hex(row.get::<_, String>("content_digest"))? != *expected_digest
                || as_u64(row.get("content_length"))? != *expected_size
                || row.get::<_, Option<i32>>("source_mode").is_none()
            {
                return Err(DurableError::Conflict(
                    "Claim navigation as-of history locator differs",
                ));
            }
            member_revisions.insert(path.clone(), revision);
        }
        active(deadline, cancelled)?;
        tx.commit()?;
        Self::charge_work(&mut self.work.claim_navigation_lookups, fresh.len() as u64)?;
        self.claim_navigation_as_of_sequence = Some(as_of_sequence);
        self.claim_navigation_baseline_sequence = Some(baseline_sequence);
        self.record_predicate(
            "range",
            NAVIGATION_REFERENCE_SCOPE,
            &completeness_token,
            completeness_generation,
        );
        for ((kind, endpoint), observation) in fresh {
            let token = &observation.predicate_token;
            self.record_predicate(
                "range",
                NAVIGATION_REFERENCE_SCOPE,
                token,
                observation.generation,
            );
            self.record_predicate(
                "range",
                NAVIGATION_REFERENCE_SCOPE,
                &observation.completeness_token,
                observation.completeness_generation,
            );
            for row in &observation.rows {
                if row.kind != kind || row.endpoint != endpoint {
                    return Err(DurableError::Corrupt(
                        "Claim navigation row belongs to another lookup",
                    ));
                }
                if row.size_bytes > MAX_STREAM_BYTES as u64 {
                    return Err(DurableError::Refused(
                        "Claim navigation source member byte bound exceeded",
                    ));
                }
                let _ = row;
            }
            self.claim_navigation_lookups
                .insert((kind, endpoint), observation);
        }

        for (path, (expected_digest, expected_size, expected_commit_seq)) in member_facts {
            if self.claim_navigation_members.contains_key(&path) {
                let member = &self.claim_navigation_members[&path].member;
                if Digest256::of_bytes(&member.raw) != expected_digest
                    || member.raw.len() as u64 != expected_size
                    || member.commit_seq != expected_commit_seq
                {
                    return Err(DurableError::Conflict(
                        "Claim navigation repeated as-of stream facts differ",
                    ));
                }
                continue;
            }
            active(deadline, cancelled)?;
            let relative = RelativePath::parse(&path)
                .map_err(|_| DurableError::Corrupt("Claim navigation source path"))?;
            let revision = member_revisions
                .get(&path)
                .copied()
                .ok_or(DurableError::Corrupt(
                    "Claim navigation as-of history revision absent",
                ))?;
            let (row, raw, placement, work) = self.coordinator.read_committed_source_selected(
                self.store,
                &domain,
                &path,
                Some(revision),
                MAX_STREAM_BYTES as u64,
                deadline,
                cancelled,
                |row| cohort_matches(row, &self.selection.cohort, true),
            )?;
            let (metadata, dependency_claims) = expose_metadata(&row, &relative)?;
            let member = ManagedCurrentMember {
                path: relative.clone(),
                custody_revision: revision,
                current_generation: as_of_sequence,
                commit_seq: as_u64(row.get("commit_seq"))?,
                raw,
                metadata,
                dependency_claims,
                placement,
            };
            if Digest256::of_bytes(&member.raw) != expected_digest
                || member.metadata.sha256 != expected_digest
                || member.raw.len() as u64 != expected_size
                || member.commit_seq != expected_commit_seq
            {
                return Err(DurableError::Conflict(
                    "Claim navigation index differs from exact as-of stream",
                ));
            }
            let locator = metadata_locator_digest(&row);
            Self::charge_work(&mut self.work.exact_source_members_read, 1)?;
            Self::charge_work(
                &mut self.work.exact_source_bytes_read,
                member.raw.len() as u64,
            )?;
            Self::charge_work(&mut self.work.historical_source_members_read, 1)?;
            Self::charge_work(
                &mut self.work.historical_source_bytes_read,
                member.raw.len() as u64,
            )?;
            Self::charge_work(
                &mut self.work.custody_metadata_bytes_read,
                work.custody_metadata_bytes_read,
            )?;
            Self::charge_work(&mut self.work.segment_bytes_read, work.segment_bytes_read)?;
            Self::charge_work(
                &mut self.work.committed_member_rows_examined,
                work.committed_member_rows_examined,
            )?;
            self.claim_navigation_members
                .insert(path, SourceRevisionObservation { member, locator });
        }

        Ok(keys
            .iter()
            .filter_map(|key| {
                self.claim_navigation_lookups
                    .get(key)
                    .cloned()
                    .map(|observation| (key.clone(), observation))
            })
            .collect())
    }

    /// Resolve every requested File ID against the typed Item manifest index
    /// and retain the exact Range generations under the same source/audit
    /// fence. An empty result is an authenticated absence only for these
    /// requested File IDs, not a whole-index inventory.
    pub(crate) fn lookup_item_file_assignments(
        &mut self,
        file_ids: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<BTreeMap<String, crate::source_item_file_index::ItemFileObservation>> {
        if file_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        if file_ids.len() > MAX_ITEM_FILE_LOOKUPS
            || file_ids
                .iter()
                .any(|file_id| file_id.is_empty() || file_id.len() > 4_096)
        {
            return Err(DurableError::Refused(
                "Item File lookup identity or count bound exceeded",
            ));
        }
        active(deadline, cancelled)?;
        let domain = self.selection.cohort.domain.as_str();
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict("Item File audit fence changed"));
        }
        let gate = tx.query_one(
            "SELECT rights_allowed,rights_version,source_complete,source_generation,
                    source_epoch,contract_digest,source_definition_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        let source_definition = gate
            .get::<_, Option<String>>(6)
            .ok_or(DurableError::Corrupt("source definition absent"))?;
        super::require_current_source_definition(parse_hex(source_definition)?)?;
        if !gate.get::<_, bool>(0)
            || as_u64(gate.get(1))? != self.selection.rights_version
            || !gate.get::<_, bool>(2)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(4) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(5) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(6) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "Item File source read epoch changed",
            ));
        }
        let mut total_rows = 0usize;
        let mut result = BTreeMap::new();
        for file_id in file_ids {
            active(deadline, cancelled)?;
            let observation = crate::source_item_file_index::lookup_current(
                &mut tx,
                domain,
                file_id,
                self.selection.cohort.definition,
            )?;
            total_rows = total_rows
                .checked_add(observation.rows.len())
                .filter(|count| *count <= MAX_ITEM_FILE_ROWS)
                .ok_or(DurableError::Refused(
                    "Item File lookup aggregate row bound exceeded",
                ))?;
            result.insert(file_id.clone(), observation);
        }
        active(deadline, cancelled)?;
        tx.commit()?;
        Self::charge_work(&mut self.work.item_file_lookups, file_ids.len() as u64)?;
        for observation in result.values() {
            self.record_predicate(
                "range",
                crate::source_item_file_index::PREDICATE_SCOPE,
                &observation.predicate_token,
                observation.generation,
            );
        }
        Ok(result)
    }

    /// Resolve every requested rights identity to its exact current Item
    /// manifests and retain the rights-index Range generations under the
    /// same source/audit fence. This is point closure, not a corpus scan.
    pub(crate) fn lookup_item_rights_assignments(
        &mut self,
        rights_ids: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<BTreeMap<String, crate::source_item_file_index::ItemRightsObservation>> {
        if rights_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        if rights_ids.len() > MAX_ITEM_FILE_LOOKUPS
            || rights_ids
                .iter()
                .any(|rights_id| rights_id.is_empty() || rights_id.len() > 4_096)
        {
            return Err(DurableError::Refused(
                "Item rights lookup identity or count bound exceeded",
            ));
        }
        active(deadline, cancelled)?;
        let domain = self.selection.cohort.domain.as_str();
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict("Item rights audit fence changed"));
        }
        let gate = tx.query_one(
            "SELECT rights_allowed,rights_version,source_complete,source_generation,
                    source_epoch,contract_digest,source_definition_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        let source_definition = gate
            .get::<_, Option<String>>(6)
            .ok_or(DurableError::Corrupt("source definition absent"))?;
        super::require_current_source_definition(parse_hex(source_definition)?)?;
        if !gate.get::<_, bool>(0)
            || as_u64(gate.get(1))? != self.selection.rights_version
            || !gate.get::<_, bool>(2)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(4) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(5) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(6) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "Item rights source read epoch changed",
            ));
        }
        let mut total_rows = 0usize;
        let mut result = BTreeMap::new();
        for rights_id in rights_ids {
            active(deadline, cancelled)?;
            let observation = crate::source_item_file_index::lookup_rights_current(
                &mut tx,
                domain,
                rights_id,
                self.selection.cohort.definition,
            )?;
            total_rows = total_rows
                .checked_add(observation.rows.len())
                .filter(|count| *count <= MAX_ITEM_FILE_ROWS)
                .ok_or(DurableError::Refused(
                    "Item rights lookup aggregate row bound exceeded",
                ))?;
            result.insert(rights_id.clone(), observation);
        }
        active(deadline, cancelled)?;
        tx.commit()?;
        Self::charge_work(&mut self.work.item_rights_lookups, rights_ids.len() as u64)?;
        for observation in result.values() {
            self.record_predicate(
                "range",
                crate::source_item_file_index::RIGHTS_PREDICATE_SCOPE,
                &observation.predicate_token,
                observation.generation,
            );
        }
        Ok(result)
    }

    /// Resolve every requested Edition ID to its current Item manifests and
    /// retain the Edition-assignment Range generation under the same source
    /// and audit fences. This is a bounded reverse lookup, not a corpus scan.
    pub(crate) fn lookup_item_edition_assignments(
        &mut self,
        edition_ids: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<BTreeMap<String, crate::source_item_file_index::ItemEditionObservation>>
    {
        if edition_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        if edition_ids.len() > MAX_ITEM_FILE_LOOKUPS
            || edition_ids
                .iter()
                .any(|edition_id| edition_id.is_empty() || edition_id.len() > 4_096)
        {
            return Err(DurableError::Refused(
                "Item Edition lookup identity or count bound exceeded",
            ));
        }
        active(deadline, cancelled)?;
        let domain = self.selection.cohort.domain.as_str();
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict("Item Edition audit fence changed"));
        }
        let gate = tx.query_one(
            "SELECT rights_allowed,rights_version,source_complete,source_generation,
                    source_epoch,contract_digest,source_definition_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        let source_definition = gate
            .get::<_, Option<String>>(6)
            .ok_or(DurableError::Corrupt("source definition absent"))?;
        super::require_current_source_definition(parse_hex(source_definition)?)?;
        if !gate.get::<_, bool>(0)
            || as_u64(gate.get(1))? != self.selection.rights_version
            || !gate.get::<_, bool>(2)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(4) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(5) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(6) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "Item Edition source read epoch changed",
            ));
        }
        let mut total_rows = 0usize;
        let mut result = BTreeMap::new();
        for edition_id in edition_ids {
            active(deadline, cancelled)?;
            let observation = crate::source_item_file_index::lookup_edition_current(
                &mut tx,
                domain,
                edition_id,
                self.selection.cohort.definition,
            )?;
            total_rows = total_rows
                .checked_add(observation.rows.len())
                .filter(|count| *count <= MAX_ITEM_FILE_ROWS)
                .ok_or(DurableError::Refused(
                    "Item Edition lookup aggregate row bound exceeded",
                ))?;
            result.insert(edition_id.clone(), observation);
        }
        active(deadline, cancelled)?;
        tx.commit()?;
        Self::charge_work(
            &mut self.work.item_edition_lookups,
            edition_ids.len() as u64,
        )?;
        for observation in result.values() {
            self.record_predicate(
                "range",
                crate::source_item_file_index::EDITION_PREDICATE_SCOPE,
                &observation.predicate_token,
                observation.generation,
            );
        }
        Ok(result)
    }

    /// Resolve selected public semantic-annotation entity IDs through their
    /// dedicated multivalued owner index. Each token binds a unique-generation
    /// predicate even when no packet currently declares it; returned packets
    /// still need exact member reads and owner validation by the caller.
    pub(crate) fn lookup_annotation_identity_owners(
        &mut self,
        tokens: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<
        BTreeMap<String, BTreeSet<crate::source_annotation_identity_index::AnnotationEntityOwner>>,
    > {
        if tokens.is_empty() {
            return Ok(BTreeMap::new());
        }
        if tokens.len() > MAX_ITEM_FILE_LOOKUPS
            || tokens
                .iter()
                .any(|token| token.is_empty() || token.len() > 4096)
        {
            return Err(DurableError::Refused(
                "annotation identity point token bound",
            ));
        }
        active(deadline, cancelled)?;
        let domain = self.selection.cohort.domain.as_str();
        let owners = {
            let mut tx = self.coordinator.client.transaction()?;
            tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
            let fence = tx.query_one(
                "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
                &[&domain],
            )?;
            if fence.get::<_, String>(1) != "normal"
                || as_u64(fence.get(0))? != self.selection.audit_generation
            {
                return Err(DurableError::Conflict(
                    "annotation identity audit fence changed",
                ));
            }
            let gate = tx.query_one(
                "SELECT rights_allowed,source_complete,source_generation,source_epoch,
                        contract_digest,source_definition_digest
                 FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&domain],
            )?;
            let definition = gate
                .get::<_, Option<String>>(5)
                .ok_or(DurableError::Corrupt("source definition absent"))?;
            super::require_current_source_definition(parse_hex(definition)?)?;
            if !gate.get::<_, bool>(0)
                || !gate.get::<_, bool>(1)
                || gate.get::<_, Option<i64>>(2) != Some(as_i64(self.selection.source_generation)?)
                || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.epoch)?)
                || gate.get::<_, String>(4) != self.selection.contract.to_hex()
                || gate.get::<_, Option<String>>(5)
                    != Some(self.selection.cohort.definition.to_hex())
            {
                return Err(DurableError::Conflict(
                    "annotation identity source epoch changed",
                ));
            }
            let mut owners = BTreeMap::new();
            let mut total = 0usize;
            for token in tokens {
                let found = crate::source_annotation_identity_index::lookup_current_owners(
                    &mut tx,
                    domain,
                    token,
                    self.selection.cohort.definition,
                )?;
                total = total
                    .checked_add(found.len())
                    .filter(|count| *count <= MAX_ITEM_FILE_ROWS)
                    .ok_or(DurableError::Refused(
                        "annotation identity point owner work bound",
                    ))?;
                owners.insert(token.clone(), found);
            }
            tx.commit()?;
            owners
        };
        Self::charge_work(&mut self.work.item_file_lookups, tokens.len() as u64)?;
        for token in tokens {
            self.observe_predicate_generation(
                PredicateKind::Unique,
                crate::source_annotation_identity_index::PREDICATE_SCOPE,
                token,
                deadline,
                cancelled,
            )?;
        }
        Ok(owners)
    }

    /// Bind the semantic dependency commitment to its own invalidation
    /// predicate. This is intentionally separate from the legacy complete
    /// `source-inventory/all` range used by cold creation.
    pub(crate) fn observe_dependency_commitment(
        &mut self,
        version: u16,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<u64> {
        let scope = match version {
            1 => "source-dependencies-v1",
            2 => "source-dependencies-v2",
            _ => {
                return Err(DurableError::Refused(
                    "bounded source dependency commitment version",
                ));
            }
        };
        self.observe_predicate_generation(
            PredicateKind::Range,
            scope,
            &version.to_string(),
            deadline,
            cancelled,
        )
    }

    fn record_exact_metadata(
        &mut self,
        key: &str,
        custody_revision: u64,
        content_digest: Digest256,
        locator: Digest256,
    ) -> DurableResult<()> {
        if let Some(read) = self.reads.reads.iter().find_map(|read| match read {
            PredicateRead::Exact {
                namespace,
                key: prior,
                expected_version,
                expected_digest,
            } if namespace == "source" && prior == key => Some((expected_version, expected_digest)),
            _ => None,
        }) {
            if read.0 != &Some(custody_revision) || read.1 != &Some(content_digest) {
                return Err(DurableError::Conflict(
                    "bounded source exact read metadata changed",
                ));
            }
        } else {
            self.reads.reads.push(PredicateRead::Exact {
                namespace: "source".to_owned(),
                key: key.to_owned(),
                expected_version: Some(custody_revision),
                expected_digest: Some(content_digest),
            });
        }
        if self
            .reads
            .locators
            .insert(key.to_owned(), locator)
            .is_some_and(|prior| prior != locator)
        {
            return Err(DurableError::Conflict(
                "bounded source exact locator changed",
            ));
        }
        Ok(())
    }

    fn record_exact(
        &mut self,
        member: ManagedCurrentMember,
        locator: Digest256,
    ) -> DurableResult<()> {
        let key = member.path.as_str().to_owned();
        self.record_exact_metadata(
            &key,
            member.custody_revision,
            member.metadata.sha256,
            locator,
        )?;
        self.observations
            .insert(key, SourceRevisionObservation { member, locator });
        Ok(())
    }

    fn record_absent(&mut self, path: &str) {
        if self.reads.reads.iter().any(|read| {
            matches!(read, PredicateRead::Absent { namespace, key } if namespace == "source" && key == path)
        }) {
            return;
        }
        self.reads.reads.push(PredicateRead::Absent {
            namespace: "source".to_owned(),
            key: path.to_owned(),
        });
    }

    fn record_predicate(&mut self, kind: &str, scope: &str, token: &str, generation: u64) {
        let key = (kind.to_owned(), scope.to_owned(), token.to_owned());
        if !self.predicate_reads.insert(key.clone()) {
            return;
        }
        self.reads.reads.push(PredicateRead::Generation {
            predicate: PredicateToken {
                kind: if kind == "unique" {
                    PredicateKind::Unique
                } else {
                    PredicateKind::Range
                },
                owner: OWNER.to_owned(),
                scope: scope.to_owned(),
                token: token.to_owned(),
                definition_version: self.selection.cohort.definition.to_hex(),
            },
            observed_generation: generation,
        });
    }

    fn require_stable_predicate_generation(
        &self,
        kind: &str,
        scope: &str,
        token: &str,
        generation: u64,
    ) -> DurableResult<()> {
        if self.reads.reads.iter().any(|read| {
            matches!(
                read,
                PredicateRead::Generation {
                    predicate,
                    observed_generation,
                } if predicate.kind.as_str() == kind
                    && predicate.owner == OWNER
                    && predicate.scope == scope
                    && predicate.token == token
                    && *observed_generation != generation
            )
        }) {
            return Err(DurableError::Conflict(
                "bounded source predicate generation changed within read session",
            ));
        }
        Ok(())
    }

    /// Recheck this actual borrowed PG read epoch. It is not a continuous SQL
    /// transaction or a substitute for the original caller's query admission.
    /// Native held callers may borrow this primitive; no readset is finalized
    /// or manufactured here. Cutoff refusal can precede the physical queries.
    pub(crate) fn verify_fresh(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<()> {
        active(deadline, cancelled)?;
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        let row = tx.query_one(
            "SELECT source_definition_digest,rights_allowed,source_complete,
                    rights_version,rule_version,head_seq,source_generation,
                    source_epoch,contract_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        super::require_current_source_definition(parse_hex(
            row.get::<_, Option<String>>("source_definition_digest")
                .ok_or(DurableError::Corrupt("source definition absent"))?,
        )?)?;
        if fence.get::<_, String>(1) != "normal"
            || !row.get::<_, bool>("rights_allowed")
            || !row.get::<_, bool>("source_complete")
            || as_u64(row.get("rights_version"))? != self.selection.rights_version
            || as_u64(row.get("rule_version"))? != self.selection.rule_version
            || as_u64(fence.get(0))? != self.selection.audit_generation
            || as_u64(row.get("head_seq"))? != self.selection.head_seq
            || as_u64(
                row.get::<_, Option<i64>>("source_generation")
                    .ok_or(DurableError::Corrupt("source generation absent"))?,
            )? != self.selection.source_generation
            || as_u64(
                row.get::<_, Option<i64>>("source_epoch")
                    .ok_or(DurableError::Corrupt("source epoch absent"))?,
            )? != self.selection.epoch
            || row.get::<_, String>("contract_digest") != self.selection.contract.to_hex()
            || row.get::<_, Option<String>>("source_definition_digest")
                != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "bounded source revision read epoch changed",
            ));
        }
        let job = tx.query_one(
            "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
            &[&self.selection.cohort.domain, &self.selection.job_id],
        )?;
        if as_u64(job.get(0))? != self.selection.job_fence {
            return Err(DurableError::Conflict(
                "bounded source revision job lease changed",
            ));
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn read_current_member(
        &mut self,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedCurrentMember> {
        active(deadline, cancelled)?;
        if !path.as_str().starts_with("ToS/") {
            return Err(DurableError::Refused(
                "bounded source revision path outside authored namespace",
            ));
        }
        let (row, raw, placement, work) = self.coordinator.read_committed_source_selected(
            self.store,
            &self.selection.cohort.domain,
            path.as_str(),
            None,
            max_bytes,
            deadline,
            cancelled,
            |row| cohort_matches(row, &self.selection.cohort, true),
        )?;
        let (metadata, dependency_claims) = expose_metadata(&row, path)?;
        let locator = metadata_locator_digest(&row);
        let member = ManagedCurrentMember {
            path: path.clone(),
            custody_revision: as_u64(row.get("revision"))?,
            current_generation: self.selection.head_seq,
            commit_seq: as_u64(row.get("commit_seq"))?,
            raw,
            metadata,
            dependency_claims,
            placement,
        };
        Self::charge_work(&mut self.work.exact_source_members_read, 1)?;
        Self::charge_work(
            &mut self.work.exact_source_bytes_read,
            member.raw.len() as u64,
        )?;
        Self::charge_work(
            &mut self.work.custody_metadata_bytes_read,
            work.custody_metadata_bytes_read,
        )?;
        Self::charge_work(&mut self.work.segment_bytes_read, work.segment_bytes_read)?;
        Self::charge_work(
            &mut self.work.committed_member_rows_examined,
            work.committed_member_rows_examined,
        )?;
        self.record_exact(member, locator)?;
        Ok(self
            .observations
            .get(path.as_str())
            .map(|observation| observation.member.clone())
            .ok_or(DurableError::Corrupt("source revision observation absent"))?)
    }

    pub(crate) fn list_immediate_children(
        &mut self,
        parent: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Vec<(String, bool)>> {
        active(deadline, cancelled)?;
        let parent = parent.as_str();
        // Global Claims grounding can require the complete current authored
        // inventory. Permit only this exact root spelling as the first
        // recursive range read; descendants remain under `ToS/` and each
        // returned directory gets its own source-home generation predicate.
        if parent != "ToS" && !parent.starts_with("ToS/") {
            return Err(DurableError::Refused(
                "bounded source revision directory outside authored namespace",
            ));
        }
        let prefix = format!("{parent}/");
        // The source subject index is byte-ordered under C collation. A
        // half-open prefix range keeps this immediate-child read bounded and
        // avoids a LIKE pattern scan over the whole corpus.
        let upper = format!("{parent}0");
        let mut tx = self.coordinator.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence
             WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0))? != self.selection.audit_generation
        {
            return Err(DurableError::Conflict(
                "bounded source directory audit fence changed",
            ));
        }
        let gate = tx.query_one(
            "SELECT rights_allowed,source_complete,source_generation,source_epoch,contract_digest,
                    source_definition_digest
             FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&self.selection.cohort.domain],
        )?;
        super::require_current_source_definition(parse_hex(
            gate.get::<_, Option<String>>(5)
                .ok_or(DurableError::Corrupt("source definition absent"))?,
        )?)?;
        if !gate.get::<_, bool>(0)
            || !gate.get::<_, bool>(1)
            || gate.get::<_, Option<i64>>(2) != Some(as_i64(self.selection.source_generation)?)
            || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.epoch)?)
            || gate.get::<_, String>(4) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(5) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "bounded source directory read epoch changed",
            ));
        }
        let predicate = tx.query_opt(
            "SELECT generation,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind='range' AND owner=$2 AND scope='source-home' AND token=$3",
            &[&self.selection.cohort.domain, &OWNER, &parent],
        )?;
        let generation = predicate
            .as_ref()
            .map(|row| as_u64(row.get(0)))
            .transpose()?
            .unwrap_or(0);
        if predicate
            .as_ref()
            .is_some_and(|row| row.get::<_, String>(1) != self.selection.cohort.definition.to_hex())
        {
            return Err(DurableError::Conflict(
                "source directory predicate definition differs",
            ));
        }
        let mut children = BTreeMap::new();
        let mut lower = prefix.clone();
        let mut inclusive = true;
        loop {
            active(deadline, cancelled)?;
            let row = tx.query_opt(
                "SELECT subject FROM cmd2_current
                 WHERE domain=$1
                   AND subject COLLATE \"C\" >= $2 COLLATE \"C\"
                   AND ($4 OR subject COLLATE \"C\" > $2 COLLATE \"C\")
                   AND subject COLLATE \"C\" < $3 COLLATE \"C\"
                 ORDER BY subject COLLATE \"C\" LIMIT 1",
                &[&self.selection.cohort.domain, &lower, &upper, &inclusive],
            )?;
            let Some(row) = row else { break };
            Self::charge_work(&mut self.work.directory_index_rows_examined, 1)?;
            let subject: String = row.get("subject");
            let rest = subject
                .strip_prefix(&prefix)
                .ok_or(DurableError::Corrupt("source child prefix differs"))?;
            let (name, directory) = match rest.split_once('/') {
                Some((name, _)) => (name, true),
                None => (rest, false),
            };
            if name.is_empty() {
                return Err(DurableError::Corrupt("source child name empty"));
            }
            if children.contains_key(name) {
                return Err(DurableError::Corrupt(
                    "source child file and directory collide",
                ));
            }
            if children.len() == MAX_CHILDREN {
                return Err(DurableError::Refused(
                    "source revision immediate child budget exceeded",
                ));
            }
            let name = name.to_owned();
            children.insert(name.clone(), directory);
            if directory {
                // Under the source path grammar, the slash that begins a
                // descendant sorts before `0` in C order.  Advancing to this
                // sibling bound skips the whole immediate directory in one
                // indexed seek; a later same-name row is rejected above.
                lower = format!("{prefix}{name}0");
                inclusive = true;
            } else {
                // A file is itself the key.  Advancing to `name0` would lose
                // valid prefixed siblings such as `foo.bar`; strict `>` keeps
                // the next key visible without a descendant probe.
                lower = subject;
                inclusive = false;
            }
        }
        tx.commit()?;
        self.record_predicate("range", "source-home", parent, generation);
        Ok(children.into_iter().collect())
    }
}

impl SourceRevisionReadSession<'_> {
    fn read_selected_import_publication_member(
        &mut self,
        row: &SelectedSourcePublicationRow,
        receipt_bytes: &[u8],
        requested_path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Vec<u8>> {
        let package = row
            .package_bytes
            .as_deref()
            .ok_or(DurableError::Corrupt("source publication package absent"))?;
        let package_digest = row.package_digest.as_deref().ok_or(DurableError::Corrupt(
            "source publication package digest absent",
        ))?;
        let (receipt_binding, receipt_digest) =
            crate::source_work_transaction::parse_legacy_selected_publication_import_receipt(
                receipt_bytes,
            )
            .map_err(DurableError::Source)?;
        let import_event_id = row.import_event_id.as_deref().ok_or(DurableError::Corrupt(
            "source publication import event absent",
        ))?;
        let import_transition_index = row.import_transition_index.ok_or(DurableError::Corrupt(
            "source publication import transition index absent",
        ))?;
        let (package_binding, transition_index, refs) =
            crate::source_work_transaction::legacy_selected_publication_import_package_refs(
                package,
            )
            .map_err(DurableError::Source)?;
        if package_binding != receipt_binding
            || package_binding.import_event_id != import_event_id
            || transition_index != import_transition_index
            || package_binding.receipt_sha256 != receipt_digest
            || Digest256::of_bytes(package)
                != Digest256::from_hex(package_digest).map_err(|_| {
                    DurableError::Corrupt("source publication package digest invalid")
                })?
        {
            return Err(DurableError::Conflict(
                "source publication import package binding differs",
            ));
        }
        if refs
            .iter()
            .filter(|reference| reference.role == "selected_transaction_blob")
            .count()
            > MAX_SOURCE_PUBLICATION_IMPORT_BLOB_MEMBERS
        {
            return Err(DurableError::Refused(
                "source publication import blob member budget exceeded",
            ));
        }
        let mut blobs = BTreeMap::new();
        let mut blob_bytes_read = 0u64;
        for reference in refs
            .iter()
            .filter(|reference| reference.role == "selected_transaction_blob")
        {
            active(deadline, cancelled)?;
            let path = RelativePath::parse(&reference.path)
                .map_err(|_| DurableError::Corrupt("source publication import blob path"))?;
            let member = self.read_current_member(
                &path,
                MAX_SOURCE_PUBLICATION_BLOB_BYTES,
                deadline,
                cancelled,
            )?;
            let expected = reference.after.as_ref().ok_or(DurableError::Corrupt(
                "source publication import blob binding absent",
            ))?;
            if reference.before.is_some()
                || member.metadata.sha256 != expected.sha256
                || member.raw.len() as u64 != expected.byte_size
            {
                return Err(DurableError::Conflict(
                    "source publication import blob differs from selected source",
                ));
            }
            self.retained_state_bytes = self
                .retained_state_bytes
                .checked_add(path.as_str().len().saturating_mul(4) + member.raw.len())
                .filter(|bytes| *bytes <= MAX_RETAINED_STATE_BYTES)
                .ok_or(DurableError::Refused(
                    "source publication import retained-state budget exceeded",
                ))?;
            blob_bytes_read = blob_bytes_read
                .checked_add(member.raw.len() as u64)
                .filter(|bytes| *bytes <= MAX_SOURCE_PUBLICATION_PACKAGE_BYTES_READ)
                .ok_or(DurableError::Refused(
                    "source publication import blob byte budget exceeded",
                ))?;
            if blobs.insert(reference.path.clone(), member.raw).is_some() {
                return Err(DurableError::Corrupt(
                    "source publication import blob path duplicated",
                ));
            }
        }
        let retained_writes = crate::source_work_transaction::
            legacy_selected_publication_import_retained_writes_from_blobs(package, &blobs)
                .map_err(DurableError::Source)?;
        let parsed =
            crate::source_work_transaction::parse_legacy_selected_publication_import_package(
                package,
                &retained_writes,
            )
            .map_err(DurableError::Source)?;
        if parsed.canonical_bytes != package
            || parsed.sha256.to_hex() != package_digest
            || parsed.origin != receipt_binding
            || parsed.transition_index != import_transition_index
            || parsed.transaction_id != row.transaction_id.as_deref().unwrap_or_default()
            || parsed.generation != row.publication_generation
            || parsed.token != row.token.as_deref().unwrap_or_default()
            || parsed.base_publication_generation != row.base_generation
            || parsed.base_publication_token != row.base_token
        {
            return Err(DurableError::Conflict(
                "source publication import transition differs from indexed row",
            ));
        }
        if blob_bytes_read > MAX_SOURCE_PUBLICATION_PACKAGE_BYTES_READ {
            return Err(DurableError::Refused(
                "source publication import blob byte budget exceeded",
            ));
        }
        let value = cmd::parse(package).map_err(DurableError::Source)?;
        let (field, expected_digest) = if requested_path == SOURCE_PUBLICATION_CONTROL_PATH {
            ("control", parsed.control_sha256)
        } else {
            let requested_transaction = selected_publication_transaction_id(requested_path)?;
            if parsed.transaction_id != requested_transaction {
                return Err(DurableError::Conflict(
                    "source publication import journal transaction differs",
                ));
            }
            if requested_path.ends_with("/manifest.json") {
                ("manifest", parsed.manifest_sha256)
            } else {
                ("completion", parsed.completion_sha256)
            }
        };
        crate::source_work_transaction::legacy_selected_publication_sidecar(
            &value,
            field,
            expected_digest,
        )
        .map_err(DurableError::Source)
    }

    fn read_selected_publication_virtual_member(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<Vec<u8>>> {
        let is_control = crate::source_work_transaction::is_selected_publication_control_path(path);
        let is_journal =
            crate::source_work_transaction::is_selected_publication_journal_metadata_path(path);
        if !is_control && !is_journal {
            return Err(DurableError::Refused(
                "source publication virtual path is outside the selected surface",
            ));
        }
        let requested_transaction = is_journal
            .then(|| selected_publication_transaction_id(path))
            .transpose()?;
        if self.work.source_publication_virtual_lookups >= MAX_SOURCE_PUBLICATION_VIRTUAL_LOOKUPS {
            return Err(DurableError::Refused(
                "source publication virtual lookup budget exceeded",
            ));
        }
        active(deadline, cancelled)?;

        let (selected, selected_import_receipt, generation, package_bytes_read) = {
            let mut tx = self.coordinator.client.transaction()?;
            tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
            let fence = tx.query_one(
                "SELECT generation,maintenance_state FROM cmd2_audit_fence
                 WHERE domain=$1 FOR SHARE",
                &[&self.selection.cohort.domain],
            )?;
            if fence.get::<_, String>(1) != "normal"
                || as_u64(fence.get(0))? != self.selection.audit_generation
            {
                return Err(DurableError::Conflict(
                    "source publication audit fence changed",
                ));
            }
            let gate = tx.query_one(
                "SELECT rights_allowed,rights_version,rule_version,source_complete,
                        source_generation,source_epoch,head_seq,contract_digest,
                        source_definition_digest
                 FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&self.selection.cohort.domain],
            )?;
            super::require_current_source_definition(parse_hex(
                gate.get::<_, Option<String>>(8)
                    .ok_or(DurableError::Corrupt("source definition absent"))?,
            )?)?;
            if !gate.get::<_, bool>(0)
                || as_u64(gate.get(1))? != self.selection.rights_version
                || as_u64(gate.get(2))? != self.selection.rule_version
                || !gate.get::<_, bool>(3)
                || gate.get::<_, Option<i64>>(4) != Some(as_i64(self.selection.source_generation)?)
                || gate.get::<_, Option<i64>>(5) != Some(as_i64(self.selection.epoch)?)
                || as_u64(gate.get(6))? != self.selection.head_seq
                || gate.get::<_, String>(7) != self.selection.contract.to_hex()
                || gate.get::<_, Option<String>>(8)
                    != Some(self.selection.cohort.definition.to_hex())
            {
                return Err(DurableError::Conflict(
                    "source publication read epoch changed",
                ));
            }

            let current = tx
                .query_opt(
                    &format!(
                        "SELECT {SOURCE_PUBLICATION_ROW_FIELDS}
                         FROM cmd2_source_publication_current WHERE domain=$1 FOR SHARE"
                    ),
                    &[&self.selection.cohort.domain],
                )?
                .as_ref()
                .map(SelectedSourcePublicationRow::from_row)
                .transpose()?
                .ok_or(DurableError::Conflict(
                    "source publication current pointer is unavailable",
                ))?;
            current.verify_selection(&self.selection)?;
            if current.origin_kind == "legacy_sto_import" {
                let (binding, _) =
                    selected_publication_import_receipt(&mut tx, &self.selection, &current)?;
                if current.publication_generation != binding.head_generation
                    || current.token.as_deref() != Some(binding.head_token.as_str())
                    || current.import_transition_index != Some(binding.transition_count)
                {
                    return Err(DurableError::Conflict(
                        "source publication current pointer is not the imported head",
                    ));
                }
            }
            let predicate = tx.query_opt(
                "SELECT generation,complete,definition_version FROM cmd2_predicate
                 WHERE domain=$1 AND kind='unique' AND owner=$2
                   AND scope='source-publication' AND token='current' FOR SHARE",
                &[&self.selection.cohort.domain, &OWNER],
            )?;
            let predicate = predicate.ok_or(DurableError::Conflict(
                "source publication current predicate is absent",
            ))?;
            let generation = as_u64(predicate.get("generation"))?;
            if !predicate.get::<_, bool>("complete")
                || predicate.get::<_, String>("definition_version")
                    != self.selection.cohort.definition.to_hex()
                || generation != current.publication_generation
            {
                return Err(DurableError::Conflict(
                    "source publication current predicate generation differs",
                ));
            }

            let current_history = tx
                .query_opt(
                    &format!(
                        "SELECT {SOURCE_PUBLICATION_ROW_FIELDS}
                         FROM cmd2_source_publication_history
                         WHERE domain=$1 AND publication_generation=$2 FOR SHARE"
                    ),
                    &[
                        &self.selection.cohort.domain,
                        &as_i64(current.publication_generation)?,
                    ],
                )?
                .as_ref()
                .map(SelectedSourcePublicationRow::from_row)
                .transpose()?
                .ok_or(DurableError::Conflict(
                    "source publication current history row is absent",
                ))?;
            if current_history != current {
                return Err(DurableError::Conflict(
                    "source publication current and history rows differ",
                ));
            }
            verify_selected_publication_parent(&mut tx, &self.selection, &current)?;

            let selected = if is_control {
                if current.state == "absent" {
                    None
                } else {
                    Some(current.clone())
                }
            } else {
                let transaction_id =
                    requested_transaction
                        .as_deref()
                        .ok_or(DurableError::Corrupt(
                            "source publication journal transaction absent",
                        ))?;
                let rows = tx.query(
                    &format!(
                        "SELECT {SOURCE_PUBLICATION_ROW_FIELDS}
                         FROM cmd2_source_publication_history
                         WHERE domain=$1 AND transaction_id=$2
                         ORDER BY publication_generation DESC LIMIT 2 FOR SHARE"
                    ),
                    &[&self.selection.cohort.domain, &transaction_id],
                )?;
                if rows.len() > 1 {
                    return Err(DurableError::Conflict(
                        "source publication transaction history is ambiguous",
                    ));
                }
                rows.first()
                    .map(SelectedSourcePublicationRow::from_row)
                    .transpose()?
            };

            let selected = match selected {
                Some(selected) => {
                    selected.verify_selection(&self.selection)?;
                    if selected.state != "present"
                        || selected.publication_generation > current.publication_generation
                        || requested_transaction
                            .as_deref()
                            .is_some_and(|transaction_id| {
                                selected.transaction_id.as_deref() != Some(transaction_id)
                            })
                    {
                        return Err(DurableError::Conflict(
                            "source publication selected history lineage differs",
                        ));
                    }
                    if selected.origin_kind == "legacy_sto_import" {
                        selected_publication_import_receipt(&mut tx, &self.selection, &selected)?;
                    }
                    verify_selected_publication_parent(&mut tx, &self.selection, &selected)?;
                    Some(selected)
                }
                None => None,
            };
            let selected_import_receipt = selected
                .as_ref()
                .filter(|selected| selected.origin_kind == "legacy_sto_import")
                .map(|selected| {
                    selected_publication_import_receipt(&mut tx, &self.selection, selected)
                        .map(|(_, bytes)| bytes)
                })
                .transpose()?;
            let package_bytes_read = selected
                .as_ref()
                .and_then(|selected| selected.package_bytes.as_ref())
                .map_or(0, |bytes| bytes.len() as u64)
                .checked_add(
                    selected_import_receipt
                        .as_ref()
                        .map_or(0, |bytes| bytes.len() as u64),
                )
                .ok_or(DurableError::Refused(
                    "source publication package read budget exceeded",
                ))?;
            if self
                .work
                .source_publication_package_bytes_read
                .checked_add(package_bytes_read)
                .filter(|bytes| *bytes <= MAX_SOURCE_PUBLICATION_PACKAGE_BYTES_READ)
                .is_none()
            {
                return Err(DurableError::Refused(
                    "source publication package read budget exceeded",
                ));
            }
            active(deadline, cancelled)?;
            tx.commit()?;
            (
                selected,
                selected_import_receipt,
                generation,
                package_bytes_read,
            )
        };

        let member = match selected.as_ref() {
            Some(selected) if selected.origin_kind == "legacy_sto_import" => Some(
                self.read_selected_import_publication_member(
                    selected,
                    selected_import_receipt
                        .as_deref()
                        .ok_or(DurableError::Corrupt(
                            "source publication import receipt bytes absent",
                        ))?,
                    path,
                    deadline,
                    cancelled,
                )?,
            ),
            Some(selected) => Some(selected_publication_package_member(selected, path)?),
            None => None,
        };

        self.require_stable_predicate_generation(
            "unique",
            "source-publication",
            "current",
            generation,
        )?;
        self.record_predicate("unique", "source-publication", "current", generation);
        Self::charge_work(&mut self.work.source_publication_virtual_lookups, 1)?;
        Self::charge_work(
            &mut self.work.source_publication_package_bytes_read,
            package_bytes_read,
        )?;
        Ok(member)
    }
}

impl ReadonlyRecordFiles for SourceRevisionReadSession<'_> {
    fn read(
        &mut self,
        name: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let path = RelativePath::parse(name)
            .map_err(|_| SourceCommandError::Invalid("bounded source member path"))?;
        if crate::source_work_transaction::is_selected_publication_control_path(path.as_str())
            || crate::source_work_transaction::is_selected_publication_journal_metadata_path(
                path.as_str(),
            )
        {
            let raw = self
                .read_selected_publication_virtual_member(path.as_str(), deadline, cancelled)
                .map_err(source_command_error)?
                .ok_or(SourceCommandError::Conflict("bounded source member absent"))?;
            if raw.len() > max_bytes {
                return Err(SourceCommandError::Unsupported(
                    "bounded source revision member budget",
                ));
            }
            self.retained_state_bytes = self
                .retained_state_bytes
                .checked_add(path.as_str().len().saturating_mul(4) + raw.len())
                .filter(|bytes| *bytes <= MAX_RETAINED_STATE_BYTES)
                .ok_or(SourceCommandError::Unsupported(
                    "bounded source revision state budget",
                ))?;
            return Ok(raw);
        }
        match self.read_current_member(&path, max_bytes as u64, deadline, cancelled) {
            Ok(member) => {
                self.retained_state_bytes = self
                    .retained_state_bytes
                    .checked_add(name.len().saturating_mul(4) + member.raw.len())
                    .filter(|bytes| *bytes <= MAX_RETAINED_STATE_BYTES)
                    .ok_or(SourceCommandError::Unsupported(
                        "bounded source revision state budget",
                    ))?;
                Ok(member.raw)
            }
            Err(DurableError::Conflict("current source member absent")) => {
                self.record_absent(name);
                Err(SourceCommandError::Conflict("bounded source member absent"))
            }
            Err(error) => Err(source_command_error(error)),
        }
    }

    fn list_directory(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>> {
        let path = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("bounded source directory path"))?;
        self.list_immediate_children(&path, deadline, cancelled)
            .map_err(source_command_error)
    }

    fn has_file(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<bool> {
        let path = RelativePath::parse(path)
            .map_err(|_| SourceCommandError::Invalid("bounded source member path"))?;
        if !path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Denied(
                "bounded source revision path outside authored namespace",
            ));
        }
        if crate::source_work_transaction::is_selected_publication_control_path(path.as_str())
            || crate::source_work_transaction::is_selected_publication_journal_metadata_path(
                path.as_str(),
            )
        {
            return self
                .read_selected_publication_virtual_member(path.as_str(), deadline, cancelled)
                .map(|member| member.is_some())
                .map_err(source_command_error);
        }
        active(deadline, cancelled).map_err(source_command_error)?;
        let mut tx = self
            .coordinator
            .client
            .transaction()
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        let fence = tx
            .query_one(
                "SELECT generation,maintenance_state FROM cmd2_audit_fence
                 WHERE domain=$1 FOR SHARE",
                &[&self.selection.cohort.domain],
            )
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        if fence.get::<_, String>(1) != "normal"
            || as_u64(fence.get(0)).map_err(source_command_error)?
                != self.selection.audit_generation
        {
            return Err(SourceCommandError::Conflict(
                "bounded source presence audit fence changed",
            ));
        }
        let gate = tx
            .query_one(
                "SELECT rights_allowed,source_complete,source_generation,source_epoch,
                        contract_digest,source_definition_digest
                 FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                &[&self.selection.cohort.domain],
            )
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        if !gate.get::<_, bool>(0)
            || !gate.get::<_, bool>(1)
            || gate.get::<_, Option<i64>>(2)
                != Some(as_i64(self.selection.source_generation).map_err(source_command_error)?)
            || gate.get::<_, Option<i64>>(3)
                != Some(as_i64(self.selection.epoch).map_err(source_command_error)?)
            || gate.get::<_, String>(4) != self.selection.contract.to_hex()
            || gate.get::<_, Option<String>>(5) != Some(self.selection.cohort.definition.to_hex())
        {
            return Err(SourceCommandError::Conflict(
                "bounded source presence read epoch changed",
            ));
        }
        let row = tx
            .query_opt(
                "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=$2",
                &[&self.selection.cohort.domain, &path.as_str()],
            )
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        Self::charge_work(&mut self.work.metadata_member_lookups, 1)
            .map_err(source_command_error)?;
        let Some(row) = row else {
            tx.commit()
                .map_err(|error| source_command_error(DurableError::Database(error)))?;
            self.record_absent(path.as_str());
            return Ok(false);
        };
        let custody_revision = as_u64(row.get("revision")).map_err(source_command_error)?;
        let content_digest =
            parse_hex(row.get::<_, String>("content_digest")).map_err(source_command_error)?;
        let locator = metadata_locator_digest(&row);
        tx.commit()
            .map_err(|error| source_command_error(DurableError::Database(error)))?;
        self.record_exact_metadata(path.as_str(), custody_revision, content_digest, locator)
            .map_err(source_command_error)?;
        Ok(true)
    }

    fn set_retained_state_bytes(
        &mut self,
        bytes: usize,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if bytes > MAX_RETAINED_STATE_BYTES {
            return Err(SourceCommandError::Unsupported(
                "bounded source revision state budget",
            ));
        }
        self.retained_state_bytes = bytes;
        Ok(())
    }

    fn retained_state_bytes(&self) -> usize {
        self.retained_state_bytes
    }
}

/// Run the real owner exit once on both body outcomes, preserving the original
/// refusal if the exit also fails. The caller retains the actual session until
/// this returns, and can finalize its readset only on success.
fn finish_source_revision_epoch<T>(
    body: DurableResult<T>,
    fence: impl FnOnce() -> DurableResult<()>,
) -> DurableResult<T> {
    let fenced = fence();
    match (body, fenced) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

#[cfg(test)]
mod epoch_exit_tests {
    use super::*;

    #[test]
    fn refused_body_attempts_exit_and_keeps_first_error() {
        let mut attempts = 0;
        let result: DurableResult<()> = finish_source_revision_epoch(
            Err(DurableError::Refused("body refused")),
            || {
                attempts += 1;
                Err(DurableError::Conflict("epoch changed"))
            },
        );
        assert_eq!(attempts, 1);
        assert!(matches!(result, Err(DurableError::Refused("body refused"))));

        let result = finish_source_revision_epoch(Ok(7), || {
            attempts += 1;
            Err(DurableError::Conflict("epoch changed"))
        });
        assert_eq!(attempts, 2);
        assert!(matches!(result, Err(DurableError::Conflict("epoch changed"))));

        assert_eq!(finish_source_revision_epoch(Ok(7), || Ok(())).unwrap(), 7);
    }
}

fn source_command_error(error: DurableError) -> SourceCommandError {
    match error {
        DurableError::Source(error) => error,
        DurableError::Conflict(_) => {
            SourceCommandError::Conflict("bounded source revision read changed or is unavailable")
        }
        // Refusal labels are static owner-authored diagnostics. Retain them so
        // bounded selected reads can identify the exact pre-commit guard
        // without exposing source, request, configuration, or database data.
        DurableError::Refused(reason) => SourceCommandError::Denied(reason),
        DurableError::Invalid(_) | DurableError::Corrupt(_) => {
            SourceCommandError::Invalid("bounded source revision read metadata")
        }
        DurableError::Database(_) | DurableError::Storage(_) | DurableError::Indeterminate(_) => {
            SourceCommandError::Conflict("bounded source revision read custody unavailable")
        }
    }
}
