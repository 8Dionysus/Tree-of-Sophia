//! Executable bounded source-layer family mechanics. These reports describe
//! exact owner predicates, never source admission, textual truth or rights.
//! Native JSON families retain Python's decoded-field semantics; the existing
//! text helpers retain their separate strict published profile.
use crate::text_metadata_rules::{self, TextMetadataLimits, TextMetadataReport, TextMetadataState};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
static NO_METADATA_CANCELLATION: AtomicBool = AtomicBool::new(false);
const OPENING_SENTENCE_PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-opening-sentence-alignment.plan.v1.json";
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::text_rules::{self, LayerResource, TextRuleReport, TextRuleState};
use crate::{KeyState, PredicateRead};
use serde_json::{Value, json};
use tos_foundation::{Digest256, RelativePath};

pub trait LayerFamilySource {
    fn current(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal>;
    /// Resolves only the exact current or explicitly retained bytes. Never a
    /// mutable checkout, path search, unselected revision or network lookup.
    fn recorded(
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
        deadline: Instant,
    ) -> Result<bool, ItemRefusal>;
    fn payload(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<LayerPayload, ItemRefusal> {
        let _ = (path, max_bytes, deadline);
        Ok(LayerPayload::Unavailable)
    }
    fn exists(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        Ok(self.current(path, max_bytes, deadline)?.is_some())
    }
    fn discovered_item_manifest(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        Ok(path.starts_with("ToS/source-witnesses/")
            && path.ends_with("/item.manifest.json")
            && self.exists(path, max_bytes, deadline)?)
    }
    fn cancellation(&self) -> &AtomicBool {
        &NO_METADATA_CANCELLATION
    }
    fn generation(&self) -> String;
    fn checkpoint(&self, deadline: Instant) -> Result<(), ItemRefusal>;
}
/// Observation supplied only by the explicitly selected immutable payload
/// custody owner. A declaration in the representation is not such evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerPayload {
    Unavailable,
    File {
        byte_size: u64,
        sha256: String,
        source_member: bool,
        sha1: Option<String>,
        jpeg_dimensions: Option<(u64, u64)>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerFamilyIssue {
    pub path: String,
    pub code: &'static str,
    pub subject: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerProfileGap {
    pub path: String,
    pub profile: String,
}
#[derive(Debug, Clone, Default)]
pub struct LayerFamilyReport {
    pub issues: Vec<LayerFamilyIssue>,
    pub reads: Vec<PredicateRead>,
    /// Predicate names, not a completeness claim for a record or corpus.
    pub checked_predicates: Vec<(String, String)>,
    pub unsupported: Vec<LayerProfileGap>,
    pub text_reports: Vec<TextRuleReport>,
    pub metadata_reports: Vec<TextMetadataReport>,
    pub metadata_bytes: u64,
}
pub struct LayerFamilyRules {
    limits: ItemLimits,
    state_bytes: usize,
    named_transient: usize,
    report: LayerFamilyReport,
    identities: BTreeMap<(String, String), String>,
    discovery_events: Option<BTreeMap<String, (String, Value, Vec<u8>)>>,
    boundary_events: Option<BTreeMap<String, (String, Value, Vec<u8>)>>,
    require_local_payloads: bool,
}
/// Apply only the decoded translation-alignment mapping law to an owner-supplied
/// packet. The caller retains raw-byte admission, schema validation and source
/// custody; this report grants none of those properties.
pub fn inspect_supplied_translation_alignment(
    packet: &Value,
    limits: ItemLimits,
) -> Result<LayerFamilyReport, ItemRefusal> {
    let mut rules = LayerFamilyRules::new(limits);
    rules.translation_alignment("<owner-local-native-alignment>", packet)?;
    Ok(rules.finish())
}

/// Apply the existing semantic-annotation owner predicate to an already
/// decoded packet. The caller retains source reads and schedules full schema
/// diagnostics; this function deliberately does not call the bool schema
/// adapter or claim complete lab coverage.
pub fn inspect_supplied_semantic_annotation(
    path: &str,
    packet: &Value,
    limits: ItemLimits,
) -> Result<LayerFamilyReport, ItemRefusal> {
    let mut rules = LayerFamilyRules::new(limits);
    rules.semantic_annotation(path, packet)?;
    Ok(rules.finish())
}

/// Apply only the existing source-text-unit semantic predicates to an
/// owner-supplied packet and frozen text. The `schema_checked` execution bit
/// only releases the text kernel's semantic path here; this function performs
/// no schema call and returns no schema verdict. The source adapter retains
/// raw reads and schedules full schema diagnostics separately.
pub fn inspect_supplied_source_text_unit_semantics(
    packet_path: &str,
    raw_packet: &[u8],
    frozen_text_locator: &str,
    frozen_text: &[u8],
    snapshot_generation: &str,
    limits: ItemLimits,
) -> Result<TextRuleReport, ItemRefusal> {
    if raw_packet.len() > limits.max_member_bytes
        || frozen_text.len() > limits.max_member_bytes
        || packet_path.is_empty()
        || frozen_text_locator.is_empty()
        || snapshot_generation.is_empty()
    {
        return Err(ItemRefusal::Budget);
    }
    if std::time::Instant::now() >= limits.deadline {
        return Err(ItemRefusal::Deadline);
    }
    let context = text_rules::TextRuleContext {
        packet_path: packet_path.into(),
        frozen_text_locator: frozen_text_locator.into(),
        schema_checked: true,
        requested_profiles: vec![text_rules::TEXT_UNIT_PROFILE.into()],
        interval_generation: snapshot_generation.into(),
        reverse_generation: snapshot_generation.into(),
    };
    let report = text_rules::inspect_source_text_unit_v1(raw_packet, frozen_text, &context);
    if std::time::Instant::now() >= limits.deadline {
        return Err(ItemRefusal::Deadline);
    }
    if report.state == TextRuleState::BudgetExceeded {
        return Err(ItemRefusal::Budget);
    }
    Ok(report)
}

impl LayerFamilyRules {
    pub fn new(limits: ItemLimits) -> Self {
        Self {
            limits,
            state_bytes: 0,
            named_transient: 0,
            report: LayerFamilyReport::default(),
            identities: BTreeMap::new(),
            discovery_events: None,
            boundary_events: None,
            require_local_payloads: false,
        }
    }
    pub fn require_local_payloads(&mut self, required: bool) {
        self.require_local_payloads = required;
    }
    fn reserve(&mut self, n: usize) -> Result<(), ItemRefusal> {
        if Instant::now() >= self.limits.deadline {
            return Err(ItemRefusal::Deadline);
        }
        self.state_bytes = self
            .state_bytes
            .checked_add(n)
            .filter(|n| {
                n.checked_add(self.named_transient)
                    .is_some_and(|total| total <= self.limits.max_state_bytes)
            })
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }
    fn reserve_named(&mut self, n: usize) -> Result<(), ItemRefusal> {
        self.named_transient = self
            .named_transient
            .checked_add(n)
            .filter(|total| {
                self.state_bytes
                    .checked_add(*total)
                    .is_some_and(|used| used <= self.limits.max_state_bytes)
            })
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }
    fn issue(
        &mut self,
        path: &str,
        code: &'static str,
        subject: impl Into<String>,
    ) -> Result<(), ItemRefusal> {
        if self.report.issues.len() >= self.limits.max_issues {
            return Err(ItemRefusal::Budget);
        }
        let subject = subject.into();
        self.reserve(path.len() + code.len() + subject.len() + 96)?;
        self.report.issues.push(LayerFamilyIssue {
            path: path.into(),
            code,
            subject,
        });
        Ok(())
    }
    fn read(&mut self, read: PredicateRead) -> Result<(), ItemRefusal> {
        self.reserve(format!("{read:?}").len() + 96)?;
        self.report.reads.push(read);
        Ok(())
    }
    fn gap(&mut self, path: &str, profile: &str) -> Result<(), ItemRefusal> {
        self.reserve(path.len() + profile.len() + 64)?;
        self.report.unsupported.push(LayerProfileGap {
            path: path.into(),
            profile: profile.into(),
        });
        Ok(())
    }
    fn checked(&mut self, path: &str, predicate: &str) -> Result<(), ItemRefusal> {
        self.reserve(path.len() + predicate.len() + 64)?;
        self.report
            .checked_predicates
            .push((path.into(), predicate.into()));
        Ok(())
    }
    fn bytes(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        digest: Option<&str>,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        self.lookup(source, path, digest, true)
    }
    fn lookup(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        digest: Option<&str>,
        required: bool,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        source.checkpoint(self.limits.deadline)?;
        safe(path)?;
        let raw = match digest {
            Some(d) => {
                source.recorded(path, d, self.limits.max_member_bytes, self.limits.deadline)?
            }
            None => source.current(path, self.limits.max_member_bytes, self.limits.deadline)?,
        };
        source.checkpoint(self.limits.deadline)?;
        if let Some(raw) = &raw {
            if raw.len() > self.limits.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            if digest.is_some_and(|d| Digest256::of_bytes(raw).to_hex() != d) {
                return Err(ItemRefusal::Source(
                    "layer-family recorded-input digest mismatch".into(),
                ));
            }
            self.report.metadata_bytes = self
                .report
                .metadata_bytes
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= self.limits.max_total_bytes)
                .ok_or(ItemRefusal::Budget)?;
            // Charge simultaneously retained parser state as well as transport.
            self.reserve(raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
            self.read(PredicateRead::ExactBytes {
                locator: match digest {
                    Some(d) => format!("retained-or-current:{path}@{d}"),
                    None => path.into(),
                },
                digest: Digest256::of_bytes(raw).to_hex(),
            })?;
        } else {
            self.read(PredicateRead::AbsentKey {
                namespace: format!("source-layer/{}", source.generation()),
                key: match digest {
                    Some(d) => format!("{path}@{d}"),
                    None => path.into(),
                },
            })?;
            if required {
                self.issue(path, "missing-exact-input", digest.unwrap_or("current"))?;
            }
        }
        Ok(raw)
    }
    fn schema(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        raw: &[u8],
        contract: &str,
    ) -> Result<bool, ItemRefusal> {
        let Some(schema) = self.bytes(source, contract, None)? else {
            return Err(ItemRefusal::Unsupported(
                "missing current layer contract".into(),
            ));
        };
        self.read(PredicateRead::SchemaResource {
            uri: format!("https://tree-of-sophia.local/{contract}"),
            digest: Digest256::of_bytes(&schema).to_hex(),
        })?;
        source.schema(path, raw, contract, self.limits.deadline)
    }
    pub fn record_gap(&mut self, path: &str, profile: &str) -> Result<(), ItemRefusal> {
        self.gap(path, profile)
    }
    fn decoded(
        &mut self,
        path: &str,
        raw: &[u8],
        invalid_code: &'static str,
    ) -> Result<Option<Value>, ItemRefusal> {
        match crate::native_decoded_value(raw, self.limits.max_member_bytes) {
            Ok(value) => Ok(Some(value)),
            Err(ItemRefusal::Source(reason)) => {
                self.issue(path, invalid_code, &reason)?;
                Ok(None)
            }
            Err(ItemRefusal::Unsupported(reason)) => {
                self.gap(path, &reason)?;
                Ok(None)
            }
            Err(reason) => Err(reason),
        }
    }
    fn object(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        contract: Option<&str>,
    ) -> Result<Option<(Value, Vec<u8>)>, ItemRefusal> {
        let Some(raw) = self.bytes(source, path, None)? else {
            return Ok(None);
        };
        let Some(value) = self.decoded(path, &raw, "invalid-json")? else {
            return Ok(None);
        };
        if !value.is_object() {
            self.issue(path, "object-required", path)?;
            return Ok(None);
        }
        if let Some(contract) = contract {
            if !self.schema(source, path, &raw, contract)? {
                self.issue(path, "schema", contract)?;
            }
            self.checked(path, "Draft2020-12-owner-schema")?;
        }
        Ok(Some((value, raw)))
    }
    // Only the named bridge retains its decoded plan and all four outputs at
    // once. The generic bytes() path has already charged the raw transport;
    // this adds the simultaneously live decoded tree and output collection.
    fn named_object(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        contract: Option<&str>,
        schema_enabled: bool,
    ) -> Result<Option<(Value, Vec<u8>)>, ItemRefusal> {
        let Some(raw) = self.bytes(source, path, None)? else {
            return Ok(None);
        };
        let available = self
            .limits
            .max_state_bytes
            .checked_sub(self.state_bytes)
            .and_then(|n| n.checked_sub(self.named_transient))
            .ok_or(ItemRefusal::Budget)?;
        let codec =
            tos_foundation::JsonLimits::new(self.limits.max_member_bytes, 64, 300_000, 4_300)
                .map_err(|_| ItemRefusal::Budget)?;
        let (value, decoded_state) =
            match crate::validation_codec::bounded_legacy_decoded_state_with_limits(
                &raw,
                codec,
                available,
                self.limits.deadline,
                source.cancellation(),
            ) {
                Ok(result) => result,
                Err(ItemRefusal::Source(reason)) => {
                    self.issue(path, "invalid-json", reason)?;
                    return Ok(None);
                }
                Err(ItemRefusal::Unsupported(reason)) => {
                    self.gap(path, &reason)?;
                    return Ok(None);
                }
                Err(reason) => return Err(reason),
            };
        self.reserve_named(decoded_state)?;
        if !value.is_object() {
            self.issue(path, "object-required", path)?;
            return Ok(None);
        }
        if schema_enabled {
            if let Some(contract) = contract {
                if !self.schema(source, path, &raw, contract)? {
                    self.issue(path, "schema", contract)?;
                }
                self.checked(path, "Draft2020-12-owner-schema")?;
            }
        }
        self.reserve_named(
            path.len()
                .checked_add(std::mem::size_of::<Vec<u8>>())
                .ok_or(ItemRefusal::Budget)?,
        )?;
        Ok(Some((value, raw)))
    }
    fn identity(
        &mut self,
        source: &impl LayerFamilySource,
        path: &str,
        namespace: &str,
        id: &str,
    ) -> Result<(), ItemRefusal> {
        self.reserve(namespace.len() + id.len() + path.len() + 96)?;
        if self
            .identities
            .insert((namespace.into(), id.into()), path.into())
            .is_some()
        {
            self.issue(path, "duplicate-identity", id)?;
        }
        self.read(PredicateRead::UniqueKey {
            namespace: namespace.into(),
            key: id.into(),
            owner: path.into(),
        })?;
        self.read(PredicateRead::Range {
            namespace: namespace.into(),
            lower: id.into(),
            upper: id.into(),
            generation: source.generation(),
        })
    }
    fn endpoint(
        &mut self,
        path: &str,
        label: &str,
        id: &str,
        present: bool,
    ) -> Result<(), ItemRefusal> {
        self.read(PredicateRead::RefEndpoint {
            endpoint_type: format!("layer-packet/{path}/{label}"),
            id: id.into(),
            observed: if present {
                KeyState::Present
            } else {
                KeyState::Absent
            },
        })?;
        if !present {
            self.issue(path, "unresolved-local-reference", format!("{label}:{id}"))?;
        }
        Ok(())
    }
    pub fn inspect(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
    ) -> Result<(), ItemRefusal> {
        let Some((v, raw)) = self.object(source, path, None)? else {
            return Ok(());
        };
        if path.ends_with("/artifact-witness.json")
            || path.ends_with("/composite-witness.json")
            || (path.ends_with("/representation.json")
                && (path.starts_with("ToS/source-witnesses/artifacts/")
                    || path.starts_with("ToS/source-witnesses/scholarly-composites/")))
        {
            return self.witness(source, path, &v, &raw);
        }
        if path.starts_with("ToS/source-witnesses/server-import/plans/")
            && path
                .strip_prefix("ToS/source-witnesses/server-import/plans/")
                .is_some_and(|name| !name.contains('/'))
        {
            return self.server_plan(source, path, &v, &raw);
        }
        let profile = s(&v, "schema_version");
        let contract = match profile {
            text_rules::TEXT_UNIT_PROFILE => "source-text-unit-packet-v1.schema.json",
            text_rules::TEXT_LAYER_PROFILE => "source-text-layer.schema.json",
            text_rules::ANCHOR_V2_PROFILE => "source-anchor-v2.schema.json",
            "tos_semantic_ladder_packet_v4" => "semantic-ladder-packet.schema.json",
            "tos_transfer_candidate_structural_crosswalk_v1" => {
                "transfer-candidate-structural-crosswalk.schema.json"
            }
            "tos_semantic_annotation_packet_v2" => "semantic-annotation-packet-v2.schema.json",
            "tos_translation_alignment_packet_v1" => "translation-alignment-packet-v1.schema.json",
            _ => {
                self.gap(
                    path,
                    if profile.is_empty() {
                        "unknown-layer-profile"
                    } else {
                        profile
                    },
                )?;
                return Ok(());
            }
        };
        let contract = format!("ToS/contracts/{contract}");
        let schema_valid = self.schema(source, path, &raw, &contract)?;
        if !schema_valid {
            self.issue(path, "schema", &contract)?;
        }
        self.checked(path, "Draft2020-12-owner-schema")?;
        if matches!(
            profile,
            text_rules::TEXT_UNIT_PROFILE
                | text_rules::TEXT_LAYER_PROFILE
                | text_rules::ANCHOR_V2_PROFILE
        ) {
            self.text_metadata(source, path, &raw, profile)?;
        }
        match profile {
            "tos_semantic_ladder_packet_v4" => self.semantic_ladder(path, &v)?,
            "tos_transfer_candidate_structural_crosswalk_v1" => self.transfer(source, path, &v)?,
            text_rules::TEXT_UNIT_PROFILE | text_rules::TEXT_LAYER_PROFILE => {
                self.text(source, path, &v, &raw, profile, schema_valid)?
            }
            text_rules::ANCHOR_V2_PROFILE => {
                self.gap(path, "anchor-target-and-method-explicit-binding-required")?
            }
            "tos_semantic_annotation_packet_v2" => self.semantic_annotation(path, &v)?,
            "tos_translation_alignment_packet_v1" => self.translation_alignment(path, &v)?,
            _ => {}
        }
        source.checkpoint(self.limits.deadline)
    }
    /// The source-owned real sentence bridge is a named, text-free closure.
    /// It relates already inspected packets but does not replay private spans,
    /// decide translation fidelity, or authorize publication.
    pub fn inspect_zarathustra_opening_sentence(
        &mut self,
        source: &mut impl LayerFamilySource,
    ) -> Result<(), ItemRefusal> {
        self.inspect_zarathustra_opening_sentence_inner(source, true)
    }

    /// Run the same named bridge mechanics while leaving full schema
    /// diagnostics to the caller's scheduled diagnostic-v2 requests.
    pub fn inspect_zarathustra_opening_sentence_without_schema(
        &mut self,
        source: &mut impl LayerFamilySource,
    ) -> Result<(), ItemRefusal> {
        self.inspect_zarathustra_opening_sentence_inner(source, false)
    }

    fn inspect_zarathustra_opening_sentence_inner(
        &mut self,
        source: &mut impl LayerFamilySource,
        schema_enabled: bool,
    ) -> Result<(), ItemRefusal> {
        let prior = self.named_transient;
        let result = self.inspect_zarathustra_opening_sentence_inner_body(source, schema_enabled);
        // The named plan and output trees leave scope together, including on
        // an early refusal. Report and read state remain charged by reserve().
        self.named_transient = prior;
        result
    }
    fn inspect_zarathustra_opening_sentence_inner_body(
        &mut self,
        source: &mut impl LayerFamilySource,
        schema_enabled: bool,
    ) -> Result<(), ItemRefusal> {
        source.checkpoint(self.limits.deadline)?;
        if !source.exists(
            OPENING_SENTENCE_PLAN,
            self.limits.max_member_bytes,
            self.limits.deadline,
        )? {
            return self.gap(
                OPENING_SENTENCE_PLAN,
                "named-opening-sentence-plan-not-selected",
            );
        }
        let Some((plan, _)) =
            self.named_object(source, OPENING_SENTENCE_PLAN, None, schema_enabled)?
        else {
            return Ok(());
        };
        if s(&plan, "schema_version") != "tos_zarathustra_opening_sentence_alignment_plan_v1" {
            self.issue(
                OPENING_SENTENCE_PLAN,
                "opening-sentence-plan-version",
                s(&plan, "schema_version"),
            )?;
        }
        let output_fields = [
            "source_sentence_packet_ref",
            "target_sentence_packet_ref",
            "alignment_packet_ref",
            "provenance_event_ref",
        ];
        let contracts = [
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
            "ToS/contracts/translation-alignment-packet-v1.schema.json",
            "ToS/contracts/provenance-event-v2.schema.json",
        ];
        self.reserve_named(
            std::mem::size_of::<Vec<(String, Value, Vec<u8>)>>()
                .checked_add(
                    4usize
                        .checked_mul(std::mem::size_of::<(String, Value, Vec<u8>)>())
                        .ok_or(ItemRefusal::Budget)?,
                )
                .ok_or(ItemRefusal::Budget)?,
        )?;
        let mut outputs = Vec::with_capacity(4);
        for (field, contract) in output_fields.into_iter().zip(contracts) {
            source.checkpoint(self.limits.deadline)?;
            let Some(path) = plan["outputs"][field].as_str().filter(|p| !p.is_empty()) else {
                self.issue(
                    OPENING_SENTENCE_PLAN,
                    "opening-sentence-output-ref-absent",
                    field,
                )?;
                continue;
            };
            if let Some((value, raw)) =
                self.named_object(source, path, Some(contract), schema_enabled)?
            {
                outputs.push((path.to_owned(), value, raw));
            }
        }
        if outputs.len() != 4 {
            return Ok(());
        }
        let (source_path, source_packet, source_raw) =
            (&outputs[0].0, &outputs[0].1, &outputs[0].2);
        let (target_path, target_packet, target_raw) =
            (&outputs[1].0, &outputs[1].1, &outputs[1].2);
        let (alignment_path, alignment, alignment_raw) =
            (&outputs[2].0, &outputs[2].1, &outputs[2].2);
        let (event_path, event, _) = (&outputs[3].0, &outputs[3].1, &outputs[3].2);
        for (path, packet, version) in [
            (source_path, source_packet, text_rules::TEXT_UNIT_PROFILE),
            (target_path, target_packet, text_rules::TEXT_UNIT_PROFILE),
            (
                alignment_path,
                alignment,
                "tos_translation_alignment_packet_v1",
            ),
        ] {
            if s(packet, "schema_version") != version {
                self.issue(path, "opening-sentence-output-profile", version)?;
            }
        }
        if s(event, "schema_version") != "tos_provenance_event_v2" {
            self.issue(
                event_path,
                "opening-sentence-event-profile",
                s(event, "schema_version"),
            )?;
        }
        let available = self
            .limits
            .max_state_bytes
            .checked_sub(self.state_bytes)
            .and_then(|n| n.checked_sub(self.named_transient))
            .ok_or(ItemRefusal::Budget)?;
        let workspace = crate::provenance_rules::semantic_workspace(
            event,
            self.limits
                .max_issues
                .saturating_sub(self.report.issues.len()),
            available,
        )?;
        self.reserve_named(workspace)?;
        for message in crate::provenance_rules::semantic_issues(
            event,
            self.limits
                .max_issues
                .saturating_sub(self.report.issues.len()),
            self.limits.deadline,
        )? {
            self.issue(event_path, "opening-sentence-event-semantic", message)?;
        }
        self.named_transient -= workspace;
        self.opening_sentence_side(
            &plan,
            "source",
            source_path,
            source_packet,
            source_raw,
            alignment_path,
            alignment,
        )?;
        self.opening_sentence_side(
            &plan,
            "target",
            target_path,
            target_packet,
            target_raw,
            alignment_path,
            alignment,
        )?;
        self.opening_sentence_authority(&plan, alignment_path, alignment, event_path, event)?;
        self.opening_sentence_bindings(source, &plan)?;
        let output_rows = rows(&event["entities"], "outputs");
        let output_slots = output_rows
            .iter()
            .filter(|row| row["entity_ref"].as_str().is_some() && row["sha256"].as_str().is_some())
            .count();
        let map_state = std::mem::size_of::<BTreeMap<&str, &str>>()
            .checked_add(
                output_slots
                    .checked_mul(std::mem::size_of::<(&str, &str)>())
                    .ok_or(ItemRefusal::Budget)?,
            )
            .and_then(|n| n.checked_add(3 * (std::mem::size_of::<String>() + 64)))
            .ok_or(ItemRefusal::Budget)?;
        self.reserve_named(map_state)?;
        // Last duplicate wins, as in the source owner's keyed entity view.
        // Borrowed keys/values avoid cloning whole event strings.
        let actual_outputs: BTreeMap<&str, &str> = output_rows
            .iter()
            .filter_map(|row| Some((row["entity_ref"].as_str()?, row["sha256"].as_str()?)))
            .collect();
        let source_digest = Digest256::of_bytes(source_raw).to_hex();
        let target_digest = Digest256::of_bytes(target_raw).to_hex();
        let alignment_digest = Digest256::of_bytes(alignment_raw).to_hex();
        if actual_outputs.len() != 3
            || actual_outputs.get(source_path.as_str()) != Some(&source_digest.as_str())
            || actual_outputs.get(target_path.as_str()) != Some(&target_digest.as_str())
            || actual_outputs.get(alignment_path.as_str()) != Some(&alignment_digest.as_str())
        {
            self.issue(
                event_path,
                "opening-sentence-event-output-closure",
                event_path,
            )?;
        }
        self.checked(
            OPENING_SENTENCE_PLAN,
            "named-zarathustra-opening-sentence-tracked-closure-v1",
        )?;
        source.checkpoint(self.limits.deadline)
    }
    fn opening_sentence_side(
        &mut self,
        plan: &Value,
        label: &str,
        packet_path: &str,
        packet: &Value,
        packet_raw: &[u8],
        alignment_path: &str,
        alignment: &Value,
    ) -> Result<(), ItemRefusal> {
        self.reserve(0)?;
        let side_plan = &plan[label];
        let side = &alignment[if label == "source" {
            "source_side"
        } else {
            "target_side"
        }];
        if !opening_scope_matches(&packet["source_scope"], side_plan) {
            self.issue(packet_path, "opening-sentence-source-scope", label)?;
        }
        let source_layer = &packet["source_layer"];
        if source_layer["text_layer_ref"] != side_plan["text_layer_ref"]
            || source_layer["text_layer_sha256"] != side_plan["text_layer_sha256"]
            || source_layer["language"] != side_plan["language"]
            || source_layer["visibility"] != "local_only"
            || source_layer["publication_authorized"] != false
        {
            self.issue(packet_path, "opening-sentence-frozen-layer", label)?;
        }
        if !rows(packet, "reviews").is_empty() || !rows(packet, "projections").is_empty() {
            self.issue(
                packet_path,
                "opening-sentence-fabricated-review-projection",
                label,
            )?;
        }
        let segmentations = rows(packet, "segmentations");
        let units = rows(packet, "units");
        let anchors = rows(packet, "anchors");
        if segmentations.len() != 1 || s(&segmentations[0], "status") != "proposed" {
            self.issue(packet_path, "opening-sentence-segmentation-posture", label)?;
        } else if !rows(&segmentations[0], "review_refs").is_empty()
            || s(&segmentations[0]["coverage"], "coverage_posture") != "declared_partial"
            || !rows(&segmentations[0], "declared_uses")
                .iter()
                .any(|value| value.as_str() == Some("translation_alignment"))
        {
            self.issue(packet_path, "opening-sentence-segmentation-coverage", label)?;
        }
        if units.len() != 1
            || s(&units[0], "unit_kind") != "sentence"
            || s(&units[0], "boundary_posture") != "method_proposed"
            || units[0]["semantic_promotion"] != false
        {
            self.issue(packet_path, "opening-sentence-unit-posture", label)?;
        }
        let ids = &plan["opaque_ids"];
        let sentence_id = s(ids, &format!("{label}_sentence_anchor_id"));
        let remainder_id = s(ids, &format!("{label}_remainder_anchor_id"));
        // Python's keyed anchor view keeps the final occurrence; schema and
        // family checks still report duplicate identities independently.
        let sentence = anchors
            .iter()
            .rev()
            .find(|row| s(row, "anchor_ref") == sentence_id)
            .unwrap_or(&Value::Null);
        let remainder = anchors
            .iter()
            .rev()
            .find(|row| s(row, "anchor_ref") == remainder_id)
            .unwrap_or(&Value::Null);
        if !opening_selector_matches(
            &sentence["selector"],
            &side_plan["sentence_start"],
            &side_plan["sentence_end"],
        ) || sentence["exact_sha256"] != side_plan["sentence_sha256"]
            || sentence["source_return"]["locator_ref"] != side_plan["private_content_ref"]
        {
            self.issue(packet_path, "opening-sentence-anchor", label)?;
        }
        if !opening_selector_matches(
            &remainder["selector"],
            &side_plan["sentence_end"],
            &side_plan["scope_end"],
        ) || remainder["exact_sha256"] != side_plan["remainder_sha256"]
        {
            self.issue(packet_path, "opening-sentence-excluded-remainder", label)?;
        }
        if segmentations.first().is_none_or(|row| {
            !rows(&row["coverage"], "excluded_anchor_refs")
                .iter()
                .any(|value| value.as_str() == Some(remainder_id))
        }) {
            self.issue(packet_path, "opening-sentence-remainder-coverage", label)?;
        }
        if !opening_scope_fields_match(side, side_plan)
            || side["text_layer_ref"] != side_plan["text_layer_ref"]
            || side["text_layer_sha256"] != side_plan["text_layer_sha256"]
            || side["language"] != side_plan["language"]
            || side["visibility"] != "local_only"
            || side["publication_authorized"] != false
        {
            self.issue(alignment_path, "opening-sentence-alignment-side", label)?;
        }
        let binding = &side["segmentation"];
        if binding["artifact_ref"] != packet_path
            || binding["sha256"] != Digest256::of_bytes(packet_raw).to_hex()
            || binding["state"] != "frozen"
        {
            self.issue(
                alignment_path,
                "opening-sentence-segmentation-binding",
                label,
            )?;
        }
        let aligned = rows(side, "anchors");
        if aligned.len() != 1 {
            self.issue(alignment_path, "opening-sentence-side-anchor-count", label)?;
        } else if [
            "anchor_ref",
            "text_layer_ref",
            "text_layer_sha256",
            "selector",
            "exact_sha256",
            "source_return",
        ]
        .iter()
        .any(|key| aligned[0][*key] != sentence[*key])
        {
            self.issue(alignment_path, "opening-sentence-side-anchor", label)?;
        }
        if !side["tokenization"].is_null() {
            self.issue(
                alignment_path,
                "opening-sentence-fabricated-tokenization",
                label,
            )?;
        }
        for anchor in [sentence, aligned.first().unwrap_or(&Value::Null)] {
            if anchor["anchor_ref"] != sentence_id
                || !opening_selector_matches(
                    &anchor["selector"],
                    &side_plan["sentence_start"],
                    &side_plan["sentence_end"],
                )
                || anchor["exact_sha256"] != side_plan["sentence_sha256"]
                || anchor["text_layer_ref"] != side_plan["text_layer_ref"]
                || anchor["text_layer_sha256"] != side_plan["text_layer_sha256"]
                || anchor["source_return"]["locator_ref"] != side_plan["private_content_ref"]
            {
                self.issue(
                    alignment_path,
                    "opening-sentence-plan-anchor-binding",
                    label,
                )?;
            }
        }
        Ok(())
    }
    fn opening_sentence_authority(
        &mut self,
        plan: &Value,
        alignment_path: &str,
        alignment: &Value,
        event_path: &str,
        event: &Value,
    ) -> Result<(), ItemRefusal> {
        let boundary = &plan["authority_boundary"];
        for key in [
            "source_sentence_segmentation_status",
            "target_sentence_segmentation_status",
            "alignment_status",
        ] {
            if boundary[key] != "proposed" {
                self.issue(
                    OPENING_SENTENCE_PLAN,
                    "opening-sentence-authority-boundary",
                    key,
                )?;
            }
        }
        for key in [
            "accepted_german",
            "accepted_russian",
            "accepted_translation",
            "translation_fidelity_established",
            "lexical_equivalence_established",
            "etymology_or_semantics_established",
            "human_task_created",
            "projection_created",
            "graph_or_canon_effect",
        ] {
            if boundary[key] != false {
                self.issue(
                    OPENING_SENTENCE_PLAN,
                    "opening-sentence-authority-boundary",
                    key,
                )?;
            }
        }
        if boundary.as_object().is_none_or(|map| map.len() != 12) {
            self.issue(
                OPENING_SENTENCE_PLAN,
                "opening-sentence-authority-boundary-shape",
                "authority_boundary",
            )?;
        }
        for key in [
            "dehyphenation",
            "tokenization",
            "model_invoked",
            "translation_performed",
            "recognized_translation_used_for_text_decision",
            "human_review_performed",
        ] {
            if plan["method"][key] != false {
                self.issue(
                    OPENING_SENTENCE_PLAN,
                    "opening-sentence-method-widened",
                    key,
                )?;
            }
        }
        let proposed = rows(alignment, "alignments");
        if proposed.len() != 1 {
            self.issue(
                alignment_path,
                "opening-sentence-alignment-count",
                "alignments",
            )?;
        } else {
            let row = &proposed[0];
            if s(row, "status") != "proposed"
                || s(row, "correspondence_shape") != "one_to_one"
                || !rows(row, "translation_techniques")
                    .iter()
                    .filter_map(Value::as_str)
                    .eq(std::iter::once("unresolved"))
                || s(row, "epistemic_status") != "inferred"
                || !rows(row, "review_refs").is_empty()
                || s(&row["maker"], "maker_kind") != "software"
            {
                self.issue(
                    alignment_path,
                    "opening-sentence-alignment-posture",
                    "alignment",
                )?;
            }
        }
        if !rows(alignment, "reviews").is_empty() || !rows(alignment, "projections").is_empty() {
            self.issue(
                alignment_path,
                "opening-sentence-alignment-review-projection",
                "alignment",
            )?;
        }
        let rights = &alignment["rights_and_visibility"];
        let rights_refs = rows(rights, "rights_record_refs");
        if rights.as_object().is_none_or(|map| map.len() != 8)
            || rights["source_visibility"] != "local_only"
            || rights["target_visibility"] != "local_only"
            || rights["packet_visibility"] != "public_metadata_only"
            || rights["effective_visibility"] != "local_only"
            || rights_refs.len() != 2
            || rights_refs[0] != plan["source"]["rights_ref"]
            || rights_refs[1] != plan["target"]["rights_ref"]
            || rights["private_source_used"] != true
            || rights["publication_authorized"] != false
            || rights["inheritance_policy"] != "most_restrictive_side_or_packet_wins"
        {
            self.issue(
                alignment_path,
                "opening-sentence-rights-visibility",
                "alignment",
            )?;
        }
        let review = &event["review_and_authority"];
        if review["human_review_status"] != "not_performed"
            || !rows(review, "accepted_uses").is_empty()
            || review["promotion_authorized"] != false
        {
            self.issue(event_path, "opening-sentence-event-authority", "event")?;
        }
        Ok(())
    }
    fn opening_sentence_bindings(
        &mut self,
        source: &mut impl LayerFamilySource,
        plan: &Value,
    ) -> Result<(), ItemRefusal> {
        let bindings = [
            ("source", "text_layer_ref", "text_layer_record_sha256"),
            ("source", "layout_packet_ref", "layout_packet_sha256"),
            (
                "source",
                "edition_reading_admission_ref",
                "edition_reading_admission_sha256",
            ),
            ("source", "rights_ref", "rights_sha256"),
            ("target", "text_layer_ref", "text_layer_record_sha256"),
            ("target", "layout_packet_ref", "layout_packet_sha256"),
            (
                "target",
                "expression_record_ref",
                "expression_record_sha256",
            ),
            (
                "target",
                "responsibility_claims_ref",
                "responsibility_claims_sha256",
            ),
            ("target", "rights_ref", "rights_sha256"),
        ];
        for (side, ref_field, digest_field) in bindings {
            source.checkpoint(self.limits.deadline)?;
            let Some(path) = plan[side][ref_field]
                .as_str()
                .filter(|value| !value.is_empty())
            else {
                self.issue(
                    OPENING_SENTENCE_PLAN,
                    "opening-sentence-tracked-ref",
                    format!("{side}/{ref_field}"),
                )?;
                continue;
            };
            let expected = s(&plan[side], digest_field);
            match self.bytes(source, path, None)? {
                Some(raw) if Digest256::of_bytes(&raw).to_hex() == expected => {}
                _ => self.issue(
                    OPENING_SENTENCE_PLAN,
                    "opening-sentence-tracked-binding",
                    format!("{side}/{ref_field}"),
                )?,
            }
        }
        Ok(())
    }
    /// Explicit anchor inputs avoid interpreting a source identity as a path.
    pub fn inspect_anchor(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        target: &str,
        target_digest: &str,
        method: &str,
        method_digest: &str,
    ) -> Result<(), ItemRefusal> {
        let Some((_, raw)) = self.object(source, path, None)? else {
            return Ok(());
        };
        let schema_valid = self.schema(
            source,
            path,
            &raw,
            "ToS/contracts/source-anchor-v2.schema.json",
        )?;
        if !schema_valid {
            self.issue(path, "schema", "source-anchor-v2")?;
        }
        let Some(bytes) = self.bytes(source, target, Some(target_digest))? else {
            return Ok(());
        };
        let Some(config) = self.bytes(source, method, Some(method_digest))? else {
            return Ok(());
        };
        let generation = source.generation();
        let report = text_rules::inspect_source_anchor_v2_single(
            &raw,
            &bytes,
            &config,
            &text_rules::AnchorRuleContext {
                anchor_path: path.into(),
                target_locator: target.into(),
                method_configuration_locator: method.into(),
                schema_checked: schema_valid,
                requested_profiles: vec![text_rules::ANCHOR_V2_PROFILE.into()],
                interval_generation: generation.clone(),
                reverse_generation: generation,
            },
        );
        self.absorb_text(path, report)
    }
    fn absorb_text(&mut self, path: &str, report: TextRuleReport) -> Result<(), ItemRefusal> {
        if report.state == TextRuleState::BudgetExceeded {
            return Err(ItemRefusal::Budget);
        }
        for issue in &report.issues {
            self.issue(path, issue.code, &issue.subject)?;
        }
        for gap in &report.unsupported_profiles {
            self.gap(path, gap)?;
        }
        for read in &report.reads {
            self.read(read.clone())?;
        }
        // Preserve intervals/reverse facts alongside the helper's named scope.
        self.reserve(format!("{report:?}").len())?;
        self.report.text_reports.push(report);
        Ok(())
    }
    fn text_metadata(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        raw: &[u8],
        profile: &str,
    ) -> Result<(), ItemRefusal> {
        let limits = TextMetadataLimits {
            max_packet_bytes: self.limits.max_member_bytes.min(2_097_152),
            max_state_bytes: self
                .limits
                .max_state_bytes
                .checked_sub(self.state_bytes)
                .ok_or(ItemRefusal::Budget)?
                .min(134_217_728),
            max_issues: self
                .limits
                .max_issues
                .checked_sub(self.report.issues.len())
                .ok_or(ItemRefusal::Budget)?
                .min(8192),
            deadline: self.limits.deadline,
        };
        let report = match profile {
            text_rules::TEXT_UNIT_PROFILE => {
                text_metadata_rules::inspect_source_text_unit_v1_metadata(
                    raw,
                    path,
                    limits,
                    source.cancellation(),
                )?
            }
            text_rules::TEXT_LAYER_PROFILE => {
                text_metadata_rules::inspect_source_text_layer_metadata(
                    raw,
                    path,
                    limits,
                    source.cancellation(),
                )?
            }
            _ => text_metadata_rules::inspect_source_anchor_v2_metadata(
                raw,
                path,
                limits,
                source.cancellation(),
            )?,
        };
        source.checkpoint(self.limits.deadline)?;
        for issue in &report.issues {
            self.issue(path, issue.code, &issue.message)?;
        }
        for read in &report.reads {
            self.read(read.clone())?;
        }
        if report.state == TextMetadataState::Unsupported {
            self.gap(path, "text-metadata-numeric-or-profile-unsupported")?;
        }
        self.checked(path, &format!("metadata-only/{profile}"))?;
        self.reserve(format!("{report:?}").len())?;
        self.report.metadata_reports.push(report);
        Ok(())
    }
    fn text(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        v: &Value,
        raw: &[u8],
        profile: &str,
        schema_valid: bool,
    ) -> Result<(), ItemRefusal> {
        let generation = source.generation();
        if profile == text_rules::TEXT_UNIT_PROFILE {
            let binding = &v["source_layer"];
            let locator = s(binding, "text_layer_ref");
            let Some(text) = self.bytes(source, locator, Some(s(binding, "text_layer_sha256")))?
            else {
                return Ok(());
            };
            let report = text_rules::inspect_source_text_unit_v1(
                raw,
                &text,
                &text_rules::TextRuleContext {
                    packet_path: path.into(),
                    frozen_text_locator: locator.into(),
                    schema_checked: schema_valid,
                    requested_profiles: vec![profile.into()],
                    interval_generation: generation.clone(),
                    reverse_generation: generation,
                },
            );
            self.absorb_text(path, report)?;
        } else {
            let mut bindings = BTreeMap::new();
            bindings.insert(
                s(&v["source_binding"], "source_file_ref").to_owned(),
                s(&v["source_binding"], "source_file_sha256").to_owned(),
            );
            bindings.insert(
                s(&v["representation"], "content_ref").to_owned(),
                s(&v["representation"], "content_sha256").to_owned(),
            );
            bindings.insert(
                s(&v["editorial_policy"], "policy_ref").to_owned(),
                s(&v["editorial_policy"], "policy_sha256").to_owned(),
            );
            for a in rows(&v["source_binding"], "anchors") {
                bindings.insert(
                    s(a, "anchor_record_ref").into(),
                    s(a, "anchor_record_sha256").into(),
                );
            }
            for input in rows(&v["derivation"], "input_layers") {
                let locator = s(input, "record_ref");
                let digest = s(input, "record_sha256");
                if let Some(bytes) = self.bytes(source, locator, Some(digest))? {
                    if let Some(predecessor) =
                        self.decoded(locator, &bytes, "invalid-layer-predecessor")?
                    {
                        bindings.insert(
                            s(&predecessor["representation"], "content_ref").into(),
                            s(&predecessor["representation"], "content_sha256").into(),
                        );
                    }
                    bindings.insert(locator.into(), digest.into());
                }
            }
            for key in ["rights_record_refs", "publication_authority_refs"] {
                for r in rows(&v["representation"], key) {
                    bindings.insert(s(r, "ref").into(), s(r, "sha256").into());
                }
            }
            for maker in [&v["derivation"]["maker"]] {
                if maker["configuration_ref"].is_string() {
                    bindings.insert(
                        s(maker, "configuration_ref").into(),
                        s(maker, "configuration_digest").into(),
                    );
                }
            }
            let mut owned = Vec::new();
            for (locator, digest) in bindings {
                if locator.starts_with("tos.") {
                    self.gap(path, "source-file-identity-resource-binding-required")?;
                    continue;
                }
                if let Some(bytes) = self.bytes(source, &locator, Some(&digest))? {
                    owned.push((locator, bytes));
                }
            }
            let resources: Vec<_> = owned
                .iter()
                .map(|(locator, raw)| LayerResource { locator, raw })
                .collect();
            let report = text_rules::inspect_source_text_layer_v1(
                raw,
                &resources,
                &text_rules::LayerRuleContext {
                    layer_path: path.into(),
                    schema_checked: schema_valid,
                    requested_profiles: vec![profile.into()],
                    interval_generation: generation.clone(),
                    reverse_generation: generation,
                },
            );
            self.absorb_text(path, report)?;
        }
        Ok(())
    }
    fn local_index<'a>(
        &mut self,
        path: &str,
        items: &'a [Value],
        key: &str,
        label: &str,
    ) -> Result<BTreeMap<&'a str, &'a Value>, ItemRefusal> {
        let mut result = BTreeMap::new();
        for row in items.iter().filter(|r| r.is_object()) {
            self.reserve(0)?;
            if let Some(id) = row[key].as_str() {
                self.reserve(id.len() + 48)?;
                if result.insert(id, row).is_some() {
                    self.issue(path, "duplicate-local-identity", format!("{label}:{id}"))?;
                }
                self.read(PredicateRead::UniqueKey {
                    namespace: format!("packet/{path}/{label}"),
                    key: id.into(),
                    owner: path.into(),
                })?;
            }
        }
        Ok(result)
    }
    fn local_refs(
        &mut self,
        path: &str,
        refs: Vec<&str>,
        index: &BTreeMap<&str, &Value>,
        label: &str,
    ) -> Result<(), ItemRefusal> {
        for id in refs {
            self.endpoint(path, label, id, index.contains_key(id))?;
        }
        Ok(())
    }
    fn semantic_annotation(&mut self, path: &str, v: &Value) -> Result<(), ItemRefusal> {
        let entities = self.local_index(path, rows(v, "entities"), "entity_id", "entity")?;
        let claims = self.local_index(path, rows(v, "claims"), "claim_id", "claim")?;
        let relations = self.local_index(path, rows(v, "relations"), "relation_id", "relation")?;
        let reviews = self.local_index(path, rows(v, "reviews"), "review_id", "review")?;
        let anchors = self.local_index(
            path,
            rows(&v["source_scope"], "source_anchors"),
            "anchor_ref",
            "anchor",
        )?;
        if !v["annotation_id"].is_null() && v["annotation_id"] == v["supersedes_annotation_ref"] {
            self.issue(path, "annotation-self-supersession", s(v, "annotation_id"))?;
        }
        for entity in rows(v, "entities").iter().filter(|r| r.is_object()) {
            self.reserve(0)?;
            let id = s(entity, "entity_id");
            let kind = s(entity, "entity_kind");
            let status = s(entity, "admission_status");
            let prefix = match kind {
                "occurrence" => Some("tos.occurrence."),
                "lexeme" => Some("tos.lexeme.sid-"),
                "lexical_sense" => Some("tos.sense.sid-"),
                "sign" => Some("tos.sign.sid-"),
                "concept" => Some("tos.concept.sid-"),
                _ => None,
            };
            if prefix.is_some_and(|p| !id.starts_with(p)) {
                self.issue(path, "entity-kind-namespace-drift", id)?;
            }
            if entity["entity_id"] == entity["supersedes_entity_ref"] {
                self.issue(path, "entity-self-supersession", id)?;
            }
            let basis = &entity["identity_basis"];
            self.local_refs(path, strs(basis, "anchor_refs"), &anchors, "entity-anchor")?;
            self.local_refs(path, strs(basis, "claim_refs"), &claims, "entity-claim")?;
            self.local_refs(
                path,
                strs(basis, "parent_entity_refs"),
                &entities,
                "entity-parent",
            )?;
            let review_refs = strs(entity, "admission_review_refs");
            self.local_refs(path, review_refs.clone(), &reviews, "entity-review")?;
            if decided(status) && review_refs.is_empty() {
                self.issue(path, "decided-entity-review-absent", id)?;
            }
            if accepted(status) && matches!(kind, "sign" | "concept") {
                let review_kind = if kind == "sign" {
                    "sign_promotion"
                } else {
                    "interpretive"
                };
                let selected: Vec<_> = review_refs
                    .iter()
                    .filter_map(|id| reviews.get(id).copied())
                    .filter(|r| s(r, "review_kind") == review_kind && accepting(s(r, "decision")))
                    .collect();
                if selected.is_empty() {
                    self.issue(path, "accepted-entity-review-kind-absent", id)?;
                }
                for review in selected {
                    if kind == "sign" {
                        let baseline = &review["unassisted_baseline"];
                        if baseline["required"] != true
                            || s(baseline, "status") != "frozen"
                            || baseline["frozen_before_model_suggestions"] != true
                            || !baseline["evidence_ref"].is_string()
                        {
                            self.issue(path, "sign-baseline-not-frozen", s(review, "review_id"))?;
                        }
                    }
                    let competence: BTreeMap<_, _> = rows(review, "competence")
                        .iter()
                        .filter(|r| r.is_object())
                        .map(|r| (s(r, "scope"), s(r, "status")))
                        .collect();
                    for scope in if kind == "sign" {
                        vec!["source_reading", "semantic_interpretation"]
                    } else {
                        vec!["semantic_interpretation"]
                    } {
                        if !competence.get(scope).is_some_and(|state| competent(state)) {
                            self.issue(
                                path,
                                "entity-review-competence-absent",
                                format!("{}/{scope}", s(review, "review_id")),
                            )?;
                        }
                    }
                }
            }
        }
        for claim in rows(v, "claims").iter().filter(|r| r.is_object()) {
            self.reserve(0)?;
            let id = s(claim, "claim_id");
            let status = s(claim, "claim_status");
            if claim["claim_id"] == claim["supersedes_claim_ref"] {
                self.issue(path, "semantic-claim-self-supersession", id)?;
            }
            let prop = &claim["proposition"];
            self.local_refs(
                path,
                vec![s(prop, "subject_ref")],
                &entities,
                "claim-subject",
            )?;
            let object = &prop["object"];
            if s(object, "kind") == "entity_ref" {
                self.local_refs(
                    path,
                    vec![s(object, "entity_ref")],
                    &entities,
                    "claim-object",
                )?;
            } else if s(object, "kind") == "entity_set" {
                self.local_refs(path, strs(object, "entity_refs"), &entities, "claim-object")?;
            }
            self.local_refs(
                path,
                strs(claim, "target_anchor_refs"),
                &anchors,
                "claim-anchor",
            )?;
            for evidence in rows(claim, "evidence").iter().filter(|r| r.is_object()) {
                self.local_refs(
                    path,
                    strs(evidence, "anchor_refs"),
                    &anchors,
                    "evidence-anchor",
                )?;
            }
            self.local_refs(
                path,
                strs(claim, "competing_claim_refs"),
                &claims,
                "competing-claim",
            )?;
            let review_refs = strs(claim, "review_refs");
            self.local_refs(path, review_refs.clone(), &reviews, "claim-review")?;
            for other in strs(claim, "competing_claim_refs") {
                if other == id {
                    self.issue(path, "semantic-claim-self-competition", id)?;
                } else if claims
                    .get(other)
                    .is_some_and(|c| !strs(c, "competing_claim_refs").contains(&id))
                {
                    self.issue(
                        path,
                        "semantic-claim-competition-not-reciprocal",
                        format!("{id}->{other}"),
                    )?;
                }
            }
            if decided(status) && review_refs.is_empty() {
                self.issue(path, "decided-semantic-claim-review-absent", id)?;
            }
            if accepted(status) {
                if !review_refs
                    .iter()
                    .filter_map(|id| reviews.get(id))
                    .any(|r| accepting(s(r, "decision")))
                {
                    self.issue(path, "accepted-semantic-claim-review-absent", id)?;
                }
                if s(&claim["maker"], "maker_kind") == "synthetic_fixture" {
                    self.issue(path, "accepted-synthetic-semantic-claim", id)?;
                }
            }
            if s(v, "content_posture") != "public_synthetic_contract_exercise"
                && s(&claim["maker"], "maker_kind") == "synthetic_fixture"
            {
                self.issue(path, "semantic-synthetic-maker-outside-lab", id)?;
            }
        }
        for relation in rows(v, "relations").iter().filter(|r| r.is_object()) {
            self.reserve(0)?;
            let id = s(relation, "relation_id");
            let subject = s(relation, "subject_ref");
            let object = s(relation, "object_ref");
            let claim_ref = s(relation, "claim_ref");
            self.local_refs(path, vec![subject, object], &entities, "relation-endpoint")?;
            self.local_refs(path, vec![claim_ref], &claims, "relation-claim")?;
            self.local_refs(
                path,
                strs(relation, "target_anchor_refs"),
                &anchors,
                "relation-anchor",
            )?;
            let review_refs = strs(relation, "review_refs");
            self.local_refs(path, review_refs.clone(), &reviews, "relation-review")?;
            if relation["subject_ref"] == relation["object_ref"] {
                self.issue(path, "semantic-relation-collapsed-endpoints", id)?;
            }
            let claim = claims.get(claim_ref);
            if let Some(claim) = claim {
                let prop = &claim["proposition"];
                let obj = &prop["object"];
                if s(claim, "claim_type") != "relation"
                    || prop["subject_ref"] != relation["subject_ref"]
                    || prop["predicate"] != relation["relation_type"]
                    || !obj.is_object()
                    || s(obj, "kind") != "entity_ref"
                    || obj["entity_ref"] != relation["object_ref"]
                {
                    self.issue(path, "relation-supporting-proposition-drift", id)?;
                }
            }
            if accepted(s(relation, "relation_status")) {
                if claim.is_none_or(|c| !accepted(s(c, "claim_status"))) {
                    self.issue(path, "accepted-relation-supporting-claim-not-accepted", id)?;
                }
                if !review_refs
                    .iter()
                    .filter_map(|id| reviews.get(id))
                    .any(|r| accepting(s(r, "decision")))
                {
                    self.issue(path, "accepted-semantic-relation-review-absent", id)?;
                }
            }
        }
        let graph = &v["graph_projection"];
        self.local_refs(path, strs(graph, "node_refs"), &entities, "graph-node")?;
        for edge in rows(graph, "edges").iter().filter(|r| r.is_object()) {
            self.reserve(0)?;
            let relation_ref = s(edge, "relation_ref");
            let claim_ref = s(edge, "claim_ref");
            self.local_refs(path, vec![relation_ref], &relations, "graph-relation")?;
            self.local_refs(path, vec![claim_ref], &claims, "graph-claim")?;
            self.local_refs(
                path,
                vec![s(edge, "subject_ref"), s(edge, "object_ref")],
                &entities,
                "graph-endpoint",
            )?;
            self.local_refs(
                path,
                strs(edge, "source_return_anchor_refs"),
                &anchors,
                "graph-source-return-anchor",
            )?;
            if relations
                .get(relation_ref)
                .is_none_or(|r| !accepted(s(r, "relation_status")))
            {
                self.issue(path, "graph-semantic-relation-not-accepted", relation_ref)?;
            }
            if claims
                .get(claim_ref)
                .is_none_or(|r| !accepted(s(r, "claim_status")))
            {
                self.issue(path, "graph-semantic-claim-not-accepted", claim_ref)?;
            }
            if let Some(relation) = relations.get(relation_ref) {
                if relation["claim_ref"] != edge["claim_ref"]
                    || relation["subject_ref"] != edge["subject_ref"]
                    || relation["object_ref"] != edge["object_ref"]
                {
                    self.issue(path, "graph-semantic-relation-binding-drift", relation_ref)?;
                }
            }
        }
        let rights = &v["rights_and_visibility"];
        if rights["publication_authorized"] == true
            && (rights["private_source_used"] == true
                || matches!(
                    s(rights, "source_content_visibility"),
                    "local_only" | "restricted" | "unknown"
                ))
        {
            self.issue(path, "semantic-publication-boundary-widened", path)?;
        }
        self.checked(path, "_semantic_annotation_v2_issues/v2")
    }
    fn translation_side<'a>(
        &mut self,
        path: &str,
        side: &'a Value,
        label: &str,
    ) -> Result<BTreeMap<&'a str, &'a Value>, ItemRefusal> {
        let anchors = self.local_index(path, rows(side, "anchors"), "anchor_ref", label)?;
        let mut ordinals = BTreeSet::new();
        for anchor in rows(side, "anchors").iter().filter(|r| r.is_object()) {
            self.reserve(0)?;
            if anchor["ordinal"].is_number() {
                let ordinal = integer_i64(&anchor["ordinal"])?;
                if !ordinals.insert(ordinal) {
                    self.issue(path, "alignment-duplicate-anchor-ordinal", label)?;
                }
            }
            if anchor["text_layer_ref"] != side["text_layer_ref"] {
                self.issue(
                    path,
                    "alignment-anchor-layer-escape",
                    s(anchor, "anchor_ref"),
                )?;
            }
            if anchor["text_layer_sha256"] != side["text_layer_sha256"] {
                self.issue(
                    path,
                    "alignment-anchor-layer-digest-drift",
                    s(anchor, "anchor_ref"),
                )?;
            }
            let selector = &anchor["selector"];
            if selector["start"].is_number()
                && selector["end"].is_number()
                && integer_i64(&selector["start"])? >= integer_i64(&selector["end"])?
            {
                self.issue(
                    path,
                    "alignment-anchor-selector-reversed",
                    s(anchor, "anchor_ref"),
                )?;
            }
        }
        Ok(anchors)
    }
    fn translation_alignment(&mut self, path: &str, v: &Value) -> Result<(), ItemRefusal> {
        let alignments =
            self.local_index(path, rows(v, "alignments"), "alignment_id", "alignment")?;
        let claims =
            self.local_index(path, rows(v, "alignments"), "claim_id", "alignment-claim")?;
        let reviews =
            self.local_index(path, rows(v, "reviews"), "review_id", "alignment-review")?;
        self.local_index(
            path,
            rows(v, "projections"),
            "projection_id",
            "alignment-projection",
        )?;
        let source = &v["source_side"];
        let target = &v["target_side"];
        let source_anchors = self.translation_side(path, source, "source-anchor")?;
        let target_anchors = self.translation_side(path, target, "target-anchor")?;
        for id in source_anchors
            .keys()
            .filter(|id| target_anchors.contains_key(**id))
        {
            self.issue(path, "alignment-shared-side-anchor-identity", *id)?;
        }
        if v["packet_id"] == v["supersedes_packet_ref"] {
            self.issue(
                path,
                "alignment-packet-self-supersession",
                s(v, "packet_id"),
            )?;
        }
        if source.is_object()
            && target.is_object()
            && source["expression_ref"] == target["expression_ref"]
        {
            self.issue(path, "alignment-identical-expressions", path)?;
        }
        for (side, label) in [(source, "source"), (target, "target")] {
            for key in ["segmentation", "tokenization"] {
                let binding = &side[key];
                if binding.is_object() && s(binding, "state") != "frozen" {
                    self.issue(path, "alignment-analysis-not-frozen", label)?;
                }
            }
        }
        let required: BTreeSet<_> = [
            "source_language_reading",
            "target_language_reading",
            "translation_analysis",
        ]
        .into_iter()
        .collect();
        for review in rows(v, "reviews").iter().filter(|r| r.is_object()) {
            self.reserve(0)?;
            let id = s(review, "review_id");
            let refs = strs(review, "reviewed_alignment_refs");
            self.local_refs(path, refs.clone(), &alignments, "reviewed-alignment")?;
            let scopes: Vec<_> = rows(review, "competence")
                .iter()
                .filter(|r| r.is_object())
                .map(|r| s(r, "scope"))
                .collect();
            let scope_set: BTreeSet<_> = scopes.iter().copied().collect();
            if scopes.len() != scope_set.len() || scope_set != required {
                self.issue(path, "alignment-review-competence-scope-drift", id)?;
            }
            for alignment in refs {
                if alignments
                    .get(alignment)
                    .is_some_and(|a| !strs(a, "review_refs").contains(&id))
                {
                    self.issue(
                        path,
                        "review-alignment-not-reciprocal",
                        format!("{id}->{alignment}"),
                    )?;
                }
            }
        }
        for alignment in rows(v, "alignments").iter().filter(|r| r.is_object()) {
            self.reserve(0)?;
            let id = s(alignment, "alignment_id");
            let claim_id = s(alignment, "claim_id");
            if alignment["alignment_id"] == alignment["supersedes_alignment_ref"] {
                self.issue(path, "alignment-self-supersession", id)?;
            }
            if alignment["claim_id"] == alignment["supersedes_claim_ref"] {
                self.issue(path, "alignment-claim-self-supersession", claim_id)?;
            }
            let source_refs = strs(alignment, "ordered_source_anchor_refs");
            let target_refs = strs(alignment, "ordered_target_anchor_refs");
            self.local_refs(
                path,
                source_refs.clone(),
                &source_anchors,
                "alignment-source-anchor",
            )?;
            self.local_refs(
                path,
                target_refs.clone(),
                &target_anchors,
                "alignment-target-anchor",
            )?;
            let n = source_refs.len();
            let m = target_refs.len();
            let shape = s(alignment, "correspondence_shape");
            let cardinality = match shape {
                "one_to_one" => n == 1 && m == 1,
                "one_to_many" => n == 1 && m > 1,
                "many_to_one" => n > 1 && m == 1,
                "many_to_many" => n > 1 && m > 1,
                "source_omission" => n > 0 && m == 0,
                "target_addition" => n == 0 && m > 0,
                "unresolved" => n + m > 0,
                _ => true,
            };
            if !cardinality {
                self.issue(path, "alignment-shape-cardinality-drift", id)?;
            }
            if matches!(shape, "source_omission" | "target_addition")
                && !matches!(
                    s(alignment, "order_posture"),
                    "not_applicable" | "unresolved"
                )
            {
                self.issue(path, "alignment-unaligned-order-posture", id)?;
            }
            let techniques = strs(alignment, "translation_techniques");
            if techniques.contains(&"unresolved") && techniques.len() > 1 {
                self.issue(path, "alignment-mixed-unresolved-techniques", id)?;
            }
            let competitors = strs(alignment, "competing_alignment_refs");
            self.local_refs(
                path,
                competitors.clone(),
                &alignments,
                "competing-alignment",
            )?;
            let review_refs = strs(alignment, "review_refs");
            self.local_refs(path, review_refs.clone(), &reviews, "alignment-review")?;
            for review in &review_refs {
                if reviews
                    .get(review)
                    .is_some_and(|r| !strs(r, "reviewed_alignment_refs").contains(&id))
                {
                    self.issue(
                        path,
                        "alignment-review-not-reciprocal",
                        format!("{id}->{review}"),
                    )?;
                }
            }
            for other in competitors {
                if other == id {
                    self.issue(path, "alignment-self-competition", id)?;
                } else if alignments
                    .get(other)
                    .is_some_and(|a| !strs(a, "competing_alignment_refs").contains(&id))
                {
                    self.issue(
                        path,
                        "alignment-competition-not-reciprocal",
                        format!("{id}->{other}"),
                    )?;
                }
            }
            let status = s(alignment, "status");
            let maker = s(&alignment["maker"], "maker_kind");
            if decided(status) && review_refs.is_empty() {
                self.issue(path, "decided-alignment-review-absent", id)?;
            }
            if accepted(status) {
                if matches!(
                    maker,
                    "software" | "model" | "mixed" | "imported_source" | "synthetic_fixture"
                ) {
                    self.issue(path, "alignment-direct-machine-acceptance", id)?;
                }
                let decision = if status == "accepted" {
                    "accept"
                } else {
                    "accept_with_limits"
                };
                let selected: Vec<_> = review_refs
                    .iter()
                    .filter_map(|id| reviews.get(id).copied())
                    .filter(|r| s(r, "decision") == decision)
                    .collect();
                if selected.is_empty() {
                    self.issue(path, "accepted-alignment-matching-decision-absent", id)?;
                }
                for review in selected {
                    let competence: BTreeMap<_, _> = rows(review, "competence")
                        .iter()
                        .filter(|r| r.is_object())
                        .map(|r| (s(r, "scope"), s(r, "status")))
                        .collect();
                    for scope in &required {
                        if !competence
                            .get(scope)
                            .is_some_and(|status| competent(status))
                        {
                            self.issue(
                                path,
                                "accepted-alignment-review-competence-absent",
                                format!("{}/{scope}", s(review, "review_id")),
                            )?;
                        }
                    }
                }
            }
            if s(v, "content_posture") != "public_synthetic_contract_exercise"
                && maker == "synthetic_fixture"
            {
                self.issue(path, "alignment-synthetic-maker-outside-lab", id)?;
            }
            let mut evidence_source = BTreeSet::new();
            let mut evidence_target = BTreeSet::new();
            for evidence in rows(alignment, "evidence").iter().filter(|r| r.is_object()) {
                let src = strs(evidence, "source_anchor_refs");
                let dst = strs(evidence, "target_anchor_refs");
                self.local_refs(
                    path,
                    src.clone(),
                    &source_anchors,
                    "alignment-evidence-source",
                )?;
                self.local_refs(
                    path,
                    dst.clone(),
                    &target_anchors,
                    "alignment-evidence-target",
                )?;
                evidence_source.extend(src);
                evidence_target.extend(dst);
            }
            if source_refs.iter().any(|id| !evidence_source.contains(id)) {
                self.issue(path, "alignment-source-evidence-incomplete", id)?;
            }
            if target_refs.iter().any(|id| !evidence_target.contains(id)) {
                self.issue(path, "alignment-target-evidence-incomplete", id)?;
            }
        }
        for (key, ref_key, index) in [
            ("alignment_id", "supersedes_alignment_ref", &alignments),
            ("claim_id", "supersedes_claim_ref", &claims),
        ] {
            for row in rows(v, "alignments").iter().filter(|r| r.is_object()) {
                let mut seen = BTreeSet::from([s(row, key)]);
                let mut cursor = row[ref_key].as_str();
                while let Some(id) = cursor {
                    self.reserve(0)?;
                    if !seen.insert(id) {
                        self.issue(path, "alignment-lineage-cycle", id)?;
                        break;
                    }
                    let Some(prior) = index.get(id) else {
                        self.issue(path, "alignment-lineage-predecessor-unresolved", id)?;
                        break;
                    };
                    cursor = prior[ref_key].as_str();
                }
            }
        }
        let rights = &v["rights_and_visibility"];
        if rights.is_object() {
            for (side, field) in [(source, "source_visibility"), (target, "target_visibility")] {
                if side.is_object() && rights[field] != side["visibility"] {
                    self.issue(path, "alignment-side-visibility-drift", field)?;
                }
            }
            let ranks: Option<Vec<_>> = [
                "source_visibility",
                "target_visibility",
                "packet_visibility",
            ]
            .iter()
            .map(|key| visibility_rank(s(rights, key)))
            .collect();
            if let Some(ranks) = ranks {
                if visibility_rank(s(rights, "effective_visibility")) != ranks.into_iter().max() {
                    self.issue(path, "alignment-effective-visibility-drift", path)?;
                }
            }
            if rights["publication_authorized"] == true
                && (s(rights, "effective_visibility") != "public"
                    || rights["private_source_used"] == true
                    || !source.is_object()
                    || !target.is_object()
                    || source["publication_authorized"] != true
                    || target["publication_authorized"] != true)
            {
                self.issue(path, "alignment-publication-boundary-widened", path)?;
            }
        }
        for projection in rows(v, "projections").iter().filter(|r| r.is_object()) {
            let id = s(projection, "projection_id");
            let refs = strs(projection, "source_alignment_refs");
            self.local_refs(path, refs.clone(), &alignments, "projected-alignment")?;
            for alignment in refs {
                if alignments
                    .get(alignment)
                    .is_some_and(|a| !accepted(s(a, "status")))
                {
                    self.issue(
                        path,
                        "projection-alignment-not-accepted",
                        format!("{id}->{alignment}"),
                    )?;
                }
            }
            if let (Some(packet), Some(projected)) = (
                visibility_rank(s(rights, "effective_visibility")),
                visibility_rank(s(projection, "effective_visibility")),
            ) {
                if projected < packet {
                    self.issue(path, "alignment-projection-visibility-widened", id)?;
                }
            }
        }
        self.checked(path, "_translation_alignment_v1_issues/v1")
    }
    fn semantic_ladder(&mut self, path: &str, v: &Value) -> Result<(), ItemRefusal> {
        let stages: BTreeMap<_, _> = rows(v, "stages")
            .iter()
            .filter_map(|row| row["stage"].as_str().map(|id| (id, row)))
            .collect();
        let empty = Value::Null;
        let stage = |name| stages.get(name).copied().unwrap_or(&empty);
        let active = |st: &Value| !matches!(s(st, "status"), "blocked" | "not-started");
        let result = &v["result"];
        let candidate = stage("stable_sign_candidate");
        let body = &candidate["body"];
        if body.is_object() && active(candidate) {
            if body["candidate_ref"] != v["candidate_ref"] {
                self.issue(path, "candidate-identity-drift", s(v, "candidate_ref"))?;
            }
            let mut prior = BTreeSet::new();
            for name in [
                "exact_form",
                "frequency_and_concordance",
                "context",
                "morphology",
                "lemma",
                "recurrence_within_section",
                "recurrence_within_work",
                "recurrence_within_author_corpus",
            ] {
                prior.extend(strs(&stage(name)["body"], "occurrence_refs"));
            }
            let actual = set(body, "occurrence_refs");
            if actual.is_empty() || !actual.is_subset(&prior) {
                self.issue(path, "candidate-occurrence-evidence-unresolved", path)?;
            }
        }
        let manual = stage("manual_confirmation_or_rejection");
        if s(manual, "status") == "human-accepted" {
            if !manual["body"].is_object()
                || manual["body"]["accepted_sign_ref"] != v["accepted_sign_ref"]
            {
                self.issue(path, "manual-sign-identity-drift", path)?;
            }
            if manual["body"].is_object()
                && !rows(result, "human_decision_refs")
                    .contains(&manual["body"]["review_receipt_ref"])
            {
                self.issue(path, "manual-sign-receipt-absent", path)?;
            }
        }
        let relation = stage("relations_between_signs");
        let rel_body = &relation["body"];
        let records = rows(rel_body, "relation_records");
        let relation_ids: BTreeSet<_> = records
            .iter()
            .filter_map(|r| r["relation_ref"].as_str())
            .collect();
        let claim_ids: BTreeSet<_> = records
            .iter()
            .filter_map(|r| r["claim_ref"].as_str())
            .collect();
        if !records.is_empty() {
            let endpoints: BTreeSet<_> = records
                .iter()
                .flat_map(|r| {
                    [
                        r["subject_sign_ref"].as_str(),
                        r["object_sign_ref"].as_str(),
                    ]
                })
                .flatten()
                .collect();
            if endpoints != set(rel_body, "sign_refs") {
                self.issue(path, "relation-sign-endpoint-drift", path)?;
            }
            if !relation_ids.is_subset(&set(result, "relation_refs")) {
                self.issue(path, "relation-identity-absent", path)?;
            }
            if !claim_ids.is_subset(&set(result, "claim_refs")) {
                self.issue(path, "relation-claim-absent", path)?;
            }
        }
        let concept = stage("conceptual_interpretations");
        let body = &concept["body"];
        if body.is_object() && active(concept) {
            if body["accepted_sign_ref"] != v["accepted_sign_ref"] {
                self.issue(path, "concept-sign-identity-drift", path)?;
            }
            for field in ["concept_refs", "claim_refs"] {
                if !set(body, field).is_subset(&set(result, field)) {
                    self.issue(path, "concept-result-identity-absent", field)?;
                }
            }
        }
        let counter = stage("competing_readings");
        let body = &counter["body"];
        if body.is_object() && active(counter) {
            let mut claims = set(body, "primary_claim_refs");
            claims.extend(strs(body, "competing_claim_refs"));
            if !claims.is_subset(&set(result, "claim_refs")) {
                self.issue(path, "competing-reading-claim-absent", path)?;
            }
        }
        let graph = stage("graph_projection");
        let body = &graph["body"];
        if s(graph, "status") == "projected" && body.is_object() {
            if !set(body, "relation_refs").is_subset(&relation_ids) {
                self.issue(path, "graph-relation-unresolved", path)?;
            }
            if !set(body, "claim_refs").is_subset(&set(result, "claim_refs")) {
                self.issue(path, "graph-claim-absent", path)?;
            }
            if !rows(result, "graph_projection_refs").contains(&body["projection_ref"]) {
                self.issue(path, "graph-projection-absent", path)?;
            }
        }
        self.checked(path, "_semantic_ladder_identity_issues/v4")
    }
    fn transfer(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        v: &Value,
    ) -> Result<(), ItemRefusal> {
        let mut inputs = BTreeMap::new();
        for name in [
            "transfer_plan",
            "candidate_anchor_set",
            "target_numbered_unit_map",
            "shared_label_correspondence",
            "source_rights",
            "target_rights",
        ] {
            let binding = &v["inputs"][name];
            let locator = s(binding, "ref");
            let digest = s(binding, "sha256");
            let Some(raw) = self.bytes(source, locator, Some(digest))? else {
                return Ok(());
            };
            if ![
                "transfer_plan",
                "target_numbered_unit_map",
                "shared_label_correspondence",
            ]
            .contains(&name)
            {
                continue;
            }
            let Some(input) = self.decoded(locator, &raw, "invalid-crosswalk-input")? else {
                return Ok(());
            };
            if !input.is_object() {
                self.issue(path, "crosswalk-input-object-required", name)?;
                return Ok(());
            }
            inputs.insert(name, input);
        }
        self.transfer_values(
            path,
            v,
            &inputs["transfer_plan"],
            &inputs["target_numbered_unit_map"],
            &inputs["shared_label_correspondence"],
        )?;
        self.checked(
            path,
            "_transfer_candidate_crosswalk_issues/v1-and-six-exact-input-bindings",
        )
    }
    fn transfer_values(
        &mut self,
        path: &str,
        v: &Value,
        plan: &Value,
        map: &Value,
        labels: &Value,
    ) -> Result<(), ItemRefusal> {
        let expected: BTreeMap<_, _> = rows(plan, "candidate_target_units")
            .iter()
            .filter(|r| r.is_object() && r["work_ref"] == v["work_ref"])
            .map(|r| (s(r, "unit_id"), r))
            .collect();
        let pairings: BTreeMap<_, _> = rows(labels, "pairings")
            .iter()
            .filter(|r| r.is_object())
            .map(|r| (s(r, "unit_key"), r))
            .collect();
        for row in rows(map, "unit_starts") {
            if row["pdf_page"].is_number() && row["pdf_page"].as_i64().is_none() {
                return Err(ItemRefusal::Unsupported(
                    "crosswalk integer exceeds bounded native comparison profile".into(),
                ));
            }
        }
        for row in expected.values() {
            if row["page"].is_number() && row["page"].as_i64().is_none() {
                return Err(ItemRefusal::Unsupported(
                    "crosswalk integer exceeds bounded native comparison profile".into(),
                ));
            }
        }
        let starts: Vec<_> = rows(map, "unit_starts")
            .iter()
            .filter(|r| {
                r.is_object() && r["pdf_page"].as_i64().is_some() && r["unit_key"].is_string()
            })
            .collect();
        let candidates = rows(v, "candidates");
        let actual: Vec<_> = candidates
            .iter()
            .filter(|r| r.is_object())
            .map(|r| s(r, "candidate_unit_id"))
            .collect();
        let actual_set: BTreeSet<_> = actual.iter().copied().collect();
        if actual.len() != actual_set.len() {
            self.issue(path, "crosswalk-duplicate-candidate", path)?;
        }
        if actual_set != expected.keys().copied().collect() {
            self.issue(path, "crosswalk-work-quota-drift", path)?;
        }
        let mut pairing_count = 0usize;
        let mut on_page_count = 0usize;
        for candidate in candidates {
            self.reserve(0)?;
            if !candidate.is_object() {
                self.issue(path, "crosswalk-non-object-candidate", path)?;
                continue;
            }
            let id = s(candidate, "candidate_unit_id");
            let Some(expected) = expected.get(id) else {
                continue;
            };
            for (actual, expected_key) in [
                ("candidate_anchor_ref", "anchor_ref"),
                ("target_pdf_page", "page"),
                ("stratum", "stratum"),
            ] {
                if candidate[actual] != expected[expected_key] {
                    self.issue(
                        path,
                        "crosswalk-candidate-binding-drift",
                        format!("{id}/{actual}"),
                    )?;
                }
            }
            let Some(page) = expected["page"].as_i64() else {
                self.issue(path, "crosswalk-non-integer-page", id)?;
                continue;
            };
            let prior: Vec<_> = starts
                .iter()
                .filter(|r| r["pdf_page"].as_i64().unwrap() < page)
                .collect();
            let on_page: Vec<_> = starts
                .iter()
                .filter(|r| r["pdf_page"].as_i64().unwrap() == page)
                .collect();
            let following: Vec<_> = starts
                .iter()
                .filter(|r| r["pdf_page"].as_i64().unwrap() > page)
                .collect();
            let keys: Vec<_> = prior
                .last()
                .into_iter()
                .map(|r| s(r, "unit_key"))
                .chain(on_page.iter().map(|r| s(r, "unit_key")))
                .collect();
            let on_keys: Vec<_> = on_page.iter().map(|r| s(r, "unit_key")).collect();
            if candidate["possible_unit_keys"] != json!(keys) {
                self.issue(path, "crosswalk-possible-unit-drift", id)?;
            }
            if candidate["starts_on_page_unit_keys"] != json!(on_keys) {
                self.issue(path, "crosswalk-on-page-unit-drift", id)?;
            }
            let relation = if on_page.is_empty() {
                "within-one-proposed-numbered-unit"
            } else {
                "prior-unit-spill-plus-unit-starts"
            };
            if s(candidate, "page_relation") != relation {
                self.issue(path, "crosswalk-page-relation-drift", id)?;
            }
            if let Some(next) = following.first() {
                if candidate["next_proposed_start"]
                    != json!({"unit_key":next["unit_key"],"target_pdf_page":next["pdf_page"]})
                {
                    self.issue(path, "crosswalk-next-start-drift", id)?;
                }
            } else {
                self.issue(path, "crosswalk-no-following-start", id)?;
            }
            for key in &keys {
                match pairings.get(key) {
                    None => self.issue(path, "crosswalk-no-label-pairing", *key)?,
                    Some(p) if p["translation_alignment_claimed"] != false => {
                        self.issue(path, "crosswalk-translation-alignment-widened", *key)?
                    }
                    _ => {}
                }
            }
            pairing_count += keys.len();
            on_page_count += usize::from(!on_page.is_empty());
        }
        let summary = &v["summary"];
        if !summary.is_object() {
            self.issue(path, "crosswalk-summary-object-required", path)?;
            return Ok(());
        }
        let values = [
            ("candidate_page_count", expected.len()),
            (
                "random_page_count",
                expected
                    .values()
                    .filter(|r| s(r, "stratum") == "random")
                    .count(),
            ),
            (
                "hard_page_count",
                expected
                    .values()
                    .filter(|r| s(r, "stratum") == "hard")
                    .count(),
            ),
            ("page_with_unit_start_count", on_page_count),
            (
                "page_without_unit_start_count",
                expected.len().saturating_sub(on_page_count),
            ),
            ("possible_pairing_count", pairing_count),
        ];
        for (key, count) in values {
            if summary[key] != json!(count) {
                self.issue(path, "crosswalk-summary-drift", key)?;
            }
        }
        Ok(())
    }
    fn witness(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        v: &Value,
        raw: &[u8],
    ) -> Result<(), ItemRefusal> {
        let composite = path.starts_with("ToS/source-witnesses/scholarly-composites/");
        let representation = path.ends_with("/representation.json");
        let id_key = if composite {
            "composite_id"
        } else {
            "artifact_id"
        };
        let kind = if composite {
            "scholarly-composite"
        } else {
            "artifact"
        };
        if !representation
            && !composite
            && !matches!(
                s(v, "schema_version"),
                "tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2"
            )
        {
            self.gap(path, "unknown-artifact-source-witness-profile")?;
            return Ok(());
        }
        let contract = if representation {
            if composite {
                "scholarly-composite-file-representation.schema.json"
            } else {
                "artifact-visual-representation.schema.json"
            }
        } else if composite {
            "scholarly-composite-witness.schema.json"
        } else if s(v, "schema_version") == "tos_artifact_source_witness_v2" {
            "artifact-source-witness-v2.schema.json"
        } else {
            "artifact-source-witness.schema.json"
        };
        let contract = format!("ToS/contracts/{contract}");
        if !self.schema(source, path, raw, &contract)? {
            self.issue(path, "schema", &contract)?;
        }
        self.checked(path, "Draft2020-12-owner-schema")?;
        if representation {
            self.identity(
                source,
                path,
                "source-representation/file_id",
                s(v, "file_id"),
            )?;
            if composite {
                self.identity(
                    source,
                    path,
                    "composite-representation/id",
                    s(v, "representation_id"),
                )?;
            }
            let reference = s(
                v,
                if composite {
                    "composite_ref"
                } else {
                    "artifact_ref"
                },
            );
            if let Some((owner, _)) = self.object(source, reference, None)? {
                if owner[id_key] != v[id_key] {
                    self.issue(path, "representation-owner-id-drift", reference)?;
                }
            }
        } else {
            self.identity(source, path, &format!("{kind}/id"), s(v, id_key))?;
            let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
            if parent.split('/').any(|p| {
                p.eq_ignore_ascii_case("cdli")
                    || (composite
                        && (p.eq_ignore_ascii_case("dcclt") || p.eq_ignore_ascii_case("oracc")))
            }) {
                self.issue(path, "provider-keyed-source-identity", parent)?;
            }
            for target in [s(v, "research_ref")]
                .into_iter()
                .chain(strs(v, "philosophy_planting_refs"))
            {
                self.bytes(source, target, None)?;
            }
            self.public_metadata_ceiling(path, v)?;
            if composite {
                let mut coverage = BTreeSet::new();
                for row in rows(v, "coverage_observations") {
                    if !coverage.insert((s(row, "provider"), s(row, "surface"))) {
                        self.issue(path, "duplicate-composite-coverage", s(row, "provider"))?;
                    }
                }
                // Composite member closure needs the complete source-owned artifact
                // identity namespace, not snapshot stable_ids navigation claims.
                for row in rows(v, "member_observations") {
                    let id = s(row, "member_artifact_id");
                    let present = self
                        .identities
                        .contains_key(&("artifact/id".into(), id.into()));
                    self.endpoint(path, "member-artifact", id, present)?;
                }
            }
        }
        let rights_path = s(v, "rights_ref");
        if let Some((rights, _)) = self.object(
            source,
            rights_path,
            Some("ToS/contracts/rights-record.schema.json"),
        )? {
            let required = if representation {
                if composite {
                    vec![s(v, id_key), s(v, "representation_id"), s(v, "file_id")]
                } else {
                    vec![s(v, id_key), s(v, "file_id")]
                }
            } else {
                vec![s(v, id_key)]
            };
            if rights["scope_refs"].is_array()
                && required
                    .iter()
                    .any(|id| !strs(&rights, "scope_refs").contains(id))
            {
                self.issue(path, "witness-rights-scope-drift", rights_path)?;
            }
            let visibility = if !representation {
                "public_metadata_only"
            } else if composite {
                "local_only"
            } else {
                "public_payload"
            };
            let postures: &[&str] = if !representation {
                &["metadata_only"]
            } else if composite {
                &[
                    "not_authorized",
                    "unknown",
                    "authorized_with_conditions",
                    "authorized",
                ]
            } else {
                &["authorized", "authorized_with_conditions"]
            };
            if s(&rights, "visibility") != visibility
                || !postures.contains(&s(&rights, "redistribution_posture"))
            {
                self.issue(path, "witness-rights-posture-drift", rights_path)?;
            }
        }
        let discovery_path = s(v, "discovery_ref");
        if let Some((discovery, _)) = self.object(
            source,
            discovery_path,
            Some("ToS/contracts/material-discovery-record.schema.json"),
        )? {
            if !strs(&discovery["target"], "known_tos_refs").contains(&s(v, id_key)) {
                self.issue(path, "witness-discovery-target-omits-id", discovery_path)?;
            }
            if !representation && s(&discovery["target"], "target_kind") != kind {
                self.issue(path, "witness-discovery-target-kind-drift", discovery_path)?;
            }
            self.discovery(path, &discovery)?;
        }
        let payload_path = if representation {
            self.representation_payload(source, path, v, composite)?
        } else {
            None
        };
        if !composite
            && !representation
            && s(v, "schema_version") == "tos_artifact_source_witness_v2"
        {
            self.gap(
                path,
                "native-artifact-retained-creation-software-and-event-verification",
            )?;
        } else {
            let mut required = BTreeSet::from([path.to_owned(), rights_path.to_owned()]);
            if representation {
                if let Some(payload) = &payload_path {
                    required.insert(payload.clone());
                }
            } else {
                required.insert(discovery_path.into());
                required.insert(s(v, "research_ref").into());
                required.extend(
                    strs(v, "philosophy_planting_refs")
                        .into_iter()
                        .map(str::to_owned),
                );
            }
            self.witness_event(
                source,
                path,
                s(v, "provenance_event_ref"),
                &required,
                representation,
                payload_path.as_deref(),
            )?;
        }
        self.checked(
            path,
            "witness-identity-source-refs-metadata-ceiling-and-bound-rights-discovery-posture",
        )
    }
    fn discovery(&mut self, path: &str, v: &Value) -> Result<(), ItemRefusal> {
        let mut ids = BTreeSet::new();
        let mut selected = BTreeSet::new();
        let mut rejected = BTreeSet::new();
        let mut channels = BTreeSet::new();
        let mut sequences = BTreeSet::new();
        let mut max_sequence = None;
        let mut max_general = None;
        for channel in rows(v, "channels").iter().filter(|r| r.is_object()) {
            self.reserve(0)?;
            if !channels.insert(s(channel, "channel_id")) {
                self.issue(
                    path,
                    "discovery-duplicate-channel-id",
                    s(channel, "channel_id"),
                )?;
            }
            let sequence = integer_i64(&channel["sequence"])?;
            if !sequences.insert(sequence) {
                self.issue(
                    path,
                    "discovery-duplicate-channel-sequence",
                    s(channel, "channel_id"),
                )?;
            }
            max_sequence = Some(max_sequence.map_or(sequence, |prior: i64| prior.max(sequence)));
            if s(channel, "channel_type") == "general-web-search" {
                max_general = Some(max_general.map_or(sequence, |prior: i64| prior.max(sequence)));
            }
            for (index, result) in rows(channel, "results")
                .iter()
                .filter(|r| r.is_object())
                .enumerate()
            {
                if result["rank"] != json!(index + 1) {
                    self.issue(
                        path,
                        "discovery-noncontiguous-result-rank",
                        s(channel, "channel_id"),
                    )?;
                }
                if let Some(id) = result["result_id"].as_str() {
                    if !ids.insert(id) {
                        self.issue(path, "discovery-duplicate-result-id", id)?;
                    }
                    match s(result, "decision") {
                        "select" => {
                            selected.insert(id);
                        }
                        "reject" => {
                            rejected.insert(id);
                        }
                        _ => {}
                    }
                }
            }
        }
        if max_general.is_some() && max_general != max_sequence {
            self.issue(path, "discovery-general-web-not-final", path)?;
        }
        let declared_selected = set(v, "selected_result_ids");
        let declared_rejected = set(v, "rejected_result_ids");
        if selected != declared_selected {
            self.issue(path, "discovery-selected-result-drift", path)?;
        }
        if rejected != declared_rejected {
            self.issue(path, "discovery-rejected-result-drift", path)?;
        }
        if !declared_selected.is_disjoint(&declared_rejected) {
            self.issue(path, "discovery-selected-rejected-overlap", path)?;
        }
        for id in declared_selected.union(&declared_rejected) {
            if !ids.contains(id) {
                self.issue(path, "discovery-unresolved-decision-result", *id)?;
            }
        }
        let comparison: BTreeSet<_> = rows(v, "channel_comparison")
            .iter()
            .filter(|r| r.is_object())
            .map(|r| s(r, "channel_id"))
            .collect();
        if comparison != channels {
            self.issue(path, "discovery-channel-comparison-closure-drift", path)?;
        }
        self.checked(
            path,
            "discovery-decision-channel-sequence-rank-and-comparison-closure",
        )
    }
    fn source_refs(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        v: &Value,
    ) -> Result<(), ItemRefusal> {
        let mut refs = Vec::new();
        for field in ["source_refs", "source_record_refs", "receipt_refs"] {
            refs.extend(rows(v, field));
        }
        for field in [
            "rights_ref",
            "provenance_ref",
            "forensic_report_ref",
            "resource_inventory_ref",
            "generated_from_manifest_ref",
            "item_manifest_ref",
        ] {
            if let Some(target) = v.get(field) {
                refs.push(target);
            }
        }
        for target in refs {
            if let Some(target) = target.as_str() {
                if target.starts_with("ToS/") {
                    safe(target)?;
                    let present = source.exists(
                        target,
                        self.limits.max_member_bytes,
                        self.limits.deadline,
                    )?;
                    self.endpoint(path, "current-source-reference", target, present)?;
                    self.read(PredicateRead::Prefix {
                        namespace: "current-source-member-or-directory".into(),
                        prefix: target.into(),
                        generation: source.generation(),
                    })?;
                }
            } else {
                self.issue(path, "invalid-source-reference", path)?;
            }
        }
        Ok(())
    }
    fn load_discovery_events(
        &mut self,
        source: &mut impl LayerFamilySource,
    ) -> Result<(), ItemRefusal> {
        if self.discovery_events.is_some() {
            return Ok(());
        }
        let path = "ToS/source-witnesses/discovery/provenance.jsonl";
        let Some(raw) = self.bytes(source, path, None)? else {
            return Ok(());
        };
        let text = std::str::from_utf8(&raw)
            .map_err(|_| ItemRefusal::Unsupported("discovery provenance UTF-8".into()))?;
        let mut events = BTreeMap::new();
        for (index, line) in text.lines().enumerate() {
            source.checkpoint(self.limits.deadline)?;
            if line.trim().is_empty() {
                continue;
            }
            let location = format!("{path}:{}", index + 1);
            let Ok(event) = serde_json::from_str::<Value>(line) else {
                self.issue(&location, "invalid-provenance-jsonl", path)?;
                continue;
            };
            if !event.is_object() {
                self.issue(&location, "provenance-object-required", path)?;
                continue;
            }
            let Some(id) = event["event_id"].as_str() else {
                self.issue(&location, "provenance-event-id-absent", path)?;
                continue;
            };
            self.reserve(
                line.len()
                    .checked_mul(8)
                    .ok_or(ItemRefusal::Budget)?
                    .checked_add(id.len() + location.len() + 128)
                    .ok_or(ItemRefusal::Budget)?,
            )?;
            let id = id.to_owned();
            self.read(PredicateRead::UniqueKey {
                namespace: "discovery-provenance/event_id".into(),
                key: id.clone(),
                owner: location.clone(),
            })?;
            if events
                .insert(id.clone(), (location, event, line.as_bytes().to_vec()))
                .is_some()
            {
                self.issue(path, "duplicate-discovery-event-id", id)?;
            }
        }
        self.discovery_events = Some(events);
        Ok(())
    }
    fn witness_event(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        id: &str,
        required: &BTreeSet<String>,
        digests: bool,
        payload_path: Option<&str>,
    ) -> Result<(), ItemRefusal> {
        self.load_discovery_events(source)?;
        let entry = self
            .discovery_events
            .as_ref()
            .and_then(|events| events.get(id))
            .cloned();
        self.endpoint(path, "discovery-provenance-event", id, entry.is_some())?;
        let Some((location, event, raw)) = entry else {
            return Ok(());
        };
        self.reserve(raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
        if !self.schema(
            source,
            &location,
            &raw,
            "ToS/contracts/provenance-event.schema.json",
        )? {
            self.issue(&location, "schema", "provenance-event")?;
        }
        self.source_refs(source, &location, &event)?;
        self.read(PredicateRead::ExactBytes {
            locator: location.clone(),
            digest: Digest256::of_bytes(&raw).to_hex(),
        })?;
        let outputs: BTreeMap<_, _> = rows(&event, "outputs")
            .iter()
            .filter(|r| r.is_object())
            .filter_map(|r| r["ref"].as_str().map(|id| (id, r)))
            .collect();
        for target in required {
            source.checkpoint(self.limits.deadline)?;
            let Some(output) = outputs.get(target.as_str()) else {
                self.issue(&location, "witness-provenance-output-absent", target)?;
                continue;
            };
            if !digests || !target.starts_with("ToS/") {
                continue;
            }
            if Some(target.as_str()) == payload_path {
                match source.payload(target, self.limits.max_member_bytes, self.limits.deadline)? {
                    LayerPayload::Unavailable => {
                        self.gap(
                            path,
                            "payload-output-digest-unavailable-in-selected-custody",
                        )?;
                    }
                    LayerPayload::File { sha256, .. } => {
                        if s(output, "sha256") != sha256 {
                            self.issue(
                                &location,
                                "provenance-payload-output-digest-drift",
                                target,
                            )?;
                        }
                        self.read(PredicateRead::ExactBytes {
                            locator: format!("payload:{target}"),
                            digest: sha256,
                        })?;
                    }
                }
            } else if let Some(raw) = self.bytes(source, target, None)? {
                if s(output, "sha256") != Digest256::of_bytes(&raw).to_hex() {
                    self.issue(&location, "provenance-metadata-output-digest-drift", target)?;
                }
            }
        }
        self.checked(
            path,
            "witness-discovery-provenance-exact-output-closure-and-available-output-digests",
        )
    }
    fn representation_payload(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        v: &Value,
        composite: bool,
    ) -> Result<Option<String>, ItemRefusal> {
        let payload = &v["payload"];
        let relative = s(payload, "relative_path");
        let parent = path
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        let payload_path = format!("{parent}/{relative}");
        if safe(&payload_path).is_err() || relative.is_empty() {
            self.issue(path, "representation-unsafe-payload-path", relative)?;
            return Ok(None);
        }
        if composite {
            let owner = parent
                .strip_prefix("ToS/source-witnesses/scholarly-composites/")
                .unwrap_or("");
            let parts: Vec<_> = owner.split('/').collect();
            let filename = relative.strip_prefix("payload/").unwrap_or("");
            if parts.len() != 5
                || parts[3] != "representations"
                || filename.is_empty()
                || matches!(filename, "." | "..")
                || filename.contains('/')
                || filename.contains('\\')
                || filename.contains('\0')
            {
                self.issue(path, "composite-payload-not-direct-owned-file", relative)?;
                return Ok(None);
            }
            let expected = match s(payload, "materialization_status") {
                "not_materialized" => "unmaterialized_payload",
                "materialized" => "local_gitignored_payload",
                _ => "",
            };
            if expected.is_empty()
                || s(payload, "storage_posture") != expected
                || payload["git_tracked"] != false
            {
                self.issue(path, "composite-payload-declared-posture-drift", relative)?;
            }
        }
        if s(v, "file_id") != format!("tos.file.sha256.{}", s(payload, "sha256")) {
            self.issue(
                path,
                "representation-file-id-not-content-addressed",
                s(v, "file_id"),
            )?;
        }
        let observation = source.payload(
            &payload_path,
            self.limits.max_member_bytes,
            self.limits.deadline,
        )?;
        match observation {
            LayerPayload::Unavailable => {
                if !composite || self.require_local_payloads {
                    self.issue(path, "representation-payload-unavailable", &payload_path)?;
                }
                self.gap(
                    path,
                    "representation-local-fixity-unavailable-in-selected-custody",
                )?;
            }
            LayerPayload::File {
                byte_size,
                sha256,
                source_member,
                sha1,
                jpeg_dimensions,
            } => {
                self.read(PredicateRead::ExactBytes {
                    locator: format!("payload:{payload_path}"),
                    digest: sha256.clone(),
                })?;
                if byte_size > self.limits.max_member_bytes as u64 {
                    return Err(ItemRefusal::Budget);
                }
                if payload["byte_size"] != json!(byte_size) {
                    self.issue(path, "representation-payload-size-drift", &payload_path)?;
                }
                if s(payload, "sha256") != sha256 {
                    self.issue(path, "representation-payload-sha256-drift", &payload_path)?;
                }
                if composite {
                    if source_member {
                        self.issue(
                            path,
                            "local-composite-payload-in-source-membership",
                            &payload_path,
                        )?;
                    }
                    if s(payload, "materialization_status") != "materialized" {
                        self.issue(
                            path,
                            "present-composite-payload-not-declared-materialized",
                            &payload_path,
                        )?;
                    }
                } else {
                    if !source_member {
                        self.issue(
                            path,
                            "public-artifact-payload-outside-source-membership",
                            &payload_path,
                        )?;
                    }
                    if let Some(sha1) = sha1 {
                        if s(payload, "source_sha1") != sha1 {
                            self.issue(path, "artifact-payload-sha1-drift", &payload_path)?;
                        }
                    } else {
                        self.gap(path, "artifact-source-sha1-observation-unavailable")?;
                    }
                    if let Some((width, height)) = jpeg_dimensions {
                        if payload["width_pixels"] != json!(width)
                            || payload["height_pixels"] != json!(height)
                        {
                            self.issue(path, "artifact-jpeg-dimensions-drift", &payload_path)?;
                        }
                    } else {
                        self.gap(path, "artifact-jpeg-dimensions-observation-unavailable")?;
                    }
                }
            }
        }
        self.checked(
            path,
            "representation-owned-payload-path-and-declared-custody-available-fixity",
        )?;
        Ok(Some(payload_path))
    }
    fn load_boundary_events(
        &mut self,
        source: &mut impl LayerFamilySource,
    ) -> Result<(), ItemRefusal> {
        if self.boundary_events.is_some() {
            return Ok(());
        }
        let mut events = BTreeMap::new();
        for path in [
            "ToS/source-witnesses/access-requests/provenance.jsonl",
            "ToS/source-witnesses/server-import/provenance.jsonl",
        ] {
            // This family's caller supplies the current boundary-event universe.
            // Missing unrelated streams introduce no requirement for new plans.
            let Some(raw) = self.lookup(source, path, None, false)? else {
                continue;
            };
            let text = std::str::from_utf8(&raw)
                .map_err(|_| ItemRefusal::Unsupported("boundary event UTF-8".into()))?;
            for (index, line) in text.lines().enumerate() {
                source.checkpoint(self.limits.deadline)?;
                if line.trim().is_empty() {
                    continue;
                }
                let location = format!("{path}:{}", index + 1);
                let Ok(event) = serde_json::from_str::<Value>(line) else {
                    self.issue(&location, "invalid-boundary-event-jsonl", path)?;
                    continue;
                };
                if !event.is_object() {
                    self.issue(&location, "boundary-event-object-required", path)?;
                    continue;
                }
                let Some(id) = event["event_id"].as_str() else {
                    self.issue(&location, "boundary-event-id-absent", path)?;
                    continue;
                };
                self.reserve(
                    line.len()
                        .checked_mul(8)
                        .ok_or(ItemRefusal::Budget)?
                        .checked_add(id.len() + location.len() + 128)
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                let id = id.to_owned();
                self.read(PredicateRead::UniqueKey {
                    namespace: "boundary-provenance/event_id".into(),
                    key: id.clone(),
                    owner: location.clone(),
                })?;
                if events
                    .insert(id.clone(), (location, event, line.as_bytes().to_vec()))
                    .is_some()
                {
                    self.issue(path, "duplicate-boundary-event-id", id)?;
                }
            }
        }
        self.boundary_events = Some(events);
        Ok(())
    }
    fn server_plan(
        &mut self,
        source: &mut impl LayerFamilySource,
        path: &str,
        v: &Value,
        raw: &[u8],
    ) -> Result<(), ItemRefusal> {
        // Source owner 2941d174: every present plan targets an actually
        // discovered manifest; local-only manifests need no server plan.
        let contract = "ToS/contracts/server-import-contract.schema.json";
        if !self.schema(source, path, raw, contract)? {
            self.issue(path, "schema", contract)?;
        }
        let evidence = &v["manifest"];
        if let Some(manifest_path) = evidence["ref"].as_str() {
            let present = source.discovered_item_manifest(
                manifest_path,
                self.limits.max_member_bytes,
                self.limits.deadline,
            )?;
            self.endpoint(path, "discovered-item-manifest", manifest_path, present)?;
            self.read(PredicateRead::Prefix {
                namespace: "source-foundation/discovered-item-manifests".into(),
                prefix: "ToS/source-witnesses/".into(),
                generation: source.generation(),
            })?;
            if let Some((manifest, bytes)) = self.object(source, manifest_path, None)? {
                if s(evidence, "sha256") != Digest256::of_bytes(&bytes).to_hex() {
                    self.issue(path, "server-plan-manifest-digest-drift", manifest_path)?;
                }
                if manifest["item_id"] != v["item_ref"] {
                    self.issue(path, "server-plan-item-id-drift", manifest_path)?;
                }
                let expected:Vec<_>=rows(&manifest,"payload_files").iter().filter(|r|r.is_object()).map(|entry|json!({"file_ref":entry["file_id"],"relative_path":entry["relative_path"],"byte_size":entry["byte_size"],"sha256":entry["sha256"],"verified":true})).collect();
                if v["payload_files"] != json!(expected) {
                    self.issue(path, "server-plan-payload-inventory-drift", manifest_path)?;
                }
                let policy = &v["rights_policy"];
                if policy["rights_record_ref"] != manifest["rights_ref"] {
                    self.issue(path, "server-plan-rights-ref-drift", manifest_path)?;
                } else if let Some(rights_path) = policy["rights_record_ref"].as_str() {
                    if let Some(bytes) = self.bytes(source, rights_path, None)? {
                        if s(policy, "rights_record_sha256") != Digest256::of_bytes(&bytes).to_hex()
                        {
                            self.issue(path, "server-plan-rights-digest-drift", rights_path)?;
                        }
                    }
                }
            }
        }
        if !rows(v, "provenance_event_refs").is_empty() {
            self.load_boundary_events(source)?;
        }
        for id in strs(v, "provenance_event_refs") {
            source.checkpoint(self.limits.deadline)?;
            let entry = self
                .boundary_events
                .as_ref()
                .and_then(|events| events.get(id))
                .cloned();
            self.endpoint(path, "server-plan-boundary-event", id, entry.is_some())?;
            let Some((location, event, event_raw)) = entry else {
                continue;
            };
            self.reserve(event_raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
            self.read(PredicateRead::ExactBytes {
                locator: location.clone(),
                digest: Digest256::of_bytes(&event_raw).to_hex(),
            })?;
            if !self.schema(
                source,
                &location,
                &event_raw,
                "ToS/contracts/provenance-event.schema.json",
            )? {
                self.issue(&location, "schema", "provenance-event")?;
            }
            self.source_refs(source, &location, &event)?;
            let version = match v.get("contract_version") {
                Some(value) => integer_i64(value)?,
                None => 1,
            };
            if version < 2 {
                continue;
            }
            let actual_outputs: BTreeSet<_> = rows(&event, "outputs")
                .iter()
                .filter(|r| r.is_object())
                .map(|entry| (s(entry, "ref"), s(entry, "sha256")))
                .collect();
            let digest = Digest256::of_bytes(raw).to_hex();
            if !actual_outputs.contains(&(path, digest.as_str())) {
                self.issue(path, "server-plan-provenance-output-digest-drift", id)?;
            }
            let rights = &v["rights_policy"];
            let expected = BTreeSet::from([
                (s(evidence, "ref"), s(evidence, "sha256")),
                (
                    s(rights, "rights_record_ref"),
                    s(rights, "rights_record_sha256"),
                ),
            ]);
            let actual: BTreeSet<_> = rows(&event, "inputs")
                .iter()
                .filter(|r| r.is_object())
                .map(|entry| (s(entry, "ref"), s(entry, "sha256")))
                .collect();
            if !expected.is_subset(&actual) {
                self.issue(path, "server-plan-provenance-input-digest-drift", id)?;
            }
        }
        self.checked(
            path,
            "_validate_server_import_plans/2941d174-present-plan-exact-evidence-only",
        )
    }
    fn public_metadata_ceiling(&mut self, path: &str, v: &Value) -> Result<(), ItemRefusal> {
        let mut stack = vec![v];
        while let Some(value) = stack.pop() {
            self.reserve(0)?;
            match value {
                Value::Object(map) => {
                    if map.keys().any(|key| {
                        [
                            "text",
                            "source_text",
                            "transliteration",
                            "translation",
                            "image_data",
                            "line_art_data",
                            "payload",
                        ]
                        .contains(&key.as_str())
                    }) {
                        self.issue(path, "metadata-content-exposure", path)?;
                        break;
                    }
                    stack.extend(map.values());
                }
                Value::Array(rows) => stack.extend(rows),
                Value::String(value)
                    if ["/srv/", "/home/", "/tmp/", "/var/tmp/"]
                        .iter()
                        .any(|p| value.starts_with(p)) =>
                {
                    self.issue(path, "metadata-owner-local-path-exposure", path)?;
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn finish(self) -> LayerFamilyReport {
        self.report
    }
}
fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
fn rows<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
fn strs<'a>(v: &'a Value, key: &str) -> Vec<&'a str> {
    rows(v, key).iter().filter_map(Value::as_str).collect()
}
fn set<'a>(v: &'a Value, key: &str) -> BTreeSet<&'a str> {
    strs(v, key).into_iter().collect()
}
fn safe(path: &str) -> Result<(), ItemRefusal> {
    RelativePath::parse(path)
        .map(|_| ())
        .map_err(|_| ItemRefusal::Unsupported("unsafe source-layer path".into()))
}
fn opening_scope_fields_match(scope: &Value, plan: &Value) -> bool {
    [
        "work_ref",
        "expression_ref",
        "edition_ref",
        "item_ref",
        "file_ref",
        "file_sha256",
    ]
    .iter()
    .all(|key| scope[*key] == plan[*key])
}
fn opening_scope_matches(scope: &Value, plan: &Value) -> bool {
    scope.as_object().is_some_and(|map| map.len() == 6) && opening_scope_fields_match(scope, plan)
}
fn opening_selector_matches(selector: &Value, start: &Value, end: &Value) -> bool {
    selector.as_object().is_some_and(|map| map.len() == 5)
        && selector["type"] == "text_position"
        && selector.get("start") == Some(start)
        && selector.get("end") == Some(end)
        && selector["position_unit"] == "unicode_code_point"
        && selector["interval"] == "half_open"
}

fn accepted(status: &str) -> bool {
    matches!(status, "accepted" | "accepted_with_limits")
}
fn decided(status: &str) -> bool {
    accepted(status) || matches!(status, "rejected" | "superseded")
}
fn accepting(decision: &str) -> bool {
    matches!(decision, "accept" | "accept_with_limits")
}
fn competent(status: &str) -> bool {
    matches!(status, "self_attested" | "evidence_attested")
}
fn visibility_rank(status: &str) -> Option<u8> {
    match status {
        "public" => Some(0),
        "public_metadata_only" => Some(1),
        "controlled" => Some(2),
        "local_only" => Some(3),
        "restricted" => Some(4),
        "unknown" => Some(5),
        _ => None,
    }
}
fn integer_i64(value: &Value) -> Result<i64, ItemRefusal> {
    value.as_i64().ok_or_else(|| {
        ItemRefusal::Unsupported("integer exceeds bounded native comparison profile".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn limits() -> ItemLimits {
        ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 32_000_000,
            max_state_bytes: 64_000_000,
            max_issues: 1000,
            deadline: Instant::now() + Duration::from_secs(10),
        }
    }
    struct Fixture {
        files: BTreeMap<String, Vec<u8>>,
        schemas: Vec<String>,
        lie: bool,
    }
    impl LayerFamilySource for Fixture {
        fn current(
            &mut self,
            path: &str,
            _: usize,
            _: Instant,
        ) -> Result<Option<Vec<u8>>, ItemRefusal> {
            Ok(self.files.get(path).cloned())
        }
        fn recorded(
            &mut self,
            path: &str,
            digest: &str,
            _: usize,
            _: Instant,
        ) -> Result<Option<Vec<u8>>, ItemRefusal> {
            Ok(self
                .files
                .get(path)
                .filter(|bytes| self.lie || Digest256::of_bytes(bytes).to_hex() == digest)
                .cloned())
        }
        fn schema(
            &mut self,
            _: &str,
            _: &[u8],
            contract: &str,
            _: Instant,
        ) -> Result<bool, ItemRefusal> {
            self.schemas.push(contract.into());
            Ok(true)
        }
        fn generation(&self) -> String {
            "synthetic-current-cut".into()
        }
        fn checkpoint(&self, deadline: Instant) -> Result<(), ItemRefusal> {
            if Instant::now() >= deadline {
                Err(ItemRefusal::Deadline)
            } else {
                Ok(())
            }
        }
    }
    fn crosswalk() -> (Value, Fixture) {
        let plan = json!({"candidate_target_units":[{"unit_id":"candidate","work_ref":"work","page":4,"anchor_ref":"anchor","stratum":"hard"}]});
        let map = json!({"unit_starts":[{"unit_key":"old","pdf_page":1},{"unit_key":"new","pdf_page":4},{"unit_key":"next","pdf_page":7}]});
        let labels = json!({"pairings":[{"unit_key":"old","translation_alignment_claimed":false},{"unit_key":"new","translation_alignment_claimed":false}]});
        let mut files = BTreeMap::new();
        let mut inputs = serde_json::Map::new();
        for (name, value) in [
            ("transfer_plan", plan),
            ("target_numbered_unit_map", map),
            ("shared_label_correspondence", labels),
            ("candidate_anchor_set", json!({})),
            ("source_rights", json!({})),
            ("target_rights", json!({})),
        ] {
            let path = format!("ToS/research-packets/test/{name}.json");
            let raw = serde_json::to_vec(&value).unwrap();
            inputs.insert(
                name.into(),
                json!({"ref":path,"sha256":Digest256::of_bytes(&raw).to_hex()}),
            );
            files.insert(path, raw);
        }
        files.insert(
            "ToS/contracts/transfer-candidate-structural-crosswalk.schema.json".into(),
            b"{}".to_vec(),
        );
        let value = json!({"schema_version":"tos_transfer_candidate_structural_crosswalk_v1","work_ref":"work","inputs":inputs,"candidates":[{"candidate_unit_id":"candidate","candidate_anchor_ref":"anchor","target_pdf_page":4,"stratum":"hard","possible_unit_keys":["old","new"],"starts_on_page_unit_keys":["new"],"page_relation":"prior-unit-spill-plus-unit-starts","next_proposed_start":{"unit_key":"next","target_pdf_page":7}}],"summary":{"candidate_page_count":1,"random_page_count":0,"hard_page_count":1,"page_with_unit_start_count":1,"page_without_unit_start_count":0,"possible_pairing_count":2}});
        (
            value,
            Fixture {
                files,
                schemas: Vec::new(),
                lie: false,
            },
        )
    }
    fn run(value: &Value, mut fixture: Fixture) -> (LayerFamilyReport, Fixture) {
        let path = "ToS/research-packets/test/crosswalk.json";
        fixture
            .files
            .insert(path.into(), serde_json::to_vec(value).unwrap());
        let mut rules = LayerFamilyRules::new(limits());
        rules.inspect(&mut fixture, path).unwrap();
        (rules.finish(), fixture)
    }
    #[test]
    fn transfer_source_schema_inputs_and_owner_predicates_remain_bound() {
        let (v, fixture) = crosswalk();
        let (report, fixture) = run(&v, fixture);
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        assert!(report.unsupported.is_empty());
        assert_eq!(
            fixture.schemas,
            vec!["ToS/contracts/transfer-candidate-structural-crosswalk.schema.json"]
        );
        assert!(
            report
                .checked_predicates
                .iter()
                .any(|(_, p)| p.starts_with("_transfer_candidate_crosswalk_issues"))
        );
        assert!(report.reads.iter().any(|r|matches!(r,PredicateRead::ExactBytes{locator,..} if locator.contains("target_numbered_unit_map"))));
        let (mut v, fixture) = crosswalk();
        v["candidates"][0]["next_proposed_start"]["target_pdf_page"] = json!(8);
        v["summary"]["possible_pairing_count"] = json!(1);
        let (report, _) = run(&v, fixture);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "crosswalk-next-start-drift")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "crosswalk-summary-drift")
        );
        let (v, mut fixture) = crosswalk();
        let path = v["inputs"]["shared_label_correspondence"]["ref"]
            .as_str()
            .unwrap();
        fixture.files.insert(
            path.into(),
            serde_json::to_vec(
                &json!({"pairings":[{"unit_key":"old","translation_alignment_claimed":true}]}),
            )
            .unwrap(),
        );
        let (report, _) = run(&v, fixture);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "missing-exact-input")
        );
        assert!(
            !report
                .checked_predicates
                .iter()
                .any(|(_, p)| p.starts_with("_transfer_candidate_crosswalk_issues"))
        );
    }
    #[test]
    fn retained_adapter_lies_and_state_deadlines_refuse() {
        let path = "ToS/research-packets/test/native.json";
        let mut fixture = Fixture {
            files: BTreeMap::from([(
                path.into(),
                br#"{"schema_version":"tos_source_anchor_v2","extension":"\ud800"}"#.to_vec(),
            )]),
            schemas: Vec::new(),
            lie: false,
        };
        let mut rules = LayerFamilyRules::new(limits());
        rules.inspect(&mut fixture, path).unwrap();
        let report = rules.finish();
        assert!(report.issues.is_empty());
        assert_eq!(report.unsupported.len(), 1);
        assert!(report.checked_predicates.is_empty());
        fixture.files.insert(
            path.into(),
            br#"{"schema_version":"tos_source_anchor_v2",}"#.to_vec(),
        );
        let mut rules = LayerFamilyRules::new(limits());
        rules.inspect(&mut fixture, path).unwrap();
        let report = rules.finish();
        assert!(report.unsupported.is_empty());
        assert_eq!(report.issues[0].code, "invalid-json");
        let (v, mut fixture) = crosswalk();
        fixture.lie = true;
        let path = v["inputs"]["transfer_plan"]["ref"].as_str().unwrap();
        fixture.files.insert(path.into(), b"{}".to_vec());
        let packet = "ToS/research-packets/test/crosswalk.json";
        fixture
            .files
            .insert(packet.into(), serde_json::to_vec(&v).unwrap());
        let mut rules = LayerFamilyRules::new(limits());
        assert!(matches!(
            rules.inspect(&mut fixture, packet),
            Err(ItemRefusal::Source(_))
        ));
        let mut budget = limits();
        budget.max_state_bytes = 1;
        let mut rules = LayerFamilyRules::new(budget);
        assert_eq!(
            rules.inspect(&mut fixture, packet),
            Err(ItemRefusal::Budget)
        );
        let mut budget = limits();
        budget.deadline = Instant::now();
        let mut rules = LayerFamilyRules::new(budget);
        assert_eq!(
            rules.inspect(&mut fixture, packet),
            Err(ItemRefusal::Deadline)
        );
    }
    #[test]
    fn semantic_ladder_keeps_candidate_evidence_and_graph_identities_distinct() {
        let value = json!({"candidate_ref":"candidate","accepted_sign_ref":"sign","stages":[{"stage":"exact_form","body":{"occurrence_refs":["occurrence"]}},{"stage":"stable_sign_candidate","status":"proposed","body":{"candidate_ref":"candidate","occurrence_refs":["other"]}},{"stage":"relations_between_signs","body":{"sign_refs":["sign","other-sign"],"relation_records":[{"relation_ref":"relation","claim_ref":"claim","subject_sign_ref":"sign","object_sign_ref":"other-sign"}]}},{"stage":"graph_projection","status":"projected","body":{"relation_refs":["wrong"],"claim_refs":["claim"],"projection_ref":"projection"}}],"result":{"relation_refs":["relation"],"claim_refs":["claim"],"graph_projection_refs":["projection"]}});
        let mut rules = LayerFamilyRules::new(limits());
        rules.semantic_ladder("fixture", &value).unwrap();
        let report = rules.finish();
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "candidate-occurrence-evidence-unresolved")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "graph-relation-unresolved")
        );
    }
    fn owner_fixture(relative: &str) -> Value {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        serde_json::from_slice(&std::fs::read(root.join(relative)).unwrap()).unwrap()
    }
    #[test]
    fn ordinary_semantic_and_alignment_proposals_cannot_promote_machine_results() {
        let root = "ToS/research-packets/foundation-laboratory-2026-07";
        for name in [
            "variant-a-occurrences-only.json",
            "variant-b-competing-sign-proposals.json",
        ] {
            let value = owner_fixture(&format!("{root}/semantic-annotation-v2-abc/{name}"));
            let mut rules = LayerFamilyRules::new(limits());
            rules.semantic_annotation(name, &value).unwrap();
            assert!(rules.report.issues.is_empty(), "{:?}", rules.report.issues);
        }
        let value = owner_fixture(&format!(
            "{root}/semantic-annotation-v2-abc/variant-c-invalid-model-promotion.json"
        ));
        let mut rules = LayerFamilyRules::new(limits());
        rules
            .semantic_annotation("invalid semantic promotion", &value)
            .unwrap();
        assert!(rules.report.issues.iter().any(|i| matches!(
            i.code,
            "accepted-entity-review-kind-absent"
                | "accepted-semantic-claim-review-absent"
                | "graph-semantic-claim-not-accepted"
        )));
        for name in [
            "variant-a-one-to-one-proposal.json",
            "variant-b-competing-mappings.json",
        ] {
            let value = owner_fixture(&format!("{root}/translation-alignment-v1-abc/{name}"));
            let mut rules = LayerFamilyRules::new(limits());
            rules.translation_alignment(name, &value).unwrap();
            assert!(rules.report.issues.is_empty(), "{:?}", rules.report.issues);
        }
        let value = owner_fixture(&format!(
            "{root}/translation-alignment-v1-abc/variant-c-invalid-acceptance.json"
        ));
        let mut rules = LayerFamilyRules::new(limits());
        rules
            .translation_alignment("invalid alignment acceptance", &value)
            .unwrap();
        assert!(
            rules
                .report
                .issues
                .iter()
                .any(|i| i.code == "alignment-direct-machine-acceptance")
        );
    }
    fn server_fixture(manifest_path: &str) -> (Value, Fixture) {
        let mut fixture = Fixture {
            files: BTreeMap::new(),
            schemas: Vec::new(),
            lie: false,
        };
        let rights = "ToS/source-witnesses/test/item/rights.json";
        let rights_raw = b"{}".to_vec();
        let manifest = json!({"item_id":"item","rights_ref":rights,"payload_files":[{"file_id":"file","relative_path":"payload/file.xml","byte_size":1,"sha256":"0".repeat(64)}]});
        let manifest_raw = serde_json::to_vec(&manifest).unwrap();
        let plan = json!({"manifest":{"ref":manifest_path,"sha256":Digest256::of_bytes(&manifest_raw).to_hex()},"item_ref":"item","rights_policy":{"rights_record_ref":rights,"rights_record_sha256":Digest256::of_bytes(&rights_raw).to_hex()},"payload_files":[{"file_ref":"file","relative_path":"payload/file.xml","byte_size":1,"sha256":"0".repeat(64),"verified":true}],"provenance_event_refs":[],"contract_version":2});
        fixture.files.insert(manifest_path.into(), manifest_raw);
        fixture.files.insert(rights.into(), rights_raw);
        fixture.files.insert(
            "ToS/contracts/server-import-contract.schema.json".into(),
            b"{}".to_vec(),
        );
        (plan, fixture)
    }
    #[test]
    fn present_server_plans_require_discovered_manifest_and_exact_inventory_without_universal_plan_coverage()
     {
        let manifest = "ToS/source-witnesses/test/item/item.manifest.json";
        let (plan, mut fixture) = server_fixture(manifest);
        let path = "ToS/source-witnesses/server-import/plans/test.json";
        let raw = serde_json::to_vec(&plan).unwrap();
        let mut rules = LayerFamilyRules::new(limits());
        rules.server_plan(&mut fixture, path, &plan, &raw).unwrap();
        assert!(rules.report.issues.is_empty());
        let (manipulated, mut fixture) =
            server_fixture("ToS/source-witnesses/test/item/not-a-manifest.json");
        let raw = serde_json::to_vec(&manipulated).unwrap();
        let mut rules = LayerFamilyRules::new(limits());
        rules
            .server_plan(&mut fixture, path, &manipulated, &raw)
            .unwrap();
        assert!(
            rules
                .report
                .issues
                .iter()
                .any(|i| i.code == "unresolved-local-reference"
                    && i.subject.contains("discovered-item-manifest"))
        );
        let (mut plan, mut fixture) = server_fixture(manifest);
        plan["payload_files"][0]["verified"] = json!(false);
        let raw = serde_json::to_vec(&plan).unwrap();
        let mut rules = LayerFamilyRules::new(limits());
        rules.server_plan(&mut fixture, path, &plan, &raw).unwrap();
        assert!(
            rules
                .report
                .issues
                .iter()
                .any(|i| i.code == "server-plan-payload-inventory-drift")
        );
        // A second local-only Item with no plan contributes no missing-plan issue.
        let (plan, mut fixture) = server_fixture(manifest);
        fixture.files.insert(
            "ToS/source-witnesses/test/local-only/item.manifest.json".into(),
            b"{}".to_vec(),
        );
        let mut rules = LayerFamilyRules::new(limits());
        rules
            .server_plan(
                &mut fixture,
                path,
                &plan,
                &serde_json::to_vec(&plan).unwrap(),
            )
            .unwrap();
        assert!(rules.report.issues.is_empty());
    }
}
