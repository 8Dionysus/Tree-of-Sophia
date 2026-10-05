//! Candidate validation interfaces for the ToS Rust migration.
//!
//! This crate does not admit source. `SchemaBackendProbe` evaluates a named,
//! locally supplied JSON Schema set; it cannot assert that all ToS rules or
//! invalidation dependencies are complete. A source admission caller must use
//! FND's published strict parser, exact source revisions, owner rule modules,
//! current authority fences, and CMD's atomic seal before publishing.

use std::collections::{BTreeMap, BTreeSet};

use jsonschema::{Draft, Registry};
use serde_json::Value;
use tos_foundation::{
    Digest256, Digest256Hasher, FoundationErrorCode, JsonLimits, JsonMode, JsonValue, parse_json,
};

pub mod assessment;
pub mod biblio_rules;
pub mod executor;
pub mod item_rules;
pub mod layer_family_cut;
pub mod layer_family_rules;
pub mod native_compound;
pub mod operation;
pub mod provenance_rules;
pub mod record_biblio_cut;
pub mod record_rules;
pub mod relation_rules;
pub mod retirement_rules;
pub mod rights_rules;
pub mod semantic_registry_rules;
pub mod source_copy;
pub mod source_cut;
pub mod source_forms;
pub mod source_shapes;
pub mod text_metadata_rules;
pub mod text_rules;

/// An immutable private prepare view over an exact base plus proposed delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareIdentity {
    pub exact_base_revision: String,
    pub overlay_id: String,
    pub delta_digest: String,
}

/// A versioned, code-owned rule; source descriptors cannot supply executable code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleIdentity {
    pub id: String,
    pub version: String,
    pub owner_ref: String,
    pub binary_digest: String,
}

/// Registered by owner code, never interpreted from a source record as code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDescriptor {
    pub identity: RuleIdentity,
    pub source_contract_refs: Vec<String>,
    pub engine_profile: String,
    pub scope_kind: String,
    pub dependency_schema_version: String,
    pub coverage: Coverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    FullOnly,
    ProvedAffected,
    Unsupported,
}

/// Typed reads that must participate in serializable conflict detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredicateRead {
    ExactRecord {
        id: String,
        version: String,
        digest: String,
    },
    ExactPath {
        path: String,
        digest: String,
    },
    ExactBytes {
        locator: String,
        digest: String,
    },
    IdentityKey {
        namespace: String,
        key: String,
        observed: KeyState,
    },
    RefEndpoint {
        endpoint_type: String,
        id: String,
        observed: KeyState,
    },
    AbsentKey {
        namespace: String,
        key: String,
    },
    UniqueKey {
        namespace: String,
        key: String,
        owner: String,
    },
    Range {
        namespace: String,
        lower: String,
        upper: String,
        generation: String,
    },
    Prefix {
        namespace: String,
        prefix: String,
        generation: String,
    },
    ReverseRefs {
        target: String,
        relation: String,
        generation: String,
    },
    Interval {
        scope: String,
        start: u64,
        end: u64,
        generation: String,
    },
    SchemaResource {
        uri: String,
        digest: String,
    },
    Registry {
        uri: String,
        version: String,
        digest: String,
    },
}

/// Prospective writes allow CMD to conflict-check the full candidate cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateWrite {
    Upsert {
        namespace: String,
        key: String,
        digest: String,
    },
    Remove {
        namespace: String,
        key: String,
    },
    Reserve {
        namespace: String,
        key: String,
    },
    Tombstone {
        namespace: String,
        key: String,
        digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationFact {
    pub namespace: String,
    pub key: String,
    pub value_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Present,
    Absent,
    Retired,
    Reserved,
}

/// The owner supplies the linearization protocol; an external observation alone is insufficient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityRead {
    pub owner: String,
    pub scope: String,
    pub subject: String,
    pub decision_ref: String,
    pub decision_digest: String,
    pub version_or_fence: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationTrace {
    pub prepare: PrepareIdentity,
    pub rule: RuleIdentity,
    pub schema_profile_id: String,
    pub schema_backend_digest: String,
    pub schema_set_digest: String,
    pub entity_registry_digest: String,
    pub relation_registry_digest: String,
    pub reads: Vec<PredicateRead>,
    pub authority_reads: Vec<AuthorityRead>,
    pub prospective_writes: Vec<CandidateWrite>,
    pub facts: Vec<ValidationFact>,
}

/// Stored with CMD's atomic receipt after predicates and owner fences are
/// checked against the complete coordinator cut. No publication root exists yet.
/// No external crate can construct this until the private engine and CMD
/// attestation path are implemented and expose a checked constructor.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitValidationAttestation {
    pub commit_seq: u64,
    pub coordinator_cut_digest: String,
    pub checked_predicates_digest: String,
    pub checked_rule_versions_digest: String,
    pub owner_fences_digest: String,
    pub trace_digest: String,
    pub schema_profile_id: String,
    pub schema_backend_digest: String,
}

/// Issued later by the publisher for one exact, fully materialized cut.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationValidationSeal {
    pub through_commit_seq: u64,
    pub publication_root: String,
    pub membership_root: String,
    pub index_root: String,
    pub contiguous_receipts_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndeterminateReason {
    DependencyCoverageUnknown,
    IndexIncomplete,
    AuthorityFreshnessUnproved { owner: String, scope: String },
    SchemaProfileUnsupported,
    BudgetExceeded,
}

/// No helper here upgrades `FullOnly` to `ProvedAffected` or creates an attestation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationScope {
    Full {
        membership_root: String,
        audited_rules_digest: String,
    },
    Affected {
        closure_digest: String,
        coverage_certificate_digest: String,
    },
}

/// Evidence needed before an affected result can be used by CMD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageCertificate {
    pub rule_set_digest: String,
    pub index_root: String,
    pub index_generation: String,
    pub predicate_coverage_version: String,
    pub prepare: PrepareIdentity,
    pub closure_algorithm_version: String,
    pub replayable_counts_digest: String,
}

/// `Unsupported` names missing required rule code or coverage; it is distinct
/// from an invalid judgment and a transient budget failure.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationOutcome {
    Invalid {
        issues: Vec<String>,
        trace: ValidationTrace,
    },
    Indeterminate {
        reason: IndeterminateReason,
    },
    Unsupported {
        required_profile: String,
        missing_rule_ids: Vec<String>,
    },
    #[non_exhaustive]
    MechanicallyValid {
        scope: ValidationScope,
        trace: ValidationTrace,
        coverage_certificate: Option<CoverageCertificate>,
    },
}

