//! Repository raw collections from an independently selected authored cut and
//! an exact Git capture inventory. The capture supplies resource metadata only;
//! it is weaker than authored meaning and does not establish current admission.
use crate::knowledge_repository::RepositoryRootInput;
use crate::knowledge_stage::{InputCollectionReceipt, InputRow, KnowledgeStage};
use crate::{Error, QueryVocabulary, Result};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_source_store::{
    CorpusCutReader, SoftwareCaptureReader, SoftwareCaptureSelectionV1, SourceMembershipV1,
};
use tos_validation::source_cut::CutSchemaExecutor;

const HOME: &str = "ToS/source_home.manifest.json";
const SELF: &str = "ToS/derived-exports/tos_corpus_index.min.json";
const PARTS: [&str; 2] = [
    "ToS/derived-exports/tos_corpus_index.min.parts",
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.parts",
];
const PROFILE: &str = "repository-topology-v1";

#[derive(Clone, Copy, Debug)]
pub struct RepositorySourceLimits {
    pub max_inventory_members: u64,
    pub max_source_bytes: usize,
    pub max_row_bytes: usize,
    /// This cold plan owns its output bytes, with one explicit finite cap.
    pub max_plan_bytes: usize,
    pub max_work_bytes: u64,
}
impl RepositorySourceLimits {
    fn validate(self) -> Result<()> {
        if self.max_inventory_members == 0
            || self.max_inventory_members > 65_536
            || self.max_source_bytes == 0
            || self.max_source_bytes > 8 * 1024 * 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self.max_plan_bytes == 0
            || self.max_plan_bytes > 64 * 1024 * 1024
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("repository source limits"));
        }
        Ok(())
    }
}
struct Row {
    collection: String,
    id: String,
    raw: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct RepositorySourceReceipt {
    pub source_revision: String,
    pub source_membership: SourceMembershipV1,
    pub inventory_selection: SoftwareCaptureSelectionV1,
    pub inventory_members: u64,
    pub inventory_root_sha256: String,
    pub collections: Vec<InputCollectionReceipt>,
    pub plan_bytes: usize,
}
/// No public constructor or mutable output: target receipt roots are derived
/// before target creation, then compared before any input ingestion.
pub struct RepositorySourcePlan {
    receipt: RepositorySourceReceipt,
    rows: Vec<Row>,
    job_source_cut: String,
    descriptor_sha256: String,
    root_material: Vec<u8>,
    root_material_sha256: String,
    root_identity: String,
}
impl RepositorySourcePlan {
    pub fn receipt(&self) -> &RepositorySourceReceipt {
        &self.receipt
    }
    pub fn root_input(&self) -> RepositoryRootInput<'_> {
        RepositoryRootInput {
            source_cut: &self.job_source_cut,
            material: &self.root_material,
            material_sha256: &self.root_material_sha256,
            identity_id: &self.root_identity,
        }
    }
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Invalid("repository source cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(Error::Budget("repository source deadline"));
    }
    Ok(())
}
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("repository source required string"))
}
fn selected_path(path: &str) -> bool {
    path.starts_with("ToS/")
        && path != SELF
        && !path.split('/').any(|part| part == "payload")
        && !PARTS
            .iter()
            .any(|root| path.starts_with(&format!("{root}/")))
}
fn owner(path: &str) -> &str {
    let tail = path.strip_prefix("ToS/").unwrap_or("");
    match tail.split_once('/') {
        Some((branch, _)) => branch,
        None => "ToS",
    }
}
fn authority(path: &str) -> &'static str {
    // Maintained path-to-authority recipe, not source identity or admission.
    match owner(path) {
        "ToS" => "source_home",
        "candidate-intake" => "candidate_intake",
        "canon" => "canon",
        "contracts" => "contract",
        "derived-exports" => "derived_export",
        "doctrine" => "doctrine",
        "philosophy" => "domain_topology",
        "public-compatibility" => "public_compatibility",
        "research-packets" => "research_packet",
        "review-ledger" => "review_evidence",
        "source-witnesses" => "source_witness",
        "zarathustra" => "golden_route_orientation",
        _ => "repository",
    }
}
fn branch(path: &str) -> String {
    let owner = owner(path);
    if owner == "ToS" {
        "ToS".into()
    } else {
        format!("ToS/{owner}")
    }
}
fn kind(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let suffix = name
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(_, suffix)| suffix.to_lowercase())
        .unwrap_or_default();
    let known = match name {
        "AGENTS.md" => Some("route_card"),
        "source_home.manifest.json" => Some("source_home_manifest"),
        "philosophy.manifest.json" => Some("philosophy_manifest"),
        "branch.manifest.json" => Some("branch_manifest"),
        "node.json" => Some("node_payload"),
        "edges.csv" => Some("relation_pack"),
        _ => None,
    };
    if let Some(kind) = known {
        return kind.into();
    }
    for (prefix, kind) in [
        ("ToS/source-witnesses/", "source_witness"),
        ("ToS/research-packets/", "research_packet"),
        ("ToS/review-ledger/", "review_note"),
    ] {
        if path.starts_with(prefix) {
            return kind.into();
        }
    }
    if path.starts_with("ToS/contracts/") && suffix == "json" {
        return "contract_schema".into();
    }
    if path.starts_with("ToS/derived-exports/") {
        return "derived_export".into();
    }
    match suffix.as_str() {
        "md" => "markdown".into(),
        "csv" => "tabular".into(),
        "json" => "json".into(),
        "xlsx" | "xls" => "binary".into(),
        "" => "file".into(),
        _ => suffix,
    }
}
fn push(
    rows: &mut Vec<Row>,
    bytes: &mut usize,
    collection: &str,
    id: &str,
    value: Value,
    limits: RepositorySourceLimits,
) -> Result<()> {
    let raw = serde_json::to_vec(&value).map_err(|_| Error::Invalid("repository source JSON"))?;
    *bytes = bytes
        .checked_add(raw.len() + id.len() + collection.len())
        .filter(|bytes| *bytes <= limits.max_plan_bytes)
        .ok_or(Error::Budget("repository cold plan bytes"))?;
    if raw.len() > limits.max_row_bytes {
        return Err(Error::Budget("repository source row"));
    }
    rows.push(Row {
        collection: collection.into(),
        id: id.into(),
        raw,
    });
    Ok(())
}

