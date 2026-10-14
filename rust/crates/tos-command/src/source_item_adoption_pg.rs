//! PG owner projection for the maintained Edition -> Item package.
//!
//! The projection is derived only from the complete output buffers already
//! produced by the native EditionItem owner. It adds enough exact custody to
//! the Item Claim stream for the mixed source inventory; it does not replace
//! the native receipt or authorize semantic admission.
use crate::source_cohort::revision::SourceRevisionReadSession;
use crate::source_cohort::{IndexRows, SourceRevisionWrite};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::CreationFilesystem;
use crate::source_work_transaction::{
    PublicationSnapshot, SelectedPublicationBinding, SelectedPublicationSourceRef,
    WorkArchivePackage,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_validation::item_rules::{ItemLimits, ItemRefusal};
use tos_validation::source_cut::CutSchemaExecutor;

pub(crate) const PG_ITEM_ADOPTION_FAMILY_TAG: &str =
    "tos-pg-item-adoption-v1:tos_local_item_adoption_owner_v1:item.adopt";

/// The exact current Item closure needed by the maintained owner. This is
/// deliberately keyed from the Edition and its reverse Claim predicates; it
/// does not enumerate unrelated metadata records or Item packages.
pub(crate) struct PgItemCurrentClosure {
    pub(crate) before: BTreeMap<String, Vec<u8>>,
    pub(crate) related_items: BTreeMap<String, BTreeMap<String, Vec<u8>>>,
    pub(crate) claims: BTreeMap<String, JsonValue>,
    pub(crate) claim_paths: BTreeMap<String, String>,
    pub(crate) catalog: super::ItemCatalog,
    pub(crate) new_directories: Vec<RelativePath>,
}

const ITEM_CLOSURE_MAX_BYTES: usize = 33_554_432;
const ITEM_CLOSURE_MAX_MEMBERS: usize = 512;

/// Opaque owner selection produced only after the protected Item grant has
/// been checked against exact current PG identity and reverse Claim closure.
pub(crate) struct PgItemAdoptionSelection {
    owner: super::ItemOwner,
    closure: PgItemCurrentClosure,
    proposal: bool,
}

pub(crate) fn select_pg_item_adoption_owner(
    fs: &CreationFilesystem,
    context: &CommandContext,
    session: &mut SourceRevisionReadSession<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgItemAdoptionSelection> {
    select_pg_item_adoption(fs, context, session, false, deadline, cancelled)
}

pub(crate) fn select_pg_item_adoption_proposal(
    fs: &CreationFilesystem,
    context: &CommandContext,
    session: &mut SourceRevisionReadSession<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgItemAdoptionSelection> {
    select_pg_item_adoption(fs, context, session, true, deadline, cancelled)
}

fn select_pg_item_adoption(
    fs: &CreationFilesystem,
    context: &CommandContext,
    session: &mut SourceRevisionReadSession<'_>,
    proposal: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgItemAdoptionSelection> {
    let owner = if proposal {
        super::ItemOwner::select_proposal(fs, context, deadline, cancelled)?
    } else {
        super::ItemOwner::select(fs, context, deadline, cancelled)?
    };
    let closure = current_closure(session, &owner, proposal, deadline, cancelled)?;
    Ok(PgItemAdoptionSelection {
        owner,
        closure,
        proposal,
    })
}

pub(crate) fn pg_item_adoption_transaction_id(
    fs: &CreationFilesystem,
    context: &CommandContext,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    super::ItemOwner::select(fs, context, deadline, cancelled)?.transaction_id()
}

pub(crate) fn pg_item_adoption_transaction_id_from_request(
    request: &JsonValue,
) -> SourceCommandResult<String> {
    if cmd::text(request, "operation")? != "item.adopt" {
        return Err(SourceCommandError::Unsupported(
            "PG Item adoption transaction operation",
        ));
    }
    super::item_adoption_transaction_id(request)
}

pub(crate) fn pg_item_adoption_response_from_attempt(
    fs: &CreationFilesystem,
    context: &CommandContext,
    attempt: &crate::source_cohort::SourceRevisionAttempt,
    stage: &serde_json::Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<serde_json::Value> {
    let owner = super::ItemOwner::select(fs, context, deadline, cancelled)?;
    let edition_home = owner
        .edition_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("PG Item Edition home"))?
        .0;
    let item_home = owner
        .item_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("PG Item target home"))?
        .0;
    let mut parent = BTreeMap::new();
    let mut child = BTreeMap::new();
    let mut other_writes = Vec::new();
    for write in &attempt.writes {
        let mut selected = false;
        for (home, files) in [(item_home, &mut child), (edition_home, &mut parent)] {
            let prefix = format!("{home}/");
            if let Some(name) = write.subject.strip_prefix(&prefix) {
                if name.contains('/')
                    || files
                        .insert(name.to_owned(), write.exact_bytes.clone())
                        .is_some()
                {
                    return Err(SourceCommandError::Conflict(
                        "PG Item recovery package member repeats",
                    ));
                }
                selected = true;
                break;
            }
        }
        if !selected {
            other_writes.push(write);
        }
    }
    let parent_names = BTreeSet::from([
        "edition.json",
        "edition.human-forms.json",
        "source-revision-history.json",
    ]);
    let claim_forms = format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(owner.claim_id.as_bytes()).to_hex()
    );
    let child_names = BTreeSet::from([
        "item.json",
        "item.human-forms.json",
        "source-claims.jsonl",
        claim_forms.as_str(),
        "source-create-request.json",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
        "edition-item-receipt.json",
        "item.manifest.json",
        "rights.json",
        "resource-inventory.json",
        "fixity.sha256",
        "forensic-report.md",
        "provenance.jsonl",
        "item-deposit-receipt.json",
    ]);
    if parent.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != parent_names.iter().copied().collect::<BTreeSet<_>>()
        || child.keys().map(String::as_str).collect::<BTreeSet<_>>()
            != child_names.iter().copied().collect::<BTreeSet<_>>()
    {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery package shape differs",
        ));
    }
    let receipt = super::finished_receipt(&child)?;
    if cmd::text(&receipt, "transaction_id")? != owner.transaction_id()? {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery receipt transaction differs",
        ));
    }
    let archive_path = cmd::text(&receipt, "parent_archive_ref")?;
    let archive_leaf = archive_path
        .strip_prefix("ToS/source-witnesses/.record-revisions/")
        .ok_or(SourceCommandError::Conflict(
            "PG Item recovery archive path differs",
        ))?;
    let (archive_claim, archive_revision) =
        archive_leaf
            .split_once('-')
            .ok_or(SourceCommandError::Conflict(
                "PG Item recovery archive identity differs",
            ))?;
    if !lower_hex_64(archive_claim) || !lower_hex_64(archive_revision) {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery archive identity differs",
        ));
    }
    let transaction_id = owner.transaction_id()?;
    let publication_package =
        attempt
            .publication_package
            .as_ref()
            .ok_or(SourceCommandError::Conflict(
                "PG Item recovery publication package absent",
            ))?;
    let source_refs = JsonValue::Array(
        publication_package
            .source_refs
            .iter()
            .map(selected_publication_source_ref_value)
            .collect(),
    );
    let publication_value = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_pg_selected_publication_package_v1"),
        ),
        (
            "transaction_id",
            cmd::string(&publication_package.transaction_id),
        ),
        ("control", publication_package.control.clone()),
        ("manifest", publication_package.manifest.clone()),
        ("completion", publication_package.completion.clone()),
        ("source_refs", source_refs),
    ]);
    if publication_package.transaction_id != transaction_id
        || publication_package.canonical_bytes.len() > 1_048_576
        || Digest256::of_bytes(&publication_package.canonical_bytes) != publication_package.sha256
        || cmd::canonical(&publication_value)? != publication_package.canonical_bytes
    {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery publication package identity differs",
        ));
    }
    let mut publication_changes = attempt
        .writes
        .iter()
        .map(|write| {
            Ok(cmd::SourceChange {
                path: RelativePath::parse(&write.subject)
                    .map_err(|_| SourceCommandError::Invalid("PG Item recovery source path"))?,
                before: write.expected_predecessor.map(|(_, digest)| digest),
                after: Some(write.exact_bytes.clone()),
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let journal = format!(
        "ToS/source-witnesses/.metadata-transactions/{}",
        &transaction_id[7..]
    );
    for (path, value) in [
        (
            "ToS/source-witnesses/.metadata-publication.json".to_owned(),
            &publication_package.control,
        ),
        (
            format!("{journal}/manifest.json"),
            &publication_package.manifest,
        ),
        (
            format!("{journal}/completion.json"),
            &publication_package.completion,
        ),
    ] {
        let mut raw = cmd::canonical(value)?;
        raw.push(b'\n');
        publication_changes.push(cmd::SourceChange {
            path: RelativePath::parse(&path)
                .map_err(|_| SourceCommandError::Invalid("PG Item recovery sidecar path"))?,
            before: None,
            after: Some(raw),
        });
    }
    let (decoded, _) = crate::source_work_transaction::decode_selected_publication_package(
        &publication_package.canonical_bytes,
        &publication_changes,
    )?;
    if &decoded != publication_package {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery decoded publication differs",
        ));
    }
    let journal_prefix = format!(
        "ToS/source-witnesses/.metadata-transactions/{}/",
        &transaction_id[7..]
    );
    let mut archive_members = BTreeMap::new();
    for write in other_writes {
        if write.subject.starts_with(&journal_prefix) {
            // Exact role, fixity and predecessor coverage are checked below
            // against the decoded publication source references.
            continue;
        }
        let is_archive = write.subject.strip_prefix(&format!("{archive_path}/"));
        let is_archive = is_archive.is_some_and(|name| {
            if flat_digest_blob_or_manifest(name) {
                archive_members
                    .insert(name.to_owned(), write.exact_bytes.clone())
                    .is_none()
            } else {
                false
            }
        });
        if !is_archive {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery has an unrecognized source member",
            ));
        }
    }
    let manifest_value = &publication_package.manifest;
    let completion = &publication_package.completion;
    let control = &publication_package.control;
    let mut journal_manifest = cmd::canonical(manifest_value)?;
    journal_manifest.push(b'\n');
    let mut journal_completion = cmd::canonical(completion)?;
    journal_completion.push(b'\n');
    if journal_manifest.len() > 524_288 || journal_completion.len() > 16_384 {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery transaction members exceed bounds",
        ));
    }
    crate::source_work_transaction::state(control)?;
    // The shared decoder above owns the manifest grammar, including paired
    // optional physical-store provenance; this owner checks Item identity.
    if cmd::text(manifest_value, "schema_version")?
        != crate::source_work_transaction::PG_SOURCE_MANIFEST_SCHEMA
        || cmd::text(manifest_value, "transaction_id")? != transaction_id
        || cmd::text(manifest_value, "parent_binding_profile")?
            != "tos_source_revision_parent_bindings_v1"
    {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery transaction manifest identity differs",
        ));
    }
    let base_publication = cmd::field(manifest_value, "base_publication")?;
    cmd::exact_keys(base_publication, &["token", "generation"])?;
    let base_generation = cmd::integer(base_publication, "generation")?;
    let base_token = match cmd::field(base_publication, "token")? {
        JsonValue::Null => None,
        JsonValue::String(_) => Some(cmd::text(base_publication, "token")?.to_owned()),
        _ => {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery publication base differs",
            ));
        }
    };
    if base_generation != publication_package.base_publication_generation
        || base_token != publication_package.base_publication_token
        || publication_package.generation != cmd::integer(control, "generation")?
        || publication_package.generation
            != base_generation
                .checked_add(2)
                .ok_or(SourceCommandError::Conflict(
                    "PG Item recovery publication generation overflow",
                ))?
        || publication_package.token != cmd::text(control, "token")?
        || cmd::text(control, "transaction_id")? != transaction_id
        || cmd::text(control, "manifest_sha256")?
            != Digest256::of_bytes(&journal_manifest).to_prefixed()
        || cmd::text(control, "phase")? != "ready"
        || cmd::text(control, "outcome")? != "committed"
    {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery publication generation differs",
        ));
    }
    let plan = cmd::field(manifest_value, "plan")?;
    cmd::exact_keys(
        plan,
        &["authorization", "new_directories", "files", "path_profile"],
    )?;
    let path_profile = cmd::field(plan, "path_profile")?;
    cmd::exact_keys(path_profile, &["schema_version", "item_source_path"])?;
    if cmd::text(path_profile, "schema_version")? != "tos_item_metadata_paths_v1"
        || cmd::text(path_profile, "item_source_path")? != owner.item_path.as_str()
    {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery transaction path profile differs",
        ));
    }
    let file_rows = cmd::field(plan, "files")?
        .as_array()
        .ok_or(SourceCommandError::Conflict(
            "PG Item recovery transaction file plan shape differs",
        ))?;
    let mut planned_writes = attempt
        .writes
        .iter()
        .filter(|write| {
            publication_package.source_refs.iter().any(|source_ref| {
                source_ref.role == "authored_source" && source_ref.path == write.subject
            })
        })
        .collect::<Vec<_>>();
    planned_writes.sort_by(|left, right| left.subject.cmp(&right.subject));
    if file_rows.len() != planned_writes.len() {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery transaction file count differs",
        ));
    }
    let mut attempt_source_refs = BTreeMap::new();
    for write in &attempt.writes {
        if attempt_source_refs
            .insert(write.subject.as_str(), write)
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery source write repeats",
            ));
        }
    }
    if publication_package.source_refs.len() != attempt_source_refs.len() {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery publication source coverage differs",
        ));
    }
    let mut publication_refs = publication_package
        .source_refs
        .iter()
        .map(|source_ref| (source_ref.path.as_str(), source_ref))
        .collect::<BTreeMap<_, _>>();
    if publication_refs.len() != publication_package.source_refs.len() {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery publication source path repeats",
        ));
    }
    for (row, write) in file_rows.iter().zip(planned_writes) {
        cmd::exact_keys(row, &["path", "before", "after"])?;
        let path = cmd::text(row, "path")?;
        if path != write.subject {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery transaction file path differs",
            ));
        }
        let source_ref = publication_refs
            .remove(path)
            .ok_or(SourceCommandError::Conflict(
                "PG Item recovery publication source path absent",
            ))?;
        if source_ref.role != "authored_source" {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery publication source role differs",
            ));
        }
        let before = cmd::field(row, "before")?;
        match write.expected_predecessor {
            Some((_, expected)) => {
                cmd::exact_keys(before, &["sha256", "bytes"])?;
                let source_before =
                    source_ref
                        .before
                        .as_ref()
                        .ok_or(SourceCommandError::Conflict(
                            "PG Item recovery publication predecessor absent",
                        ))?;
                if cmd::text(before, "sha256")? != expected.to_prefixed()
                    || source_before.sha256 != expected
                    || source_before.byte_size != cmd::integer(before, "bytes")?
                {
                    return Err(SourceCommandError::Conflict(
                        "PG Item recovery transaction predecessor differs",
                    ));
                }
            }
            None if before == &JsonValue::Null && source_ref.before.is_none() => {}
            None => {
                return Err(SourceCommandError::Conflict(
                    "PG Item recovery transaction insertion differs",
                ));
            }
        }
        let after = cmd::field(row, "after")?;
        cmd::exact_keys(after, &["sha256", "bytes"])?;
        let digest = Digest256::of_bytes(&write.exact_bytes);
        let source_after = source_ref
            .after
            .as_ref()
            .ok_or(SourceCommandError::Conflict(
                "PG Item recovery publication successor absent",
            ))?;
        if cmd::text(after, "sha256")? != digest.to_prefixed()
            || cmd::integer(after, "bytes")? != write.exact_bytes.len() as u64
            || source_after.sha256 != digest
            || source_after.byte_size != write.exact_bytes.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery transaction successor differs",
            ));
        }
    }
    // Retained transaction blobs are distinct from authored source rows.
    // Their complete set and digest-derived names were checked by the shared
    // package decoder; bind every one back to the durable attempt's bytes.
    for (path, source_ref) in publication_refs {
        let write = attempt_source_refs
            .get(path)
            .ok_or(SourceCommandError::Conflict(
                "PG Item recovery journal write absent",
            ))?;
        let digest = Digest256::of_bytes(&write.exact_bytes);
        if source_ref.role != "selected_transaction_blob"
            || path != format!("{journal_prefix}{}.blob", digest.to_hex())
            || write.expected_predecessor.is_some()
            || source_ref.before.is_some()
            || source_ref.after.as_ref().is_none_or(|binding| {
                binding.sha256 != digest || binding.byte_size != write.exact_bytes.len() as u64
            })
        {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery journal binding differs",
            ));
        }
    }
    cmd::exact_keys(completion, &["schema_version", "publication"])?;
    if cmd::text(&completion, "schema_version")? != "tos_selected_metadata_completion_v1" {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery completion schema differs",
        ));
    }
    let publication = cmd::field(completion, "publication")?;
    crate::source_work_transaction::state(publication)?;
    if !cmd::same(publication, control)?
        || cmd::text(publication, "transaction_id")? != transaction_id
        || cmd::text(publication, "manifest_sha256")?
            != Digest256::of_bytes(&journal_manifest).to_prefixed()
        || cmd::text(publication, "phase")? != "ready"
        || cmd::text(publication, "outcome")? != "committed"
    {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery completion publication differs",
        ));
    }
    let archive_manifest_raw =
        archive_members
            .get("manifest.json")
            .ok_or(SourceCommandError::Conflict(
                "PG Item recovery archive manifest absent",
            ))?;
    let archive_manifest = cmd::parse(archive_manifest_raw)?;
    cmd::exact_keys(
        &archive_manifest,
        &[
            "schema_version",
            "source_path",
            "source",
            "revision",
            "files",
            "publication_protocol",
        ],
    )?;
    if cmd::text(&archive_manifest, "schema_version")? != "tos_source_package_archive_v2"
        || cmd::text(&archive_manifest, "source_path")? != owner.edition_path.as_str()
        || cmd::text(&archive_manifest, "publication_protocol")?
            != "tos_selected_source_metadata_v1"
        || cmd::text(&archive_manifest, "revision")? != cmd::text(&receipt, "parent_revision")?
        || !cmd::same(
            cmd::field(&archive_manifest, "source")?,
            cmd::field(&receipt, "parent_before")?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery archive manifest differs",
        ));
    }
    let manifest_source_id = cmd::text(cmd::field(&archive_manifest, "source")?, "id")?;
    let archive_config = cmd::object(vec![("record_id", cmd::string(manifest_source_id))]);
    if crate::source_revisions::archive_path(
        &archive_config,
        cmd::text(&archive_manifest, "revision")?,
    )? != archive_path
    {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery archive path does not match manifest",
        ));
    }
    let file_rows =
        cmd::field(&archive_manifest, "files")?
            .as_object()
            .ok_or(SourceCommandError::Conflict(
                "PG Item recovery archive file table shape differs",
            ))?;
    if !matches!(file_rows.len(), 2 | 3) {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery archive file table count differs",
        ));
    }
    let mut predecessor = BTreeMap::new();
    for (name, row) in file_rows {
        let name = name.as_str().ok_or(SourceCommandError::Invalid(
            "PG Item recovery archive file name",
        ))?;
        if ![
            "edition.json",
            "edition.human-forms.json",
            "source-revision-history.json",
        ]
        .contains(&name)
        {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery archive file name differs",
            ));
        }
        cmd::exact_keys(row, &["sha256", "bytes", "blob"])?;
        let digest = cmd::text(row, "sha256")?;
        let digest_hex = digest
            .strip_prefix("sha256:")
            .filter(|value| lower_hex_64(value))
            .ok_or(SourceCommandError::Conflict(
                "PG Item recovery archive file digest differs",
            ))?;
        let blob_name = cmd::text(row, "blob")?;
        if blob_name != format!("{digest_hex}.blob") {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery archive blob reference differs",
            ));
        }
        let raw = archive_members
            .get(blob_name)
            .ok_or(SourceCommandError::Conflict(
                "PG Item recovery archive blob absent",
            ))?;
        if raw.len() as u64 != cmd::integer(row, "bytes")?
            || Digest256::of_bytes(raw).to_prefixed() != digest
            || predecessor.insert(name.to_owned(), raw.clone()).is_some()
        {
            return Err(SourceCommandError::Conflict(
                "PG Item recovery archive blob fixity differs",
            ));
        }
    }
    let _ = exact_parent_before(&receipt, &predecessor, edition_home)?;
    let predecessor_edition = cmd::parse(predecessor.get("edition.json").ok_or(
        SourceCommandError::Conflict("PG Item recovery predecessor Edition absent"),
    )?)?;
    let predecessor_id = cmd::text(&predecessor_edition, "record_id")?;
    let predecessor_version = cmd::integer(&predecessor_edition, "record_version")?;
    let predecessor_digest = cmd::record_digest(&predecessor_edition)?.to_prefixed();
    exact_record_ref(
        cmd::field(&receipt, "parent_before")?,
        predecessor_id,
        predecessor_version,
        &predecessor_digest,
    )?;
    let expected_archive = archive_package(
        owner.edition_path.as_str(),
        &predecessor_edition,
        &predecessor,
        cmd::text(&receipt, "parent_revision")?,
    )?;
    if expected_archive.path != archive_path || expected_archive.members != archive_members {
        return Err(SourceCommandError::Conflict(
            "PG Item recovery archive differs from predecessor package",
        ));
    }
    let materializations = super::compound_views(&parent, &child, &owner.claim_id)?;
    let mut deposit = crate::source_item_deposit::public_state(stage)?;
    deposit["metadata_committed"] = serde_json::Value::Bool(true);
    let response = cmd::object(vec![
        ("transaction_id", cmd::string(&owner.transaction_id()?)),
        ("receipt", receipt),
        ("materializations", materializations),
        ("deposit", super::foundation_value(&deposit)?),
        ("metadata_committed", JsonValue::Bool(true)),
    ]);
    super::serde_value(&response)
}

