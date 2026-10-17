//! Selected-input PG preparation for the ObjectLink creation owner.
//!
//! This adapter reuses the same owner configuration, scope grammar, native
//! byte composer, Claim-local profile and process capture as the filesystem
//! route. It deliberately prepares creation only; the local ObjectLink
//! transaction's explicit resume/rollback recovery is not a PG attempt replay.

use super::*;
use crate::source_cohort::revision::{PgRecordRevisionIdentity, SourceRevisionReadSet};
use crate::source_cohort::{self, IndexRows, PreparedPgSourceRevision, SourceRevisionWrite};
use crate::source_command::PreparedCommand;
use crate::{DurableError, DurableResult, PredicateKind, PredicateRead};
use postgres::Transaction;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::{
    item_rules::{ItemLimits, ItemRefusal},
    record_biblio_cut::SourceCutInputWithIdentity,
    source_cut::{CandidateCutWorkerSchemaExecutor, CutSchemaExecutor},
};

pub(crate) const PG_FAMILY: &str = "tos-pg-maintained-growth-v1:native-object-link-create";
const PG_HANDLER: &str = "native-object-link-create";
const MAX_SELECTED_MEMBER: usize = 2_097_152;

#[derive(Clone, Debug)]
pub(crate) struct PgObjectLinkRequirements {
    homes: Vec<RelativePath>,
    unique: Vec<(String, String)>,
}

impl PgObjectLinkRequirements {
    pub(crate) fn homes(&self) -> &[RelativePath] {
        &self.homes
    }

    pub(crate) fn unique_indexes(&self) -> &[(String, String)] {
        &self.unique
    }
}

fn requirements(owner: &Owner) -> SourceCommandResult<PgObjectLinkRequirements> {
    let mut homes = BTreeSet::new();
    for key in ["link_source_path", "claim_source_path"] {
        homes.insert(relative(owner.home(key)?)?);
    }
    let mut identities = BTreeSet::from([
        cmd::text(&owner.scope, "link_id")?.to_owned(),
        cmd::text(&owner.scope, "claim_id")?.to_owned(),
        cmd::text(&owner.scope, "provenance_event_id")?.to_owned(),
    ]);
    for field in ["forms", "claim_forms"] {
        for form in cmd::array(&owner.request, field)? {
            identities.insert(cmd::text(form, "form_id")?.to_owned());
        }
    }
    // The PG index uses one global identity namespace across these kinds.
    // Absence is observed for every kind before generation, so cross-kind
    // collisions cannot slip through an owner-specific probe.
    let unique = identities
        .into_iter()
        .flat_map(|token| {
            ["metadata", "claim", "event", "anchor", "form"]
                .into_iter()
                .map(move |kind| (kind.to_owned(), token.clone()))
        })
        .collect();
    Ok(PgObjectLinkRequirements {
        homes: homes.into_iter().collect(),
        unique,
    })
}