/// The capture must include all of ToS. Only the maintained payload/partition
/// exclusions are permitted. This proves completeness for that exact selected
/// Git tree and finite capture, not untracked checkout files or a newer tree.
#[allow(clippy::too_many_arguments)]
pub fn plan_repository_source_inputs(
    cut: &CorpusCutReader,
    expected_revision: SourceRevision,
    expected_membership: SourceMembershipV1,
    inventory: &SoftwareCaptureReader,
    expected_inventory: &SoftwareCaptureSelectionV1,
    job_source_cut: &str,
    vocabulary: &QueryVocabulary,
    root: RepositoryRootInput<'_>,
    validator: &mut impl CutSchemaExecutor,
    limits: RepositorySourceLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<RepositorySourcePlan> {
    limits.validate()?;
    check(deadline, cancelled)?;
    if cut.current().revision() != expected_revision
        || cut
            .stream(expected_revision)
            .map_err(|e| Error::Source(e.to_string()))?
            .expectation()
            != expected_membership
        || inventory.selection() != expected_inventory
        || !inventory.include_prefixes().iter().any(|p| p == "ToS")
        || inventory
            .exclude_path_parts()
            .iter()
            .any(|part| part != "payload" && part != ".git")
        || inventory.exclude_prefixes().iter().any(|prefix| {
            (prefix == "ToS" || prefix.starts_with("ToS/"))
                && prefix != SELF
                && !PARTS.contains(&prefix.as_str())
        })
        || root.source_cut != job_source_cut
        || job_source_cut.is_empty()
        || Digest256::of_bytes(root.material).to_hex() != root.material_sha256
        || root.material.len() > limits.max_row_bytes
        || root.identity_id.is_empty()
        || root.identity_id.len() > 4096
        || root.identity_id.contains('\0')
    {
        return Err(Error::Invalid(
            "repository independently selected source/inventory/root",
        ));
    }
    let registrations: Vec<_> = vocabulary
        .sources
        .iter()
        .filter(|source| source.adapter_profile == PROFILE)
        .collect();
    if registrations.len() != 1 {
        return Err(Error::Invalid("repository source registration"));
    }
    let registration = registrations[0];
    let mut members = BTreeMap::new();
    let mut member_root = Digest256Hasher::new();
    let mut count = 0u64;
    let mut metadata_bytes = root.material.len();
    for member in inventory.members() {
        check(deadline, cancelled)?;
        count = count
            .checked_add(1)
            .filter(|count| *count <= limits.max_inventory_members)
            .ok_or(Error::Budget("repository inventory count"))?;
        let path = member.path.as_str();
        member_root.update(&(path.len() as u64).to_be_bytes());
        member_root.update(path.as_bytes());
        member_root.update(&member.size_bytes.to_be_bytes());
        member_root.update(member.sha256.as_bytes());
        member_root.update(&member.mode.to_be_bytes());
        if selected_path(path) {
            metadata_bytes = metadata_bytes
                .checked_add(path.len() + 64)
                .filter(|n| *n <= limits.max_plan_bytes)
                .ok_or(Error::Budget("repository inventory metadata bytes"))?;
            members.insert(path.to_owned(), member);
        }
    }
    let mut seen = 0u64;
    for member in cut.current().members() {
        check(deadline, cancelled)?;
        seen += 1;
        if seen > limits.max_inventory_members {
            return Err(Error::Budget("repository source membership"));
        }
        if selected_path(member.path.as_str())
            && members.get(member.path.as_str()).copied() != Some(member)
        {
            return Err(Error::Invalid("repository capture/source member differs"));
        }
    }
    if seen != expected_membership.count {
        return Err(Error::Invalid("repository source membership EOF"));
    }
    let home_path = RelativePath::parse(HOME).map_err(|e| Error::Source(e.to_string()))?;
    let home = cut
        .read_member(
            expected_revision,
            &home_path,
            limits.max_source_bytes as u64,
            deadline,
            cancelled,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
    if !validator
        .check(
            HOME,
            &home.raw,
            "ToS/contracts/tos-source-home.schema.json",
            deadline,
            cancelled,
        )
        .map_err(|e| Error::Source(format!("repository source schema: {e:?}")))?
    {
        return Err(Error::Invalid("repository source home schema"));
    }
    let home_value =
        crate::knowledge_normalization::SourceRow::parse(&home.raw, limits.max_source_bytes)?;
    let mut rows = Vec::new();
    let mut output_bytes = metadata_bytes;
    let mut work_bytes = home.raw.len() as u64;
    let branches = home_value
        .value()
        .get("branches")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("repository source branches"))?;
    let mut branch_ids = BTreeSet::new();
    for (ordinal, value) in branches.iter().enumerate() {
        check(deadline, cancelled)?;
        let id = field(value, "id")?;
        let path = field(value, "path")?;
        let surface = field(value, "owner_surface")?;
        if !branch_ids.insert(id)
            || !members.contains_key(surface)
            || !members
                .keys()
                .any(|member| member.starts_with(&format!("{}/", path.trim_end_matches('/'))))
        {
            return Err(Error::Invalid("repository branch declaration route"));
        }
        // A branch path denotes a directory: unlike file ownership, its final
        // component is part of the branch route.
        let layer = authority(&format!("{}/", path.trim_end_matches('/')));
        push(
            &mut rows,
            &mut output_bytes,
            "branches",
            id,
            json!({"id":id,"path":path,"owner_surface":surface,
            "authority_layer":layer,"role":field(value,"role")?}),
            limits,
        )?;
        push(
            &mut rows,
            &mut output_bytes,
            "source_order",
            &format!("branches:{id}"),
            json!({"collection":"branches","id":id,"ordinal":ordinal}),
            limits,
        )?;
    }
    for (ordinal, (path, member)) in members.iter().enumerate() {
        check(deadline, cancelled)?;
        push(
            &mut rows,
            &mut output_bytes,
            "resources",
            path,
            json!({"path":path,"resource_kind":kind(path),
            "owner_branch":branch(path),"authority_layer":authority(path),"sha256":member.sha256.to_hex(),"size_bytes":member.size_bytes}),
            limits,
        )?;
        push(
            &mut rows,
            &mut output_bytes,
            "source_order",
            &format!("resources:{path}"),
            json!({"collection":"resources","id":path,"ordinal":ordinal}),
            limits,
        )?;
    }
    let mut manifest_ordinal = 0usize;
    for (path, member) in &members {
        if !path.ends_with(".manifest.json") {
            continue;
        }
        check(deadline, cancelled)?;
        let relative = RelativePath::parse(path).map_err(|e| Error::Source(e.to_string()))?;
        let raw = if cut.current().member(&relative).is_some() {
            cut.read_member(
                expected_revision,
                &relative,
                limits.max_source_bytes as u64,
                deadline,
                cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))?
            .raw
        } else {
            let selection = inventory
                .select_components(&[relative.clone()])
                .map_err(|e| Error::Source(e.to_string()))?;
            inventory
                .read_selected_component(
                    &selection,
                    &relative,
                    limits.max_source_bytes as u64,
                    deadline,
                    cancelled,
                )
                .map_err(|e| Error::Source(e.to_string()))?
        };
        work_bytes = work_bytes
            .checked_add(raw.len() as u64)
            .filter(|n| *n <= limits.max_work_bytes)
            .ok_or(Error::Budget("repository source work bytes"))?;
        if raw.len() as u64 != member.size_bytes || Digest256::of_bytes(&raw) != member.sha256 {
            return Err(Error::Invalid("repository manifest source digest/size"));
        }
        let parsed =
            crate::knowledge_normalization::SourceRow::parse(&raw, limits.max_source_bytes)?;
        let value = parsed.value();
        let declared = if path == HOME {
            value.get("home")
        } else {
            value.get("path")
        }
        .and_then(Value::as_str);
        push(
            &mut rows,
            &mut output_bytes,
            "manifests",
            path,
            json!({"path":path,"manifest_kind":kind(path),
            "owner_branch":branch(path),"authority_layer":authority(path),"schema_version":field(value,"schema_version")?,
            "branch_id":value.get("branch_id").and_then(Value::as_str),"declared_path":declared,"sha256":member.sha256.to_hex()}),
            limits,
        )?;
        push(
            &mut rows,
            &mut output_bytes,
            "source_order",
            &format!("manifests:{path}"),
            json!({"collection":"manifests","id":path,"ordinal":manifest_ordinal}),
            limits,
        )?;
        manifest_ordinal += 1;
    }
    if !members.contains_key(HOME) {
        return Err(Error::Invalid("repository inventory source home absent"));
    }
    if work_bytes
        .checked_add(output_bytes as u64)
        .is_none_or(|n| n > limits.max_work_bytes)
    {
        return Err(Error::Budget("repository total source and projection work"));
    }
    rows.sort_by(|a, b| (&a.collection, &a.id).cmp(&(&b.collection, &b.id)));
    let mut collections = Vec::new();
    for collection in ["branches", "manifests", "resources", "source_order"] {
        let mut root = Digest256Hasher::new();
        let mut count = 0u64;
        for row in rows.iter().filter(|row| row.collection == collection) {
            root.update(&(row.id.len() as u64).to_be_bytes());
            root.update(row.id.as_bytes());
            root.update(Digest256::of_bytes(&row.raw).as_bytes());
            count += 1;
        }
        collections.push(InputCollectionReceipt {
            source_graph: registration.source_graph_id.clone(),
            collection: collection.into(),
            input_role: registration.input_role.clone(),
            adapter_profile: PROFILE.into(),
            expected_count: count,
            expected_root_sha256: root.finalize().to_hex(),
        });
    }
    Ok(RepositorySourcePlan {
        receipt: RepositorySourceReceipt {
            source_revision: expected_revision.0.to_hex(),
            source_membership: expected_membership,
            inventory_selection: expected_inventory.clone(),
            inventory_members: count,
            inventory_root_sha256: member_root.finalize().to_hex(),
            collections,
            plan_bytes: output_bytes,
        },
        rows,
        job_source_cut: job_source_cut.into(),
        descriptor_sha256: vocabulary.descriptor_sha256.clone(),
        root_material: root.material.to_vec(),
        root_material_sha256: root.material_sha256.into(),
        root_identity: root.identity_id.into(),
    })
}

