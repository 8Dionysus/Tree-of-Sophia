//! Source-derived relation checks for an explicitly supplied complete candidate view.
//!
//! This is a family-local shadow. The caller supplies exact raw bytes and the
//! current union; neither this module nor a successful `RelationShadow` can
//! construct a corpus admission result or attest that the supplied union is
//! complete. Special source profiles remain named, explicit gaps.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use tos_foundation::Digest256;

#[cfg(feature = "native")]
use crate::biblio_rules::SourceFoundationBiblioManifestSink;
#[cfg(feature = "native")]
use crate::source_foundation_default_rules::{
    SourceFoundationDefaultClaims, SourceFoundationDefaultRecordsLookup,
};
use crate::{
    KeyState, PredicateRead, SchemaBackendProbe, SchemaProbeError, ValidationFact, published_value,
};

const MAX_REGISTRY_BYTES: usize = 1_048_576;
const MAX_CLAIM_FILE_BYTES: usize = 16_777_216;
const MAX_CLAIM_BYTES: usize = 1_048_576;
const MAX_RECORD_BYTES: usize = 1_048_576;
const MAX_CLAIM_STREAMS: usize = 4_096;
const MAX_TOTAL_CLAIM_BYTES: usize = 268_435_456;
const MAX_CLAIMS: usize = 65_536;
const MAX_ENDPOINTS: usize = 65_536;
const MAX_TOPOLOGY_OBJECTS: usize = 65_536;
const MAX_PROVENANCE_REFERENCES: usize = 65_536;
const MAX_READS: usize = 262_144;
const MAX_FACTS: usize = 65_536;
const MAX_ISSUES: usize = 64;
const MAX_SKIPPED_PROFILES: usize = 1_024;
const MAX_SOURCE_REF_BYTES: usize = 4_096;
const MAX_GENERATION_BYTES: usize = 256;
const ENTITY_CONTRACT: &str =
    "https://treeofsophia.local/ToS/contracts/semantic-entity-type-registry.schema.json";
const RELATION_CONTRACT: &str =
    "https://treeofsophia.local/ToS/contracts/semantic-relation-type-registry.schema.json";
const CLAIM_BASE: &str =
    "https://tree-of-sophia.local/ToS/contracts/source-claim-record.schema.json";
const PROVENANCE_CONTRACT: &str =
    "https://tree-of-sophia.local/ToS/contracts/provenance-event.schema.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationIssue {
    pub code: &'static str,
    pub location: String,
}

/// `unsupported` and `skipped_profiles` are mandatory consumer gates. Empty
/// issues are only a local observation, never a source-admission verdict.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelationShadow {
    pub reads: Vec<PredicateRead>,
    pub facts: Vec<ValidationFact>,
    pub issues: Vec<RelationIssue>,
    pub declared_profiles: BTreeSet<String>,
    pub checked_profiles: BTreeSet<String>,
    pub skipped_profiles: BTreeSet<String>,
    pub unsupported: bool,
    pub issue_sink_truncated: bool,
    pub observed_claims: usize,
    pub observed_endpoints: usize,
}

impl RelationShadow {
    fn issue(&mut self, code: &'static str, location: impl Into<String>) {
        if self.issues.len() < MAX_ISSUES {
            self.issues.push(RelationIssue {
                code,
                location: location.into(),
            });
        } else {
            self.issue_sink_truncated = true;
            self.unsupported = true;
        }
    }

    fn skip(&mut self, profile: impl Into<String>) {
        self.unsupported = true;
        if self.skipped_profiles.len() < MAX_SKIPPED_PROFILES {
            self.skipped_profiles.insert(profile.into());
        } else {
            self.skipped_profiles
                .insert("skipped-profile-sink-capacity".into());
        }
    }

    fn read(&mut self, read: PredicateRead) {
        if self.reads.len() < MAX_READS {
            self.reads.push(read);
        } else {
            self.skip("predicate-read-sink-capacity");
        }
    }

    fn fact(&mut self, fact: ValidationFact) {
        if self.facts.len() < MAX_FACTS {
            self.facts.push(fact);
        } else {
            self.skip("validation-fact-sink-capacity");
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationError {
    BudgetExceeded,
    InvalidPublishedJson(SchemaProbeError),
    MalformedRegistry,
    SchemaBackend(SchemaProbeError),
}

pub struct ClaimStream<'a> {
    pub source_ref: &'a str,
    pub raw: &'a [u8],
}

pub struct EndpointRecord<'a> {
    pub source_ref: &'a str,
    pub raw: &'a [u8],
}

/// `complete_union` is a caller declaration, not a completeness certificate.
/// A future full auditor must verify the source membership root independently.
pub struct ClaimFamilyInput<'a> {
    pub entity_registry_raw: &'a [u8],
    pub relation_registry_raw: &'a [u8],
    pub claim_streams: &'a [ClaimStream<'a>],
    pub endpoints: &'a [EndpointRecord<'a>],
    pub complete_union: bool,
    pub union_generation: &'a str,
}

struct ClaimRoute {
    reader: String,
    domain: Vec<String>,
    range: Vec<String>,
    versions: BTreeMap<String, String>,
    profile: Value,
}

impl ClaimRoute {
    fn generic_endpoint(&self, schema_ref: &str) -> bool {
        (self.reader == "identity-relation-v1"
            && schema_ref == "ToS/contracts/source-relation-claim.schema.json")
            || (self.reader == "semantic-relation-v1"
                && schema_ref == "ToS/contracts/semantic-relation-claim.schema.json")
    }
}

fn field_str<'a>(row: &'a Value, field: &str) -> Option<&'a str> {
    row.get(field)?.as_str()
}

fn string_array(row: &Value, field: &str) -> Option<Vec<String>> {
    row.get(field)?
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn raw_value(raw: &[u8], limit: usize) -> Result<Value, RelationError> {
    if raw.len() > limit {
        return Err(RelationError::BudgetExceeded);
    }
    published_value(raw, limit).map_err(RelationError::InvalidPublishedJson)
}

fn schema_valid(probe: &SchemaBackendProbe, uri: &str, raw: &[u8]) -> Result<bool, RelationError> {
    probe
        .is_valid_raw(uri, raw)
        .map_err(RelationError::SchemaBackend)
}

fn resource_read(probe: &SchemaBackendProbe, uri: &str) -> Option<PredicateRead> {
    Some(PredicateRead::SchemaResource {
        uri: uri.to_owned(),
        digest: probe.resource_digest(uri)?.to_prefixed(),
    })
}

fn ancestry_contains(types: &BTreeMap<String, Value>, actual: &str, allowed: &[String]) -> bool {
    let mut pending = vec![actual.to_owned()];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }
        if allowed.contains(&current) {
            return true;
        }
        let Some(entry) = types.get(&current) else {
            return false;
        };
        let Some(parents) = string_array(entry, "parent_type_ids") else {
            return false;
        };
        pending.extend(parents);
    }
    false
}