pub(crate) fn pg_object_link_requirements(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgObjectLinkRequirements> {
    let owner = Owner::select(fs, ctx, false, limits, cancelled)?;
    requirements(&owner)
}

/// A selected-PG ObjectLink description or prepare preview. It carries the
/// exact source-read snapshot that produced the response, but no prepared
/// source package or publication authority.
pub(crate) struct PgObjectLinkPreview {
    response: JsonValue,
    materializations: JsonValue,
    identity: PgRecordRevisionIdentity,
    reads: Option<crate::source_cohort::SourceReads>,
    requirements: PgObjectLinkRequirements,
}

impl PgObjectLinkPreview {
    pub(crate) fn response(&self) -> &JsonValue {
        &self.response
    }

    pub(crate) fn bind_read_snapshot(
        &mut self,
        reads: crate::source_cohort::SourceReads,
    ) -> DurableResult<()> {
        if self.reads.is_some() {
            return Err(DurableError::Conflict(
                "ObjectLink PG preview read snapshot was already bound",
            ));
        }
        self.reads = Some(reads);
        Ok(())
    }

    /// Record the complete set of output-home and semantic-identity absences
    /// against the selected PG session. The caller captures the resulting
    /// read snapshot after this call and validates it before returning the
    /// preview.
    pub(crate) fn observe_requirements(
        &self,
        session: &mut crate::source_cohort::revision::SourceRevisionReadSession<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<()> {
        for home in &self.requirements.homes {
            session.observe_home_absence(home, deadline, cancelled)?;
        }
        for (kind, token) in &self.requirements.unique {
            session.observe_unique_index(kind, token, deadline, cancelled)?;
        }
        Ok(())
    }

    pub(crate) fn validate_readset(&self, readset: &SourceRevisionReadSet) -> DurableResult<()> {
        let Some(reads) = self.reads.as_ref() else {
            return Err(DurableError::Conflict(
                "ObjectLink PG preview has no final selected read snapshot",
            ));
        };
        if self.identity != readset.selection.identity() || !reads_equal(reads, &readset.reads) {
            return Err(DurableError::Conflict(
                "ObjectLink PG preview differs from its fenced readset",
            ));
        }
        for home in &self.requirements.homes {
            let token = home.as_str();
            if !readset.reads.reads.iter().any(|read| {
                matches!(read, PredicateRead::Generation { predicate, .. }
                    if predicate.kind == PredicateKind::Range
                        && predicate.owner == crate::source_cohort::OWNER
                        && predicate.scope == "source-home"
                        && predicate.token == token
                        && predicate.definition_version == readset.selection.cohort.definition_digest().to_hex())
            }) {
                return Err(DurableError::Conflict(
                    "ObjectLink PG preview source-home absence was not observed",
                ));
            }
        }
        for (kind, token) in &self.requirements.unique {
            if !readset.reads.reads.iter().any(|read| {
                matches!(read, PredicateRead::Generation { predicate, .. }
                    if predicate.kind == PredicateKind::Unique
                        && predicate.owner == crate::source_cohort::OWNER
                        && predicate.scope == *kind
                        && predicate.token == *token
                        && predicate.definition_version == readset.selection.cohort.definition_digest().to_hex())
            }) {
                return Err(DurableError::Conflict(
                    "ObjectLink PG preview identity absence was not observed",
                ));
            }
        }
        Ok(())
    }
}

fn selected_profiles<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    scope: &JsonValue,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let entity = super::serde_value(&cmd::parse(&selected_bytes(
        input,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        MAX_SELECTED_MEMBER,
        limits,
        cancelled,
    )?)?)?;
    let relations = super::serde_value(&cmd::parse(&selected_bytes(
        input,
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        MAX_SELECTED_MEMBER,
        limits,
        cancelled,
    )?)?)?;
    super::source_profiles(scope, &entity, &relations)
}

fn selected_result_fields<I: Copy + Eq>(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    publication_token: Option<&str>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    receipt: JsonValue,
    materializations: JsonValue,
) -> SourceCommandResult<JsonValue> {
    let (config, scope, digest) = configuration(fs, ctx, true, None, limits, cancelled)?;
    let profiles = selected_profiles(input, &scope, limits, cancelled)?;
    Ok(cmd::object(vec![
        ("schema_version", cmd::string("tos_object_link_result_v1")),
        ("authentication", cmd::string("local-unix-account")),
        ("owner_configuration", cmd::string(&digest)),
        ("operation", cmd::string(OPERATION)),
        (
            "command_operations",
            JsonValue::Array(
                ["describe", "prepare-create", OPERATION, RECOVERY]
                    .iter()
                    .map(|value| cmd::string(value))
                    .collect(),
            ),
        ),
        (
            "allowed_operations",
            cmd::field(&config, "allowed_operations")?.clone(),
        ),
        ("scope", scope),
        (
            "publication_snapshot",
            publication_token
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        ("source_profiles", profiles),
        ("receipt", receipt),
        ("replayed", JsonValue::Bool(false)),
        ("recovery", JsonValue::Null),
        ("materializations", materializations),
        ("grants_admission", JsonValue::Bool(false)),
    ]))
}

/// Describe the protected ObjectLink owner against the exact selected PG
/// registries. The enclosing session owns read capture and final freshness.
pub(crate) fn describe_pg_object_link(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<PgRecordRevisionIdentity>,
    publication_token: Option<&str>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgObjectLinkPreview> {
    let response = selected_result_fields(
        fs,
        ctx,
        input,
        publication_token,
        limits,
        cancelled,
        JsonValue::Null,
        JsonValue::Null,
    )?;
    Ok(PgObjectLinkPreview {
        response,
        materializations: JsonValue::Null,
        identity: *input.input_identity(),
        reads: None,
        requirements: PgObjectLinkRequirements {
            homes: Vec::new(),
            unique: Vec::new(),
        },
    })
}

fn selected_bytes<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    path: &str,
    max_bytes: usize,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    if !tos_source_store::is_authored_source_path_v1(path) {
        return Err(SourceCommandError::Denied(
            "ObjectLink authored dependency path",
        ));
    }
    let mut selected = None;
    input
        .with_current_member(
            path,
            max_bytes,
            limits.deadline,
            cancelled,
            &mut |meta, raw| {
                if meta.path != path || selected.is_some() || raw.len() > max_bytes {
                    return Err(ItemRefusal::Source(
                        "ObjectLink selected member identity or size differs".into(),
                    ));
                }
                selected = Some(raw.to_vec());
                Ok(())
            },
        )
        .map_err(error)?;
    selected.ok_or(SourceCommandError::Conflict(
        "ObjectLink dependency absent from selected PG source",
    ))
}

fn selected_context<I: Copy + Eq>(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    owner: &Owner,
    input: &dyn SourceCutInputWithIdentity<I>,
    schemas: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let publication_token = cmd::field(&owner.request, "expected_publication")?.as_str();
    let catalog = crate::source_creation_store::select_source_witness_catalog_from_input(
        input.source_input(),
        publication_token,
        limits.deadline,
        cancelled,
    )?;
    for id in [
        cmd::text(&owner.scope, "link_id")?,
        cmd::text(&owner.scope, "claim_id")?,
    ] {
        if catalog.record_ids.contains(id) || catalog.claim_ids.contains(id) {
            return Err(SourceCommandError::Conflict(
                "ObjectLink new identity occupied",
            ));
        }
    }
    let event_id = cmd::text(&owner.scope, "provenance_event_id")?;
    if catalog.entries.iter().any(|(_, entry)| {
        entry
            .object_get("provenance_event_ref")
            .and_then(JsonValue::as_str)
            == Some(event_id)
    }) {
        return Err(SourceCommandError::Conflict(
            "ObjectLink event identity occupied",
        ));
    }
    let subject_id = cmd::text(&owner.scope, "subject_id")?;
    let subject_kind = cmd::text(&owner.scope, "subject_record_type")?;
    let subject_path = cmd::text(&owner.scope, "subject_source_path")?;
    let entry = catalog
        .entries
        .iter()
        .find(|(kind, entry)| {
            kind == subject_kind
                && entry.object_get("record_id").and_then(JsonValue::as_str) == Some(subject_id)
        })
        .ok_or(SourceCommandError::Conflict(
            "ObjectLink catalog subject absent",
        ))?;
    let subject = cmd::field(&owner.request, "subject")?;
    if cmd::text(&entry.1, "source_record_ref")? != subject_path
        || cmd::text(&entry.1, "record_sha256")? != cmd::record_digest(subject)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink subject catalog stale",
        ));
    }
    let raw = selected_bytes(input, subject_path, MAX_SELECTED_MEMBER, limits, cancelled)?;
    if !cmd::same(&cmd::parse(&raw)?, subject)? {
        return Err(SourceCommandError::Conflict(
            "ObjectLink exact subject changed",
        ));
    }
    let mut digests = catalog.digests;
    digests.insert(subject_path.to_owned(), raw_hex(&raw));

    let mut selected_forms = BTreeSet::new();
    for field in ["forms", "claim_forms"] {
        for form in cmd::array(&owner.request, field)? {
            selected_forms.insert(cmd::text(form, "form_id")?.to_owned());
        }
    }
    let mut adjacent = BTreeSet::new();
    for (kind, entry) in &catalog.entries {
        if kind == "claim" {
            let path = cmd::text(entry, "source_claim_file_ref")?;
            if let Some(home) = path.strip_suffix("/source-claims.jsonl") {
                adjacent.insert(format!(
                    "{home}/source-claims.{}.human-forms.json",
                    Digest256::of_bytes(cmd::text(entry, "claim_id")?.as_bytes()).to_hex()
                ));
            }
        } else {
            let path = cmd::text(entry, "source_record_ref")?;
            if let Some(stem) = path.strip_suffix(".json") {
                adjacent.insert(format!("{stem}.human-forms.json"));
            }
        }
    }
    let mut form_bytes = 0usize;
    for path in adjacent {
        if input
            .path_presence(&path, limits.deadline, cancelled)
            .map_err(error)?
            .is_none()
        {
            continue;
        }
        let form_raw = selected_bytes(input, &path, MAX_SELECTED_MEMBER, limits, cancelled)?;
        form_bytes = form_bytes
            .checked_add(form_raw.len())
            .ok_or(SourceCommandError::Invalid("ObjectLink form cost overflow"))?;
        if form_bytes > 16_777_216 || digests.len() >= 384 {
            return Err(SourceCommandError::Invalid(
                "ObjectLink adjacent form budget",
            ));
        }
        let value = cmd::parse(&form_raw)?;
        tos_validation::source_forms::source_copy_kernel::validate_history(
            &value,
            cmd::field(&value, "subject")?,
        )
        .map_err(crate::source_forms::form_error)?;
        for section in ["forms", "prior_forms"] {
            for form in cmd::array(&value, section)? {
                if selected_forms.contains(cmd::text(form, "form_id")?) {
                    return Err(SourceCommandError::Conflict(
                        "ObjectLink form identity occupied",
                    ));
                }
            }
        }
        digests.insert(path, raw_hex(&form_raw));
    }

    let claim = cmd::field(&owner.request, "claim")?;
    if let Some(alternatives) = claim.object_get("alternative_claim_refs") {
        for alternative in alternatives
            .as_array()
            .ok_or(SourceCommandError::Invalid("ObjectLink alternative claims"))?
        {
            if !catalog.claim_ids.contains(
                alternative
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("ObjectLink alternative claim"))?,
            ) {
                return Err(SourceCommandError::Conflict(
                    "ObjectLink alternative claim absent",
                ));
            }
        }
    }
    let mut evidence = BTreeSet::from([cmd::text(&owner.scope, "observation_ref")?.to_owned()]);
    for (record, key) in [
        (cmd::field(&owner.request, "link")?, "source_refs"),
        (claim, "evidence_refs"),
        (claim, "counterevidence_refs"),
    ] {
        if let Some(values) = record.object_get(key) {
            for value in values
                .as_array()
                .ok_or(SourceCommandError::Invalid("ObjectLink evidence refs"))?
            {
                evidence.insert(
                    value
                        .as_str()
                        .ok_or(SourceCommandError::Invalid("ObjectLink evidence ref"))?
                        .to_owned(),
                );
            }
        }
    }
    for path in evidence {
        if path.starts_with("ToS/") {
            let evidence_raw =
                selected_bytes(input, &path, MAX_SELECTED_MEMBER, limits, cancelled)?;
            digests.insert(path, raw_hex(&evidence_raw));
        } else {
            tos_validation::native_compound::validate_object_link_address(&path).map_err(error)?;
        }
    }

    let mut claim_raw = cmd::canonical(claim)?;
    claim_raw.push(b'\n');
    let report = tos_validation::record_rules::validate_source_claim_from_selected_input(
        input, &claim_raw, schemas, limits, cancelled,
    )
    .map_err(error)?;
    if !report.issues.is_empty() || report.input_identity != *input.input_identity() {
        return Err(SourceCommandError::Invalid(
            "ObjectLink local claim profile",
        ));
    }
    let mut contracts = report
        .dependency_digests
        .into_iter()
        .map(|(path, digest)| (path, digest.to_hex()))
        .collect::<BTreeMap<_, _>>();
    let subject_schema = if subject_kind == "artifact" {
        match cmd::text(subject, "schema_version")? {
            "tos_artifact_source_witness_v1" => "ToS/contracts/artifact-source-witness.schema.json",
            "tos_artifact_source_witness_v2" => {
                "ToS/contracts/artifact-source-witness-v2.schema.json"
            }
            _ => return Err(SourceCommandError::Invalid("ObjectLink artifact schema")),
        }
    } else {
        "ToS/contracts/corpus-record.schema.json"
    };
    for path in [
        subject_schema,
        "ToS/contracts/source-link.schema.json",
        "ToS/contracts/object-link-claim-v2.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
        "ToS/contracts/human-form-template.schema.json",
        "ToS/contracts/provenance-event-v2.schema.json",
    ] {
        let contract_raw = selected_bytes(input, path, MAX_SELECTED_MEMBER, limits, cancelled)?;
        contracts.insert(path.to_owned(), raw_hex(&contract_raw));
    }
    let mut implementation = BTreeMap::new();
    for path in IMPLEMENTATIONS {
        let raw = ctx
            .file(&relative(path)?)?
            .ok_or(SourceCommandError::Unsupported(
                "ObjectLink implementation subset incomplete",
            ))?;
        if raw.len() > MAX_SELECTED_MEMBER {
            return Err(SourceCommandError::Invalid(
                "ObjectLink software byte budget",
            ));
        }
        implementation.insert((*path).to_owned(), raw_hex(raw));
    }
    let _ = fs;
    Ok(cmd::object(vec![
        ("catalog_and_sources", digest_map(&digests)),
        ("contracts", digest_map(&contracts)),
        ("implementation", digest_map(&implementation)),
        ("retained_transactions", cmd::object(vec![])),
    ]))
}