/// Errors from the schema *probe*, not source-admission or owner assessment errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaProbeError {
    BudgetExceeded,
    InvalidJson,
    InvalidPublishedJson(FoundationErrorCode),
    IncompatibleJsonRepresentation,
    NotSchema202012,
    InvalidResourceId,
    DuplicateResourceId,
    UnknownKeyword(String),
    MissingResource,
    Backend(String),
}

/// Only declared, exact local schema bytes enter the probe.
#[derive(Debug, Clone)]
pub struct SchemaResource {
    pub uri: String,
    pub raw: Vec<u8>,
}

/// The observed legacy profile is evidence for migration comparison only.
/// A normative profile is selected by the source owner, not by installed
/// optional format packages on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatProfile {
    LegacyPythonObserved20260923,
    AssertedSourceCandidateV1,
}

impl FormatProfile {
    pub fn id(self) -> &'static str {
        match self {
            Self::LegacyPythonObserved20260923 => {
                "tos.schema.format.legacy-python-observed-20260923"
            }
            Self::AssertedSourceCandidateV1 => "tos.schema.format.asserted-source-candidate-v1",
        }
    }
}

/// FND owns decoded-member uniqueness and the raw JSON budget. Convert its
/// strict tree directly so no second parser can erase a duplicate before the
/// probe sees it; arbitrary-precision decimal lexemes stay exact.
fn published_value(raw: &[u8], max_bytes: usize) -> Result<Value, SchemaProbeError> {
    let limits = JsonLimits::new(max_bytes, 64, 300_000, 4_300)
        .map_err(|_| SchemaProbeError::BudgetExceeded)?;
    let document = parse_json(raw, JsonMode::PublishedStrict, limits).map_err(|error| {
        if error.code == FoundationErrorCode::BudgetExceeded {
            SchemaProbeError::BudgetExceeded
        } else {
            SchemaProbeError::InvalidPublishedJson(error.code)
        }
    })?;
    fn convert(value: &JsonValue) -> Result<Value, SchemaProbeError> {
        match value {
            JsonValue::Null => Ok(Value::Null),
            JsonValue::Bool(value) => Ok(Value::Bool(*value)),
            JsonValue::Number(number) => number
                .lexeme
                .parse()
                .map(Value::Number)
                .map_err(|_| SchemaProbeError::IncompatibleJsonRepresentation),
            JsonValue::String(value) => value
                .as_str()
                .map(|text| Value::String(text.to_owned()))
                .ok_or(SchemaProbeError::IncompatibleJsonRepresentation),
            JsonValue::Array(items) => items
                .iter()
                .map(convert)
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            JsonValue::Object(entries) => {
                let mut object = serde_json::Map::new();
                for (key, value) in entries {
                    let key = key
                        .as_str()
                        .ok_or(SchemaProbeError::IncompatibleJsonRepresentation)?;
                    if object.insert(key.to_owned(), convert(value)?).is_some() {
                        return Err(SchemaProbeError::IncompatibleJsonRepresentation);
                    }
                }
                Ok(Value::Object(object))
            }
        }
    }
    convert(document.root())
}

/// Native source loaders retain last decoded field semantics, while FND owns
/// parsing budgets. A Python-admitted WTF-16 scalar gap is Unsupported, never
/// relabeled as an invalid source instance by serde's narrower representation.
pub(crate) fn native_decoded_value(
    raw: &[u8],
    max_bytes: usize,
) -> Result<Value, crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    let limits = JsonLimits::new(max_bytes, 64, 300_000, 4_300).map_err(|_| ItemRefusal::Budget)?;
    parse_json(raw, JsonMode::RequestLastWins, limits).map_err(|error| {
        if error.code == FoundationErrorCode::BudgetExceeded {
            ItemRefusal::Budget
        } else {
            ItemRefusal::Source("invalid finite native JSON".into())
        }
    })?;
    serde_json::from_slice(raw).map_err(|_| {
        ItemRefusal::Unsupported(
            "native JSON representation outside the scalar-string Rust profile".into(),
        )
    })
}

pub struct SchemaBackendProbe {
    resources: BTreeMap<String, Value>,
    resource_digests: BTreeMap<String, Digest256>,
    schema_set_digest: Digest256,
    profile: FormatProfile,
}

impl SchemaBackendProbe {
    pub const MAX_RESOURCES: usize = 512;
    pub const MAX_RESOURCE_BYTES: usize = 4 * 1024 * 1024;
    pub const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
    pub const MAX_INSTANCE_BYTES: usize = 1024 * 1024;

    pub fn new(
        resources: impl IntoIterator<Item = SchemaResource>,
        profile: FormatProfile,
    ) -> Result<Self, SchemaProbeError> {
        Self::prepare_resources(resources, profile, true)
    }

    // Only the framed diagnostics worker may defer keyword semantics until
    // after its authenticated ACK. All raw/resource identity checks are shared
    // with the public strict probe; an unchecked root must never be evaluated.
    pub(crate) fn prepare_diagnostics_resources(
        resources: impl IntoIterator<Item = SchemaResource>,
        profile: FormatProfile,
    ) -> Result<Self, SchemaProbeError> {
        Self::prepare_resources(resources, profile, false)
    }