fn inspect_value_endpoints(
    claim: &Value,
    route: &ClaimRoute,
    types: &BTreeMap<String, Value>,
    endpoints: &BTreeMap<String, String>,
    generation: &str,
    location: &str,
    shadow: &mut RelationShadow,
) {
    let Some(object) = claim.get("object").and_then(Value::as_object) else {
        shadow.issue("structured-object", location);
        return;
    };
    if let Some(expected_kind) = field_str(&route.profile, "value_kind") {
        if object.get("kind").and_then(Value::as_str) != Some(expected_kind) {
            shadow.issue("structured-value-kind", location);
        }
    }
    if route.reader == "historical-temporal-v1" || route.reader == "document-catalogue-temporal-v1"
    {
        if object.get("kind").and_then(Value::as_str) == Some("relative-order") {
            let anchor = object
                .get("relative")
                .and_then(|r| r.get("anchor_ref"))
                .and_then(Value::as_str);
            match anchor.and_then(|id| endpoints.get(id).map(|kind| (id, kind))) {
                Some((id, kind))
                    if ancestry_contains(
                        types,
                        kind,
                        &["tos.entity.historical-situation".into()],
                    ) =>
                {
                    shadow.read(PredicateRead::RefEndpoint {
                        endpoint_type: kind.clone(),
                        id: id.into(),
                        observed: KeyState::Present,
                    });
                    shadow.read(PredicateRead::ReverseRefs {
                        target: id.into(),
                        relation: "relative-temporal-anchor".into(),
                        generation: generation.into(),
                    });
                }
                _ => {
                    if let Some(id) = anchor {
                        shadow.read(PredicateRead::RefEndpoint {
                            endpoint_type: "historical-situation".into(),
                            id: id.into(),
                            observed: if endpoints.contains_key(id) {
                                KeyState::Present
                            } else {
                                KeyState::Absent
                            },
                        });
                    }
                    shadow.issue("relative-anchor-type-or-missing", location);
                }
            }
        }
        return;
    }
    if route.reader != "structured-reference-value-v1" {
        return;
    }
    let Some(constraint) = route.profile.get("object_reference_set") else {
        shadow.issue("reference-set-profile", location);
        return;
    };
    let (Some(members), Some(allowed)) = (
        object.get("members").and_then(Value::as_array),
        string_array(constraint, "member_type_ids"),
    ) else {
        shadow.issue("reference-set-shape", location);
        return;
    };
    let min = constraint
        .get("min_items")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX);
    let max = constraint
        .get("max_items")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if (members.len() as u64) < min || (members.len() as u64) > max {
        shadow.issue("reference-set-cardinality", location);
    }
    let mut seen = BTreeSet::new();
    for member in members {
        let Some(id) = member.as_str() else {
            shadow.issue("reference-member-shape", location);
            continue;
        };
        if !seen.insert(id) || claim.get("claim_id").and_then(Value::as_str) == Some(id) {
            shadow.issue("reference-member-duplicate-or-self", location);
        }
        match endpoints.get(id) {
            Some(actual) if ancestry_contains(types, actual, &allowed) => {
                shadow.read(PredicateRead::RefEndpoint {
                    endpoint_type: actual.clone(),
                    id: id.into(),
                    observed: KeyState::Present,
                });
                shadow.read(PredicateRead::ReverseRefs {
                    target: id.into(),
                    relation: "structured-value-member".into(),
                    generation: generation.into(),
                });
            }
            _ => {
                shadow.read(PredicateRead::RefEndpoint {
                    endpoint_type: "declared-member-range".into(),
                    id: id.into(),
                    observed: if endpoints.contains_key(id) {
                        KeyState::Present
                    } else {
                        KeyState::Absent
                    },
                });
                shadow.issue("reference-member-type-or-missing", location);
            }
        }
    }
    if constraint.get("subject_is_member") == Some(&Value::Bool(true))
        && !seen.contains(
            claim
                .get("subject_ref")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )
    {
        shadow.issue("reference-subject-not-member", location);
    }
    if let Some(order) = object.get("ordering") {
        let mode = field_str(order, "mode");
        let precedes = order.get("precedes").and_then(Value::as_array);
        if let (Some(mode), Some(edges)) = (mode, precedes) {
            if mode == "unordered" && !edges.is_empty() {
                shadow.issue("unordered-has-precedence", location);
            }
            let mut outgoing: BTreeMap<&str, BTreeSet<&str>> = seen
                .iter()
                .copied()
                .map(|id| (id, BTreeSet::new()))
                .collect();
            let mut indegree: BTreeMap<&str, usize> =
                seen.iter().copied().map(|id| (id, 0)).collect();
            for edge in edges {
                let pair = edge.as_array();
                let points = pair
                    .filter(|p| p.len() == 2)
                    .and_then(|p| Some((p[0].as_str()?, p[1].as_str()?)));
                let Some((before, after)) = points else {
                    shadow.issue("precedence-edge-shape", location);
                    continue;
                };
                if !seen.contains(before) || !seen.contains(after) {
                    shadow.issue("precedence-endpoint", location);
                    continue;
                }
                if outgoing
                    .get_mut(before)
                    .is_some_and(|targets| targets.insert(after))
                {
                    *indegree.get_mut(after).expect("member counted") += 1;
                }
            }
            let mut ready: Vec<&str> = indegree
                .iter()
                .filter_map(|(id, count)| (*count == 0).then_some(*id))
                .collect();
            let mut visited = 0usize;
            while let Some(id) = ready.pop() {
                if mode == "total" && !ready.is_empty() {
                    shadow.issue("total-order-incomplete", location);
                }
                visited += 1;
                for after in outgoing.get(id).into_iter().flat_map(|edges| edges.iter()) {
                    let count = indegree.get_mut(after).expect("member counted");
                    *count -= 1;
                    if *count == 0 {
                        ready.push(after);
                    }
                }
            }
            if visited != seen.len() {
                shadow.issue("precedence-cycle", location);
            }
        }
    }
    if constraint.get("basis_adapter").is_some() {
        shadow.skip("collection-membership-exact-version-basis");
    }
}

fn relation_profile_routes(
    relation_registry: &Value,
    shadow: &mut RelationShadow,
) -> Result<BTreeMap<String, ClaimRoute>, RelationError> {
    let relations = relation_registry
        .get("relations")
        .and_then(Value::as_array)
        .ok_or(RelationError::MalformedRegistry)?;
    let mut relation_ids = BTreeSet::new();
    let mut routes = BTreeMap::new();
    for entry in relations {
        let id = field_str(entry, "relation_type_id").ok_or(RelationError::MalformedRegistry)?;
        if !relation_ids.insert(id) {
            shadow.issue("duplicate-relation-type", id);
        }
        let Some(profile) = entry.get("source_claim_profile") else {
            continue;
        };
        let Some(reader) = field_str(profile, "reader") else {
            return Err(RelationError::MalformedRegistry);
        };
        let Some(domain) = string_array(entry, "domain_type_ids") else {
            return Err(RelationError::MalformedRegistry);
        };
        let Some(range) = string_array(entry, "range_type_ids") else {
            return Err(RelationError::MalformedRegistry);
        };
        let Some(mappings) = entry.get("source_mappings").and_then(Value::as_array) else {
            return Err(RelationError::MalformedRegistry);
        };
        let predicates: Vec<_> = mappings
            .iter()
            .filter(|m| {
                field_str(m, "source_graph") == Some("source-claims")
                    && field_str(m, "scope") == Some("claim-predicate")
            })
            .filter_map(|m| field_str(m, "source_predicate_id"))
            .collect();
        if predicates.len() != 1
            || entry.get("abstract") != Some(&Value::Bool(false))
            || field_str(entry, "assertion_mode") != Some("reified-claim")
            || entry.get("evidence_required") != Some(&Value::Bool(true))
        {
            shadow.issue("invalid-relation-profile-route", id);
            continue;
        }
        let Some(schemas) = profile.get("schemas").and_then(Value::as_array) else {
            return Err(RelationError::MalformedRegistry);
        };
        let mut versions = BTreeMap::new();
        for schema in schemas {
            let (Some(version), Some(reference)) = (
                field_str(schema, "schema_version"),
                field_str(schema, "schema_ref"),
            ) else {
                return Err(RelationError::MalformedRegistry);
            };
            if versions
                .insert(version.to_owned(), reference.to_owned())
                .is_some()
            {
                shadow.issue("duplicate-schema-route", id);
            }
        }
        let predicate = predicates[0].to_owned();
        if routes
            .insert(
                predicate.clone(),
                ClaimRoute {
                    reader: reader.to_owned(),
                    domain,
                    range,
                    versions,
                    profile: profile.clone(),
                },
            )
            .is_some()
        {
            shadow.issue("duplicate-predicate-owner", predicate);
        }
    }
    Ok(routes)
}

