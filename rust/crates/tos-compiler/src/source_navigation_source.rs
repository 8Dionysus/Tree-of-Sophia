//! Whole authored SourceNavigation composition. The current cut and catalogue
//! retain custody; this private projection does not grant rights or admission.
use crate::knowledge_stage::KnowledgeStage;
use crate::source_bibliographic::{
    BibliographicForms, BibliographicLimits, BibliographicSourceCut,
};
use crate::source_bibliographic_render::{array, encode, text};
use crate::source_bibliographic_versions::Versions;
use crate::source_witness_catalog::{
    self as catalog, SourceCatalogReceipt, SourceCatalogValidator,
};
use crate::{Error, Result};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

// Only exact immutable source results enter these maps. The caller's aggregate
// row/byte caps bound retained memory as well as final JSON serialization.
struct Projection<'a> {
    nodes: BTreeMap<String, Value>,
    edges: BTreeMap<String, Value>,
    rights: Vec<Value>,
    diagnostics: Vec<Value>,
    bytes: usize,
    limits: BibliographicLimits,
    cancelled: &'a std::sync::atomic::AtomicBool,
}
impl Projection<'_> {
    fn charge(&mut self, row: &Value) -> Result<()> {
        if self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
            || std::time::Instant::now() >= self.limits.deadline
        {
            return Err(Error::Budget("navigation source cancelled/deadline"));
        }
        let size = encode(row, self.limits.catalog.max_output_row_bytes)?.len();
        self.bytes = self
            .bytes
            .checked_add(size)
            .filter(|n| *n as u64 <= self.limits.max_output_bytes)
            .ok_or(Error::Budget("navigation whole projection bytes"))?;
        if (self.nodes.len() + self.edges.len() + self.rights.len() + self.diagnostics.len()) as u64
            >= self.limits.max_output_rows
        {
            return Err(Error::Budget("navigation whole projection rows"));
        }
        Ok(())
    }
    fn diagnostic(&mut self, level: &str, path: &str, message: String) -> Result<()> {
        let row = json!({"level":level,"path":path,"message":message});
        self.charge(&row)?;
        self.diagnostics.push(row);
        Ok(())
    }
    fn node(&mut self, row: Value) -> Result<()> {
        let id = text(&row, "node_id")?.to_owned();
        if let Some(existing) = self.nodes.get(&id) {
            if existing != &row {
                return self.diagnostic(
                    "error",
                    text(&row, "source_ref")?,
                    format!("source-navigation node {id} has conflicting projections"),
                );
            }
            return Ok(());
        }
        self.charge(&row)?;
        self.nodes.insert(id, row);
        Ok(())
    }
    fn edge(&mut self, row: Value) -> Result<()> {
        let id = text(&row, "edge_id")?.to_owned();
        if let Some(existing) = self.edges.get(&id) {
            if existing != &row {
                let path = row["source_refs"]
                    .as_array()
                    .and_then(|r| r.first())
                    .and_then(Value::as_str)
                    .unwrap_or("ToS");
                return self.diagnostic(
                    "error",
                    path,
                    format!("source-navigation edge {id} has conflicting projections"),
                );
            }
            return Ok(());
        }
        self.charge(&row)?;
        self.edges.insert(id, row);
        Ok(())
    }
    fn rights(&mut self, record: &Value, source: &str) -> Result<()> {
        let mut assessments = vec![(record, "aggregate")];
        if let Some(layers) = record.get("layer_assessments") {
            assessments.extend(
                layers
                    .as_array()
                    .ok_or(Error::Invalid("navigation rights layer assessments"))?
                    .iter()
                    .filter(|v| v.is_object())
                    .map(|v| (v, "layer")),
            );
        }
        for (index, (assessment, kind)) in assessments.into_iter().enumerate() {
            let fallback_id = format!("{source}#{index}");
            let id = assessment
                .get("layer_id")
                .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                .or_else(|| record.get("rights_id"))
                .cloned()
                .unwrap_or(json!(fallback_id));
            let mut refs = assessment
                .get("scope_refs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>();
            refs.sort();
            let mut row = json!({"rights_id":id,"assessment_kind":kind,
                "scope_refs":refs,"visibility":record.get("visibility").cloned().unwrap_or(json!("unknown")),
                "source_ref":source});
            for field in [
                "assessment_status",
                "review_status",
                "redistribution_posture",
                "derivative_posture",
                "server_processing_posture",
            ] {
                row[field] = assessment
                    .get(field)
                    .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                    .or_else(|| {
                        if field == "assessment_status" {
                            None
                        } else {
                            record.get(field)
                        }
                    })
                    .cloned()
                    .unwrap_or(json!("unknown"));
            }
            for field in ["license_uri", "rights_statement_uri"] {
                row[field] = assessment
                    .get(field)
                    .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                    .or_else(|| record.get(field))
                    .cloned()
                    .unwrap_or(Value::Null);
            }
            row["restrictions"] = assessment
                .get("restrictions")
                .or_else(|| record.get("restrictions"))
                .cloned()
                .unwrap_or(json!([]));
            self.charge(&row)?;
            self.rights.push(row);
        }
        Ok(())
    }
}

