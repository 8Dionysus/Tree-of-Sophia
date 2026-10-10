//! One protected Collection membership append. Shared transport moves exact
//! bytes; this owner alone constructs the Collection-specific authorization.

use super::work_expression::{
    allowed_forms, catalog_lines, checked_current_descriptor, checked_current_source, digest_map,
    known_catalog_kind, raw_hex, relative, selected_forms, slug,
};
use super::work_transaction::PublicationSnapshot;
use super::{CreationFilesystem, active, walk, work_transaction};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};

const CONFIG: &str = "tos_local_collection_membership_owner_v1";
const REQUEST: &str = "tos_local_collection_membership_command_v1";
const OPERATION: &str = "collection.work.attach";
const RECOVERY: &str = "collection.work.recover";
const AUTHORIZATION: &str = "tos_collection_membership_authorization_v1";
const SCOPE_KEYS: &[&str] = &[
    "collection_id",
    "collection_source_path",
    "work_id",
    "work_source_path",
    "predicate",
    "claim_id",
    "claim_source_path",
    "provenance_event_id",
    "allowed_collection_form_ids",
    "allowed_claim_form_ids",
    "allowed_evidence_refs",
    "retained_membership_provenance_refs",
];

struct CollectionOwner {
    configuration: JsonValue,
    request: JsonValue,
    collection_path: RelativePath,
    work_path: RelativePath,
    claim_path: RelativePath,
    collection_id: String,
    work_id: String,
    claim_id: String,
    configuration_digest: String,
}

fn bounded_strings(
    config: &JsonValue,
    key: &str,
    minimum: usize,
    maximum: usize,
) -> SourceCommandResult<BTreeSet<String>> {
    let rows = cmd::array(config, key)?;
    if !(minimum..=maximum).contains(&rows.len()) {
        return Err(SourceCommandError::Denied(
            "Collection bounded reference grant",
        ));
    }
    let mut result = BTreeSet::new();
    for row in rows {
        let text = row
            .as_str()
            .ok_or(SourceCommandError::Invalid("Collection reference"))?;
        if !cmd::nonblank(text)
            || key == "allowed_evidence_refs" && text.chars().count() > 4096
            || !result.insert(text.to_owned())
        {
            return Err(SourceCommandError::Denied(
                "Collection reference grant identity",
            ));
        }
    }
    Ok(result)
}

struct CollectionGrant {
    configuration: JsonValue,
    collection_path: RelativePath,
    work_path: RelativePath,
    claim_path: RelativePath,
    collection_forms: BTreeSet<String>,
    claim_forms: BTreeSet<String>,
    evidence: BTreeSet<String>,
}
impl CollectionGrant {
    fn select(
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        required_operation: Option<&str>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        fs.current_context(ctx, deadline, cancelled)?;
        let config = cmd::parse(&ctx.configuration_raw)?;
        let mut keys = SCOPE_KEYS.to_vec();
        keys.extend([
            "schema_version",
            "uid",
            "principal_id",
            "maker_type",
            "source_root",
            "authority_ref",
            "expires_at",
            "allowed_operations",
        ]);
        cmd::exact_keys(&config, &keys)?;
        if cmd::text(&config, "schema_version")? != CONFIG
            || cmd::integer(&config, "uid")? != u64::from(fs.uid)
            || Path::new(cmd::text(&config, "source_root")?) != fs.root_path
            || !cmd::nonblank(cmd::text(&config, "principal_id")?)
            || !cmd::nonblank(cmd::text(&config, "authority_ref")?)
            || !matches!(
                cmd::text(&config, "maker_type")?,
                "human" | "software" | "model"
            )
        {
            return Err(SourceCommandError::Denied(
                "Collection delegation root/account/schema",
            ));
        }
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        let operations = cmd::array(&config, "allowed_operations")?;
        let mut allowed = BTreeSet::new();
        if operations.len() > 2 {
            return Err(SourceCommandError::Denied("Collection operation count"));
        }
        for operation in operations {
            let operation = operation
                .as_str()
                .ok_or(SourceCommandError::Invalid("Collection operation"))?;
            if !matches!(operation, OPERATION | RECOVERY) || !allowed.insert(operation) {
                return Err(SourceCommandError::Denied("Collection operation grant"));
            }
        }
        if required_operation.is_some_and(|operation| !allowed.contains(operation)) {
            return Err(SourceCommandError::Denied(
                "Collection membership append not delegated",
            ));
        }
        for (key, prefix) in [
            ("collection_id", "tos.collection."),
            ("work_id", "tos.work."),
            ("claim_id", "tos.claim."),
            ("provenance_event_id", "tos.event."),
        ] {
            if !slug(cmd::text(&config, key)?, prefix) {
                return Err(SourceCommandError::Denied(
                    "Collection typed identity grant",
                ));
            }
        }
        let collection_path = relative(cmd::text(&config, "collection_source_path")?)?;
        let work_path = relative(cmd::text(&config, "work_source_path")?)?;
        let claim_path = relative(cmd::text(&config, "claim_source_path")?)?;
        for (path, district, basename) in [
            (&collection_path, "collections", "collection.json"),
            (&work_path, "works", "work.json"),
            (&claim_path, "relations", "source-claims.jsonl"),
        ] {
            let parts = path.as_str().split('/').collect::<Vec<_>>();
            if parts.len() < if district == "works" { 4 } else { 5 }
                || parts[..3] != ["ToS", "source-witnesses", district]
                || parts.last() != Some(&basename)
                || district == "relations" && (parts.len() != 5 || !slug(parts[3], ""))
            {
                return Err(SourceCommandError::Denied(
                    "Collection public exact endpoint homes",
                ));
            }
        }
        if cmd::text(&config, "predicate")? != "contains_work" {
            return Err(SourceCommandError::Denied(
                "Collection exact membership predicate",
            ));
        }
        let collection_forms = allowed_forms(&config, "allowed_collection_form_ids")?;
        let claim_forms = allowed_forms(&config, "allowed_claim_form_ids")?;
        if !collection_forms.is_disjoint(&claim_forms) {
            return Err(SourceCommandError::Denied(
                "Collection form identities overlap",
            ));
        }
        let evidence = bounded_strings(&config, "allowed_evidence_refs", 1, 128)?;
        for reference in bounded_strings(&config, "retained_membership_provenance_refs", 0, 32)? {
            let path = relative(&reference)?;
            let name = path.as_str().rsplit('/').next().unwrap_or("");
            if !name.ends_with(".jsonl") || !name.contains("provenance") {
                return Err(SourceCommandError::Denied(
                    "Collection retained provenance stream",
                ));
            }
        }
        Ok(Self {
            configuration: config,
            collection_path,
            work_path,
            claim_path,
            collection_forms,
            claim_forms,
            evidence,
        })
    }
}
impl CollectionOwner {
    fn select(
        fs: &CreationFilesystem,
        ctx: &CommandContext,
        proposal: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let grant = CollectionGrant::select(fs, ctx, Some(OPERATION), deadline, cancelled)?;
        Self::from_grant(grant, &ctx.request_raw, proposal, None)
    }
    fn from_grant(
        grant: CollectionGrant,
        request_raw: &[u8],
        proposal: bool,
        historical_configuration_digest: Option<&str>,
    ) -> SourceCommandResult<Self> {
        let CollectionGrant {
            configuration: config,
            collection_path,
            work_path,
            claim_path,
            collection_forms,
            claim_forms,
            evidence,
        } = grant;
        let request = cmd::parse(request_raw)?;
        let mut request_keys = vec![
            "schema_version",
            "operation",
            "work",
            "claim",
            "forms",
            "claim_forms",
            "reason",
        ];
        if !proposal {
            request_keys.extend([
                "command_id",
                "fields",
                "expected_configuration",
                "expected_source",
                "expected_revision",
                "expected_dependencies",
                "expected_publication",
            ]);
        }
        cmd::exact_keys(&request, &request_keys)?;
        if cmd::text(&request, "schema_version")? != REQUEST
            || cmd::text(&request, "operation")?
                != if proposal {
                    "prepare-attach"
                } else {
                    OPERATION
                }
            || cmd::canonical(&request)?.len() > 1_048_576
        {
            return Err(SourceCommandError::Invalid(
                "Collection request schema/operation/budget",
            ));
        }
        let reason = cmd::text(&request, "reason")?;
        if !cmd::nonblank(reason) || reason.trim().chars().count() > 4096 {
            return Err(SourceCommandError::Invalid("Collection reason"));
        }
        let configuration_digest = match historical_configuration_digest {
            Some(digest) if work_transaction::is_hash(digest) => digest.to_owned(),
            Some(_) => {
                return Err(SourceCommandError::Invalid(
                    "Collection retained owner digest",
                ));
            }
            None => cmd::record_digest(&config)?.to_prefixed(),
        };
        if !proposal {
            let id = cmd::text(&request, "command_id")?;
            if id.is_empty() || id.chars().count() > 256 {
                return Err(SourceCommandError::Invalid("Collection command identity"));
            }
            for key in [
                "expected_configuration",
                "expected_revision",
                "expected_dependencies",
            ] {
                if !work_transaction::is_hash(cmd::text(&request, key)?) {
                    return Err(SourceCommandError::Invalid("Collection expected digest"));
                }
            }
            if cmd::field(&request, "expected_publication")? != &JsonValue::Null
                && !cmd::field(&request, "expected_publication")?
                    .as_str()
                    .is_some_and(work_transaction::is_hash)
            {
                return Err(SourceCommandError::Invalid(
                    "Collection expected publication",
                ));
            }
            cmd::exact_keys(cmd::field(&request, "fields")?, &["membership_claim_refs"])?;
            if cmd::text(&request, "expected_configuration")? != configuration_digest {
                return Err(SourceCommandError::Conflict(
                    "Collection current owner digest differs",
                ));
            }
        }
        let work = cmd::field(&request, "work")?;
        let claim = cmd::field(&request, "claim")?;
        let collection_id = cmd::text(&config, "collection_id")?.to_owned();
        let work_id = cmd::text(&config, "work_id")?.to_owned();
        let claim_id = cmd::text(&config, "claim_id")?.to_owned();
        let maker = cmd::object(vec![
            ("maker_type", cmd::field(&config, "maker_type")?.clone()),
            ("agent_ref", cmd::field(&config, "principal_id")?.clone()),
        ]);
        if cmd::text(work, "record_type")? != "work"
            || cmd::text(work, "record_id")? != work_id
            || cmd::text(claim, "claim_id")? != claim_id
            || cmd::text(claim, "subject_ref")? != collection_id
            || cmd::text(claim, "object")? != work_id
            || cmd::text(claim, "predicate")? != "contains_work"
            || cmd::text(claim, "provenance_event_ref")?
                != cmd::text(&config, "provenance_event_id")?
            || !cmd::same(cmd::field(claim, "maker")?, &maker)?
        {
            return Err(SourceCommandError::Denied(
                "Collection packet endpoint/maker/event grant",
            ));
        }
        for key in ["evidence_refs", "counterevidence_refs"] {
            if let Some(rows) = claim.object_get(key) {
                let rows = rows
                    .as_array()
                    .ok_or(SourceCommandError::Invalid("Collection evidence list"))?;
                if rows
                    .iter()
                    .any(|row| !row.as_str().is_some_and(|text| evidence.contains(text)))
                {
                    return Err(SourceCommandError::Denied(
                        "Collection evidence leaves grant",
                    ));
                }
            }
        }
        selected_forms(&request, "forms", &collection_forms)?;
        selected_forms(&request, "claim_forms", &claim_forms)?;
        Ok(Self {
            configuration: config,
            request,
            collection_path,
            work_path,
            claim_path,
            collection_id,
            work_id,
            claim_id,
            configuration_digest,
        })
    }