/// Check exact registered shape and typed endpoints for the declared Claim
/// streams. This intentionally does not decide provenance truth, review,
/// rights, historical correction-chain completeness, or special readers.
pub fn inspect_profiled_claims(
    input: ClaimFamilyInput<'_>,
    probe: &SchemaBackendProbe,
) -> Result<RelationShadow, RelationError> {
    let mut shadow = RelationShadow::default();
    if !input.complete_union || input.union_generation.is_empty() {
        shadow.skip("complete-current-claim-union");
        return Ok(shadow);
    }
    if input.union_generation.len() > MAX_GENERATION_BYTES
        || input
            .claim_streams
            .iter()
            .any(|stream| stream.source_ref.len() > MAX_SOURCE_REF_BYTES)
        || input
            .endpoints
            .iter()
            .any(|endpoint| endpoint.source_ref.len() > MAX_SOURCE_REF_BYTES)
    {
        return Err(RelationError::BudgetExceeded);
    }
    if input.claim_streams.len() > MAX_CLAIM_STREAMS || input.endpoints.len() > MAX_ENDPOINTS {
        return Err(RelationError::BudgetExceeded);
    }
    let total_bytes = input
        .claim_streams
        .iter()
        .try_fold(0usize, |sum, stream| sum.checked_add(stream.raw.len()))
        .ok_or(RelationError::BudgetExceeded)?;
    if total_bytes > MAX_TOTAL_CLAIM_BYTES {
        return Err(RelationError::BudgetExceeded);
    }
    let entities = raw_value(input.entity_registry_raw, MAX_REGISTRY_BYTES)?;
    let relations = raw_value(input.relation_registry_raw, MAX_REGISTRY_BYTES)?;
    if !schema_valid(probe, ENTITY_CONTRACT, input.entity_registry_raw)?
        || !schema_valid(probe, RELATION_CONTRACT, input.relation_registry_raw)?
    {
        shadow.issue("registry-schema", "entity/relation");
        return Ok(shadow);
    }
    for (uri, raw, registry) in [
        (ENTITY_CONTRACT, input.entity_registry_raw, &entities),
        (RELATION_CONTRACT, input.relation_registry_raw, &relations),
    ] {
        shadow.read(PredicateRead::Registry {
            uri: uri.to_owned(),
            version: registry
                .get("registry_version")
                .map(Value::to_string)
                .unwrap_or_default(),
            digest: Digest256::of_bytes(raw).to_prefixed(),
        });
        if let Some(read) = resource_read(probe, uri) {
            shadow.read(read);
        }
    }
    let mut types = BTreeMap::new();
    let mut kind_types = BTreeMap::new();
    for entry in entities
        .get("types")
        .and_then(Value::as_array)
        .ok_or(RelationError::MalformedRegistry)?
    {
        let id = field_str(entry, "type_id")
            .ok_or(RelationError::MalformedRegistry)?
            .to_owned();
        if types.insert(id.clone(), entry.clone()).is_some() {
            shadow.issue("duplicate-entity-type", id.clone());
        }
        if let Some(mappings) = entry.get("source_mappings").and_then(Value::as_array) {
            for mapping in mappings {
                if field_str(mapping, "source_graph") != Some("source-claims") {
                    continue;
                }
                if let Some(kind) = field_str(mapping, "source_kind_id") {
                    if kind_types.insert(kind.to_owned(), id.clone()).is_some() {
                        shadow.issue("duplicate-source-kind-owner", kind);
                    }
                }
            }
        }
    }
    let routes = relation_profile_routes(&relations, &mut shadow)?;
    for (predicate, route) in &routes {
        for (version, schema_ref) in &route.versions {
            let profile_id = format!("{predicate}@{version}");
            shadow.declared_profiles.insert(profile_id.clone());
            if !route.generic_endpoint(schema_ref) {
                shadow.skip(format!("{}:{profile_id}", route.reader));
            }
        }
    }
    let mut endpoint_types = BTreeMap::new();
    for endpoint in input.endpoints {
        shadow.observed_endpoints += 1;
        let value = raw_value(endpoint.raw, MAX_RECORD_BYTES)?;
        let identity = match (
            field_str(&value, "record_id"),
            field_str(&value, "record_type"),
        ) {
            (Some(id), Some(kind)) => Some((id, kind)),
            _ if endpoint.source_ref.ends_with("/artifact-witness.json") => {
                shadow.skip("native-artifact-endpoint-adapter");
                field_str(&value, "artifact_id").map(|id| (id, "artifact"))
            }
            _ if endpoint.source_ref.ends_with("/composite-witness.json") => {
                shadow.skip("native-composite-endpoint-adapter");
                field_str(&value, "composite_id").map(|id| (id, "composite"))
            }
            _ => None,
        };
        let Some((id, kind)) = identity else {
            shadow.skip(format!(
                "unregistered-endpoint-adapter:{}",
                endpoint.source_ref
            ));
            continue;
        };
        let Some(type_id) = kind_types.get(kind) else {
            shadow.issue("endpoint-type-unmapped", endpoint.source_ref);
            continue;
        };
        if endpoint_types
            .insert(id.to_owned(), type_id.clone())
            .is_some()
        {
            shadow.issue("duplicate-endpoint-id", id);
        }
        shadow.read(PredicateRead::ExactPath {
            path: endpoint.source_ref.to_owned(),
            digest: Digest256::of_bytes(endpoint.raw).to_prefixed(),
        });
    }
    let mut seen_claim_ids = BTreeSet::new();
    for stream in input.claim_streams {
        if stream.raw.len() > MAX_CLAIM_FILE_BYTES {
            return Err(RelationError::BudgetExceeded);
        }
        if !stream.source_ref.starts_with("ToS/source-witnesses/")
            || !stream.source_ref.ends_with("/source-claims.jsonl")
            || stream.source_ref.split('/').any(|part| {
                matches!(
                    part,
                    ".." | "." | "payload" | "catalog" | "owner-local" | "local-content"
                )
            })
        {
            shadow.issue("claim-path", stream.source_ref);
            continue;
        }
        shadow.read(PredicateRead::ExactPath {
            path: stream.source_ref.to_owned(),
            digest: Digest256::of_bytes(stream.raw).to_prefixed(),
        });
        for (index, line) in stream.raw.split(|byte| *byte == b'\n').enumerate() {
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            shadow.observed_claims += 1;
            if shadow.observed_claims > MAX_CLAIMS {
                return Err(RelationError::BudgetExceeded);
            }
            let location = format!("{}:{}", stream.source_ref, index + 1);
            let claim = raw_value(line, MAX_CLAIM_BYTES)?;
            let (Some(predicate), Some(version), Some(claim_id), Some(subject)) = (
                field_str(&claim, "predicate"),
                field_str(&claim, "schema_version"),
                field_str(&claim, "claim_id"),
                field_str(&claim, "subject_ref"),
            ) else {
                shadow.issue("claim-shape", location);
                continue;
            };
            if !seen_claim_ids.insert(claim_id.to_owned()) {
                shadow.issue("duplicate-claim-id", &location);
            }
            shadow.read(PredicateRead::UniqueKey {
                namespace: "source-claim-id".into(),
                key: claim_id.to_owned(),
                owner: location.clone(),
            });
            let Some(route) = routes.get(predicate) else {
                shadow.issue("unrecognized-predicate", location);
                continue;
            };
            let Some(schema_ref) = route.versions.get(version) else {
                shadow.issue("unrecognized-schema-version", location);
                continue;
            };
            let uri = format!("https://tree-of-sophia.local/{schema_ref}");
            let route_id = format!("{predicate}@{version}");
            let supported = route.generic_endpoint(schema_ref);
            if supported {
                shadow.checked_profiles.insert(route_id.clone());
            } else {
                shadow.skip(format!("{}:{route_id}", route.reader));
            }
            if !schema_valid(probe, &uri, line)? || !schema_valid(probe, CLAIM_BASE, line)? {
                shadow.issue("claim-schema", &location);
                continue;
            }
            if let Some(read) = resource_read(probe, &uri) {
                shadow.read(read);
            }
            if let Some(read) = resource_read(probe, CLAIM_BASE) {
                shadow.read(read);
            }
            if claim.get("claim_type").and_then(Value::as_str) != Some("relation")
                || !matches!(
                    claim.get("visibility").and_then(Value::as_str),
                    Some("public" | "public_metadata_only")
                )
            {
                shadow.issue("claim-public-shape", &location);
                continue;
            }
            if claim_id == subject {
                shadow.issue("self-subject-claim-id", &location);
            }
            let allowed_layers =
                string_array(&route.profile, "assertion_layers").unwrap_or_default();
            if !claim
                .get("assertion_layer")
                .and_then(Value::as_str)
                .is_some_and(|layer| allowed_layers.iter().any(|allowed| allowed == layer))
            {
                shadow.issue("claim-assertion-layer", &location);
            }
            let Some(actual_subject) = endpoint_types.get(subject) else {
                shadow.read(PredicateRead::RefEndpoint {
                    endpoint_type: "declared-domain".into(),
                    id: subject.into(),
                    observed: KeyState::Absent,
                });
                shadow.issue("missing-subject", &location);
                continue;
            };
            shadow.read(PredicateRead::RefEndpoint {
                endpoint_type: actual_subject.clone(),
                id: subject.to_owned(),
                observed: KeyState::Present,
            });
            if !ancestry_contains(&types, actual_subject, &route.domain) {
                shadow.issue("subject-domain", &location);
            }
            if let Some(object) = claim.get("object").and_then(Value::as_str) {
                if object == claim_id {
                    shadow.issue("self-object-claim-id", &location);
                }
                let Some(actual_object) = endpoint_types.get(object) else {
                    shadow.read(PredicateRead::RefEndpoint {
                        endpoint_type: "declared-range".into(),
                        id: object.into(),
                        observed: KeyState::Absent,
                    });
                    shadow.issue("missing-object", &location);
                    continue;
                };
                shadow.read(PredicateRead::RefEndpoint {
                    endpoint_type: actual_object.clone(),
                    id: object.to_owned(),
                    observed: KeyState::Present,
                });
                if !ancestry_contains(&types, actual_object, &route.range) {
                    shadow.issue("object-range", &location);
                }
                shadow.read(PredicateRead::ReverseRefs {
                    target: object.to_owned(),
                    relation: predicate.to_owned(),
                    generation: input.union_generation.to_owned(),
                });
            } else if claim.get("object").and_then(Value::as_object).is_some() {
                inspect_value_endpoints(
                    &claim,
                    route,
                    &types,
                    &endpoint_types,
                    input.union_generation,
                    &location,
                    &mut shadow,
                );
            } else {
                shadow.issue("object-type", &location);
            }
            // Negative and unknown polarity are Claim values, not an absence assertion.
            shadow.fact(ValidationFact {
                namespace: "source-claim-assertion".into(),
                key: claim_id.to_owned(),
                value_digest: Digest256::of_bytes(line).to_prefixed(),
            });
        }
    }
    shadow.read(PredicateRead::Range {
        namespace: "source-claim-id-union".into(),
        lower: String::new(),
        upper: String::new(),
        generation: input.union_generation.to_owned(),
    });
    // A profile-local scan cannot prove retained legacy/native currentness,
    // provenance, owner assessment, or the publication member root.
    shadow.skip("legacy-native-claim-version-and-provenance-union");
    Ok(shadow)
}

/// A separately verified current union is required. The source owner joins
/// three retained topology streams (work-expression, expression-edition,
/// edition-item) to native source-claims rows only after their respective
/// fixed event/evidence and compound-command verification. This local
/// structural closure does not inspect or certify those carrier routes.
/// Polarity, including negative/unknown, remains a Claim value in the union.
pub fn inspect_current_topology(
    records: &[Value],
    claims: &[Value],
    item_edition: &BTreeMap<String, String>,
    verified_complete_union: bool,
    union_generation: &str,
) -> RelationShadow {
    // This pure compatibility surface retains its existing source count caps.
    // The actual cut consumer below additionally supplies its remaining state,
    // original deadline and cancellation; neither route grants source authority.
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    match inspect_current_topology_inner(
        records,
        claims,
        item_edition,
        verified_complete_union,
        union_generation,
        None,
        &cancelled,
        true,
    ) {
        Ok((shadow, _)) => shadow,
        Err(_) => {
            let mut shadow = RelationShadow::default();
            shadow.skip("current-topology-state-capacity");
            shadow
        }
    }
}