pub fn render_repository_source_plan(
    stage: &mut KnowledgeStage<'_>,
    plan: &RepositorySourcePlan,
    vocabulary: &QueryVocabulary,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let result = (|| {
        if stage.exact_receipt().binding.source_cut != plan.job_source_cut
            || vocabulary.descriptor_sha256 != plan.descriptor_sha256
        {
            return Err(Error::Invalid("repository source plan target binding"));
        }
        for expected in &plan.receipt.collections {
            let matches: Vec<_> = stage
                .exact_receipt()
                .collections
                .iter()
                .filter(|row| {
                    row.source_graph == expected.source_graph
                        && row.collection == expected.collection
                })
                .collect();
            if matches.len() != 1
                || matches[0].input_role != expected.input_role
                || matches[0].adapter_profile != expected.adapter_profile
                || matches[0].expected_count != expected.expected_count
                || matches[0].expected_root_sha256 != expected.expected_root_sha256
            {
                return Err(Error::Invalid("repository source plan target collection"));
            }
        }
        let source = &plan.receipt.collections[0].source_graph;
        if stage
            .exact_receipt()
            .collections
            .iter()
            .filter(|row| &row.source_graph == source)
            .count()
            != 4
        {
            return Err(Error::Invalid(
                "repository source plan complete target recipe",
            ));
        }
        for row in &plan.rows {
            check(deadline, cancelled)?;
            stage.ingest_input(InputRow {
                source_graph: source,
                collection: &row.collection,
                id: &row.id,
                payload: &row.raw,
            })?;
        }
        Ok(())
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