fn selected_publication_source_ref_value(source_ref: &SelectedPublicationSourceRef) -> JsonValue {
    cmd::object(vec![
        ("path", cmd::string(&source_ref.path)),
        ("role", cmd::string(&source_ref.role)),
        (
            "before",
            source_ref
                .before
                .as_ref()
                .map_or(JsonValue::Null, selected_publication_binding_value),
        ),
        (
            "after",
            source_ref
                .after
                .as_ref()
                .map_or(JsonValue::Null, selected_publication_binding_value),
        ),
    ])
}

fn selected_publication_binding_value(binding: &SelectedPublicationBinding) -> JsonValue {
    cmd::object(vec![
        ("sha256", cmd::string(&binding.sha256.to_prefixed())),
        ("byte_size", cmd::number(binding.byte_size)),
    ])
}

fn flat_digest_blob_or_manifest(name: &str) -> bool {
    name == "manifest.json" || flat_digest_blob(name)
}

fn flat_digest_blob(name: &str) -> bool {
    name.strip_suffix(".blob").is_some_and(lower_hex_64)
}

fn lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn current_children(
    session: &mut SourceRevisionReadSession<'_>,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<(String, bool)>> {
    let parent = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("PG Item directory path"))?;
    session
        .list_immediate_children(&parent, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Item directory generation"))
}

fn read_current(
    session: &mut SourceRevisionReadSession<'_>,
    path: &str,
    total: &mut usize,
    digests: &mut BTreeMap<String, String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    if !digests.contains_key(path) && digests.len() >= ITEM_CLOSURE_MAX_MEMBERS {
        return Err(SourceCommandError::Invalid("PG Item closure member budget"));
    }
    let relative = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("PG Item closure source path"))?;
    let member = session
        .read_current_member(&relative, 2_097_152, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Item exact current member"))?;
    *total = total
        .checked_add(member.raw.len())
        .filter(|bytes| *bytes <= ITEM_CLOSURE_MAX_BYTES)
        .ok_or(SourceCommandError::Invalid("PG Item closure byte budget"))?;
    let digest = Digest256::of_bytes(&member.raw).to_hex();
    if let Some(previous) = digests.insert(path.to_owned(), digest.clone()) {
        if previous != digest {
            return Err(SourceCommandError::Conflict(
                "PG Item current member changed inside read epoch",
            ));
        }
    }
    Ok(member.raw)
}