fn json_lines(raw: &[u8], label: &'static str) -> SourceCommandResult<Vec<JsonValue>> {
    raw.split(|byte| *byte == b'\n')
        .filter(|line| {
            line.iter()
                .any(|byte| !matches!(*byte, b' ' | b'\t' | b'\r' | b'\n'))
        })
        .map(|line| {
            let end = line
                .iter()
                .rposition(|byte| !matches!(*byte, b' ' | b'\t' | b'\r' | b'\n'))
                .map_or(0, |index| index + 1);
            cmd::parse(&line[..end]).map_err(|_| SourceCommandError::Invalid(label))
        })
        .collect()
}

fn generated_form_indexes(
    request: &JsonValue,
    path: &str,
    raw: Option<&Vec<u8>>,
    field: &str,
) -> SourceCommandResult<IndexRows> {
    let selected = cmd::array(request, field)?
        .iter()
        .map(|form| cmd::text(form, "form_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let mut actual = BTreeSet::new();
    if let Some(raw) = raw {
        let value = cmd::parse(raw)?;
        actual = crate::source_forms::projection_form_ids(&value)?
            .into_iter()
            .map(|id| {
                id.as_str()
                    .map(str::to_owned)
                    .ok_or(SourceCommandError::Invalid(
                        "ObjectLink generated form identity is malformed",
                    ))
            })
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    }
    if actual != selected {
        return Err(SourceCommandError::Conflict(
            "ObjectLink generated forms differ from selected request",
        ));
    }
    Ok(actual
        .into_iter()
        .map(|id| ("form".to_owned(), id, path.to_owned()))
        .collect())
}

fn generated_semantic_indexes(
    owner: &Owner,
    files: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<IndexRows> {
    let link_id = cmd::text(&owner.scope, "link_id")?.to_owned();
    let claim_id = cmd::text(&owner.scope, "claim_id")?.to_owned();
    let event_id = cmd::text(&owner.scope, "provenance_event_id")?.to_owned();
    let link_path = cmd::text(&owner.scope, "link_source_path")?.to_owned();
    let claim_path = cmd::text(&owner.scope, "claim_source_path")?.to_owned();
    let link_home = link_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("ObjectLink link path"))?
        .0;
    let claim_home = claim_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("ObjectLink claim path"))?
        .0;
    let link = cmd::parse(files.get(&link_path).ok_or(SourceCommandError::Invalid(
        "ObjectLink package omits link metadata",
    ))?)?;
    if cmd::text(&link, "record_id")? != link_id {
        return Err(SourceCommandError::Conflict(
            "ObjectLink generated metadata identity differs",
        ));
    }
    let claim_raw = files.get(&claim_path).ok_or(SourceCommandError::Invalid(
        "ObjectLink package omits claim metadata",
    ))?;
    let claims = json_lines(claim_raw, "ObjectLink generated Claim encoding")?;
    if claims.len() != 1 || cmd::text(&claims[0], "claim_id")? != claim_id {
        return Err(SourceCommandError::Conflict(
            "ObjectLink generated Claim identity differs",
        ));
    }
    let event_path = format!("{claim_home}/source-create-provenance.jsonl");
    let events = json_lines(
        files.get(&event_path).ok_or(SourceCommandError::Invalid(
            "ObjectLink package omits provenance event",
        ))?,
        "ObjectLink generated event encoding",
    )?;
    if events.len() != 1 || cmd::text(&events[0], "event_id")? != event_id {
        return Err(SourceCommandError::Conflict(
            "ObjectLink generated event identity differs",
        ));
    }

    let link_forms_path = format!("{link_home}/link.human-forms.json");
    let claim_forms_path = format!(
        "{claim_home}/source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
    );
    let mut indexes = IndexRows::from([
        ("metadata".to_owned(), link_id, link_path),
        ("claim".to_owned(), claim_id.clone(), claim_path),
        ("event".to_owned(), event_id, event_path),
    ]);
    indexes.extend(generated_form_indexes(
        &owner.request,
        &link_forms_path,
        files.get(&link_forms_path),
        "forms",
    )?);
    indexes.extend(generated_form_indexes(
        &owner.request,
        &claim_forms_path,
        files.get(&claim_forms_path),
        "claim_forms",
    )?);
    let mut identities = BTreeSet::new();
    if indexes
        .iter()
        .any(|(_, token, _)| !identities.insert(token.as_str()))
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink semantic index identities are not globally unique",
        ));
    }
    Ok(indexes)
}

