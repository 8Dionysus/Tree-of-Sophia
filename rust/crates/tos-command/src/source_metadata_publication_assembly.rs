//! Exact addressed catalog addition and initial metadata carrier cohort.
use crate::{
    source_claim_publication_bytes as bytes,
    source_claim_publication_roots::{Change, Roots},
    source_command as cmd,
    source_creation_store::CommittedMetadataCreationObservation,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};
use tos_compiler::{
    Error, Result,
    source_bibliographic::{self as bib, BibliographicLimits},
};
use tos_foundation::{
    JsonLimits, JsonMode, JsonValue, RelativePath, emit_value_preserved_json, parse_json,
};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
const CATALOG: &str = "ToS/contracts/source-catalog-projection-v2.schema.json";
pub(super) struct Assembled {
    pub id: String,
    pub request_digest: String,
    pub header: Value,
    pub catalog_changes: Vec<Change>,
    pub nodes: BTreeMap<(String, String), Vec<u8>>,
    pub edges: BTreeMap<(String, String), Vec<u8>>,
}
fn owner<T>(r: cmd::SourceCommandResult<T>) -> Result<T> {
    r.map_err(|e| Error::Source(format!("Metadata source: {e:?}")))
}
fn view(v: &JsonValue, cap: usize) -> Result<Value> {
    let limits = JsonLimits::new(cap, 128, 1_000_000, 4300)
        .map_err(|_| Error::Budget("Metadata JSON limits"))?;
    bytes::parse(
        &emit_value_preserved_json(v, limits).map_err(|_| Error::Budget("Metadata JSON"))?,
        cap,
    )
}
fn typed(v: &Value, cap: usize) -> Result<JsonValue> {
    Ok(parse_json(
        &bytes::canonical(v, cap)?,
        JsonMode::PublishedStrict,
        JsonLimits::new(cap, 128, 1_000_000, 4300)
            .map_err(|_| Error::Budget("Metadata typed limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?
    .root()
    .clone())
}
fn read(
    o: &CommittedMetadataCreationObservation<'_>,
    reference: &str,
    l: BibliographicLimits,
    c: &AtomicBool,
) -> Result<Vec<u8>> {
    owner(o.read_selected(
        &RelativePath::parse(reference).map_err(|_| Error::Invalid("Metadata reference"))?,
        l.deadline,
        c,
    ))
}
fn check(
    w: &mut CutWorkerSchemaExecutor,
    reference: &str,
    raw: &[u8],
    contract: &str,
    l: BibliographicLimits,
    c: &AtomicBool,
) -> Result<()> {
    match w.check(reference, raw, contract, l.deadline, c) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::Invalid("Metadata addressed schema refused")),
        Err(e) => Err(Error::Source(format!("Metadata schema worker: {e:?}"))),
    }
}
fn source_canonical(value: &Value) -> Result<Vec<u8>> {
    owner(cmd::canonical(&owner(cmd::parse(
        &serde_json::to_vec(value).map_err(|_| Error::Invalid("Metadata source JSON"))?,
    ))?))
}
fn put(
    rows: &mut BTreeMap<(String, String), Vec<u8>>,
    graph: &str,
    values: Vec<Value>,
    field: &str,
    cap: usize,
    budget: &mut (usize, usize),
    limits: BibliographicLimits,
) -> Result<()> {
    for value in values {
        let raw = bytes::canonical(&value, cap)?;
        budget.0 = budget
            .0
            .checked_add(1)
            .filter(|n| *n as u64 <= limits.max_output_rows)
            .ok_or(Error::Budget("Metadata complete cohort rows"))?;
        budget.1 = budget
            .1
            .checked_add(raw.len())
            .filter(|n| *n as u64 <= limits.max_output_bytes)
            .ok_or(Error::Budget("Metadata complete cohort bytes"))?;
        let native = bytes::text(&value, field)?.to_owned();
        if rows.insert((graph.to_owned(), native), raw).is_some() {
            return Err(Error::Invalid("Metadata duplicate carrier"));
        }
    }
    Ok(())
}
pub(super) fn assemble(
    o: &CommittedMetadataCreationObservation<'_>,
    roots: &mut Roots,
    w: &mut CutWorkerSchemaExecutor,
    l: BibliographicLimits,
    c: &AtomicBool,
) -> Result<Assembled> {
    if w.source_revision() != o.cut().current().revision()
        || o.context().base_revision != o.cut().current().revision()
    {
        return Err(Error::Invalid("Metadata actual current cut worker"));
    }
    let mut header = roots
        .snapshots
        .get("source-catalog")
        .ok_or(Error::Invalid("Metadata catalog absent"))?
        .manifest["header"]
        .clone();
    if header["claims_addressed"] != true
        || header["source_publication"] != view(&o.publication(), 1_048_576)?
    {
        return Err(Error::Invalid("Metadata addressed publication predecessor"));
    }
    check(
        w,
        "Metadata catalog predecessor",
        &bytes::canonical(&header, 1_048_576)?,
        &format!("{CATALOG}#/$defs/header"),
        l,
        c,
    )?;
    for namespace in ["source", "execution"] {
        for (reference, expected) in header["profile_bindings"][namespace]
            .as_object()
            .ok_or(Error::Invalid("Metadata catalog profiles"))?
        {
            let raw = read(o, reference, l, c)?;
            if *expected != json!({"sha256":bytes::digest(&raw),"bytes":raw.len()}) {
                return Err(Error::Invalid("Metadata retained catalog profile differs"));
            }
        }
    }
    let package = o.package().prepared();
    let config = owner(cmd::parse(&package.context().configuration_raw))?;
    let reference = owner(cmd::text(&config, "source_path"))?.to_owned();
    let raw = read(o, &reference, l, c)?;
    let record = bytes::parse(&raw, l.catalog.max_row_bytes)?;
    let source = owner(cmd::parse(&raw))?;
    let subject = view(
        &owner(crate::source_forms::metadata_subject(&source))?,
        l.catalog.max_row_bytes,
    )?;
    let id = bytes::text(&subject, "id")?.to_owned();
    if record["source_refs"].as_array().is_some_and(|refs| {
        refs.iter()
            .any(|r| r.as_str().is_some_and(|s| s.starts_with("ToS/canon/")))
    }) {
        return Err(Error::Invalid("Metadata external canon growth closure"));
    }
    for collection in ["records", "claims"] {
        if roots.get("source-catalog", collection, &id)?.is_some() {
            return Err(Error::Invalid("Metadata identity exists in catalog"));
        }
    }
    for kind in ["claim", "anchor", "provenance_event"] {
        let key = String::from_utf8(source_canonical(&json!([kind, id]))?)
            .map_err(|_| Error::Invalid("Metadata slot key"))?;
        if roots.get("source-catalog", "source_slots", &key)?.is_some() {
            return Err(Error::Invalid("Metadata identity slot exists"));
        }
    }
    let preview = view(&owner(package.preview())?, 1_048_576)?;
    let schema = bytes::text(&preview["source_profile"], "schema_ref")?;
    check(w, &reference, &raw, schema, l, c)?;
    if package.family() == crate::source_creation::CreationFamily::PublicProfile {
        let (_, resources, _, _) = owner(crate::source_revisions::public_profile(
            Some(o.cut()),
            w,
            l.deadline,
            c,
            o.context(),
            &config,
            &source,
        ))?;
        for reference in resources {
            let raw = read(o, &reference, l, c)?;
            header["profile_bindings"]["source"][&reference] =
                json!({"sha256":bytes::digest(&raw),"bytes":raw.len()});
        }
    }
    let entry = tos_compiler::source_witness_catalog::render_catalog_record(
        &record,
        &reference,
        Some(schema),
        l.catalog.max_output_row_bytes,
    )?;
    let entities = bytes::parse(
        &read(
            o,
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            l,
            c,
        )?,
        l.catalog.max_contract_bytes,
    )?;
    let descriptor =
        bib::supplied_metadata_descriptor(&entry, &record, &entities, l.catalog.into())?;
    let row = json!({"record_id":id,"entry":entry,"source":{"source_ref":reference,"raw_sha256":bytes::digest(&raw),"raw_bytes":raw.len(),"record_ref":subject}});
    let (home, name) = reference
        .rsplit_once('/')
        .ok_or(Error::Invalid("Metadata package parent"))?;
    let stem = name
        .strip_suffix(".json")
        .ok_or(Error::Invalid("Metadata source basename"))?;
    let form_ref = format!("{home}/{stem}.human-forms.json");
    let form_raw = read(o, &form_ref, l, c)?;
    check(
        w,
        &form_ref,
        &form_raw,
        "ToS/contracts/human-form-set.schema.json",
        l,
        c,
    )?;
    let forms = bytes::parse(&form_raw, l.catalog.max_file_bytes)?;
    if forms["subject"] != subject
        || forms
            .get("prior_forms")
            .is_some_and(|v| !v.is_null() && v != &json!([]))
        || forms
            .get("growth_history")
            .is_some_and(|v| !v.is_null() && v != &json!([]))
    {
        return Err(Error::Invalid("Metadata exact initial forms"));
    }
    let material = Value::Array(crate::source_forms_compiler::materialize_compiler_forms(
        &record, &forms, 262_144,
    )?);
    let resolved = owner(
        crate::source_revisions::resolve_record_version_evidence_at_from_cut(
            o.context(),
            o.cut(),
            &reference,
            tos_validation::item_rules::ItemLimits {
                max_member_bytes: l.catalog.max_file_bytes.min(8_388_608),
                max_total_bytes: 33_554_432,
                max_state_bytes: 33_554_432,
                max_issues: 256,
                deadline: l.deadline,
            },
            &typed(&subject, l.catalog.max_row_bytes)?,
            w,
            l.deadline,
            c,
        ),
    )?;
    if resolved.version_status != "current"
        || view(&resolved.current_ref, l.catalog.max_row_bytes)? != subject
        || view(&resolved.record, l.catalog.max_row_bytes)? != record
    {
        return Err(Error::Invalid("Metadata exact initial cold resolution"));
    }
    let address = json!({"schema_version":"tos_source_catalog_address_v2","catalog_namespace":header["catalog_namespace"],"profile_id":"tos.source-catalog.public-records.v2","record_key":id,"row_sha256":bytes::row_digest(&row,l.catalog.max_row_bytes)?,"source_ref":reference,"raw_sha256":bytes::digest(&raw),"raw_bytes":raw.len(),"record_ref":subject});
    let provenance = json!({"verification_scope":"selected-record-chain","all_package_bytes_verified":false,"catalog":address,"descriptor":descriptor,"history":view(&resolved.history,l.catalog.max_row_bytes)?});
    let history = json!({"status":"available","reason":"verified-record-references","record_id":id,"current_ref":subject,"refs":[subject],"provenance":provenance,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false});
    let mut vp = provenance.clone();
    vp["source"] = view(&resolved.source, l.catalog.max_row_bytes)?;
    vp["transition"] = view(&resolved.transition, l.catalog.max_row_bytes)?;
    let versions = vec![(
        subject.clone(),
        json!({"status":"available","reason":"exact-current-version","exact_ref":subject,"version_status":"current","record":record,"record_digest":subject["digest"],"provenance":vp,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false}),
    )];
    let nav = bib::render_supplied_navigation_record(
        bib::SuppliedNavigationRecordInput {
            entry: &entry,
            source_record: &record,
            forms: Some((&form_ref, &material)),
            history: Some(&history),
            versions: &versions,
            native_composite: false,
        },
        l,
    )?;
    if !nav.diagnostics.is_empty() {
        return Err(Error::Invalid("Metadata unresolved navigation"));
    }
    let identity = bib::supplied_bibliographic_identity(
        &entry,
        &record,
        Some((&form_ref, &material)),
        l.catalog.into(),
    )?;
    let mut nodes = BTreeMap::new();
    let mut edges = BTreeMap::new();
    let mut output_budget = (0usize, 0usize);
    put(
        &mut nodes,
        "source-navigation",
        nav.nodes,
        "node_id",
        l.catalog.max_output_row_bytes,
        &mut output_budget,
        l,
    )?;
    put(
        &mut nodes,
        "source-claims",
        vec![identity],
        "node_id",
        l.catalog.max_output_row_bytes,
        &mut output_budget,
        l,
    )?;
    put(
        &mut edges,
        "source-navigation",
        nav.edges,
        "edge_id",
        l.catalog.max_output_row_bytes,
        &mut output_budget,
        l,
    )?;
    let selected: BTreeSet<_> = nodes.keys().map(|(g, id)| format!("{g}:{id}")).collect();
    for ((graph, _), raw) in &edges {
        let v = bytes::parse(raw, l.catalog.max_output_row_bytes)?;
        for end in ["from", "to"] {
            let g = v
                .get(format!("{end}_source_graph"))
                .and_then(Value::as_str)
                .unwrap_or(graph);
            if !selected.contains(&format!("{g}:{}", bytes::text(&v, &format!("{end}_id"))?)) {
                return Err(Error::Invalid("Metadata external raw incidence"));
            }
        }
        if v.get("claim_ref").is_some_and(|v| !v.is_null()) {
            return Err(Error::Invalid("Metadata Claim carrier"));
        }
    }
    let event_ref = format!("{home}/source-create-provenance.jsonl");
    let events = read(o, &event_ref, l, c)?;
    let mut slots = Vec::new();
    let mut offset = 0;
    let mut event_ids = BTreeSet::new();
    for (line, raw) in events.split_inclusive(|b| *b == b'\n').enumerate() {
        let content = raw.strip_suffix(b"\n").unwrap_or(raw);
        if content.is_empty() {
            return Err(Error::Invalid("Metadata empty provenance"));
        }
        let event = bytes::parse(content, l.catalog.max_row_bytes)?;
        let event_id = bytes::text(&event, "event_id")?.to_owned();
        if !event_ids.insert(event_id.clone()) {
            return Err(Error::Invalid("Metadata duplicate provenance"));
        }
        let key = String::from_utf8(source_canonical(&json!(["provenance_event", event_id]))?)
            .map_err(|_| Error::Invalid("Metadata provenance key"))?;
        if roots.get("source-catalog", "source_slots", &key)?.is_some() {
            return Err(Error::Invalid("Metadata provenance slot occupied"));
        }
        let slot = json!({"source_slot_key":key,"kind":"provenance_event","identity":event_id,"source":{"source_ref":event_ref,"source_line":line+1,"byte_offset":offset,"row_bytes":content.len(),"raw_row_sha256":bytes::digest(content),"delimiter":if raw.ends_with(b"\n"){"lf"}else{"eof"},"file_sha256":bytes::digest(&events),"file_bytes":events.len(),"canonical_sha256":bytes::digest(&owner(cmd::canonical(&owner(cmd::parse(content))?))?)}});
        offset += raw.len();
        slots.push(slot);
    }
    if event_ids != BTreeSet::from([owner(cmd::text(&config, "provenance_event_id"))?.to_owned()]) {
        return Err(Error::Invalid("Metadata provenance closure"));
    }
    let mut families: BTreeSet<String> = header["record_families"]
        .as_array()
        .ok_or(Error::Invalid("Metadata record families"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or(Error::Invalid("Metadata family"))
        })
        .collect::<Result<_>>()?;
    families.insert(bytes::text(&record, "record_type")?.to_owned());
    header["record_families"] = json!(families);
    header["record_count"] = json!(
        header["record_count"]
            .as_u64()
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::Budget("Metadata record count"))?
    );
    header["source_slot_count"] = json!(
        header["source_slot_count"]
            .as_u64()
            .and_then(|n| n.checked_add(slots.len() as u64))
            .ok_or(Error::Budget("Metadata slot count"))?
    );
    header["last_transition"] = Value::Null;
    check(
        w,
        "Metadata successor header",
        &bytes::canonical(&header, 1_048_576)?,
        &format!("{CATALOG}#/$defs/header"),
        l,
        c,
    )?;
    check(
        w,
        &id,
        &bytes::canonical(&row, l.catalog.max_row_bytes)?,
        &format!("{CATALOG}#/$defs/row"),
        l,
        c,
    )?;
    let mut changes = vec![Change {
        collection: "records".into(),
        key: id.clone(),
        before_sha256: None,
        after: row,
    }];
    for slot in slots {
        check(
            w,
            "Metadata provenance slot",
            &bytes::canonical(&slot, l.catalog.max_row_bytes)?,
            &format!("{CATALOG}#/$defs/slotRow"),
            l,
            c,
        )?;
        changes.push(Change {
            collection: "source_slots".into(),
            key: bytes::text(&slot, "source_slot_key")?.to_owned(),
            before_sha256: None,
            after: slot,
        });
    }
    let receipt = bytes::parse(
        package
            .files()
            .get("source-create-receipt.json")
            .ok_or(Error::Invalid("Metadata receipt absent"))?,
        1_048_576,
    )?;
    Ok(Assembled {
        id,
        request_digest: bytes::text(&receipt, "request_digest")?.to_owned(),
        header,
        catalog_changes: changes,
        nodes,
        edges,
    })
}