fn closure_claims(
    session: &mut SourceRevisionReadSession<'_>,
    owner: &super::ItemOwner,
    deadline: Instant,
    cancelled: &AtomicBool,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
) -> SourceCommandResult<(BTreeMap<String, JsonValue>, BTreeMap<String, String>)> {
    let keys = [
        (
            "object".to_owned(),
            "embodied_by".to_owned(),
            owner.edition_id.clone(),
        ),
        (
            "subject".to_owned(),
            "exemplified_by".to_owned(),
            owner.edition_id.clone(),
        ),
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    let observations = session
        .lookup_claim_dependencies(&keys, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Item Claim closure lookup"))?;
    if observations.len() != keys.len() {
        return Err(SourceCommandError::Conflict(
            "PG Item Claim closure predicate observation missing",
        ));
    }
    let mut edges_by_path =
        BTreeMap::<String, Vec<crate::source_claim_dependency_index::ClaimDependencyEdge>>::new();
    for observation in observations.values() {
        for edge in &observation.rows {
            edges_by_path
                .entry(edge.path.clone())
                .or_default()
                .push(edge.clone());
        }
    }
    let mut claims = BTreeMap::new();
    let mut claim_paths = BTreeMap::new();
    for (path, edges) in edges_by_path {
        if edges.is_empty() || edges.len() > 256 {
            return Err(SourceCommandError::Invalid(
                "PG Item Claim stream closure bound",
            ));
        }
        let raw = read_current(session, &path, total, digests, deadline, cancelled)?;
        let raw_digest = Digest256::of_bytes(&raw);
        let expected = edges
            .into_iter()
            .map(|edge| {
                if edge.member_digest != raw_digest {
                    return Err(SourceCommandError::Conflict(
                        "PG Item reverse Claim index stream digest differs",
                    ));
                }
                Ok((edge.claim_id.clone(), edge))
            })
            .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
        if expected.len() > 256 {
            return Err(SourceCommandError::Invalid(
                "PG Item Claim closure row bound",
            ));
        }
        let mut found = BTreeMap::new();
        for line in raw.split(|byte| *byte == b'\n' || *byte == b'\r') {
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let claim = cmd::parse(line)?;
            let claim_id = cmd::text(&claim, "claim_id")?;
            let Some(edge) = expected.get(claim_id) else {
                continue;
            };
            if cmd::text(&claim, "predicate")? != edge.predicate
                || cmd::text(&claim, "subject_ref")? != edge.subject_endpoint
                || cmd::text(&claim, "object")? != edge.object_endpoint
                || found.insert(claim_id.to_owned(), claim).is_some()
            {
                return Err(SourceCommandError::Conflict(
                    "PG Item reverse Claim edge differs from exact stream",
                ));
            }
        }
        if found.len() != expected.len() {
            return Err(SourceCommandError::Conflict(
                "PG Item reverse Claim edge absent from exact stream",
            ));
        }
        for (claim_id, claim) in found {
            if claims.contains_key(&claim_id) {
                return Err(SourceCommandError::Conflict(
                    "PG Item Claim identity repeats across reverse predicates",
                ));
            }
            let (kind, indexed_path) = session
                .lookup_source_identity(&claim_id, deadline, cancelled)
                .map_err(|_| SourceCommandError::Conflict("PG Item Claim identity lookup"))?
                .ok_or(SourceCommandError::Conflict(
                    "PG Item Claim identity index absent",
                ))?;
            if kind != "claim" || indexed_path.as_str() != path {
                return Err(SourceCommandError::Conflict(
                    "PG Item Claim identity index differs from reverse edge",
                ));
            }
            claim_paths.insert(claim_id.clone(), path.clone());
            claims.insert(claim_id, claim);
        }
    }
    Ok((claims, claim_paths))
}

/// Select the Edition package and only its reverse-indexed origin/exemplar
/// closure. Each selected Claim stream digest is checked against its reverse
/// index row before those exact source bytes become owner dependencies.
pub(crate) fn current_closure(
    session: &mut SourceRevisionReadSession<'_>,
    owner: &super::ItemOwner,
    proposal: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgItemCurrentClosure> {
    let (kind, edition_path) = session
        .lookup_source_identity(&owner.edition_id, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Item Edition identity lookup"))?
        .ok_or(SourceCommandError::Conflict(
            "PG Item Edition identity absent",
        ))?;
    // Source identity indexing classifies catalogue records as metadata;
    // the selected record bytes below establish the Edition type and identity.
    if kind != "metadata" || edition_path.as_str() != owner.edition_path.as_str() {
        return Err(SourceCommandError::Conflict(
            "PG Item Edition source identity differs from protected owner",
        ));
    }
    let edition_home = owner
        .edition_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("PG Item Edition home"))?
        .0;
    let items_name = owner
        .item_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("PG Item target home"))?
        .0
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("PG Item target directory"))?
        .1
        .to_owned();
    let target_item_home = owner
        .item_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("PG Item target home"))?
        .0;
    let mut new_directories = Vec::new();
    let edition_children = current_children(session, edition_home, deadline, cancelled)?;
    let items_entry = edition_children.iter().find(|(name, _)| name == "items");
    match items_entry {
        Some((_, true)) => {}
        Some((_, false)) => {
            return Err(SourceCommandError::Conflict(
                "PG Item items parent is not a directory",
            ));
        }
        None => new_directories.push(
            RelativePath::parse(&format!("{edition_home}/items"))
                .map_err(|_| SourceCommandError::Invalid("PG Item items directory"))?,
        ),
    }
    if target_item_home != format!("{edition_home}/items/{items_name}") {
        return Err(SourceCommandError::Conflict("PG Item target path differs"));
    }
    // Observe only this new home. Sibling Items, archives and journals grow
    // independently and do not belong in a new command's directory result.
    session
        .observe_home_absence(&super::relative(target_item_home)?, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Item target home occupied or changed"))?;
    new_directories.push(
        RelativePath::parse(target_item_home)
            .map_err(|_| SourceCommandError::Invalid("PG Item target directory"))?,
    );
    let mut total = 0usize;
    let mut digests = BTreeMap::new();
    let mut before = BTreeMap::new();
    for name in [
        "edition.json",
        "edition.human-forms.json",
        "source-revision-history.json",
    ] {
        if name == "source-revision-history.json" {
            match edition_children.iter().find(|(child, _)| child == name) {
                Some((_, false)) => {}
                Some((_, true)) => {
                    return Err(SourceCommandError::Conflict(
                        "PG Item Edition history is not a file",
                    ));
                }
                None => continue,
            }
        }
        before.insert(
            name.to_owned(),
            read_current(
                session,
                &format!("{edition_home}/{name}"),
                &mut total,
                &mut digests,
                deadline,
                cancelled,
            )?,
        );
    }
    let edition = cmd::parse(
        before
            .get("edition.json")
            .ok_or(SourceCommandError::Conflict(
                "PG Item Edition package absent",
            ))?,
    )?;
    if cmd::text(&edition, "record_type")? != "edition"
        || cmd::text(&edition, "record_id")? != owner.edition_id
    {
        return Err(SourceCommandError::Conflict(
            "PG Item Edition record differs from protected owner",
        ));
    }
    let record_history = tos_validation::native_compound::inspect_record_history(
        &before,
        before.get("edition.json").unwrap(),
        deadline,
        cancelled,
    )
    .map_err(super::item_error)?;
    let _ = record_history;
    let expected_revision = crate::source_revisions::revision(&before)?;
    let archive_path = crate::source_revisions::archive_path(
        &cmd::object(vec![(
            "record_id",
            cmd::field(&edition, "record_id")?.clone(),
        )]),
        &expected_revision,
    )?;
    let archive_parent = archive_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("PG Item archive parent"))?
        .0;
    let witness_children = current_children(session, "ToS/source-witnesses", deadline, cancelled)?;
    match witness_children
        .iter()
        .find(|(name, _)| name == ".record-revisions")
    {
        Some((_, true)) => {}
        Some((_, false)) => {
            return Err(SourceCommandError::Conflict(
                "PG Item archive root is not a directory",
            ));
        }
        None => new_directories.push(
            RelativePath::parse(archive_parent)
                .map_err(|_| SourceCommandError::Invalid("PG Item archive parent"))?,
        ),
    }
    session
        .observe_home_absence(&super::relative(&archive_path)?, deadline, cancelled)
        .map_err(|_| {
            SourceCommandError::Conflict("PG Item predecessor archive occupied or changed")
        })?;
    new_directories.push(
        RelativePath::parse(&archive_path)
            .map_err(|_| SourceCommandError::Invalid("PG Item archive path"))?,
    );

    // A prepare-create proposal has no caller-selected command_id yet, so it
    // cannot identify the durable transaction journal. Preserve the root's
    // shape check for proposals; only transaction identity and leaf collision
    // checks require the committed request's stable command_id.
    let transaction_root_exists = match witness_children
        .iter()
        .find(|(name, _)| name == ".metadata-transactions")
    {
        Some((_, true)) => true,
        Some((_, false)) => {
            return Err(SourceCommandError::Conflict(
                "PG Item transaction root is not a directory",
            ));
        }
        None => false,
    };
    if !proposal {
        let transaction_id = owner.transaction_id()?;
        let transaction_leaf = transaction_id
            .strip_prefix("sha256:")
            .ok_or(SourceCommandError::Invalid("PG Item transaction identity"))?;
        let transaction_home = RelativePath::parse(&format!(
            "ToS/source-witnesses/.metadata-transactions/{transaction_leaf}"
        ))
        .map_err(|_| SourceCommandError::Invalid("PG Item transaction directory"))?;
        session
            .observe_home_absence(&transaction_home, deadline, cancelled)
            .map_err(|_| {
                SourceCommandError::Conflict("PG Item transaction journal occupied or changed")
            })?;
        if !transaction_root_exists {
            new_directories.push(
                RelativePath::parse("ToS/source-witnesses/.metadata-transactions")
                    .map_err(|_| SourceCommandError::Invalid("PG Item transaction root"))?,
            );
        }
        new_directories.push(transaction_home);
    }
    let (claims, claim_paths) = closure_claims(
        session,
        owner,
        deadline,
        cancelled,
        &mut digests,
        &mut total,
    )?;

    // The reverse Claim index is the bounded adjacency source. Require it to
    // agree with both the Edition's forward references and declared origins.
    let exemplars = claims
        .values()
        .filter(|claim| {
            claim.object_get("predicate").and_then(JsonValue::as_str) == Some("exemplified_by")
                && claim.object_get("subject_ref").and_then(JsonValue::as_str)
                    == Some(owner.edition_id.as_str())
        })
        .collect::<Vec<_>>();
    let exemplar_ids = exemplars
        .iter()
        .map(|claim| cmd::text(claim, "claim_id"))
        .collect::<SourceCommandResult<std::collections::BTreeSet<_>>>()?;
    let forward_ids = cmd::array(&edition, "exemplar_claim_refs")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or(SourceCommandError::Invalid("PG Item exemplar Claim ref"))
        })
        .collect::<SourceCommandResult<std::collections::BTreeSet<_>>>()?;
    if exemplar_ids.len() != exemplars.len()
        || forward_ids.len() != cmd::array(&edition, "exemplar_claim_refs")?.len()
        || exemplar_ids != forward_ids
        || exemplars.len() > 128
    {
        return Err(SourceCommandError::Conflict(
            "PG Item Edition exemplar forward/reverse closure differs",
        ));
    }
    let origins = claims
        .values()
        .filter(|claim| {
            claim.object_get("predicate").and_then(JsonValue::as_str) == Some("embodied_by")
                && claim.object_get("object").and_then(JsonValue::as_str)
                    == Some(owner.edition_id.as_str())
        })
        .collect::<Vec<_>>();
    let declared_origins = cmd::array(&edition, "embodies_expression_refs")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or(SourceCommandError::Invalid("PG Item Edition origin ref"))
        })
        .collect::<SourceCommandResult<std::collections::BTreeSet<_>>>()?;
    let observed_origins = origins
        .iter()
        .map(|claim| cmd::text(claim, "subject_ref"))
        .collect::<SourceCommandResult<std::collections::BTreeSet<_>>>()?;
    if declared_origins.len() != cmd::array(&edition, "embodies_expression_refs")?.len()
        || declared_origins != observed_origins
        || origins.len() > 128
    {
        return Err(SourceCommandError::Conflict(
            "PG Item Edition origin forward/reverse closure differs",
        ));
    }

    // Bind every related Item source record and package, but no unrelated
    // Item. Native compound verification consumes the exact Claim paths and
    // verifies each retained transaction/history chain separately.
    let mut related_items = BTreeMap::new();
    let mut expected_file_assignments = Vec::new();
    let mut expected_rights_assignments = Vec::new();
    let mut targets = std::collections::BTreeSet::new();
    for claim in exemplars {
        let target_id = cmd::text(claim, "object")?.to_owned();
        if !targets.insert(target_id.clone()) || related_items.len() >= 96 {
            return Err(SourceCommandError::Conflict(
                "PG Item exemplar target closure",
            ));
        }
        let (kind, path) = session
            .lookup_source_identity(&target_id, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("PG Item target identity lookup"))?
            .ok_or(SourceCommandError::Conflict(
                "PG Item target identity absent",
            ))?;
        if kind != "metadata" {
            return Err(SourceCommandError::Conflict(
                "PG Item exemplar target is not metadata",
            ));
        }
        let home = path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("PG Item target home"))?
            .0;
        if !home.starts_with(&format!("{edition_home}/items/")) {
            return Err(SourceCommandError::Conflict(
                "PG Item target is outside Edition",
            ));
        }
        let mut package = BTreeMap::new();
        for name in [
            "item.json",
            "item.human-forms.json",
            "source-claims.jsonl",
            "edition-item-receipt.json",
            "item.manifest.json",
            "rights.json",
            "resource-inventory.json",
        ] {
            let raw = read_current(
                session,
                &format!("{home}/{name}"),
                &mut total,
                &mut digests,
                deadline,
                cancelled,
            )?;
            package.insert(name.to_owned(), raw);
        }
        let item = cmd::parse(package.get("item.json").unwrap())?;
        let manifest = cmd::parse(package.get("item.manifest.json").unwrap())?;
        // The identity point index binds the stable ID to this exact path;
        // native compound verification checks the current record bytes.
        if path.as_str() != format!("{home}/item.json")
            || cmd::text(&item, "record_type")? != "item"
            || cmd::text(&item, "record_id")? != target_id
        {
            return Err(SourceCommandError::Conflict(
                "PG Item target record differs",
            ));
        }
        let manifest_path = format!("{home}/item.manifest.json");
        let rights_path = format!("{home}/rights.json");
        let inventory_path = format!("{home}/resource-inventory.json");
        if cmd::text(&manifest, "schema_version")? != "tos_source_item_manifest_v1"
            || cmd::text(&manifest, "item_id")? != target_id
            || cmd::text(&manifest, "embodiment_ref")? != owner.edition_id
            || cmd::text(&manifest, "rights_ref")? != rights_path
            || cmd::text(&manifest, "resource_inventory_ref")? != inventory_path
        {
            return Err(SourceCommandError::Conflict(
                "PG Item target manifest differs",
            ));
        }
        let rights_value = cmd::parse(
            package
                .get("rights.json")
                .ok_or(SourceCommandError::Conflict("PG Item rights absent"))?,
        )?;
        let rights_id = cmd::text(&rights_value, "rights_id")?.to_owned();
        let manifest_digest = Digest256::of_bytes(
            package
                .get("item.manifest.json")
                .ok_or(SourceCommandError::Conflict("PG Item manifest absent"))?,
        );
        let rights_digest = Digest256::of_bytes(
            package
                .get("rights.json")
                .ok_or(SourceCommandError::Conflict("PG Item rights absent"))?,
        );
        let assignments = cmd::array(&manifest, "payload_files")?;
        if assignments.is_empty() || assignments.len() > 4096 {
            return Err(SourceCommandError::Invalid(
                "PG related Item manifest assignment bound",
            ));
        }
        for (ordinal, assignment) in assignments.iter().enumerate() {
            expected_file_assignments.push((
                cmd::text(assignment, "file_id")?.to_owned(),
                target_id.clone(),
                manifest_path.clone(),
                ordinal,
                manifest_digest,
                rights_id.clone(),
                rights_digest,
            ));
        }
        expected_rights_assignments.push((
            rights_id,
            target_id.clone(),
            manifest_path,
            manifest_digest,
            rights_digest,
        ));
        related_items.insert(target_id, package);
    }
    if targets.len() != related_items.len() {
        return Err(SourceCommandError::Conflict("PG Item target set differs"));
    }
    let edition_ids = std::collections::BTreeSet::from([owner.edition_id.clone()]);
    let edition_observations = session
        .lookup_item_edition_assignments(&edition_ids, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Item Edition membership lookup"))?;
    if edition_observations.len() != edition_ids.len() {
        return Err(SourceCommandError::Conflict(
            "PG Item Edition membership observations incomplete",
        ));
    }
    let mut expected_edition_rows = std::collections::BTreeSet::new();
    for (_, item_id, manifest_path, manifest_digest, _) in &expected_rights_assignments {
        expected_edition_rows.insert(crate::source_item_file_index::ItemEditionAssignment {
            item_id: item_id.clone(),
            manifest_path: manifest_path.clone(),
            manifest_digest: *manifest_digest,
        });
    }
    let edition_observation =
        edition_observations
            .get(&owner.edition_id)
            .ok_or(SourceCommandError::Conflict(
                "PG Item Edition membership observation absent",
            ))?;
    let observed_edition_item_ids = edition_observation
        .rows
        .iter()
        .map(|row| row.item_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let target_item_ids = targets
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if edition_observation.rows != expected_edition_rows
        || observed_edition_item_ids != target_item_ids
    {
        return Err(SourceCommandError::Conflict(
            "PG Item Edition manifest inventory differs from exemplar Claims",
        ));
    }
    let file_ids = expected_file_assignments
        .iter()
        .map(|(id, ..)| id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if file_ids.len() > crate::source_item_file_index::MAX_LOOKUP_ROWS {
        return Err(SourceCommandError::Invalid(
            "PG related Item File point count bound",
        ));
    }
    let file_observations = session
        .lookup_item_file_assignments(&file_ids, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG related Item File index lookup"))?;
    if file_observations.len() != file_ids.len() {
        return Err(SourceCommandError::Conflict(
            "PG related Item File index observations incomplete",
        ));
    }
    for (file_id, item_id, manifest_path, ordinal, manifest_digest, rights_id, rights_digest) in
        expected_file_assignments
    {
        let observation = file_observations
            .get(&file_id)
            .ok_or(SourceCommandError::Conflict(
                "PG related Item File point absent",
            ))?;
        if !observation.rows.iter().any(|row| {
            row.item_id == item_id
                && row.manifest_path == manifest_path
                && row.payload_ordinal == ordinal
                && row.file_id == file_id
                && row.member_digest == manifest_digest
                && row.rights_id == rights_id
                && row.rights_digest == rights_digest
        }) {
            return Err(SourceCommandError::Conflict(
                "PG related Item File assignment index differs from package",
            ));
        }
    }
    let rights_ids = expected_rights_assignments
        .iter()
        .map(|(id, ..)| id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let rights_observations = session
        .lookup_item_rights_assignments(&rights_ids, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG related Item rights index lookup"))?;
    if rights_observations.len() != rights_ids.len() {
        return Err(SourceCommandError::Conflict(
            "PG related Item rights index observations incomplete",
        ));
    }
    for (rights_id, item_id, manifest_path, manifest_digest, rights_digest) in
        expected_rights_assignments
    {
        let observation =
            rights_observations
                .get(&rights_id)
                .ok_or(SourceCommandError::Conflict(
                    "PG related Item rights point absent",
                ))?;
        if !observation.rows.iter().any(|row| {
            row.item_id == item_id
                && row.manifest_path == manifest_path
                && row.manifest_digest == manifest_digest
                && row.rights_id == rights_id
                && row.rights_digest == rights_digest
        }) {
            return Err(SourceCommandError::Conflict(
                "PG related Item rights index differs from package",
            ));
        }
    }

    let mut identity_values = vec![
        owner.item_id.clone(),
        owner.claim_id.clone(),
        cmd::text(&owner.configuration, "file_id")?.to_owned(),
        cmd::text(&owner.configuration, "rights_id")?.to_owned(),
        owner.event_id.clone(),
        cmd::text(&owner.configuration, "acquisition_event_id")?.to_owned(),
        cmd::text(&owner.configuration, "inventory_event_id")?.to_owned(),
    ];
    for field in [
        "allowed_edition_form_ids",
        "allowed_item_form_ids",
        "allowed_claim_form_ids",
    ] {
        identity_values.extend(super::allowed_forms(&owner.configuration, field)?.into_iter());
    }
    let identities = identity_values
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if identities.len() != identity_values.len() {
        return Err(SourceCommandError::Conflict(
            "PG Item new identity collision",
        ));
    }
    // The compound command rebinds the Edition's existing source-copy forms.
    // Only those exact current identities may already be occupied; child forms
    // and any newly selected Edition form still require global absence.
    let edition_forms = cmd::parse(
        before
            .get("edition.human-forms.json")
            .ok_or(SourceCommandError::Conflict("PG Item Edition forms absent"))?,
    )?;
    let retained_edition_forms = cmd::array(&edition_forms, "forms")?
        .iter()
        .map(|form| cmd::text(form, "form_id").map(str::to_owned))
        .collect::<SourceCommandResult<std::collections::BTreeSet<_>>>()?;
    let allowed_edition_forms =
        super::allowed_forms(&owner.configuration, "allowed_edition_form_ids")?;
    let edition_forms_path = format!("{edition_home}/edition.human-forms.json");
    for token in identities {
        let existing = session
            .lookup_source_identity(&token, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("PG Item identity lookup"))?;
        if allowed_edition_forms.contains(&token) && retained_edition_forms.contains(&token) {
            if !matches!(existing, Some((kind, path))
                if kind == "form" && path.as_str() == edition_forms_path)
            {
                return Err(SourceCommandError::Conflict(
                    "PG Item retained Edition form identity differs",
                ));
            }
        } else if existing.is_some() {
            return Err(SourceCommandError::Conflict(
                "PG Item new identity is occupied",
            ));
        }
        // lookup_source_identity already binds all five namespace generations
        // for both occupied and absent tokens. observe_unique_index additionally
        // requires absence, so it cannot be used for retained parent forms.
    }
    let file_id = cmd::text(&owner.configuration, "file_id")?.to_owned();
    let file_rows = session
        .lookup_item_file_assignments(
            &std::collections::BTreeSet::from([file_id]),
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("PG Item File assignment lookup"))?;
    if file_rows
        .values()
        .any(|observation| !observation.rows.is_empty())
    {
        return Err(SourceCommandError::Conflict(
            "PG Item File is already assigned",
        ));
    }
    let rights_id = cmd::text(&owner.configuration, "rights_id")?.to_owned();
    let rights_rows = session
        .lookup_item_rights_assignments(
            &std::collections::BTreeSet::from([rights_id]),
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("PG Item rights assignment lookup"))?;
    if rights_rows
        .values()
        .any(|observation| !observation.rows.is_empty())
    {
        return Err(SourceCommandError::Conflict(
            "PG Item rights identity is occupied",
        ));
    }

    Ok(PgItemCurrentClosure {
        before,
        related_items,
        claims,
        claim_paths,
        catalog: super::ItemCatalog {
            digests,
            retained_transactions: BTreeMap::new(),
            native_reads: Vec::new(),
            native_bytes: 0,
            native_state: 0,
            source_bytes: total as u64,
        },
        new_directories,
    })
}

fn read_selected_input<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let mut selected = None;
    input
        .source_input()
        .with_current_member(path, 2_097_152, deadline, cancelled, &mut |meta, raw| {
            if meta.path != path || selected.is_some() {
                return Err(tos_validation::item_rules::ItemRefusal::Source(
                    "PG Item exact member identity differs".into(),
                ));
            }
            selected = Some(raw.to_vec());
            Ok(())
        })
        .map_err(super::item_error)?;
    selected.ok_or(SourceCommandError::Conflict("PG Item exact member absent"))
}

fn read_optional_selected_input<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    let mut selected = None;
    input
        .source_input()
        .with_current_member(path, 2_097_152, deadline, cancelled, &mut |meta, raw| {
            if meta.path != path || selected.is_some() {
                return Err(tos_validation::item_rules::ItemRefusal::Source(
                    "PG Item exact optional member identity differs".into(),
                ));
            }
            selected = Some(raw.to_vec());
            Ok(())
        })
        .map_err(super::item_error)?;
    Ok(selected)
}

/// Reuse the native compound reader for every indexed relation in the exact
/// Edition closure. A committed native transaction is proved by the current
/// PG source journal read under the same input identity; local transaction
/// journals are never consulted here.
pub(crate) fn verify_current_topology<I: Copy + Eq>(
    closure: &mut PgItemCurrentClosure,
    history: &serde_json::Value,
    input: &dyn SourceCutInputWithIdentity<I>,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if input.input_identity() != worker.input_identity() {
        return Err(SourceCommandError::Conflict(
            "PG Item topology schema input identity differs",
        ));
    }
    let mut total = usize::try_from(closure.catalog.source_bytes)
        .map_err(|_| SourceCommandError::Invalid("PG Item source byte count overflow"))?;
    let mut native_bytes = 0u64;
    let mut native_state = 0usize;
    let mut retained = BTreeMap::<String, String>::new();
    let edition = cmd::parse(
        closure
            .before
            .get("edition.json")
            .ok_or(SourceCommandError::Conflict("PG Item Edition absent"))?,
    )?;
    for (claim_id, claim) in &closure.claims {
        let path = closure
            .claim_paths
            .get(claim_id)
            .ok_or(SourceCommandError::Conflict("PG Item Claim path absent"))?;
        let predicate = cmd::text(claim, "predicate")?;
        if path.ends_with("/source-claims.jsonl") {
            let remaining_bytes = limits
                .max_total_bytes
                .checked_sub(native_bytes)
                .ok_or(SourceCommandError::Invalid("PG Item topology byte budget"))?;
            let remaining_state = limits
                .max_state_bytes
                .checked_sub(native_state)
                .ok_or(SourceCommandError::Invalid("PG Item topology state budget"))?;
            let mut remaining = limits;
            remaining.max_total_bytes = remaining_bytes;
            remaining.max_state_bytes = remaining_state;
            let claim_json = super::serde_value(claim)?;
            let observation = match predicate {
                "embodied_by" => {
                    tos_validation::native_compound::verify_expression_edition_from_selected_input(
                        input,
                        worker,
                        path,
                        &claim_json,
                        remaining,
                        cancelled,
                    )
                }
                "exemplified_by" => {
                    tos_validation::native_compound::verify_edition_item_from_selected_input(
                        input,
                        worker,
                        path,
                        &claim_json,
                        remaining,
                        cancelled,
                    )
                }
                _ => {
                    return Err(SourceCommandError::Conflict(
                        "PG Item topology predicate is unsupported",
                    ));
                }
            }
            .map_err(super::item_error)?;
            if observation.claim_id != *claim_id
                || observation.transport
                    != tos_validation::native_compound::NativeTransportState::Committed
            {
                return Err(SourceCommandError::Conflict(
                    "PG Item related native transaction is not committed",
                ));
            }
            if predicate == "embodied_by" {
                let receipt_path = format!(
                    "{}/expression-edition-receipt.json",
                    path.rsplit_once('/')
                        .ok_or(SourceCommandError::Invalid("PG Item origin Claim home"))?
                        .0
                );
                let receipt = cmd::parse(&read_selected_input(
                    input,
                    &receipt_path,
                    limits.deadline,
                    cancelled,
                )?)?;
                super::initial_child(&receipt, "edition", &edition, history)?;
            } else if !history["receipts"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["publication"]["transaction_id"].as_str()
                        == Some(observation.transaction_id.as_str())
                })
            }) {
                return Err(SourceCommandError::Conflict(
                    "PG Item native exemplar is absent from Edition history",
                ));
            }
            let manifest = observation.manifest_sha256.clone();
            if retained
                .insert(observation.transaction_id.clone(), manifest.clone())
                .is_some_and(|old| old != manifest)
            {
                return Err(SourceCommandError::Conflict(
                    "PG Item native transaction identity changed",
                ));
            }
            native_bytes = native_bytes
                .checked_add(observation.bytes_read)
                .ok_or(SourceCommandError::Invalid("PG Item native byte overflow"))?;
            native_state = native_state
                .checked_add(observation.returned_state_bytes)
                .ok_or(SourceCommandError::Invalid("PG Item native state overflow"))?;
            closure.catalog.native_reads.extend(observation.reads);
        } else {
            if cmd::text(claim, "claim_type")? != "bibliographic"
                || cmd::text(claim, "assertion_layer")? != "bibliographic_assertion"
                || cmd::text(claim, "provenance_event_ref")?
                    != "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31"
            {
                return Err(SourceCommandError::Denied(
                    "legacy Item Claim lacks exact evidence route",
                ));
            }
            let provenance_path = "ToS/source-witnesses/relations/provenance.jsonl";
            let raw = read_selected_input(input, provenance_path, limits.deadline, cancelled)?;
            total = total
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= ITEM_CLOSURE_MAX_BYTES)
                .ok_or(SourceCommandError::Invalid("PG Item source byte budget"))?;
            let provenance_digest = Digest256::of_bytes(&raw).to_hex();
            if let Some(previous) = closure
                .catalog
                .digests
                .insert(provenance_path.to_owned(), provenance_digest.clone())
            {
                if previous != provenance_digest {
                    return Err(SourceCommandError::Conflict(
                        "PG Item provenance batch changed inside read epoch",
                    ));
                }
            }
            let mut lines = raw
                .split(|byte| *byte == b'\n' || *byte == b'\r')
                .filter(|line| !line.iter().all(u8::is_ascii_whitespace));
            let batch = cmd::parse(
                lines
                    .next()
                    .ok_or(SourceCommandError::Conflict("legacy topology batch absent"))?,
            )?;
            if lines.next().is_some()
                || !cmd::array(&batch, "outputs")?.iter().any(|row| {
                    row.object_get("ref").and_then(JsonValue::as_str) == Some(path)
                        && row.object_get("sha256").and_then(JsonValue::as_str)
                            == closure.catalog.digests.get(path).map(String::as_str)
                })
            {
                return Err(SourceCommandError::Conflict(
                    "legacy topology retained batch differs",
                ));
            }
        }
    }
    closure.catalog.retained_transactions = retained;
    closure.catalog.native_bytes = native_bytes;
    closure.catalog.native_state = native_state;
    closure.catalog.source_bytes = u64::try_from(total)
        .map_err(|_| SourceCommandError::Invalid("PG Item source byte count overflow"))?;
    Ok(())
}
use tos_validation::native_compound::{EditionItemBytes, EditionItemGrammarObservation};
use tos_validation::record_biblio_cut::SourceCutInputWithIdentity;
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

