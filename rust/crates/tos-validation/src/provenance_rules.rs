//! Executable provenance-v2 cross-field rules and the source-owned A/B/C lab.
//! Current schema authority and recorded original-input fixity are separate.
//! A family report has no source admission, rights, review or truth authority.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use serde_json::{Value, json};
use tos_foundation::{Digest256, RelativePath};
use unicode_normalization::UnicodeNormalization;

use crate::datetime_support::{ObservedDateTimeError, observed_datetime_raw_order};
use crate::item_rules::{ItemIssue, ItemLimits, ItemRefusal};

pub const CONTRACT: &str = "ToS/contracts/provenance-event-v2.schema.json";
pub const LAB_MANIFEST: &str =
    "ToS/research-packets/foundation-laboratory-2026-07/provenance-event-v2-abc/lab.manifest.json";

/// The historical inline Python invocation and native digest-only invocation
/// are distinct recorded forms. Neither establishes execution truth.
pub fn lab_command_matches(builder: &Value, event: &Value, row: &Value, id: &str) -> bool {
    let command = &event["method"]["command_capture"];
    match builder["ref"].as_str() {
        Some("scripts/build_provenance_event_v2_lab.py") => {
            let expected = json!([
                "python",
                "scripts/build_provenance_event_v2_lab.py",
                "--variant",
                id
            ]);
            command["argv"] == expected && row["captured_command"] == expected
        }
        Some("rust/crates/tos-compiler/src/provenance_event_lab.rs") => {
            let operation = match id {
                "A" => "identity-copy",
                "B" => "unicode-nfc",
                "C" => "ascii-strict-negative-control",
                _ => return false,
            };
            command["disclosure"] == "withheld_digest_only"
                && command["argv"].is_null()
                && row["captured_command"].is_null()
                && command["argv_sha256"]
                    .as_str()
                    .is_some_and(|d| d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()))
                && command["argv_sha256"] == row["captured_command_sha256"]
                && command["withholding_reason"]
                    .as_str()
                    .is_some_and(|r| !r.is_empty())
                && event["method"]["procedure"]["name"] == operation
                && event["responsibility"][0]["evidence_binding"] == *builder
                && event["method"]["software_components"][0]["artifact_ref"] == builder["ref"]
                && event["method"]["software_components"][0]["artifact_sha256"] == builder["sha256"]
                && event["method"]["environment"]["runtime"] == "Rust native executable"
                && event["method"]["environment"]["backend"] == "rust-unicode-normalization"
                && event["reproducibility"]["classification"] == "partially_specified"
        }
        _ => false,
    }
}