fn sealed_package_corrupt() -> DurableError {
    DurableError::Corrupt("PG ObjectLink sealed package shape differs")
}

fn package_parse(raw: &[u8]) -> DurableResult<JsonValue> {
    cmd::parse(raw).map_err(|_| sealed_package_corrupt())
}

fn package_field<'a>(value: &'a JsonValue, key: &str) -> DurableResult<&'a JsonValue> {
    cmd::field(value, key).map_err(|_| sealed_package_corrupt())
}

fn package_text<'a>(value: &'a JsonValue, key: &str) -> DurableResult<&'a str> {
    cmd::text(value, key).map_err(|_| sealed_package_corrupt())
}

fn package_record_digest(value: &JsonValue) -> DurableResult<String> {
    cmd::record_digest(value)
        .map(|digest| digest.to_prefixed())
        .map_err(|_| sealed_package_corrupt())
}

fn package_exact_keys(value: &JsonValue, keys: &[&str]) -> DurableResult<()> {
    cmd::exact_keys(value, keys).map_err(|_| sealed_package_corrupt())
}

fn verify_record_reference(
    receipt: &JsonValue,
    key: &str,
    record: &JsonValue,
    id_key: &str,
    version_key: &str,
) -> DurableResult<()> {
    let reference = package_field(receipt, key)?;
    package_exact_keys(reference, &["id", "version", "digest"])?;
    let digest = package_record_digest(record)?;
    if package_text(reference, "id")? != package_text(record, id_key)?
        || cmd::integer(reference, "version").map_err(|_| sealed_package_corrupt())?
            != cmd::integer(record, version_key).map_err(|_| sealed_package_corrupt())?
        || package_text(reference, "digest")? != digest.as_str()
    {
        return Err(sealed_package_corrupt());
    }
    Ok(())
}

fn verify_form_references(
    receipt_forms: &JsonValue,
    request: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    path: &str,
    request_key: &str,
    receipt_key: &str,
) -> DurableResult<()> {
    let raw = files.get(path).ok_or_else(sealed_package_corrupt)?;
    let form_set = package_parse(raw)?;
    let _ = crate::source_forms::projection_form_ids(&form_set)
        .map_err(|_| sealed_package_corrupt())?;
    let mut actual = BTreeMap::new();
    for form in cmd::array(&form_set, "forms").map_err(|_| sealed_package_corrupt())? {
        let id = package_text(form, "form_id")?.to_owned();
        if actual.insert(id, form).is_some() {
            return Err(sealed_package_corrupt());
        }
    }
    let requested = cmd::array(request, request_key).map_err(|_| sealed_package_corrupt())?;
    let requested_ids = requested
        .iter()
        .map(|row| package_text(row, "form_id").map(str::to_owned))
        .collect::<DurableResult<BTreeSet<_>>>()?;
    if requested_ids.len() != requested.len() || requested_ids != actual.keys().cloned().collect() {
        return Err(sealed_package_corrupt());
    }
    let references =
        cmd::array(receipt_forms, receipt_key).map_err(|_| sealed_package_corrupt())?;
    if references.len() != actual.len() {
        return Err(sealed_package_corrupt());
    }
    let mut seen = BTreeSet::new();
    for reference in references {
        package_exact_keys(reference, &["id", "version", "digest"])?;
        let id = package_text(reference, "id")?;
        let form = actual.get(id).ok_or_else(sealed_package_corrupt)?;
        let digest = package_record_digest(form)?;
        if !seen.insert(id.to_owned())
            || cmd::integer(reference, "version").map_err(|_| sealed_package_corrupt())?
                != cmd::integer(form, "form_version").map_err(|_| sealed_package_corrupt())?
            || package_text(reference, "digest")? != digest.as_str()
        {
            return Err(sealed_package_corrupt());
        }
    }
    Ok(())
}