/// Reuse the maintained Item grammar and explicit bibliographic append laws
/// with the same exact selected-input seam used by the native compound owner.
/// This is the non-inventory preview path; the input cannot enumerate the
/// corpus and every observed source dependency remains in the PG readset.
pub(crate) fn unavailable_grammar<I: Copy + Eq>(
    owner: &super::ItemOwner,
    before: &BTreeMap<String, Vec<u8>>,
    input: &dyn SourceCutInputWithIdentity<I>,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<EditionItemGrammarObservation> {
    let edition_raw = before
        .get("edition.json")
        .ok_or(SourceCommandError::Conflict("Item parent absent"))?;
    let item_raw = cmd::canonical(cmd::field(&owner.request, "record")?)?;
    let claim_raw = cmd::canonical(cmd::field(&owner.request, "claim")?)?;
    let mut grammar =
        tos_validation::native_compound::inspect_edition_item_grammar_from_selected_input(
            input,
            worker,
            edition_raw,
            &item_raw,
            &claim_raw,
            limits,
            cancelled,
        )
        .map_err(super::item_error)?;

    let mut revised = cmd::parse(edition_raw)?;
    cmd::set(
        &mut revised,
        "exemplar_claim_refs",
        cmd::field(cmd::field(&owner.request, "fields")?, "exemplar_claim_refs")?.clone(),
    )?;
    let version = cmd::integer(&revised, "record_version")?
        .checked_add(1)
        .ok_or(SourceCommandError::Invalid("Item parent version overflow"))?;
    cmd::set(&mut revised, "record_version", cmd::number(version))?;
    let revised_raw = cmd::canonical(&revised)?;
    let claim_path = format!(
        "{}/source-claims.jsonl",
        owner.item_path.as_str().rsplit_once('/').unwrap().0
    );
    let mut delta_limits = limits;
    delta_limits.max_total_bytes = limits
        .max_total_bytes
        .checked_sub(grammar.bytes_read)
        .ok_or(SourceCommandError::Invalid(
            "Item unavailable grammar read budget",
        ))?;
    delta_limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(grammar.returned_state_bytes)
        .ok_or(SourceCommandError::Invalid(
            "Item unavailable grammar state budget",
        ))?;
    let mut delta = tos_validation::biblio_rules::inspect_bibliographic_delta(
        tos_validation::biblio_rules::BiblioDeltaInput {
            parent_path: owner.edition_path.as_str(),
            endpoint_path: owner.item_path.as_str(),
            claim_path: &claim_path,
            parent_before_raw: edition_raw,
            parent_after_raw: &revised_raw,
            endpoint_raw: &item_raw,
            claim_raw: &claim_raw,
        },
        delta_limits,
        cancelled,
        worker,
    )
    .map_err(super::item_error)?;
    let required = format!(
        "{}@{}:exemplified_by",
        tos_validation::biblio_rules::BIBLIOGRAPHIC_DELTA_RULE_ID,
        tos_validation::biblio_rules::BIBLIOGRAPHIC_DELTA_RULE_VERSION
    );
    if !delta.issues.is_empty()
        || delta.issue_sink_truncated
        || !delta.checked_profiles.contains(&required)
        || delta.skipped_profiles.len() != 1
        || !delta
            .skipped_profiles
            .contains("whole-compound-plan-forms-dependency-byte-custody-current-lineage-and-permission-fence")
    {
        return Err(SourceCommandError::Conflict(
            "Item explicit append/backlink mechanics refused",
        ));
    }
    // The delta validator consumes hypothetical buffers. Rebind those
    // supplied exact-path observations to exact bytes so no unavailable Item
    // or Claim is mistaken for current PG source.
    for read in &mut delta.reads {
        let tos_validation::PredicateRead::ExactPath { path, digest } = read else {
            continue;
        };
        let supplied = [
            (owner.edition_path.as_str(), revised_raw.as_slice()),
            (owner.item_path.as_str(), item_raw.as_slice()),
            (claim_path.as_str(), claim_raw.as_slice()),
        ]
        .into_iter()
        .find(|(candidate, _)| *candidate == path.as_str());
        if let Some((_, raw)) = supplied {
            if Digest256::of_bytes(raw).to_prefixed() != *digest {
                return Err(SourceCommandError::Conflict(
                    "Item supplied delta observation differs from validated bytes",
                ));
            }
            *read = tos_validation::PredicateRead::ExactBytes {
                locator: format!("supplied-item-delta:{path}"),
                digest: digest.clone(),
            };
        }
    }
    let supplied_bytes = [
        edition_raw.as_slice(),
        revised_raw.as_slice(),
        item_raw.as_slice(),
        claim_raw.as_slice(),
    ]
    .iter()
    .try_fold(0u64, |n, raw| n.checked_add(raw.len() as u64))
    .ok_or(SourceCommandError::Invalid(
        "Item delta input budget overflow",
    ))?;
    grammar.bytes_read = grammar
        .bytes_read
        .checked_add(supplied_bytes)
        .filter(|bytes| *bytes <= limits.max_total_bytes)
        .ok_or(SourceCommandError::Invalid(
            "Item whole grammar/delta read budget",
        ))?;
    let retained = delta
        .reads
        .iter()
        .try_fold(0usize, |n, row| {
            n.checked_add(format!("{row:?}").len() + 64)
        })
        .ok_or(SourceCommandError::Invalid(
            "Item retained delta observation state overflow",
        ))?;
    grammar.returned_state_bytes = grammar
        .returned_state_bytes
        .checked_add(retained)
        .filter(|bytes| *bytes <= limits.max_state_bytes)
        .ok_or(SourceCommandError::Invalid(
            "Item whole grammar/delta state budget",
        ))?;
    grammar.reads.extend(delta.reads);
    Ok(grammar)
}

/// Exact output draft from the maintained Item owner. The payload stage and
/// identity stay private to the native PG adapter so its final owner check can
/// prove custody again after registration and immediately before commit.
pub(crate) struct PgItemAdoptionOwnerPlan {
    pub(crate) transaction_id: String,
    pub(crate) response: JsonValue,
    pub(crate) outputs: BTreeMap<String, Vec<u8>>,
    pub(crate) item_claim_owner: JsonValue,
    pub(crate) dependency_bindings: JsonValue,
    pub(crate) stage: serde_json::Value,
    pub(crate) payload_identity: serde_json::Value,
    pub(crate) publication_control: Option<(Option<Digest256>, Vec<u8>)>,
    pub(crate) prepared_request: Option<JsonValue>,
}

// PG source roots hold the segment store, not a checkout of the software.
// Recheck the explicitly selected software capture at the same owner edges as
// the physical adapter, and keep the exact implementation dependency binding.
fn selected_software_current(
    context: &CommandContext,
    software: &tos_source_store::SoftwareCaptureReader,
    components: &tos_source_store::SoftwareComponentSelectionV1,
    dependencies: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut total = 0usize;
    for input in context
        .files
        .iter()
        .filter(|file| !file.path.as_str().starts_with("ToS/"))
    {
        total = total
            .checked_add(input.raw.len())
            .filter(|total| *total <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "PG Item software aggregate budget",
            ))?;
        let current = software
            .read_selected_component(components, &input.path, 8_388_608, deadline, cancelled)
            .map_err(|_| {
                SourceCommandError::Conflict("PG Item selected software member changed")
            })?;
        if current != input.raw {
            return Err(SourceCommandError::Conflict(
                "PG Item selected software bytes changed",
            ));
        }
    }
    let bindings = cmd::field(dependencies, "implementation")?
        .as_object()
        .ok_or(SourceCommandError::Invalid(
            "PG Item implementation dependency group",
        ))?;
    if bindings.len() > 512 {
        return Err(SourceCommandError::Invalid(
            "PG Item implementation dependency count",
        ));
    }
    let mut total = 0usize;
    for (reference, digest) in bindings {
        super::active(deadline, cancelled)?;
        let path = super::relative(
            reference
                .as_str()
                .ok_or(SourceCommandError::Invalid("PG Item implementation path"))?,
        )?;
        if components.member(&path).is_none() || path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Conflict(
                "PG Item implementation outside selected software",
            ));
        }
        let raw = context.file(&path)?.ok_or(SourceCommandError::Conflict(
            "PG Item implementation absent",
        ))?;
        total = total
            .checked_add(raw.len())
            .filter(|total| *total <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "PG Item implementation aggregate budget",
            ))?;
        if raw.len() > 2_097_152 || digest.as_str() != Some(super::raw_hex(raw).as_str()) {
            return Err(SourceCommandError::Conflict(
                "PG Item implementation bytes changed",
            ));
        }
    }
    Ok(())
}