// A topology-local running logical total: borrowed index slots plus owned
// report slots/payloads. It is not allocator-node or RSS accounting. Indexes
// remain charged until the algorithm returns; each serialization streams into
// the existing SHA-256 implementation rather than retaining a second JSON Vec.
struct TopologyState<'a> {
    shadow: RelationShadow,
    state: usize,
    report_state: usize,
    limits: Option<crate::item_rules::ItemLimits>,
    cancelled: &'a std::sync::atomic::AtomicBool,
}
impl TopologyState<'_> {
    fn reserve(&mut self, amount: usize) -> Result<(), crate::item_rules::ItemRefusal> {
        if let Some(limits) = self.limits {
            crate::validation_codec::check(limits.deadline, self.cancelled)?;
        }
        let next = self
            .state
            .checked_add(amount)
            .ok_or(crate::item_rules::ItemRefusal::Budget)?;
        let limit = self
            .limits
            .map_or(usize::MAX, |limits| limits.max_state_bytes);
        if next > limit {
            return Err(crate::item_rules::ItemRefusal::BudgetCheck {
                check: "bibliography topology simultaneous logical state",
                used: Some(next as u64),
                limit: Some(limit as u64),
            });
        }
        self.state = next;
        Ok(())
    }
    fn report(&mut self, amount: usize) -> Result<(), crate::item_rules::ItemRefusal> {
        self.reserve(amount)?;
        self.report_state = self
            .report_state
            .checked_add(amount)
            .ok_or(crate::item_rules::ItemRefusal::Budget)?;
        Ok(())
    }
    fn issue(
        &mut self,
        code: &'static str,
        location: &str,
    ) -> Result<(), crate::item_rules::ItemRefusal> {
        if self.shadow.issues.len() < MAX_ISSUES {
            self.report(std::mem::size_of::<RelationIssue>() + location.len())?;
        }
        self.shadow.issue(code, location);
        Ok(())
    }
    fn skip(&mut self, profile: impl AsRef<str>) -> Result<(), crate::item_rules::ItemRefusal> {
        let profile = profile.as_ref();
        if !self.shadow.skipped_profiles.contains(profile) {
            let retained = if self.shadow.skipped_profiles.len() < MAX_SKIPPED_PROFILES {
                profile
            } else {
                "skipped-profile-sink-capacity"
            };
            if !self.shadow.skipped_profiles.contains(retained) {
                self.report(std::mem::size_of::<String>() + retained.len())?;
            }
        }
        self.shadow.skip(profile);
        Ok(())
    }
    fn read(
        &mut self,
        payload: usize,
        make: impl FnOnce() -> PredicateRead,
    ) -> Result<(), crate::item_rules::ItemRefusal> {
        if self.shadow.reads.len() < MAX_READS {
            self.report(
                std::mem::size_of::<PredicateRead>()
                    .checked_add(payload)
                    .ok_or(crate::item_rules::ItemRefusal::Budget)?,
            )?;
            self.shadow.reads.push(make());
        } else {
            self.skip("predicate-read-sink-capacity")?;
        }
        Ok(())
    }
    fn fact(
        &mut self,
        namespace: &str,
        key_len: usize,
        make_key: impl FnOnce() -> String,
        digest: Digest256,
    ) -> Result<(), crate::item_rules::ItemRefusal> {
        if self.shadow.facts.len() < MAX_FACTS {
            self.report(
                std::mem::size_of::<ValidationFact>()
                    + namespace.len()
                    + key_len
                    + "sha256:".len()
                    + digest.as_bytes().len() * 2,
            )?;
            self.shadow.facts.push(ValidationFact {
                namespace: namespace.into(),
                key: make_key(),
                value_digest: digest.to_prefixed(),
            });
        } else {
            self.skip("validation-fact-sink-capacity")?;
        }
        Ok(())
    }
    fn strings<'a>(
        &mut self,
        record: &'a Value,
        field: &str,
    ) -> Result<(Option<Vec<&'a str>>, usize), crate::item_rules::ItemRefusal> {
        let count = record
            .get(field)
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let cost = std::mem::size_of::<Option<Vec<&str>>>()
            .checked_add(
                count
                    .checked_mul(std::mem::size_of::<&str>())
                    .ok_or(crate::item_rules::ItemRefusal::Budget)?,
            )
            .ok_or(crate::item_rules::ItemRefusal::Budget)?;
        self.reserve(cost)?;
        Ok((topology_strings(record, field), cost))
    }
    fn json_key<const N: usize>(
        &mut self,
        parts: [&str; N],
    ) -> Result<(String, usize), crate::item_rules::ItemRefusal> {
        let remaining = self
            .limits
            .map_or(usize::MAX, |limits| limits.max_state_bytes)
            .checked_sub(self.state)
            .and_then(|n| n.checked_sub(std::mem::size_of::<String>()))
            .ok_or(crate::item_rules::ItemRefusal::Budget)?;
        let bytes = crate::validation_codec::serialized_wire_size(remaining, |writer| {
            serde_json::to_writer(writer, parts.as_slice())
        })?;
        let cost = std::mem::size_of::<String>() + bytes;
        self.reserve(cost)?;
        let key = serde_json::to_string(parts.as_slice()).map_err(|_| {
            crate::item_rules::ItemRefusal::Unsupported("topology key serialization".into())
        })?;
        Ok((key, cost))
    }
    fn checked(&mut self, profile: &str) -> Result<(), crate::item_rules::ItemRefusal> {
        if !self.shadow.checked_profiles.contains(profile) {
            self.report(std::mem::size_of::<String>() + profile.len())?;
            self.shadow.checked_profiles.insert(profile.into());
        }
        Ok(())
    }
}
struct TopologyDigest(tos_foundation::Digest256Hasher);
impl std::io::Write for TopologyDigest {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn topology_digest(
    write: impl FnOnce(&mut TopologyDigest) -> serde_json::Result<()>,
) -> serde_json::Result<Digest256> {
    let mut writer = TopologyDigest(tos_foundation::Digest256Hasher::new());
    write(&mut writer)?;
    Ok(writer.0.finalize())
}

pub(crate) fn inspect_current_topology_bounded(
    records: &[Value],
    claims: &[Value],
    item_edition: &BTreeMap<String, String>,
    verified_complete_union: bool,
    union_generation: &str,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(RelationShadow, usize), crate::item_rules::ItemRefusal> {
    inspect_current_topology_inner(
        records,
        claims,
        item_edition,
        verified_complete_union,
        union_generation,
        Some(limits),
        cancelled,
        true,
    )
}

/// Run the existing topology owner kernel against the candidate's bounded
/// current-record, Claim, and Item-manifest providers. The projection keeps
/// every record's identity pair for global duplicate detection, adds only the
/// route-specific fields needed by topology, and retains only topology Claim
/// rows plus malformed Claim values. Unrelated record fields/events and
/// non-topology Claim values remain in their provider. Projection values and
/// the kernel's indexes/report are charged simultaneously before row copies.
/// For N rows in these selected projections, the charge follows their actual
/// retained JSON and index fields; N is not a capacity or total-input claim.
#[cfg(feature = "native")]
pub(crate) fn inspect_current_topology_from_stored<
    C: SourceFoundationDefaultClaims + ?Sized,
    R: SourceFoundationDefaultRecordsLookup + ?Sized,
    M: SourceFoundationBiblioManifestSink + ?Sized,
>(
    records: &R,
    claims: &C,
    manifests: &M,
    verified_complete_union: bool,
    total_record_count: usize,
    total_claim_count: usize,
    union_generation: &str,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(RelationShadow, usize), crate::item_rules::ItemRefusal> {
    if !verified_complete_union || union_generation.is_empty() {
        return empty_topology_skip("verified-current-topology-union", limits, cancelled);
    }
    if union_generation.len() > MAX_GENERATION_BYTES {
        return empty_topology_skip("current-topology-generation-capacity", limits, cancelled);
    }
    if total_record_count > MAX_TOPOLOGY_OBJECTS || total_claim_count > MAX_TOPOLOGY_OBJECTS {
        return empty_topology_skip("current-topology-input-capacity", limits, cancelled);
    }

    let mut projection = TopologyState {
        shadow: RelationShadow::default(),
        state: 0,
        report_state: 0,
        limits: Some(limits),
        cancelled,
    };
    let mut records_projection = Vec::new();
    let mut claims_projection = Vec::new();
    let mut item_edition = BTreeMap::new();
    let mut non_topology_profiles = Vec::new();
    let mut seen_non_topology_profiles = BTreeSet::new();
    let mut overlong_profile_seen = false;
    let mut manifests_over_capacity = false;
    let mut manifest_count = 0usize;
    projection.reserve(
        std::mem::size_of::<TopologyState<'_>>()
            .checked_add(std::mem::size_of::<Vec<Value>>() * 2)
            .and_then(|n| n.checked_add(std::mem::size_of::<BTreeMap<String, String>>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<String>>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<BTreeSet<String>>()))
            .ok_or(crate::item_rules::ItemRefusal::Budget)?,
    )?;

    manifests.for_each_biblio_manifest(&mut |id, edition| {
        projection.reserve(0)?;
        manifest_count = manifest_count
            .checked_add(1)
            .ok_or(crate::item_rules::ItemRefusal::Budget)?;
        if manifest_count > MAX_TOPOLOGY_OBJECTS {
            manifests_over_capacity = true;
            return Ok(());
        }
        let cost = std::mem::size_of::<(String, String)>()
            .checked_add(id.len())
            .and_then(|n| n.checked_add(edition.len()))
            .ok_or(crate::item_rules::ItemRefusal::Budget)?;
        projection.reserve(cost)?;
        item_edition.insert(id.to_owned(), edition.to_owned());
        Ok(())
    })?;
    if manifests_over_capacity {
        drop(records_projection);
        drop(claims_projection);
        drop(item_edition);
        drop(non_topology_profiles);
        drop(seen_non_topology_profiles);
        return empty_topology_skip("current-topology-input-capacity", limits, cancelled);
    }

    claims.for_each_claim(&mut |_, claim| {
        projection.reserve(0)?;
        if !claim.value.is_object() {
            let cost = crate::record_biblio_cut::decoded_state(&claim.value)?;
            projection.reserve(cost)?;
            claims_projection.push(claim.value.clone());
            return Ok(());
        }
        let Some(predicate) = field_str(&claim.value, "predicate") else {
            return Ok(());
        };
        if matches!(
            predicate,
            "has_expression" | "embodied_by" | "exemplified_by"
        ) {
            let cost = crate::record_biblio_cut::decoded_state(&claim.value)?;
            projection.reserve(cost)?;
            claims_projection.push(claim.value.clone());
        } else if predicate.len() <= MAX_GENERATION_BYTES {
            let prefix_len = "non-topology-claim-profile:".len();
            let profile_len = prefix_len
                .checked_add(predicate.len())
                .ok_or(crate::item_rules::ItemRefusal::Budget)?;
            if !seen_non_topology_profiles.contains(predicate)
                && non_topology_profiles.len() <= MAX_SKIPPED_PROFILES
            {
                let cost = std::mem::size_of::<String>()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(profile_len))
                    .and_then(|n| n.checked_add(predicate.len()))
                    .ok_or(crate::item_rules::ItemRefusal::Budget)?;
                projection.reserve(cost)?;
                let profile = format!("non-topology-claim-profile:{predicate}");
                seen_non_topology_profiles.insert(predicate.to_owned());
                non_topology_profiles.push(profile);
            }
        } else if !overlong_profile_seen && non_topology_profiles.len() <= MAX_SKIPPED_PROFILES {
            let profile_len = "non-topology-claim-profile-overlong".len();
            let cost = std::mem::size_of::<String>()
                .checked_add(profile_len)
                .ok_or(crate::item_rules::ItemRefusal::Budget)?;
            projection.reserve(cost)?;
            let profile = "non-topology-claim-profile-overlong".to_owned();
            non_topology_profiles.push(profile);
            overlong_profile_seen = true;
        }
        Ok(())
    })?;

    records.for_each_current_record(&mut |id, record| {
        projection.reserve(0)?;
        let record_id = field_str(&record.value, "record_id");
        let record_type = field_str(&record.value, "record_type");
        // The unchanged topology kernel detects duplicate IDs across every
        // record, so retain this minimal identity pair for all rows. Only the
        // route-specific fields below are copied when their type can use them.
        let route_fields: &[&str] = match record_type {
            Some("work") => &["expression_claim_refs"],
            Some("expression") => &["work_ref", "embodiment_claim_refs"],
            Some("edition") => &["embodies_expression_refs", "exemplar_claim_refs"],
            _ => &[],
        };
        const ID_FIELDS: [&str; 2] = ["record_id", "record_type"];
        let mut projection_cost = std::mem::size_of::<Value>();
        for field in ID_FIELDS.into_iter().chain(route_fields.iter().copied()) {
            if let Some(value) = record.value.get(field) {
                let child_heap = crate::record_biblio_cut::decoded_state(value)?
                    .checked_sub(std::mem::size_of::<Value>())
                    .ok_or(crate::item_rules::ItemRefusal::Budget)?;
                projection_cost = projection_cost
                    .checked_add(std::mem::size_of::<(String, Value)>())
                    .and_then(|n| n.checked_add(field.len()))
                    .and_then(|n| n.checked_add(child_heap))
                    .ok_or(crate::item_rules::ItemRefusal::Budget)?;
            }
        }
        projection.reserve(projection_cost)?;
        let mut projected = serde_json::Map::new();
        for field in ID_FIELDS.into_iter().chain(route_fields.iter().copied()) {
            if let Some(value) = record.value.get(field) {
                projected.insert(field.to_owned(), value.clone());
            }
        }
        records_projection.push(Value::Object(projected));
        Ok(())
    })?;

    let mut topology_limits = limits;
    topology_limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(projection.state)
        .ok_or(crate::item_rules::ItemRefusal::Budget)?;
    let (mut shadow, report_state) = inspect_current_topology_inner(
        &records_projection,
        &claims_projection,
        &item_edition,
        true,
        union_generation,
        Some(topology_limits),
        cancelled,
        false,
    )?;
    let mut combined_state = TopologyState {
        shadow,
        state: report_state,
        report_state,
        // The projection remains live while profile findings are appended,
        // so additions must fit the post-projection allowance too.
        limits: Some(topology_limits),
        cancelled,
    };
    for profile in non_topology_profiles {
        combined_state.skip(profile)?;
    }
    shadow = combined_state.shadow;
    drop(records_projection);
    drop(claims_projection);
    drop(item_edition);
    drop(seen_non_topology_profiles);
    Ok((shadow, combined_state.report_state))
}

fn empty_topology_skip(
    profile: &str,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(RelationShadow, usize), crate::item_rules::ItemRefusal> {
    let root = std::mem::size_of::<RelationShadow>();
    let mut state = TopologyState {
        shadow: RelationShadow::default(),
        state: 0,
        report_state: 0,
        limits: Some(limits),
        cancelled,
    };
    state.report(root)?;
    state.reserve(
        std::mem::size_of::<TopologyState<'_>>() - root + std::mem::size_of::<TopologyDigest>(),
    )?;
    state.skip(profile)?;
    Ok((state.shadow, state.report_state))
}

fn inspect_current_topology_inner(
    records: &[Value],
    claims: &[Value],
    item_edition: &BTreeMap<String, String>,
    verified_complete_union: bool,
    union_generation: &str,
    limits: Option<crate::item_rules::ItemLimits>,
    cancelled: &std::sync::atomic::AtomicBool,
    include_compound_skip: bool,
) -> Result<(RelationShadow, usize), crate::item_rules::ItemRefusal> {
    let root = std::mem::size_of::<RelationShadow>();
    let mut shadow = TopologyState {
        shadow: RelationShadow::default(),
        state: 0,
        report_state: 0,
        limits,
        cancelled,
    };
    shadow.report(root)?;
    shadow.reserve(
        std::mem::size_of::<TopologyState<'_>>() - root + std::mem::size_of::<TopologyDigest>(),
    )?;
    if !verified_complete_union || union_generation.is_empty() {
        shadow.skip("verified-current-topology-union")?;
        return Ok((shadow.shadow, shadow.report_state));
    }
    if union_generation.len() > MAX_GENERATION_BYTES {
        shadow.skip("current-topology-generation-capacity")?;
        return Ok((shadow.shadow, shadow.report_state));
    }
    if records.len() > MAX_TOPOLOGY_OBJECTS
        || claims.len() > MAX_TOPOLOGY_OBJECTS
        || item_edition.len() > MAX_TOPOLOGY_OBJECTS
    {
        shadow.skip("current-topology-input-capacity")?;
        return Ok((shadow.shadow, shadow.report_state));
    }
    const ROUTES: [(&str, &str, &str, &str); 3] = [
        (
            "has_expression",
            "work",
            "expression",
            "expression_claim_refs",
        ),
        (
            "embodied_by",
            "expression",
            "edition",
            "embodiment_claim_refs",
        ),
        ("exemplified_by", "edition", "item", "exemplar_claim_refs"),
    ];
    let mut by_id = BTreeMap::new();
    shadow.reserve(std::mem::size_of_val(&by_id))?;
    for record in records {
        shadow.reserve(0)?;
        if let (Some(id), Some(_)) = (
            field_str(record, "record_id"),
            field_str(record, "record_type"),
        ) {
            if !by_id.contains_key(id) {
                shadow.reserve(std::mem::size_of::<(&str, &Value)>())?;
            }
            if by_id.insert(id, record).is_some() {
                shadow.issue("duplicate-record-id", id)?;
            }
        } else {
            shadow.issue("record-shape", "records")?;
        }
    }
    let mut expected: BTreeMap<(&str, &str), BTreeSet<&str>> = BTreeMap::new();
    shadow.reserve(std::mem::size_of_val(&expected))?;
    for record in records {
        shadow.reserve(0)?;
        let (Some(id), Some(kind)) = (
            field_str(record, "record_id"),
            field_str(record, "record_type"),
        ) else {
            continue;
        };
        if kind == "expression" {
            if let Some(work) = field_str(record, "work_ref") {
                topology_expected(&mut expected, "has_expression", work, id, &mut shadow)?;
            } else {
                shadow.issue("missing-work-ref", id)?;
            }
        } else if kind == "edition" {
            let (refs, refs_state) = shadow.strings(record, "embodies_expression_refs")?;
            if let Some(refs) = refs {
                let scratch = std::mem::size_of::<BTreeSet<&str>>()
                    + refs.len() * std::mem::size_of::<&str>();
                shadow.reserve(scratch)?;
                if refs.iter().any(|id| id.is_empty())
                    || refs.iter().copied().collect::<BTreeSet<_>>().len() != refs.len()
                {
                    shadow.issue("invalid-expression-refs", id)?;
                }
                for expression in &refs {
                    topology_expected(&mut expected, "embodied_by", expression, id, &mut shadow)?;
                }
                drop(refs);
                shadow.state -= scratch;
            } else {
                shadow.issue("invalid-expression-refs", id)?;
            }
            shadow.state -= refs_state;
        } else if kind == "item" && !item_edition.contains_key(id) {
            shadow.issue("missing-item-embodiment", id)?;
        }
    }
    for (item, edition) in item_edition {
        shadow.reserve(0)?;
        topology_expected(&mut expected, "exemplified_by", edition, item, &mut shadow)?;
    }
    let mut actual: BTreeMap<(&str, &str), BTreeMap<&str, &str>> = BTreeMap::new();
    let mut seen_ids = BTreeSet::new();
    let mut seen_pairs = BTreeSet::new();
    let mut expression_works = BTreeMap::<&str, &str>::new();
    shadow.reserve(
        std::mem::size_of_val(&actual)
            + std::mem::size_of_val(&seen_ids)
            + std::mem::size_of_val(&seen_pairs)
            + std::mem::size_of_val(&expression_works),
    )?;
    for claim in claims {
        shadow.reserve(0)?;
        if !claim.is_object() {
            shadow.issue("topology-claim-object", "claims")?;
            continue;
        }
        let Some(predicate) = field_str(claim, "predicate") else {
            continue;
        };
        let Some((_, subject_type, object_type, _)) =
            ROUTES.iter().find(|route| route.0 == predicate)
        else {
            if predicate.len() <= MAX_GENERATION_BYTES {
                let temporary = std::mem::size_of::<String>()
                    + "non-topology-claim-profile:".len()
                    + predicate.len();
                shadow.reserve(temporary)?;
                let profile = format!("non-topology-claim-profile:{predicate}");
                shadow.skip(&profile)?;
                drop(profile);
                shadow.state -= temporary;
            } else {
                shadow.skip("non-topology-claim-profile-overlong")?;
            }
            continue;
        };
        let (Some(id), Some(subject), Some(object)) = (
            field_str(claim, "claim_id"),
            field_str(claim, "subject_ref"),
            field_str(claim, "object"),
        ) else {
            shadow.issue("topology-claim-shape", predicate)?;
            continue;
        };
        if id.is_empty() || subject.is_empty() || object.is_empty() {
            shadow.issue("topology-claim-shape", predicate)?;
            continue;
        }
        if !seen_ids.contains(id) {
            shadow.reserve(std::mem::size_of::<&str>())?;
        }
        if !seen_ids.insert(id) {
            shadow.issue("duplicate-topology-claim-id", id)?;
        }
        shadow.read(
            "bibliographic-topology-claim-id".len()
                + id.len()
                + predicate.len()
                + subject.len()
                + object.len()
                + 2,
            || PredicateRead::UniqueKey {
                namespace: "bibliographic-topology-claim-id".into(),
                key: id.into(),
                owner: format!("{predicate}:{subject}:{object}"),
            },
        )?;
        if !seen_pairs.contains(&(predicate, subject, object)) {
            shadow.reserve(std::mem::size_of::<(&str, &str, &str)>())?;
        }
        if !seen_pairs.insert((predicate, subject, object)) {
            shadow.issue("duplicate-topology-pair", id)?;
        }
        let (pair_key, key_state) = shadow.json_key([predicate, subject, object])?;
        shadow.state -= key_state; // ownership moves into the report; no second key payload.
        shadow.read(
            "bibliographic-topology-pair".len() + pair_key.len() + id.len(),
            || PredicateRead::UniqueKey {
                namespace: "bibliographic-topology-pair".into(),
                key: pair_key,
                owner: id.into(),
            },
        )?;
        let subject_state = if by_id.get(subject).and_then(|r| field_str(r, "record_type"))
            == Some(*subject_type)
        {
            KeyState::Present
        } else {
            KeyState::Absent
        };
        let object_state =
            if by_id.get(object).and_then(|r| field_str(r, "record_type")) == Some(*object_type) {
                KeyState::Present
            } else {
                KeyState::Absent
            };
        shadow.read(subject_type.len() + subject.len(), || {
            PredicateRead::RefEndpoint {
                endpoint_type: (*subject_type).into(),
                id: subject.into(),
                observed: subject_state,
            }
        })?;
        shadow.read(object_type.len() + object.len(), || {
            PredicateRead::RefEndpoint {
                endpoint_type: (*object_type).into(),
                id: object.into(),
                observed: object_state,
            }
        })?;
        if by_id.get(subject).and_then(|r| field_str(r, "record_type")) != Some(*subject_type)
            || by_id.get(object).and_then(|r| field_str(r, "record_type")) != Some(*object_type)
        {
            shadow.issue("topology-endpoint", id)?;
        }
        if !expected
            .get(&(predicate.into(), subject.into()))
            .is_some_and(|targets| targets.contains(object))
        {
            shadow.issue("topology-link-unbacked", id)?;
        }
        if predicate == "has_expression" && !expression_works.contains_key(object) {
            shadow.reserve(std::mem::size_of::<(&str, &str)>())?;
        }
        if predicate == "has_expression"
            && expression_works
                .insert(object.into(), subject.into())
                .is_some_and(|previous| previous != subject)
        {
            shadow.issue("expression-two-work-owners", id)?;
        }
        if !actual.contains_key(&(predicate, subject)) {
            shadow.reserve(std::mem::size_of::<((&str, &str), BTreeMap<&str, &str>)>())?;
        }
        let rows = actual.entry((predicate, subject)).or_default();
        if !rows.contains_key(id) {
            shadow.reserve(std::mem::size_of::<(&str, &str)>())?;
        }
        rows.insert(id, object);
        if let Ok(canonical_claim) = topology_digest(|writer| serde_json::to_writer(writer, claim))
        {
            // Polarity is retained in this value digest. A negative Claim is
            // still an observed Claim, never an AbsentKey for its relation.
            shadow.fact(
                "bibliographic-topology-claim-value",
                id.len(),
                || id.into(),
                canonical_claim,
            )?;
        } else {
            shadow.issue("topology-claim-serialization", id)?;
        }
    }
    for (predicate, subject_type, _, field) in ROUTES {
        for record in records {
            shadow.reserve(0)?;
            if field_str(record, "record_type") != Some(subject_type) {
                continue;
            }
            let Some(subject) = field_str(record, "record_id") else {
                continue;
            };
            let (refs, refs_state) = shadow.strings(record, field)?;
            let scratch = std::mem::size_of::<BTreeSet<&str>>()
                + actual
                    .get(&(predicate, subject))
                    .map_or(0, |r| r.len() * std::mem::size_of::<&str>());
            let compare_state = std::mem::size_of::<BTreeSet<&str>>()
                + refs
                    .as_ref()
                    .map_or(0, |r| r.len() * std::mem::size_of::<&str>());
            shadow.reserve(scratch + compare_state)?;
            let found = actual.get(&(predicate.into(), subject.into()));
            let found_ids: BTreeSet<&str> = found
                .map(|rows| rows.keys().copied().collect())
                .unwrap_or_default();
            if refs.as_ref().is_none_or(|items| {
                items.iter().any(|id| id.is_empty())
                    || items.iter().cloned().collect::<BTreeSet<_>>() != found_ids
                    || items.len() != found_ids.len()
            }) {
                shadow.issue("topology-reverse-closure", subject)?;
            }
            drop(found_ids);
            drop(refs);
            shadow.state -= compare_state + refs_state;
            let found_targets: BTreeSet<&str> = found
                .map(|rows| rows.values().copied().collect())
                .unwrap_or_default();
            if expected
                .get(&(predicate, subject))
                .map_or(!found_targets.is_empty(), |wanted| &found_targets != wanted)
            {
                shadow.issue("topology-forward-closure", subject)?;
            }
            if let Ok(closure) = topology_digest(|writer| {
                serde_json::to_writer(writer, &(found, expected.get(&(predicate, subject))))
            }) {
                let (key, key_state) = shadow.json_key([predicate, subject])?;
                shadow.state -= key_state;
                shadow.fact(
                    "bibliographic-topology-subject-closure",
                    key.len(),
                    || key,
                    closure,
                )?;
            } else {
                shadow.issue("topology-closure-serialization", subject)?;
            }
            shadow.read(
                "topology:".len()
                    + predicate.len()
                    + ":subject".len()
                    + subject.len()
                    + subject.len()
                    + union_generation.len(),
                || PredicateRead::Range {
                    namespace: format!("topology:{predicate}:subject"),
                    lower: subject.into(),
                    upper: subject.into(),
                    generation: union_generation.into(),
                },
            )?;
            drop(found_targets);
            shadow.state -= scratch;
        }
    }
    for (predicate, _, _, _) in ROUTES {
        shadow.read(
            "source-topology-current-claims".len()
                + predicate.len()
                + predicate.len()
                + union_generation.len(),
            || PredicateRead::Range {
                namespace: "source-topology-current-claims".into(),
                lower: predicate.into(),
                upper: predicate.into(),
                generation: union_generation.into(),
            },
        )?;
        let rows_count = actual
            .keys()
            .filter(|(route, _)| *route == predicate)
            .count();
        let rows_state = std::mem::size_of::<Vec<(&(&str, &str), &BTreeMap<&str, &str>)>>()
            + rows_count * std::mem::size_of::<(&(&str, &str), &BTreeMap<&str, &str>)>();
        shadow.reserve(rows_state)?;
        let rows: Vec<_> = actual
            .iter()
            .filter(|((route, _), _)| *route == predicate)
            .collect();
        if let Ok(union) = topology_digest(|writer| serde_json::to_writer(writer, &rows)) {
            shadow.fact(
                "bibliographic-topology-predicate-union",
                predicate.len(),
                || predicate.into(),
                union,
            )?;
        } else {
            shadow.issue("topology-union-serialization", predicate)?;
        }
        drop(rows);
        shadow.state -= rows_state;
    }
    let mut reverse_objects = BTreeSet::new();
    shadow.reserve(std::mem::size_of_val(&reverse_objects))?;
    for ((predicate, subject), targets) in &expected {
        let Some(route) = ROUTES.iter().find(|route| route.0 == *predicate) else {
            shadow.skip("unknown-expected-topology-route")?;
            continue;
        };
        let subject_present = by_id
            .get(subject)
            .and_then(|record| field_str(record, "record_type"))
            == Some(route.1);
        shadow.read(route.1.len() + subject.len(), || {
            PredicateRead::RefEndpoint {
                endpoint_type: route.1.into(),
                id: (*subject).into(),
                observed: if subject_present {
                    KeyState::Present
                } else {
                    KeyState::Absent
                },
            }
        })?;
        if !subject_present {
            shadow.issue("topology-declared-link-endpoint", subject)?;
        }
        for target in targets {
            let target_present = by_id
                .get(target)
                .and_then(|record| field_str(record, "record_type"))
                == Some(route.2);
            shadow.read(route.2.len() + target.len(), || {
                PredicateRead::RefEndpoint {
                    endpoint_type: route.2.into(),
                    id: (*target).into(),
                    observed: if target_present {
                        KeyState::Present
                    } else {
                        KeyState::Absent
                    },
                }
            })?;
            if !target_present {
                shadow.issue("topology-declared-link-endpoint", target)?;
            }
        }
        shadow.read(
            subject.len()
                + "source-topology:".len()
                + predicate.len()
                + ":subject".len()
                + union_generation.len(),
            || PredicateRead::ReverseRefs {
                target: (*subject).into(),
                relation: format!("source-topology:{predicate}:subject"),
                generation: union_generation.into(),
            },
        )?;
        for target in targets {
            if !reverse_objects.contains(&(*predicate, *target)) {
                shadow.reserve(std::mem::size_of::<(&str, &str)>())?;
            }
            reverse_objects.insert((*predicate, *target));
        }
    }
    for ((predicate, _), rows) in &actual {
        for target in rows.values() {
            if !reverse_objects.contains(&(*predicate, *target)) {
                shadow.reserve(std::mem::size_of::<(&str, &str)>())?;
            }
            reverse_objects.insert((*predicate, *target));
        }
    }
    for (predicate, target) in reverse_objects {
        shadow.read(
            target.len()
                + "source-topology:".len()
                + predicate.len()
                + ":object".len()
                + union_generation.len(),
            || PredicateRead::ReverseRefs {
                target: target.into(),
                relation: format!("source-topology:{predicate}:object"),
                generation: union_generation.into(),
            },
        )?;
    }
    shadow.checked("source-bibliographic-topology-current-union@1")?;
    if include_compound_skip {
        shadow.skip("compound-native-and-retained-provenance-verification")?;
    }
    Ok((shadow.shadow, shadow.report_state))
}
fn topology_strings<'a>(record: &'a Value, field: &str) -> Option<Vec<&'a str>> {
    record
        .get(field)?
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect()
}
fn topology_expected<'a>(
    expected: &mut BTreeMap<(&'a str, &'a str), BTreeSet<&'a str>>,
    predicate: &'a str,
    subject: &'a str,
    target: &'a str,
    state: &mut TopologyState<'_>,
) -> Result<(), crate::item_rules::ItemRefusal> {
    if !expected.contains_key(&(predicate, subject)) {
        state.reserve(std::mem::size_of::<((&str, &str), BTreeSet<&str>)>())?;
    }
    let targets = expected.entry((predicate, subject)).or_default();
    if !targets.contains(target) {
        state.reserve(std::mem::size_of::<&str>())?;
        targets.insert(target);
    }
    Ok(())
}