/// Re-derive the exact path and semantic index rows from an immutable
/// ObjectLink package. The writer and recovery path share this validator so a
/// sealed source attempt cannot claim identities that its package does not
/// actually publish.
pub(crate) fn expected_revision_indexes_from_writes(
    writes: &[SourceRevisionWrite],
) -> DurableResult<IndexRows> {
    if writes.is_empty() || writes.len() > 8 {
        return Err(sealed_package_corrupt());
    }
    let mut files = BTreeMap::new();
    for write in writes {
        let parsed = RelativePath::parse(&write.subject).map_err(|_| sealed_package_corrupt())?;
        if parsed.as_str() != write.subject
            || !tos_source_store::is_authored_source_path_v1(&write.subject)
            || write.expected_predecessor.is_some()
            || write.proposed_revision == 0
            || files
                .insert(write.subject.clone(), write.exact_bytes.clone())
                .is_some()
        {
            return Err(sealed_package_corrupt());
        }
    }
    let receipts = files
        .iter()
        .filter(|(path, _)| path.ends_with(&format!("/{RECEIPT}")))
        .collect::<Vec<_>>();
    if receipts.len() != 1 {
        return Err(sealed_package_corrupt());
    }
    let (receipt_path, receipt_raw) = receipts[0];
    let receipt = package_parse(receipt_raw)?;
    package_exact_keys(
        &receipt,
        &[
            "schema_version",
            "operation",
            "transaction_id",
            "command_id",
            "request_digest",
            "owner_configuration",
            "principal_id",
            "authority_ref",
            "recorded_at",
            "scope",
            "dependencies",
            "subject",
            "subject_source_sha256",
            "link",
            "claim",
            "forms",
            "files",
            "grants_admission",
        ],
    )?;
    if package_text(&receipt, "schema_version")? != "tos_object_link_receipt_v1"
        || package_text(&receipt, "operation")? != OPERATION
        || package_field(&receipt, "grants_admission")?.as_bool() != Some(false)
        || package_text(&receipt, "recorded_at")?.is_empty()
    {
        return Err(sealed_package_corrupt());
    }
    let scope = package_field(&receipt, "scope")?.clone();
    let scope_value = serde_value(&scope).map_err(|_| sealed_package_corrupt())?;
    tos_validation::native_compound::validate_object_link_scope(&scope_value)
        .map_err(|_| sealed_package_corrupt())?;
    let link_path = package_text(&scope, "link_source_path")?.to_owned();
    let claim_path = package_text(&scope, "claim_source_path")?.to_owned();
    let link_home = link_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or_else(sealed_package_corrupt)?;
    let claim_home = claim_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or_else(sealed_package_corrupt)?;
    let expected_receipt_path = format!("{claim_home}/{RECEIPT}");
    let request_path = format!("{claim_home}/source-create-request.json");
    let environment_path = format!("{claim_home}/source-create-environment.json");
    let event_path = format!("{claim_home}/source-create-provenance.jsonl");
    let link_forms_path = format!("{link_home}/link.human-forms.json");
    let claim_id = package_text(&scope, "claim_id")?;
    let claim_forms_path = format!(
        "{claim_home}/source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
    );
    let expected_paths = BTreeSet::from([
        link_path.clone(),
        claim_path.clone(),
        request_path.clone(),
        environment_path.clone(),
        event_path.clone(),
        link_forms_path.clone(),
        claim_forms_path.clone(),
        expected_receipt_path.clone(),
    ]);
    if receipt_path.as_str() != expected_receipt_path.as_str()
        || files.keys().cloned().collect::<BTreeSet<_>>() != expected_paths
    {
        return Err(sealed_package_corrupt());
    }

    // The receipt lists every output except itself and binds the precise bytes
    // and sizes later carried in source-revision writes.
    let listed = package_field(&receipt, "files")?;
    let listed_rows = listed.as_object().ok_or_else(sealed_package_corrupt)?;
    if listed_rows.len() + 1 != files.len() {
        return Err(sealed_package_corrupt());
    }
    for (path, row) in listed_rows {
        let path = path.as_str().ok_or_else(sealed_package_corrupt)?;
        if path == expected_receipt_path {
            return Err(sealed_package_corrupt());
        }
        let raw = files.get(path).ok_or_else(sealed_package_corrupt)?;
        package_exact_keys(row, &["sha256", "bytes"])?;
        let digest = Digest256::of_bytes(raw).to_prefixed();
        if package_text(row, "sha256")? != digest.as_str()
            || cmd::integer(row, "bytes").map_err(|_| sealed_package_corrupt())?
                != u64::try_from(raw.len()).map_err(|_| sealed_package_corrupt())?
        {
            return Err(sealed_package_corrupt());
        }
    }
    if expected_paths
        .iter()
        .filter(|path| path.as_str() != expected_receipt_path.as_str())
        .any(|path| listed.object_get(path).is_none())
    {
        return Err(sealed_package_corrupt());
    }

    let request = package_parse(
        files
            .get(&request_path)
            .ok_or_else(sealed_package_corrupt)?,
    )?;
    package_exact_keys(
        &request,
        &[
            "schema_version",
            "operation",
            "subject",
            "link",
            "claim",
            "forms",
            "claim_forms",
            "reason",
            "command_id",
            "expected_configuration",
            "expected_dependencies",
            "expected_publication",
        ],
    )?;
    if package_text(&request, "schema_version")? != REQUEST
        || package_text(&request, "operation")? != OPERATION
        || package_text(&request, "command_id")? != package_text(&receipt, "command_id")?
        || package_field(&request, "expected_configuration")?
            != package_field(&receipt, "owner_configuration")?
        || package_field(&request, "expected_dependencies")?
            != package_field(&receipt, "dependencies")?
        || package_record_digest(&request)? != package_text(&receipt, "request_digest")?
    {
        return Err(sealed_package_corrupt());
    }
    let reason = package_text(&request, "reason")?;
    if reason.trim().is_empty() || reason.chars().count() > 4096 {
        return Err(sealed_package_corrupt());
    }
    let owner_configuration = package_text(&receipt, "owner_configuration")?;
    let request_digest = package_text(&receipt, "request_digest")?;
    for digest in [
        owner_configuration,
        request_digest,
        package_text(&receipt, "dependencies")?,
    ] {
        Digest256::from_prefixed(digest).map_err(|_| sealed_package_corrupt())?;
    }
    for identity in [
        package_text(&receipt, "principal_id")?,
        package_text(&receipt, "authority_ref")?,
    ] {
        if identity.trim().is_empty() {
            return Err(sealed_package_corrupt());
        }
    }
    if let Some(publication) = package_field(&request, "expected_publication")?.as_str() {
        Digest256::from_prefixed(publication).map_err(|_| sealed_package_corrupt())?;
    }
    let transaction_seed = cmd::object(vec![
        ("operation", cmd::string(OPERATION)),
        ("command_id", package_field(&request, "command_id")?.clone()),
        (
            "owner_configuration",
            package_field(&request, "expected_configuration")?.clone(),
        ),
        ("request_digest", cmd::string(request_digest)),
    ]);
    let transaction_id = Digest256::of_bytes(
        &cmd::canonical(&transaction_seed).map_err(|_| sealed_package_corrupt())?,
    )
    .to_prefixed();
    if package_text(&receipt, "transaction_id")? != transaction_id.as_str() {
        return Err(sealed_package_corrupt());
    }

    let claims = json_lines(
        files.get(&claim_path).ok_or_else(sealed_package_corrupt)?,
        "PG ObjectLink Claim package",
    )
    .map_err(|_| sealed_package_corrupt())?;
    if claims.len() != 1 {
        return Err(sealed_package_corrupt());
    }
    let request_claim_digest = package_record_digest(package_field(&request, "claim")?)?;
    if package_record_digest(&claims[0])? != request_claim_digest {
        return Err(sealed_package_corrupt());
    }
    let link = package_parse(files.get(&link_path).ok_or_else(sealed_package_corrupt)?)?;
    let request_link_digest = package_record_digest(package_field(&request, "link")?)?;
    if package_record_digest(&link)? != request_link_digest {
        return Err(sealed_package_corrupt());
    }
    let events = json_lines(
        files.get(&event_path).ok_or_else(sealed_package_corrupt)?,
        "PG ObjectLink event package",
    )
    .map_err(|_| sealed_package_corrupt())?;
    if events.len() != 1
        || package_text(&events[0], "event_id")? != package_text(&scope, "provenance_event_id")?
    {
        return Err(sealed_package_corrupt());
    }
    let environment = package_parse(
        files
            .get(&environment_path)
            .ok_or_else(sealed_package_corrupt)?,
    )?;
    package_exact_keys(
        &environment,
        &[
            "runtime",
            "runtime_version",
            "runtime_artifact_sha256",
            "backend",
            "hardware_target",
            "unicode_version",
            "argv_sha256",
        ],
    )?;
    for field in [
        "runtime",
        "runtime_version",
        "runtime_artifact_sha256",
        "backend",
        "hardware_target",
        "unicode_version",
        "argv_sha256",
    ] {
        if package_text(&environment, field)?.is_empty() {
            return Err(sealed_package_corrupt());
        }
    }
    for field in ["runtime_artifact_sha256", "argv_sha256"] {
        Digest256::from_hex(package_text(&environment, field)?)
            .map_err(|_| sealed_package_corrupt())?;
    }

    let maker_type = package_text(
        package_field(package_field(&request, "claim")?, "maker")?,
        "maker_type",
    )?;
    let authorization = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_object_link_authorization_v1"),
        ),
        ("scope", scope.clone()),
        (
            "principal_id",
            package_field(&receipt, "principal_id")?.clone(),
        ),
        ("maker_type", cmd::string(maker_type)),
        (
            "authority_ref",
            package_field(&receipt, "authority_ref")?.clone(),
        ),
        (
            "owner_configuration",
            package_field(&receipt, "owner_configuration")?.clone(),
        ),
        ("command_id", package_field(&request, "command_id")?.clone()),
        ("request_digest", cmd::string(request_digest)),
        ("dependency_bindings", cmd::object(Vec::new())),
    ]);
    let scope_value = serde_value(&scope).map_err(|_| sealed_package_corrupt())?;
    let request_value = serde_value(&request).map_err(|_| sealed_package_corrupt())?;
    let authorization_value = serde_value(&authorization).map_err(|_| sealed_package_corrupt())?;
    tos_validation::native_compound::validate_object_link_recovery_scope(
        &scope_value,
        &request_value,
        &authorization_value,
    )
    .map_err(|_| sealed_package_corrupt())?;
    verify_record_reference(
        &receipt,
        "subject",
        package_field(&request, "subject")?,
        if package_text(&scope, "subject_record_type")? == "artifact" {
            "artifact_id"
        } else {
            "record_id"
        },
        "record_version",
    )?;
    verify_record_reference(&receipt, "link", &link, "record_id", "record_version")?;
    verify_record_reference(&receipt, "claim", &claims[0], "claim_id", "claim_version")?;
    Digest256::from_hex(package_text(&receipt, "subject_source_sha256")?)
        .map_err(|_| sealed_package_corrupt())?;
    let forms = package_field(&receipt, "forms")?;
    package_exact_keys(forms, &["link", "claim"])?;
    verify_form_references(forms, &request, &files, &link_forms_path, "forms", "link")?;
    verify_form_references(
        forms,
        &request,
        &files,
        &claim_forms_path,
        "claim_forms",
        "claim",
    )?;

    let owner = Owner {
        frozen_authorization: None,
        config: cmd::object(Vec::new()),
        request,
        scope,
        digest: owner_configuration.to_owned(),
    };
    let semantic =
        generated_semantic_indexes(&owner, &files).map_err(|_| sealed_package_corrupt())?;
    let mut indexes = files
        .keys()
        .map(|path| ("path".to_owned(), path.clone(), path.clone()))
        .collect::<IndexRows>();
    indexes.extend(semantic);
    Ok(indexes)
}