    fn scope(&self) -> SourceCommandResult<JsonValue> {
        Ok(JsonValue::Object(
            self.configuration
                .as_object()
                .ok_or(SourceCommandError::Invalid("Collection scope"))?
                .iter()
                .filter(|(key, _)| key.as_str().is_some_and(|key| SCOPE_KEYS.contains(&key)))
                .cloned()
                .collect(),
        ))
    }

    fn authorization(&self, dependencies: JsonValue) -> SourceCommandResult<JsonValue> {
        Ok(cmd::object(vec![
            ("schema_version", cmd::string(AUTHORIZATION)),
            ("scope", self.scope()?),
            (
                "principal_id",
                cmd::field(&self.configuration, "principal_id")?.clone(),
            ),
            (
                "maker_type",
                cmd::field(&self.configuration, "maker_type")?.clone(),
            ),
            (
                "authority_ref",
                cmd::field(&self.configuration, "authority_ref")?.clone(),
            ),
            (
                "owner_configuration",
                cmd::string(&self.configuration_digest),
            ),
            (
                "command_id",
                cmd::field(&self.request, "command_id")?.clone(),
            ),
            (
                "request_digest",
                cmd::string(&cmd::record_digest(&self.request)?.to_prefixed()),
            ),
            ("dependency_bindings", dependencies),
        ]))
    }
}

impl CollectionOwner {
    fn transaction_id(&self) -> SourceCommandResult<String> {
        Ok(cmd::record_digest(&cmd::object(vec![
            ("operation", cmd::string(OPERATION)),
            (
                "command_id",
                cmd::field(&self.request, "command_id")?.clone(),
            ),
            (
                "owner_configuration",
                cmd::field(&self.request, "expected_configuration")?.clone(),
            ),
            (
                "request_digest",
                cmd::string(&cmd::record_digest(&self.request)?.to_prefixed()),
            ),
        ]))?
        .to_prefixed())
    }

    fn plan(
        &self,
        authorization: JsonValue,
        before: &BTreeMap<String, Vec<u8>>,
        parent: BTreeMap<String, Vec<u8>>,
        child: BTreeMap<String, Vec<u8>>,
        new_directories: Vec<RelativePath>,
    ) -> SourceCommandResult<work_transaction::WorkPlan> {
        let collection_home = self
            .collection_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Collection parent home"))?
            .0;
        let claim_home = self
            .claim_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Collection Claim home"))?
            .0;
        let parent_names = [
            "collection.json",
            "collection.human-forms.json",
            "source-revision-history.json",
        ];
        let claim_forms = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(self.claim_id.as_bytes()).to_hex()
        );
        let child_names = [
            "source-claims.jsonl",
            claim_forms.as_str(),
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
            "membership-attachment-receipt.json",
        ];
        if parent.len() != parent_names.len()
            || parent_names.iter().any(|name| !parent.contains_key(*name))
            || before
                .keys()
                .any(|name| !parent_names.contains(&name.as_str()))
            || child.len() != child_names.len()
            || child_names.iter().any(|name| !child.contains_key(*name))
            || new_directories != [relative(claim_home)?]
        {
            return Err(SourceCommandError::Invalid(
                "Collection exact selected package shape",
            ));
        }
        let mut files = Vec::with_capacity(parent.len() + child.len());
        for (name, raw) in parent {
            files.push(work_transaction::SelectedFile {
                path: relative(&format!("{collection_home}/{name}"))?,
                before: before.get(&name).cloned(),
                after: Some(raw),
            });
        }
        for (name, raw) in child {
            files.push(work_transaction::SelectedFile {
                path: relative(&format!("{claim_home}/{name}"))?,
                before: None,
                after: Some(raw),
            });
        }
        Ok(work_transaction::WorkPlan {
            transaction_id: self.transaction_id()?,
            authorization,
            item_path_profile: None,
            files,
            new_directories,
            source_readset: None,
            source_successor: None,
        })
    }
}

use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};

struct CollectionCatalogRows {
    records: BTreeMap<String, JsonValue>,
    claims: BTreeMap<String, JsonValue>,
    digests: BTreeMap<String, String>,
    source_bytes: usize,
}
fn catalog_rows(
    fs: &CreationFilesystem,
    publication_token: Option<&str>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionCatalogRows> {
    let catalog = walk(&fs.root, "ToS/source-witnesses/catalog", fs.uid)?;
    let manifest_raw = work_transaction::read_at(
        &catalog,
        "catalog.manifest.json",
        fs.uid,
        2_097_152,
        deadline,
        cancelled,
    )?
    .ok_or(SourceCommandError::Conflict(
        "Collection catalog manifest absent",
    ))?;
    let manifest = cmd::parse(&manifest_raw)?;
    if cmd::text(&manifest, "schema_version")? != "tos_source_witness_catalog_v3"
        || cmd::text(&manifest, "claim_file")? != "ToS/source-witnesses/catalog/claims.jsonl"
    {
        return Err(SourceCommandError::Unsupported(
            "Collection catalog route/version",
        ));
    }
    let record_files =
        cmd::field(&manifest, "record_files")?
            .as_object()
            .ok_or(SourceCommandError::Invalid(
                "Collection catalog record routes",
            ))?;
    if record_files.is_empty() || record_files.len() > 128 {
        return Err(SourceCommandError::Invalid(
            "Collection catalog route count",
        ));
    }
    let mut routes = Vec::with_capacity(record_files.len() + 1);
    for (kind, value) in record_files {
        let kind = kind
            .as_str()
            .ok_or(SourceCommandError::Invalid("catalog kind"))?;
        let reference = value
            .as_str()
            .ok_or(SourceCommandError::Invalid("catalog route"))?;
        let prefix = "ToS/source-witnesses/catalog/";
        if !reference.starts_with(prefix) || !known_catalog_kind(kind, &reference[prefix.len()..]) {
            return Err(SourceCommandError::Unsupported(
                "Collection catalog undeclared profile route",
            ));
        }
        routes.push((kind.to_owned(), reference.to_owned()));
    }
    routes.sort();
    routes.push((
        "claim".to_owned(),
        "ToS/source-witnesses/catalog/claims.jsonl".to_owned(),
    ));
    let mut digests = BTreeMap::new();
    let mut record_ids = BTreeSet::new();
    let mut claim_ids = BTreeSet::new();
    let mut records = BTreeMap::new();
    let mut claims = BTreeMap::new();
    let mut total_bytes = manifest_raw.len();
    let mut count = 0usize;
    for (kind, reference) in routes {
        active(deadline, cancelled)?;
        let leaf = reference
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("catalog leaf"))?
            .1;
        let raw =
            work_transaction::read_at(&catalog, leaf, fs.uid, 16_777_216, deadline, cancelled)?
                .ok_or(SourceCommandError::Conflict(
                    "Collection catalog route absent",
                ))?;
        total_bytes = total_bytes
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "Collection catalog aggregate overflow",
            ))?;
        if total_bytes > 16_777_216 {
            return Err(SourceCommandError::Invalid(
                "Collection catalog aggregate byte budget",
            ));
        }
        digests.insert(reference.clone(), raw_hex(&raw));
        for line in catalog_lines(&raw) {
            active(deadline, cancelled)?;
            count += 1;
            if count > 8192 {
                return Err(SourceCommandError::Invalid("Collection catalog row budget"));
            }
            let entry = cmd::parse(line)?;
            let id = if kind == "claim" {
                if cmd::text(&entry, "schema_version")?
                    != "tos_source_witness_claim_catalog_entry_v1"
                {
                    return Err(SourceCommandError::Invalid(
                        "Collection claim catalog schema",
                    ));
                }
                cmd::text(&entry, "claim_id")?
            } else {
                if cmd::text(&entry, "schema_version")? != "tos_source_witness_catalog_entry_v1"
                    || cmd::text(&entry, "record_type")? != kind
                {
                    return Err(SourceCommandError::Invalid(
                        "Collection record catalog schema",
                    ));
                }
                cmd::text(&entry, "record_id")?
            };
            if record_ids.contains(id)
                || claim_ids.contains(id)
                || !(if kind == "claim" {
                    &mut claim_ids
                } else {
                    &mut record_ids
                })
                .insert(id.to_owned())
            {
                return Err(SourceCommandError::Conflict(
                    "duplicate Collection catalog identity",
                ));
            }
            if kind == "claim" {
                claims.insert(id.to_owned(), entry);
            } else {
                records.insert(id.to_owned(), entry);
            }
        }
    }
    let binding = manifest.object_get("selected_metadata_publication");
    match (binding, publication_token) {
        (None, None) => (),
        (Some(binding), Some(token)) => {
            cmd::exact_keys(binding, &["protocol", "token", "files"])?;
            if cmd::text(binding, "protocol")? != "tos_selected_source_metadata_v1"
                || cmd::text(binding, "token")? != token
            {
                return Err(SourceCommandError::Conflict(
                    "Collection catalog publication token",
                ));
            }
            let rows =
                cmd::field(binding, "files")?
                    .as_object()
                    .ok_or(SourceCommandError::Invalid(
                        "Collection catalog publication file map",
                    ))?;
            if rows.len() != digests.len()
                || digests.iter().any(|(path, digest)| {
                    rows.iter()
                        .find(|(key, _)| key.as_str() == Some(path.as_str()))
                        .and_then(|(_, value)| value.as_str())
                        != Some(digest.as_str())
                })
            {
                return Err(SourceCommandError::Conflict(
                    "Collection catalog publication file closure",
                ));
            }
        }
        _ => {
            return Err(SourceCommandError::Conflict(
                "Collection catalog publication binding absent",
            ));
        }
    }
    digests.insert(
        "ToS/source-witnesses/catalog/catalog.manifest.json".to_owned(),
        raw_hex(&manifest_raw),
    );
    Ok(CollectionCatalogRows {
        records,
        claims,
        digests,
        source_bytes: total_bytes,
    })
}
fn source_dependency(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    path: &str,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let raw = checked_current_source(fs, cut, path, 2_097_152, deadline, cancelled)?;
    *total = total
        .checked_add(raw.len())
        .ok_or(SourceCommandError::Invalid(
            "Collection current source byte overflow",
        ))?;
    if *total > 33_554_432 || digests.len() >= 512 {
        return Err(SourceCommandError::Invalid(
            "Collection current source dependency budget",
        ));
    }
    if let Some(prior) = digests.insert(path.to_owned(), raw_hex(&raw)) {
        if prior != raw_hex(&raw) {
            return Err(SourceCommandError::Conflict(
                "Collection current dependency changed",
            ));
        }
    }
    Ok(raw)
}
fn catalog_claim(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    entry: &JsonValue,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let path = cmd::text(entry, "source_claim_file_ref")?;
    let raw = source_dependency(fs, cut, path, digests, total, deadline, cancelled)?;
    let line = usize::try_from(cmd::integer(entry, "source_claim_line")?)
        .map_err(|_| SourceCommandError::Invalid("Collection Claim line"))?
        .checked_sub(1)
        .ok_or(SourceCommandError::Invalid("Collection Claim line"))?;
    let claim = cmd::parse(
        catalog_lines(&raw)
            .nth(line)
            .ok_or(SourceCommandError::Conflict("Collection Claim line absent"))?,
    )?;
    for key in [
        "claim_id",
        "predicate",
        "object",
        "subject_ref",
        "provenance_event_ref",
    ] {
        if cmd::field(&claim, key)? != cmd::field(entry, key)? {
            return Err(SourceCommandError::Conflict(
                "Collection catalog Claim endpoint stale",
            ));
        }
    }
    if cmd::record_digest(&claim)?.to_hex() != cmd::text(entry, "claim_sha256")? {
        return Err(SourceCommandError::Conflict(
            "Collection catalog Claim digest stale",
        ));
    }
    Ok(claim)
}

