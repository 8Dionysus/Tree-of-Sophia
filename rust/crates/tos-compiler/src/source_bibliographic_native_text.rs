//! Metadata-only native TextUnit return. Never opens original/content payloads,
//! never replays a derivation, and never treats recorded rights as a new grant.
use crate::knowledge_normalization::SourceRow;
use crate::knowledge_stage::KnowledgeStage;
use crate::source_bibliographic_render::{array, encode, text};
use crate::source_witness_catalog::{
    self as catalog, CATALOG_SOURCE, NATIVE_TEXT, SOURCE_FILES, SourceCatalogLimits,
    SourceCatalogValidator,
};
use crate::{Error, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::Digest256;

struct Resolver<'a, 'b, 'c> {
    stage: &'a KnowledgeStage<'b>,
    validator: &'a SourceCatalogValidator<'c>,
    limits: SourceCatalogLimits,
    files: BTreeMap<String, Vec<u8>>,
    total: usize,
    deadline: std::time::Instant,
}
impl Resolver<'_, '_, '_> {
    fn path(reference: &str, support: bool, content: bool) -> Result<()> {
        let home = if support {
            "ToS/"
        } else {
            "ToS/source-witnesses/"
        };
        if !reference.starts_with(home)
            || reference.len() > 4096
            || reference.contains(['\\', '\0'])
            || reference.split('/').any(|p| {
                p.is_empty()
                    || p == "."
                    || p == ".."
                    || p == "catalog"
                    || p == "owner-local"
                    || p.starts_with('.')
            })
            || !content
                && reference
                    .split('/')
                    .any(|p| matches!(p, "payload" | "local-content"))
        {
            return Err(Error::Invalid("native text exact public metadata path"));
        }
        Ok(())
    }
    fn read(&mut self, reference: &str, expected: Option<&str>, support: bool) -> Result<Vec<u8>> {
        if std::time::Instant::now() >= self.deadline
            || self
                .validator
                .cancelled
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(Error::Budget("native text metadata deadline/cancel"));
        }
        Self::path(reference, support, false)?;
        let raw = if let Some(raw) = self.files.get(reference) {
            raw.clone()
        } else {
            if self.files.len() >= 128 {
                return Err(Error::Budget("native text metadata file closure"));
            }
            let mut raw = None;
            for collection in [NATIVE_TEXT, SOURCE_FILES] {
                if let Some(row) = self
                    .stage
                    .raw_by_id(CATALOG_SOURCE, collection, reference)?
                {
                    if raw.is_some() {
                        return Err(Error::Invalid(
                            "native text source file duplicate collection",
                        ));
                    }
                    raw = Some(row.payload);
                }
            }
            let raw = raw.ok_or(Error::Invalid("native text exact metadata file missing"))?;
            if raw.len() > 1_048_576 {
                return Err(Error::Budget("native text metadata file bytes"));
            }
            self.total = self
                .total
                .checked_add(raw.len())
                .filter(|n| *n <= 8_388_608)
                .ok_or(Error::Budget("native text metadata closure bytes"))?;
            self.files.insert(reference.into(), raw.clone());
            raw
        };
        if expected.is_some_and(|sha| Digest256::of_bytes(&raw).to_hex() != sha) {
            return Err(Error::Invalid("native text metadata exact raw digest"));
        }
        Ok(raw)
    }
    fn record(&mut self, reference: &str, expected: Option<&str>, schema: &str) -> Result<Value> {
        let raw = self.read(reference, expected, false)?;
        catalog::check_catalog_schema(self.stage, self.validator, self.limits, schema, "", &raw)?;
        Ok(SourceRow::parse(&raw, self.limits.max_row_bytes)?
            .value()
            .clone())
    }
    fn schema(&self, value: &Value, schema: &str) -> Result<()> {
        catalog::check_catalog_schema(
            self.stage,
            self.validator,
            self.limits,
            schema,
            "",
            &encode(value, self.limits.max_row_bytes)?,
        )
    }
    fn layer_dependencies(
        &mut self,
        layer: &Value,
        path: &str,
        visiting: &BTreeSet<String>,
    ) -> Result<()> {
        let id = text(layer, "layer_id")?;
        if visiting.contains(id) || visiting.len() >= 16 {
            return Err(Error::Invalid("native text cyclic/deep layer lineage"));
        }
        metadata(
            &self.read(path, None, false)?,
            path,
            self.validator,
            self.deadline,
            MetadataKind::Layer,
        )?;
        let policy = &layer["editorial_policy"];
        self.read(
            text(policy, "policy_ref")?,
            Some(text(policy, "policy_sha256")?),
            true,
        )?;
        let maker = &layer["derivation"]["maker"];
        let mut retained_configuration = None;
        if let Some(reference) = maker.get("configuration_ref").filter(|v| !v.is_null()) {
            let raw = self.read(
                reference
                    .as_str()
                    .ok_or(Error::Invalid("native text maker configuration ref"))?,
                Some(text(maker, "configuration_digest")?),
                true,
            )?;
            if let Ok(configuration) = SourceRow::parse(&raw, self.limits.max_row_bytes) {
                retained_configuration = Some(configuration.value().clone());
            }
        }
        let mut previous_layers = Vec::new();
        let mut next = visiting.clone();
        next.insert(id.into());
        for target in array(&layer["derivation"], "input_layers")? {
            let path = text(target, "record_ref")?;
            let previous = self.record(
                path,
                Some(text(target, "record_sha256")?),
                "ToS/contracts/source-text-layer.schema.json",
            )?;
            if previous["layer_id"] != target["layer_id"]
                || previous["representation"]["content_sha256"] != target["content_sha256"]
            {
                return Err(Error::Invalid(
                    "native text predecessor identity/content binding",
                ));
            }
            self.layer_dependencies(&previous, path, &next)?;
            previous_layers.push(previous);
        }
        if let Some(config) = retained_configuration {
            match config["schema_version"].as_str() {
                Some("tos_local_text_layer_derive_owner_v1") => {
                    self.derived_metadata(layer, &config, &previous_layers, path)?;
                }
                Some(
                    "tos_local_text_layer_record_owner_ocr_v1"
                    | "tos_local_text_layer_record_owner_page_ocr_v1",
                ) => {
                    return Err(Error::Invalid(
                        "native owner OCR requires exact private signature verification context",
                    ));
                }
                _ => (), // Historical methods may retain opaque or unrelated configurations.
            }
        }
        Ok(())
    }
    fn derived_metadata(
        &mut self,
        layer: &Value,
        config: &Value,
        previous: &[Value],
        record_ref: &str,
    ) -> Result<()> {
        let operation = config["allowed_operations"]
            .as_array()
            .filter(|ops| ops.len() == 1)
            .and_then(|ops| ops[0].as_str())
            .ok_or(Error::Invalid("native derivation exact operation"))?;
        let method = match operation {
            "text-layer.correct" => "correction",
            "text-layer.normalize" => "unicode_normalization",
            "text-layer.record-transcription" => "manual_transcription",
            "text-layer.record-ocr" => "ocr",
            _ => return Err(Error::Invalid("native derivation operation profile")),
        };
        let representation = &layer["representation"];
        let derivation = &layer["derivation"];
        let maker = &derivation["maker"];
        let base = text(config, "source_path")?
            .rsplit_once('/')
            .map(|(base, _)| base)
            .ok_or(Error::Invalid("native derivation source locator"))?;
        let raw_policy = self.read(
            text(&layer["editorial_policy"], "policy_ref")?,
            Some(text(&layer["editorial_policy"], "policy_sha256")?),
            true,
        )?;
        let policy = SourceRow::parse(&raw_policy, self.limits.max_row_bytes)?
            .value()
            .clone();
        let scope = &layer["source_binding"];
        let mut exact_scope = serde_json::Map::new();
        for key in ["work_ref", "expression_ref", "edition_ref", "item_ref"] {
            exact_scope.insert(
                key.into(),
                scope
                    .get(key)
                    .ok_or(Error::Invalid("native derivation scope"))?
                    .clone(),
            );
        }
        exact_scope.insert("file_ref".into(), scope["source_file_ref"].clone());
        exact_scope.insert("file_sha256".into(), scope["source_file_sha256"].clone());
        let selected_method = text(derivation, "method")?;
        let method_matches = if operation == "text-layer.record-transcription" {
            matches!(
                selected_method,
                "manual_transcription" | "model_transcription"
            )
        } else {
            selected_method == method
        };
        let role = match operation {
            "text-layer.normalize" => "normalized_text",
            "text-layer.record-ocr" => "raw_ocr",
            _ if selected_method == "model_transcription" => "machine_transcription",
            _ => "diplomatic_transcription",
        };
        let mut expected_maker = serde_json::Map::new();
        for key in ["maker_type", "agent_ref", "method", "version"] {
            expected_maker.insert(
                key.into(),
                maker
                    .get(key)
                    .ok_or(Error::Invalid("native derivation maker"))?
                    .clone(),
            );
        }
        if config["source_path"] != record_ref
            || config["source_scope"] != Value::Object(exact_scope)
            || config["identities"]["layer_id"] != layer["layer_id"]
            || config["identities"]["provenance_event_id"] != layer["provenance_event_ref"]
            || !method_matches
            || config["policy"] != policy
            || policy["schema_version"] != "tos_native_text_layer_derivation_policy_v1"
            || policy["operation"] != operation
            || policy["method"] != selected_method
            || policy["provider_execution_verified"] != false
            || policy["inherited_quality"] != "not-transferred"
            || layer["layer_role"] != role
            || config["language"] != representation["language"]
            || representation["content_ref"] != format!("{base}/content.txt")
            || representation["character_normalization"] != policy["unicode_normalization"]
            || maker["configuration_ref"]
                != format!("{base}/source-create-owner-configuration.json")
            || config["maker"] != Value::Object(expected_maker)
            || representation["rights_record_refs"]
                != config["derivation_access"]["rights_record_refs"]
        {
            return Err(Error::Invalid(
                "native derived layer differs from exact retained configuration",
            ));
        }
        if matches!(operation, "text-layer.correct" | "text-layer.normalize") {
            let previous =
                previous
                    .first()
                    .filter(|_| previous.len() == 1)
                    .ok_or(Error::Invalid(
                        "native derived layer requires exact predecessor",
                    ))?;
            if config.get("source_record_refs").is_none()
                || config
                    .pointer("/input/binding/source_record_refs")
                    .is_none()
            {
                return Err(Error::Invalid(
                    "native derivation source record closure missing",
                ));
            }
            let target = &config["input"]["binding"]["text_layer"];
            let expected = serde_json::json!({"layer_id":target["layer_id"],
                "record_ref":target["record_ref"], "record_sha256":target["record_sha256"],
                "content_sha256":previous["representation"]["content_sha256"]});
            let prior_version = previous["layer_version"]
                .as_u64()
                .ok_or(Error::Invalid("native derivation predecessor version"))?;
            if derivation["input_layers"] != serde_json::json!([expected])
                || previous["layer_version"] != target["layer_version"]
                || previous["source_binding"] != *scope
                || previous["representation"]["language"] != representation["language"]
                || layer["supersedes_layer_ref"] != previous["layer_id"]
                || layer["layer_version"].as_u64() != prior_version.checked_add(1)
                || config["input"]["binding"]["source_record_refs"] != config["source_record_refs"]
            {
                return Err(Error::Invalid(
                    "native derivation predecessor version/scope differs",
                ));
            }
            if operation == "text-layer.correct"
                && (previous["layer_role"] == "normalized_text"
                    || previous["representation"]["character_normalization"] != "none")
            {
                return Err(Error::Invalid(
                    "native source-near correction erases normalization",
                ));
            }
        } else {
            let target = &config["input"]["anchor"];
            let expected = serde_json::json!([{"anchor_id":target["anchor_id"],
                "anchor_record_ref":target["record_ref"], "anchor_record_sha256":target["record_sha256"]}]);
            if !previous.is_empty()
                || layer["layer_version"] != 1
                || !layer["supersedes_layer_ref"].is_null()
                || scope["anchors"] != expected
                || config["material"]["provider_execution"] != "not_observed"
                || representation["content_sha256"] != config["material"]["content_sha256"]
            {
                return Err(Error::Invalid(
                    "native supplied result differs from retained declaration",
                ));
            }
        }
        Ok(())
    }
    fn source_scope(&mut self, binding: &Value, scope: &Value, layer: &Value) -> Result<Value> {
        let mut records = BTreeMap::new();
        for (kind, reference) in binding["source_record_refs"]
            .as_object()
            .ok_or(Error::Invalid("native text source metadata refs"))?
        {
            let reference = reference
                .as_str()
                .ok_or(Error::Invalid("native text source metadata ref"))?;
            if reference.rsplit('/').next() != Some(format!("{kind}.json").as_str()) {
                return Err(Error::Invalid("native text metadata kind locator"));
            }
            let record = self.record(reference, None, "ToS/contracts/corpus-record.schema.json")?;
            let field = format!("{kind}_ref");
            if record["record_type"] != *kind
                || record["record_id"] != scope[&field]
                || layer["source_binding"][&field] != scope[&field]
            {
                return Err(Error::Invalid("native text source scope metadata identity"));
            }
            records.insert(kind.clone(), record);
        }
        let expression = records
            .get("expression")
            .ok_or(Error::Invalid("native text Expression missing"))?;
        let edition = records
            .get("edition")
            .ok_or(Error::Invalid("native text Edition missing"))?;
        let item = records
            .get("item")
            .ok_or(Error::Invalid("native text Item missing"))?;
        if expression["work_ref"] != scope["work_ref"]
            || !array(edition, "embodies_expression_refs")?.contains(&scope["expression_ref"])
        {
            return Err(Error::Invalid("native text bibliographic topology"));
        }
        let manifest_ref = text(item, "item_manifest_ref")?;
        let item_ref = text(&binding["source_record_refs"], "item")?;
        if manifest_ref.rsplit_once('/').map(|p| p.0) != item_ref.rsplit_once('/').map(|p| p.0)
            || manifest_ref.rsplit('/').next() != Some("item.manifest.json")
        {
            return Err(Error::Invalid("native text item manifest exact sibling"));
        }
        let manifest = self.record(
            manifest_ref,
            None,
            "ToS/contracts/source-item-manifest.schema.json",
        )?;
        if manifest["item_id"] != scope["item_ref"]
            || manifest["embodiment_ref"] != scope["edition_ref"]
            || layer["source_binding"]["source_file_ref"] != scope["file_ref"]
            || layer["source_binding"]["source_file_sha256"] != scope["file_sha256"]
        {
            return Err(Error::Invalid("native text Item/File scope binding"));
        }
        let matches = array(&manifest, "payload_files")?
            .iter()
            .filter(|f| f["file_id"] == scope["file_ref"])
            .collect::<Vec<_>>();
        if matches.len() != 1 || matches[0]["sha256"] != scope["file_sha256"] {
            return Err(Error::Invalid(
                "native text File unique Item manifest binding",
            ));
        }
        Ok(manifest)
    }
    fn resolve(&mut self, binding: &Value) -> Result<()> {
        self.schema(
            binding,
            "ToS/contracts/native-text-unit-binding.schema.json",
        )?;
        let packet_ref = text(binding, "packet_ref")?;
        let packet = self.record(
            packet_ref,
            Some(text(binding, "packet_sha256")?),
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
        )?;
        if packet["content_posture"] != "source_bound"
            || packet["packet_id"] != binding["packet_id"]
            || packet["packet_version"] != binding["packet_version"]
        {
            return Err(Error::Invalid("native text packet exact identity/posture"));
        }
        let target = &binding["text_layer"];
        let layer_ref = text(target, "record_ref")?;
        if packet["source_layer"]["text_layer_ref"] != target["record_ref"] {
            return Err(Error::Invalid("native text packet layer locator"));
        }
        let layer = self.record(
            layer_ref,
            Some(text(target, "record_sha256")?),
            "ToS/contracts/source-text-layer.schema.json",
        )?;
        if layer["layer_id"] != target["layer_id"]
            || layer["layer_version"] != target["layer_version"]
        {
            return Err(Error::Invalid("native text layer exact identity/version"));
        }
        metadata(
            &self.read(packet_ref, None, false)?,
            packet_ref,
            self.validator,
            self.deadline,
            MetadataKind::Packet,
        )?;
        self.layer_dependencies(&layer, layer_ref, &BTreeSet::new())?;
        let manifestation = self.source_scope(binding, &packet["source_scope"], &layer)?;
        let rep = &layer["representation"];
        let declared = &packet["source_layer"];
        let form = if rep["character_normalization"] == "none" {
            Value::String("source_preserved".into())
        } else {
            rep["character_normalization"].clone()
        };
        if declared["text_layer_sha256"] != rep["content_sha256"]
            || declared["language"] != rep["language"]
            || declared["unicode_form"] != form
            || declared["visibility"] != rep["content_visibility"]
            || declared["publication_authorized"] != rep["publication_authorized"]
            || !matches!(
                rep["media_type"].as_str(),
                Some("text/plain" | "text/plain; charset=utf-8")
            )
        {
            return Err(Error::Invalid(
                "native text packet/layer representation declarations",
            ));
        }
        let start = rep["text_scope"]["start"]
            .as_u64()
            .ok_or(Error::Invalid("native text layer scope start"))?;
        let end = rep["text_scope"]["end"]
            .as_u64()
            .ok_or(Error::Invalid("native text layer scope end"))?;
        for anchor in array(&packet, "anchors")? {
            let a = anchor["selector"]["start"]
                .as_u64()
                .ok_or(Error::Invalid("native text unit anchor start"))?;
            let b = anchor["selector"]["end"]
                .as_u64()
                .ok_or(Error::Invalid("native text unit anchor end"))?;
            if anchor["source_return"]["locator_ref"] != rep["content_ref"]
                || !(start <= a && a <= b && b <= end)
            {
                return Err(Error::Invalid(
                    "native text unit anchor representation scope",
                ));
            }
        }
        for target in array(&layer["source_binding"], "anchors")? {
            let reference = text(target, "anchor_record_ref")?;
            let anchor = self.record(
                reference,
                Some(text(target, "anchor_record_sha256")?),
                "ToS/contracts/source-anchor-v2.schema.json",
            )?;
            if anchor["anchor_id"] != target["anchor_id"]
                || anchor["target"]["item_id"] != packet["source_scope"]["item_ref"]
                || anchor["target"]["file_id"] != packet["source_scope"]["file_ref"]
                || anchor["target"]["file_sha256"] != packet["source_scope"]["file_sha256"]
            {
                return Err(Error::Invalid("native text source anchor metadata binding"));
            }
            metadata(
                &self.read(reference, None, false)?,
                reference,
                self.validator,
                self.deadline,
                MetadataKind::Anchor,
            )?;
        }
        let rights = &packet["rights_and_visibility"];
        let packet_refs = array(rights, "rights_record_refs")?
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        let layer_refs = array(rep, "rights_record_refs")?
            .iter()
            .map(|r| text(r, "ref"))
            .collect::<Result<BTreeSet<_>>>()?;
        if layer_refs.is_empty()
            || layer_refs != packet_refs
            || !layer_refs.contains(text(&manifestation, "rights_ref")?)
        {
            return Err(Error::Invalid(
                "native text exact packet/layer rights closure",
            ));
        }
        let mut relevant = packet["source_scope"]
            .as_object()
            .ok_or(Error::Invalid("native text source scope object"))?
            .iter()
            .filter(|(k, _)| k.ends_with("_ref"))
            .filter_map(|(_, v)| v.as_str())
            .collect::<BTreeSet<_>>();
        relevant.insert(text(&layer, "layer_id")?);
        relevant.insert(text(rep, "content_file_id")?);
        let mut records = Vec::new();
        for target in array(rep, "rights_record_refs")? {
            let record = self.record(
                text(target, "ref")?,
                Some(text(target, "sha256")?),
                "ToS/contracts/rights-record.schema.json",
            )?;
            let scopes = array(&record, "scope_refs")?
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>();
            if relevant.is_disjoint(&scopes)
                || target["ref"] == manifestation["rights_ref"]
                    && (!scopes.contains(text(&packet["source_scope"], "item_ref")?)
                        || !scopes.contains(text(&packet["source_scope"], "file_ref")?))
            {
                return Err(Error::Invalid("native text rights scope differs"));
            }
            records.push(record);
        }
        for target in array(rep, "publication_authority_refs")? {
            self.read(text(target, "ref")?, Some(text(target, "sha256")?), true)?;
        }
        let units = array(&packet, "units")?
            .iter()
            .filter(|u| u["unit_id"] == binding["unit_id"])
            .collect::<Vec<_>>();
        let segmentations = array(&packet, "segmentations")?
            .iter()
            .filter(|s| s["segmentation_id"] == binding["segmentation_id"])
            .collect::<Vec<_>>();
        if units.len() != 1 || segmentations.len() != 1 {
            return Err(Error::Invalid("native text unique unit/segmentation"));
        }
        let unit = units[0];
        let segmentation = segmentations[0];
        if unit["unit_version"] != binding["unit_version"]
            || unit["ordered_anchor_refs"] != binding["ordered_anchor_refs"]
            || unit["surface_posture"] != "source_bearing"
            || segmentation["segmentation_version"] != binding["segmentation_version"]
            || !array(segmentation, "ordered_unit_refs")?.contains(&unit["unit_id"])
        {
            return Err(Error::Invalid("native text exact unit membership/version"));
        }
        let public = rep["content_visibility"] == "public"
            && rep["publication_authorized"] == true
            && rights["packet_visibility"] == "public"
            && rights["effective_visibility"] == "public"
            && rights["publication_authorized"] == true
            && rights["private_source_used"] == false;
        if !public {
            return Err(Error::Invalid(
                "native text public description cannot disclose private binding",
            ));
        }
        let exact = [text(&layer, "layer_id")?, text(rep, "content_file_id")?]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let selected = records
            .iter()
            .filter(|r| {
                r["scope_refs"].as_array().is_some_and(|s| {
                    s.iter()
                        .any(|v| v.as_str().is_some_and(|v| exact.contains(v)))
                })
            })
            .collect::<Vec<_>>();
        let applicable = if selected.is_empty() {
            records.iter().collect::<Vec<_>>()
        } else {
            selected
        };
        for record in applicable {
            if !matches!(
                record["assessment_status"].as_str(),
                Some("public_domain_reviewed" | "licensed" | "permission_granted")
            ) || record["visibility"] != "public_payload"
                || !matches!(
                    record["redistribution_posture"].as_str(),
                    Some("authorized" | "authorized_with_conditions")
                )
                || !matches!(
                    record["derivative_posture"].as_str(),
                    Some("allowed" | "allowed_with_conditions")
                )
                || matches!(
                    record["review_status"].as_str(),
                    Some("legal_review_requested" | "superseded")
                )
            {
                return Err(Error::Invalid(
                    "native text recorded public gate contradicts declaration",
                ));
            }
        }
        Self::path(text(rep, "content_ref")?, false, true)?;
        // No content read occurs. The sealed source cut, exact byte hashes and
        // private catalog receipt retain currentness; no assessment is applied.
        Ok(())
    }
}