/// Rebuild the full mixed-inventory contribution from exact sealed
/// ObjectLink bytes and the selected source-profile inputs.
pub(crate) fn expected_inventory_contributions_from_writes(
    writes: &[SourceRevisionWrite],
    context: &CommandContext,
    response: &JsonValue,
) -> DurableResult<BTreeMap<String, JsonValue>> {
    expected_revision_indexes_from_writes(writes)?;
    let receipt = writes
        .iter()
        .find(|write| write.subject.ends_with(&format!("/{RECEIPT}")))
        .ok_or_else(sealed_package_corrupt)?;
    let receipt_value = package_parse(&receipt.exact_bytes)?;
    let scope = package_field(&receipt_value, "scope")?;
    let link_path = package_text(scope, "link_source_path")?.to_owned();
    let claim_path = package_text(scope, "claim_source_path")?.to_owned();
    let claim_home = claim_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or_else(sealed_package_corrupt)?;
    let event_path = format!("{claim_home}/source-create-provenance.jsonl");
    let link_write = writes
        .iter()
        .find(|write| write.subject == link_path)
        .ok_or_else(sealed_package_corrupt)?;
    let link = package_parse(&link_write.exact_bytes)?;
    let source_profiles = package_field(response, "source_profiles")?;
    let link_profile = package_field(source_profiles, "link")?;
    let schema_ref = package_text(link_profile, "schema_ref")?.to_owned();
    let records = [crate::source_creation_store::PgGrowthRecordProjection {
        kind: "link".to_owned(),
        path: link_path,
        record: link,
        schema_ref: Some(schema_ref),
    }];
    crate::source_creation_store::pg_growth_owner_contributions(
        writes,
        &context.files,
        &records,
        &event_path,
        &claim_path,
        "tos_managed_object_link_claim_owner_inventory_v1",
    )
}