use super::work_expression::{
    WorkApplicationGuard, WorkControlRead, after_cut_budget, complete_current_cut,
    original_prior_publication, physical_current, same_selected_plan, selected_reads_current,
    selected_sides, software_current, work_dependencies_current,
};
use tos_validation::item_rules::{ItemLimits, ItemRefusal};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

fn item_error(reason: ItemRefusal) -> SourceCommandError {
    SourceCommandError::SchemaExecution {
        path: OPERATION.to_owned(),
        root: "selected Collection membership mechanics".to_owned(),
        reason,
    }
}
fn serde_value(value: &JsonValue) -> SourceCommandResult<serde_json::Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("Collection JSON value conversion"))
}
fn foundation_value(value: &serde_json::Value) -> SourceCommandResult<JsonValue> {
    cmd::parse(
        &serde_json::to_vec(value)
            .map_err(|_| SourceCommandError::Invalid("Collection JSON value conversion"))?,
    )
}
struct CollectionCatalog {
    digests: BTreeMap<String, String>,
    retained_transactions: BTreeMap<String, String>,
    native_reads: Vec<tos_validation::PredicateRead>,
    native_bytes: u64,
    native_state: usize,
    source_bytes: u64,
}
fn catalog_record(
    fs: &CreationFilesystem,
    cut: &CorpusCutReader,
    entry: &JsonValue,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let path = cmd::text(entry, "source_record_ref")?;
    let raw = source_dependency(fs, cut, path, digests, total, deadline, cancelled)?;
    let record = cmd::parse(&raw)?;
    if cmd::field(&record, "record_id")? != cmd::field(entry, "record_id")?
        || cmd::field(&record, "record_type")? != cmd::field(entry, "record_type")?
        || cmd::record_digest(&record)?.to_hex() != cmd::text(entry, "record_sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "Collection catalog record stale",
        ));
    }
    Ok(record)
}
fn current_catalog(
    fs: &CreationFilesystem,
    owner: &CollectionOwner,
    before: &BTreeMap<String, Vec<u8>>,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    publication_token: Option<&str>,
    mut check_publication: impl FnMut() -> SourceCommandResult<()>,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionCatalog> {
    check_publication()?;
    let deadline = limits.deadline;
    let CollectionCatalogRows {
        records,
        claims,
        mut digests,
        source_bytes: mut total,
    } = catalog_rows(fs, publication_token, deadline, cancelled)?;
    let collection_raw = before
        .get("collection.json")
        .ok_or(SourceCommandError::Conflict(
            "Collection predecessor absent",
        ))?;
    let collection = cmd::parse(collection_raw)?;
    let history = tos_validation::native_compound::inspect_record_history(
        before,
        collection_raw,
        deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let parent = records
        .get(&owner.collection_id)
        .ok_or(SourceCommandError::Conflict(
            "Collection catalog parent absent",
        ))?;
    if cmd::text(&collection, "record_type")? != "collection"
        || cmd::text(&collection, "record_id")? != owner.collection_id
        || cmd::text(parent, "source_record_ref")? != owner.collection_path.as_str()
        || cmd::text(parent, "record_sha256")? != cmd::record_digest(&collection)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "Collection catalog exact parent stale",
        ));
    }
    if records.contains_key(&owner.claim_id)
        || claims.contains_key(&owner.claim_id)
        || claims.values().any(|entry| {
            entry.object_get("provenance_event_ref")
                == owner.configuration.object_get("provenance_event_id")
        })
    {
        return Err(SourceCommandError::Conflict(
            "Collection new Claim/event identity occupied",
        ));
    }
    let work_entry = records
        .get(&owner.work_id)
        .ok_or(SourceCommandError::Conflict(
            "Collection existing Work absent",
        ))?;
    if cmd::text(work_entry, "record_type")? != "work"
        || cmd::text(work_entry, "source_record_ref")? != owner.work_path.as_str()
    {
        return Err(SourceCommandError::Denied(
            "Collection existing exact Work endpoint",
        ));
    }
    let work = catalog_record(
        fs,
        cut,
        work_entry,
        &mut digests,
        &mut total,
        deadline,
        cancelled,
    )?;
    if !cmd::same(&work, cmd::field(&owner.request, "work")?)? {
        return Err(SourceCommandError::Conflict(
            "Collection caller Work differs from current source",
        ));
    }
    let mut works = BTreeMap::from([(owner.work_id.clone(), work)]);
    let mut selected = BTreeSet::new();
    let mut retained_transactions = BTreeMap::new();
    let mut native_reads = Vec::new();
    let mut native_bytes = 0u64;
    let mut native_state = 0usize;
    // Legacy provenance streams are selected explicitly and loaded once for
    // this entire membership union, rather than once for every legacy Claim.
    let mut legacy_events: Option<Vec<JsonValue>> = None;
    for (id, entry) in claims.iter().filter(|(_, entry)| {
        entry.object_get("predicate").and_then(JsonValue::as_str) == Some("contains_work")
            && entry.object_get("subject_ref").and_then(JsonValue::as_str)
                == Some(owner.collection_id.as_str())
    }) {
        active(deadline, cancelled)?;
        let claim = catalog_claim(
            fs,
            cut,
            entry,
            &mut digests,
            &mut total,
            deadline,
            cancelled,
        )?;
        let target = cmd::text(&claim, "object")?;
        let endpoint = records.get(target).ok_or(SourceCommandError::Conflict(
            "Collection member Work absent",
        ))?;
        if cmd::text(endpoint, "record_type")? != "work" {
            return Err(SourceCommandError::Conflict(
                "Collection member endpoint is not Work",
            ));
        }
        if !works.contains_key(target) {
            let value = catalog_record(
                fs,
                cut,
                endpoint,
                &mut digests,
                &mut total,
                deadline,
                cancelled,
            )?;
            works.insert(target.to_owned(), value);
        }
        let path = cmd::text(entry, "source_claim_file_ref")?;
        if path.ends_with("/source-claims.jsonl") {
            let mut remaining = limits;
            remaining.max_total_bytes = limits.max_total_bytes.checked_sub(native_bytes).ok_or(
                SourceCommandError::Invalid("Collection cumulative native bytes"),
            )?;
            remaining.max_state_bytes = limits.max_state_bytes.checked_sub(native_state).ok_or(
                SourceCommandError::Invalid("Collection cumulative native state"),
            )?;
            let observation =
                tos_validation::native_compound::verify_collection_membership_from_cut(
                    cut,
                    worker,
                    path,
                    &serde_value(&claim)?,
                    remaining,
                    cancelled,
                )
                .map_err(item_error)?;
            let (actual, plan, _, _) = work_transaction::inspect_committed(
                fs,
                &observation.transaction_id,
                deadline,
                cancelled,
            )?;
            if observation.transport
                != tos_validation::native_compound::NativeTransportState::Committed
                || actual != observation.manifest_sha256
                || cmd::text(&plan.authorization, "schema_version")? != AUTHORIZATION
                || cmd::text(cmd::field(&plan.authorization, "scope")?, "collection_id")?
                    != owner.collection_id
                || !history["receipts"]
                    .as_array()
                    .ok_or(SourceCommandError::Invalid("Collection history"))?
                    .iter()
                    .any(|receipt| {
                        receipt["publication"]["transaction_id"].as_str()
                            == Some(observation.transaction_id.as_str())
                    })
            {
                return Err(SourceCommandError::Conflict(
                    "Collection member lacks exact committed parent lineage",
                ));
            }
            if let Some(previous) =
                retained_transactions.insert(observation.transaction_id.clone(), actual.clone())
            {
                if previous != actual {
                    return Err(SourceCommandError::Conflict(
                        "Collection retained transaction changed",
                    ));
                }
            }
            native_bytes = native_bytes.checked_add(observation.bytes_read).ok_or(
                SourceCommandError::Invalid("Collection native byte overflow"),
            )?;
            native_state = native_state
                .checked_add(observation.returned_state_bytes)
                .ok_or(SourceCommandError::Invalid(
                    "Collection native state overflow",
                ))?;
            native_reads.extend(observation.reads);
        } else if path.ends_with("/membership-claims.jsonl") {
            if legacy_events.is_none() {
                let mut events = Vec::new();
                for reference in
                    cmd::array(&owner.configuration, "retained_membership_provenance_refs")?
                {
                    let reference = reference
                        .as_str()
                        .ok_or(SourceCommandError::Invalid("Collection provenance path"))?;
                    let raw = source_dependency(
                        fs,
                        cut,
                        reference,
                        &mut digests,
                        &mut total,
                        deadline,
                        cancelled,
                    )?;
                    for line in catalog_lines(&raw) {
                        if events.len() >= 8192 {
                            return Err(SourceCommandError::Invalid(
                                "Collection retained provenance event budget",
                            ));
                        }
                        events.push(cmd::parse(line)?);
                    }
                }
                legacy_events = Some(events);
            }
            let matches = legacy_events
                .as_ref()
                .unwrap()
                .iter()
                .filter(|event| {
                    event.object_get("event_id") == claim.object_get("provenance_event_ref")
                })
                .collect::<Vec<_>>();
            if cmd::text(&claim, "claim_type")? != "bibliographic"
                || !matches!(
                    cmd::text(&claim, "assertion_layer")?,
                    "bibliographic_assertion" | "scholarly_report"
                )
                || matches.len() != 1
                || !cmd::array(matches[0], "outputs")?.iter().any(|output| {
                    output.object_get("ref").and_then(JsonValue::as_str) == Some(path)
                        && output.object_get("sha256").and_then(JsonValue::as_str)
                            == digests.get(path).map(String::as_str)
                })
            {
                return Err(SourceCommandError::Conflict(
                    "Collection legacy membership retained batch differs",
                ));
            }
        } else {
            return Err(SourceCommandError::Denied(
                "Collection member has no declared evidence carrier",
            ));
        }
        selected.insert(id.clone());
    }
    let declared = cmd::array(&collection, "membership_claim_refs")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(SourceCommandError::Invalid(
                    "Collection membership reference",
                ))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if declared.len() != selected.len()
        || declared.iter().cloned().collect::<BTreeSet<_>>() != selected
    {
        return Err(SourceCommandError::Conflict(
            "Collection complete membership union differs",
        ));
    }
    let claim = cmd::field(&owner.request, "claim")?;
    for key in ["evidence_refs", "counterevidence_refs"] {
        if let Some(values) = claim.object_get(key) {
            for reference in values
                .as_array()
                .ok_or(SourceCommandError::Invalid("Collection evidence list"))?
            {
                let reference = reference
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("Collection evidence reference"))?;
                if reference.starts_with("ToS/") && reference != owner.collection_path.as_str() {
                    source_dependency(
                        fs,
                        cut,
                        reference,
                        &mut digests,
                        &mut total,
                        deadline,
                        cancelled,
                    )?;
                }
                // External citation grammar remains with the selected Claim
                // profile in the one native preparation pass. No fetching.
            }
        }
    }
    if let Some(values) = claim.object_get("alternative_claim_refs") {
        if values
            .as_array()
            .ok_or(SourceCommandError::Invalid(
                "Collection alternative Claim list",
            ))?
            .iter()
            .any(|value| !value.as_str().is_some_and(|id| claims.contains_key(id)))
        {
            return Err(SourceCommandError::Conflict(
                "Collection alternative Claim lacks catalog identity",
            ));
        }
    }
    check_publication()?;
    Ok(CollectionCatalog {
        digests,
        retained_transactions,
        native_reads,
        native_bytes,
        native_state,
        source_bytes: total as u64,
    })
}