pub(crate) fn resolve_native_text_binding(
    stage: &KnowledgeStage<'_>,
    binding: &Value,
    validator: &SourceCatalogValidator<'_>,
    limits: SourceCatalogLimits,
) -> Result<()> {
    Resolver {
        stage,
        validator,
        limits,
        files: BTreeMap::new(),
        total: 0,
        deadline: std::time::Instant::now()
            .checked_add(validator.budget.execution_wall)
            .ok_or(Error::Budget("native text deadline arithmetic"))?,
    }
    .resolve(binding)
}
enum MetadataKind {
    Packet,
    Layer,
    Anchor,
}
fn metadata(
    raw: &[u8],
    path: &str,
    validator: &SourceCatalogValidator<'_>,
    deadline: std::time::Instant,
    kind: MetadataKind,
) -> Result<()> {
    use tos_validation::text_metadata_rules::{
        TextMetadataLimits, TextMetadataState, inspect_source_anchor_v2_metadata,
        inspect_source_text_layer_metadata, inspect_source_text_unit_v1_metadata,
    };
    let limits = TextMetadataLimits {
        max_packet_bytes: 1_048_576,
        max_state_bytes: 8_388_608,
        max_issues: 4096,
        deadline,
    };
    let result = match kind {
        MetadataKind::Packet => {
            inspect_source_text_unit_v1_metadata(raw, path, limits, validator.cancelled)
        }
        MetadataKind::Layer => {
            inspect_source_text_layer_metadata(raw, path, limits, validator.cancelled)
        }
        MetadataKind::Anchor => {
            inspect_source_anchor_v2_metadata(raw, path, limits, validator.cancelled)
        }
    }
    .map_err(|_| Error::Budget("native text metadata predicate execution"))?;
    if result.state != TextMetadataState::CheckedMetadata
        || !result.issues.is_empty()
        || result.packet_digest != Digest256::of_bytes(raw).to_hex()
    {
        return Err(Error::Invalid("native text metadata owner predicates"));
    }
    Ok(())
}