    fn prepare_resources(
        resources: impl IntoIterator<Item = SchemaResource>,
        profile: FormatProfile,
        require_known_keywords: bool,
    ) -> Result<Self, SchemaProbeError> {
        let mut parsed = BTreeMap::new();
        let mut digests = BTreeMap::new();
        let mut total = 0usize;
        for resource in resources {
            total = total
                .checked_add(resource.raw.len())
                .ok_or(SchemaProbeError::BudgetExceeded)?;
            if parsed.len() >= Self::MAX_RESOURCES
                || resource.raw.len() > Self::MAX_RESOURCE_BYTES
                || total > Self::MAX_TOTAL_BYTES
            {
                return Err(SchemaProbeError::BudgetExceeded);
            }
            let value = published_value(&resource.raw, Self::MAX_RESOURCE_BYTES)?;
            if value.get("$schema").and_then(Value::as_str)
                != Some("https://json-schema.org/draft/2020-12/schema")
            {
                return Err(SchemaProbeError::NotSchema202012);
            }
            if value.get("$id").and_then(Value::as_str) != Some(resource.uri.as_str())
                || !resource.uri.starts_with("https://")
                || resource.uri.contains('#')
            {
                return Err(SchemaProbeError::InvalidResourceId);
            }
            if require_known_keywords {
                check_known_keywords(&value)?;
            }
            let digest = Digest256::of_bytes(&resource.raw);
            if parsed.insert(resource.uri.clone(), value).is_some() {
                return Err(SchemaProbeError::DuplicateResourceId);
            }
            digests.insert(resource.uri, digest);
        }
        // Domain-separated, length-framed exact raw resource identity. Caller
        // can bind this digest into a prepare trace; no path is reopened.
        let mut set_hasher = Digest256Hasher::new();
        set_hasher.update(b"tos-schema-set-v1\0");
        for (uri, digest) in &digests {
            set_hasher.update(&(uri.len() as u64).to_be_bytes());
            set_hasher.update(uri.as_bytes());
            set_hasher.update(digest.as_bytes());
        }
        Ok(Self {
            resources: parsed,
            resource_digests: digests,
            schema_set_digest: set_hasher.finalize(),
            profile,
        })
    }

    pub fn schema_set_digest(&self) -> Digest256 {
        self.schema_set_digest
    }

    pub fn resource_digest(&self, uri: &str) -> Option<Digest256> {
        self.resource_digests.get(uri).copied()
    }

    /// Parse raw instance bytes under FND `PublishedStrict` and a bounded
    /// profile before schema evaluation. This remains a probe, not admission.
    pub fn is_valid_raw(&self, root_uri: &str, raw: &[u8]) -> Result<bool, SchemaProbeError> {
        let instance = published_value(raw, Self::MAX_INSTANCE_BYTES)?;
        self.is_valid_value(root_uri, &instance)
    }

    /// Validate an already-decoded value with this exact local schema set.
    /// Callers must first decode source bytes with `PublishedStrict`; this
    /// entry point exists for owner routes whose strict decoder produced the
    /// `Value` before selecting a schema.
    pub fn is_valid_value(
        &self,
        root_uri: &str,
        instance: &Value,
    ) -> Result<bool, SchemaProbeError> {
        let schema = self.selected_schema(root_uri)?;
        let registry = Registry::new()
            .extend(
                self.resources
                    .iter()
                    .map(|(uri, value)| (uri.as_str(), value.clone())),
            )
            .map_err(|error| SchemaProbeError::Backend(error.to_string()))?
            .prepare()
            .map_err(|error| SchemaProbeError::Backend(error.to_string()))?;
        let mut options = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .with_registry(&registry)
            .offline()
            .should_validate_formats(true)
            .should_ignore_unknown_formats(false);
        if self.profile == FormatProfile::LegacyPythonObserved20260923 {
            options = options
                .with_format("date-time", |_| true)
                .with_format("uri", |_| true)
                .with_format("uri-reference", |_| true);
        }
        let validator = options
            .build(schema.as_ref())
            .map_err(|error| SchemaProbeError::Backend(error.to_string()))?;
        Ok(validator.is_valid(instance))
    }

    /// Compatibility name for existing in-crate callers. New source routes
    /// should use `is_valid_value` to make strict-decoding responsibility clear.
    fn is_valid(&self, root_uri: &str, instance: &Value) -> Result<bool, SchemaProbeError> {
        self.is_valid_value(root_uri, instance)
    }

    /// Keep fragment resolution inside the original registry and its $id scopes.
    fn selected_schema(
        &self,
        root_uri: &str,
    ) -> Result<std::borrow::Cow<'_, Value>, SchemaProbeError> {
        let (base_uri, fragment) = match root_uri.split_once('#') {
            Some((base, fragment))
                if !base.is_empty() && fragment.starts_with('/') && !fragment.contains('#') =>
            {
                (base, Some(fragment))
            }
            Some(_) => return Err(SchemaProbeError::InvalidResourceId),
            None => (root_uri, None),
        };
        let root_schema = self
            .resources
            .get(base_uri)
            .ok_or(SchemaProbeError::MissingResource)?;
        // Resolve a selected JSON Pointer through the original registry. Taking
        // a Value::pointer subtree and compiling it alone would lose enclosing
        // $id scopes and relative $ref resolution. This selector adds no resource
        // bytes, identity, network retrieval or alternative schema engine.
        Ok(if fragment.is_some() {
            std::borrow::Cow::Owned(serde_json::json!({"$ref": root_uri}))
        } else {
            std::borrow::Cow::Borrowed(root_schema)
        })
    }

    /// Compile the complete supplied set against one bounded, local registry.
    /// This checks backend/resource support only; no instance or owner rule is admitted.
    pub fn compile_all(&self) -> Result<usize, SchemaProbeError> {
        let registry = Registry::new()
            .extend(
                self.resources
                    .iter()
                    .map(|(uri, value)| (uri.as_str(), value.clone())),
            )
            .map_err(|error| SchemaProbeError::Backend(error.to_string()))?
            .prepare()
            .map_err(|error| SchemaProbeError::Backend(error.to_string()))?;
        for schema in self.resources.values() {
            let mut options = jsonschema::options()
                .with_draft(Draft::Draft202012)
                .with_registry(&registry)
                .offline()
                .should_validate_formats(true)
                .should_ignore_unknown_formats(false);
            if self.profile == FormatProfile::LegacyPythonObserved20260923 {
                options = options
                    .with_format("date-time", |_| true)
                    .with_format("uri", |_| true)
                    .with_format("uri-reference", |_| true);
            }
            options
                .build(schema)
                .map_err(|error| SchemaProbeError::Backend(error.to_string()))?;
        }
        Ok(self.resources.len())
    }
}