fn node(id: &str, kind: &str, label: &str, source: &str, status: &str, properties: Value) -> Value {
    json!({"node_id":id,"node_kind":kind,"label":label,"source_ref":source,
        "identity_status":status,"properties":properties})
}
fn edge(id: &str, left: &str, predicate: &str, right: &str, kind: &str, refs: &[&str]) -> Value {
    let refs = refs.iter().copied().collect::<BTreeSet<_>>();
    json!({"edge_id":id,"from_id":left,"predicate_id":predicate,"to_id":right,
        "edge_kind":kind,"review_status":"not_applicable","source_refs":refs})
}
fn branch_kind(path: &str) -> &'static str {
    let parts = path.split('/').collect::<Vec<_>>();
    for (name, kind) in [
        ("traditions", "tradition"),
        ("regions", "region"),
        ("eras", "era"),
    ] {
        if parts.iter().position(|p| *p == name) == parts.len().checked_sub(2) {
            return kind;
        }
    }
    "branch"
}

fn field<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}
fn visible(value: &Value) -> bool {
    matches!(
        value.get("visibility").and_then(Value::as_str),
        Some("public" | "public_payload" | "public_metadata_only")
    )
}
fn original(raw: &[u8], l: BibliographicLimits) -> Result<Value> {
    Ok(
        crate::knowledge_normalization::SourceRow::parse(raw, l.catalog.max_row_bytes)?
            .value()
            .clone(),
    )
}
fn python_title(value: &str) -> String {
    let mut previous = false;
    let mut result = String::new();
    for ch in value.chars() {
        let cased = ch.is_lowercase() || ch.is_uppercase();
        if cased && previous {
            result.extend(ch.to_lowercase());
        } else if cased {
            result.extend(ch.to_uppercase());
        } else {
            result.push(ch);
        }
        previous = cased;
    }
    result
}