fn selected_before(
    fs: &CreationFilesystem,
    owner: &CollectionOwner,
    cut: &CorpusCutReader,
    snapshot: &PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    snapshot.verify_current(fs, deadline, cancelled)?;
    let parent_ref = owner
        .collection_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Collection parent"))?
        .0;
    let parent = walk(&fs.root, parent_ref, fs.uid)?;
    let mut before = BTreeMap::new();
    for name in [
        "collection.json",
        "collection.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = relative(&format!("{parent_ref}/{name}"))?;
        let observed =
            work_transaction::read_at(&parent, name, fs.uid, 2_097_152, deadline, cancelled)?;
        match observed {
            Some(bytes) => {
                let member = cut
                    .current()
                    .member(&path)
                    .ok_or(SourceCommandError::Conflict(
                        "physical Collection selected member absent from cut",
                    ))?;
                if member.sha256 != Digest256::of_bytes(&bytes)
                    || member.size_bytes != bytes.len() as u64
                {
                    return Err(SourceCommandError::Conflict(
                        "physical Collection member differs from selected cut",
                    ));
                }
                let selected = cut
                    .read_member(
                        cut.current().revision(),
                        &path,
                        2_097_152,
                        deadline,
                        cancelled,
                    )
                    .map_err(|_| {
                        SourceCommandError::Unsupported(
                            "selected Collection custody read incomplete",
                        )
                    })?;
                if selected.raw != bytes {
                    return Err(SourceCommandError::Conflict(
                        "Collection cut and current bytes differ",
                    ));
                }
                before.insert(name.to_owned(), bytes);
            }
            None if name == "collection.json" || cut.current().member(&path).is_some() => {
                return Err(SourceCommandError::Conflict(
                    "selected Collection member missing",
                ));
            }
            None => (),
        }
    }
    if before.values().map(Vec::len).sum::<usize>() > 8_388_608 {
        return Err(SourceCommandError::Invalid(
            "selected Collection package byte budget",
        ));
    }
    snapshot.verify_current(fs, deadline, cancelled)?;
    Ok(before)
}

fn original_before_from_cut(
    owner: &CollectionOwner,
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let parent_ref = owner
        .collection_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Collection original parent"))?
        .0;
    let mut before = BTreeMap::new();
    let mut total = 0usize;
    for name in [
        "collection.json",
        "collection.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = relative(&format!("{parent_ref}/{name}"))?;
        if cut.current().member(&path).is_none() {
            if name == "collection.json" {
                return Err(SourceCommandError::Conflict(
                    "Collection original record absent",
                ));
            }
            continue;
        }
        let raw = cut
            .read_member(
                cut.current().revision(),
                &path,
                2_097_152,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Unsupported("Collection original cut custody incomplete")
            })?
            .raw;
        total = total
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "Collection original package overflow",
            ))?;
        if total > 8_388_608 {
            return Err(SourceCommandError::Invalid(
                "Collection original package budget",
            ));
        }
        before.insert(name.to_owned(), raw);
    }
    Ok(before)
}

const IMPLEMENTATIONS: &[&str] = &[
    "rust/crates/tos-command/src/source_collection_membership.rs",
    "rust/crates/tos-command/src/source_native_cli.rs",
    "rust/crates/tos-command/src/source_command.rs",
    "rust/crates/tos-command/src/source_revisions.rs",
    "rust/crates/tos-command/src/source_work_transaction.rs",
    "rust/crates/tos-command/src/source_read_owner.rs",
    "rust/crates/tos-command/src/source_claim_publication.rs",
    "rust/crates/tos-command/src/source_forms.rs",
    "rust/crates/tos-command/src/source_private_assessment_sources.rs",
    "rust/crates/tos-command/src/source_assessment_journal.rs",
    "rust/crates/tos-validation/src/biblio_rules.rs",
    "rust/crates/tos-compiler/src/source_bibliographic_versions.rs",
    "rust/crates/tos-command/src/source_corpus_index_projection.rs",
    "rust/crates/tos-compiler/src/source_bibliographic.rs",
    "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
    "rust/crates/tos-command/src/source_private_profile.rs",
    "rust/crates/tos-compiler/src/source_witness_catalog.rs",
];

fn dependency_bindings(
    catalog: &CollectionCatalog,
    grammar: &BTreeMap<String, String>,
    ctx: &CommandContext,
) -> SourceCommandResult<JsonValue> {
    if catalog.digests.len() + grammar.len() + IMPLEMENTATIONS.len() > 512
        || catalog.retained_transactions.len() > 128
    {
        return Err(SourceCommandError::Invalid(
            "Collection dependency binding budget",
        ));
    }
    let mut implementation = BTreeMap::new();
    for path in IMPLEMENTATIONS {
        let raw = ctx
            .file(&relative(path)?)?
            .ok_or(SourceCommandError::Unsupported(
                "Collection software subset incomplete",
            ))?;
        if raw.len() > 2_097_152 {
            return Err(SourceCommandError::Invalid(
                "Collection software member budget",
            ));
        }
        implementation.insert((*path).to_owned(), raw_hex(raw));
    }
    Ok(cmd::object(vec![
        ("catalog_and_sources", digest_map(&catalog.digests)),
        ("contracts", digest_map(grammar)),
        ("implementation", digest_map(&implementation)),
        (
            "retained_transactions",
            digest_map(&catalog.retained_transactions),
        ),
    ]))
}