// The current authored 2020-12 corpus schemas use exactly these keyword
// positions. A future keyword is an explicit engine/owner conformance change.
fn check_schema_keyword_node<'a>(
    schema: &'a Value,
    follow_definitions: bool,
    mut child: impl FnMut(&'a Value) -> Result<(), SchemaProbeError>,
) -> Result<(), SchemaProbeError> {
    const KNOWN: &[&str] = &[
        "$defs",
        "$id",
        "$ref",
        "$schema",
        "additionalProperties",
        "allOf",
        "anyOf",
        "const",
        "contains",
        "default",
        "dependentRequired",
        "deprecated",
        "description",
        "else",
        "enum",
        "exclusiveMinimum",
        "format",
        "if",
        "items",
        "maxContains",
        "maxItems",
        "maxLength",
        "maxProperties",
        "maximum",
        "minContains",
        "minItems",
        "minLength",
        "minProperties",
        "minimum",
        "not",
        "oneOf",
        "pattern",
        "patternProperties",
        "prefixItems",
        "properties",
        "propertyNames",
        "required",
        "then",
        "title",
        "type",
        "unevaluatedProperties",
        "uniqueItems",
    ];
    const SINGLE: &[&str] = &[
        "additionalProperties",
        "unevaluatedProperties",
        "items",
        "contains",
        "not",
        "if",
        "then",
        "else",
        "propertyNames",
    ];
    const ARRAYS: &[&str] = &["allOf", "anyOf", "oneOf", "prefixItems"];
    const MAPS: &[&str] = &["$defs", "properties", "patternProperties"];
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    for (key, value) in object {
        if !KNOWN.contains(&key.as_str()) {
            return Err(SchemaProbeError::UnknownKeyword(key.clone()));
        }
        if SINGLE.contains(&key.as_str()) {
            child(value)?;
        } else if ARRAYS.contains(&key.as_str()) {
            if let Some(array) = value.as_array() {
                for item in array {
                    child(item)?;
                }
            }
        } else if MAPS.contains(&key.as_str()) && (follow_definitions || key != "$defs") {
            if let Some(map) = value.as_object() {
                for item in map.values() {
                    child(item)?;
                }
            }
        }
    }
    Ok(())
}

fn check_known_keywords(root: &Value) -> Result<(), SchemaProbeError> {
    check_schema_keyword_node(root, true, check_known_keywords)
}

