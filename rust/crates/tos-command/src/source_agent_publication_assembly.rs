//! Source-locked addressed cohort assembly for descriptive Agent correction.
//! Original and current source custody stay with CommittedRecordObservation;
//! rendering does not create a source receipt or grant of current use.
use crate::{
    source_agent_publication_closure::Cohort, source_claim_publication_assembly::OrderedRow,
    source_claim_publication_bytes as bytes, source_claim_publication_dependencies::Declaration,
    source_claim_publication_roots::Change as RootsChange, source_claim_publication_roots::Roots,
    source_command as cmd, source_creation_store::CommittedRecordObservation,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use tos_compiler::{
    Error, Result,
    source_bibliographic::{self as bib, BibliographicLimits, SuppliedBibliographicClaimInputs},
};
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, JsonValue, RelativePath, emit_value_preserved_json, parse_json,
};
use tos_validation::{
    item_rules::ItemLimits,
    source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor},
};
const CATALOG_SCHEMA: &str = "ToS/contracts/source-catalog-projection-v2.schema.json";
const ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";

// Preserve source-owned static refusal reasons without serializing selected
// paths, dynamic IO/schema messages or payloads into the public CLI error.
pub(super) fn source_failure(error: cmd::SourceCommandError) -> Error {
    match error {
        cmd::SourceCommandError::Invalid(reason)
        | cmd::SourceCommandError::Conflict(reason)
        | cmd::SourceCommandError::Denied(reason)
        | cmd::SourceCommandError::Unsupported(reason) => Error::Invalid(reason),
        cmd::SourceCommandError::MissingProductionAdmission => {
            Error::Invalid("Agent source production admission missing")
        }
        cmd::SourceCommandError::SchemaExecution { reason, .. } => schema_failure(reason),
    }
}
pub(super) fn schema_failure(error: tos_validation::item_rules::ItemRefusal) -> Error {
    use tos_validation::item_rules::ItemRefusal;
    match error {
        ItemRefusal::Budget => Error::Budget("Agent selected schema budget"),
        ItemRefusal::BudgetCheck { check, .. } => Error::Budget(check),
        ItemRefusal::Deadline => Error::Budget("Agent selected schema deadline"),
        ItemRefusal::Source(_) => Error::Invalid("Agent selected schema source refusal"),
        ItemRefusal::Unsupported(_) => {
            Error::PreparedUnsupported("Agent selected schema unsupported")
        }
    }
}
fn owner<T>(value: cmd::SourceCommandResult<T>) -> Result<T> {
    value.map_err(source_failure)
}
fn path(reference: &str) -> Result<RelativePath> {
    if !reference.starts_with("ToS/")
        || reference
            .split('/')
            .any(|p| matches!(p, "payload" | "private" | "owner-local" | "local-content"))
    {
        return Err(Error::Invalid("Agent public source path"));
    }
    RelativePath::parse(reference).map_err(|_| Error::Invalid("Agent exact source path"))
}
fn typed(value: &Value, cap: usize) -> Result<JsonValue> {
    Ok(parse_json(
        &bytes::canonical(value, cap)?,
        JsonMode::PublishedStrict,
        JsonLimits::new(cap, 128, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Agent typed JSON"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?
    .into_root())
}
fn view(value: &JsonValue, cap: usize) -> Result<Value> {
    let raw = emit_value_preserved_json(
        value,
        JsonLimits::new(cap, 128, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Agent resolver JSON"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    bytes::parse(&raw, cap)
}
#[derive(Clone, Copy)]
enum Side {
    Original,
    Current,
}
struct Metadata {
    entry: Value,
    record: Value,
    identity: Value,
    forms: Option<(String, Value)>,
    history: Value,
    versions: Vec<(Value, Value)>,
}
struct Reader<'a, 'o> {
    observation: &'a CommittedRecordObservation<'o>,
    side: Side,
    roots: &'a mut Roots,
    worker: &'a mut CutWorkerSchemaExecutor,
    header: &'a Value,
    revised_row: Option<&'a Value>,
    files: BTreeMap<String, Option<Vec<u8>>>,
    file_bytes: usize,
    metadata: BTreeMap<String, Metadata>,
    metadata_bytes: usize,
    limits: BibliographicLimits,
    cancelled: &'a AtomicBool,
}
impl Reader<'_, '_> {
    fn active(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Relaxed)
            || std::time::Instant::now() >= self.limits.deadline
        {
            return Err(Error::Budget("Agent source assembly deadline/cancellation"));
        }
        Ok(())
    }
    fn optional(&mut self, reference: &str) -> Result<Option<Vec<u8>>> {
        self.active()?;
        if let Some(raw) = self.files.get(reference) {
            return Ok(raw.clone());
        }
        if self.files.len() as u64 >= self.limits.catalog.max_files {
            return Err(Error::Budget("Agent addressed source files"));
        }
        let p = path(reference)?;
        let raw = match self.side {
            Side::Original => owner(self.observation.read_original_optional(
                &p,
                self.limits.deadline,
                self.cancelled,
            ))?,
            Side::Current => owner(self.observation.read_current_optional(
                &p,
                self.limits.deadline,
                self.cancelled,
            ))?,
        };
        if let Some(raw) = &raw {
            self.file_bytes = self
                .file_bytes
                .checked_add(raw.len())
                .filter(|n| *n <= 33_554_432)
                .ok_or(Error::Budget("Agent addressed source aggregate"))?;
            if raw.len() > self.limits.catalog.max_file_bytes {
                return Err(Error::Budget("Agent source file bytes"));
            }
        }
        self.files.insert(reference.to_owned(), raw.clone());
        Ok(raw)
    }
    fn read(&mut self, reference: &str) -> Result<Vec<u8>> {
        self.optional(reference)?
            .ok_or(Error::Invalid("Agent required selected source missing"))
    }
    fn check(&mut self, reference: &str, raw: &[u8], contract: &str) -> Result<()> {
        let (resource, _) = contract.split_once('#').unwrap_or((contract, ""));
        let schema = self.read(resource)?;
        if self.worker.contract_digest(resource) != Some(Digest256::of_bytes(&schema)) {
            return Err(Error::Invalid(
                "Agent actual selected schema/executor binding",
            ));
        }
        match self.worker.check(
            reference,
            raw,
            contract,
            self.limits.deadline,
            self.cancelled,
        ) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::Invalid("Agent source schema refused")),
            Err(e) => Err(schema_failure(e)),
        }
    }
    fn record_row(&mut self, id: &str) -> Result<Option<Value>> {
        if id == self.observation.record_id()
            && let Some(row) = self.revised_row
        {
            return Ok(Some(row.clone()));
        }
        self.roots.get("source-catalog", "records", id)
    }
    fn slot(&mut self, kind: &str, id: &str) -> Result<Option<(Value, Value)>> {
        let key = String::from_utf8(owner(cmd::canonical(&typed(&json!([kind, id]), 8192)?))?)
            .map_err(|_| Error::Invalid("Agent slot key UTF8"))?;
        let Some(slot) = self.roots.get("source-catalog", "source_slots", &key)? else {
            return Ok(None);
        };
        self.check(
            id,
            &bytes::canonical(&slot, self.limits.catalog.max_row_bytes)?,
            &format!("{CATALOG_SCHEMA}#/$defs/slotRow"),
        )?;
        if slot["kind"] != kind || slot["identity"] != id || slot["source_slot_key"] != key {
            return Err(Error::Invalid("Agent addressed slot identity"));
        }
        let source = &slot["source"];
        let reference = bytes::text(source, "source_ref")?.to_owned();
        let file = self.read(&reference)?;
        if source["file_bytes"].as_u64() != Some(file.len() as u64)
            || source["file_sha256"] != bytes::digest(&file)
        {
            return Err(Error::Invalid("Agent selected slot file binding"));
        }
        let offset = usize::try_from(bytes::number(source, "byte_offset")?)
            .map_err(|_| Error::Budget("Agent slot offset"))?;
        let length = usize::try_from(bytes::number(source, "row_bytes")?)
            .map_err(|_| Error::Budget("Agent slot length"))?;
        if length > self.limits.catalog.max_row_bytes {
            return Err(Error::Budget("Agent source slot row bytes"));
        }
        let delimiter: &[u8] = match bytes::text(source, "delimiter")? {
            "lf" => b"\n",
            "crlf" => b"\r\n",
            "cr" => b"\r",
            "eof" => b"",
            _ => return Err(Error::Invalid("Agent slot delimiter")),
        };
        let end = offset
            .checked_add(length)
            .ok_or(Error::Budget("Agent slot range"))?;
        let final_end = end
            .checked_add(delimiter.len())
            .ok_or(Error::Budget("Agent slot delimiter range"))?;
        if final_end > file.len()
            || offset > 0 && !matches!(file[offset - 1], b'\n' | b'\r')
            || offset > 0 && file[offset - 1] == b'\r' && file.get(offset) == Some(&b'\n')
            || file[end..final_end] != *delimiter
            || delimiter == b"\r" && file.get(final_end) == Some(&b'\n')
            || delimiter.is_empty() && final_end != file.len()
        {
            return Err(Error::Invalid("Agent slot exact row boundary"));
        }
        let raw = &file[offset..end];
        if source["raw_row_sha256"] != bytes::digest(raw) {
            return Err(Error::Invalid("Agent slot raw digest"));
        }
        let payload = bytes::parse(raw, self.limits.catalog.max_row_bytes)?;
        let idfield = match kind {
            "claim" => "claim_id",
            "anchor" => "anchor_id",
            "provenance_event" => "event_id",
            _ => return Err(Error::Invalid("Agent slot kind")),
        };
        let canonical = owner(cmd::canonical(&typed(
            &payload,
            self.limits.catalog.max_row_bytes,
        )?))?;
        if payload[idfield] != id || source["canonical_sha256"] != bytes::digest(&canonical) {
            return Err(Error::Invalid("Agent slot canonical payload"));
        }
        if kind == "claim"
            && !matches!(
                payload["visibility"].as_str(),
                Some("public" | "public_metadata_only")
            )
        {
            return Err(Error::Invalid("Agent private source Claim"));
        }
        if kind == "provenance_event" && payload["schema_version"] == "tos_provenance_event_v2" {
            if !matches!(
                payload
                    .pointer("/rights_and_visibility/content_visibility")
                    .and_then(Value::as_str),
                Some("tracked_public_metadata" | "public_content" | "public_synthetic")
            ) || !tos_validation::provenance_rules::semantic_issues(
                &payload,
                4096,
                self.limits.deadline,
            )
            .map_err(|_| Error::Budget("Agent provenance semantics"))?
            .is_empty()
            {
                return Err(Error::Invalid("Agent provenance public semantics"));
            }
        }
        Ok(Some((payload, source.clone())))
    }
    fn identity_node(&mut self, id: &str, entities: &Value) -> Result<Value> {
        Ok(self
            .metadata(id, entities)?
            .ok_or(Error::Invalid(
                "Agent unresolved addressed metadata endpoint",
            ))?
            .identity
            .clone())
    }
    fn evidence(
        &mut self,
        reference: &str,
        claim: &Value,
        entry: &Value,
        entities: &Value,
    ) -> Result<Value> {
        let cap = self.limits.catalog.max_output_row_bytes;
        if reference.starts_with("ToS/") {
            return bib::supplied_bibliographic_path_evidence(
                reference,
                &self.read(reference)?,
                cap,
            );
        }
        if let Some((anchor, location)) = self.slot("anchor", reference)? {
            return bib::supplied_bibliographic_evidence(
                reference,
                claim,
                entry,
                bib::SuppliedEvidenceResolution::Anchor {
                    anchor: &anchor,
                    location: &location,
                },
                cap,
            );
        }
        if let Some(metadata) = self.metadata(reference, entities)? {
            return bib::supplied_bibliographic_evidence(
                reference,
                claim,
                entry,
                bib::SuppliedEvidenceResolution::Identity(&metadata.identity),
                cap,
            );
        }
        if let Some((_, location)) = self.slot("provenance_event", reference)? {
            return bib::supplied_bibliographic_evidence(
                reference,
                claim,
                entry,
                bib::SuppliedEvidenceResolution::ProvenanceEvent(&location),
                cap,
            );
        }
        bib::supplied_bibliographic_evidence(
            reference,
            claim,
            entry,
            bib::SuppliedEvidenceResolution::ExternalCitation,
            cap,
        )
    }
    fn claim_inputs(
        &mut self,
        id: &str,
        _entities: &Value,
        _registry: &Value,
    ) -> Result<(Value, Value)> {
        let row = self
            .roots
            .get("source-catalog", "claims", id)?
            .ok_or(Error::Invalid(
                "Agent reverse Claim missing from addressed catalog",
            ))?;
        self.check(
            id,
            &bytes::canonical(&row, self.limits.catalog.max_row_bytes)?,
            &format!("{CATALOG_SCHEMA}#/$defs/claimRow"),
        )?;
        let (claim, location) = self
            .slot("claim", id)?
            .ok_or(Error::Invalid("Agent addressed Claim slot missing"))?;
        let entry = &row["entry"];
        let key = String::from_utf8(owner(cmd::canonical(&typed(&json!(["claim", id]), 8192)?))?)
            .map_err(|_| Error::Invalid("Agent Claim slot key"))?;
        if row["claim_id"] != id
            || entry["claim_id"] != id
            || row["source_slot_key"] != key
            || row["claim_ref"]
                != json!({"id":id,"version":claim["claim_version"],"digest":format!("sha256:{}",bytes::text(&location,"canonical_sha256")?)})
            || entry["source_claim_file_ref"] != location["source_ref"]
            || entry["source_claim_line"] != location["source_line"]
            || tos_compiler::source_witness_catalog::render_catalog_claim(
                &claim,
                bytes::text(&location, "source_ref")?,
                bytes::number(&location, "source_line")?,
                entry["source_schema_ref"].as_str(),
                self.limits.catalog.max_output_row_bytes,
            )? != *entry
        {
            return Err(Error::Invalid(
                "Agent addressed Claim catalog/source slot differs",
            ));
        }
        Ok((row, claim))
    }
    fn project_claim(
        &mut self,
        declaration: &Declaration,
        entities: &Value,
        registry: &Value,
    ) -> Result<bib::SuppliedBibliographicClaimCohort> {
        let (row, claim) = self.claim_inputs(&declaration.id, entities, registry)?;
        let entry = &row["entry"];
        let source = bytes::text(entry, "source_claim_file_ref")?;
        let predicate = bytes::text(&claim, "predicate")?;
        let native = source.rsplit('/').next() == Some("source-claims.jsonl");
        let relations = array(registry, "relations")?
            .iter()
            .filter(|v| {
                v["source_mappings"].as_array().is_some_and(|a| {
                    a.iter().any(|m| {
                        m["source_graph"] == "source-claims"
                            && m["scope"] == "claim-predicate"
                            && m["source_predicate_id"] == predicate
                    })
                })
            })
            .collect::<Vec<_>>();
        if relations.len() != 1 {
            return Err(Error::Invalid(
                "Agent Claim relation registry mapping closure",
            ));
        }
        let relation = relations[0];
        let profile = if native {
            Some(&relation["source_claim_profile"])
        } else {
            None
        };
        let legacy_link =
            source == "ToS/source-witnesses/relations/object-link/object-link-claims.jsonl";
        let historical = matches!(
            predicate,
            "historical_participant" | "historical_place" | "historical_work" | "historical_dating"
        );
        let reader = profile.and_then(|p| p["reader"].as_str());
        if !matches!(
            claim["claim_type"].as_str(),
            Some("bibliographic" | "relation")
        ) || !native
            && !legacy_link
            && claim["claim_type"] == "relation"
            && predicate != "is_derivative_of"
            && !historical
            || !native
                && !legacy_link
                && !matches!(
                    claim["assertion_layer"].as_str(),
                    Some("bibliographic_assertion" | "scholarly_report")
                )
        {
            return Err(Error::Invalid(
                "Agent retained bibliographic source profile",
            ));
        }
        let schema = if let Some(profile) = profile {
            if !matches!(
                reader,
                Some(
                    "identity-relation-v1"
                        | "historical-temporal-v1"
                        | "document-catalogue-temporal-v1"
                        | "structured-value-v1"
                        | "structured-reference-value-v1"
                        | "identity-transition-v1"
                        | "identity-transition-v2"
                )
            ) || claim["claim_type"] != "relation"
                || !array(profile, "assertion_layers")?.contains(&claim["assertion_layer"])
                || claim["claim_id"] == claim["subject_ref"]
                || claim["claim_id"] == claim["object"]
                || reader == Some("identity-relation-v1") && !claim["object"].is_string()
                || reader != Some("identity-relation-v1") && !claim["object"].is_object()
            {
                return Err(Error::Invalid("Agent retained native Claim shape/profile"));
            }
            let route = array(profile, "schemas")?
                .iter()
                .find(|r| r["schema_version"] == claim["schema_version"])
                .ok_or(Error::Invalid("Agent exact Claim schema route"))?;
            let schema = bytes::text(route, "schema_ref")?;
            if entry["source_schema_ref"] != schema {
                return Err(Error::Invalid("Agent addressed Claim schema profile"));
            }
            schema.to_owned()
        } else {
            entry["source_schema_ref"]
                .as_str()
                .unwrap_or(match claim["schema_version"].as_str() {
                    Some("tos_claim_packet_v1") => "ToS/contracts/claim-packet.schema.json",
                    Some("tos_object_link_claim_v1") => {
                        "ToS/contracts/object-link-claim.schema.json"
                    }
                    Some("tos_object_link_claim_v2") => {
                        "ToS/contracts/object-link-claim-v2.schema.json"
                    }
                    _ => return Err(Error::Invalid("Agent legacy Claim schema")),
                })
                .to_owned()
        };
        let claimraw = bytes::canonical(&claim, self.limits.catalog.max_row_bytes)?;
        self.check(source, &claimraw, &schema)?;
        if native {
            self.check(
                source,
                &claimraw,
                "ToS/contracts/source-claim-record.schema.json",
            )?;
        }
        if matches!(
            reader,
            Some("historical-temporal-v1" | "document-catalogue-temporal-v1")
        ) || historical
        {
            let temporal = if reader == Some("document-catalogue-temporal-v1") {
                "ToS/contracts/document-catalogue-claim.schema.json#/$defs/documentDate"
            } else {
                "ToS/contracts/historical-claim.schema.json#/$defs/historicalDate"
            };
            if claim["object"].is_object() {
                self.check(
                    source,
                    &bytes::canonical(&claim["object"], self.limits.catalog.max_row_bytes)?,
                    temporal,
                )?;
            }
        }
        if matches!(
            reader,
            Some(
                "structured-value-v1"
                    | "structured-reference-value-v1"
                    | "identity-transition-v1"
                    | "identity-transition-v2"
            )
        ) {
            self.check(
                source,
                &bytes::canonical(&claim["object"], self.limits.catalog.max_row_bytes)?,
                "ToS/contracts/source-structured-value.schema.json",
            )?;
            if claim["object"]["kind"] != profile.unwrap()["value_kind"] {
                return Err(Error::Invalid("Agent declared structured value kind"));
            }
        }
        if claim
            .pointer("/qualifiers/display_fields/schema_version")
            .and_then(Value::as_str)
            == Some("tos_claim_display_fields_v1")
        {
            self.check(
                source,
                &bytes::canonical(&claim["qualifiers"], self.limits.catalog.max_row_bytes)?,
                "ToS/contracts/claim-display-fields.schema.json",
            )?;
        }
        let cap = self.limits.catalog.max_output_row_bytes;
        let subject = self.identity_node(bytes::text(&claim, "subject_ref")?, entities)?;
        let object = if let Some(id) = claim["object"].as_str() {
            if let Some(meta) = self.metadata(id, entities)? {
                meta.identity.clone()
            } else if id.starts_with("tos.") {
                return Err(Error::Invalid(
                    "Agent identity-like Claim object unresolved",
                ));
            } else {
                bib::supplied_bibliographic_literal(entry, &claim, self.limits.catalog.into())?
            }
        } else {
            bib::supplied_bibliographic_literal(entry, &claim, self.limits.catalog.into())?
        };
        let transition = matches!(
            reader,
            Some("identity-transition-v1" | "identity-transition-v2")
        );
        if (native || historical)
            && !transition
            && (!bib::supplied_bibliographic_endpoint_matches(
                &subject,
                &relation["domain_type_ids"],
                entities,
                self.limits.catalog.into(),
            )? || object["node_kind"] == "identity"
                && !bib::supplied_bibliographic_endpoint_matches(
                    &object,
                    &relation["range_type_ids"],
                    entities,
                    self.limits.catalog.into(),
                )?)
        {
            return Err(Error::Invalid("Agent Claim endpoint domain/range"));
        }
        let mut members = Vec::new();
        let mut basis = None;
        if let Some(profile) = profile {
            for id in bib::supplied_claim_reference_members(
                &claim,
                profile,
                self.limits.catalog.max_row_bytes,
            )? {
                let node = self.identity_node(id, entities)?;
                if !transition
                    && !bib::supplied_bibliographic_endpoint_matches(
                        &node,
                        &profile["object_reference_set"]["member_type_ids"],
                        entities,
                        self.limits.catalog.into(),
                    )?
                {
                    return Err(Error::Invalid("Agent Claim member endpoint domain"));
                }
                if transition {
                    let kind = bytes::text(&node["properties"], "identity_kind")?;
                    let eligible = array(entities, "types")?.iter().any(|e| {
                        e["abstract"] == false
                            && e["source_mappings"].as_array().is_some_and(|m| {
                                m.iter().any(|m| {
                                    m["source_graph"] == "source-claims"
                                        && m["source_kind_id"] == kind
                                })
                            })
                            && (e["object_role"] == "identity"
                                || reader == Some("identity-transition-v2")
                                    && e["object_role"] == "semantic"
                                    && e.pointer(
                                        "/source_record_profile/identity_proposal_adapter",
                                    )
                                    .and_then(Value::as_str)
                                        == Some("exact-semantic-metadata-v1"))
                    });
                    if !eligible {
                        return Err(Error::Invalid(
                            "Agent proposal participant concrete source role",
                        ));
                    }
                }
                members.push(node);
            }
            if profile
                .pointer("/object_reference_set/structure_adapter")
                .and_then(Value::as_str)
                == Some("scoped-members-v1")
            {
                self.check(
                    source,
                    &bytes::canonical(&claim["object"], self.limits.catalog.max_row_bytes)?,
                    "ToS/contracts/scoped-member-structure.schema.json",
                )?;
            }
            if profile
                .pointer("/object_reference_set/basis_adapter")
                .and_then(Value::as_str)
                == Some("collection-membership-versions-v1")
            {
                let (ctx, cut) = match self.side {
                    Side::Original => (
                        self.observation.original_context(),
                        self.observation.original_cut(),
                    ),
                    Side::Current => (
                        self.observation.current_context(),
                        self.observation.current_cut(),
                    ),
                };
                basis = Some(view(
                    &owner(
                        crate::source_claims::ground_addressed_collection_membership(
                            ctx,
                            cut,
                            &typed(&claim, self.limits.catalog.max_row_bytes)?,
                            self.worker,
                            self.limits.deadline,
                            self.cancelled,
                        ),
                    )?,
                    self.limits.max_claim_cohort_bytes,
                )?);
            }
        }
        bib::validate_supplied_catalogue_attribution(&claim)?;
        let (event_source, event_location) = self
            .slot(
                "provenance_event",
                bytes::text(&claim, "provenance_event_ref")?,
            )?
            .ok_or(Error::Invalid("Agent Claim provenance event unresolved"))?;
        let eventschema = match event_source["schema_version"].as_str() {
            Some("tos_provenance_event_v1") => "ToS/contracts/provenance-event.schema.json",
            Some("tos_provenance_event_v2") => "ToS/contracts/provenance-event-v2.schema.json",
            _ => return Err(Error::Invalid("Agent provenance schema")),
        };
        self.check(
            bytes::text(&event_location, "source_ref")?,
            &bytes::canonical(&event_source, self.limits.catalog.max_row_bytes)?,
            eventschema,
        )?;
        let event = bib::supplied_bibliographic_event(&event_source, &event_location, cap)?;
        let maker_id = bytes::text(&claim["maker"], "agent_ref")?;
        let maker_identity = self
            .metadata(maker_id, entities)?
            .map(|m| m.identity.clone());
        let fallback = bytes::text(
            &self.header["legacy_baseline"]["files"]["ToS/source-witnesses/catalog/claims.jsonl"],
            "sha256",
        )?;
        let (maker, maker_identity) =
            bib::supplied_bibliographic_maker(&claim, maker_identity, fallback, cap)?;
        let mut evidence = Vec::new();
        let mut counterevidence = Vec::new();
        for (field, resolved) in [
            ("evidence_refs", &mut evidence),
            ("counterevidence_refs", &mut counterevidence),
        ] {
            if claim.get(field).is_some() {
                for reference in array(&claim, field)? {
                    resolved.push(
                        self.evidence(
                            reference
                                .as_str()
                                .ok_or(Error::Invalid("Agent evidence ref string"))?,
                            &claim,
                            entry,
                            entities,
                        )?,
                    );
                }
            }
        }
        let mut normalized = Vec::new();
        if predicate == "provision_activity" {
            let mut refs = BTreeSet::new();
            for (field, key, edge, kinds) in [
                (
                    "places",
                    "normalized_place_ref",
                    "has_normalized_place",
                    &["place"][..],
                ),
                (
                    "agents",
                    "normalized_agent_ref",
                    "has_normalized_agent",
                    &["agent", "organization"][..],
                ),
            ] {
                for row in claim["object"][field].as_array().into_iter().flatten() {
                    if let Some(id) = row[key].as_str() {
                        let node = self.identity_node(id, entities)?;
                        if !kinds.contains(&bytes::text(&node["properties"], "identity_kind")?) {
                            return Err(Error::Invalid("Agent normalized provision kind"));
                        }
                        refs.insert((edge.to_owned(), id.to_owned()));
                    }
                }
            }
            for (edge, id) in refs {
                normalized.push((edge, self.identity_node(&id, entities)?));
            }
        }
        if (predicate == "historical_dating"
            || matches!(
                reader,
                Some("historical-temporal-v1" | "document-catalogue-temporal-v1")
            ))
            && !claim["object"]["relative"].is_null()
        {
            let node = self.identity_node(
                bytes::text(&claim["object"]["relative"], "anchor_ref")?,
                entities,
            )?;
            if !bib::supplied_bibliographic_endpoint_matches(
                &node,
                &json!(["tos.entity.historical-situation"]),
                entities,
                self.limits.catalog.into(),
            )? {
                return Err(Error::Invalid("Agent relative historical anchor domain"));
            }
            normalized.push(("has_historical_date_anchor".to_owned(), node));
        }
        for id in claim["alternative_claim_refs"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(claim.get("supersedes_claim_ref").filter(|v| !v.is_null()))
        {
            self.claim_inputs(
                id.as_str()
                    .ok_or(Error::Invalid("Agent alternative Claim ref"))?,
                entities,
                registry,
            )?;
        }
        let forms = if matches!(
            source.rsplit('/').next(),
            Some("source-claims.jsonl" | "historical-claims.jsonl")
        ) {
            let r = format!(
                "{}.{}.human-forms.json",
                source
                    .strip_suffix(".jsonl")
                    .ok_or(Error::Invalid("Agent Claim forms source"))?,
                bytes::digest(declaration.id.as_bytes())
            );
            if let Some(raw) = self.optional(&r)? {
                self.check(&r, &raw, "ToS/contracts/human-form-set.schema.json")?;
                Some((
                    r,
                    Value::Array(crate::source_forms_compiler::materialize_compiler_forms(
                        &claim,
                        &bytes::parse(&raw, 2_097_152)?,
                        262_144,
                    )?),
                ))
            } else {
                None
            }
        } else {
            None
        };
        let legacy_context = if legacy_link {
            if claim["schema_version"] != "tos_object_link_claim_v1"
                || !matches!(
                    predicate,
                    "described_by" | "metadata_at" | "downloadable_at" | "rights_statement_at"
                )
                || !matches!(
                    subject["properties"]["identity_kind"].as_str(),
                    Some("work" | "expression" | "edition" | "collection" | "item")
                )
                || object["properties"]["identity_kind"] != "link"
            {
                return Err(Error::Invalid("Agent legacy object link domain"));
            }
            Some(
                json!({"source_claim":claim,"source_claim_file_ref":source,"source_claim_line":entry["source_claim_line"],"source_sha256":entry["claim_sha256"],"source_schema_ref":"ToS/contracts/object-link-claim.schema.json","source_adapter":"retained-object-link-v1"}),
            )
        } else {
            None
        };
        let descriptor = bib::supplied_claim_navigation_descriptor(
            &claim,
            &subject,
            &object,
            registry,
            entities,
            self.limits.catalog.into(),
        )?;
        let inputs = SuppliedBibliographicClaimInputs {
            entry,
            claim: &claim,
            subject,
            object,
            event,
            maker,
            maker_identity,
            evidence,
            counterevidence,
            members,
            normalized,
            descriptor,
            forms,
            collection_order_basis: basis,
            legacy_context,
        };
        let dependencies = enumerate(&inputs, self.limits.catalog.max_row_bytes)?;
        let rebuilt = json!({"schema":"tos_source_claim_dependencies_v1","claim_id":declaration.id,"source_entry":entry,"source_entry_sha256":bytes::row_digest(entry,self.limits.catalog.max_row_bytes)?,"input_sha256":entry["claim_sha256"],"dependencies":dependencies});
        if bytes::row_digest(&rebuilt, self.limits.catalog.max_row_bytes)? != declaration.digest
            || rebuilt != declaration.value
        {
            return Err(Error::Invalid(
                "Agent reverse declaration differs from actual forward dependencies",
            ));
        }
        bib::render_supplied_claim(inputs, self.limits)
    }
    fn cohort(
        &mut self,
        declarations: &[Declaration],
        entities: &Value,
        registry: &Value,
    ) -> Result<Cohort> {
        let record_id = self.observation.record_id().to_owned();
        let limits = self.limits;
        let meta = self
            .metadata(&record_id, entities)?
            .ok_or(Error::Invalid("Agent selected record unavailable"))?;
        if meta.entry["record_type"] != "agent" {
            return Err(Error::Invalid(
                "Agent correction requires selected Agent record",
            ));
        }
        let navigation = bib::render_supplied_navigation_record(
            bib::SuppliedNavigationRecordInput {
                entry: &meta.entry,
                source_record: &meta.record,
                forms: meta.forms.as_ref().map(|(r, v)| (r.as_str(), v)),
                history: Some(&meta.history),
                versions: &meta.versions,
                native_composite: false,
            },
            limits,
        )?;
        if !navigation.diagnostics.is_empty() {
            return Err(Error::Invalid("Agent exact record history has diagnostics"));
        }
        let identity = meta.identity.clone();
        let mut result = Cohort {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            traces: BTreeMap::new(),
        };
        for node in navigation.nodes {
            put_unique(
                &mut result.nodes,
                "source-navigation",
                "node_id",
                node,
                limits.catalog.max_output_row_bytes,
            )?;
        }
        for edge in navigation.edges {
            put_unique(
                &mut result.edges,
                "source-navigation",
                "edge_id",
                edge,
                limits.catalog.max_output_row_bytes,
            )?;
        }
        put_unique(
            &mut result.nodes,
            "source-claims",
            "node_id",
            identity,
            limits.catalog.max_output_row_bytes,
        )?;
        let mut seen = BTreeSet::new();
        for declaration in declarations {
            self.active()?;
            if !seen.insert(declaration.id.clone()) {
                return Err(Error::Invalid("Agent reverse declaration repeats Claim"));
            }
            let validated =
                Declaration::parse(&declaration.raw, limits.catalog.max_row_bytes, 4096)?;
            if validated.id != declaration.id
                || validated.value != declaration.value
                || validated.digest != declaration.digest
            {
                return Err(Error::Invalid("Agent retained declaration carrier differs"));
            }
            let projected = self.project_claim(&validated, entities, registry)?;
            for node in projected.nodes {
                put_unique(
                    &mut result.nodes,
                    "source-claims",
                    "node_id",
                    node,
                    limits.catalog.max_output_row_bytes,
                )?;
            }
            for edge in projected.edges {
                put_unique(
                    &mut result.edges,
                    "source-claims",
                    "edge_id",
                    edge,
                    limits.catalog.max_output_row_bytes,
                )?;
            }
            let claim_ref = bytes::text(&projected.trace, "claim_ref")?.to_owned();
            if let Some(old) = result.traces.get(&claim_ref) {
                if bytes::row_digest(&old.value, limits.catalog.max_output_row_bytes)?
                    != bytes::row_digest(&projected.trace, limits.catalog.max_output_row_bytes)?
                {
                    return Err(Error::Invalid("Agent trace contributors disagree"));
                }
            }
            let raw = bytes::canonical(&projected.trace, limits.catalog.max_output_row_bytes)?;
            result.traces.insert(
                claim_ref,
                OrderedRow {
                    value: projected.trace,
                    raw,
                },
            );
            self.check_cohort(&result)?;
        }
        self.check_cohort(&result)?;
        Ok(result)
    }
    fn check_cohort(&self, cohort: &Cohort) -> Result<()> {
        self.active()?;
        if cohort.nodes.len() > 4096 || cohort.edges.len() > 16384 || cohort.traces.len() > 512 {
            return Err(Error::Budget("Agent captured source cohort count"));
        }
        let mut total = 0usize;
        for row in cohort
            .nodes
            .values()
            .chain(cohort.edges.values())
            .chain(cohort.traces.values())
        {
            total = total
                .checked_add(row.raw.len())
                .filter(|n| *n <= self.limits.max_output_bytes.min(16_777_216) as usize)
                .ok_or(Error::Budget("Agent captured source cohort bytes"))?;
        }
        Ok(())
    }
    fn metadata(&mut self, id: &str, entities: &Value) -> Result<Option<&Metadata>> {
        if self.metadata.contains_key(id) {
            return Ok(self.metadata.get(id));
        }
        let Some(row) = self.record_row(id)? else {
            return Ok(None);
        };
        self.check(
            id,
            &bytes::canonical(&row, self.limits.catalog.max_row_bytes)?,
            &format!("{CATALOG_SCHEMA}#/$defs/row"),
        )?;
        let reference = bytes::text(&row["source"], "source_ref")?.to_owned();
        let raw = self.read(&reference)?;
        let record = bytes::parse(&raw, self.limits.catalog.max_row_bytes)?;
        let exact = view(
            &owner(crate::source_forms::metadata_subject(&typed(
                &record,
                self.limits.catalog.max_row_bytes,
            )?))?,
            self.limits.catalog.max_row_bytes,
        )?;
        if row["record_id"] != id
            || row["entry"]["record_id"] != id
            || row["source"]
                != json!({"source_ref":reference,"raw_sha256":bytes::digest(&raw),"raw_bytes":raw.len(),"record_ref":exact})
        {
            return Err(Error::Invalid("Agent metadata addressed source binding"));
        }
        let descriptor = bib::supplied_metadata_descriptor(
            &row["entry"],
            &record,
            entities,
            self.limits.catalog.into(),
        )?;
        let schema = bytes::text(&descriptor, "source_schema_ref")?.to_owned();
        self.check(&reference, &raw, &schema)?;
        let entry = tos_compiler::source_witness_catalog::render_catalog_record(
            &record,
            &reference,
            row["entry"]["source_schema_ref"].as_str(),
            self.limits.catalog.max_output_row_bytes,
        )?;
        if entry != row["entry"] {
            return Err(Error::Invalid("Agent metadata catalog renderer differs"));
        }
        let (home, name) = reference
            .rsplit_once('/')
            .ok_or(Error::Invalid("Agent metadata source parent"))?;
        let stem = name
            .strip_suffix(".json")
            .ok_or(Error::Invalid("Agent metadata source basename"))?;
        let forms_ref = format!("{home}/{stem}.human-forms.json");
        let forms = if let Some(forms_raw) = self.optional(&forms_ref)? {
            self.check(
                &forms_ref,
                &forms_raw,
                "ToS/contracts/human-form-set.schema.json",
            )?;
            let material = Value::Array(crate::source_forms_compiler::materialize_compiler_forms(
                &record,
                &bytes::parse(&forms_raw, 2_097_152)?,
                262_144,
            )?);
            Some((forms_ref, material))
        } else {
            None
        };
        let history_ref = format!("{home}/source-revision-history.json");
        let mut refs = Vec::new();
        if let Some(history_raw) = self.optional(&history_ref)? {
            let history = bytes::parse(&history_raw, self.limits.catalog.max_file_bytes)?;
            for receipt in array(&history, "receipts")? {
                refs.push(receipt["previous_source"].clone());
            }
        }
        refs.push(exact.clone());
        if refs.len() > 129 {
            return Err(Error::Budget("Agent retained metadata versions"));
        }
        let mut versions = Vec::new();
        let address = json!({"schema_version":"tos_source_catalog_address_v2","catalog_namespace":self.header["catalog_namespace"],"profile_id":"tos.source-catalog.public-records.v2","record_key":id,"row_sha256":bytes::row_digest(&row,self.limits.catalog.max_row_bytes)?,"source_ref":reference,"raw_sha256":bytes::digest(&raw),"raw_bytes":raw.len(),"record_ref":exact});
        let mut base_provenance = Value::Null;
        let mut retained_bytes = 0usize;
        for exact_ref in &refs {
            self.active()?;
            let (ctx, cut) = match self.side {
                Side::Original => (
                    self.observation.original_context(),
                    self.observation.original_cut(),
                ),
                Side::Current => (
                    self.observation.current_context(),
                    self.observation.current_cut(),
                ),
            };
            if self.worker.source_revision() != cut.current().revision()
                || ctx.base_revision != cut.current().revision()
            {
                return Err(Error::Invalid("Agent actual resolver worker/cut revision"));
            }
            let resolved = owner(
                crate::source_revisions::resolve_record_version_evidence_at_from_cut(
                    ctx,
                    cut,
                    &reference,
                    ItemLimits {
                        max_member_bytes: self.limits.catalog.max_file_bytes.min(8_388_608),
                        max_total_bytes: 33_554_432,
                        max_state_bytes: 33_554_432,
                        max_issues: 256,
                        deadline: self.limits.deadline,
                    },
                    &typed(exact_ref, self.limits.catalog.max_row_bytes)?,
                    self.worker,
                    self.limits.deadline,
                    self.cancelled,
                ),
            )?;
            if view(&resolved.current_ref, self.limits.catalog.max_row_bytes)? != exact
                || resolved.source_path != reference
            {
                return Err(Error::Invalid(
                    "Agent cold resolver current addressed binding",
                ));
            }
            let history_evidence = view(&resolved.history, self.limits.catalog.max_row_bytes)?;
            base_provenance = json!({"verification_scope":"selected-record-chain","all_package_bytes_verified":false,"catalog":address,"descriptor":descriptor,"history":history_evidence});
            let mut provenance = base_provenance.clone();
            provenance["source"] = view(&resolved.source, self.limits.catalog.max_row_bytes)?;
            provenance["transition"] =
                view(&resolved.transition, self.limits.catalog.max_row_bytes)?;
            let value = json!({"status":"available","reason":format!("exact-{}-version",resolved.version_status),"exact_ref":exact_ref,"version_status":resolved.version_status,"record":view(&resolved.record,self.limits.catalog.max_row_bytes)?,"record_digest":exact_ref["digest"],"provenance":provenance,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false});
            retained_bytes = retained_bytes
                .checked_add(bytes::canonical(&value, self.limits.max_claim_cohort_bytes)?.len())
                .filter(|n| *n <= self.limits.max_claim_cohort_bytes)
                .ok_or(Error::Budget("Agent metadata history state"))?;
            versions.push((exact_ref.clone(), value));
        }
        let history = json!({"status":"available","reason":"verified-record-references","record_id":id,"current_ref":exact,"refs":refs,"provenance":base_provenance,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false});
        let identity = bib::supplied_bibliographic_identity(
            &entry,
            &record,
            forms.as_ref().map(|(r, v)| (r.as_str(), v)),
            self.limits.catalog.into(),
        )?;
        let retained = json!({"entry":entry,"record":record,"identity":identity,"forms":forms,"history":history});
        let state_bytes = bytes::canonical(&retained, 33_554_432)?
            .len()
            .checked_add(retained_bytes)
            .ok_or(Error::Budget("Agent metadata retained arithmetic"))?;
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(state_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(Error::Budget("Agent aggregate addressed metadata state"))?;
        drop(retained);
        self.metadata.insert(
            id.to_owned(),
            Metadata {
                entry,
                record,
                identity,
                forms,
                history,
                versions,
            },
        );
        Ok(self.metadata.get(id))
    }
}

/// Actual source observer, addressed immutable baseline and complete indexed
/// reverse declarations are required. No externally supplied cohort is accepted.
/// The sole whole caller just authenticated the held source guard and authenticates
/// it again immediately before commit; reads retain their exact namespace fences.
/// The parent retains observation and checks the indexed selection again under
/// its caller-owned transaction before any prepared publication.
#[allow(clippy::too_many_arguments)]
pub(super) fn assemble(
    observation: &CommittedRecordObservation<'_>,
    roots: &mut Roots,
    reverse: &[Declaration],
    original_worker: &mut CutWorkerSchemaExecutor,
    current_worker: &mut CutWorkerSchemaExecutor,
    limits: BibliographicLimits,
    cancelled: &AtomicBool,
) -> Result<AssembledAgentCorrection> {
    if reverse.len() > 512
        || limits.catalog.max_row_bytes == 0
        || limits.catalog.max_row_bytes > 1_048_576
        || limits.catalog.max_file_bytes == 0
        || limits.catalog.max_file_bytes > 8_388_608
        || limits.max_output_bytes == 0
        || limits.max_output_bytes > 16_777_216
        || original_worker.source_revision() != observation.original_cut().current().revision()
        || current_worker.source_revision() != observation.current_cut().current().revision()
    {
        return Err(Error::Invalid("Agent assembly bounded actual cut workers"));
    }
    let header = roots
        .snapshots
        .get("source-catalog")
        .ok_or(Error::Invalid("Agent source catalog root absent"))?
        .manifest["header"]
        .clone();
    let binding = observation.binding();
    if header["claims_addressed"] != true
        || header["source_publication"]
            != view(observation.original_source_publication(), 1_048_576)?
        || !binding.source_path.as_str().ends_with("/agent.json")
    {
        return Err(Error::Invalid("Agent catalog exact immediate predecessor"));
    }
    let request = bytes::parse(&observation.current_context().request_raw, 1_048_576)?;
    let fields = request["fields"]
        .as_object()
        .filter(|m| !m.is_empty())
        .ok_or(Error::Invalid("Agent descriptive source fields"))?;
    if fields
        .keys()
        .any(|f| !matches!(f.as_str(), "preferred_label" | "notes" | "field_languages"))
    {
        return Err(Error::Invalid(
            "Agent prepared correction forbids membership/reference edits",
        ));
    }
    let before_row = roots
        .get("source-catalog", "records", observation.record_id())?
        .ok_or(Error::Invalid("Agent selected predecessor catalog row"))?;
    if before_row["source"]["raw_sha256"] != binding.original.sha256.to_hex()
        || before_row["source"]["raw_bytes"].as_u64() != Some(binding.original.raw.len() as u64)
        || before_row["source"]["record_ref"] != view(&binding.original.subject, 1_048_576)?
        || before_row["source"]["source_ref"] != binding.source_path.as_str()
    {
        return Err(Error::Invalid(
            "Agent predecessor row differs from authenticated archive",
        ));
    }
    let current = bytes::parse(&binding.current.raw, limits.catalog.max_row_bytes)?;
    let entry = tos_compiler::source_witness_catalog::render_catalog_record(
        &current,
        binding.source_path.as_str(),
        before_row["entry"]["source_schema_ref"].as_str(),
        limits.catalog.max_output_row_bytes,
    )?;
    let new_row = json!({"record_id":observation.record_id(),"entry":entry,"source":{"source_ref":binding.source_path.as_str(),"raw_sha256":binding.current.sha256.to_hex(),"raw_bytes":binding.current.raw.len(),"record_ref":view(&binding.current.subject,1_048_576)?}});
    let mut after_header = header.clone();
    after_header["source_publication"] = view(observation.source_publication(), 1_048_576)?;
    after_header["last_transition"] = json!({"transaction_id":observation.transaction_id(),"manifest_sha256":observation.manifest_sha256().to_prefixed(),"record_id":observation.record_id()});
    let (old, new) = {
        let mut old = Reader {
            observation,
            side: Side::Original,
            roots,
            worker: original_worker,
            header: &header,
            revised_row: None,
            files: BTreeMap::new(),
            file_bytes: 0,
            metadata: BTreeMap::new(),
            metadata_bytes: 0,
            limits,
            cancelled,
        };
        old.check(
            "Agent predecessor catalog header",
            &bytes::canonical(&header, 1_048_576)?,
            &format!("{CATALOG_SCHEMA}#/$defs/header"),
        )?;
        let source_profiles = header["profile_bindings"]["source"]
            .as_object()
            .ok_or(Error::Invalid("Agent source catalog profiles"))?;
        for (reference, profile) in source_profiles {
            let raw = old.read(reference)?;
            if *profile != json!({"sha256":bytes::digest(&raw),"bytes":raw.len()}) {
                return Err(Error::Invalid(
                    "Agent original source profile bytes changed",
                ));
            }
        }
        let entities = bytes::parse(&old.read(ENTITIES)?, limits.catalog.max_contract_bytes)?;
        let registry = bytes::parse(&old.read(RELATIONS)?, limits.catalog.max_contract_bytes)?;
        let old_cohort = old.cohort(reverse, &entities, &registry)?;
        drop(old);
        let mut new = Reader {
            observation,
            side: Side::Current,
            roots,
            worker: current_worker,
            header: &after_header,
            revised_row: Some(&new_row),
            files: BTreeMap::new(),
            file_bytes: 0,
            metadata: BTreeMap::new(),
            metadata_bytes: 0,
            limits,
            cancelled,
        };
        for (reference, profile) in source_profiles {
            let raw = new.read(reference)?;
            if *profile != json!({"sha256":bytes::digest(&raw),"bytes":raw.len()}) {
                return Err(Error::Invalid(
                    "Agent successor source profile bytes changed",
                ));
            }
        }
        if bytes::parse(&new.read(ENTITIES)?, limits.catalog.max_contract_bytes)? != entities
            || bytes::parse(&new.read(RELATIONS)?, limits.catalog.max_contract_bytes)? != registry
        {
            return Err(Error::Invalid(
                "Agent registry changes require explicit bootstrap",
            ));
        }
        new.check(
            "Agent successor catalog header",
            &bytes::canonical(&after_header, 1_048_576)?,
            &format!("{CATALOG_SCHEMA}#/$defs/header"),
        )?;
        new.check(
            observation.record_id(),
            &bytes::canonical(&new_row, limits.catalog.max_row_bytes)?,
            &format!("{CATALOG_SCHEMA}#/$defs/row"),
        )?;
        (old_cohort, new.cohort(reverse, &entities, &registry)?)
    };
    if old.traces.len() != new.traces.len()
        || old.traces.iter().any(|(id, trace)| {
            new.traces
                .get(id)
                .is_none_or(|other| other.value != trace.value)
        })
    {
        return Err(Error::Invalid(
            "Agent descriptive correction changes Claim traces",
        ));
    }

    Ok(AssembledAgentCorrection {
        old,
        new,
        catalog_header: after_header,
        catalog_changes: vec![RootsChange {
            collection: "records".to_owned(),
            key: observation.record_id().to_owned(),
            before_sha256: Some(bytes::row_digest(
                &before_row,
                limits.catalog.max_row_bytes,
            )?),
            after: new_row,
        }],
        reverse_dependent_claims: reverse.len(),
    })
}

pub(super) struct AssembledAgentCorrection {
    pub old: Cohort,
    pub new: Cohort,
    pub catalog_header: Value,
    pub catalog_changes: Vec<RootsChange>,
    pub reverse_dependent_claims: usize,
}

type DependencyRows = BTreeMap<(String, Option<String>), (BTreeSet<String>, BTreeSet<String>)>;
fn add(rows: &mut DependencyRows, kind: &str, reference: &Value, field: &str, reason: &str) {
    let reference = reference
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let (kind, reason) = if reference.is_none() {
        (
            "unresolved",
            format!("{reason}:missing-or-nonidentifiable-ref"),
        )
    } else {
        (kind, reason.to_owned())
    };
    let row = rows.entry((kind.to_owned(), reference)).or_default();
    row.0.insert(field.to_owned());
    row.1.insert(reason);
}
fn node_sources(rows: &mut DependencyRows, node: &Value, field: &str) {
    add(
        rows,
        "path",
        &node["source_ref"],
        &format!("{field}/source_ref"),
        "resolved-node-source",
    );
    if !node["properties"]["human_forms"].is_null()
        || !node["properties"]["human_forms_source_ref"].is_null()
    {
        add(
            rows,
            "path",
            &node["properties"]["human_forms_source_ref"],
            &format!("{field}/properties/human_forms_source_ref"),
            "resolved-identity-human-forms",
        );
    }
}
fn identity(rows: &mut DependencyRows, node: &Value, field: &str) {
    add(
        rows,
        "identity",
        &node["properties"]["identity_ref"],
        &format!("{field}/properties/identity_ref"),
        "resolved-identity",
    );
    node_sources(rows, node, field);
}
fn array<'a>(value: &'a Value, field: &str) -> Result<&'a [Value]> {
    value[field]
        .as_array()
        .map(Vec::as_slice)
        .ok_or(Error::Invalid("Agent Claim source array"))
}
/// Exact enumerate_bibliographic_claim_dependencies grammar. This is consumed
/// only after actual addressed source assembly, never supplied declarations.
fn enumerate(inputs: &SuppliedBibliographicClaimInputs<'_>, cap: usize) -> Result<Value> {
    let mut rows = DependencyRows::new();
    let claim = inputs.claim;
    add(
        &mut rows,
        "claim",
        &claim["claim_id"],
        "/source_claim/claim_id",
        "projected-claim",
    );
    add(
        &mut rows,
        "path",
        &inputs.entry["source_claim_file_ref"],
        "/entry/source_claim_file_ref",
        "source-claim-slot",
    );
    add(
        &mut rows,
        "identity",
        &claim["subject_ref"],
        "/source_claim/subject_ref",
        "declared-subject",
    );
    identity(&mut rows, &inputs.subject, "/subject_node");
    match inputs.object["node_kind"].as_str() {
        Some("identity") => {
            add(
                &mut rows,
                "identity",
                &claim["object"],
                "/source_claim/object",
                "declared-identity-object",
            );
            identity(&mut rows, &inputs.object, "/object_node");
        }
        Some("literal") => node_sources(&mut rows, &inputs.object, "/object_node"),
        _ => {
            add(
                &mut rows,
                "unresolved",
                &claim["object"],
                "/source_claim/object",
                "unknown-object-node-kind",
            );
            node_sources(&mut rows, &inputs.object, "/object_node");
        }
    }
    add(
        &mut rows,
        "provenance_event",
        &claim["provenance_event_ref"],
        "/source_claim/provenance_event_ref",
        "declared-provenance-event",
    );
    add(
        &mut rows,
        "provenance_event",
        &inputs.event["properties"]["event_ref"],
        "/event_node/properties/event_ref",
        "resolved-provenance-event",
    );
    node_sources(&mut rows, &inputs.event, "/event_node");
    let maker_ref = &claim["maker"]["agent_ref"];
    let resolved_maker = &inputs.maker["properties"]["agent_ref"];
    let maker_kind = if inputs.maker_identity.is_some()
        || !inputs.maker["properties"]["identity_node_id"].is_null()
    {
        "identity"
    } else {
        "unresolved"
    };
    add(
        &mut rows,
        maker_kind,
        maker_ref,
        "/source_claim/maker/agent_ref",
        if maker_kind == "identity" {
            "maker-identity"
        } else {
            "maker-without-resolved-metadata-identity"
        },
    );
    add(
        &mut rows,
        maker_kind,
        resolved_maker,
        "/maker_node/properties/agent_ref",
        "resolved-maker-reference",
    );
    if maker_ref != resolved_maker {
        add(
            &mut rows,
            "unresolved",
            maker_ref,
            "/source_claim/maker/agent_ref",
            "mismatched-maker-resolution",
        );
    }
    node_sources(&mut rows, &inputs.maker, "/maker_node");
    if let Some(node) = &inputs.maker_identity {
        identity(&mut rows, node, "/maker_identity_node");
    }
    for (source_field, nodes_field, nodes) in [
        (
            "evidence_refs",
            "evidence_nodes",
            inputs.evidence.as_slice(),
        ),
        (
            "counterevidence_refs",
            "counterevidence_nodes",
            inputs.counterevidence.as_slice(),
        ),
    ] {
        let refs = if claim.get(source_field).is_some() {
            array(claim, source_field)?
        } else {
            &[]
        };
        for index in 0..refs.len().max(nodes.len()) {
            let node = nodes.get(index).unwrap_or(&Value::Null);
            let evidence_kind = node["properties"]["evidence_kind"]
                .as_str()
                .unwrap_or("missing");
            let kind = match evidence_kind {
                "identity" => "identity",
                "provenance_event" => "provenance_event",
                "repo_path" => "path",
                _ => "unresolved",
            };
            let reason = format!("evidence-kind:{evidence_kind}");
            if let Some(reference) = refs.get(index) {
                add(
                    &mut rows,
                    kind,
                    reference,
                    &format!("/source_claim/{source_field}/{index}"),
                    &reason,
                );
            }
            if nodes.get(index).is_some() {
                let field = format!("/{nodes_field}/{index}");
                add(
                    &mut rows,
                    kind,
                    &node["properties"]["evidence_ref"],
                    &format!("{field}/properties/evidence_ref"),
                    &reason,
                );
                node_sources(&mut rows, node, &field);
            }
            if refs.get(index).is_none()
                || nodes.get(index).is_none()
                || refs.get(index) != Some(&node["properties"]["evidence_ref"])
            {
                add(
                    &mut rows,
                    "unresolved",
                    refs.get(index)
                        .unwrap_or(&node["properties"]["evidence_ref"]),
                    &format!("/source_claim/{source_field}/{index}"),
                    "incomplete-or-mismatched-evidence-resolution",
                );
            }
        }
    }
    for (index, node) in inputs.members.iter().enumerate() {
        identity(&mut rows, node, &format!("/member_nodes/{index}"));
    }
    for (index, (_, node)) in inputs.normalized.iter().enumerate() {
        identity(
            &mut rows,
            node,
            &format!("/normalized_identity_edges/{index}/1"),
        );
    }
    if claim.get("alternative_claim_refs").is_some() {
        for (index, reference) in array(claim, "alternative_claim_refs")?.iter().enumerate() {
            add(
                &mut rows,
                "claim",
                reference,
                &format!("/source_claim/alternative_claim_refs/{index}"),
                "alternative-claim",
            );
        }
    }
    if !claim["supersedes_claim_ref"].is_null() {
        add(
            &mut rows,
            "claim",
            &claim["supersedes_claim_ref"],
            "/source_claim/supersedes_claim_ref",
            "superseded-claim",
        );
    }
    if let Some((reference, _)) = &inputs.forms {
        add(
            &mut rows,
            "path",
            &json!(reference),
            "/forms/0",
            "claim-human-forms",
        );
    }
    if let Some(context) = &inputs.legacy_context {
        if let Some(reference) = context.get("source_claim_file_ref") {
            add(
                &mut rows,
                "path",
                reference,
                "/legacy_object_link_context/source_claim_file_ref",
                "legacy-object-link-source",
            );
        }
    }
    if let Some(basis) = &inputs.collection_order_basis {
        add(
            &mut rows,
            "identity",
            &basis["collection"]["ref"]["id"],
            "/collection_order_basis/collection/ref/id",
            "exact-collection-version-basis",
        );
        if basis.get("memberships").is_some() {
            for (index, membership) in array(basis, "memberships")?.iter().enumerate() {
                add(
                    &mut rows,
                    "claim",
                    &membership["ref"]["id"],
                    &format!("/collection_order_basis/memberships/{index}/ref/id"),
                    "exact-membership-version-basis",
                );
            }
        }
        if let Some(digests) = basis["input_digests"].as_object() {
            for reference in digests.keys() {
                let pointer = reference.replace('~', "~0").replace('/', "~1");
                add(
                    &mut rows,
                    "path",
                    &json!(reference),
                    &format!("/collection_order_basis/input_digests/{pointer}"),
                    "exact-version-basis-input",
                );
            }
        }
    }
    let value = Value::Array(rows.into_iter().map(|((kind, reference), (fields, reasons))|
        json!({"kind":kind,"ref":reference,"field_paths":fields,"reasons":reasons})).collect());
    bytes::canonical(&value, cap)?;
    Ok(value)
}

fn put_unique(
    rows: &mut BTreeMap<(String, String), OrderedRow>,
    graph: &str,
    field: &str,
    row: Value,
    cap: usize,
) -> Result<()> {
    let key = (graph.to_owned(), bytes::text(&row, field)?.to_owned());
    if let Some(old) = rows.get(&key) {
        if bytes::row_digest(&old.value, cap)? != bytes::row_digest(&row, cap)? {
            return Err(Error::Invalid("Agent source cohort contributors disagree"));
        }
    }
    let raw = bytes::canonical(&row, cap)?;
    rows.insert(key, OrderedRow { value: row, raw });
    Ok(())
}