fn remaining(
    catalog: &CollectionCatalog,
    mut limits: ItemLimits,
) -> SourceCommandResult<ItemLimits> {
    limits.max_total_bytes = limits
        .max_total_bytes
        .checked_sub(catalog.native_bytes)
        .and_then(|n| n.checked_sub(catalog.source_bytes))
        .ok_or(SourceCommandError::Invalid("Collection whole read budget"))?;
    limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(catalog.native_state)
        .ok_or(SourceCommandError::Invalid("Collection whole state budget"))?;
    Ok(limits)
}

impl CollectionOwner {
    fn new_directories(
        &self,
        fs: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<RelativePath>> {
        active(deadline, cancelled)?;
        let home = self
            .claim_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Collection Claim home"))?
            .0;
        let parent = home
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Collection Claim parent"))?
            .0;
        if work_transaction::read_existing_parent(fs, parent)?.is_none() {
            return Err(SourceCommandError::Conflict(
                "Collection relation parent absent",
            ));
        }
        if work_transaction::read_existing_parent(fs, home)?.is_some() {
            return Err(SourceCommandError::Conflict(
                "Collection new Claim home occupied",
            ));
        }
        active(deadline, cancelled)?;
        Ok(vec![relative(home)?])
    }
}
pub struct CollectionMembershipPreparation {
    request: JsonValue,
    projected_outputs: BTreeMap<String, Vec<u8>>,
}
impl CollectionMembershipPreparation {
    pub fn request(&self) -> &JsonValue {
        &self.request
    }
    pub fn projected_outputs(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.projected_outputs
    }
}

pub fn prepare_isolated_collection_membership_from_proposal(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionMembershipPreparation> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    original_prior_publication(cut, snapshot.token.as_deref(), limits.deadline, cancelled)?;
    let mut owner = CollectionOwner::select(fs, ctx, true, limits.deadline, cancelled)?;
    complete_current_cut(fs, cut, &snapshot, limits.deadline, cancelled)?;
    let before = selected_before(fs, &owner, cut, &snapshot, limits.deadline, cancelled)?;
    let catalog = current_catalog(
        fs,
        &owner,
        &before,
        cut,
        worker,
        limits,
        snapshot.token.as_deref(),
        || snapshot.verify_current(fs, limits.deadline, cancelled),
        cancelled,
    )?;
    owner.new_directories(fs, limits.deadline, cancelled)?;
    let work = cmd::parse(
        before
            .get("collection.json")
            .ok_or(SourceCommandError::Conflict(
                "Collection preview selected record absent",
            ))?,
    )?;
    let mut refs = cmd::array(&work, "membership_claim_refs")?.to_vec();
    if refs.len() >= 128
        || refs
            .iter()
            .any(|value| value.as_str() == Some(owner.claim_id.as_str()))
    {
        return Err(SourceCommandError::Conflict(
            "Collection preview Claim append invalid",
        ));
    }
    refs.push(cmd::string(&owner.claim_id));
    let claim = cmd::field(&owner.request, "claim")?;
    let mut claim_raw = cmd::canonical(claim)?;
    claim_raw.push(b'\n');
    let scope = serde_value(&owner.scope()?)?;
    let remaining_limits = remaining(&catalog, limits)?;
    let mut prepared_request = None;
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_collection_membership_preview_bytes(
        cut,
        worker,
        &scope,
        &claim_raw,
        &before,
        &ctx.recorded_at,
        remaining_limits,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, ctx)?;
                let mut request = owner.request.clone();
                cmd::set(&mut request, "operation", cmd::string(OPERATION))?;
                cmd::set(
                    &mut request,
                    "command_id",
                    cmd::string("preview:uncommitted"),
                )?;
                cmd::set(
                    &mut request,
                    "fields",
                    cmd::object(vec![("membership_claim_refs", JsonValue::Array(refs))]),
                )?;
                cmd::set(
                    &mut request,
                    "expected_configuration",
                    cmd::string(&owner.configuration_digest),
                )?;
                cmd::set(
                    &mut request,
                    "expected_source",
                    cmd::reference(&work, "record_id", "record_version")?,
                )?;
                cmd::set(
                    &mut request,
                    "expected_revision",
                    cmd::string(&crate::source_revisions::revision(&before)?),
                )?;
                cmd::set(
                    &mut request,
                    "expected_dependencies",
                    cmd::string(&cmd::record_digest(&dependencies)?.to_prefixed()),
                )?;
                cmd::set(
                    &mut request,
                    "expected_publication",
                    snapshot
                        .token
                        .as_ref()
                        .map_or(JsonValue::Null, |value| cmd::string(value)),
                )?;
                owner.request = request.clone();
                let authorization = serde_value(&owner.authorization(dependencies)?)?;
                let mut raw = cmd::canonical(&request)?;
                raw.push(b'\n');
                prepared_request = Some(request);
                Ok((raw, authorization))
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("Collection preview authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    let request = prepared_request.ok_or(SourceCommandError::Invalid(
        "Collection preview full request absent",
    ))?;
    let authorization = foundation_value(core.authorization())?;
    let outputs = core.outputs().map_err(item_error)?;
    let mut all_reads = catalog.native_reads;
    all_reads.extend_from_slice(core.reads());
    selected_reads_current(
        fs,
        cut,
        &all_reads,
        &BTreeMap::new(),
        &outputs,
        WorkControlRead::Ready(&snapshot),
        limits.max_total_bytes,
        limits.deadline,
        cancelled,
    )?;
    drop(outputs);
    let projected_outputs = core.into_prepared_outputs().map_err(item_error)?;
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    fs.current_context(ctx, limits.deadline, cancelled)?;
    software_current(fs, ctx, limits.deadline, cancelled)?;
    work_dependencies_current(
        fs,
        cmd::field(&authorization, "dependency_bindings")?,
        limits.deadline,
        cancelled,
    )?;
    complete_current_cut(fs, cut, &snapshot, limits.deadline, cancelled)?;
    Ok(CollectionMembershipPreparation {
        request,
        projected_outputs,
    })
}

pub struct CollectionMembershipPublication {
    transaction_id: String,
    manifest_sha256: String,
    publication: JsonValue,
    receipt: JsonValue,
    replayed: bool,
}
impl CollectionMembershipPublication {
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
    pub fn publication(&self) -> &JsonValue {
        &self.publication
    }
    pub fn receipt(&self) -> &JsonValue {
        &self.receipt
    }
    pub fn replayed(&self) -> bool {
        self.replayed
    }
}
struct PreparedCollectionApplication {
    plan: work_transaction::WorkPlan,
    guard: WorkApplicationGuard,
    receipt: JsonValue,
}
fn finished_receipt(child: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<JsonValue> {
    cmd::parse(child.get("membership-attachment-receipt.json").ok_or(
        SourceCommandError::Conflict("Collection finished receipt absent"),
    )?)
}

fn prepare_collection_application(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCollectionApplication> {
    ctx.check_from_selected_captures(cut, software, components, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    let prior_id =
        original_prior_publication(cut, snapshot.token.as_deref(), limits.deadline, cancelled)?;
    let owner = CollectionOwner::select(fs, ctx, false, limits.deadline, cancelled)?;
    let requested_publication = cmd::field(&owner.request, "expected_publication")?;
    if requested_publication.as_str() != snapshot.token.as_deref()
        && !(requested_publication == &JsonValue::Null && snapshot.token.is_none())
    {
        return Err(SourceCommandError::Conflict(
            "Collection requested publication snapshot changed",
        ));
    }
    let before = selected_before(fs, &owner, cut, &snapshot, limits.deadline, cancelled)?;
    let catalog = current_catalog(
        fs,
        &owner,
        &before,
        cut,
        worker,
        limits,
        snapshot.token.as_deref(),
        || snapshot.verify_current(fs, limits.deadline, cancelled),
        cancelled,
    )?;
    let directories = owner.new_directories(fs, limits.deadline, cancelled)?;
    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_collection_membership_bytes(
        cut,
        worker,
        &scope,
        &request_raw,
        &before,
        &ctx.recorded_at,
        remaining(&catalog, limits)?,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, ctx)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "Collection current dependency closure changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("Collection owner authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    if core.transaction_id() != owner.transaction_id()? {
        return Err(SourceCommandError::Conflict(
            "Collection transaction identity differs from request",
        ));
    }
    let authorization = foundation_value(core.authorization())?;
    let archive_path = core.archive_path().to_owned();
    let outputs = core.outputs().map_err(item_error)?;
    let borrowed = outputs
        .iter()
        .map(|(path, raw)| (path.as_str(), *raw))
        .collect::<Vec<_>>();
    let capture = crate::source_serialization::capture_collection_membership(
        &owner.request,
        cmd::text(&owner.configuration, "provenance_event_id")?,
        owner.claim_path.as_str().rsplit_once('/').unwrap().0,
        &archive_path,
        &before,
        &borrowed,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let finished = tos_validation::native_compound::finish_collection_membership_bytes(
        core,
        &capture.environment_raw,
        &capture.event_raw,
        worker,
    )
    .map_err(item_error)?;
    if finished.transaction_id != owner.transaction_id()? || finished.archive_path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Collection finished byte recipe differs from selected transaction",
        ));
    }
    let receipt = finished_receipt(&finished.child)?;
    let mut read_observations = catalog.native_reads;
    read_observations.extend(finished.reads);
    let plan = owner.plan(
        authorization,
        &before,
        finished.parent,
        finished.child,
        directories,
    )?;
    let selected = selected_sides(&plan)?;
    after_cut_budget(cut, &selected)?;
    // The worker operation must finish before a protected source mutation.
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let old = cmd::parse(
        before
            .get("collection.json")
            .ok_or(SourceCommandError::Conflict("Collection parent absent"))?,
    )?;
    let stored_archive = work_transaction::collection_archive(
        fs,
        owner.collection_path.as_str(),
        &old,
        &before,
        cmd::text(&owner.request, "expected_revision")?,
        limits.deadline,
        cancelled,
        true,
    )?;
    if stored_archive.path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Collection retained archive locator differs from recipe",
        ));
    }
    let authorization = plan.authorization.clone();
    Ok(PreparedCollectionApplication {
        plan,
        guard: WorkApplicationGuard {
            snapshot,
            prior_id,
            selected,
            read_observations,
            stored_archive,
            authorization,
        },
        receipt,
    })
}