// Use the same schema positions/keyword policy as the public probe, with the
// original prepared registry retaining URI scopes and JSON Pointer selectors.
// Only the requested closure is classified: unrelated resources cannot poison
// a unit, and jsonschema must not silently ignore an unknown reachable keyword.
fn check_selected_schema_keywords(
    registry: &Registry<'_>,
    root_uri: &str,
    mut charge: impl FnMut(usize) -> Result<(), SchemaProbeError>,
) -> Result<(), SchemaProbeError> {
    let backend =
        |error: jsonschema::ReferencingError| SchemaProbeError::Backend(error.to_string());
    charge(256 + root_uri.len())?;
    let base_uri = root_uri.split_once('#').map_or(root_uri, |(base, _)| base);
    let uri = jsonschema::uri::from_str(base_uri).map_err(backend)?;
    let resolver = registry.resolver(uri);
    let (root, resolver, _) = resolver.lookup(root_uri).map_err(backend)?.into_inner();
    // Charge before allocating traversal state. The caller supplies the
    // existing preparation scan-work/scan-byte budget, including mixed-call
    // remaining counters; there is no additional whole-call budget.
    charge(256 + resolver.base_uri().as_str().len())?;
    let mut pending = vec![(root, resolver)];
    let mut visited = BTreeSet::new();
    while let Some((schema, resolver)) = pending.pop() {
        // Include the resolver's static scope in cycle identity. Dynamic-ref
        // keywords remain unknown under the shared strict keyword policy.
        let base = resolver.base_uri();
        charge(256 + base.as_str().len())?;
        if !visited.insert((schema as *const Value as usize, base.as_str().to_owned())) {
            continue;
        }
        // Charge keyword visits before inspecting or allocating child state.
        if let Some(object) = schema.as_object() {
            for keyword in object.keys() {
                charge(keyword.len())?;
                charge(0)?; // Reserve the shared keyword-policy inspection too.
            }
        }
        // Definitions do not apply a schema: skip their child enumeration.
        // Resolved references bring selected definition children into closure.
        check_schema_keyword_node(schema, false, |child| {
            let id_bytes = child.get("$id").and_then(Value::as_str).map_or(0, str::len);
            charge(256 + base.as_str().len() + id_bytes)?;
            let child_resolver = resolver
                .in_subresource(Draft::Draft202012.create_resource_ref(child))
                .map_err(backend)?;
            pending.push((child, child_resolver));
            Ok(())
        })?;
        if let Some(reference) = schema.get("$ref") {
            let reference = reference.as_str().ok_or_else(|| {
                SchemaProbeError::Backend("non-string schema reference".to_owned())
            })?;
            charge(256 + base.as_str().len() + reference.len())?;
            let (target, target_resolver, _) =
                resolver.lookup(reference).map_err(backend)?.into_inner();
            pending.push((target, target_resolver));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";
    fn resource(uri: &str, body: &str) -> SchemaResource {
        SchemaResource {
            uri: uri.into(),
            raw: body.as_bytes().to_vec(),
        }
    }
    fn probe(schema: &str) -> SchemaBackendProbe {
        SchemaBackendProbe::new(
            [resource("https://tree-of-sophia.local/probe", schema)],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap()
    }

    #[test]
    fn diagnostics_keyword_refusal_follows_only_the_requested_closure() {
        let uri = "https://tree-of-sophia.local/diagnostics-keywords";
        let clean_uri = "https://tree-of-sophia.local/diagnostics-clean";
        let unsupported_uri = "https://tree-of-sophia.local/diagnostics-unsupported";
        let unknown = "x-conformance-unsupported-assertion";
        let resources = || {
            vec![
                resource(
                    uri,
                    &json!({
                        "$schema": DRAFT, "$id": uri,
                        "$defs": {
                            "supported": {"type": "integer"},
                            "unsupported": {(unknown): true},
                            "cycle": {"$ref": "#/$defs/cycle"}
                        }
                    })
                    .to_string(),
                ),
                resource(
                    clean_uri,
                    &json!({
                        "$schema": DRAFT, "$id": clean_uri,
                        "$ref": format!("{uri}#/$defs/supported")
                    })
                    .to_string(),
                ),
                resource(
                    unsupported_uri,
                    &json!({
                        "$schema": DRAFT, "$id": unsupported_uri,
                        "$ref": format!("{uri}#/$defs/unsupported")
                    })
                    .to_string(),
                ),
            ]
        };
        assert!(matches!(
            SchemaBackendProbe::new(resources(), FormatProfile::AssertedSourceCandidateV1),
            Err(SchemaProbeError::UnknownKeyword(_))
        ));
        let prepared = SchemaBackendProbe::prepare_diagnostics_resources(
            resources(),
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        let registry = Registry::new()
            .extend(
                prepared
                    .resources
                    .iter()
                    .map(|(uri, value)| (uri.as_str(), value.clone())),
            )
            .unwrap()
            .prepare()
            .unwrap();
        let scan = |root: &str| {
            let mut work = 0usize;
            let mut bytes = 0usize;
            check_selected_schema_keywords(&registry, root, |cost| {
                work += 1;
                bytes += cost;
                if work > 300_000 || bytes > SchemaBackendProbe::MAX_TOTAL_BYTES {
                    Err(SchemaProbeError::BudgetExceeded)
                } else {
                    Ok(())
                }
            })
        };
        assert_eq!(scan(uri), Ok(())); // Unreferenced definitions do not apply.
        assert_eq!(scan(clean_uri), Ok(()));
        assert_eq!(
            scan(unsupported_uri),
            Err(SchemaProbeError::UnknownKeyword(unknown.to_owned()))
        );
        assert_eq!(scan(&format!("{uri}#/$defs/supported")), Ok(()));
        assert_eq!(scan(&format!("{uri}#/$defs/cycle")), Ok(()));
        assert_eq!(
            scan(&format!("{uri}#/$defs/unsupported")),
            Err(SchemaProbeError::UnknownKeyword(unknown.to_owned()))
        );
        assert!(matches!(
            scan(&format!("{uri}#/$defs/missing")),
            Err(SchemaProbeError::Backend(_))
        ));
        assert_eq!(
            check_selected_schema_keywords(&registry, clean_uri, |_| {
                Err(SchemaProbeError::BudgetExceeded)
            }),
            Err(SchemaProbeError::BudgetExceeded)
        );
    }

    #[test]
    fn source_refs_resolve_only_from_supplied_resources() {
        let source = include_str!("../../../../ToS/contracts/source-structured-value.schema.json");
        let corpus = include_str!("../../../../ToS/contracts/corpus-record.schema.json");
        let root = "https://tree-of-sophia.local/ToS/contracts/source-structured-value.schema.json";
        let dependency = "https://tree-of-sophia.local/ToS/contracts/corpus-record.schema.json";
        let instance = json!({"kind":"historical-date", "source_wording":{"text":"1872", "language":"de", "script":null}});
        let complete = SchemaBackendProbe::new(
            [resource(root, source), resource(dependency, corpus)],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        assert_eq!(complete.is_valid(root, &instance), Ok(true));
        assert_eq!(complete.is_valid(root, &json!({"kind":"historical-date", "source_wording":{"text":"1872", "language":"bad tag", "script":null}})), Ok(false));
        assert_eq!(complete.is_valid(root, &json!({"kind":"historical-date", "source_wording":{"text":"1872", "language":"de\n", "script":null}})), Ok(false));
        let missing = SchemaBackendProbe::new(
            [resource(root, source)],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        assert!(missing.is_valid(root, &instance).is_err());
    }

    #[test]
    fn actual_source_structured_value_and_invalid_mutations() {
        // Baseline 8ca90023: relations/independent-classifications/source-claims.jsonl:3,
        // line SHA-256 4bd0abc348095ce6c630cbc6cb8225420531ad4ef1a51ab62680eebac87a7ee3.
        // Fixture keeps the source Claim's exact `object` value as test data.
        let source = include_str!("../../../../ToS/contracts/source-structured-value.schema.json");
        let corpus = include_str!("../../../../ToS/contracts/corpus-record.schema.json");
        let root = "https://tree-of-sophia.local/ToS/contracts/source-structured-value.schema.json";
        let dependency = "https://tree-of-sophia.local/ToS/contracts/corpus-record.schema.json";
        let backend = SchemaBackendProbe::new(
            [resource(root, source), resource(dependency, corpus)],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        let mut instance: Value =
            serde_json::from_str(include_str!("../fixtures/structured-value-source.json")).unwrap();
        assert_eq!(backend.is_valid(root, &instance), Ok(true));
        instance["source_wording"]["language"] = json!("not a language tag");
        assert_eq!(backend.is_valid(root, &instance), Ok(false));
        instance["source_wording"]["language"] = json!("en");
        instance["source_wording"]["text"] = json!("   ");
        assert_eq!(backend.is_valid(root, &instance), Ok(false));
    }

    #[test]
    fn all_current_owner_contract_schemas_compile_from_local_resources() {
        let contract_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../ToS/contracts");
        let mut resources = Vec::new();
        let mut paths = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeMap::new();
        for entry in std::fs::read_dir(contract_dir).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let filename = entry.file_name().to_string_lossy().into_owned();
            assert!(
                filename.ends_with(".schema.json"),
                "every owner contract JSON must be an explicit schema resource: {filename}"
            );
            assert!(paths.insert(filename.clone()), "duplicate contract path");
            let raw = std::fs::read(entry.path()).unwrap();
            let value: Value = serde_json::from_slice(&raw).unwrap();
            assert_eq!(
                value.get("$schema").and_then(Value::as_str),
                Some(DRAFT),
                "all enumerated owner contract JSON must declare the supported dialect"
            );
            let uri = value.get("$id").and_then(Value::as_str).unwrap().to_owned();
            ids.insert(filename, uri.clone());
            resources.push(SchemaResource { uri, raw });
        }
        let expected = resources.len();
        for (required, uri) in [
            (
                "corpus-record.schema.json",
                "https://tree-of-sophia.local/ToS/contracts/corpus-record.schema.json",
            ),
            (
                "human-form.schema.json",
                "https://treeofsophia.local/ToS/contracts/human-form.schema.json",
            ),
            (
                "human-form-set.schema.json",
                "https://treeofsophia.local/ToS/contracts/human-form-set.schema.json",
            ),
            (
                "provenance-event.schema.json",
                "https://tree-of-sophia.local/ToS/contracts/provenance-event.schema.json",
            ),
            (
                "source-structured-value.schema.json",
                "https://tree-of-sophia.local/ToS/contracts/source-structured-value.schema.json",
            ),
        ] {
            assert!(
                paths.contains(required) && ids.get(required).map(String::as_str) == Some(uri),
                "missing or changed required owner contract: {required}"
            );
        }
        let backend =
            SchemaBackendProbe::new(resources, FormatProfile::AssertedSourceCandidateV1).unwrap();
        assert_eq!(backend.compile_all(), Ok(expected));
    }

    #[test]
    fn actual_provenance_event_exposes_legacy_format_gap() {
        // Baseline 8ca90023, acquired-note-utf8-20260910/provenance.jsonl:2,
        // line SHA-256 673d2fadd5b437c79f298f56321e3a7eff16a4b32673c3705e612c106e605410.
        let root = "https://tree-of-sophia.local/ToS/contracts/provenance-event.schema.json";
        let schema = include_str!("../../../../ToS/contracts/provenance-event.schema.json");
        let instance: Value =
            serde_json::from_str(include_str!("../fixtures/provenance-event-source.json")).unwrap();
        let strict = SchemaBackendProbe::new(
            [resource(root, schema)],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        let legacy = SchemaBackendProbe::new(
            [resource(root, schema)],
            FormatProfile::LegacyPythonObserved20260923,
        )
        .unwrap();
        assert_eq!(strict.is_valid(root, &instance), Ok(true));
        assert_eq!(legacy.is_valid(root, &instance), Ok(true));
        let mut invalid = instance.clone();
        invalid["ended_at"] = json!("not-a-date-time");
        assert_eq!(strict.is_valid(root, &invalid), Ok(false));
        assert_eq!(legacy.is_valid(root, &invalid), Ok(true));
    }

    #[test]
    fn source_keyword_families_have_independent_positive_and_negative_cases() {
        let schema = format!(
            r#"{{"$schema":"{DRAFT}","$id":"https://tree-of-sophia.local/probe","type":"object","properties":{{"mode":{{"enum":["strict","loose"]}},"items":{{"type":"array","contains":{{"type":"integer","minimum":1}},"minContains":2,"maxContains":2,"uniqueItems":true}},"seal":{{"type":"boolean"}}}},"required":["mode","items"],"if":{{"properties":{{"mode":{{"const":"strict"}}}}}},"then":{{"required":["seal"]}},"else":{{"not":{{"required":["seal"]}}}},"unevaluatedProperties":false}}"#
        );
        let backend = probe(&schema);
        let root = "https://tree-of-sophia.local/probe";
        assert_eq!(
            backend.is_valid(root, &json!({"mode":"strict","items":[1,2],"seal":true})),
            Ok(true)
        );
        assert_eq!(
            backend.is_valid(root, &json!({"mode":"strict","items":[1,2]})),
            Ok(false)
        );
        assert_eq!(
            backend.is_valid(root, &json!({"mode":"loose","items":[1,2],"seal":true})),
            Ok(false)
        );
        assert_eq!(
            backend.is_valid(root, &json!({"mode":"loose","items":[1,1]})),
            Ok(false)
        );
        assert_eq!(
            backend.is_valid(root, &json!({"mode":"loose","items":[0,1]})),
            Ok(false)
        );
        assert_eq!(
            backend.is_valid(root, &json!({"mode":"loose","items":[1,2,3]})),
            Ok(false)
        );
    }

    #[test]
    fn numbers_and_formats_use_declared_profile() {
        let schema = format!(
            r#"{{"$schema":"{DRAFT}","$id":"https://tree-of-sophia.local/probe","type":"object","properties":{{"n":{{"type":"integer","minimum":9007199254740992,"maximum":9007199254740994}},"when":{{"type":"string","format":"date-time"}}}},"required":["n","when"]}}"#
        );
        let backend = probe(&schema);
        let root = "https://tree-of-sophia.local/probe";
        let valid: Value =
            serde_json::from_str(r#"{"n":9007199254740993,"when":"2026-09-23T00:00:00Z"}"#)
                .unwrap();
        let low: Value =
            serde_json::from_str(r#"{"n":9007199254740991,"when":"2026-09-23T00:00:00Z"}"#)
                .unwrap();
        let high: Value =
            serde_json::from_str(r#"{"n":9007199254740995,"when":"2026-09-23T00:00:00Z"}"#)
                .unwrap();
        assert_eq!(backend.is_valid(root, &valid), Ok(true));
        assert_eq!(backend.is_valid(root, &low), Ok(false));
        assert_eq!(backend.is_valid(root, &high), Ok(false));
        assert_eq!(
            backend.is_valid(root, &json!({"n":9007199254740993u64,"when":"not-a-date"})),
            Ok(false)
        );
        let legacy = SchemaBackendProbe::new(
            [resource(root, &schema)],
            FormatProfile::LegacyPythonObserved20260923,
        )
        .unwrap();
        assert_eq!(
            legacy.is_valid(root, &json!({"n":9007199254740993u64,"when":"not-a-date"})),
            Ok(true)
        );

        let exact = format!(
            r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"integer","minimum":18446744073709551616,"maximum":18446744073709551618}}"#
        );
        let backend = probe(&exact);
        let middle: Value = serde_json::from_str("18446744073709551617").unwrap();
        let below: Value = serde_json::from_str("18446744073709551615").unwrap();
        let above: Value = serde_json::from_str("18446744073709551619").unwrap();
        assert_eq!(backend.is_valid(root, &middle), Ok(true));
        assert_eq!(backend.is_valid(root, &below), Ok(false));
        assert_eq!(backend.is_valid(root, &above), Ok(false));

        let integer = format!(r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"integer"}}"#);
        let backend = probe(&integer);
        assert_eq!(
            backend.is_valid(root, &serde_json::from_str("3.0").unwrap()),
            Ok(true)
        );
        assert_eq!(
            backend.is_valid(root, &serde_json::from_str("3.5").unwrap()),
            Ok(false)
        );
    }

    #[test]
    fn four_source_formats_have_explicit_profiles() {
        let root = "https://tree-of-sophia.local/probe";
        let schema = format!(
            r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"object","properties":{{"day":{{"type":"string","format":"date"}},"moment":{{"type":"string","format":"date-time"}},"page":{{"type":"string","format":"uri"}},"relative":{{"type":"string","format":"uri-reference"}}}},"required":["day","moment","page","relative"]}}"#
        );
        let strict = SchemaBackendProbe::new(
            [resource(root, &schema)],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        let legacy = SchemaBackendProbe::new(
            [resource(root, &schema)],
            FormatProfile::LegacyPythonObserved20260923,
        )
        .unwrap();
        let good = json!({"day":"2026-09-23","moment":"2026-09-23T00:00:00Z","page":"https://example.org/x","relative":"/x"});
        assert_eq!(strict.is_valid(root, &good), Ok(true));
        assert_eq!(legacy.is_valid(root, &good), Ok(true));
        let invalid_day = json!({"day":"2026-02-30","moment":"2026-09-23T00:00:00Z","page":"https://example.org/x","relative":"/x"});
        assert_eq!(strict.is_valid(root, &invalid_day), Ok(false));
        assert_eq!(legacy.is_valid(root, &invalid_day), Ok(false));
        let other_invalid = json!({"day":"2026-09-23","moment":"not-a-date","page":"has space","relative":"bad space"});
        assert_eq!(strict.is_valid(root, &other_invalid), Ok(false));
        assert_eq!(legacy.is_valid(root, &other_invalid), Ok(true));
    }

    #[test]
    fn resource_identity_unknown_keyword_and_absent_resource_fail_closed() {
        let root = "https://tree-of-sophia.local/probe";
        let simple = format!(r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"string"}}"#);
        assert_eq!(
            SchemaBackendProbe::new(
                [resource(root, &simple), resource(root, &simple)],
                FormatProfile::AssertedSourceCandidateV1
            )
            .err(),
            Some(SchemaProbeError::DuplicateResourceId)
        );
        assert_eq!(
            SchemaBackendProbe::new(
                [resource("https://other.local/probe", &simple)],
                FormatProfile::AssertedSourceCandidateV1
            )
            .err(),
            Some(SchemaProbeError::InvalidResourceId)
        );
        let unknown = format!(r#"{{"$schema":"{DRAFT}","$id":"{root}","unrecognizedRule":true}}"#);
        assert_eq!(
            SchemaBackendProbe::new(
                [resource(root, &unknown)],
                FormatProfile::AssertedSourceCandidateV1
            )
            .err(),
            Some(SchemaProbeError::UnknownKeyword("unrecognizedRule".into()))
        );
        let missing = format!(
            r#"{{"$schema":"{DRAFT}","$id":"{root}","$ref":"https://tree-of-sophia.local/unavailable"}}"#
        );
        assert!(probe(&missing).is_valid(root, &json!("x")).is_err());
        let unavailable_format = format!(
            r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"string","format":"unavailable-format"}}"#
        );
        assert!(
            probe(&unavailable_format)
                .is_valid(root, &json!("x"))
                .is_err()
        );
    }

    #[test]
    fn selected_json_pointer_keeps_original_resource_scope_and_failures() {
        let root = "https://tree-of-sophia.local/schemas/root.json";
        let types = "https://tree-of-sophia.local/schemas/nested/types.json";
        let source = format!(
            r#"{{"$schema":"{DRAFT}","$id":"{root}","$defs":{{"scoped":{{"$id":"nested/child.json","properties":{{"a/b~c":{{"$ref":"types.json#/$defs/code"}}}}}},"allow":true,"deny":false}}}}"#
        );
        let target = format!(
            r#"{{"$schema":"{DRAFT}","$id":"{types}","$defs":{{"code":{{"type":"string","const":"owned"}}}}}}"#
        );
        let backend = SchemaBackendProbe::new(
            [resource(root, &source), resource(types, &target)],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        let digest = backend.schema_set_digest();
        let selected = format!("{root}#/$defs/scoped/properties/a~1b~0c");
        assert_eq!(backend.is_valid_raw(&selected, br#""owned""#), Ok(true));
        assert_eq!(backend.is_valid_raw(&selected, br#""other""#), Ok(false));
        assert_eq!(
            backend.is_valid_raw(&format!("{root}#/$defs/allow"), b"3"),
            Ok(true)
        );
        assert_eq!(
            backend.is_valid_raw(&format!("{root}#/$defs/deny"), b"3"),
            Ok(false)
        );
        assert!(
            backend
                .is_valid_raw(&format!("{root}#/$defs/absent"), b"3")
                .is_err()
        );
        assert_eq!(
            backend.is_valid_raw(&format!("{root}#named-anchor"), b"3"),
            Err(SchemaProbeError::InvalidResourceId)
        );
        assert_eq!(
            backend.is_valid_raw("https://tree-of-sophia.local/absent#/type", b"3"),
            Err(SchemaProbeError::MissingResource)
        );
        assert_eq!(backend.schema_set_digest(), digest);
        assert_eq!(
            backend.resource_digest(root),
            Some(Digest256::of_bytes(source.as_bytes()))
        );
    }

    #[test]
    fn published_schema_resources_reject_decoded_duplicate_members() {
        let root = "https://tree-of-sophia.local/probe";
        let literal =
            format!(r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"string","type":"integer"}}"#);
        let escaped = format!(
            r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"string","typ\u0065":"integer"}}"#
        );
        for raw in [literal, escaped] {
            assert_eq!(
                SchemaBackendProbe::new(
                    [resource(root, &raw)],
                    FormatProfile::AssertedSourceCandidateV1
                )
                .err(),
                Some(SchemaProbeError::InvalidPublishedJson(
                    FoundationErrorCode::DuplicateMember
                ))
            );
        }
        let oversized = SchemaResource {
            uri: root.into(),
            raw: vec![b' '; SchemaBackendProbe::MAX_RESOURCE_BYTES + 1],
        };
        assert_eq!(
            SchemaBackendProbe::new([oversized], FormatProfile::AssertedSourceCandidateV1).err(),
            Some(SchemaProbeError::BudgetExceeded)
        );
    }

    #[test]
    fn exact_schema_resources_are_frozen_after_construction() {
        let root = "https://tree-of-sophia.local/probe";
        let raw = format!(r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"string"}}"#);
        let mut owner_copy = raw.as_bytes().to_vec();
        let backend = SchemaBackendProbe::new(
            [SchemaResource {
                uri: root.into(),
                raw: owner_copy.clone(),
            }],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        let resource_digest = backend.resource_digest(root).unwrap();
        let set_digest = backend.schema_set_digest();
        assert_eq!(resource_digest, Digest256::of_bytes(raw.as_bytes()));
        owner_copy.fill(b' ');
        assert_eq!(backend.resource_digest(root), Some(resource_digest));
        assert_eq!(backend.schema_set_digest(), set_digest);
        assert_eq!(backend.is_valid_raw(root, br#""okay""#), Ok(true));
        assert_eq!(backend.is_valid_raw(root, b"42"), Ok(false));
        let changed = format!(r#"{{"$schema":"{DRAFT}","$id":"{root}", "type":"string"}}"#);
        let changed = SchemaBackendProbe::new(
            [resource(root, &changed)],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        assert_ne!(changed.schema_set_digest(), set_digest);
    }

    #[test]
    fn raw_instance_bridge_preserves_precision_and_rejects_ambiguous_json() {
        let root = "https://tree-of-sophia.local/probe";
        let ranged = format!(
            r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"object","properties":{{"n":{{"type":"integer","minimum":18446744073709551616,"maximum":18446744073709551618}}}},"required":["n"]}}"#
        );
        let backend = probe(&ranged);
        assert_eq!(
            backend.is_valid_raw(root, br#"{"n":18446744073709551617}"#),
            Ok(true)
        );
        assert_eq!(
            backend.is_valid_raw(root, br#"{"n":18446744073709551615}"#),
            Ok(false)
        );
        assert_eq!(
            backend.is_valid_raw(root, br#"{"n":18446744073709551619}"#),
            Ok(false)
        );
        for raw in [
            &br#"{"n":18446744073709551617,"n":0}"#[..],
            &br#"{"n":18446744073709551617,"\u006e":0}"#[..],
        ] {
            assert_eq!(
                backend.is_valid_raw(root, raw),
                Err(SchemaProbeError::InvalidPublishedJson(
                    FoundationErrorCode::DuplicateMember
                ))
            );
        }
        assert_eq!(
            backend.is_valid_raw(root, b"{\"n\":\"\xff\"}"),
            Err(SchemaProbeError::InvalidPublishedJson(
                FoundationErrorCode::InvalidUtf8
            ))
        );
        assert_eq!(
            backend.is_valid_raw(root, br#"{"n":"\ud800"}"#),
            Err(SchemaProbeError::IncompatibleJsonRepresentation)
        );
        assert_eq!(
            backend.is_valid_raw(
                root,
                &vec![b' '; SchemaBackendProbe::MAX_INSTANCE_BYTES + 1]
            ),
            Err(SchemaProbeError::BudgetExceeded)
        );
        let nested = format!("{}null{}", "[".repeat(65), "]".repeat(65));
        assert_eq!(
            backend.is_valid_raw(root, nested.as_bytes()),
            Err(SchemaProbeError::BudgetExceeded)
        );
        let huge_integer = "1".repeat(4_301);
        assert_eq!(
            backend.is_valid_raw(root, huge_integer.as_bytes()),
            Err(SchemaProbeError::BudgetExceeded)
        );
        let too_many_values = format!("[{}]", vec!["0"; 300_000].join(","));
        assert_eq!(
            backend.is_valid_raw(root, too_many_values.as_bytes()),
            Err(SchemaProbeError::BudgetExceeded)
        );

        let integer = format!(r#"{{"$schema":"{DRAFT}","$id":"{root}","type":"integer"}}"#);
        let backend = probe(&integer);
        assert_eq!(backend.is_valid_raw(root, b"3.0"), Ok(true));
        assert_eq!(backend.is_valid_raw(root, b"3.5"), Ok(false));
        assert_eq!(backend.is_valid_raw(root, b"-0"), Ok(true));
        assert_eq!(backend.is_valid_raw(root, b"3e0"), Ok(true));
    }

    #[test]
    fn raw_source_fixture_and_format_profiles_match_independent_oracle() {
        let root = "https://tree-of-sophia.local/ToS/contracts/provenance-event.schema.json";
        let schema = include_str!("../../../../ToS/contracts/provenance-event.schema.json");
        let strict = SchemaBackendProbe::new(
            [resource(root, schema)],
            FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        let legacy = SchemaBackendProbe::new(
            [resource(root, schema)],
            FormatProfile::LegacyPythonObserved20260923,
        )
        .unwrap();
        let source = include_bytes!("../fixtures/provenance-event-source.json");
        assert_eq!(strict.is_valid_raw(root, source), Ok(true));
        assert_eq!(legacy.is_valid_raw(root, source), Ok(true));
        let changed = String::from_utf8(source.to_vec())
            .unwrap()
            .replace("2026-09-10T01:53:38.746445+00:00", "not-a-date-time");
        assert_ne!(changed.as_bytes(), source);
        assert_eq!(strict.is_valid_raw(root, changed.as_bytes()), Ok(false));
        assert_eq!(legacy.is_valid_raw(root, changed.as_bytes()), Ok(true));
    }

    #[test]
    fn prepare_trace_is_not_a_committed_seal() {
        let trace = ValidationTrace {
            prepare: PrepareIdentity {
                exact_base_revision: "base".into(),
                overlay_id: "private-overlay".into(),
                delta_digest: "delta".into(),
            },
            rule: RuleIdentity {
                id: "owner.rule".into(),
                version: "1".into(),
                owner_ref: "ToS/contracts/example".into(),
                binary_digest: "binary".into(),
            },
            schema_profile_id: FormatProfile::AssertedSourceCandidateV1.id().into(),
            schema_backend_digest: "schema-backend".into(),
            schema_set_digest: "schema-set".into(),
            entity_registry_digest: "entity-registry".into(),
            relation_registry_digest: "relation-registry".into(),
            reads: vec![PredicateRead::AbsentKey {
                namespace: "source-id".into(),
                key: "example".into(),
            }],
            authority_reads: vec![AuthorityRead {
                owner: "rights".into(),
                scope: "item/example".into(),
                subject: "example".into(),
                decision_ref: "decision".into(),
                decision_digest: "digest".into(),
                version_or_fence: "fence".into(),
                expires_at: None,
            }],
            prospective_writes: vec![CandidateWrite::Reserve {
                namespace: "source-id".into(),
                key: "example".into(),
            }],
            facts: vec![],
        };
        assert_ne!(trace.prepare.overlay_id, trace.prepare.exact_base_revision);
        let outcome = ValidationOutcome::Indeterminate {
            reason: IndeterminateReason::AuthorityFreshnessUnproved {
                owner: "rights".into(),
                scope: "item/example".into(),
            },
        };
        assert!(matches!(outcome, ValidationOutcome::Indeterminate { .. }));
    }
}

pub mod source_witness_foundation;

pub mod source_foundation_schema;

pub mod source_foundation_records;

pub mod source_foundation_labs;

pub mod source_foundation_goldsets;

pub mod source_foundation_discovery;

pub mod source_foundation_closure;

pub mod source_foundation_default_rules;

pub mod source_record_selection;