/// Verify one event's declared local digest bindings, not historical truth or
/// event-ID uniqueness across the entire candidate. Missing local bytes are
/// an explicit unsupported dependency, never silently ignored.
pub fn inspect_provenance_event(
    event_ref: &str,
    event_raw: &[u8],
    local_bytes: &BTreeMap<String, Vec<u8>>,
    probe: &SchemaBackendProbe,
) -> Result<RelationShadow, RelationError> {
    let mut shadow = RelationShadow::default();
    if event_ref.len() > MAX_SOURCE_REF_BYTES {
        return Err(RelationError::BudgetExceeded);
    }
    if local_bytes.len() > MAX_PROVENANCE_REFERENCES
        || local_bytes
            .values()
            .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
            .ok_or(RelationError::BudgetExceeded)?
            > MAX_TOTAL_CLAIM_BYTES
    {
        return Err(RelationError::BudgetExceeded);
    }
    let event = raw_value(event_raw, MAX_RECORD_BYTES)?;
    if !schema_valid(probe, PROVENANCE_CONTRACT, event_raw)? {
        shadow.issue("provenance-schema", event_ref);
        return Ok(shadow);
    }
    shadow.read(PredicateRead::ExactPath {
        path: event_ref.into(),
        digest: Digest256::of_bytes(event_raw).to_prefixed(),
    });
    if let Some(read) = resource_read(probe, PROVENANCE_CONTRACT) {
        shadow.read(read);
    }
    let Some(id) = field_str(&event, "event_id") else {
        shadow.issue("provenance-event-id", event_ref);
        return Ok(shadow);
    };
    shadow.read(PredicateRead::UniqueKey {
        namespace: "source-event-id".into(),
        key: id.into(),
        owner: event_ref.into(),
    });
    for section in ["inputs", "outputs"] {
        let Some(rows) = event.get(section).and_then(Value::as_array) else {
            shadow.issue("provenance-section", section);
            continue;
        };
        let mut seen = BTreeSet::new();
        for row in rows {
            let (Some(reference), Some(digest)) = (field_str(row, "ref"), field_str(row, "sha256"))
            else {
                continue;
            };
            if !seen.insert(reference) {
                shadow.issue("provenance-duplicate-ref", reference);
            }
            if !reference.starts_with("ToS/") {
                continue;
            }
            let Some(raw) = local_bytes.get(reference) else {
                shadow.skip(format!("provenance-unavailable-local:{reference}"));
                continue;
            };
            let actual = Digest256::of_bytes(raw).to_prefixed();
            if actual != digest && actual.strip_prefix("sha256:") != Some(digest) {
                shadow.issue("provenance-digest", reference);
            }
            shadow.read(PredicateRead::ExactPath {
                path: reference.into(),
                digest: actual,
            });
        }
    }
    shadow.fact(ValidationFact {
        namespace: "source-provenance-event".into(),
        key: id.into(),
        value_digest: Digest256::of_bytes(event_raw).to_prefixed(),
    });
    shadow
        .checked_profiles
        .insert("source-provenance-local-digests@1".into());
    shadow.skip("provenance-global-id-chronology-and-owner-review");
    Ok(shadow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FormatProfile, SchemaResource};
    use serde_json::json;
    use std::path::{Path, PathBuf};

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }
    fn bytes(relative: &str) -> Vec<u8> {
        std::fs::read(root().join(relative)).unwrap()
    }
    fn probe() -> SchemaBackendProbe {
        let mut resources = Vec::new();
        for entry in std::fs::read_dir(root().join("ToS/contracts")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read(path).unwrap();
            let value: Value = serde_json::from_slice(&raw).unwrap();
            if value.get("$schema").and_then(Value::as_str)
                == Some("https://json-schema.org/draft/2020-12/schema")
            {
                resources.push(SchemaResource {
                    uri: value["$id"].as_str().unwrap().into(),
                    raw,
                });
            }
        }
        SchemaBackendProbe::new(resources, FormatProfile::AssertedSourceCandidateV1).unwrap()
    }

    #[test]
    fn real_claim_route_and_typed_endpoint_vectors_are_shadows() {
        const CLAIM_PATH: &str = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/source-claims.jsonl";
        const WORK_PATH: &str =
            "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/work.json";
        const EXPRESSION_PATH: &str = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/expression.json";
        let entity = bytes("ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation = bytes("ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let claim = bytes(CLAIM_PATH);
        let work = bytes(WORK_PATH);
        let expression = bytes(EXPRESSION_PATH);
        let streams = [ClaimStream {
            source_ref: CLAIM_PATH,
            raw: &claim,
        }];
        let endpoints = [
            EndpointRecord {
                source_ref: WORK_PATH,
                raw: &work,
            },
            EndpointRecord {
                source_ref: EXPRESSION_PATH,
                raw: &expression,
            },
        ];
        let backend = probe();
        let run = |raw: &[u8], endpoints: &[EndpointRecord<'_>]| {
            inspect_profiled_claims(
                ClaimFamilyInput {
                    entity_registry_raw: &entity,
                    relation_registry_raw: &relation,
                    claim_streams: &[ClaimStream {
                        source_ref: CLAIM_PATH,
                        raw,
                    }],
                    endpoints,
                    complete_union: true,
                    union_generation: "test-source-union",
                },
                &backend,
            )
            .unwrap()
        };
        let positive = run(streams[0].raw, &endpoints);
        assert!(positive.issues.is_empty(), "{:?}", positive.issues);
        assert!(
            positive.unsupported,
            "the family-local check cannot admit the full corpus"
        );
        assert!(
            positive
                .checked_profiles
                .contains("has_expression@tos_source_relation_claim_v1")
        );
        let mut changed: Value = serde_json::from_slice(&claim).unwrap();
        changed["predicate"] = json!("arbitrary_edge");
        assert!(
            run(&serde_json::to_vec(&changed).unwrap(), &endpoints)
                .issues
                .iter()
                .any(|issue| issue.code == "unrecognized-predicate")
        );
        changed["predicate"] = json!("has_expression");
        changed["schema_version"] = json!("v99");
        assert!(
            run(&serde_json::to_vec(&changed).unwrap(), &endpoints)
                .issues
                .iter()
                .any(|issue| issue.code == "unrecognized-schema-version")
        );
        changed["schema_version"] = json!("tos_source_relation_claim_v1");
        changed["visibility"] = json!("local_only");
        assert!(
            run(&serde_json::to_vec(&changed).unwrap(), &endpoints)
                .issues
                .iter()
                .any(|issue| issue.code == "claim-schema" || issue.code == "claim-public-shape")
        );
        assert!(
            run(&claim, &endpoints[..1])
                .issues
                .iter()
                .any(|issue| issue.code == "missing-object")
        );
        let fake_expression: Value = serde_json::from_slice(&expression).unwrap();
        let mut fake_expression = fake_expression;
        fake_expression["record_type"] = json!("work");
        let fake_raw = serde_json::to_vec(&fake_expression).unwrap();
        let wrong_endpoints = [
            EndpointRecord {
                source_ref: WORK_PATH,
                raw: &work,
            },
            EndpointRecord {
                source_ref: EXPRESSION_PATH,
                raw: &fake_raw,
            },
        ];
        assert!(
            run(&claim, &wrong_endpoints)
                .issues
                .iter()
                .any(|issue| issue.code == "object-range")
        );
    }

    #[test]
    fn real_structured_member_claim_retains_native_endpoint_gap() {
        const CLAIM_PATH: &str =
            "ToS/source-witnesses/relations/oim-a00645-physical-composition/source-claims.jsonl";
        const WHOLE: &str = "ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645-plus-a00649a-i/artifact-witness.json";
        const MEMBER: &str =
            "ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645/artifact-witness.json";
        let entity = bytes("ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation = bytes("ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let claim = bytes(CLAIM_PATH);
        let whole = bytes(WHOLE);
        let member = bytes(MEMBER);
        let endpoints = [
            EndpointRecord {
                source_ref: WHOLE,
                raw: &whole,
            },
            EndpointRecord {
                source_ref: MEMBER,
                raw: &member,
            },
        ];
        let backend = probe();
        let check = |raw: &[u8]| {
            inspect_profiled_claims(
                ClaimFamilyInput {
                    entity_registry_raw: &entity,
                    relation_registry_raw: &relation,
                    claim_streams: &[ClaimStream {
                        source_ref: CLAIM_PATH,
                        raw,
                    }],
                    endpoints: &endpoints,
                    complete_union: true,
                    union_generation: "test-union",
                },
                &backend,
            )
            .unwrap()
        };
        let positive = check(&claim);
        assert!(positive.issues.is_empty(), "{:?}", positive.issues);
        assert!(positive.unsupported);
        assert!(
            positive
                .skipped_profiles
                .contains("native-artifact-endpoint-adapter")
        );
        let mut duplicate: Value = serde_json::from_slice(&claim).unwrap();
        let member_id = duplicate["object"]["members"][0].clone();
        duplicate["object"]["members"]
            .as_array_mut()
            .unwrap()
            .push(member_id);
        assert!(
            check(&serde_json::to_vec(&duplicate).unwrap())
                .issues
                .iter()
                .any(|issue| issue.code == "claim-schema"
                    || issue.code == "reference-member-duplicate-or-self")
        );
        let mut absent: Value = serde_json::from_slice(&claim).unwrap();
        absent["object"]["members"] = json!(["tos.artifact.absent"]);
        assert!(
            check(&serde_json::to_vec(&absent).unwrap())
                .issues
                .iter()
                .any(|issue| issue.code == "reference-member-type-or-missing")
        );
    }

    #[test]
    fn topology_exact_forward_reverse_and_pair_oracles() {
        let work = json!({"record_id":"tos.work.a","record_type":"work","expression_claim_refs":["tos.claim.a"]});
        let expression = json!({"record_id":"tos.expression.b","record_type":"expression","work_ref":"tos.work.a","embodiment_claim_refs":[]});
        let edge = json!({"claim_id":"tos.claim.a","predicate":"has_expression","subject_ref":"tos.work.a","object":"tos.expression.b"});
        let items = BTreeMap::new();
        let positive = inspect_current_topology(
            &[work.clone(), expression.clone()],
            &[edge.clone()],
            &items,
            true,
            "test-union",
        );
        assert!(positive.issues.is_empty(), "{:?}", positive.issues);
        assert!(positive.unsupported);
        let mixed = inspect_current_topology(
            &[work.clone(), expression.clone()],
            &[edge.clone(), json!({"predicate":"contains_work"})],
            &items,
            true,
            "test-union",
        );
        assert!(mixed.issues.is_empty(), "{:?}", mixed.issues);
        assert!(
            mixed
                .skipped_profiles
                .contains("non-topology-claim-profile:contains_work")
        );
        assert!(positive.reads.iter().any(|read| matches!(read,
            PredicateRead::Range { namespace, lower, upper, .. }
            if namespace == "source-topology-current-claims"
                && lower == "embodied_by" && upper == "embodied_by"
        )));
        assert!(positive.reads.iter().any(|read| matches!(read,
            PredicateRead::ReverseRefs { target, relation, .. }
            if target == "tos.expression.b" && relation == "source-topology:has_expression:object"
        )));
        assert!(positive.facts.iter().any(|fact| fact.namespace
            == "bibliographic-topology-subject-closure"
            && fact.key == r#"["has_expression","tos.work.a"]"#));
        let mut negative = edge.clone();
        negative["polarity"] = json!("negative");
        let negative_result = inspect_current_topology(
            &[work.clone(), expression.clone()],
            &[negative],
            &items,
            true,
            "test-union",
        );
        assert!(
            negative_result.issues.is_empty(),
            "{:?}",
            negative_result.issues
        );
        assert!(
            !negative_result
                .reads
                .iter()
                .any(|read| matches!(read, PredicateRead::AbsentKey { .. }))
        );
        let claim_digest = |shadow: &RelationShadow| {
            shadow
                .facts
                .iter()
                .find(|fact| fact.namespace == "bibliographic-topology-claim-value")
                .unwrap()
                .value_digest
                .clone()
        };
        assert_ne!(claim_digest(&positive), claim_digest(&negative_result));
        let mut unknown = edge.clone();
        unknown["polarity"] = json!("unknown");
        assert!(
            inspect_current_topology(
                &[work.clone(), expression.clone()],
                &[unknown],
                &items,
                true,
                "test-union"
            )
            .issues
            .is_empty()
        );
        let edition = json!({"record_id":"tos.edition.c","record_type":"edition",
            "embodies_expression_refs":[],"exemplar_claim_refs":[]});
        assert!(
            inspect_current_topology(
                &[work.clone(), expression.clone(), edition],
                &[edge.clone()],
                &items,
                true,
                "test-union"
            )
            .issues
            .is_empty()
        );
        let missing = inspect_current_topology(
            &[work.clone(), expression.clone()],
            &[],
            &items,
            true,
            "test-union",
        );
        assert!(
            missing
                .issues
                .iter()
                .any(|issue| issue.code == "topology-reverse-closure")
        );
        let mut reverse = work.clone();
        reverse["expression_claim_refs"] = json!([]);
        assert!(
            inspect_current_topology(
                &[reverse, expression.clone()],
                &[edge.clone()],
                &items,
                true,
                "test-union"
            )
            .issues
            .iter()
            .any(|issue| issue.code == "topology-reverse-closure")
        );
        let mut other = edge.clone();
        other["claim_id"] = json!("tos.claim.b");
        let mut unbacked = edge.clone();
        unbacked["object"] = json!("tos.expression.absent");
        let bad_endpoint = inspect_current_topology(
            &[work.clone(), expression.clone()],
            &[unbacked],
            &items,
            true,
            "test-union",
        );
        assert!(
            bad_endpoint
                .issues
                .iter()
                .any(|issue| issue.code == "topology-link-unbacked")
        );
        assert!(bad_endpoint.reads.iter().any(|read| matches!(read,
            PredicateRead::RefEndpoint { id, observed: KeyState::Absent, .. }
            if id == "tos.expression.absent"
        )));
        let mut mistyped = expression.clone();
        mistyped["work_ref"] = json!("tos.expression.b");
        assert!(
            inspect_current_topology(
                &[work.clone(), mistyped],
                &[edge.clone()],
                &items,
                true,
                "test-union"
            )
            .issues
            .iter()
            .any(|issue| issue.code == "topology-declared-link-endpoint")
        );
        assert!(
            inspect_current_topology(
                &[work.clone(), expression.clone()],
                &[edge.clone(), edge.clone()],
                &items,
                true,
                "test-union"
            )
            .issues
            .iter()
            .any(|issue| issue.code == "duplicate-topology-claim-id")
        );
        assert!(
            inspect_current_topology(
                &[work, expression],
                &[edge, other],
                &items,
                true,
                "test-union"
            )
            .issues
            .iter()
            .any(|issue| issue.code == "duplicate-topology-pair")
        );
    }

    #[test]
    fn real_provenance_local_digest_and_mutation_oracles() {
        let path = "ToS/source-witnesses/discovery/provenance.jsonl";
        let file = bytes(path);
        let event_raw = file.split(|byte| *byte == b'\n').next().unwrap();
        let event: Value = serde_json::from_slice(event_raw).unwrap();
        let mut local = BTreeMap::new();
        for section in ["inputs", "outputs"] {
            for row in event[section].as_array().unwrap() {
                let Some(reference) = row["ref"].as_str() else {
                    continue;
                };
                if reference.starts_with("ToS/") && row.get("sha256").is_some() {
                    local.insert(reference.to_owned(), bytes(reference));
                }
            }
        }
        let backend = probe();
        let positive = inspect_provenance_event(path, event_raw, &local, &backend).unwrap();
        assert!(positive.issues.is_empty(), "{:?}", positive.issues);
        assert!(positive.unsupported);
        let first = event["outputs"][0]["ref"].as_str().unwrap();
        local.insert(first.to_owned(), b"different bytes".to_vec());
        let altered = inspect_provenance_event(path, event_raw, &local, &backend).unwrap();
        assert!(
            altered
                .issues
                .iter()
                .any(|issue| issue.code == "provenance-digest")
        );
    }
}