fn apply_collection_application(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    prepared: PreparedCollectionApplication,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionMembershipPublication> {
    let PreparedCollectionApplication {
        plan,
        guard,
        receipt,
    } = prepared;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    let applied = fence.apply(
        plan,
        &guard.snapshot,
        |summary, extent| guard.check(fs, ctx, cut, summary, extent, limits, cancelled),
        limits.deadline,
        cancelled,
    )?;
    if !applied.committed {
        return Err(SourceCommandError::Conflict(
            "Collection selected publication rolled back",
        ));
    }
    Ok(CollectionMembershipPublication {
        transaction_id: applied.transaction_id,
        manifest_sha256: applied.manifest_sha256,
        publication: applied.publication,
        receipt,
        replayed: false,
    })
}

/// Publish one qualified membership append through the existing exact selected
/// journal only after current owner/cut/capture checks and the worker FINAL.
pub fn execute_isolated_collection_membership_from_captures(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionMembershipPublication> {
    if work_transaction::read_pending(fs, limits.deadline, cancelled)?.is_some() {
        return recover_collection_selected(
            fs,
            ctx,
            cut,
            software,
            components,
            worker,
            CollectionRecoveryDecision::Resume,
            false,
            limits,
            cancelled,
        );
    }
    let prepared = prepare_collection_application(
        fs, ctx, cut, software, components, worker, limits, cancelled,
    )?;
    apply_collection_application(fs, ctx, cut, prepared, limits, cancelled)
}

pub fn replay_isolated_collection_membership_from_captures(
    fs: &CreationFilesystem,
    original_ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    current_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionMembershipPublication> {
    original_ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let owner = CollectionOwner::select(fs, original_ctx, false, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    complete_current_cut(fs, current_cut, &snapshot, limits.deadline, cancelled)?;
    software_current(fs, original_ctx, limits.deadline, cancelled)?;
    let transaction_id = owner.transaction_id()?;
    let (manifest_sha256, retained, base, terminal) =
        work_transaction::inspect_committed(fs, &transaction_id, limits.deadline, cancelled)?;
    let requested = cmd::field(&owner.request, "expected_publication")?;
    if cmd::field(&base, "token")? != requested
        || cmd::text(&retained.authorization, "schema_version")? != AUTHORIZATION
        || !cmd::same(
            &retained.authorization,
            &owner.authorization(
                cmd::field(&retained.authorization, "dependency_bindings")?.clone(),
            )?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "Collection original replay basis differs",
        ));
    }
    let before = original_before_from_cut(&owner, original_cut, limits.deadline, cancelled)?;
    if crate::source_revisions::revision(&before)?
        != cmd::text(&owner.request, "expected_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "Collection original revision differs",
        ));
    }
    let work_home = owner.collection_path.as_str().rsplit_once('/').unwrap().0;
    let expression_home = owner.claim_path.as_str().rsplit_once('/').unwrap().0;
    let mut parent = BTreeMap::new();
    let mut child = BTreeMap::new();
    for file in &retained.files {
        let path = file.path.as_str();
        let (home, name) = path.rsplit_once('/').ok_or(SourceCommandError::Invalid(
            "Collection retained selected path",
        ))?;
        let after = file.after.as_ref().ok_or(SourceCommandError::Conflict(
            "Collection retained selected output absent",
        ))?;
        if home == work_home {
            parent.insert(name.to_owned(), after.clone());
        } else if home == expression_home {
            child.insert(name.to_owned(), after.clone());
        } else {
            return Err(SourceCommandError::Conflict(
                "Collection retained selected path outside scope",
            ));
        }
    }
    let expected = owner.plan(
        retained.authorization.clone(),
        &before,
        parent,
        child,
        retained.new_directories.clone(),
    )?;
    if !same_selected_plan(&expected, &retained)? {
        return Err(SourceCommandError::Conflict(
            "Collection retained plan differs from original cut",
        ));
    }
    let receipt_path = format!("{expression_home}/membership-attachment-receipt.json");
    let receipt_raw = retained
        .files
        .iter()
        .find(|file| file.path.as_str() == receipt_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "Collection retained receipt absent",
        ))?;
    let receipt = cmd::parse(receipt_raw)?;
    let claims_path = relative(&format!("{expression_home}/source-claims.jsonl"))?;
    let claims_raw = current_cut
        .read_member(
            current_cut.current().revision(),
            &claims_path,
            2_097_152,
            limits.deadline,
            cancelled,
        )
        .map_err(|_| {
            SourceCommandError::Unsupported("current Collection Claim custody incomplete")
        })?
        .raw;
    let mut current_claim = None;
    for line in catalog_lines(&claims_raw) {
        active(limits.deadline, cancelled)?;
        if crate::source_claims::python_bytes_blank(line) {
            continue;
        }
        let claim = cmd::parse(line)?;
        if cmd::text(&claim, "claim_id")? == owner.claim_id {
            if current_claim.replace(claim).is_some() {
                return Err(SourceCommandError::Conflict(
                    "duplicate current Collection Claim",
                ));
            }
        }
    }
    let current_claim = current_claim.ok_or(SourceCommandError::Conflict(
        "current Collection Claim absent",
    ))?;
    let observation = tos_validation::native_compound::verify_collection_membership_from_cut(
        current_cut,
        worker,
        claims_path.as_str(),
        &serde_value(&current_claim)?,
        limits,
        cancelled,
    )
    .map_err(item_error)?;
    if observation.transport != tos_validation::native_compound::NativeTransportState::Committed
        || observation.transaction_id != transaction_id
        || observation.manifest_sha256 != manifest_sha256
        || observation.claim_id != owner.claim_id
        || observation.work_parent_transition_sha256.as_deref()
            != Some(cmd::text(&receipt, "parent_transition_sha256")?)
    {
        return Err(SourceCommandError::Conflict(
            "Collection committed native lineage differs",
        ));
    }
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    fs.current_context(original_ctx, limits.deadline, cancelled)?;
    fence.verify(limits.deadline, cancelled)?;
    complete_current_cut(fs, current_cut, &snapshot, limits.deadline, cancelled)?;
    software_current(fs, original_ctx, limits.deadline, cancelled)?;
    let (current_manifest, _, _, _) =
        work_transaction::inspect_committed(fs, &transaction_id, limits.deadline, cancelled)?;
    if current_manifest != manifest_sha256 {
        return Err(SourceCommandError::Conflict(
            "Collection retained replay changed",
        ));
    }
    snapshot.verify_current(fs, limits.deadline, cancelled)?;
    Ok(CollectionMembershipPublication {
        transaction_id,
        manifest_sha256,
        publication: terminal,
        receipt,
        replayed: true,
    })
}

#[derive(Clone, Copy)]
pub enum CollectionRecoveryDecision {
    Resume,
    Rollback,
}

fn retained_owner(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    pending: &work_transaction::PendingWork,
    explicit: bool,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionOwner> {
    let authority = &pending.plan.authorization;
    cmd::exact_keys(
        authority,
        &[
            "schema_version",
            "scope",
            "principal_id",
            "maker_type",
            "authority_ref",
            "owner_configuration",
            "command_id",
            "request_digest",
            "dependency_bindings",
        ],
    )?;
    if cmd::text(authority, "schema_version")? != AUTHORIZATION {
        return Err(SourceCommandError::Denied(
            "Collection retained authority family",
        ));
    }
    let scope = cmd::field(authority, "scope")?;
    cmd::exact_keys(scope, SCOPE_KEYS)?;
    let mut grant = CollectionGrant::select(
        fs,
        ctx,
        Some(if explicit { RECOVERY } else { OPERATION }),
        limits.deadline,
        cancelled,
    )?;
    for key in SCOPE_KEYS.iter().filter(|key| !key.starts_with("allowed_")) {
        if !cmd::same(
            cmd::field(&grant.configuration, key)?,
            cmd::field(scope, key)?,
        )? {
            return Err(SourceCommandError::Denied(
                "Collection recovery changes frozen endpoint scope",
            ));
        }
    }
    let home = grant
        .claim_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid(
            "Collection retained Claim home",
        ))?
        .0;
    let request_path = format!("{home}/source-create-request.json");
    let raw = pending
        .plan
        .files
        .iter()
        .find(|file| file.path.as_str() == request_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "Collection retained request absent",
        ))?;
    let request = cmd::parse(raw)?;
    if cmd::record_digest(&request)?.to_prefixed() != cmd::text(authority, "request_digest")?
        || cmd::field(&request, "command_id")? != cmd::field(authority, "command_id")?
        || cmd::field(&request, "expected_configuration")?
            != cmd::field(authority, "owner_configuration")?
    {
        return Err(SourceCommandError::Conflict(
            "Collection retained request authority differs",
        ));
    }
    selected_forms(&request, "forms", &grant.collection_forms)?;
    selected_forms(&request, "claim_forms", &grant.claim_forms)?;
    for key in ["evidence_refs", "counterevidence_refs"] {
        if let Some(values) = cmd::field(&request, "claim")?.object_get(key) {
            if values
                .as_array()
                .ok_or(SourceCommandError::Invalid(
                    "Collection retained evidence list",
                ))?
                .iter()
                .any(|value| {
                    !value
                        .as_str()
                        .is_some_and(|reference| grant.evidence.contains(reference))
                })
            {
                return Err(SourceCommandError::Denied(
                    "Collection retained evidence leaves current recovery grant",
                ));
            }
        }
    }
    if !explicit
        && (!cmd::same(&request, &cmd::parse(&ctx.request_raw)?)?
            || cmd::record_digest(&grant.configuration)?.to_prefixed()
                != cmd::text(authority, "owner_configuration")?)
    {
        return Err(SourceCommandError::Conflict(
            "Collection exact retry changed original grant or request",
        ));
    }
    // Reconstruct historical scope only from the exact selected manifest.
    // Current grant selection above remains the sole permission/expiry source.
    for key in SCOPE_KEYS {
        cmd::set(
            &mut grant.configuration,
            key,
            cmd::field(scope, key)?.clone(),
        )?;
    }
    for key in ["principal_id", "maker_type", "authority_ref"] {
        cmd::set(
            &mut grant.configuration,
            key,
            cmd::field(authority, key)?.clone(),
        )?;
    }
    if !cmd::nonblank(cmd::text(&grant.configuration, "principal_id")?)
        || !cmd::nonblank(cmd::text(&grant.configuration, "authority_ref")?)
        || !matches!(
            cmd::text(&grant.configuration, "maker_type")?,
            "human" | "software" | "model"
        )
    {
        return Err(SourceCommandError::Denied(
            "Collection historical actor shape",
        ));
    }
    grant.collection_forms = allowed_forms(&grant.configuration, "allowed_collection_form_ids")?;
    grant.claim_forms = allowed_forms(&grant.configuration, "allowed_claim_form_ids")?;
    grant.evidence = bounded_strings(&grant.configuration, "allowed_evidence_refs", 1, 128)?;
    let owner = CollectionOwner::from_grant(
        grant,
        raw,
        false,
        Some(cmd::text(authority, "owner_configuration")?),
    )?;
    if owner.transaction_id()? != pending.plan.transaction_id
        || !cmd::same(
            authority,
            &owner.authorization(cmd::field(authority, "dependency_bindings")?.clone())?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "Collection retained original identity differs",
        ));
    }
    Ok(owner)
}