pub(crate) fn verify_pg_item_adoption_owner(
    fs: &CreationFilesystem,
    context: &CommandContext,
    software: &tos_source_store::SoftwareCaptureReader,
    components: &tos_source_store::SoftwareComponentSelectionV1,
    transaction_id: &str,
    dependencies: &JsonValue,
    stage: &serde_json::Value,
    payload_identity: &serde_json::Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let configuration = cmd::parse(&context.configuration_raw)?;
    super::current_grant(fs, context, &configuration, deadline, cancelled)?;
    selected_software_current(
        context,
        software,
        components,
        dependencies,
        deadline,
        cancelled,
    )?;
    let config = super::serde_value(&configuration)?;
    crate::source_item_deposit::stage_current(
        &config,
        stage,
        payload_identity,
        fs.uid,
        deadline,
        cancelled,
    )?;
    crate::source_item_deposit::verify_deposit(
        &config,
        stage,
        fs.uid,
        deadline,
        cancelled,
        &mut || super::current_grant(fs, context, &configuration, deadline, cancelled),
    )?;
    if stage["transaction_id"].as_str() != Some(transaction_id) {
        return Err(SourceCommandError::Conflict(
            "PG Item retained deposit transaction changed",
        ));
    }
    Ok(())
}

/// Run the maintained Edition -> Item recipe over the selected PG input and
/// the existing independently protected payload owner. No authored source is
/// consulted from the local mirror; directory, presence, and byte reads go
/// through the selected input and therefore enter the source readset.
pub(crate) fn prepare_pg_item_adoption(
    fs: &CreationFilesystem,
    context: &CommandContext,
    selection: PgItemAdoptionSelection,
    input: &dyn SourceCutInputWithIdentity<
        crate::source_cohort::revision::PgRecordRevisionIdentity,
    >,
    software: &tos_source_store::SoftwareCaptureReader,
    components: &tos_source_store::SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<
        crate::source_cohort::revision::PgRecordRevisionIdentity,
    >,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgItemAdoptionOwnerPlan> {
    let PgItemAdoptionSelection {
        mut owner,
        mut closure,
        proposal,
    } = selection;
    let snapshot =
        PublicationSnapshot::select_from_input(input.source_input(), limits.deadline, cancelled)?;
    let edition_raw = closure
        .before
        .get("edition.json")
        .ok_or(SourceCommandError::Conflict("PG Item Edition absent"))?;
    let edition = cmd::parse(edition_raw)?;
    let edition_history = tos_validation::native_compound::inspect_record_history(
        &closure.before,
        edition_raw,
        limits.deadline,
        cancelled,
    )
    .map_err(super::item_error)?;
    let revision = crate::source_revisions::revision(&closure.before)?;
    if context.base_revision != input.input_identity().bootstrap_revision {
        return Err(SourceCommandError::Conflict(
            "PG Item bootstrap revision changed",
        ));
    }
    if proposal {
        let configuration = super::serde_value(&owner.configuration)?;
        let observation = crate::source_item_deposit::observe(
            &configuration,
            fs.uid,
            limits.deadline,
            cancelled,
            &mut || {
                super::current_grant(
                    fs,
                    context,
                    &owner.configuration,
                    limits.deadline,
                    cancelled,
                )?;
                snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)
            },
        )?;
        owner.request = super::prepare_request(
            &owner,
            &closure.before,
            &snapshot,
            &observation,
            &context.recorded_at,
        )?;
    } else if cmd::text(&owner.request, "expected_revision")? != revision
        || !cmd::same(
            cmd::field(&owner.request, "expected_source")?,
            &cmd::reference(&edition, "record_id", "record_version")?,
        )?
        || cmd::field(&owner.request, "expected_publication")?.as_str() != snapshot.token.as_deref()
    {
        return Err(SourceCommandError::Conflict(
            "PG Item expected source or publication changed",
        ));
    }
    verify_current_topology(
        &mut closure,
        &edition_history,
        input,
        worker,
        limits,
        cancelled,
    )?;
    let catalog = closure.catalog;
    let grammar = unavailable_grammar(
        &owner,
        &closure.before,
        input,
        worker,
        super::remaining(&catalog, limits)?,
        cancelled,
    )?;
    let dependencies = super::dependency_bindings(&catalog, &grammar.grammar_digests, context)?;
    let dependency_digest = cmd::record_digest(&dependencies)?.to_prefixed();
    if proposal {
        cmd::set(
            &mut owner.request,
            "expected_dependencies",
            cmd::string(&dependency_digest),
        )?;
    } else if dependency_digest != cmd::text(&owner.request, "expected_dependencies")? {
        return Err(SourceCommandError::Conflict(
            "PG Item current preparation dependencies changed",
        ));
    }
    super::current_grant(
        fs,
        context,
        &owner.configuration,
        limits.deadline,
        cancelled,
    )?;
    selected_software_current(
        context,
        software,
        components,
        &dependencies,
        limits.deadline,
        cancelled,
    )?;

    if proposal {
        worker
            .finish(limits.deadline, cancelled)
            .map_err(super::item_error)?;
        super::current_grant(
            fs,
            context,
            &owner.configuration,
            limits.deadline,
            cancelled,
        )?;
        snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
        let response = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_native_pg_item_adoption_preparation_v1"),
            ),
            ("operation", cmd::string("prepare-create")),
            (
                "owner_configuration",
                cmd::string(&owner.configuration_digest),
            ),
            (
                "edition_source_path",
                cmd::string(owner.edition_path.as_str()),
            ),
            ("item_source_path", cmd::string(owner.item_path.as_str())),
            (
                "expected_source",
                cmd::field(&owner.request, "expected_source")?.clone(),
            ),
            (
                "expected_revision",
                cmd::field(&owner.request, "expected_revision")?.clone(),
            ),
            (
                "expected_dependencies",
                cmd::field(&owner.request, "expected_dependencies")?.clone(),
            ),
            (
                "expected_publication",
                cmd::field(&owner.request, "expected_publication")?.clone(),
            ),
            ("prepared_request", owner.request.clone()),
            ("grants_admission", JsonValue::Bool(false)),
            ("content_disclosure", cmd::string("withheld")),
        ]);
        return Ok(PgItemAdoptionOwnerPlan {
            transaction_id: String::new(),
            response,
            outputs: BTreeMap::new(),
            item_claim_owner: JsonValue::Null,
            dependency_bindings: dependencies,
            stage: serde_json::Value::Null,
            payload_identity: serde_json::Value::Null,
            publication_control: None,
            prepared_request: Some(owner.request),
        });
    }

    let configuration = super::serde_value(&owner.configuration)?;
    let transaction_id = owner.transaction_id()?;
    let request_value = super::serde_value(&owner.request)?;
    let stage = crate::source_item_deposit::ensure_deposit(
        &configuration,
        &request_value,
        &transaction_id,
        fs.uid,
        false,
        limits.deadline,
        cancelled,
        &mut || {
            super::current_grant(
                fs,
                context,
                &owner.configuration,
                limits.deadline,
                cancelled,
            )?;
            snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)
        },
    )?;
    let payload_identity = crate::source_item_deposit::deposited_identity(
        &configuration,
        &stage,
        fs.uid,
        limits.deadline,
        cancelled,
    )?;
    let public_deposit =
        super::foundation_value(&crate::source_item_deposit::public_state(&stage)?)?;
    if cmd::field(&owner.request, "inventory")? == &JsonValue::Null {
        worker
            .finish(limits.deadline, cancelled)
            .map_err(super::item_error)?;
        super::current_grant(
            fs,
            context,
            &owner.configuration,
            limits.deadline,
            cancelled,
        )?;
        snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
        return Ok(PgItemAdoptionOwnerPlan {
            transaction_id: transaction_id.clone(),
            response: cmd::object(vec![
                ("transaction_id", cmd::string(&transaction_id)),
                ("receipt", JsonValue::Null),
                ("materializations", JsonValue::Null),
                ("deposit", public_deposit),
                ("metadata_committed", JsonValue::Bool(false)),
            ]),
            outputs: BTreeMap::new(),
            item_claim_owner: JsonValue::Null,
            dependency_bindings: dependencies,
            stage,
            payload_identity,
            publication_control: None,
            prepared_request: None,
        });
    }

    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let receipt_raw = cmd::canonical(&super::foundation_value(
        &crate::source_item_deposit::public_receipt(&stage)?,
    )?)?;
    let mut core_limits = super::remaining(&catalog, limits)?;
    core_limits.max_total_bytes = core_limits
        .max_total_bytes
        .checked_sub(grammar.bytes_read)
        .ok_or(SourceCommandError::Invalid(
            "PG Item pre-copy grammar budget",
        ))?;
    core_limits.max_state_bytes = core_limits
        .max_state_bytes
        .checked_sub(grammar.returned_state_bytes)
        .ok_or(SourceCommandError::Invalid(
            "PG Item pre-copy grammar state budget",
        ))?;
    let authorization = owner.authorization(dependencies.clone())?;
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_selected_edition_item_bytes(
        input,
        worker,
        &super::serde_value(&owner.scope()?)?,
        &request_raw,
        &closure.before,
        &context.recorded_at,
        &receipt_raw,
        super::GENERATOR,
        core_limits,
        cancelled,
        |actual_grammar| {
            let result = (|| {
                let actual = super::dependency_bindings(&catalog, actual_grammar, context)?;
                if !cmd::same(&actual, &dependencies)? {
                    return Err(SourceCommandError::Conflict(
                        "PG Item serialization grammar differs from checked copy basis",
                    ));
                }
                super::serde_value(&authorization)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("PG Item owner authorization rejected".to_owned())
            })
        },
    );
    let core =
        core.map_err(|error| authorization_error.unwrap_or_else(|| super::item_error(error)))?;
    if core.transaction_id() != transaction_id {
        return Err(SourceCommandError::Conflict(
            "PG Item native transaction identity differs",
        ));
    }
    let archive_path = core.archive_path().to_owned();
    let outputs = core.outputs().map_err(super::item_error)?;
    let borrowed = outputs
        .iter()
        .map(|(path, bytes)| (path.as_str(), *bytes))
        .collect::<Vec<_>>();
    let home = owner
        .item_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("PG Item home"))?
        .0;
    let capture = crate::source_serialization::capture_item_adoption(
        &owner.request,
        &owner.event_id,
        home,
        &archive_path,
        &closure.before,
        &borrowed,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let finished = tos_validation::native_compound::finish_selected_edition_item_bytes(
        core,
        &capture.environment_raw,
        &capture.event_raw,
        worker,
    )
    .map_err(super::item_error)?;
    if finished.transaction_id != transaction_id || finished.archive_path != archive_path {
        return Err(SourceCommandError::Conflict(
            "PG Item finished native byte identity differs",
        ));
    }
    let receipt = super::finished_receipt(&finished.child)?;
    let materializations =
        super::compound_views(&finished.parent, &finished.child, &owner.claim_id)?;
    let claim_owner = claim_owner_contribution(
        &finished,
        owner
            .edition_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("PG Item Edition home"))?
            .0,
        home,
        &closure.before,
    )?;
    let mut plan = owner.plan(
        authorization.clone(),
        &closure.before,
        finished.parent.clone(),
        finished.child.clone(),
        closure.new_directories.clone(),
    )?;
    let archive = super::pg_archive_package(
        owner.edition_path.as_str(),
        &edition,
        &closure.before,
        &revision,
    )?;
    if archive.path != archive_path {
        return Err(SourceCommandError::Conflict(
            "PG Item immutable predecessor archive differs",
        ));
    }
    for (name, raw) in &archive.members {
        plan.files
            .push(crate::source_work_transaction::SelectedFile {
                path: tos_foundation::RelativePath::parse(&format!("{}/{name}", archive.path))
                    .map_err(|_| SourceCommandError::Invalid("PG Item archive member path"))?,
                before: None,
                after: Some(raw.clone()),
            });
    }
    let commit_members = crate::source_work_transaction::pg_selected_commit_members(
        input.source_input(),
        &snapshot,
        &plan,
        limits.deadline,
        cancelled,
    )?;
    let mut publication_control = None;
    for (path, raw) in commit_members {
        let before = read_optional_selected_input(input, &path, limits.deadline, cancelled)?;
        if path == "ToS/source-witnesses/.metadata-publication.json" {
            if publication_control
                .replace((before.as_ref().map(|bytes| Digest256::of_bytes(bytes)), raw))
                .is_some()
            {
                return Err(SourceCommandError::Conflict(
                    "PG Item publication control repeats",
                ));
            }
            continue;
        }
        if before.is_some() {
            return Err(SourceCommandError::Conflict(
                "PG Item transaction journal path already occupied",
            ));
        }
        plan.files
            .push(crate::source_work_transaction::SelectedFile {
                path: tos_foundation::RelativePath::parse(&path)
                    .map_err(|_| SourceCommandError::Invalid("PG Item commit member path"))?,
                before,
                after: Some(raw),
            });
    }
    for file in &plan.files {
        if file.before.is_none()
            && input
                .source_input()
                .path_presence(file.path.as_str(), limits.deadline, cancelled)
                .map_err(super::item_error)?
                .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "PG Item new package member became occupied",
            ));
        }
    }
    let mut outputs = BTreeMap::new();
    for file in &plan.files {
        let after = file.after.as_ref().ok_or(SourceCommandError::Conflict(
            "PG Item package output absent",
        ))?;
        if outputs
            .insert(file.path.as_str().to_owned(), after.clone())
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "PG Item package output repeats",
            ));
        }
    }
    if outputs.len() > 512 || outputs.values().map(Vec::len).sum::<usize>() > ITEM_CLOSURE_MAX_BYTES
    {
        return Err(SourceCommandError::Invalid("PG Item output package budget"));
    }
    if publication_control.is_none() {
        return Err(SourceCommandError::Conflict(
            "PG Item publication control output absent",
        ));
    }
    worker
        .finish(limits.deadline, cancelled)
        .map_err(super::item_error)?;
    super::current_grant(
        fs,
        context,
        &owner.configuration,
        limits.deadline,
        cancelled,
    )?;
    selected_software_current(
        context,
        software,
        components,
        &dependencies,
        limits.deadline,
        cancelled,
    )?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    Ok(PgItemAdoptionOwnerPlan {
        transaction_id,
        response: cmd::object(vec![
            ("transaction_id", cmd::string(&finished.transaction_id)),
            ("receipt", receipt),
            ("materializations", materializations),
            ("deposit", public_deposit),
            ("metadata_committed", JsonValue::Bool(true)),
        ]),
        outputs,
        item_claim_owner: claim_owner,
        dependency_bindings: dependencies,
        stage,
        payload_identity,
        publication_control,
        prepared_request: None,
    })
}