pub(crate) struct PreparedPgObjectLink<'a> {
    filesystem: &'a CreationFilesystem,
    context: CommandContext,
    proposal: PreparedCommand,
    identity: PgRecordRevisionIdentity,
    family_tag: String,
    reads: Option<crate::source_cohort::SourceReads>,
    requirements: PgObjectLinkRequirements,
    semantic_indexes: IndexRows,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl PreparedPgObjectLink<'_> {
    /// Bind all PG reads after catalog, subject, form, Claim and composer
    /// preparation has finished. The caller captures this while the same
    /// `SourceRevisionReadSession` is still held.
    pub(crate) fn bind_read_snapshot(
        &mut self,
        reads: crate::source_cohort::SourceReads,
    ) -> DurableResult<()> {
        if self.reads.is_some() {
            return Err(DurableError::Conflict(
                "ObjectLink PG read snapshot was already bound",
            ));
        }
        self.reads = Some(reads);
        Ok(())
    }
}

pub(crate) fn prepare_pg_object_link<'a>(
    fs: &'a CreationFilesystem,
    ctx: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<PgRecordRevisionIdentity>,
    schemas: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
) -> SourceCommandResult<PreparedPgObjectLink<'a>> {
    let owner = Owner::select(fs, ctx, false, limits, cancelled)?;
    let requirements = requirements(&owner)?;
    let dependencies = selected_context(fs, ctx, &owner, input, schemas, limits, cancelled)?;
    if cmd::record_digest(&dependencies)?.to_prefixed()
        != cmd::text(&owner.request, "expected_dependencies")?
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink prepared dependencies changed",
        ));
    }
    let authorization = owner.authorization(dependencies)?;
    let request_raw = {
        let mut raw = cmd::canonical(&owner.request)?;
        raw.push(b'\n');
        raw
    };
    let event_id = cmd::text(&owner.scope, "provenance_event_id")?.to_owned();
    let claim_home = owner.home("claim_source_path")?.to_owned();
    let mut capture_error = None;
    let prepared = tos_validation::native_compound::prepare_selected_native_object_link_bytes(
        input,
        schemas,
        &serde_value(&owner.scope)?,
        &serde_value(&authorization)?,
        &request_raw,
        &ctx.recorded_at,
        limits,
        cancelled,
        |outputs| {
            let rows = outputs
                .iter()
                .map(|(path, raw)| (path.as_str(), *raw))
                .collect::<Vec<_>>();
            let result = crate::source_serialization::capture_object_link(
                &owner.request,
                &event_id,
                &claim_home,
                &rows,
                software,
                components,
                limits.deadline,
                cancelled,
            );
            result
                .map(|capture| (capture.environment_raw, capture.event_raw))
                .map_err(|_| {
                    capture_error = Some(SourceCommandError::Invalid(
                        "ObjectLink process capture failed",
                    ));
                    ItemRefusal::Source("ObjectLink process capture failed".into())
                })
        },
    )
    .map_err(|refusal| capture_error.unwrap_or_else(|| error(refusal)))?;
    schemas.finish(limits.deadline, cancelled).map_err(error)?;

    let mut changes = Vec::with_capacity(prepared.files.len());
    for (path, raw) in &prepared.files {
        let presence = input
            .path_presence(path, limits.deadline, cancelled)
            .map_err(error)?;
        if presence.is_some() {
            return Err(SourceCommandError::Conflict(
                "ObjectLink output path already exists in PG source",
            ));
        }
        changes.push(cmd::SourceChange {
            path: relative(path)?,
            before: None,
            after: Some(raw.clone()),
        });
    }
    let plan = ctx.plan(PG_HANDLER, foundation(&prepared.receipt)?, changes, false)?;
    let semantic_indexes = generated_semantic_indexes(&owner, &prepared.files)?;
    Ok(PreparedPgObjectLink {
        filesystem: fs,
        context: ctx.clone(),
        proposal: plan,
        identity: *input.input_identity(),
        family_tag: PG_FAMILY.to_owned(),
        reads: None,
        requirements,
        semantic_indexes,
        deadline: limits.deadline,
        cancelled,
    })
}