fn recover_collection_selected(
    fs: &CreationFilesystem,
    original_ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    decision: CollectionRecoveryDecision,
    explicit: bool,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionMembershipPublication> {
    original_ctx.check_from_selected_captures(
        original_cut,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let pending = work_transaction::read_pending(fs, limits.deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("Collection exact pending transaction absent"),
    )?;
    let owner = retained_owner(fs, original_ctx, &pending, explicit, limits, cancelled)?;
    if explicit {
        let request = cmd::parse(&original_ctx.request_raw)?;
        cmd::exact_keys(
            &request,
            &[
                "schema_version",
                "operation",
                "transaction_id",
                "decision",
                "expected_configuration",
            ],
        )?;
        let config = cmd::parse(&original_ctx.configuration_raw)?;
        if cmd::text(&request, "schema_version")? != REQUEST
            || cmd::text(&request, "operation")? != RECOVERY
            || cmd::text(&request, "expected_configuration")?
                != cmd::record_digest(&config)?.to_prefixed()
            || cmd::text(&request, "transaction_id")? != pending.plan.transaction_id
            || cmd::text(&request, "decision")?
                != if matches!(decision, CollectionRecoveryDecision::Rollback) {
                    "rollback"
                } else {
                    "resume"
                }
        {
            return Err(SourceCommandError::Denied(
                "Collection recovery exact current delegation/decision",
            ));
        }
    }
    let transaction_id = owner.transaction_id()?;
    if cmd::text(&pending.state, "transaction_id")? != transaction_id
        || cmd::field(&pending.base_publication, "token")?
            != cmd::field(&owner.request, "expected_publication")?
        || pending.plan.transaction_id != transaction_id
        || cmd::text(&pending.plan.authorization, "schema_version")? != AUTHORIZATION
        || !cmd::same(
            &pending.plan.authorization,
            &owner.authorization(
                cmd::field(&pending.plan.authorization, "dependency_bindings")?.clone(),
            )?,
        )?
    {
        return Err(SourceCommandError::Conflict(
            "Collection pending owner/request basis differs",
        ));
    }
    let before = original_before_from_cut(&owner, original_cut, limits.deadline, cancelled)?;
    if crate::source_revisions::revision(&before)?
        != cmd::text(&owner.request, "expected_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "Collection pending predecessor revision differs",
        ));
    }
    let old = cmd::parse(
        before
            .get("collection.json")
            .ok_or(SourceCommandError::Conflict(
                "Collection pending predecessor absent",
            ))?,
    )?;
    let archive = work_transaction::collection_archive(
        fs,
        owner.collection_path.as_str(),
        &old,
        &before,
        cmd::text(&owner.request, "expected_revision")?,
        limits.deadline,
        cancelled,
        false,
    )?;
    let publication_token = cmd::field(&owner.request, "expected_publication")?.as_str();
    let prior_id =
        original_prior_publication(original_cut, publication_token, limits.deadline, cancelled)?;
    let catalog = current_catalog(
        fs,
        &owner,
        &before,
        original_cut,
        worker,
        limits,
        publication_token,
        || work_transaction::still_pending(fs, &pending.state, limits.deadline, cancelled),
        cancelled,
    )?;
    let receipt_path = format!(
        "{}/membership-attachment-receipt.json",
        owner.claim_path.as_str().rsplit_once('/').unwrap().0
    );
    let retained_receipt_raw = pending
        .plan
        .files
        .iter()
        .find(|file| file.path.as_str() == receipt_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "Collection pending receipt absent",
        ))?;
    let recorded_at = cmd::text(&cmd::parse(retained_receipt_raw)?, "recorded_at")?.to_owned();
    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_collection_membership_bytes(
        original_cut,
        worker,
        &scope,
        &request_raw,
        &before,
        &recorded_at,
        remaining(&catalog, limits)?,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, original_ctx)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "Collection pending dependencies changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("Collection recovery authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    if core.transaction_id() != transaction_id || core.archive_path() != archive.path {
        return Err(SourceCommandError::Conflict(
            "Collection pending recipe identity differs",
        ));
    }
    if !cmd::same(
        &foundation_value(core.authorization())?,
        &pending.plan.authorization,
    )? {
        return Err(SourceCommandError::Conflict(
            "Collection pending owner recipe differs",
        ));
    }
    let expression_home = owner.claim_path.as_str().rsplit_once('/').unwrap().0;
    let selected_after = |name: &str| -> SourceCommandResult<&[u8]> {
        let path = format!("{expression_home}/{name}");
        pending
            .plan
            .files
            .iter()
            .find(|file| file.path.as_str() == path)
            .and_then(|file| file.after.as_deref())
            .ok_or(SourceCommandError::Conflict(
                "Collection pending native capture absent",
            ))
    };
    let finished = tos_validation::native_compound::restore_collection_membership_bytes(
        core,
        selected_after("source-create-environment.json")?,
        selected_after("source-create-provenance.jsonl")?,
        worker,
    )
    .map_err(item_error)?;
    let receipt = finished_receipt(&finished.child)?;
    let expected = owner.plan(
        pending.plan.authorization.clone(),
        &before,
        finished.parent,
        finished.child,
        pending.plan.new_directories.clone(),
    )?;
    if !same_selected_plan(&expected, &pending.plan)? {
        return Err(SourceCommandError::Conflict(
            "Collection pending plan differs from native recipe",
        ));
    }
    let selected = selected_sides(&pending.plan)?;
    let mut read_observations = catalog.native_reads;
    read_observations.extend(finished.reads);
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    let fence = work_transaction::WorkCorpusFence::hold(fs, limits.deadline, cancelled)?;
    let held_pending = work_transaction::read_pending(fs, limits.deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("Collection selected pending vanished before recovery"),
    )?;
    if !cmd::same(&held_pending.state, &pending.state)?
        || !cmd::same(&held_pending.base_publication, &pending.base_publication)?
        || !same_selected_plan(&held_pending.plan, &pending.plan)?
    {
        return Err(SourceCommandError::Conflict(
            "Collection pending changed before recovery lock",
        ));
    }
    let authorization = pending.plan.authorization.clone();
    let guard = |summary: &JsonValue, extent: work_transaction::WorkGuard<'_>| {
        if !cmd::same(cmd::field(summary, "authorization")?, &authorization)? {
            return Err(SourceCommandError::Conflict(
                "Collection pending authorization changed",
            ));
        }
        physical_current(
            fs,
            original_ctx,
            original_cut,
            &selected,
            cmd::field(&authorization, "dependency_bindings")?,
            None,
            Some(&archive),
            prior_id.as_deref().zip(publication_token),
            &extent,
            limits.deadline,
            cancelled,
        )?;
        let pending_state = extent.pending_state.ok_or(SourceCommandError::Conflict(
            "Collection recovery publication is not pending",
        ))?;
        selected_reads_current(
            fs,
            original_cut,
            &read_observations,
            &selected,
            &[],
            WorkControlRead::Pending(pending_state),
            limits.max_total_bytes,
            limits.deadline,
            cancelled,
        )
    };
    let rollback = matches!(decision, CollectionRecoveryDecision::Rollback);
    let current = cmd::parse(&original_ctx.configuration_raw)?;
    let renewal = if explicit {
        Some(cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_collection_membership_recovery_authorization_v1"),
            ),
            (
                "principal_id",
                cmd::field(&current, "principal_id")?.clone(),
            ),
            (
                "authority_ref",
                cmd::field(&current, "authority_ref")?.clone(),
            ),
            (
                "owner_configuration",
                cmd::string(&cmd::record_digest(&current)?.to_prefixed()),
            ),
            ("transaction_id", cmd::string(&transaction_id)),
            (
                "decision",
                cmd::string(if rollback { "rollback" } else { "resume" }),
            ),
        ]))
    } else {
        None
    };
    let result = fence.recover(
        &held_pending,
        rollback,
        renewal,
        guard,
        limits.deadline,
        cancelled,
    )?;
    Ok(CollectionMembershipPublication {
        transaction_id: result.transaction_id,
        manifest_sha256: result.manifest_sha256,
        publication: result.publication,
        receipt: if result.committed {
            receipt
        } else {
            JsonValue::Null
        },
        replayed: false,
    })
}