const ITEM_OWNER_SCHEMA: &str = "tos_managed_item_adoption_claim_owner_inventory_v1";

fn conflict(reason: &'static str) -> SourceCommandError {
    SourceCommandError::Conflict(reason)
}

fn member_row(path: &str, role: &str, raw: &[u8]) -> JsonValue {
    cmd::object(vec![
        ("ref", cmd::string(path)),
        ("role", cmd::string(role)),
        ("bytes", cmd::number(raw.len() as u64)),
        (
            "sha256",
            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
        ),
    ])
}

fn member_ref(path: &str, raw: &[u8]) -> JsonValue {
    cmd::object(vec![
        ("ref", cmd::string(path)),
        ("bytes", cmd::number(raw.len() as u64)),
        (
            "sha256",
            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
        ),
    ])
}

fn expected_child_role(name: &str) -> Option<&'static str> {
    Some(match name {
        "item.json" => "item-record",
        "item.human-forms.json" => "item-form-set",
        "source-claims.jsonl" => "claim-stream",
        "source-create-request.json" => "creation-request",
        "source-create-environment.json" => "creation-environment",
        "source-create-provenance.jsonl" => "creation-event",
        "edition-item-receipt.json" => "compound-receipt",
        "item.manifest.json" => "file-manifest",
        "rights.json" => "rights-record",
        "resource-inventory.json" => "resource-inventory",
        "fixity.sha256" => "fixity",
        "forensic-report.md" => "forensic-report",
        "provenance.jsonl" => "provenance",
        "item-deposit-receipt.json" => "deposit-receipt",
        name if name.starts_with("source-claims.") && name.ends_with(".human-forms.json") => {
            "claim-form-set"
        }
        _ => return None,
    })
}