/// Prepare a PG-backed ObjectLink preview through the maintained native
/// composer. This returns semantic response and HumanForm materializations
/// only; the generated source package is neither returned nor registered.
pub(crate) fn prepare_pg_object_link_preview(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<PgRecordRevisionIdentity>,
    schemas: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    publication_token: Option<&str>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgObjectLinkPreview> {
    let mut owner = Owner::select(fs, ctx, true, limits, cancelled)?;
    let requirements = requirements(&owner)?;
    cmd::set(&mut owner.request, "operation", cmd::string(OPERATION))?;
    cmd::set(
        &mut owner.request,
        "command_id",
        cmd::string("preview:uncommitted"),
    )?;
    cmd::set(
        &mut owner.request,
        "expected_configuration",
        cmd::string(&owner.digest),
    )?;
    cmd::set(
        &mut owner.request,
        "expected_publication",
        publication_token
            .map(cmd::string)
            .unwrap_or(JsonValue::Null),
    )?;

    let dependencies = selected_context(fs, ctx, &owner, input, schemas, limits, cancelled)?;
    let dependency_digest = cmd::record_digest(&dependencies)?.to_prefixed();
    cmd::set(
        &mut owner.request,
        "expected_dependencies",
        cmd::string(&dependency_digest),
    )?;
    let authorization = owner.authorization(dependencies)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let event_id = cmd::text(&owner.scope, "provenance_event_id")?.to_owned();
    let claim_home = owner.home("claim_source_path")?.to_owned();
    let mut capture_error = None;
    let prepared = tos_validation::native_compound::prepare_selected_native_object_link_bytes(
        input,
        schemas,
        &serde_value(&owner.scope)?,
        &serde_value(&authorization)?,
        &request_raw,
        &ctx.recorded_at,
        limits,
        cancelled,
        |outputs| {
            let rows = outputs
                .iter()
                .map(|(path, raw)| (path.as_str(), *raw))
                .collect::<Vec<_>>();
            crate::source_serialization::capture_object_link(
                &owner.request,
                &event_id,
                &claim_home,
                &rows,
                software,
                components,
                limits.deadline,
                cancelled,
            )
            .map(|capture| (capture.environment_raw, capture.event_raw))
            .map_err(|_| {
                capture_error = Some(SourceCommandError::Invalid(
                    "ObjectLink process capture failed",
                ));
                ItemRefusal::Source("ObjectLink process capture failed".into())
            })
        },
    )
    .map_err(|refusal| capture_error.unwrap_or_else(|| error(refusal)))?;
    schemas.finish(limits.deadline, cancelled).map_err(error)?;

    for path in prepared.files.keys() {
        if input
            .path_presence(path, limits.deadline, cancelled)
            .map_err(error)?
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "ObjectLink output path already exists in PG source",
            ));
        }
    }
    let link_path = cmd::text(&owner.scope, "link_source_path")?;
    let claim_path = cmd::text(&owner.scope, "claim_source_path")?;
    let link_home = link_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("ObjectLink link path"))?
        .0;
    let claim_home = claim_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("ObjectLink claim path"))?
        .0;
    let link_forms_path = format!("{link_home}/link.human-forms.json");
    let claim_forms_path = format!(
        "{claim_home}/source-claims.{}.human-forms.json",
        Digest256::of_bytes(cmd::text(&owner.scope, "claim_id")?.as_bytes()).to_hex()
    );
    let link = cmd::parse(
        prepared
            .files
            .get(link_path)
            .ok_or(SourceCommandError::Invalid(
                "ObjectLink prepared link absent",
            ))?,
    )?;
    let claim_raw = prepared
        .files
        .get(claim_path)
        .ok_or(SourceCommandError::Invalid(
            "ObjectLink prepared Claim absent",
        ))?;
    let claims = json_lines(claim_raw, "ObjectLink prepared Claim encoding")?;
    if claims.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "ObjectLink prepared Claim count differs",
        ));
    }
    let link_forms = cmd::parse(prepared.files.get(&link_forms_path).ok_or(
        SourceCommandError::Invalid("ObjectLink prepared link forms absent"),
    )?)?;
    let claim_forms = cmd::parse(prepared.files.get(&claim_forms_path).ok_or(
        SourceCommandError::Invalid("ObjectLink prepared Claim forms absent"),
    )?)?;
    let forms = cmd::object(vec![("link", link_forms), ("claim", claim_forms)]);
    let materializations = super::materializations_from_outputs(&owner.request, &prepared.files)?;
    let mut response = selected_result_fields(
        fs,
        ctx,
        input,
        publication_token,
        limits,
        cancelled,
        foundation(&prepared.receipt)?,
        materializations.clone(),
    )?;
    cmd::set(&mut response, "prepared_link", link)?;
    let claim = claims
        .into_iter()
        .next()
        .ok_or(SourceCommandError::Conflict(
            "ObjectLink prepared Claim absent",
        ))?;
    cmd::set(&mut response, "prepared_claim", claim)?;
    cmd::set(&mut response, "prepared_forms", forms)?;
    cmd::set(
        &mut response,
        "prepared_materializations",
        materializations.clone(),
    )?;
    cmd::set(
        &mut response,
        "expected_publication",
        publication_token
            .map(cmd::string)
            .unwrap_or(JsonValue::Null),
    )?;
    cmd::set(
        &mut response,
        "expected_dependencies",
        cmd::string(&dependency_digest),
    )?;
    Ok(PgObjectLinkPreview {
        response,
        materializations,
        identity: *input.input_identity(),
        reads: None,
        requirements,
    })
}

fn reads_equal(
    left: &crate::source_cohort::SourceReads,
    right: &crate::source_cohort::SourceReads,
) -> bool {
    left.reads == right.reads
        && left.locators == right.locators
        && left.managed_original == right.managed_original
        && left.revision == right.revision
}

impl PreparedPgSourceRevision for PreparedPgObjectLink<'_> {
    fn context(&self) -> &CommandContext {
        &self.context
    }

    fn proposal(&self) -> &PreparedCommand {
        &self.proposal
    }

    fn identity(&self) -> PgRecordRevisionIdentity {
        self.identity
    }

    fn family_tag(&self) -> &str {
        &self.family_tag
    }

    fn legacy_family_encoding(&self) -> bool {
        false
    }

    fn validate_readset(&self, readset: &SourceRevisionReadSet) -> DurableResult<()> {
        let Some(reads) = self.reads.as_ref() else {
            return Err(DurableError::Conflict(
                "ObjectLink PG preparation has no final selected read snapshot",
            ));
        };
        if self.identity != readset.selection.identity()
            || self.context.base_revision != readset.selection.cohort.initial_revision()
            || self.proposal.base_revision != readset.selection.cohort.initial_revision()
            || self.proposal.operation != OPERATION
            || self.proposal.handler_id != PG_HANDLER
            || self.proposal.replayed
            || self.proposal.changes.is_empty()
            || self.family_tag != PG_FAMILY
            || !reads_equal(reads, &readset.reads)
        {
            return Err(DurableError::Conflict(
                "ObjectLink PG preparation differs from its fenced readset",
            ));
        }
        for home in &self.requirements.homes {
            let token = home.as_str();
            if !readset.reads.reads.iter().any(|read| {
                matches!(read, PredicateRead::Generation { predicate, .. }
                    if predicate.kind == PredicateKind::Range
                        && predicate.owner == crate::source_cohort::OWNER
                        && predicate.scope == "source-home"
                        && predicate.token == token
                        && predicate.definition_version == readset.selection.cohort.definition_digest().to_hex())
            }) {
                return Err(DurableError::Conflict(
                    "ObjectLink source-home absence was not observed",
                ));
            }
        }
        for (kind, token) in &self.requirements.unique {
            if !readset.reads.reads.iter().any(|read| {
                matches!(read, PredicateRead::Generation { predicate, .. }
                    if predicate.kind == PredicateKind::Unique
                        && predicate.owner == crate::source_cohort::OWNER
                        && predicate.scope == *kind
                        && predicate.token == *token
                        && predicate.definition_version == readset.selection.cohort.definition_digest().to_hex())
            }) {
                return Err(DurableError::Conflict(
                    "ObjectLink identity absence was not observed",
                ));
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
        self.filesystem
            .current_context(&self.context, self.deadline, self.cancelled)
            .map_err(DurableError::Source)?;
        if writes.len() != self.proposal.changes.len()
            || writes.iter().any(|write| {
                write.expected_predecessor.is_some()
                    || !self.proposal.changes.iter().any(|change| {
                        change.path.as_str() == write.subject
                            && change.before.is_none()
                            && change.after.as_deref() == Some(write.exact_bytes.as_slice())
                    })
            })
        {
            return Err(DurableError::Corrupt(
                "ObjectLink PG writes differ from prepared owner package",
            ));
        }
        let mut indexes = source_cohort::check_revision_path_indexes(tx, domain, writes)?;
        indexes.extend(self.semantic_indexes.iter().cloned());
        if indexes != expected_revision_indexes_from_writes(writes)? {
            return Err(DurableError::Corrupt(
                "ObjectLink PG index rows differ from sealed owner package",
            ));
        }
        Ok(indexes)
    }

    fn inventory_owner_contributions(
        &self,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<BTreeMap<String, JsonValue>> {
        expected_inventory_contributions_from_writes(writes, &self.context, &self.proposal.response)
    }
}