/// Full ordinary SourceNavigation derived from one exact current cut/catalog.
/// History and immutable issuance refs resolve through the existing Versions
/// owner, including original retained archive/form custody. Rights rows describe
/// source declarations and grant no publication, payload access or assessment.
pub fn project_source_navigation_from_cut(
    stage: &mut KnowledgeStage<'_>,
    receipt: &SourceCatalogReceipt,
    source: &BibliographicSourceCut<'_>,
    validator: &SourceCatalogValidator<'_>,
    entities: &Value,
    forms: &mut dyn BibliographicForms,
    l: BibliographicLimits,
) -> Result<NavigationSourceProjection> {
    let result = (|| {
        l.validate()?;
        if receipt.worker_sha256 != validator.worker.sha256.to_hex() {
            return Err(Error::Invalid("navigation exact catalog worker identity"));
        }
        let mut versions = Versions::new(source, stage, validator, receipt, l)?;
        let selected_entities = original(
            &versions.required(
                "ToS/doctrine/semantic-interchange/entity-types.v1.json",
                validator,
                l,
            )?,
            l,
        )?;
        if &selected_entities != entities {
            return Err(Error::Invalid("navigation exact selected entity registry"));
        }
        let mut projection = Projection {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            rights: Vec::new(),
            diagnostics: Vec::new(),
            bytes: 0,
            limits: l,
            cancelled: validator.cancelled,
        };
        let mut paths = Vec::new();
        let mut path_bytes = 0usize;
        for member in source.cut.current().members() {
            if paths.len() >= source.max_read_files {
                return Err(Error::Budget("navigation current inventory rows"));
            }
            path_bytes = path_bytes
                .checked_add(member.path.as_str().len())
                .filter(|n| *n <= source.max_read_bytes)
                .ok_or(Error::Budget("navigation current inventory bytes"))?;
            paths.push(member.path.as_str().to_owned());
        }
        if paths.len() as u64 != source.expected_membership.count {
            return Err(Error::Invalid("navigation complete selected inventory"));
        }
        let annotation_report = inspect_annotation_owner(source, &paths, validator, l)?;
        let mut branches = BTreeMap::<String, Value>::new();
        for path in paths.iter().filter(|p| {
            p.starts_with("ToS/philosophy/eras/") && p.ends_with("/branch.manifest.json")
        }) {
            let manifest = original(&versions.required(path, validator, l)?, l)?;
            let (Some(id), Some(branch)) =
                (field(&manifest, "branch_id"), field(&manifest, "path"))
            else {
                continue;
            };
            let label = field(&manifest, "role")
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    python_title(
                        &branch
                            .rsplit('/')
                            .next()
                            .unwrap_or(branch)
                            .replace('-', " "),
                    )
                });
            projection.node(node(id,branch_kind(branch),&label,path,"not_applicable",
                json!({"branch_path":branch,"role":manifest.get("role").and_then(Value::as_str).unwrap_or("")})))?;
            branches.insert(branch.into(), manifest);
        }
        for (child, manifest) in &branches {
            let mut parent = child.as_str();
            while let Some((next, _)) = parent.rsplit_once('/') {
                parent = next;
                if let Some(owner) = branches.get(parent) {
                    let left = text(owner, "branch_id")?;
                    let right = text(manifest, "branch_id")?;
                    projection.edge(edge(
                        &format!("source-navigation:branch:{left}:{right}"),
                        left,
                        "contains",
                        right,
                        "authored_branch_hierarchy",
                        &[&format!("{child}/branch.manifest.json")],
                    ))?;
                    break;
                }
            }
        }
        let mut after = None;
        while let Some(id) = catalog::catalog_next(stage, "records", after.as_deref())? {
            let output =
                crate::source_bibliographic_navigation::prepare_navigation_record_from_catalog(
                    stage,
                    receipt,
                    &mut versions,
                    &id,
                    entities,
                    validator,
                    forms,
                    l,
                )?;
            for row in output.nodes {
                projection.node(row)?;
            }
            for row in output.edges {
                projection.edge(row)?;
            }
            for row in output.diagnostics {
                projection.charge(&row)?;
                projection.diagnostics.push(row);
            }
            after = Some(id);
        }
        for path in paths.iter().filter(|p| {
            p.starts_with("ToS/philosophy/eras/") && p.ends_with("/source-planting.json")
        }) {
            let planting = original(&versions.required(path, validator, l)?, l)?;
            let (Some(id), Some(branch), Some(witness)) = (
                field(&planting, "planting_id"),
                field(&planting, "branch_path"),
                planting.get("source_witness").filter(|v| v.is_object()),
            ) else {
                continue;
            };
            catalog::check_catalog_schema(
                stage,
                validator,
                l.catalog,
                "ToS/contracts/philosophy-source-planting.schema.json",
                "",
                &encode(&planting, l.catalog.max_file_bytes)?,
            )?;
            let label = planting
                .pointer("/source_backlog_anchor/source_label")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(id);
            let status = planting
                .pointer("/authority/source_status")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("unknown");
            projection.node(node(id,"source_planting",label,path,status,json!({"status":planting["status"],"discovery_ref":planting["discovery_ref"],"research_ref":planting["research_ref"]})))?;
            if let Some(owner) = branches.get(branch) {
                let branch_id = text(owner, "branch_id")?;
                projection.edge(edge(
                    &format!("source-navigation:planting:{branch_id}:{id}"),
                    branch_id,
                    "has_source_planting",
                    id,
                    "authored_source_planting",
                    &[path],
                ))?;
            }
            if let Some(witness_id) = ["work_id", "artifact_id", "composite_id", "item_id"]
                .into_iter()
                .find_map(|key| field(witness, key))
            {
                let witness_ref = field(witness, "record_ref").unwrap_or(path);
                if !projection.nodes.contains_key(witness_id) {
                    projection.node(node(
                        witness_id,
                        "source_witness",
                        witness_id,
                        witness_ref,
                        "provisional",
                        json!({}),
                    ))?;
                }
                projection.edge(edge(
                    &format!("source-navigation:witness:{id}:{witness_id}"),
                    id,
                    field(witness, "relationship").unwrap_or("references_source_witness"),
                    witness_id,
                    "authored_source_planting",
                    &[path, witness_ref],
                ))?;
            }
        }
        for basename in [
            "work-expression-claims.jsonl",
            "expression-edition-claims.jsonl",
            "edition-item-claims.jsonl",
            "object-link-claims.jsonl",
            "responsibility-claims.jsonl",
        ] {
            for path in paths.iter().filter(|p| {
                p.starts_with("ToS/source-witnesses/") && p.rsplit('/').next() == Some(basename)
            }) {
                let raw = versions.required(path, validator, l)?;
                let lines = claim_lines(
                    &raw,
                    path == "ToS/source-witnesses/relations/object-link/object-link-claims.jsonl",
                )?;
                for (line, content) in lines {
                    if content
                        .trim_matches(crate::source_philosophy_support::source_space)
                        .is_empty()
                    {
                        continue;
                    }
                    let claim = original(content.as_bytes(), l)?;
                    if !visible(&claim) {
                        continue;
                    }
                    let (Some(left), Some(right), Some(id)) = (
                        field(&claim, "subject_ref"),
                        field(&claim, "object"),
                        field(&claim, "claim_id"),
                    ) else {
                        continue;
                    };
                    if !projection.nodes.contains_key(left) || !projection.nodes.contains_key(right)
                    {
                        projection.diagnostic(
                            "error",
                            path,
                            format!("source-navigation claim {id} has an unresolved endpoint"),
                        )?;
                        continue;
                    }
                    let mut refs = vec![path.as_str()];
                    for evidence in array(&claim, "evidence_refs")? {
                        refs.push(
                            evidence
                                .as_str()
                                .ok_or(Error::Invalid("navigation legacy evidence reference"))?,
                        );
                    }
                    let mut projected = edge(
                        &format!("source-navigation:claim:{id}"),
                        left,
                        field(&claim, "predicate").unwrap_or("related_to"),
                        right,
                        "evidence_claim",
                        &refs,
                    );
                    projected["review_status"] =
                        json!(field(&claim, "review_status").unwrap_or("unknown"));
                    projected["claim_ref"] = json!(id);
                    if path == "ToS/source-witnesses/relations/object-link/object-link-claims.jsonl"
                    {
                        let (bound, location) =
                            crate::source_bibliographic::slot(stage, "claim", id, l)?.ok_or(
                                Error::Invalid("navigation retained object-link slot absent"),
                            )?;
                        if bound != claim
                            || location["source_ref"] != json!(path)
                            || location["source_line"] != json!(line)
                        {
                            return Err(Error::Invalid(
                                "navigation retained object-link source binding",
                            ));
                        }
                        projected["properties"] = json!({"source_claim":claim,"source_claim_file_ref":path,
                            "source_claim_line":line,"source_sha256":crate::source_bibliographic_render::digest(&claim,l.catalog.max_row_bytes)?,
                            "source_schema_ref":"ToS/contracts/object-link-claim.schema.json","source_adapter":"retained-object-link-v1"});
                    }
                    projection.edge(projected)?;
                }
            }
        }
        project_files(stage, &mut projection, &mut versions, &paths, validator, l)?;
        for path in paths
            .iter()
            .filter(|p| p.starts_with("ToS/source-witnesses/") && packet_name(p))
        {
            if path
                .split('/')
                .any(|p| matches!(p, "payload" | "local-content"))
            {
                continue;
            }
            let raw = versions.required(path, validator, l)?;
            let packet = crate::source_navigation_packets::project_packet(
                stage,
                &raw,
                path,
                validator,
                l.catalog,
                l.catalog.max_row_bytes,
                l.max_claim_cohort_bytes,
                l.max_claim_cohort_rows,
                l.deadline,
                validator.cancelled,
            )?;
            for row in packet.nodes {
                projection.node(row)?;
            }
            for row in packet.edges {
                let left = text(&row, "from_id")?;
                let right = text(&row, "to_id")?;
                if !projection.nodes.contains_key(left) || !projection.nodes.contains_key(right) {
                    projection.diagnostic(
                        "warning",
                        path,
                        format!(
                            "text spine endpoint not projected: {}",
                            text(&row, "edge_id")?
                        ),
                    )?;
                    continue;
                }
                projection.edge(row)?;
            }
        }
        for path in paths
            .iter()
            .filter(|p| p.starts_with("ToS/source-witnesses/") && p.ends_with("/rights.json"))
        {
            let raw = versions.required(path, validator, l)?;
            let rights = original(&raw, l)?;
            if visible(&rights) {
                catalog::check_catalog_schema(
                    stage,
                    validator,
                    l.catalog,
                    "ToS/contracts/rights-record.schema.json",
                    "",
                    &raw,
                )?;
                projection.rights(&rights, path)?;
            }
        }
        versions.verify_catalog_binding(stage, receipt, l)?;
        projection
            .rights
            .sort_by(|a, b| a["rights_id"].as_str().cmp(&b["rights_id"].as_str()));
        let counts = json!({"nodes":projection.nodes.len(),"edges":projection.edges.len(),"rights":projection.rights.len()});
        let value = json!({"schema_version":"tos_source_navigation_v1","authority_boundary":"generated read-only navigation; authored branch manifests, source records, claims, item manifests, and rights records retain authority",
            "counts":counts,"nodes":projection.nodes.into_values().collect::<Vec<_>>(),
            "edges":projection.edges.into_values().collect::<Vec<_>>(),"rights":projection.rights});
        encode(
            &value,
            usize::try_from(l.max_output_bytes)
                .map_err(|_| Error::Budget("navigation output usize"))?,
        )?;
        Ok(NavigationSourceProjection {
            value,
            diagnostics: projection.diagnostics,
            source_binding: versions.binding(),
            catalog_root_sha256: receipt.row_root_sha256.clone(),
            annotation_report,
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
pub struct NavigationSourceProjection {
    pub(crate) value: Value,
    pub(crate) diagnostics: Vec<Value>,
    pub(crate) source_binding: Value,
    pub(crate) catalog_root_sha256: String,
    pub annotation_report: tos_validation::layer_family_rules::LayerFamilyReport,
}
impl NavigationSourceProjection {
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn diagnostics(&self) -> &[Value] {
        &self.diagnostics
    }
}
fn packet_name(path: &str) -> bool {
    if path
        .split('/')
        .any(|part| matches!(part, "payload" | "local-content"))
    {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    (name.starts_with("source-text-unit") && name.ends_with(".json"))
        || name.ends_with(".source-text-unit.v1.json")
        || (name.starts_with("semantic-annotation") && name.ends_with(".json"))
}

struct FileGroup {
    digest: Value,
    size: Value,
    media: Value,
    refs: BTreeSet<String>,
    memberships: BTreeMap<String, BTreeMap<String, Value>>,
    invalid: bool,
}
fn project_files(
    stage: &KnowledgeStage<'_>,
    projection: &mut Projection,
    versions: &mut Versions<'_, '_>,
    paths: &[String],
    validator: &SourceCatalogValidator<'_>,
    l: BibliographicLimits,
) -> Result<()> {
    let mut files = BTreeMap::<String, FileGroup>::new();
    let mut diagnostics = Vec::new();
    let mut report = |path: &str, message: String| {
        diagnostics.push(json!({"level":"error","path":path,"message":message}));
    };
    for path in paths
        .iter()
        .filter(|p| p.starts_with("ToS/source-witnesses/") && p.ends_with("/item.manifest.json"))
    {
        let raw = versions.required(path, validator, l)?;
        let manifest = original(&raw, l)?;
        let Some(item) =
            field(&manifest, "item_id").filter(|id| projection.nodes.contains_key(*id))
        else {
            continue;
        };
        catalog::check_catalog_schema(
            stage,
            validator,
            l.catalog,
            "ToS/contracts/source-item-manifest.schema.json",
            "",
            &raw,
        )?;
        let acquisition = field(&manifest, "acquisition_event_ref");
        let rights = field(&manifest, "rights_ref");
        if acquisition.is_none() {
            report(
                path,
                "source-navigation Item manifest has no acquisition event reference".into(),
            );
        }
        if rights.is_none() {
            report(
                path,
                "source-navigation Item manifest has no rights reference".into(),
            );
        }
        let entries = match manifest.get("payload_files") {
            None => &[][..],
            Some(Value::Array(entries)) => entries.as_slice(),
            _ => {
                report(
                    path,
                    "source-navigation Item manifest payload_files is not an array".into(),
                );
                continue;
            }
        };
        let mut entries = entries
            .iter()
            .map(|row| Ok((encode(row, l.catalog.max_output_row_bytes)?, row)))
            .collect::<Result<Vec<_>>>()?;
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, entry) in entries {
            if !entry.is_object() {
                report(
                    path,
                    "source-navigation Item manifest payload entry is not an object".into(),
                );
                continue;
            }
            let (Some(id), Some(digest), Some(media)) = (
                field(entry, "file_id"),
                field(entry, "sha256"),
                field(entry, "media_type"),
            ) else {
                report(
                    path,
                    "source-navigation Item manifest payload entry has incomplete File identity"
                        .into(),
                );
                continue;
            };
            let Some(size) = entry.get("byte_size") else {
                report(
                    path,
                    format!("source-navigation File {id} has invalid byte_size"),
                );
                continue;
            };
            if !files.contains_key(id) {
                projection.charge(entry)?;
            }
            let group = files.entry(id.into()).or_insert_with(|| FileGroup {
                digest: json!(digest),
                size: size.clone(),
                media: json!(media),
                refs: BTreeSet::new(),
                memberships: BTreeMap::new(),
                invalid: false,
            });
            if id != format!("tos.file.sha256.{digest}") {
                report(
                    path,
                    format!("source-navigation File ID {id} differs from its payload digest"),
                );
                group.invalid = true;
            }
            if !size
                .as_number()
                .is_some_and(|n| n.to_string().bytes().all(|c| c.is_ascii_digit()))
            {
                report(
                    path,
                    format!("source-navigation File {id} has invalid byte_size"),
                );
                group.invalid = true;
            }
            for (field, expected) in [
                ("sha256", &group.digest),
                ("byte_size", &group.size),
                ("media_type", &group.media),
            ] {
                if entry.get(field) != Some(expected) {
                    report(
                        path,
                        format!("source-navigation File {id} has conflicting {field}"),
                    );
                    group.invalid = true;
                }
            }
            group.refs.insert(path.clone());
            let (Some(acquisition), Some(rights)) = (acquisition, rights) else {
                group.invalid = true;
                continue;
            };
            let (Some(relative), Some(basename), Some(fixity)) = (
                field(entry, "relative_path"),
                field(entry, "original_basename"),
                field(entry, "fixity_verified_at"),
            ) else {
                report(
                    path,
                    format!("source-navigation File {id} has incomplete Item membership context"),
                );
                group.invalid = true;
                continue;
            };
            let container = entry
                .get("container_member")
                .cloned()
                .unwrap_or(json!(false));
            if !container.is_boolean() {
                report(
                    path,
                    format!("source-navigation File {id} has incomplete Item membership context"),
                );
                group.invalid = true;
                continue;
            }
            let context=group.memberships.entry(item.into()).or_default().entry(path.clone()).or_insert_with(||json!({"acquisition_event_ref":acquisition,"rights_ref":rights,"payload_entries":[]}));
            if context["acquisition_event_ref"] != json!(acquisition)
                || context["rights_ref"] != json!(rights)
            {
                report(
                    path,
                    format!(
                        "source-navigation Item manifest {path} has conflicting acquisition or rights references"
                    ),
                );
                group.invalid = true;
            }
            let membership = json!({"relative_path":relative,"original_basename":basename,"fixity_verified_at":fixity,"container_member":container});
            projection.charge(&membership)?;
            context["payload_entries"]
                .as_array_mut()
                .ok_or(Error::Invalid("navigation private file context"))?
                .push(membership);
        }
    }
    drop(report);
    diagnostics.sort_by(|a, b| {
        ["path", "message", "level"]
            .iter()
            .map(|key| a[*key].as_str())
            .cmp(
                ["path", "message", "level"]
                    .iter()
                    .map(|key| b[*key].as_str()),
            )
    });
    for diagnostic in diagnostics {
        projection.charge(&diagnostic)?;
        projection.diagnostics.push(diagnostic);
    }
    for (id, group) in files {
        if group.invalid {
            continue;
        }
        let refs = group.refs.into_iter().collect::<Vec<_>>();
        let digest = group
            .digest
            .as_str()
            .ok_or(Error::Invalid("navigation file digest"))?;
        let mut projected = node(
            &id,
            "file",
            &format!("SHA-256 {}", digest.chars().take(12).collect::<String>()),
            refs.first()
                .ok_or(Error::Invalid("navigation file manifest closure"))?,
            "content_addressed",
            json!({"media_type":group.media,"byte_size":group.size,"sha256":group.digest}),
        );
        projected["source_refs"] = json!(refs);
        projection.node(projected)?;
        for (item, contexts) in group.memberships {
            let mut refs = Vec::new();
            let mut memberships = Vec::new();
            for (manifest, mut context) in contexts {
                let entries = context["payload_entries"]
                    .as_array_mut()
                    .ok_or(Error::Invalid("navigation file context array"))?;
                entries.sort_by(|a, b| {
                    ["relative_path", "original_basename", "fixity_verified_at"]
                        .iter()
                        .map(|k| a[*k].as_str())
                        .cmp(
                            ["relative_path", "original_basename", "fixity_verified_at"]
                                .iter()
                                .map(|k| b[*k].as_str()),
                        )
                        .then_with(|| {
                            a["container_member"]
                                .as_bool()
                                .cmp(&b["container_member"].as_bool())
                        })
                });
                refs.push(manifest.clone());
                context["manifest_ref"] = json!(manifest);
                memberships.push(context);
            }
            let refs = refs.iter().map(String::as_str).collect::<Vec<_>>();
            let mut projected = edge(
                &format!("source-navigation:file:{item}:{id}"),
                &item,
                "has_file",
                &id,
                "authored_item_manifest",
                &refs,
            );
            projected["properties"] = json!({"item_file_contexts":memberships});
            projection.edge(projected)?;
        }
    }
    Ok(())
}

fn claim_lines(raw: &[u8], byte_lines: bool) -> Result<Vec<(u64, &str)>> {
    let value = std::str::from_utf8(raw).map_err(|_| Error::Invalid("navigation claim UTF8"))?;
    let mut rows = Vec::new();
    let mut start = 0;
    let mut number = 1;
    let mut positions = value.char_indices().peekable();
    while let Some((offset, ch)) = positions.next() {
        let boundary = matches!(ch, '\n' | '\r')
            || (!byte_lines
                && matches!(
                    ch,
                    '\u{b}'
                        | '\u{c}'
                        | '\u{1c}'
                        | '\u{1d}'
                        | '\u{1e}'
                        | '\u{85}'
                        | '\u{2028}'
                        | '\u{2029}'
                ));
        if boundary {
            rows.push((number, &value[start..offset]));
            number += 1;
            start = offset + ch.len_utf8();
            if ch == '\r' && positions.peek().is_some_and(|(_, ch)| *ch == '\n') {
                let (offset, ch) = positions.next().expect("checked CRLF");
                start = offset + ch.len_utf8();
            }
        }
    }
    if start < value.len() {
        rows.push((number, &value[start..]));
    }
    Ok(rows)
}
fn inspect_annotation_owner(
    source: &BibliographicSourceCut<'_>,
    paths: &[String],
    validator: &SourceCatalogValidator<'_>,
    l: BibliographicLimits,
) -> Result<tos_validation::layer_family_rules::LayerFamilyReport> {
    use tos_validation::layer_family_cut::{CutLayerFamilySource, UnavailableLayerPayloads};
    use tos_validation::layer_family_rules::LayerFamilyRules;
    let mut schemas = validator.schemas(source.expected_revision)?;
    if schemas.source_revision() != source.expected_revision
        || source.cut.current().revision() != source.expected_revision
    {
        return Err(Error::Invalid("navigation annotation source revision"));
    }
    let limits = tos_validation::item_rules::ItemLimits {
        max_member_bytes: l.catalog.max_file_bytes,
        max_total_bytes: source.max_read_bytes as u64,
        max_state_bytes: l.max_claim_cohort_bytes,
        max_issues: l.max_claim_cohort_rows,
        deadline: l.deadline,
    };
    let mut payloads = UnavailableLayerPayloads;
    let mut adapter = CutLayerFamilySource {
        cut: source.cut,
        schemas: &mut *schemas,
        cancelled: validator.cancelled,
        max_read_bytes: limits.max_total_bytes,
        read_bytes: 0,
        payloads: &mut payloads,
    };
    let mut rules = LayerFamilyRules::new(limits);
    for path in paths
        .iter()
        .filter(|p| p.starts_with("ToS/source-witnesses/") && packet_name(p))
    {
        let relative = tos_foundation::RelativePath::parse(path)
            .map_err(|_| Error::Invalid("navigation annotation path"))?;
        let member = source
            .cut
            .read_member(
                source.expected_revision,
                &relative,
                l.catalog.max_file_bytes as u64,
                l.deadline,
                validator.cancelled,
            )
            .map_err(|e| Error::Source(e.to_string()))?;
        let value = original(&member.raw, l)?;
        if value["schema_version"] == "tos_semantic_annotation_packet_v2" {
            rules.inspect(&mut adapter, path).map_err(|e| {
                Error::Source(format!("navigation owner annotation predicates:{e:?}"))
            })?;
        }
    }
    let report = rules.finish();
    if !report.issues.is_empty() || !report.unsupported.is_empty() {
        return Err(Error::Source(format!(
            "navigation owner annotation report issues={:?} unsupported={:?}",
            report.issues, report.unsupported
        )));
    }
    Ok(report)
}