fn exact_map(actual: &JsonValue, expected: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<()> {
    let values = actual
        .as_object()
        .ok_or(SourceCommandError::Invalid("Item receipt member map"))?;
    if values.len() != expected.len() {
        return Err(conflict("Item native receipt member count differs"));
    }
    for (name, raw) in expected {
        let value = actual
            .object_get(name)
            .ok_or(conflict("Item native receipt member absent"))?;
        cmd::exact_keys(value, &["sha256", "bytes"])?;
        if cmd::text(value, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
            || cmd::integer(value, "bytes")? != raw.len() as u64
        {
            return Err(conflict("Item native receipt member digest differs"));
        }
    }
    Ok(())
}

fn exact_parent_before(
    receipt: &JsonValue,
    predecessor: &BTreeMap<String, Vec<u8>>,
    edition_home: &str,
) -> SourceCommandResult<Vec<JsonValue>> {
    let rows = cmd::field(receipt, "parent_before_files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("Item predecessor receipt map"))?;
    if rows.len() != predecessor.len() {
        return Err(conflict("Item predecessor receipt set differs"));
    }
    let mut refs = Vec::with_capacity(rows.len());
    for (name, raw) in predecessor {
        let row = cmd::field(cmd::field(receipt, "parent_before_files")?, name)?;
        cmd::exact_keys(row, &["sha256", "bytes"])?;
        if cmd::text(row, "sha256")? != Digest256::of_bytes(raw).to_prefixed()
            || cmd::integer(row, "bytes")? != raw.len() as u64
        {
            return Err(conflict("Item predecessor receipt digest differs"));
        }
        refs.push(member_ref(&format!("{edition_home}/{name}"), raw));
    }
    Ok(refs)
}

fn exact_record_ref(
    actual: &JsonValue,
    id: &str,
    version: u64,
    digest: &str,
) -> SourceCommandResult<()> {
    cmd::exact_keys(actual, &["id", "version", "digest"])?;
    if cmd::text(actual, "id")? != id
        || cmd::integer(actual, "version")? != version
        || cmd::text(actual, "digest")? != digest
    {
        return Err(conflict("Item compound record reference differs"));
    }
    Ok(())
}

/// Build the exact immutable predecessor archive expected by the native
/// Edition -> Item recipe. This mirrors the shared compound archive contract
/// without touching the filesystem; archive bytes join the same PG revision
/// as the Item package.
pub(crate) fn archive_package(
    edition_path: &str,
    edition: &JsonValue,
    before: &BTreeMap<String, Vec<u8>>,
    expected_revision: &str,
) -> SourceCommandResult<WorkArchivePackage> {
    let allowed_names = [
        "edition.json",
        "edition.human-forms.json",
        "source-revision-history.json",
    ];
    if cmd::text(edition, "record_type")? != "edition"
        || !edition_path.ends_with("/edition.json")
        || !(2..=3).contains(&before.len())
        || !["edition.json", "edition.human-forms.json"]
            .iter()
            .all(|name| before.contains_key(*name))
        || before
            .keys()
            .any(|name| !allowed_names.contains(&name.as_str()))
        || before.values().any(|raw| raw.len() > 2_097_152)
        || before
            .values()
            .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
            .is_none_or(|n| n > 8_388_608)
    {
        return Err(SourceCommandError::Denied(
            "Item archive selected Edition package",
        ));
    }
    let revision = crate::source_revisions::revision(before)?;
    if revision != expected_revision {
        return Err(conflict("Item archive predecessor revision differs"));
    }
    let subject = cmd::reference(edition, "record_id", "record_version")?;
    let archive_config = cmd::object(vec![("record_id", cmd::field(&subject, "id")?.clone())]);
    let path = crate::source_revisions::archive_path(&archive_config, &revision)?;
    let mut members = BTreeMap::new();
    for raw in before.values() {
        members
            .entry(format!("{}.blob", Digest256::of_bytes(raw).to_hex()))
            .or_insert_with(|| raw.clone());
    }
    let manifest = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_source_package_archive_v2"),
        ),
        ("source_path", cmd::string(edition_path)),
        ("source", subject),
        ("revision", cmd::string(&revision)),
        ("files", crate::source_revisions::file_refs(before, true)),
        (
            "publication_protocol",
            cmd::string("tos_selected_source_metadata_v1"),
        ),
    ]);
    members.insert("manifest.json".to_owned(), cmd::published(&manifest)?);
    Ok(WorkArchivePackage { path, members })
}

/// Create the Item-specific Claim owner contribution from the exact native
/// compound output and its selected predecessor Edition package.
pub(crate) fn claim_owner_contribution(
    bytes: &EditionItemBytes,
    edition_home: &str,
    item_home: &str,
    predecessor: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<JsonValue> {
    let expected_parent = [
        "edition.json",
        "edition.human-forms.json",
        "source-revision-history.json",
    ];
    if bytes.parent.len() != expected_parent.len()
        || expected_parent
            .iter()
            .any(|name| !bytes.parent.contains_key(*name))
        || predecessor
            .keys()
            .any(|name| !expected_parent.contains(&name.as_str()))
        || !predecessor.contains_key("edition.json")
        || bytes.child.len() != 15
        || bytes
            .child
            .keys()
            .any(|name| expected_child_role(name).is_none())
    {
        return Err(conflict("Item native package shape differs"));
    }

    let parent_record = cmd::parse(
        bytes
            .parent
            .get("edition.json")
            .ok_or(conflict("Item parent record absent"))?,
    )?;
    let old_parent = cmd::parse(
        predecessor
            .get("edition.json")
            .ok_or(conflict("Item predecessor record absent"))?,
    )?;
    let item_record = cmd::parse(
        bytes
            .child
            .get("item.json")
            .ok_or(conflict("Item record absent"))?,
    )?;
    let claim_raw = bytes
        .child
        .get("source-claims.jsonl")
        .ok_or(conflict("Item Claim stream absent"))?;
    let claims = claim_raw
        .split(|byte| *byte == b'\n' || *byte == b'\r')
        .enumerate()
        .map(|(line, raw)| (line + 1, raw))
        .filter(|(_, raw)| !raw.iter().all(u8::is_ascii_whitespace))
        .collect::<Vec<_>>();
    if claims.len() != 1 {
        return Err(conflict("Item exemplar Claim stream is not singular"));
    }
    let (claim_line, claim_raw_line) = claims[0];
    let claim = cmd::parse(claim_raw_line)?;
    let claim_id = cmd::text(&claim, "claim_id")?;
    let claim_version = cmd::integer(&claim, "claim_version")?;
    let claim_digest = cmd::record_digest(&claim)?.to_prefixed();
    let claim_form_name = format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
    );
    let claim_form_raw = bytes
        .child
        .get(&claim_form_name)
        .ok_or(conflict("Item exemplar Claim form set absent"))?;
    if bytes
        .child
        .keys()
        .filter(|name| name.starts_with("source-claims.") && name.ends_with(".human-forms.json"))
        .count()
        != 1
    {
        return Err(conflict("Item Claim form-set package is not singular"));
    }
    let claim_forms = cmd::parse(claim_form_raw)?;
    let claim_form_subject = cmd::reference(&claim, "claim_id", "claim_version")?;
    if cmd::text(&claim, "predicate")? != "exemplified_by"
        || cmd::text(&claim, "subject_ref")? != cmd::text(&parent_record, "record_id")?
        || cmd::text(&claim, "object")? != cmd::text(&item_record, "record_id")?
        || claim_version != 1
        || !cmd::same(cmd::field(&claim_forms, "subject")?, &claim_form_subject)?
    {
        return Err(conflict("Item exemplar Claim/form binding differs"));
    }
    let form_values = cmd::array(&claim_forms, "forms")?;
    let mut form_ids = Vec::with_capacity(form_values.len());
    let mut seen_form_ids = std::collections::BTreeSet::new();
    for form in form_values {
        let form_id = cmd::text(form, "form_id")?;
        if !seen_form_ids.insert(form_id.to_owned()) {
            return Err(conflict("Item Claim form identity duplicate"));
        }
        if !cmd::same(cmd::field(form, "subject")?, &claim_form_subject)? {
            return Err(conflict("Item Claim form subject differs"));
        }
        form_ids.push(cmd::string(form_id));
    }
    if form_ids.is_empty() {
        return Err(conflict("Item Claim form set is empty"));
    }

    let receipt = super::foundation_value(&bytes.receipt)?;
    let selected_receipt_raw = bytes
        .child
        .get("edition-item-receipt.json")
        .ok_or(conflict("Item compound receipt absent"))?;
    let selected_receipt = cmd::parse(selected_receipt_raw)?;
    if !cmd::same(&selected_receipt, &receipt)? {
        return Err(conflict("Item compound receipt binding differs"));
    }
    let parent_transition = super::foundation_value(&bytes.parent_receipt)?;
    let history = cmd::parse(
        bytes
            .parent
            .get("source-revision-history.json")
            .ok_or(conflict("Item successor Edition history absent"))?,
    )?;
    let history_receipts = cmd::array(&history, "receipts")?;
    if history_receipts
        .last()
        .is_none_or(|last| !cmd::same(last, &parent_transition).unwrap_or(false))
    {
        return Err(conflict(
            "Item parent transition absent from successor history",
        ));
    }
    let edition_id = cmd::text(&parent_record, "record_id")?;
    let edition_version = cmd::integer(&parent_record, "record_version")?;
    let edition_digest = cmd::record_digest(&parent_record)?.to_prefixed();
    let old_edition_id = cmd::text(&old_parent, "record_id")?;
    let old_edition_version = cmd::integer(&old_parent, "record_version")?;
    let old_edition_digest = cmd::record_digest(&old_parent)?.to_prefixed();
    if old_edition_id != edition_id
        || cmd::text(&history, "record_id")? != edition_id
        || cmd::text(&receipt, "schema_version")? != "tos_edition_item_receipt_v1"
        || cmd::text(&receipt, "operation")? != "item.adopt"
        || cmd::text(&receipt, "transaction_id")? != bytes.transaction_id
        || cmd::text(&receipt, "parent_archive_ref")? != bytes.archive_path
        || cmd::text(&receipt, "parent_transition_sha256")?
            != cmd::record_digest(&parent_transition)?.to_prefixed()
    {
        return Err(conflict("Item native receipt lineage identity differs"));
    }
    exact_record_ref(
        cmd::field(&receipt, "parent_before")?,
        old_edition_id,
        old_edition_version,
        &old_edition_digest,
    )?;
    exact_record_ref(
        cmd::field(&receipt, "parent_after")?,
        edition_id,
        edition_version,
        &edition_digest,
    )?;
    exact_record_ref(
        cmd::field(&receipt, "claim")?,
        claim_id,
        claim_version,
        &claim_digest,
    )?;
    if cmd::text(&parent_transition, "command_id")? != cmd::text(&receipt, "command_id")?
        || cmd::text(&parent_transition, "request_digest")?
            != cmd::text(&receipt, "request_digest")?
        || cmd::text(&parent_transition, "owner_configuration")?
            != cmd::text(&receipt, "owner_configuration")?
        || !cmd::same(
            cmd::field(&parent_transition, "source")?,
            cmd::field(&receipt, "parent_after")?,
        )?
        || !cmd::same(
            cmd::field(&parent_transition, "previous_source")?,
            cmd::field(&receipt, "parent_before")?,
        )?
    {
        return Err(conflict("Item retained parent transition binding differs"));
    }

    let mut output_package = BTreeMap::new();
    output_package.extend(
        bytes
            .parent
            .iter()
            .map(|(name, raw)| (format!("{edition_home}/{name}"), raw.clone())),
    );
    output_package.extend(
        bytes
            .child
            .iter()
            .map(|(name, raw)| (format!("{item_home}/{name}"), raw.clone())),
    );
    // The writer binds outputs before serializing the compound receipt, which
    // cannot contain its own digest. Its separate binding was checked above.
    let receipt_path = format!("{item_home}/edition-item-receipt.json");
    if output_package.remove(&receipt_path).is_none() {
        return Err(conflict("Item compound receipt package member absent"));
    }
    exact_map(cmd::field(&receipt, "files")?, &output_package)?;
    let predecessor_refs = exact_parent_before(&receipt, predecessor, edition_home)?;

    let mut direct_members = Vec::with_capacity(bytes.child.len());
    for (name, raw) in &bytes.child {
        let role = expected_child_role(name)
            .ok_or(conflict("Item owner projection child role unsupported"))?;
        direct_members.push(member_row(&format!("{item_home}/{name}"), role, raw));
    }
    direct_members.sort_by(|a, b| {
        a.object_get("ref")
            .and_then(JsonValue::as_str)
            .cmp(&b.object_get("ref").and_then(JsonValue::as_str))
    });
    let stream_path = format!("{item_home}/source-claims.jsonl");
    let claim_form_path = format!("{item_home}/{claim_form_name}");
    let claim_form = cmd::object(vec![
        ("ref", cmd::string(&claim_form_path)),
        (
            "sha256",
            cmd::string(&Digest256::of_bytes(claim_form_raw).to_prefixed()),
        ),
        ("form_ids", JsonValue::Array(form_ids)),
        ("subject", claim_form_subject),
    ]);
    let claim_row = cmd::object(vec![
        ("source_ref", cmd::string(&stream_path)),
        (
            "source_line",
            cmd::number(
                u64::try_from(claim_line)
                    .map_err(|_| SourceCommandError::Invalid("Item Claim line number overflow"))?,
            ),
        ),
        ("claim_id", cmd::string(claim_id)),
        ("claim_version", cmd::number(claim_version)),
        ("claim_sha256", cmd::string(&claim_digest)),
        ("claim", claim),
        ("form", claim_form),
    ]);

    let successor_members = bytes
        .parent
        .iter()
        .map(|(name, raw)| member_ref(&format!("{edition_home}/{name}"), raw))
        .collect::<Vec<_>>();
    let lineage = cmd::object(vec![
        ("transaction_id", cmd::string(&bytes.transaction_id)),
        ("operation", cmd::string("item.adopt")),
        ("command_id", cmd::field(&receipt, "command_id")?.clone()),
        (
            "request_digest",
            cmd::field(&receipt, "request_digest")?.clone(),
        ),
        (
            "owner_configuration",
            cmd::field(&receipt, "owner_configuration")?.clone(),
        ),
        ("recorded_at", cmd::field(&receipt, "recorded_at")?.clone()),
        ("scope", cmd::field(&receipt, "scope")?.clone()),
        (
            "dependencies",
            cmd::field(&receipt, "dependencies")?.clone(),
        ),
        (
            "parent_before",
            cmd::field(&receipt, "parent_before")?.clone(),
        ),
        (
            "parent_after",
            cmd::field(&receipt, "parent_after")?.clone(),
        ),
        (
            "parent_archive_ref",
            cmd::field(&receipt, "parent_archive_ref")?.clone(),
        ),
        (
            "parent_transition_sha256",
            cmd::field(&receipt, "parent_transition_sha256")?.clone(),
        ),
        ("parent_transition", parent_transition),
        ("predecessor_members", JsonValue::Array(predecessor_refs)),
        ("successor_members", JsonValue::Array(successor_members)),
        (
            "native_receipt_sha256",
            cmd::string(&cmd::record_digest(&receipt)?.to_prefixed()),
        ),
        ("native_receipt", receipt.clone()),
    ]);
    let mut owner = cmd::object(vec![
        ("schema_version", cmd::string(ITEM_OWNER_SCHEMA)),
        ("package_home", cmd::string(item_home)),
        ("source_path", cmd::string(&stream_path)),
        (
            "stream_sha256",
            cmd::string(&Digest256::of_bytes(claim_raw).to_prefixed()),
        ),
        ("direct_members", JsonValue::Array(direct_members)),
        ("claims", JsonValue::Array(vec![claim_row])),
        ("lineage", lineage),
    ]);
    let closure = cmd::record_digest(&owner)?.to_prefixed();
    cmd::set(&mut owner, "closure_sha256", cmd::string(&closure))?;
    Ok(owner)
}

