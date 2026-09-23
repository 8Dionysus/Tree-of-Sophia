//! Source-derived relation checks for an explicitly supplied complete candidate view.
//!
//! This is a family-local shadow. The caller supplies exact raw bytes and the
//! current union; neither this module nor a successful `RelationShadow` can
//! construct a corpus admission result or attest that the supplied union is
//! complete. Special source profiles remain named, explicit gaps.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use tos_foundation::Digest256;

use crate::{
    KeyState, PredicateRead, SchemaBackendProbe, SchemaProbeError, ValidationFact, published_value,
};

const MAX_REGISTRY_BYTES: usize = 1_048_576;
const MAX_CLAIM_FILE_BYTES: usize = 16_777_216;
const MAX_CLAIM_BYTES: usize = 1_048_576;
const MAX_RECORD_BYTES: usize = 1_048_576;
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
    pub checked_profiles: BTreeSet<String>,
    pub skipped_profiles: BTreeSet<String>,
    pub unsupported: bool,
}

impl RelationShadow {
    fn issue(&mut self, code: &'static str, location: impl Into<String>) {
        self.issues.push(RelationIssue {
            code,
            location: location.into(),
        });
    }

    fn skip(&mut self, profile: impl Into<String>) {
        self.unsupported = true;
        self.skipped_profiles.insert(profile.into());
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

fn relation_profile_routes(
    relation_registry: &Value,
    shadow: &mut RelationShadow,
) -> Result<
    BTreeMap<String, (String, Vec<String>, Vec<String>, BTreeMap<String, String>)>,
    RelationError,
> {
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
                (reader.to_owned(), domain, range, versions),
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
        shadow.reads.push(PredicateRead::Registry {
            uri: uri.to_owned(),
            version: registry
                .get("registry_version")
                .map(Value::to_string)
                .unwrap_or_default(),
            digest: Digest256::of_bytes(raw).to_prefixed(),
        });
        if let Some(read) = resource_read(probe, uri) {
            shadow.reads.push(read);
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
    let mut endpoint_types = BTreeMap::new();
    for endpoint in input.endpoints {
        let value = raw_value(endpoint.raw, MAX_RECORD_BYTES)?;
        let (Some(id), Some(kind)) = (
            field_str(&value, "record_id"),
            field_str(&value, "record_type"),
        ) else {
            shadow.issue("endpoint-record-shape", endpoint.source_ref);
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
        shadow.reads.push(PredicateRead::ExactPath {
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
        shadow.reads.push(PredicateRead::ExactPath {
            path: stream.source_ref.to_owned(),
            digest: Digest256::of_bytes(stream.raw).to_prefixed(),
        });
        for (index, line) in stream.raw.split(|byte| *byte == b'\n').enumerate() {
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
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
            shadow.reads.push(PredicateRead::UniqueKey {
                namespace: "source-claim-id".into(),
                key: claim_id.to_owned(),
                owner: location.clone(),
            });
            let Some((reader, domain, range, versions)) = routes.get(predicate) else {
                shadow.issue("unrecognized-predicate", location);
                continue;
            };
            let Some(schema_ref) = versions.get(version) else {
                shadow.issue("unrecognized-schema-version", location);
                continue;
            };
            let uri = format!("https://tree-of-sophia.local/{schema_ref}");
            let route_id = format!("{predicate}@{version}");
            let supported = (reader == "identity-relation-v1"
                && schema_ref == "ToS/contracts/source-relation-claim.schema.json")
                || (reader == "semantic-relation-v1"
                    && schema_ref == "ToS/contracts/semantic-relation-claim.schema.json");
            if supported {
                shadow.checked_profiles.insert(route_id.clone());
            } else {
                shadow.skip(format!("{reader}:{route_id}"));
            }
            if !schema_valid(probe, &uri, line)? || !schema_valid(probe, CLAIM_BASE, line)? {
                shadow.issue("claim-schema", &location);
                continue;
            }
            if let Some(read) = resource_read(probe, &uri) {
                shadow.reads.push(read);
            }
            if let Some(read) = resource_read(probe, CLAIM_BASE) {
                shadow.reads.push(read);
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
            let Some(actual_subject) = endpoint_types.get(subject) else {
                shadow.issue("missing-subject", &location);
                continue;
            };
            shadow.reads.push(PredicateRead::RefEndpoint {
                endpoint_type: actual_subject.clone(),
                id: subject.to_owned(),
                observed: KeyState::Present,
            });
            if !ancestry_contains(&types, actual_subject, domain) {
                shadow.issue("subject-domain", &location);
            }
            if let Some(object) = claim.get("object").and_then(Value::as_str) {
                if object == claim_id {
                    shadow.issue("self-object-claim-id", &location);
                }
                let Some(actual_object) = endpoint_types.get(object) else {
                    shadow.issue("missing-object", &location);
                    continue;
                };
                shadow.reads.push(PredicateRead::RefEndpoint {
                    endpoint_type: actual_object.clone(),
                    id: object.to_owned(),
                    observed: KeyState::Present,
                });
                if !ancestry_contains(&types, actual_object, range) {
                    shadow.issue("object-range", &location);
                }
                shadow.reads.push(PredicateRead::ReverseRefs {
                    target: object.to_owned(),
                    relation: predicate.to_owned(),
                    generation: input.union_generation.to_owned(),
                });
            } else if supported {
                shadow.issue("object-type", &location);
            }
            // Negative and unknown polarity are Claim values, not an absence assertion.
            shadow.facts.push(ValidationFact {
                namespace: "source-claim-assertion".into(),
                key: claim_id.to_owned(),
                value_digest: Digest256::of_bytes(line).to_prefixed(),
            });
        }
    }
    shadow.reads.push(PredicateRead::Range {
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

/// A separately verified current union is required: retained legacy and
/// compound native Claims must already have their source/provenance bindings.
pub fn inspect_current_topology(
    records: &[Value],
    claims: &[Value],
    item_edition: &BTreeMap<String, String>,
    verified_complete_union: bool,
    union_generation: &str,
) -> RelationShadow {
    let mut shadow = RelationShadow::default();
    if !verified_complete_union || union_generation.is_empty() {
        shadow.skip("verified-current-topology-union");
        return shadow;
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
    for record in records {
        if let (Some(id), Some(_)) = (
            field_str(record, "record_id"),
            field_str(record, "record_type"),
        ) {
            if by_id.insert(id.to_owned(), record).is_some() {
                shadow.issue("duplicate-record-id", id);
            }
        } else {
            shadow.issue("record-shape", "records");
        }
    }
    let mut expected: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for record in records {
        let (Some(id), Some(kind)) = (
            field_str(record, "record_id"),
            field_str(record, "record_type"),
        ) else {
            continue;
        };
        if kind == "expression" {
            if let Some(work) = field_str(record, "work_ref") {
                expected
                    .entry(("has_expression".into(), work.into()))
                    .or_default()
                    .insert(id.into());
            } else {
                shadow.issue("missing-work-ref", id);
            }
        } else if kind == "edition" {
            if let Some(refs) = string_array(record, "embodies_expression_refs") {
                if refs.is_empty() || refs.iter().collect::<BTreeSet<_>>().len() != refs.len() {
                    shadow.issue("invalid-expression-refs", id);
                }
                for expression in refs {
                    expected
                        .entry(("embodied_by".into(), expression))
                        .or_default()
                        .insert(id.into());
                }
            } else {
                shadow.issue("invalid-expression-refs", id);
            }
        } else if kind == "item" && !item_edition.contains_key(id) {
            shadow.issue("missing-item-embodiment", id);
        }
    }
    for (item, edition) in item_edition {
        expected
            .entry(("exemplified_by".into(), edition.clone()))
            .or_default()
            .insert(item.clone());
    }
    let mut actual: BTreeMap<(String, String), BTreeMap<String, String>> = BTreeMap::new();
    let mut seen_ids = BTreeSet::new();
    let mut seen_pairs = BTreeSet::new();
    let mut expression_works = BTreeMap::<String, String>::new();
    for claim in claims {
        let Some(predicate) = field_str(claim, "predicate") else {
            continue;
        };
        let Some((_, subject_type, object_type, _)) =
            ROUTES.iter().find(|route| route.0 == predicate)
        else {
            continue;
        };
        let (Some(id), Some(subject), Some(object)) = (
            field_str(claim, "claim_id"),
            field_str(claim, "subject_ref"),
            field_str(claim, "object"),
        ) else {
            shadow.issue("topology-claim-shape", predicate);
            continue;
        };
        if !seen_ids.insert(id.to_owned()) {
            shadow.issue("duplicate-topology-claim-id", id);
        }
        if !seen_pairs.insert((predicate.to_owned(), subject.to_owned(), object.to_owned())) {
            shadow.issue("duplicate-topology-pair", id);
        }
        if by_id.get(subject).and_then(|r| field_str(r, "record_type")) != Some(*subject_type)
            || by_id.get(object).and_then(|r| field_str(r, "record_type")) != Some(*object_type)
        {
            shadow.issue("topology-endpoint", id);
            continue;
        }
        shadow.reads.push(PredicateRead::RefEndpoint {
            endpoint_type: (*subject_type).into(),
            id: subject.into(),
            observed: KeyState::Present,
        });
        shadow.reads.push(PredicateRead::RefEndpoint {
            endpoint_type: (*object_type).into(),
            id: object.into(),
            observed: KeyState::Present,
        });
        if !expected
            .get(&(predicate.into(), subject.into()))
            .is_some_and(|targets| targets.contains(object))
        {
            shadow.issue("topology-link-unbacked", id);
        }
        if predicate == "has_expression"
            && expression_works
                .insert(object.into(), subject.into())
                .is_some_and(|previous| previous != subject)
        {
            shadow.issue("expression-two-work-owners", id);
        }
        actual
            .entry((predicate.into(), subject.into()))
            .or_default()
            .insert(id.into(), object.into());
        shadow.facts.push(ValidationFact {
            namespace: "bibliographic-topology-claim".into(),
            key: id.into(),
            value_digest: Digest256::of_bytes(
                format!("{predicate}\0{subject}\0{object}").as_bytes(),
            )
            .to_prefixed(),
        });
    }
    for (predicate, subject_type, _, field) in ROUTES {
        for record in records {
            if field_str(record, "record_type") != Some(subject_type) {
                continue;
            }
            let Some(subject) = field_str(record, "record_id") else {
                continue;
            };
            let refs = string_array(record, field);
            let found = actual.get(&(predicate.into(), subject.into()));
            let found_ids: BTreeSet<String> = found
                .map(|rows| rows.keys().cloned().collect())
                .unwrap_or_default();
            if refs.as_ref().is_none_or(|items| {
                items.iter().cloned().collect::<BTreeSet<_>>() != found_ids
                    || items.len() != found_ids.len()
            }) {
                shadow.issue("topology-reverse-closure", subject);
            }
            let found_targets: BTreeSet<String> = found
                .map(|rows| rows.values().cloned().collect())
                .unwrap_or_default();
            if found_targets
                != expected
                    .get(&(predicate.into(), subject.into()))
                    .cloned()
                    .unwrap_or_default()
            {
                shadow.issue("topology-forward-closure", subject);
            }
            shadow.reads.push(PredicateRead::Range {
                namespace: format!("topology:{predicate}:subject"),
                lower: subject.into(),
                upper: subject.into(),
                generation: union_generation.into(),
            });
        }
    }
    for ((predicate, subject), targets) in &expected {
        if !by_id.contains_key(subject) {
            shadow.issue("topology-missing-source", subject);
        }
        for target in targets {
            if !by_id.contains_key(target) {
                shadow.issue("topology-missing-target", target);
            }
        }
        shadow.reads.push(PredicateRead::ReverseRefs {
            target: subject.clone(),
            relation: predicate.clone(),
            generation: union_generation.into(),
        });
    }
    shadow
        .checked_profiles
        .insert("source-bibliographic-topology-current-union@1".into());
    shadow.skip("compound-native-and-retained-provenance-verification");
    shadow
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
    let event = raw_value(event_raw, MAX_RECORD_BYTES)?;
    if !schema_valid(probe, PROVENANCE_CONTRACT, event_raw)? {
        shadow.issue("provenance-schema", event_ref);
        return Ok(shadow);
    }
    shadow.reads.push(PredicateRead::ExactPath {
        path: event_ref.into(),
        digest: Digest256::of_bytes(event_raw).to_prefixed(),
    });
    if let Some(read) = resource_read(probe, PROVENANCE_CONTRACT) {
        shadow.reads.push(read);
    }
    let Some(id) = field_str(&event, "event_id") else {
        shadow.issue("provenance-event-id", event_ref);
        return Ok(shadow);
    };
    shadow.reads.push(PredicateRead::UniqueKey {
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
            shadow.reads.push(PredicateRead::ExactPath {
                path: reference.into(),
                digest: actual,
            });
        }
    }
    shadow.facts.push(ValidationFact {
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