/// Reads use a single pinned current namespace, with no ambient checkout or
/// payload fallback. `recorded_input` resolves the exact original ref+digest
/// through the owner's current/retained original-path or named archive route;
/// its result must not become the current schema used by `schema`. Named
/// schema archives must retain the original schema `$id`; a digest match in
/// an arbitrary retained revision is not the owner's archive resolution law.
/// The schema executor consumes only the exact supplied current schema digest.
/// Reader and worker implementations enforce cancellation and the deadline
/// during I/O, not merely before returning. This trait cannot interrupt an
/// arbitrary blocking implementation; actual adapters need cooperative reads
/// and process custody for worker deadlines. None may grant publication rights.
pub trait ProvenanceSource {
    fn current(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal>;
    fn recorded_input(
        &mut self,
        path: &str,
        digest: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal>;
    fn schema(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        contract_digest: &str,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceReport {
    pub issues: Vec<ItemIssue>,
    pub event_ids: BTreeSet<String>,
    pub negative_controls: BTreeMap<String, bool>,
    pub metadata_bytes: u64,
    pub current_contract_sha256: Option<String>,
    pub source_admission_complete: bool,
}

pub struct ProvenanceRules {
    limits: ItemLimits,
    issues: Vec<ItemIssue>,
    event_owners: BTreeMap<String, (String, String)>,
    negative_controls: BTreeMap<String, bool>,
    metadata_bytes: u64,
    state_bytes: usize,
    contract_digest: Option<String>,
}

impl ProvenanceRules {
    pub fn new(limits: ItemLimits) -> Self {
        Self {
            limits,
            issues: Vec::new(),
            event_owners: BTreeMap::new(),
            negative_controls: BTreeMap::new(),
            metadata_bytes: 0,
            state_bytes: 0,
            contract_digest: None,
        }
    }
    fn check(&self) -> Result<(), ItemRefusal> {
        if Instant::now() >= self.limits.deadline {
            return Err(ItemRefusal::Deadline);
        }
        Ok(())
    }
    fn reserve(&mut self, amount: usize) -> Result<(), ItemRefusal> {
        self.state_bytes = self
            .state_bytes
            .checked_add(amount)
            .filter(|n| *n <= self.limits.max_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }
    fn issue(&mut self, path: &str, code: &'static str) -> Result<(), ItemRefusal> {
        self.check()?;
        if self.issues.len() >= self.limits.max_issues {
            return Err(ItemRefusal::Budget);
        }
        self.reserve(path.len() + code.len())?;
        self.issues.push(ItemIssue {
            path: path.into(),
            code,
        });
        Ok(())
    }
    fn charge(&mut self, raw: &[u8]) -> Result<(), ItemRefusal> {
        self.check()?;
        if raw.len() > self.limits.max_member_bytes {
            return Err(ItemRefusal::Budget);
        }
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(raw.len() as u64)
            .filter(|n| *n <= self.limits.max_total_bytes)
            .ok_or(ItemRefusal::Budget)?;
        // Weighted logical work/state accounting, not an allocation or RSS
        // envelope. The finite member ceiling and the parent's process quota
        // are separate gates. Persistent IDs/issues are charged separately.
        if raw
            .len()
            .checked_mul(8)
            .and_then(|n| n.checked_add(self.state_bytes))
            .is_none_or(|n| n > self.limits.max_state_bytes)
        {
            return Err(ItemRefusal::Budget);
        }
        Ok(())
    }
    fn raw(
        &mut self,
        source: &mut impl ProvenanceSource,
        path: &str,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("unsafe provenance source path".into()))?;
        self.check()?;
        let raw = source.current(path, self.limits.max_member_bytes, self.limits.deadline)?;
        if let Some(raw) = &raw {
            self.charge(raw)?;
        }
        Ok(raw)
    }
    fn object(
        &mut self,
        source: &mut impl ProvenanceSource,
        path: &str,
    ) -> Result<Option<(Value, Vec<u8>)>, ItemRefusal> {
        let Some(raw) = self.raw(source, path)? else {
            self.issue(path, "missing-provenance-companion")?;
            return Ok(None);
        };
        let value = serde_json::from_slice::<Value>(&raw)
            .map_err(|_| ItemRefusal::Source(format!("invalid provenance JSON: {path}")))?;
        if !value.is_object() {
            self.issue(path, "provenance-object-required")?;
            return Ok(None);
        }
        Ok(Some((value, raw)))
    }
    fn contract(&mut self, source: &mut impl ProvenanceSource) -> Result<String, ItemRefusal> {
        if let Some(digest) = &self.contract_digest {
            return Ok(digest.clone());
        }
        let raw = self.raw(source, CONTRACT)?.ok_or_else(|| {
            ItemRefusal::Unsupported(
                "current provenance-v2 contract absent from pinned source".into(),
            )
        })?;
        let value: Value = serde_json::from_slice(&raw)
            .map_err(|_| ItemRefusal::Source("current provenance schema JSON".into()))?;
        if value["$id"] != format!("https://tree-of-sophia.local/{CONTRACT}") {
            return Err(ItemRefusal::Unsupported(
                "current provenance contract identity".into(),
            ));
        }
        let digest = Digest256::of_bytes(&raw).to_hex();
        self.reserve(digest.len())?;
        self.contract_digest = Some(digest.clone());
        Ok(digest)
    }
    /// Call for every current v2 event selected by the source-owner route.
    /// Duplicate IDs are scoped to this executor's complete invocation union.
    pub fn inspect_event(
        &mut self,
        source: &mut impl ProvenanceSource,
        path: &str,
        raw: &[u8],
    ) -> Result<Value, ItemRefusal> {
        self.charge(raw)?;
        let digest = self.contract(source)?;
        if !source.schema(path, raw, CONTRACT, &digest, self.limits.deadline)? {
            self.issue(path, "provenance-v2-schema")?;
        }
        let event = crate::native_decoded_value(raw, self.limits.max_member_bytes)?;
        if !event.is_object() {
            self.issue(path, "provenance-object-required")?;
            return Ok(event);
        }
        if let Some(id) = event["event_id"].as_str() {
            let owner = (path.to_owned(), Digest256::of_bytes(raw).to_hex());
            if self
                .event_owners
                .get(id)
                .is_some_and(|existing| existing != &owner)
            {
                self.issue(path, "duplicate-provenance-v2-event-id")?;
            } else if !self.event_owners.contains_key(id) {
                self.reserve(id.len() + owner.0.len() + owner.1.len())?;
                self.event_owners.insert(id.into(), owner);
            }
        }
        for code in semantic_issues(&event, self.limits.max_issues, self.limits.deadline)? {
            self.issue(path, code)?;
        }
        Ok(event)
    }
    fn binding(
        &mut self,
        source: &mut impl ProvenanceSource,
        binding: &Value,
        historical: bool,
    ) -> Result<bool, ItemRefusal> {
        let (Some(path), Some(digest)) = (binding["ref"].as_str(), binding["sha256"].as_str())
        else {
            return Ok(false);
        };
        RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("unsafe provenance binding path".into()))?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Ok(false);
        }
        let raw = if historical {
            let raw = source.recorded_input(
                path,
                digest,
                self.limits.max_member_bytes.min(1_048_576),
                self.limits.deadline,
            )?;
            if let Some(raw) = &raw {
                self.charge(raw)?;
            }
            raw
        } else {
            self.raw(source, path)?
        };
        Ok(raw.is_some_and(|raw| Digest256::of_bytes(&raw).to_hex() == digest))
    }
    pub fn inspect_lab(&mut self, source: &mut impl ProvenanceSource) -> Result<(), ItemRefusal> {
        self.contract(source)?;
        let Some((manifest, _)) = self.object(source, LAB_MANIFEST)? else {
            return Ok(());
        };
        if manifest["schema_version"] != "tos_provenance_event_v2_lab_v1" {
            self.issue(LAB_MANIFEST, "unexpected-provenance-v2-lab-version")?;
        }
        if manifest["authority_posture"] != "public_synthetic_mechanics_only" {
            self.issue(LAB_MANIFEST, "provenance-v2-lab-authority-posture")?;
        }
        if manifest["authority_limits"]
            != json!({"private_source_used":false,"model_invoked":false,"human_evidence_created":false,"execution_truth_established":false,"content_truth_established":false,"rights_clearance_established":false,"semantic_claim_created":false,"canon_effect":false})
        {
            self.issue(LAB_MANIFEST, "provenance-v2-lab-authority-limits")?;
        }
        for field in [
            "contract",
            "plan",
            "builder",
            "environment_profile",
            "input_fixture",
        ] {
            if !self.binding(
                source,
                &manifest[field],
                matches!(field, "contract" | "builder"),
            )? {
                self.issue(LAB_MANIFEST, "provenance-v2-lab-binding-drift")?;
            }
        }
        if manifest["contract"]["ref"] != CONTRACT {
            self.issue(LAB_MANIFEST, "provenance-v2-lab-contract-ref")?;
        }
        let variants = array(&manifest["variants"]).collect::<Vec<_>>();
        if !manifest["variants"].is_array() {
            self.issue(LAB_MANIFEST, "provenance-v2-variants-list")?;
        }
        if variants
            .iter()
            .filter(|v| v.is_object())
            .map(|v| &v["variant_id"])
            .collect::<Vec<_>>()
            != vec![&json!("A"), &json!("B"), &json!("C")]
        {
            self.issue(LAB_MANIFEST, "provenance-v2-variant-order")?;
        }
        let ids = variants
            .iter()
            .filter_map(|v| v["variant_id"].as_str())
            .collect::<BTreeSet<_>>();
        if ids.len() != variants.len() {
            self.issue(LAB_MANIFEST, "provenance-v2-variant-identity")?;
        }
        let mut events = BTreeMap::new();
        let mut frozen = BTreeMap::new();
        for variant in variants {
            self.check()?;
            if !variant.is_object() {
                self.issue(LAB_MANIFEST, "provenance-v2-variant-object")?;
                continue;
            }
            let (Some(id @ ("A" | "B" | "C")), Some(path)) = (
                variant["variant_id"].as_str(),
                variant["event_ref"].as_str(),
            ) else {
                self.issue(LAB_MANIFEST, "provenance-v2-variant-identity")?;
                continue;
            };
            frozen.insert(id, variant);
            let Some(raw) = self.raw(source, path)? else {
                self.issue(path, "provenance-event-record-fixity")?;
                continue;
            };
            if variant["event_sha256"] != Digest256::of_bytes(&raw).to_hex() {
                self.issue(path, "provenance-event-record-fixity")?;
                continue;
            }
            let event = self.inspect_event(source, path, &raw)?;
            if event["record_binding"]
                != json!({"manifest_ref":LAB_MANIFEST,"digest_algorithm":"sha256","digest_scope":"exact_event_record_bytes"})
            {
                self.issue(path, "event-manifest-record-binding")?;
            }
            if event["activity"]["status"] != variant["expected_status"]
                || event["activity"]["exit_code"] != variant["expected_exit_code"]
            {
                self.issue(path, "event-terminal-expectation")?;
            }
            if !lab_command_matches(&manifest["builder"], &event, variant, id) {
                self.issue(path, "lab-captured-argv")?;
            }
            for (name, group) in [
                ("input", "inputs"),
                ("output", "outputs"),
                ("byproduct", "byproducts"),
            ] {
                let reference = &variant[format!("{name}_ref")];
                let digest = &variant[format!("{name}_sha256")];
                let entries = array(&event["entities"][group]).collect::<Vec<_>>();
                if let Some(reference) = reference.as_str() {
                    let bound = json!({"ref":reference,"sha256":digest});
                    if !self.binding(source, &bound, false)? {
                        self.issue(path, "variant-entity-fixity")?;
                    }
                    if entries.len() != 1
                        || entries[0]["entity_ref"] != reference
                        || entries[0]["sha256"] != *digest
                    {
                        self.issue(path, "variant-event-entity-binding")?;
                    }
                } else if name == "input" || !entries.is_empty() {
                    self.issue(path, "variant-event-entity-binding")?;
                }
            }
            let relations = array(&event["derivations"])
                .map(|d| d["relation"].clone())
                .collect::<Vec<_>>();
            let expected = if variant["expected_relation"].is_null() {
                vec![]
            } else {
                vec![variant["expected_relation"].clone()]
            };
            if relations != expected {
                self.issue(path, "lab-derivation-relation")?;
            }
            if event["method"]["model_invocations"] != json!([]) {
                self.issue(path, "synthetic-lab-model-invoked")?;
            }
            if array(&event["responsibility"]).any(|a| a["agent_kind"] == "human") {
                self.issue(path, "synthetic-lab-human-evidence")?;
            }
            if event["review_and_authority"]["human_review_status"] != "not_performed" {
                self.issue(path, "synthetic-lab-human-review")?;
            }
            if event["rights_and_visibility"]["publication_authorized"] != false {
                self.issue(path, "synthetic-lab-publication-authority")?;
            }
            // Three events and their mutations occupy bounded transient state.
            self.reserve(raw.len().saturating_mul(8))?;
            events.insert(id, event);
        }
        if events.len() == 3 && frozen.len() == 3 {
            let input = self.required_bytes(source, manifest["input_fixture"]["ref"].as_str())?;
            let a = self.required_bytes(source, frozen["A"]["output_ref"].as_str())?;
            let b = self.required_bytes(source, frozen["B"]["output_ref"].as_str())?;
            let c = self.required_bytes(source, frozen["C"]["byproduct_ref"].as_str())?;
            if a != input {
                self.issue(LAB_MANIFEST, "variant-a-not-byte-copy")?;
            }
            match (std::str::from_utf8(&input), std::str::from_utf8(&b)) {
                (Ok(input_text), Ok(b_text)) => {
                    if !input_text.nfc().eq(b_text.chars()) || b == input {
                        self.issue(LAB_MANIFEST, "variant-b-not-exact-nfc")?;
                    }
                }
                _ => self.issue(LAB_MANIFEST, "synthetic-text-not-utf8")?,
            }
            if c != b"status=failed\nreason=non_ascii_input\nexit_code=7\n" {
                self.issue(LAB_MANIFEST, "variant-c-failure-byproduct")?;
            }
            self.negative_controls(source, &manifest, &events, &frozen)?;
        }
        let declared = array(&manifest["negative_controls"])
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        let tested = self
            .negative_controls
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if declared != tested {
            self.issue(LAB_MANIFEST, "negative-control-coverage")?;
        }
        Ok(())
    }
    fn required_bytes(
        &mut self,
        source: &mut impl ProvenanceSource,
        path: Option<&str>,
    ) -> Result<Vec<u8>, ItemRefusal> {
        let path = path.ok_or_else(|| ItemRefusal::Source("lab byte reference absent".into()))?;
        self.raw(source, path)?
            .ok_or_else(|| ItemRefusal::Source(format!("lab bytes absent: {path}")))
    }
    fn negative_controls(
        &mut self,
        source: &mut impl ProvenanceSource,
        manifest: &Value,
        events: &BTreeMap<&str, Value>,
        frozen: &BTreeMap<&str, &Value>,
    ) -> Result<(), ItemRefusal> {
        let a = &events["A"];
        let c = &events["C"];
        let zero = "0".repeat(64);
        let raw = self.required_bytes(source, frozen["A"]["event_ref"].as_str())?;
        self.control(
            "event_record_digest_drift",
            Digest256::of_bytes(&raw).to_hex() != zero,
        )?;
        let raw = self.required_bytes(source, a["entities"]["inputs"][0]["entity_ref"].as_str())?;
        self.control(
            "input_fixity_drift",
            Digest256::of_bytes(&raw).to_hex() != zero,
        )?;
        let cases = [
            (
                "command_digest_drift",
                "inline command argv digest drifted",
                0,
            ),
            (
                "identity_copy_content_drift",
                "identity copy does not preserve exact bytes and size",
                1,
            ),
            (
                "derivation_endpoint_escape",
                "derivation output leaves the authoritative output entities",
                2,
            ),
            (
                "replay_ready_with_withheld_command",
                "replay-ready provenance withholds its command",
                3,
            ),
            (
                "unsigned_claimed_signature_verification",
                "unsigned provenance event claims signature verification",
                4,
            ),
            (
                "self_supersession",
                "provenance event cannot supersede itself",
                5,
            ),
        ];
        for (name, expected, mutation) in cases {
            let mut changed = if mutation == 2 {
                events["B"].clone()
            } else {
                a.clone()
            };
            match mutation {
                0 => {
                    changed["method"]["command_capture"] = json!({"disclosure":"inline","argv":["synthetic-negative-command"],"argv_sha256":zero,"withholding_reason":null});
                }
                1 => changed["entities"]["outputs"][0]["sha256"] = json!(zero),
                2 => changed["derivations"][0]["output_entity_ref"] = json!("outside:event-output"),
                3 => {
                    changed["reproducibility"]["classification"] = json!("replay_ready");
                    changed["method"]["command_capture"]["disclosure"] =
                        json!("withheld_digest_only");
                    changed["method"]["command_capture"]["argv"] = Value::Null;
                    changed["method"]["command_capture"]["withholding_reason"] =
                        json!("synthetic negative");
                }
                4 => {
                    changed["evidence_authentication"]["verification_status"] =
                        json!("signature_verified")
                }
                _ => changed["supersedes_event_ref"] = changed["event_id"].clone(),
            }
            self.control(
                name,
                semantic_issues(&changed, self.limits.max_issues, self.limits.deadline)?
                    .contains(&expected),
            )?;
        }
        for name in [
            "completed_without_output",
            "failed_with_authoritative_output",
            "model_event_without_invocation",
            "publication_without_authority",
            "unattested_human_review",
            "manual_change_without_receipt",
        ] {
            let mut changed = if name == "failed_with_authoritative_output" {
                c.clone()
            } else {
                a.clone()
            };
            match name {
                "completed_without_output" => {
                    changed["entities"]["outputs"] = json!([]);
                    changed["derivations"] = json!([]);
                }
                "failed_with_authoritative_output" => {
                    changed["entities"]["outputs"] = a["entities"]["outputs"].clone()
                }
                "model_event_without_invocation" => {
                    changed["activity"]["event_type"] = json!("model_inference")
                }
                "publication_without_authority" => {
                    changed["rights_and_visibility"]["publication_authorized"] = json!(true)
                }
                "unattested_human_review" => {
                    changed["review_and_authority"]["human_review_status"] = json!("performed");
                    changed["review_and_authority"]["review_bindings"] = json!([manifest["plan"]]);
                }
                _ => changed["manual_changes"]["status"] = json!("recorded"),
            }
            let raw = serde_json::to_vec(&changed)
                .map_err(|_| ItemRefusal::Source("negative control JSON".into()))?;
            self.charge(&raw)?;
            let digest = self.contract(source)?;
            let rejected =
                !source.schema(LAB_MANIFEST, &raw, CONTRACT, &digest, self.limits.deadline)?;
            self.control(name, rejected)?;
        }
        Ok(())
    }
    fn control(&mut self, name: &str, rejected: bool) -> Result<(), ItemRefusal> {
        self.check()?;
        self.reserve(name.len())?;
        self.negative_controls.insert(name.into(), rejected);
        if !rejected {
            self.issue(LAB_MANIFEST, "negative-control-not-rejected")?;
        }
        Ok(())
    }
    pub fn finish(self) -> ProvenanceReport {
        ProvenanceReport {
            issues: self.issues,
            event_ids: self.event_owners.into_keys().collect(),
            negative_controls: self.negative_controls,
            metadata_bytes: self.metadata_bytes,
            current_contract_sha256: self.contract_digest,
            source_admission_complete: false,
        }
    }
}

fn array(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

// Actual bibliography consumer's complementary logical workspace for the
// semantic owner below. Indexes borrow existing event strings/Values; each
// possible entity/derivation contributes a named slot, not raw-byte scaling.
pub(crate) fn semantic_workspace(
    event: &Value,
    max_issues: usize,
    available: usize,
) -> Result<usize, ItemRefusal> {
    let entities =
        ["inputs", "outputs", "byproducts"]
            .into_iter()
            .try_fold(0usize, |n, group| {
                n.checked_add(
                    array(&event["entities"][group])
                        .filter(|v| v.is_object() && v["entity_ref"].as_str().is_some())
                        .count(),
                )
                .ok_or(ItemRefusal::Budget)
            })?;
    let derivations = array(&event["derivations"])
        .filter(|v| v.is_object())
        .count();
    let messages = std::mem::size_of::<Vec<&'static str>>()
        + max_issues
            .checked_mul(std::mem::size_of::<&'static str>())
            .ok_or(ItemRefusal::Budget)?;
    let indexes = std::mem::size_of::<BTreeMap<&str, BTreeSet<&str>>>()
        + ["inputs", "outputs", "byproducts"].len() * std::mem::size_of::<(&str, BTreeSet<&str>)>()
        + entities
            .checked_mul(std::mem::size_of::<&str>())
            .ok_or(ItemRefusal::Budget)?
        + std::mem::size_of::<BTreeMap<&str, &Value>>()
        + entities
            .checked_mul(std::mem::size_of::<(&str, &Value)>())
            .ok_or(ItemRefusal::Budget)?
        + std::mem::size_of::<BTreeSet<&str>>()
        + derivations
            .checked_mul(std::mem::size_of::<&str>())
            .ok_or(ItemRefusal::Budget)?;
    let base = messages
        .checked_add(indexes)
        .and_then(|n| n.checked_add(std::mem::size_of::<tos_foundation::Digest256Hasher>()))
        .and_then(|n| {
            n.checked_add(std::mem::size_of::<String>() + std::mem::size_of::<Digest256>() * 2)
        })
        .ok_or(ItemRefusal::Budget)?;
    let command = &event["method"]["command_capture"];
    let extra = if command["disclosure"] == "inline" && command["argv"].is_array() {
        let header = std::mem::size_of::<Vec<u8>>();
        let remaining = available
            .checked_sub(base)
            .and_then(|n| n.checked_sub(header))
            .ok_or(ItemRefusal::Budget)?;
        header
            .checked_add(crate::validation_codec::decoded_wire_size(
                &command["argv"],
                remaining,
            )?)
            .ok_or(ItemRefusal::Budget)?
    } else {
        0
    };
    let cost = base.checked_add(extra).ok_or(ItemRefusal::Budget)?;
    if cost > available {
        return Err(ItemRefusal::BudgetCheck {
            check: "bibliography provenance semantic workspace",
            used: Some(cost as u64),
            limit: Some(available as u64),
        });
    }
    Ok(cost)
}

/// Exact cross-field message vocabulary from the source Python owner. The
/// shared chronology parser follows direct Python fromisoformat here; unlike
/// retirement, this source family does not first replace every literal Z.
pub fn semantic_issues(
    event: &Value,
    max_issues: usize,
    deadline: Instant,
) -> Result<Vec<&'static str>, ItemRefusal> {
    let mut messages = Vec::new();
    let mut push = |message| -> Result<(), ItemRefusal> {
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        if messages.len() >= max_issues {
            return Err(ItemRefusal::Budget);
        }
        messages.push(message);
        Ok(())
    };
    if event["supersedes_event_ref"] == event["event_id"] {
        push("provenance event cannot supersede itself")?;
    }
    let activity = &event["activity"];
    match (
        activity["started_at"].as_str(),
        activity["ended_at"].as_str(),
    ) {
        (Some(start), Some(end)) => match observed_datetime_raw_order(start, end) {
            Ok(std::cmp::Ordering::Greater) => push("provenance activity ends before it starts")?,
            Ok(_) => {}
            Err(ObservedDateTimeError::Invalid) => {
                push("provenance activity timestamps are not comparable")?
            }
            Err(ObservedDateTimeError::Budget) => return Err(ItemRefusal::Budget),
        },
        _ => push("provenance activity timestamps are not comparable")?,
    }
    let status = activity["status"].as_str();
    let code = &activity["exit_code"];
    if matches!(status, Some("completed" | "completed_with_warnings")) {
        if !python_zero(code) {
            push("completed provenance activity has a non-zero or absent exit code")?;
        }
        if !activity["terminal_reason"].is_null() {
            push("completed provenance activity carries a terminal failure reason")?;
        }
    }
    if matches!(status, Some("failed" | "stopped")) {
        if code.is_null() || python_zero(code) {
            push("failed or stopped provenance activity lacks a non-zero exit code")?;
        }
        if !activity["terminal_reason"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
        {
            push("failed or stopped provenance activity lacks a terminal reason")?;
        }
    }
    let mut refs = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut entities = BTreeMap::<&str, &Value>::new();
    for group in ["inputs", "outputs", "byproducts"] {
        let mut seen = BTreeSet::new();
        let mut count = 0;
        for entity in array(&event["entities"][group]).filter(|v| v.is_object()) {
            if let Some(reference) = entity["entity_ref"].as_str() {
                count += 1;
                seen.insert(reference);
                entities.insert(reference, entity);
            }
            if entity["fixity_verified"] == true && entity["fixity_verified_at"].is_null() {
                push(match group {
                    "inputs" => "inputs entity claims fixity without verification time",
                    "outputs" => "outputs entity claims fixity without verification time",
                    _ => "byproducts entity claims fixity without verification time",
                })?;
            }
            if entity["fixity_verified"] == false && !entity["fixity_verified_at"].is_null() {
                push(match group {
                    "inputs" => "inputs entity has verification time while fixity is false",
                    "outputs" => "outputs entity has verification time while fixity is false",
                    _ => "byproducts entity has verification time while fixity is false",
                })?;
            }
        }
        if count != seen.len() {
            push(match group {
                "inputs" => "duplicate entity reference inside inputs",
                "outputs" => "duplicate entity reference inside outputs",
                _ => "duplicate entity reference inside byproducts",
            })?;
        }
        refs.insert(group, seen);
    }
    if !refs["inputs"].is_disjoint(&refs["outputs"]) {
        push("input and output entity identities collapse")?;
    }
    if !refs["outputs"].is_disjoint(&refs["byproducts"]) {
        push("authoritative output and byproduct identities collapse")?;
    }
    let mut covered = BTreeSet::new();
    for d in array(&event["derivations"]).filter(|v| v.is_object()) {
        let input = d["input_entity_ref"].as_str();
        let output = d["output_entity_ref"].as_str();
        if !input.is_some_and(|v| refs["inputs"].contains(v)) {
            push("derivation input leaves the declared input entities")?;
        }
        if !output.is_some_and(|v| refs["outputs"].contains(v)) {
            push("derivation output leaves the authoritative output entities")?;
        } else {
            covered.insert(output.unwrap());
        }
        if d["input_entity_ref"] == d["output_entity_ref"] {
            push("derivation collapses input and output identity")?;
        }
        if d["relation"] == "identity_copy_of" {
            let absent = Value::Null;
            let input = input
                .and_then(|v| entities.get(v).copied())
                .unwrap_or(&absent);
            let output = output
                .and_then(|v| entities.get(v).copied())
                .unwrap_or(&absent);
            if input["sha256"] != output["sha256"] || input["size_bytes"] != output["size_bytes"] {
                push("identity copy does not preserve exact bytes and size")?;
            }
        }
    }
    if !refs["outputs"].is_subset(&covered) {
        push("authoritative output lacks an explicit derivation")?;
    }
    if refs["outputs"].is_empty() && truthy(&event["derivations"]) {
        push("event without authoritative output still asserts derivation")?;
    }
    let method = &event["method"];
    let command = &method["command_capture"];
    if command["disclosure"] == "inline" && command["argv"].is_array() {
        let raw = serde_json::to_vec(&command["argv"])
            .map_err(|_| ItemRefusal::Source("argv JSON".into()))?;
        if command["argv_sha256"] != Digest256::of_bytes(&raw).to_hex() {
            push("inline command argv digest drifted")?;
        }
    }
    for invocation in array(&method["model_invocations"]).filter(|v| v.is_object()) {
        let prompt = &invocation["prompt_capture"];
        if prompt["disclosure"] == "inline" {
            if let Some(text) = prompt["text"].as_str() {
                if prompt["sha256"] != Digest256::of_bytes(text.as_bytes()).to_hex() {
                    push("inline model prompt digest drifted")?;
                }
            }
        }
        if invocation["response_status"] == "completed"
            && !array(&event["entities"]["outputs"])
                .filter(|v| v.is_object())
                .any(|v| v["sha256"] == invocation["output_sha256"])
        {
            push("completed model invocation output is not an event output")?;
        }
    }
    let auth = &event["evidence_authentication"];
    if auth["signature_status"] == "unsigned" {
        if truthy(&auth["signature_bindings"]) {
            push("unsigned provenance event carries signature bindings")?;
        }
        if auth["verification_status"] == "signature_verified" {
            push("unsigned provenance event claims signature verification")?;
        }
    }
    if matches!(
        auth["signature_status"].as_str(),
        Some("signed_unverified" | "signed_verified")
    ) && !truthy(&auth["signature_bindings"])
    {
        push("signed provenance event lacks signature bindings")?;
    }
    if matches!(
        event["reproducibility"]["classification"].as_str(),
        Some("replay_ready" | "replay_ready_negative_control")
    ) {
        if command["disclosure"] != "inline" {
            push("replay-ready provenance withholds its command")?;
        }
        if event["manual_changes"]["status"] == "unknown" {
            push("replay-ready provenance has unknown manual changes")?;
        }
        if array(&method["software_components"])
            .filter(|v| v.is_object())
            .any(|v| v["verification_status"] != "verified")
        {
            push("replay-ready provenance has an unverified software component")?;
        }
    }
    Ok(messages)
}
fn python_zero(v: &Value) -> bool {
    v == false || v.as_f64() == Some(0.0)
}
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(_) => !python_zero(v),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FormatProfile, SchemaBackendProbe, SchemaResource};
    use std::time::Duration;

    /// The existing source laboratory is the oracle surface; this test adds
    /// no synthetic registry, corpus material, runner or parallel harness.
    struct LabFixture {
        members: BTreeMap<String, Vec<u8>>,
        schemas: SchemaBackendProbe,
        schema_digest: String,
    }
    impl LabFixture {
        fn new() -> Self {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
            let mut members = BTreeMap::new();
            let lab = LAB_MANIFEST.rsplit_once('/').unwrap().0;
            for entry in std::fs::read_dir(root.join(lab)).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_file() {
                    members.insert(
                        format!("{lab}/{}", entry.file_name().to_str().unwrap()),
                        std::fs::read(entry.path()).unwrap(),
                    );
                }
            }
            let manifest: Value = serde_json::from_slice(&members[LAB_MANIFEST]).unwrap();
            for field in ["contract", "builder"] {
                let path = manifest[field]["ref"].as_str().unwrap();
                let digest = manifest[field]["sha256"].as_str().unwrap();
                let raw = match std::fs::read(root.join(path)) {
                    Ok(raw) => Some(raw),
                    Err(error)
                        if field == "builder" && error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        None
                    }
                    Err(error) => panic!("read current lab {field} {path}: {error}"),
                };
                if let Some(raw) = &raw {
                    members.insert(path.into(), raw.clone());
                }
                if raw
                    .as_deref()
                    .map(Digest256::of_bytes)
                    .map(|value| value.to_hex())
                    .as_deref()
                    != Some(digest)
                {
                    let archive = if field == "contract" {
                        format!("ToS/contracts/history/{digest}.json")
                    } else {
                        format!(
                            "ToS/research-packets/retained-builder-inputs/build_provenance_event_v2_lab/{digest}.py"
                        )
                    };
                    members.insert(archive.clone(), std::fs::read(root.join(archive)).unwrap());
                }
            }
            let raw = members[CONTRACT].clone();
            let schema: Value = serde_json::from_slice(&raw).unwrap();
            let schemas = SchemaBackendProbe::new(
                [SchemaResource {
                    uri: schema["$id"].as_str().unwrap().into(),
                    raw: raw.clone(),
                }],
                FormatProfile::LegacyPythonObserved20260923,
            )
            .unwrap();
            Self {
                members,
                schemas,
                schema_digest: Digest256::of_bytes(&raw).to_hex(),
            }
        }
    }
    impl ProvenanceSource for LabFixture {
        fn current(
            &mut self,
            path: &str,
            max: usize,
            deadline: Instant,
        ) -> Result<Option<Vec<u8>>, ItemRefusal> {
            if Instant::now() >= deadline {
                return Err(ItemRefusal::Deadline);
            }
            let raw = self.members.get(path).cloned();
            if raw.as_ref().is_some_and(|v| v.len() > max) {
                return Err(ItemRefusal::Budget);
            }
            Ok(raw)
        }
        fn recorded_input(
            &mut self,
            path: &str,
            digest: &str,
            max: usize,
            deadline: Instant,
        ) -> Result<Option<Vec<u8>>, ItemRefusal> {
            // Retired laboratory builders remain exact historical inputs. The
            // fixture reads the retained bytes without executing the program.
            if let Some(raw) = self.current(path, max, deadline)? {
                if Digest256::of_bytes(&raw).to_hex() == digest {
                    return Ok(Some(raw));
                }
            }
            let archive = if path == CONTRACT {
                format!("ToS/contracts/history/{digest}.json")
            } else if path == "scripts/build_provenance_event_v2_lab.py" {
                format!(
                    "ToS/research-packets/retained-builder-inputs/build_provenance_event_v2_lab/{digest}.py"
                )
            } else {
                return Ok(None);
            };
            self.current(&archive, max, deadline)
        }
        fn schema(
            &mut self,
            _: &str,
            raw: &[u8],
            contract: &str,
            digest: &str,
            deadline: Instant,
        ) -> Result<bool, ItemRefusal> {
            if Instant::now() >= deadline {
                return Err(ItemRefusal::Deadline);
            }
            if contract != CONTRACT || digest != self.schema_digest {
                return Err(ItemRefusal::Unsupported("fixture schema pin".into()));
            }
            let value: Value = serde_json::from_slice(raw)
                .map_err(|_| ItemRefusal::Source("fixture JSON".into()))?;
            self.schemas
                .is_valid(&format!("https://tree-of-sophia.local/{contract}"), &value)
                .map_err(|e| ItemRefusal::Unsupported(format!("{e:?}")))
        }
    }
    fn rules() -> ProvenanceRules {
        ProvenanceRules::new(ItemLimits {
            max_member_bytes: 1_048_576,
            max_total_bytes: 16 * 1_048_576,
            max_state_bytes: 4 * 1_048_576,
            max_issues: 128,
            deadline: Instant::now() + Duration::from_secs(30),
        })
    }
    #[test]
    fn source_provenance_v2_lab_and_declared_negative_controls() {
        let mut source = LabFixture::new();
        let mut good = rules();
        good.inspect_lab(&mut source).unwrap();
        let manifest: Value = serde_json::from_slice(&source.members[LAB_MANIFEST]).unwrap();
        let a_path = manifest["variants"][0]["event_ref"].as_str().unwrap();
        let a_raw = source.members[a_path].clone();
        // General and named routes can inspect the same exact immutable record.
        good.inspect_event(&mut source, a_path, &a_raw).unwrap();
        let report = good.finish();
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        assert_eq!(report.event_ids.len(), 3);
        assert_eq!(report.negative_controls.len(), 14);
        assert!(report.negative_controls.values().all(|v| *v));
        assert!(!report.source_admission_complete);

        let mut collision = rules();
        collision
            .inspect_event(&mut source, a_path, &a_raw)
            .unwrap();
        collision
            .inspect_event(
                &mut source,
                "ToS/research-packets/duplicate-current-owner.event.v2.json",
                &a_raw,
            )
            .unwrap();
        assert!(
            collision
                .finish()
                .issues
                .iter()
                .any(|issue| issue.code == "duplicate-provenance-v2-event-id")
        );
        let b = manifest["variants"][1]["output_ref"]
            .as_str()
            .unwrap()
            .to_owned();
        source.members.insert(b, b"changed\n".to_vec());
        let mut drift = rules();
        drift.inspect_lab(&mut source).unwrap();
        assert!(
            drift
                .finish()
                .issues
                .iter()
                .any(|issue| issue.code == "variant-entity-fixity")
        );

        let deadline = Instant::now() + Duration::from_secs(1);
        assert_eq!(
            observed_datetime_raw_order(
                "2026-08-11T00:00:00.123456789+01:00",
                "2026-08-10T23:00:00.123456Z"
            )
            .unwrap(),
            std::cmp::Ordering::Equal
        );
        let mut broad = serde_json::from_slice::<Value>(&a_raw).unwrap();
        broad["activity"]["started_at"] = json!("2026W331T000000");
        broad["activity"]["ended_at"] = json!("2026W331T000001");
        assert!(semantic_issues(&broad, 128, deadline).unwrap().is_empty());
        broad["activity"]["started_at"] = json!("2026-09-14T00:00:00+01:99");
        broad["activity"]["ended_at"] = broad["activity"]["started_at"].clone();
        assert!(semantic_issues(&broad, 128, deadline).unwrap().is_empty());
        broad["activity"]["started_at"] = json!("2026-09-14Z");
        assert!(
            semantic_issues(&broad, 128, deadline)
                .unwrap()
                .contains(&"provenance activity timestamps are not comparable")
        );
    }
}