/// New paths admitted by the already prepared Item package. Keep the file
/// roles with the Item owner; the common writer still checks path absence.
pub(crate) fn item_adoption_insertion_paths(
    writes: &[SourceRevisionWrite],
) -> crate::DurableResult<BTreeSet<String>> {
    let items = writes
        .iter()
        .filter(|write| write.subject.ends_with("/item.json"))
        .collect::<Vec<_>>();
    if items.len() != 1 || items[0].expected_predecessor.is_some() {
        return Err(crate::DurableError::Conflict(
            "Item insertion requires one new Item record",
        ));
    }
    let home = items[0].subject.strip_suffix("/item.json").unwrap();
    let receipt_path = format!("{home}/edition-item-receipt.json");
    let receipt = writes
        .iter()
        .find(|write| write.subject == receipt_path)
        .ok_or(crate::DurableError::Corrupt(
            "Item insertion receipt absent",
        ))?;
    let receipt = cmd::parse(&receipt.exact_bytes).map_err(crate::DurableError::Source)?;
    let transaction = cmd::text(&receipt, "transaction_id").map_err(crate::DurableError::Source)?;
    let transaction = transaction
        .strip_prefix("sha256:")
        .filter(|id| lower_hex_64(id))
        .ok_or(crate::DurableError::Corrupt(
            "Item insertion transaction identity",
        ))?;
    let journal = format!("ToS/source-witnesses/.metadata-transactions/{transaction}/");
    let prefix = format!("{home}/");
    let mut paths = BTreeSet::new();
    for write in writes
        .iter()
        .filter(|write| write.expected_predecessor.is_none())
    {
        if write
            .subject
            .strip_prefix(&prefix)
            .is_some_and(|name| !name.contains('/') && expected_child_role(name).is_some())
            || write.subject
                == format!(
                    "{journal}{}.blob",
                    Digest256::of_bytes(&write.exact_bytes).to_hex()
                )
        {
            paths.insert(write.subject.clone());
        }
    }
    Ok(paths)
}

/// Re-derive the Item adoption semantic source-index rows from the exact
/// sealed source writes. The shared revision writer calls this again during
/// recovery and commit; no caller-supplied token list is authoritative.
pub(crate) fn expected_item_adoption_revision_indexes(
    writes: &[SourceRevisionWrite],
) -> crate::DurableResult<IndexRows> {
    let mut rows = IndexRows::new();
    let mut edition_count = 0usize;
    let mut item_count = 0usize;
    let mut saw_claim = false;
    let mut event_count = 0usize;
    let mut form_count = 0usize;
    for write in writes {
        let path = write.subject.as_str();
        let value = match path.rsplit('/').next().unwrap_or(path) {
            "edition.json" if path.contains("/editions/") => {
                edition_count += 1;
                Some((
                    "metadata",
                    cmd::text(
                        &cmd::parse(&write.exact_bytes).map_err(crate::DurableError::Source)?,
                        "record_id",
                    )
                    .map_err(crate::DurableError::Source)?
                    .to_owned(),
                ))
            }
            "item.json" if path.contains("/items/") => {
                item_count += 1;
                Some((
                    "metadata",
                    cmd::text(
                        &cmd::parse(&write.exact_bytes).map_err(crate::DurableError::Source)?,
                        "record_id",
                    )
                    .map_err(crate::DurableError::Source)?
                    .to_owned(),
                ))
            }
            "source-claims.jsonl" => {
                let claims = json_lines(&write.exact_bytes).map_err(crate::DurableError::Source)?;
                if claims.len() != 1 || saw_claim {
                    return Err(crate::DurableError::Corrupt(
                        "Item adoption Claim stream shape differs",
                    ));
                }
                saw_claim = true;
                for claim in claims {
                    if !rows.insert((
                        "claim".to_owned(),
                        cmd::text(&claim, "claim_id")
                            .map_err(crate::DurableError::Source)?
                            .to_owned(),
                        path.to_owned(),
                    )) {
                        return Err(crate::DurableError::Corrupt(
                            "Item adoption Claim identity repeats",
                        ));
                    }
                }
                None
            }
            "source-create-provenance.jsonl" | "provenance.jsonl" => {
                let events = json_lines(&write.exact_bytes).map_err(crate::DurableError::Source)?;
                for event in events {
                    event_count += 1;
                    if !rows.insert((
                        "event".to_owned(),
                        cmd::text(&event, "event_id")
                            .map_err(crate::DurableError::Source)?
                            .to_owned(),
                        path.to_owned(),
                    )) {
                        return Err(crate::DurableError::Corrupt(
                            "Item adoption event identity repeats",
                        ));
                    }
                }
                None
            }
            name if name.ends_with(".human-forms.json") => {
                let set = cmd::parse(&write.exact_bytes).map_err(crate::DurableError::Source)?;
                let versions = cmd::array(&set, "forms")
                    .map_err(crate::DurableError::Source)?
                    .len()
                    + cmd::array(&set, "prior_forms")
                        .map_err(crate::DurableError::Source)?
                        .len();
                form_count += versions;
                for form_id in crate::source_forms::projection_form_ids(&set)
                    .map_err(crate::DurableError::Source)?
                {
                    let id = form_id
                        .as_str()
                        .ok_or(crate::DurableError::Corrupt(
                            "Item adoption form identity is malformed",
                        ))?
                        .to_owned();
                    if !rows.insert(("form".to_owned(), id, path.to_owned())) {
                        return Err(crate::DurableError::Corrupt(
                            "Item adoption form identity repeats across package members",
                        ));
                    }
                }
                None
            }
            _ => None,
        };
        if let Some((kind, token)) = value {
            rows.insert((kind.to_owned(), token, path.to_owned()));
        }
    }
    if edition_count != 1 || item_count != 1 || !saw_claim || event_count != 3 || form_count == 0 {
        return Err(crate::DurableError::Corrupt(
            "Item adoption semantic identity package differs",
        ));
    }
    if rows
        .iter()
        .map(|(_, token, _)| token)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != rows.len()
    {
        return Err(crate::DurableError::Corrupt(
            "Item adoption semantic identities repeat",
        ));
    }
    Ok(rows)
}

/// Build the mixed-source owner envelope for every Item-adoption write. Only
/// the new Item Claim stream receives the Item-specific semantic lineage;
/// records, forms and event rows remain ordinary typed projections.
pub(crate) fn inventory_owner_contributions(
    writes: &[SourceRevisionWrite],
    context_files: &[crate::source_command::SourceFile],
    item_claim_owner: &JsonValue,
) -> crate::DurableResult<BTreeMap<String, JsonValue>> {
    use tos_foundation::{JsonString, RelativePath};
    if writes.is_empty() || writes.len() > 512 {
        return Err(crate::DurableError::Corrupt(
            "Item adoption mixed inventory write bound",
        ));
    }
    let mut base_profiles = BTreeMap::new();
    let mut full_profiles = BTreeMap::new();
    for file in context_files {
        let path = file.path.as_str();
        if path == "ToS/doctrine/semantic-interchange/entity-types.v1.json"
            || path == "ToS/contracts/semantic-entity-type-registry.schema.json"
        {
            base_profiles.insert(path.to_owned(), Digest256::of_bytes(&file.raw).to_hex());
            full_profiles.insert(path.to_owned(), Digest256::of_bytes(&file.raw).to_hex());
        } else if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") {
            full_profiles.insert(path.to_owned(), Digest256::of_bytes(&file.raw).to_hex());
        }
    }
    if base_profiles.len() != 2 {
        return Err(crate::DurableError::Corrupt(
            "Item adoption source profile is incomplete",
        ));
    }
    let profiles = |values: BTreeMap<String, String>| {
        JsonValue::Object(
            values
                .into_iter()
                .map(|(k, v)| (JsonString::from_utf8(&k), cmd::string(&v)))
                .collect(),
        )
    };
    let base_profiles = profiles(base_profiles);
    let full_profiles = profiles(full_profiles);
    let mut contributions = BTreeMap::new();
    let mut projected_profile_paths = std::collections::BTreeSet::new();
    let mut package_form_ids = std::collections::BTreeSet::new();
    let mut seen_claim_owner = false;
    let mut seen_events = std::collections::BTreeSet::new();
    for write in writes {
        let path = write.subject.as_str();
        let relative = RelativePath::parse(path)
            .map_err(|_| crate::DurableError::Corrupt("Item adoption contribution path"))?;
        if relative.as_str() != path || !path.starts_with("ToS/source-witnesses/") {
            return Err(crate::DurableError::Corrupt(
                "Item adoption contribution leaves authored source",
            ));
        }
        let mut records = Vec::new();
        for kind in crate::source_claims::MIXED_INVENTORY_RECORD_KINDS {
            let mut rows = Vec::new();
            if (*kind == "edition" && path.ends_with("/edition.json"))
                || (*kind == "item" && path.ends_with("/item.json"))
            {
                let record = cmd::parse(&write.exact_bytes).map_err(crate::DurableError::Source)?;
                rows.push(
                    crate::source_claims::pg_catalogue_record_value(
                        &record,
                        path,
                        Some("ToS/contracts/corpus-record.schema.json"),
                    )
                    .map_err(crate::DurableError::Source)?,
                );
                projected_profile_paths.insert(path.to_owned());
            }
            records.push((JsonString::from_utf8(*kind), JsonValue::Array(rows)));
        }
        records.sort_by(|(left, _), (right, _)| left.as_str().cmp(&right.as_str()));
        let events = if path.ends_with("/source-create-provenance.jsonl")
            || path.ends_with("/provenance.jsonl")
        {
            let mut values = Vec::new();
            for (line, raw) in write.exact_bytes.split(|byte| *byte == b'\n').enumerate() {
                if raw.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                let event = cmd::parse(raw).map_err(crate::DurableError::Source)?;
                let id = cmd::text(&event, "event_id")
                    .map_err(crate::DurableError::Source)?
                    .to_owned();
                let digest = cmd::record_digest(&event).map_err(crate::DurableError::Source)?;
                if !seen_events.insert(id.clone()) {
                    return Err(crate::DurableError::Corrupt(
                        "Item adoption event identity repeats",
                    ));
                }
                values.push((
                    JsonString::from_utf8(&id),
                    cmd::object(vec![
                        ("payload", event),
                        ("source_ref", cmd::string(path)),
                        (
                            "source_line",
                            cmd::number(u64::try_from(line + 1).map_err(|_| {
                                crate::DurableError::Corrupt("Item event line number overflow")
                            })?),
                        ),
                        ("source_sha256", cmd::string(&digest.to_hex())),
                    ]),
                ));
            }
            values.sort_by(|(left, _), (right, _)| left.as_str().cmp(&right.as_str()));
            JsonValue::Object(values)
        } else {
            JsonValue::Object(Vec::new())
        };
        let form = if path.ends_with(".human-forms.json") {
            let set = cmd::parse(&write.exact_bytes).map_err(crate::DurableError::Source)?;
            let ids = crate::source_forms::projection_form_ids(&set)
                .map_err(crate::DurableError::Source)?;
            for id in &ids {
                let id = id.as_str().ok_or(crate::DurableError::Corrupt(
                    "Item adoption form identity is malformed",
                ))?;
                if !package_form_ids.insert(id.to_owned()) {
                    return Err(crate::DurableError::Corrupt(
                        "Item adoption form identity repeats across package members",
                    ));
                }
            }
            projected_profile_paths.insert(path.to_owned());
            cmd::object(vec![
                (
                    "raw_sha256",
                    cmd::string(&Digest256::of_bytes(&write.exact_bytes).to_prefixed()),
                ),
                ("form_ids", JsonValue::Array(ids)),
            ])
        } else {
            JsonValue::Null
        };
        let claim_owner = if path.ends_with("/source-claims.jsonl") && path.contains("/items/") {
            if std::mem::replace(&mut seen_claim_owner, true) {
                return Err(crate::DurableError::Corrupt(
                    "Item adoption Claim owner contribution repeats",
                ));
            }
            projected_profile_paths.insert(path.to_owned());
            item_claim_owner.clone()
        } else {
            JsonValue::Null
        };
        let source_profiles = if projected_profile_paths.contains(path) {
            full_profiles.clone()
        } else {
            base_profiles.clone()
        };
        let envelope = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_pg_source_inventory_member_contribution_v1"),
            ),
            ("records", JsonValue::Object(records)),
            ("source_profiles", source_profiles),
            ("events", events),
            ("anchors", JsonValue::Object(Vec::new())),
            ("form", form),
            ("claim_owner", claim_owner),
        ]);
        contributions.insert(path.to_owned(), envelope);
    }
    if !seen_claim_owner || seen_events.len() != 3 {
        return Err(crate::DurableError::Corrupt(
            "Item adoption owner contribution package is incomplete",
        ));
    }
    Ok(contributions)
}

fn json_lines(raw: &[u8]) -> SourceCommandResult<Vec<JsonValue>> {
    raw.split(|byte| *byte == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .map(cmd::parse)
        .collect()
}
