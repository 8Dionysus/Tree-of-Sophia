//! Addressed initial identity Claim catalog addition and complete raw cohorts.
//! Owner observation is held by the caller through its final derived commit.
//! This module neither creates source, selects roots, nor invents cut proof.
use super::{source_claim_publication_bytes as bytes, source_claim_publication_roots as roots};
use crate::source_command as cmd;
use crate::source_creation_store::CommittedClaimObservation;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use tos_compiler::source_bibliographic::{self as bib, BibliographicLimits};
use tos_compiler::{Error, Result};
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
const REGISTRY: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const CATALOG_SCHEMA: &str = "ToS/contracts/source-catalog-projection-v2.schema.json";
fn owner<T>(result: cmd::SourceCommandResult<T>) -> Result<T> {
    result.map_err(|e| Error::Source(format!("committed Claim observation: {e:?}")))
}
fn source_canonical(value: &Value) -> Result<Vec<u8>> {
    let raw =
        serde_json::to_vec(value).map_err(|_| Error::Invalid("Claim source JSON emission"))?;
    owner(cmd::canonical(&owner(cmd::parse(&raw))?))
}
fn path(reference: &str) -> Result<RelativePath> {
    RelativePath::parse(reference).map_err(|_| Error::Invalid("Claim assembly exact path"))
}
fn read(
    observation: &mut CommittedClaimObservation,
    reference: &str,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    Ok(owner(observation.read_selected(
        &path(reference)?,
        l.catalog.max_file_bytes,
        l.deadline,
        cancelled,
    ))?
    .to_vec())
}
fn check(
    worker: &mut CutWorkerSchemaExecutor,
    reference: &str,
    raw: &[u8],
    contract: &str,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<()> {
    match worker.check(reference, raw, contract, l.deadline, cancelled) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::Invalid("Claim addressed schema refused")),
        Err(e) => Err(Error::Source(format!(
            "Claim addressed schema execution: {e:?}"
        ))),
    }
}
fn check_resource(
    observation: &mut CommittedClaimObservation,
    worker: &CutWorkerSchemaExecutor,
    reference: &str,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<()> {
    let raw = read(observation, reference, l, cancelled)?;
    let value = bytes::parse(&raw, l.catalog.max_contract_bytes)?;
    if ![
        format!("https://tree-of-sophia.local/{reference}"),
        format!("https://treeofsophia.local/{reference}"),
    ]
    .iter()
    .any(|id| value["$id"] == *id)
        || worker.contract_digest(reference) != Some(Digest256::of_bytes(&raw))
    {
        return Err(Error::Invalid(
            "Claim selected schema and real executor differ",
        ));
    }
    Ok(())
}
pub(super) struct OrderedRow {
    pub value: Value,
    pub raw: Vec<u8>,
}
impl OrderedRow {
    fn new(value: Value, maximum: usize) -> Result<Self> {
        let raw = bytes::canonical(&value, maximum)?;
        Ok(Self { value, raw })
    }
}
pub(super) struct CatalogAddition {
    pub header: Value,
    pub changes: Vec<roots::Change>,
    pub claim_ids: Vec<String>,
}
pub(super) struct AssembledAddition {
    pub nodes: BTreeMap<(String, String), OrderedRow>,
    pub edges: BTreeMap<(String, String), OrderedRow>,
    pub traces: Vec<OrderedRow>,
    pub declarations: Vec<OrderedRow>,
}
fn put(
    rows: &mut BTreeMap<(String, String), OrderedRow>,
    field: &str,
    row: Value,
    max: usize,
) -> Result<()> {
    let key = (
        "source-claims".to_owned(),
        bytes::text(&row, field)?.to_owned(),
    );
    if let Some(old) = rows.get(&key) {
        if old.value != row {
            return Err(Error::Invalid("Claim raw cohort conflicting row"));
        }
        return Ok(());
    }
    rows.insert(key, OrderedRow::new(row, max)?);
    Ok(())
}
fn slot(
    kind: &str,
    id: &str,
    reference: &str,
    line: u64,
    offset: usize,
    row: &[u8],
    whole: &[u8],
    delimiter: &str,
    l: BibliographicLimits,
) -> Result<Value> {
    let value = bytes::parse(row, l.catalog.max_row_bytes)?;
    let key = String::from_utf8(source_canonical(&json!([kind, id]))?)
        .map_err(|_| Error::Invalid("Claim slot key UTF8"))?;
    Ok(
        json!({"source_slot_key":key,"kind":kind,"identity":id,"source":{"source_ref":reference,"source_line":line,"byte_offset":offset,"row_bytes":row.len(),"raw_row_sha256":bytes::digest(row),"delimiter":delimiter,"file_sha256":bytes::digest(whole),"file_bytes":whole.len(),"canonical_sha256":bytes::digest(&source_canonical(&value)?)}}),
    )
}
struct Identity {
    entry: Value,
    source: Value,
    node: Value,
}
fn metadata(
    observation: &mut CommittedClaimObservation,
    roots: &mut roots::Roots,
    role: &str,
    id: &str,
    l: BibliographicLimits,
    worker: &mut CutWorkerSchemaExecutor,
    cancelled: &AtomicBool,
) -> Result<Option<Identity>> {
    let Some((row, _material)) = roots.get_with_material(role, "records", id)? else {
        return Ok(None);
    };
    check(
        worker,
        id,
        &source_canonical(&row)?,
        &format!("{CATALOG_SCHEMA}#/$defs/row"),
        l,
        cancelled,
    )?;
    let reference = bytes::text(&row["source"], "source_ref")?.to_owned();
    let raw = read(observation, &reference, l, cancelled)?;
    let source = bytes::parse(&raw, l.catalog.max_row_bytes)?;
    let subject = owner(crate::source_forms::metadata_subject(&owner(cmd::parse(
        &raw,
    ))?))?;
    let exact: Value = serde_json::from_slice(&owner(cmd::canonical(&subject))?)
        .map_err(|_| Error::Invalid("Claim metadata exact ref codec"))?;
    if row["entry"]["record_id"] != id
        || exact["id"] != id
        || row["source"]
            != json!({"source_ref":reference,"raw_sha256":bytes::digest(&raw),"raw_bytes":raw.len(),"record_ref":exact})
    {
        return Err(Error::Invalid(
            "Claim endpoint exact current binding differs",
        ));
    }
    // This initial addition adapter selects a current declared-profile record.
    // Retained metadata history requires the owner archive reader, not a claim
    // that a current row proves its earlier versions.
    let (home, name) = reference
        .rsplit_once('/')
        .ok_or(Error::Invalid("Claim metadata parent"))?;
    let history = format!("{home}/source-revision-history.json");
    if owner(observation.read_optional(
        &path(&history)?,
        l.catalog.max_file_bytes,
        l.deadline,
        cancelled,
    ))?
    .is_some()
    {
        return Err(Error::Invalid(
            "Claim initial addition metadata history transport unsupported",
        ));
    }
    if !["public", "public_metadata_only"].contains(&source["visibility"].as_str().unwrap_or("")) {
        return Err(Error::Invalid("Claim endpoint public metadata profile"));
    }
    if source["record_version"].as_u64() != Some(1) {
        return Err(Error::Invalid(
            "Claim initial addition requires verified initial endpoint version",
        ));
    }
    let schema = bytes::text(&row["entry"], "source_schema_ref")?.to_owned();
    check_resource(observation, worker, &schema, l, cancelled)?;
    check(worker, &reference, &raw, &schema, l, cancelled)?;
    let entry = tos_compiler::source_witness_catalog::render_catalog_record(
        &source,
        &reference,
        Some(&schema),
        l.catalog.max_output_row_bytes,
    )?;
    if entry != row["entry"] {
        return Err(Error::Invalid(
            "Claim exact endpoint catalog rendering differs",
        ));
    }
    let stem = name
        .strip_suffix(".json")
        .ok_or(Error::Invalid("Claim endpoint source basename"))?;
    let forms_ref = format!("{home}/{stem}.human-forms.json");
    let selected_forms = owner(observation.read_optional(
        &path(&forms_ref)?,
        l.catalog.max_file_bytes,
        l.deadline,
        cancelled,
    ))?;
    let forms = if let Some(raw) = selected_forms {
        for contract in [
            "ToS/contracts/knowledge-assessment.schema.json",
            "ToS/contracts/human-form.schema.json",
            "ToS/contracts/human-form-set.schema.json",
            "ToS/contracts/human-form-template.schema.json",
        ] {
            check_resource(observation, worker, contract, l, cancelled)?;
        }
        check(
            worker,
            &forms_ref,
            &raw,
            "ToS/contracts/human-form-set.schema.json",
            l,
            cancelled,
        )?;
        let set = bytes::parse(&raw, l.catalog.max_file_bytes)?;
        Some(Value::Array(
            crate::source_forms_compiler::materialize_compiler_forms(&source, &set, 262144)?,
        ))
    } else {
        None
    };
    let node = bib::supplied_bibliographic_identity(
        &entry,
        &source,
        forms.as_ref().map(|v| (forms_ref.as_str(), v)),
        l.catalog.max_output_row_bytes,
    )?;
    Ok(Some(Identity {
        entry,
        source,
        node,
    }))
}
/// Existing dependency grammar's initial identity-only branch. All strings
/// below name fields the full owner enumerator actually consumes.
fn dependencies(
    entry: &Value,
    claim: &Value,
    subject: &Value,
    object: &Value,
    event: &Value,
    maker: &Value,
    maker_identity: Option<&Value>,
    evidence: &[Value],
    counter: &[Value],
) -> Result<Value> {
    let mut rows: BTreeMap<(String, String), (BTreeSet<String>, BTreeSet<String>)> =
        BTreeMap::new();
    fn add(
        rows: &mut BTreeMap<(String, String), (BTreeSet<String>, BTreeSet<String>)>,
        kind: &str,
        reference: &Value,
        field: &str,
        reason: &str,
    ) -> Result<()> {
        let reference = reference
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or(Error::Invalid(
                "Claim dependency missing consumed reference",
            ))?;
        let value = rows
            .entry((kind.to_owned(), reference.to_owned()))
            .or_default();
        value.0.insert(field.to_owned());
        value.1.insert(reason.to_owned());
        Ok(())
    }
    fn node_sources(
        rows: &mut BTreeMap<(String, String), (BTreeSet<String>, BTreeSet<String>)>,
        node: &Value,
        field: &str,
    ) -> Result<()> {
        add(
            rows,
            "path",
            &node["source_ref"],
            &format!("{field}/source_ref"),
            "resolved-node-source",
        )?;
        if node.pointer("/properties/human_forms").is_some()
            || node.pointer("/properties/human_forms_source_ref").is_some()
        {
            add(
                rows,
                "path",
                &node["properties"]["human_forms_source_ref"],
                &format!("{field}/properties/human_forms_source_ref"),
                "resolved-identity-human-forms",
            )?;
        }
        Ok(())
    }
    fn identity(
        rows: &mut BTreeMap<(String, String), (BTreeSet<String>, BTreeSet<String>)>,
        node: &Value,
        field: &str,
    ) -> Result<()> {
        add(
            rows,
            "identity",
            &node["properties"]["identity_ref"],
            &format!("{field}/properties/identity_ref"),
            "resolved-identity",
        )?;
        node_sources(rows, node, field)
    }
    add(
        &mut rows,
        "claim",
        &claim["claim_id"],
        "/source_claim/claim_id",
        "projected-claim",
    )?;
    add(
        &mut rows,
        "path",
        &entry["source_claim_file_ref"],
        "/entry/source_claim_file_ref",
        "source-claim-slot",
    )?;
    add(
        &mut rows,
        "identity",
        &claim["subject_ref"],
        "/source_claim/subject_ref",
        "declared-subject",
    )?;
    identity(&mut rows, subject, "/subject_node")?;
    add(
        &mut rows,
        "identity",
        &claim["object"],
        "/source_claim/object",
        "declared-identity-object",
    )?;
    identity(&mut rows, object, "/object_node")?;
    add(
        &mut rows,
        "provenance_event",
        &claim["provenance_event_ref"],
        "/source_claim/provenance_event_ref",
        "declared-provenance-event",
    )?;
    add(
        &mut rows,
        "provenance_event",
        &event["properties"]["event_ref"],
        "/event_node/properties/event_ref",
        "resolved-provenance-event",
    )?;
    node_sources(&mut rows, event, "/event_node")?;
    let resolved = maker_identity.is_some()
        || maker
            .pointer("/properties/identity_node_id")
            .is_some_and(|v| !v.is_null());
    let kind = if resolved { "identity" } else { "unresolved" };
    add(
        &mut rows,
        kind,
        &claim["maker"]["agent_ref"],
        "/source_claim/maker/agent_ref",
        if resolved {
            "maker-identity"
        } else {
            "maker-without-resolved-metadata-identity"
        },
    )?;
    add(
        &mut rows,
        kind,
        &maker["properties"]["agent_ref"],
        "/maker_node/properties/agent_ref",
        "resolved-maker-reference",
    )?;
    if claim["maker"]["agent_ref"] != maker["properties"]["agent_ref"] {
        return Err(Error::Invalid("Claim maker dependency association differs"));
    }
    node_sources(&mut rows, maker, "/maker_node")?;
    if let Some(node) = maker_identity {
        identity(&mut rows, node, "/maker_identity_node")?;
    }
    for (source_field, nodes_field, nodes) in [
        ("evidence_refs", "evidence_nodes", evidence),
        ("counterevidence_refs", "counterevidence_nodes", counter),
    ] {
        let refs = claim
            .get(source_field)
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if refs.len() != nodes.len() {
            return Err(Error::Invalid("Claim evidence dependency closure"));
        }
        for (index, (reference, node)) in refs.iter().zip(nodes).enumerate() {
            if node["properties"]["evidence_kind"] != "repo_path"
                || node["properties"]["evidence_ref"] != *reference
            {
                return Err(Error::Invalid("Claim initial path evidence dependency"));
            }
            add(
                &mut rows,
                "path",
                reference,
                &format!("/source_claim/{source_field}/{index}"),
                "evidence-kind:repo_path",
            )?;
            let field = format!("/{nodes_field}/{index}");
            add(
                &mut rows,
                "path",
                &node["properties"]["evidence_ref"],
                &format!("{field}/properties/evidence_ref"),
                "evidence-kind:repo_path",
            )?;
            node_sources(&mut rows, node, &field)?;
        }
    }
    Ok(Value::Array(rows.into_iter().map(|((kind,reference),(fields,reasons))|json!({"kind":kind,"ref":reference,"field_paths":fields.into_iter().collect::<Vec<_>>(),"reasons":reasons.into_iter().collect::<Vec<_>>() })).collect()))
}
/// Real source execution occurs before the caller opens its SQLite transaction.
/// `actual_revision` comes from its verified predecessor PreparedSourceInputs.
#[allow(clippy::too_many_arguments)]
pub(super) fn assemble(
    observation: &mut CommittedClaimObservation,
    roots: &mut roots::Roots,
    catalog_role: &str,
    before_header: &Value,
    actual_revision: SourceRevision,
    worker: &mut CutWorkerSchemaExecutor,
    l: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<(CatalogAddition, AssembledAddition)> {
    owner(observation.verify_current(l.deadline, cancelled))?;
    if before_header["claims_addressed"] != true
        || before_header["source_publication"] != observation.source_publication()
    {
        return Err(Error::Invalid(
            "Claim addition addressed predecessor publication differs",
        ));
    }
    let source_profiles = before_header
        .pointer("/profile_bindings/source")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("Claim predecessor source profiles"))?;
    for (reference, binding) in source_profiles {
        let raw = read(observation, reference, l, cancelled)?;
        if *binding != json!({"sha256":bytes::digest(&raw),"bytes":raw.len()}) {
            return Err(Error::Invalid("Claim predecessor source profile changed"));
        }
    }
    for reference in [
        CATALOG_SCHEMA,
        "ToS/contracts/corpus-record.schema.json",
        "ToS/contracts/source-claim-record.schema.json",
    ] {
        check_resource(observation, worker, reference, l, cancelled)?;
    }
    let registry = bytes::parse(
        &read(observation, REGISTRY, l, cancelled)?,
        l.catalog.max_contract_bytes,
    )?;
    let entities = bytes::parse(
        &read(observation, ENTITIES, l, cancelled)?,
        l.catalog.max_contract_bytes,
    )?;
    for (reference, contract, value) in [
        (
            REGISTRY,
            "ToS/contracts/semantic-relation-type-registry.schema.json",
            &registry,
        ),
        (
            ENTITIES,
            "ToS/contracts/semantic-entity-type-registry.schema.json",
            &entities,
        ),
    ] {
        check_resource(observation, worker, contract, l, cancelled)?;
        check(
            worker,
            reference,
            &source_canonical(value)?,
            contract,
            l,
            cancelled,
        )?;
    }
    let request = observation.request().clone();
    let claims = request["claims"]
        .as_array()
        .ok_or(Error::Invalid("Claim addition selected claim list"))?;
    let mut identity_ids = BTreeSet::new();
    let mut evidence_refs = BTreeSet::new();
    for claim in claims {
        for key in ["subject_ref", "object"] {
            identity_ids.insert(bytes::text(claim, key)?.to_owned());
        }
        for key in ["evidence_refs", "counterevidence_refs"] {
            if let Some(values) = claim.get(key) {
                for value in values
                    .as_array()
                    .ok_or(Error::Invalid("Claim addition evidence list"))?
                {
                    evidence_refs.insert(
                        value
                            .as_str()
                            .ok_or(Error::Invalid("Claim addition evidence ref"))?
                            .to_owned(),
                    );
                }
            }
        }
    }
    let bindings = &request["expected_inputs"];
    if bindings["objects"]
        .as_object()
        .map(|o| o.keys().cloned().collect::<BTreeSet<_>>())
        != Some(identity_ids.clone())
        || bindings["evidence"]
            .as_object()
            .map(|o| o.keys().cloned().collect::<BTreeSet<_>>())
            != Some(evidence_refs.clone())
    {
        return Err(Error::Invalid(
            "Claim addition exact selected source input sets differ",
        ));
    }
    let mut identities = BTreeMap::new();
    for id in &identity_ids {
        let identity = metadata(observation, roots, catalog_role, id, l, worker, cancelled)?
            .ok_or(Error::Invalid("Claim initial endpoint not in predecessor"))?;
        let binding = &bindings["objects"][id];
        let reference = bytes::text(&identity.entry, "source_record_ref")?;
        let raw = read(observation, reference, l, cancelled)?;
        if *binding
            != json!({"source_ref":reference,"source_sha256":format!("sha256:{}",bytes::digest(&raw)),"canonical_record_sha256":format!("sha256:{}",bytes::digest(&source_canonical(&identity.source)?)),"schema_version":identity.source["schema_version"],"record_version":identity.source["record_version"]})
        {
            return Err(Error::Invalid(
                "Claim addition endpoint creation binding changed",
            ));
        }
        identities.insert(id.clone(), identity);
    }
    let mut evidence = BTreeMap::new();
    for reference in &evidence_refs {
        let binding = &bindings["evidence"][reference];
        if binding["evidence_kind"] != "repo_path"
            || binding["source_ref"] != *reference
            || !binding["source_line"].is_null()
        {
            return Err(Error::Invalid(
                "Claim addition requires public path evidence",
            ));
        }
        let raw = read(observation, reference, l, cancelled)?;
        if binding["source_sha256"] != format!("sha256:{}", bytes::digest(&raw)) {
            return Err(Error::Invalid("Claim addition evidence binding differs"));
        }
        evidence.insert(
            reference.clone(),
            bib::supplied_bibliographic_path_evidence(
                reference,
                &raw,
                l.catalog.max_output_row_bytes,
            )?,
        );
    }
    for claim in claims {
        let maker = bytes::text(&claim["maker"], "agent_ref")?;
        if !identities.contains_key(maker) {
            if let Some(identity) = metadata(
                observation,
                roots,
                catalog_role,
                maker,
                l,
                worker,
                cancelled,
            )? {
                identities.insert(maker.to_owned(), identity);
            }
        }
    }
    let reference = observation.source_path().as_str().to_owned();
    let home = reference
        .rsplit_once('/')
        .ok_or(Error::Invalid("Claim addition package parent"))?
        .0;
    let event_ref = format!("{home}/source-create-provenance.jsonl");
    let event_raw = read(observation, &event_ref, l, cancelled)?;
    let event_lines = event_raw
        .split_inclusive(|b| *b == b'\n')
        .collect::<Vec<_>>();
    if event_lines.len() != 1 || !event_raw.ends_with(b"\n") {
        return Err(Error::Invalid(
            "Claim initial provenance exact single LF row",
        ));
    }
    let event_payload = &event_raw[..event_raw.len() - 1];
    let event = bytes::parse(event_payload, l.catalog.max_row_bytes)?;
    let event_id = bytes::text(observation.configuration(), "provenance_event_id")?.to_owned();
    if event["event_id"] != event_id {
        return Err(Error::Invalid("Claim provenance slot closure differs"));
    }
    let event_schema = match bytes::text(&event, "schema_version")? {
        "tos_provenance_event_v1" => "ToS/contracts/provenance-event.schema.json",
        "tos_provenance_event_v2" => "ToS/contracts/provenance-event-v2.schema.json",
        _ => return Err(Error::Invalid("Claim initial provenance schema")),
    };
    check_resource(observation, worker, event_schema, l, cancelled)?;
    check(
        worker,
        &event_ref,
        event_payload,
        event_schema,
        l,
        cancelled,
    )?;
    if event["schema_version"] == "tos_provenance_event_v2" {
        if ![
            "tracked_public_metadata",
            "public_content",
            "public_synthetic",
        ]
        .contains(
            &event
                .pointer("/rights_and_visibility/content_visibility")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ) {
            return Err(Error::Invalid("Claim addition provenance not public"));
        }
        if !tos_validation::provenance_rules::semantic_issues(&event, 4096, l.deadline)
            .map_err(|_| Error::Budget("Claim provenance semantic check"))?
            .is_empty()
        {
            return Err(Error::Invalid("Claim addition provenance consistency"));
        }
    }
    let event_slot = slot(
        "provenance_event",
        &event_id,
        &event_ref,
        1,
        0,
        event_payload,
        &event_raw,
        "lf",
        l,
    )?;
    let event_node = bib::supplied_bibliographic_event(
        &event,
        &event_slot["source"],
        l.catalog.max_output_row_bytes,
    )?;
    let mut changes = Vec::new();
    let mut output = AssembledAddition {
        nodes: BTreeMap::new(),
        edges: BTreeMap::new(),
        traces: Vec::new(),
        declarations: Vec::new(),
    };
    let mut claim_ids = Vec::new();
    if roots
        .get(
            catalog_role,
            "source_slots",
            bytes::text(&event_slot, "source_slot_key")?,
        )?
        .is_some()
    {
        return Err(Error::Invalid(
            "Claim added event slot already belongs to predecessor",
        ));
    }
    check(
        worker,
        &event_ref,
        &source_canonical(&event_slot)?,
        &format!("{CATALOG_SCHEMA}#/$defs/slotRow"),
        l,
        cancelled,
    )?;
    changes.push(roots::Change {
        collection: "source_slots".to_owned(),
        key: bytes::text(&event_slot, "source_slot_key")?.to_owned(),
        before_sha256: None,
        after: event_slot,
    });
    // Capture selected schema routes before creating the proposal context.
    for claim in claims {
        let predicate = bytes::text(claim, "predicate")?;
        let matches = registry["relations"]
            .as_array()
            .ok_or(Error::Invalid("Claim registry relations"))?
            .iter()
            .filter(|r| {
                r["source_mappings"].as_array().is_some_and(|m| {
                    m.iter().any(|m| {
                        m["source_graph"] == "source-claims"
                            && m["source_predicate_id"] == predicate
                    })
                })
            })
            .collect::<Vec<_>>();
        if matches.len() != 1
            || matches[0]["source_claim_profile"]["reader"] != "identity-relation-v1"
        {
            return Err(Error::Invalid(
                "Claim initial identity-only profile unresolved",
            ));
        }
        let relation = matches[0];
        if relation["abstract"] != false
            || relation["assertion_mode"] != "reified-claim"
            || relation["evidence_required"] != true
            || relation["source_mappings"].as_array().is_none_or(|m| {
                m.iter()
                    .filter(|m| {
                        m["source_graph"] == "source-claims" && m["scope"] == "claim-predicate"
                    })
                    .count()
                    != 1
            })
        {
            return Err(Error::Invalid("Claim initial reified profile contract"));
        }
        if claim
            .pointer("/qualifiers/display_fields/schema_version")
            .and_then(Value::as_str)
            == Some("tos_claim_display_fields_v1")
        {
            check_resource(
                observation,
                worker,
                "ToS/contracts/claim-display-fields.schema.json",
                l,
                cancelled,
            )?;
        }
        let profile = &matches[0]["source_claim_profile"];
        let routes = profile["schemas"]
            .as_array()
            .ok_or(Error::Invalid("Claim profile schemas"))?
            .iter()
            .filter(|r| r["schema_version"] == claim["schema_version"])
            .collect::<Vec<_>>();
        if routes.len() != 1 {
            return Err(Error::Invalid("Claim initial exact schema route"));
        }
        check_resource(
            observation,
            worker,
            bytes::text(routes[0], "schema_ref")?,
            l,
            cancelled,
        )?;
        for dependency in routes[0]
            .get("schema_dependencies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            check_resource(
                observation,
                worker,
                dependency
                    .as_str()
                    .ok_or(Error::Invalid("Claim schema dependency"))?,
                l,
                cancelled,
            )?;
        }
    }
    let context = owner(observation.command_context(actual_revision))?;
    let stream = read(observation, &reference, l, cancelled)?;
    let mut offset = 0usize;
    for (index, claim) in claims.iter().enumerate() {
        let id = bytes::text(claim, "claim_id")?.to_owned();
        if roots.get(catalog_role, "records", &id)?.is_some()
            || roots.get(catalog_role, "claims", &id)?.is_some()
        {
            return Err(Error::Invalid(
                "Claim added identity already belongs to predecessor",
            ));
        }
        let source = owner(cmd::parse(&source_canonical(claim)?))?;
        let schema = owner(
            crate::source_claims::validate_initial_identity_publication_claim(
                &context, &source, worker, l.deadline, cancelled,
            ),
        )?;
        let row_raw = source_canonical(claim)?;
        if stream.get(offset..offset + row_raw.len()) != Some(row_raw.as_slice())
            || stream.get(offset + row_raw.len()) != Some(&b'\n')
        {
            return Err(Error::Invalid("Claim initial physical row differs"));
        }
        let slot = slot(
            "claim",
            &id,
            &reference,
            (index + 1) as u64,
            offset,
            &row_raw,
            &stream,
            "lf",
            l,
        )?;
        offset += row_raw.len() + 1;
        if roots
            .get(
                catalog_role,
                "source_slots",
                bytes::text(&slot, "source_slot_key")?,
            )?
            .is_some()
        {
            return Err(Error::Invalid("Claim added typed source slot exists"));
        }
        let entry = tos_compiler::source_witness_catalog::render_catalog_claim(
            claim,
            &reference,
            (index + 1) as u64,
            Some(&schema),
            l.catalog.max_output_row_bytes,
        )?;
        let addressed = json!({"claim_id":id,"entry":entry,"source_slot_key":slot["source_slot_key"],"claim_ref":{"id":id,"version":1,"digest":format!("sha256:{}",bytes::digest(&row_raw))}});
        check(
            worker,
            &reference,
            &source_canonical(&slot)?,
            &format!("{CATALOG_SCHEMA}#/$defs/slotRow"),
            l,
            cancelled,
        )?;
        check(
            worker,
            &reference,
            &source_canonical(&addressed)?,
            &format!("{CATALOG_SCHEMA}#/$defs/claimRow"),
            l,
            cancelled,
        )?;
        let subject = identities
            .get(bytes::text(claim, "subject_ref")?)
            .ok_or(Error::Invalid("Claim subject verified input missing"))?
            .node
            .clone();
        let object = identities
            .get(bytes::text(claim, "object")?)
            .ok_or(Error::Invalid("Claim object verified input missing"))?
            .node
            .clone();
        let maker_identity = identities
            .get(bytes::text(&claim["maker"], "agent_ref")?)
            .map(|v| v.node.clone());
        let baseline = before_header
            .pointer("/legacy_baseline/files/ToS~1source-witnesses~1catalog~1claims.jsonl/sha256")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid("Claim predecessor maker baseline"))?;
        let (maker, maker_identity) = bib::supplied_bibliographic_maker(
            claim,
            maker_identity,
            baseline,
            l.catalog.max_output_row_bytes,
        )?;
        let get_evidence = |key: &str| -> Result<Vec<Value>> {
            claim
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|v| {
                    evidence
                        .get(
                            v.as_str()
                                .ok_or(Error::Invalid("Claim evidence exact ref"))?,
                        )
                        .cloned()
                        .ok_or(Error::Invalid("Claim verified evidence missing"))
                })
                .collect()
        };
        let evidence_nodes = get_evidence("evidence_refs")?;
        let counter = get_evidence("counterevidence_refs")?;
        let deps = dependencies(
            &entry,
            claim,
            &subject,
            &object,
            &event_node,
            &maker,
            maker_identity.as_ref(),
            &evidence_nodes,
            &counter,
        )?;
        let declaration = json!({"schema":"tos_source_claim_dependencies_v1","claim_id":id,"source_entry":entry,"source_entry_sha256":bytes::digest(&bytes::canonical(&entry,l.catalog.max_output_row_bytes)?),"input_sha256":entry["claim_sha256"],"dependencies":deps});
        output
            .declarations
            .push(OrderedRow::new(declaration, l.max_claim_cohort_bytes)?);
        let descriptor = bib::supplied_claim_navigation_descriptor(
            claim,
            &subject,
            &object,
            &registry,
            &entities,
            l.catalog.max_output_row_bytes,
        )?;
        let cohort = bib::render_supplied_claim(
            bib::SuppliedBibliographicClaimInputs {
                entry: &entry,
                claim,
                subject,
                object,
                event: event_node.clone(),
                maker,
                maker_identity,
                evidence: evidence_nodes,
                counterevidence: counter,
                members: Vec::new(),
                normalized: Vec::new(),
                descriptor,
                forms: None,
                collection_order_basis: None,
                legacy_context: None,
            },
            l,
        )?;
        for row in cohort.nodes {
            put(
                &mut output.nodes,
                "node_id",
                row,
                l.catalog.max_output_row_bytes,
            )?;
        }
        for row in cohort.edges {
            put(
                &mut output.edges,
                "edge_id",
                row,
                l.catalog.max_output_row_bytes,
            )?;
        }
        output.traces.push(OrderedRow::new(
            cohort.trace,
            l.catalog.max_output_row_bytes,
        )?);
        changes.push(roots::Change {
            collection: "source_slots".to_owned(),
            key: bytes::text(&slot, "source_slot_key")?.to_owned(),
            before_sha256: None,
            after: slot,
        });
        changes.push(roots::Change {
            collection: "claims".to_owned(),
            key: id.clone(),
            before_sha256: None,
            after: addressed,
        });
        claim_ids.push(id);
    }
    if offset != stream.len() {
        return Err(Error::Invalid("Claim stream addition closure differs"));
    }
    let mut header = before_header.clone();
    header["claim_count"] = json!(
        bytes::number(before_header, "claim_count")?
            .checked_add(claims.len() as u64)
            .ok_or(Error::Budget("Claim header count"))?
    );
    header["source_slot_count"] = json!(
        bytes::number(before_header, "source_slot_count")?
            .checked_add((claims.len() + 1) as u64)
            .ok_or(Error::Budget("Claim header slot count"))?
    );
    header["last_transition"] = Value::Null;
    // Native processor/profile transition is reviewed and applied separately
    // by the whole caller. Historical predecessor bindings remain untouched.
    check(
        worker,
        "source-catalog-addition-header",
        &source_canonical(&header)?,
        &format!("{CATALOG_SCHEMA}#/$defs/header"),
        l,
        cancelled,
    )?;
    let total_rows = output
        .nodes
        .len()
        .checked_add(output.edges.len())
        .and_then(|n| n.checked_add(output.traces.len()))
        .ok_or(Error::Budget("Claim assembled row count"))?;
    if total_rows as u64 > l.max_output_rows {
        return Err(Error::Budget("Claim assembled whole row count"));
    }
    let mut total = 0usize;
    for row in output
        .nodes
        .values()
        .chain(output.edges.values())
        .chain(&output.traces)
        .chain(&output.declarations)
    {
        total = total
            .checked_add(row.raw.len())
            .filter(|n| *n <= l.max_output_bytes as usize)
            .ok_or(Error::Budget("Claim assembled whole output"))?;
    }
    owner(observation.verify_current(l.deadline, cancelled))?;
    Ok((
        CatalogAddition {
            header,
            changes,
            claim_ids,
        },
        output,
    ))
}