pub fn recover_isolated_collection_membership_from_captures(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    original_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    decision: CollectionRecoveryDecision,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CollectionMembershipPublication> {
    recover_collection_selected(
        fs,
        ctx,
        original_cut,
        software,
        components,
        worker,
        decision,
        true,
        limits,
        cancelled,
    )
}

pub(crate) fn current_result_fields(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    with_materializations: bool,
) -> SourceCommandResult<JsonValue> {
    let grant = CollectionGrant::select(fs, ctx, None, limits.deadline, cancelled)?;
    let config = grant.configuration;
    fs.current_context(ctx, limits.deadline, cancelled)?;
    let snapshot = PublicationSnapshot::select(fs, limits.deadline, cancelled)?;
    let path = relative(cmd::text(&config, "collection_source_path")?)?;
    let home = path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Collection result parent path"))?
        .0;
    let parent = walk(&fs.root, home, fs.uid)?;
    let mut package = BTreeMap::new();
    for name in [
        "collection.json",
        "collection.human-forms.json",
        "source-revision-history.json",
    ] {
        if let Some(raw) =
            work_transaction::read_at(&parent, name, fs.uid, 2_097_152, limits.deadline, cancelled)?
        {
            package.insert(name.to_owned(), raw);
        }
    }
    let edition_raw = package
        .get("collection.json")
        .ok_or(SourceCommandError::Conflict(
            "Collection result parent absent",
        ))?;
    let edition = cmd::parse(edition_raw)?;
    tos_validation::native_compound::inspect_record_history(
        &package,
        edition_raw,
        limits.deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let scope = JsonValue::Object(
        config
            .as_object()
            .ok_or(SourceCommandError::Invalid("Collection result grant"))?
            .iter()
            .filter(|(k, _)| k.as_str().is_some_and(|k| SCOPE_KEYS.contains(&k)))
            .cloned()
            .collect(),
    );
    let mut profiles = Vec::new();
    let entity = serde_value(&cmd::parse(&checked_current_descriptor(
        fs,
        cut,
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        2_097_152,
        limits.deadline,
        cancelled,
    )?)?)?;
    let relation = serde_value(&cmd::parse(&checked_current_descriptor(
        fs,
        cut,
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        2_097_152,
        limits.deadline,
        cancelled,
    )?)?)?;
    let corpus = serde_value(&cmd::parse(&checked_current_descriptor(
        fs,
        cut,
        "ToS/contracts/corpus-record.schema.json",
        2_097_152,
        limits.deadline,
        cancelled,
    )?)?)?;
    for kind in ["collection", "work"] {
        let matching = entity["types"]
            .as_array()
            .ok_or(SourceCommandError::Invalid("Collection entity registry"))?
            .iter()
            .filter(|entry| {
                entry["source_mappings"].as_array().is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row["source_graph"] == "source-claims" && row["source_kind_id"] == kind
                    })
                })
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "Collection descriptor mapping ambiguous",
            ));
        }
        let value = serde_json::json!({"type_id":matching[0]["type_id"],"record_type":kind,"schema_ref":"ToS/contracts/corpus-record.schema.json","schema_version":corpus["properties"]["schema_version"]["const"],"source_basename":format!("{kind}.json")});
        profiles.push((kind, foundation_value(&value)?));
    }
    let matching = relation["relations"]
        .as_array()
        .ok_or(SourceCommandError::Invalid("Collection relation registry"))?
        .iter()
        .filter(|entry| {
            entry["source_mappings"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["source_graph"] == "source-claims"
                        && row["source_predicate_id"] == "contains_work"
                        && row["scope"] == "claim-predicate"
                })
            })
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "Collection relation descriptor ambiguous",
        ));
    }
    let profile = &matching[0]["source_claim_profile"];
    let routes = profile["schemas"]
        .as_array()
        .ok_or(SourceCommandError::Invalid(
            "Collection relation schema routes",
        ))?
        .iter()
        .filter(|row| row["schema_version"] == "tos_source_relation_claim_v1")
        .collect::<Vec<_>>();
    if routes.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "Collection Claim descriptor ambiguous",
        ));
    }
    profiles.push(("contains_work",foundation_value(&serde_json::json!({"relation_type_id":matching[0]["relation_type_id"],"predicate":"contains_work","reader":profile["reader"],"schema_ref":routes[0]["schema_ref"],"schema_version":routes[0]["schema_version"],"assertion_layers":profile["assertion_layers"],"source_basename":"source-claims.jsonl"}))?));
    let work_raw = checked_current_source(
        fs,
        cut,
        cmd::text(&config, "work_source_path")?,
        2_097_152,
        limits.deadline,
        cancelled,
    )?;
    let work = cmd::parse(&work_raw)?;
    if cmd::text(&work, "record_type")? != "work"
        || cmd::text(&work, "record_id")? != cmd::text(&config, "work_id")?
    {
        return Err(SourceCommandError::Conflict(
            "Collection result existing Work identity changed",
        ));
    }
    let materializations = if with_materializations {
        let child_home = cmd::text(&config, "claim_source_path")?
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Collection result Claim home"))?
            .0;
        let child_directory = walk(&fs.root, child_home, fs.uid)?;
        let mut child = BTreeMap::new();
        for name in [
            "source-claims.jsonl".to_owned(),
            format!(
                "source-claims.{}.human-forms.json",
                Digest256::of_bytes(cmd::text(&config, "claim_id")?.as_bytes()).to_hex()
            ),
        ] {
            let raw = work_transaction::read_at(
                &child_directory,
                &name,
                fs.uid,
                2_097_152,
                limits.deadline,
                cancelled,
            )?
            .ok_or(SourceCommandError::Conflict(
                "Collection result current Claim/form absent",
            ))?;
            child.insert(name, raw);
        }
        compound_views(&package, &child, cmd::text(&config, "claim_id")?)?
    } else {
        JsonValue::Null
    };
    let result = cmd::object(vec![
        (
            "source",
            cmd::reference(&edition, "record_id", "record_version")?,
        ),
        (
            "revision",
            cmd::string(&crate::source_revisions::revision(&package)?),
        ),
        (
            "publication_snapshot",
            snapshot
                .token
                .as_ref()
                .map_or(JsonValue::Null, |v| cmd::string(v)),
        ),
        ("work_record", work.clone()),
        (
            "work_source_binding",
            cmd::object(vec![
                (
                    "source_path",
                    cmd::field(&config, "work_source_path")?.clone(),
                ),
                (
                    "source",
                    cmd::reference(&work, "record_id", "record_version")?,
                ),
                (
                    "source_sha256",
                    cmd::string(&Digest256::of_bytes(&work_raw).to_prefixed()),
                ),
                ("source_bytes", cmd::number(work_raw.len() as u64)),
            ]),
        ),
        ("scope", scope),
        ("materializations", materializations),
        ("source_profiles", cmd::object(profiles)),
        (
            "source_fields",
            JsonValue::Array(
                crate::source_forms::metadata_fields(&edition)?
                    .iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        ),
    ]);
    snapshot.verify_current(fs, limits.deadline, cancelled)?;
    fs.current_context(ctx, limits.deadline, cancelled)?;
    Ok(result)
}

fn compound_views(
    parent: &BTreeMap<String, Vec<u8>>,
    child: &BTreeMap<String, Vec<u8>>,
    claim_id: &str,
) -> SourceCommandResult<JsonValue> {
    let selected =
        |map: &BTreeMap<String, Vec<u8>>, name: &str| -> SourceCommandResult<JsonValue> {
            cmd::parse(map.get(name).ok_or(SourceCommandError::Conflict(
                "Collection materialization buffer absent",
            ))?)
        };
    let collection = selected(parent, "collection.json")?;
    let claim = selected(child, "source-claims.jsonl")?;
    Ok(cmd::object(vec![
        (
            "collection",
            JsonValue::Array(crate::source_forms::materialize_source_forms(
                &collection,
                &selected(parent, "collection.human-forms.json")?,
            )?),
        ),
        (
            "claim",
            JsonValue::Array(crate::source_forms::materialize_source_forms(
                &claim,
                &selected(
                    child,
                    &format!(
                        "source-claims.{}.human-forms.json",
                        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
                    ),
                )?,
            )?),
        ),
    ]))
}
impl CollectionMembershipPreparation {
    pub(crate) fn result_fields(
        &self,
        configuration: &JsonValue,
    ) -> SourceCommandResult<JsonValue> {
        let collection_home = cmd::text(configuration, "collection_source_path")?
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "Collection preview parent home",
            ))?
            .0;
        let claim_home = cmd::text(configuration, "claim_source_path")?
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Collection preview Claim home"))?
            .0;
        let select = |home: &str| {
            self.projected_outputs
                .iter()
                .filter_map(|(path, raw)| {
                    path.strip_prefix(&format!("{home}/"))
                        .filter(|name| !name.contains('/'))
                        .map(|name| (name.to_owned(), raw.clone()))
                })
                .collect::<BTreeMap<_, _>>()
        };
        let parent = select(collection_home);
        let child = select(claim_home);
        let collection = cmd::parse(parent.get("collection.json").ok_or(
            SourceCommandError::Conflict("Collection projected record absent"),
        )?)?;
        let claim = cmd::parse(child.get("source-claims.jsonl").ok_or(
            SourceCommandError::Conflict("Collection projected Claim absent"),
        )?)?;
        let mut forms = Vec::new();
        for (kind, files, name) in [
            (
                "collection",
                &parent,
                "collection.human-forms.json".to_owned(),
            ),
            (
                "claim",
                &child,
                format!(
                    "source-claims.{}.human-forms.json",
                    Digest256::of_bytes(cmd::text(configuration, "claim_id")?.as_bytes()).to_hex()
                ),
            ),
        ] {
            let set = cmd::parse(files.get(&name).ok_or(SourceCommandError::Conflict(
                "Collection projected form set absent",
            ))?)?;
            forms.push((
                kind,
                JsonValue::Array(
                    cmd::array(&set, "forms")?
                        .iter()
                        .map(crate::source_forms::form_reference)
                        .collect::<SourceCommandResult<Vec<_>>>()?,
                ),
            ));
        }
        Ok(cmd::object(vec![
            ("prepared_request", self.request.clone()),
            (
                "prepared_fields",
                cmd::field(&self.request, "fields")?.clone(),
            ),
            (
                "prepared_claim",
                cmd::reference(&claim, "claim_id", "claim_version")?,
            ),
            (
                "prepared_collection",
                cmd::reference(&collection, "record_id", "record_version")?,
            ),
            (
                "prepared_work",
                cmd::reference(
                    cmd::field(&self.request, "work")?,
                    "record_id",
                    "record_version",
                )?,
            ),
            ("prepared_forms", cmd::object(forms)),
            (
                "prepared_materializations",
                compound_views(&parent, &child, cmd::text(configuration, "claim_id")?)?,
            ),
            (
                "expected_dependencies",
                cmd::field(&self.request, "expected_dependencies")?.clone(),
            ),
            (
                "expected_publication",
                cmd::field(&self.request, "expected_publication")?.clone(),
            ),
        ]))
    }
}
