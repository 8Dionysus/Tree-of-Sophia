//! Source-declared record profile checks for a future full auditor.
//!
//! This module reports observations and ID owners; it never returns an
//! admission result. The caller must prove complete member enumeration,
//! native-adapter coverage, reference closure, history and catalog parity.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tos_foundation::Digest256;

use crate::source_cut::{CutSchemaExecutor, CutSchemaReceiptRange};

use crate::{FormatProfile, SchemaBackendProbe, SchemaProbeError, SchemaResource, published_value};

const ENTITY_REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const ENTITY_CONTRACT: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";
const CORPUS_CONTRACT: &str = "ToS/contracts/corpus-record.schema.json";
const SOURCE_HOME: &str = "ToS/source-witnesses/";
const COMMON_URI: &str = "https://tree-of-sophia.local/__val/record-common-v1";
const MAX_RECORD_BYTES: usize = 1_048_576;
const MAX_SOURCE_RESOURCES: usize = 512;
const MAX_SOURCE_RESOURCE_BYTES: usize = 32 * 1_048_576;
const MAX_COMPILED_ROUTES: usize = 256;

/// Caller-selected execution quota, not a source-universe cardinality rule.
/// Large external-sort audits can raise this without changing rule semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordFactBudget {
    pub max_facts: u64,
    pub max_encoded_bytes: u64,
}

impl RecordFactBudget {
    /// A bounded laboratory default for shadow tests only. Audit callers must
    /// pass a quota chosen for their complete immutable membership cut.
    pub const fn laboratory_default() -> Self {
        Self {
            max_facts: 1_000_000,
            max_encoded_bytes: 128 * 1_048_576,
        }
    }
}

/// One exact verdict returned by a separately bounded schema worker. The
/// auditor owns worker execution, timeout, process identity and complete
/// response accounting; a value alone is never proof of those properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundedSchemaVerdict {
    pub instance_sha256: Digest256,
    pub schema_set_digest: Digest256,
    pub format_profile: FormatProfile,
    pub root_uri: String,
    pub worker_protocol_id: String,
    pub worker_binary_digest: Digest256,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundedMemberSchemaEvidence {
    pub route: BoundedSchemaVerdict,
    pub common: BoundedSchemaVerdict,
}

/// The exact resources and roots the worker must evaluate for one member.
/// This is a request, not a verdict or proof of worker execution.
pub(crate) struct BoundedMemberSchemaPlan {
    pub resources: Vec<SchemaResource>,
    pub route_uri: String,
    pub common_uri: String,
    pub schema_set_digest: Digest256,
    pub format_profile: FormatProfile,
    pub instance_sha256: Digest256,
}

/// Exact source-owned resource bytes. The URI is read from `$id`, not chosen by
/// an instance record. Neither this list nor registry data can supply code.
#[derive(Debug, Clone)]
pub struct RecordSchema<'a> {
    pub path: &'a str,
    pub raw: &'a [u8],
}

/// Source-route classification for a complete current carrier traversal.
/// Schema/identity/authority validation remains mandatory and separate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentRecordCarrier {
    pub id: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordRuleError {
    Unsupported { code: &'static str, detail: String },
    Budget { code: &'static str },
    Sink { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordObservation {
    ExactPath {
        path: String,
        raw_sha256: String,
    },
    Registry {
        path: String,
        version: String,
        raw_sha256: String,
    },
    Schema {
        path: String,
        uri: String,
        raw_sha256: String,
    },
    Profile {
        path: String,
        kind: String,
        profile_version: u64,
        schema_version: String,
    },
    Reference {
        from_path: String,
        target_path: String,
        check: PathReferenceCheck,
    },
    RecordIdReference {
        from_path: String,
        target_id: String,
        expected_kind: &'static str,
    },
    LinkUriOwner {
        uri: String,
        id: String,
        path: String,
    },
    IdOwner {
        id: String,
        kind: String,
        path: String,
        version: u64,
        raw_sha256: String,
    },
    /// Separate kind-scoped lookup namespace for typed references.
    IdKindOwner {
        kind: String,
        id: String,
        path: String,
    },
    /// A native packet can reserve the same semantic ID in successive packet
    /// versions; cross-carrier collision checks use this distinct fact.
    NativeReservation {
        id: String,
        packet_path: String,
        raw_sha256: String,
    },
    Issue {
        path: String,
        code: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathReferenceCheck {
    /// `_validate_source_refs`: non-ToS strings are allowed external locators.
    RepoExistsIfToS,
    /// Foundation Link `observation_ref` requires a regular file if ToS-local.
    FileIfToS,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdCarrier {
    Standalone,
    NativePacket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GlobalIdFact {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub carrier: IdCarrier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkUriFact {
    pub uri: String,
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypedIdRefFact {
    pub target_id: String,
    pub expected_kind: String,
    pub from_path: String,
}

impl RecordObservation {
    pub(crate) fn into_global_id_fact(self) -> Option<GlobalIdFact> {
        match self {
            Self::IdOwner { id, kind, path, .. } => Some(GlobalIdFact {
                id,
                kind,
                path,
                carrier: IdCarrier::Standalone,
            }),
            Self::NativeReservation {
                id, packet_path, ..
            } => Some(GlobalIdFact {
                kind: id.split('.').nth(1).unwrap_or("").to_owned(),
                id,
                path: packet_path,
                carrier: IdCarrier::NativePacket,
            }),
            _ => None,
        }
    }

    pub(crate) fn into_link_uri_fact(self) -> Option<LinkUriFact> {
        match self {
            Self::LinkUriOwner { uri, id, path } => Some(LinkUriFact { uri, id, path }),
            _ => None,
        }
    }

    pub(crate) fn into_typed_id_ref_fact(self) -> Option<TypedIdRefFact> {
        match self {
            Self::RecordIdReference {
                from_path,
                target_id,
                expected_kind,
            } => Some(TypedIdRefFact {
                target_id,
                expected_kind: expected_kind.to_owned(),
                from_path,
            }),
            _ => None,
        }
    }
}

/// A pure streaming join over auditor-supplied, externally sorted *complete*
/// current facts. This cannot establish membership or admission by itself.
pub(crate) struct RecordGlobalJoin;

impl RecordGlobalJoin {
    pub fn check_id_collisions(
        facts: impl IntoIterator<Item = GlobalIdFact>,
        limit: RecordFactBudget,
        sink: &mut impl RecordSink,
    ) -> Result<u64, RecordRuleError> {
        let mut budget = GlobalFactBudget::new(limit);
        let mut previous_id = String::new();
        let mut first_owner: Option<String> = None;
        let mut native_seen = false;
        let mut issues = 0;
        for fact in facts {
            budget.check(&[&fact.id, &fact.kind, &fact.path])?;
            if fact.id < previous_id {
                return Err(unsupported("id_facts_unsorted", &fact.id));
            }
            if fact.id != previous_id {
                previous_id = fact.id.clone();
                first_owner = None;
                native_seen = false;
            }
            match fact.carrier {
                IdCarrier::Standalone => {
                    if first_owner.is_some() || native_seen {
                        issues += 1;
                        emit(
                            sink,
                            RecordObservation::Issue {
                                path: fact.path.clone(),
                                code: if native_seen {
                                    "native_record_id_collision"
                                } else {
                                    "duplicate_record_id"
                                },
                            },
                        )?;
                    }
                    if first_owner.is_none() {
                        first_owner = Some(fact.path);
                    }
                }
                IdCarrier::NativePacket => {
                    if first_owner.is_some() && !native_seen {
                        issues += 1;
                        emit(
                            sink,
                            RecordObservation::Issue {
                                path: fact.path,
                                code: "native_record_id_collision",
                            },
                        )?;
                    }
                    native_seen = true;
                }
            }
        }
        Ok(issues)
    }

    pub fn check_link_uri_collisions(
        facts: impl IntoIterator<Item = LinkUriFact>,
        limit: RecordFactBudget,
        sink: &mut impl RecordSink,
    ) -> Result<u64, RecordRuleError> {
        let mut budget = GlobalFactBudget::new(limit);
        let mut previous_uri: Option<String> = None;
        let mut issues = 0;
        for fact in facts {
            budget.check(&[&fact.uri, &fact.id, &fact.path])?;
            if previous_uri
                .as_ref()
                .is_some_and(|previous| fact.uri.as_str() < previous.as_str())
            {
                return Err(unsupported("link_uri_facts_unsorted", &fact.uri));
            }
            if previous_uri
                .as_ref()
                .is_some_and(|previous| fact.uri.as_str() == previous.as_str())
            {
                issues += 1;
                emit(
                    sink,
                    RecordObservation::Issue {
                        path: fact.path,
                        code: "duplicate_link_uri",
                    },
                )?;
            }
            previous_uri = Some(fact.uri);
        }
        Ok(issues)
    }

    pub fn check_typed_references(
        owners: impl IntoIterator<Item = GlobalIdFact>,
        references: impl IntoIterator<Item = TypedIdRefFact>,
        limit: RecordFactBudget,
        sink: &mut impl RecordSink,
    ) -> Result<u64, RecordRuleError> {
        let mut budget = GlobalFactBudget::new(limit);
        let mut owners = owners.into_iter();
        let mut last_owner_id = String::new();
        let mut current = next_owner(&mut owners, &mut last_owner_id, &mut budget)?;
        let mut last_ref_id = String::new();
        let mut issues = 0;
        for reference in references {
            budget.check(&[
                &reference.target_id,
                &reference.expected_kind,
                &reference.from_path,
            ])?;
            if reference.target_id < last_ref_id {
                return Err(unsupported(
                    "typed_ref_facts_unsorted",
                    &reference.target_id,
                ));
            }
            last_ref_id = reference.target_id.clone();
            while current
                .as_ref()
                .is_some_and(|owner| owner.id < reference.target_id)
            {
                current = next_owner(&mut owners, &mut last_owner_id, &mut budget)?;
            }
            let code = match current.as_ref() {
                Some(owner)
                    if owner.id == reference.target_id
                        && owner.kind == reference.expected_kind
                        && owner.carrier == IdCarrier::Standalone =>
                {
                    None
                }
                Some(owner) if owner.id == reference.target_id => {
                    Some("record_reference_wrong_kind")
                }
                _ => Some("record_reference_missing"),
            };
            if let Some(code) = code {
                issues += 1;
                emit(
                    sink,
                    RecordObservation::Issue {
                        path: reference.from_path,
                        code,
                    },
                )?;
            }
        }
        while next_owner(&mut owners, &mut last_owner_id, &mut budget)?.is_some() {}
        Ok(issues)
    }
}

struct GlobalFactBudget {
    limit: RecordFactBudget,
    count: u64,
    bytes: u64,
}

impl GlobalFactBudget {
    fn new(limit: RecordFactBudget) -> Self {
        Self {
            limit,
            count: 0,
            bytes: 0,
        }
    }

    fn check(&mut self, fields: &[&str]) -> Result<(), RecordRuleError> {
        self.count = self.count.checked_add(1).ok_or(RecordRuleError::Budget {
            code: "global_fact_count",
        })?;
        let size = fields
            .iter()
            .try_fold(0u64, |sum, field| {
                sum.checked_add(u64::try_from(field.len()).ok()?)
            })
            .ok_or(RecordRuleError::Budget {
                code: "global_fact_bytes",
            })?;
        self.bytes = self
            .bytes
            .checked_add(size)
            .ok_or(RecordRuleError::Budget {
                code: "global_fact_bytes",
            })?;
        if self.count > self.limit.max_facts || self.bytes > self.limit.max_encoded_bytes {
            return Err(RecordRuleError::Budget {
                code: "global_fact_budget",
            });
        }
        Ok(())
    }
}

fn next_owner(
    owners: &mut impl Iterator<Item = GlobalIdFact>,
    last_id: &mut String,
    budget: &mut GlobalFactBudget,
) -> Result<Option<GlobalIdFact>, RecordRuleError> {
    let Some(owner) = owners.next() else {
        return Ok(None);
    };
    budget.check(&[&owner.id, &owner.kind, &owner.path])?;
    if owner.id < *last_id {
        return Err(unsupported("id_facts_unsorted", &owner.id));
    }
    *last_id = owner.id.clone();
    Ok(Some(owner))
}

/// A sink may spill sorted ID owners and reference queries. Its error must
/// prevent the caller from treating a truncated output as complete.
pub trait RecordSink {
    fn push(&mut self, observation: RecordObservation) -> Result<(), String>;
}

struct AccountingSink<'a, S: RecordSink> {
    inner: &'a mut S,
    limit: RecordFactBudget,
    count: u64,
    bytes: u64,
    budget_failed: bool,
}

impl<'a, S: RecordSink> AccountingSink<'a, S> {
    fn new(inner: &'a mut S, limit: RecordFactBudget, count: u64, bytes: u64) -> Self {
        Self {
            inner,
            limit,
            count,
            bytes,
            budget_failed: false,
        }
    }
}

impl<S: RecordSink> RecordSink for AccountingSink<'_, S> {
    fn push(&mut self, observation: RecordObservation) -> Result<(), String> {
        let size = observation_size(&observation);
        let Some(next_count) = self.count.checked_add(1) else {
            self.budget_failed = true;
            return Err("record fact count overflow".to_owned());
        };
        let Some(next_bytes) = self.bytes.checked_add(size) else {
            self.budget_failed = true;
            return Err("record fact byte overflow".to_owned());
        };
        if next_count > self.limit.max_facts || next_bytes > self.limit.max_encoded_bytes {
            self.budget_failed = true;
            return Err("record fact budget exceeded".to_owned());
        }
        self.inner.push(observation)?;
        self.count = next_count;
        self.bytes = next_bytes;
        Ok(())
    }
}

fn observation_size(observation: &RecordObservation) -> u64 {
    let fields: Vec<&str> = match observation {
        RecordObservation::ExactPath { path, raw_sha256 } => vec![path, raw_sha256],
        RecordObservation::Registry {
            path,
            version,
            raw_sha256,
        } => vec![path, version, raw_sha256],
        RecordObservation::Schema {
            path,
            uri,
            raw_sha256,
        } => vec![path, uri, raw_sha256],
        RecordObservation::Profile {
            path,
            kind,
            schema_version,
            ..
        } => vec![path, kind, schema_version],
        RecordObservation::Reference {
            from_path,
            target_path,
            ..
        } => vec![from_path, target_path],
        RecordObservation::RecordIdReference {
            from_path,
            target_id,
            expected_kind,
        } => vec![from_path, target_id, expected_kind],
        RecordObservation::LinkUriOwner { uri, id, path } => vec![uri, id, path],
        RecordObservation::IdOwner {
            id,
            kind,
            path,
            raw_sha256,
            ..
        } => vec![id, kind, path, raw_sha256],
        RecordObservation::IdKindOwner { kind, id, path } => vec![kind, id, path],
        RecordObservation::NativeReservation {
            id,
            packet_path,
            raw_sha256,
        } => vec![id, packet_path, raw_sha256],
        RecordObservation::Issue { path, code } => vec![path, code],
    };
    // Six bytes per input byte conservatively covers JSON escape expansion;
    // the parent spill sink still owns its exact encoded-byte quota.
    fields.into_iter().fold(32u64, |total, field| {
        total.saturating_add((field.len() as u64).saturating_mul(6))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFamilyReport {
    pub registry_version: String,
    pub registry_sha256: String,
    pub inspected_members: u64,
    pub issue_count: u64,
    pub emitted_member_fact_count: u64,
    pub emitted_member_fact_bytes: u64,
    pub enumerated_profile_ids: Vec<String>,
    pub skipped_profile_ids: Vec<String>,
    /// Always true: whole-source admission also needs other owner modules.
    pub incomplete: bool,
}

#[derive(Clone)]
struct Route {
    schema_ref: String,
    schema_dependencies: Vec<String>,
}

#[derive(Clone)]
struct Profile {
    kind: String,
    version: u64,
    reader: String,
    id_prefix: String,
    basename: String,
    catalog_filename: String,
    routes: BTreeMap<String, Route>,
}

struct Entity<'a> {
    parents: Vec<&'a str>,
    role: &'a str,
    abstract_type: bool,
    mappings: Vec<(&'a str, &'a str)>,
    profile: Option<&'a Value>,
}

pub struct RecordFamily {
    registry_sha256: String,
    registry_version: String,
    resources: BTreeMap<String, (String, Vec<u8>)>,
    profiles: BTreeMap<String, Profile>,
    compiled_routes: BTreeMap<(String, String), SchemaBackendProbe>,
    format_profile: FormatProfile,
    fact_budget: RecordFactBudget,
    inspected_members: u64,
    issue_count: u64,
    seen_profiles: BTreeSet<String>,
    native_packet_count: usize,
    native_packet_bytes: usize,
    fact_count: u64,
    fact_bytes: u64,
}

#[derive(Clone, Copy)]
struct NativeCarrier {
    kind: &'static str,
    schema_path: &'static str,
    schema_version: &'static str,
    id_field: &'static str,
    semantic_packet: bool,
}

pub(crate) struct NativeSchemaPlan {
    pub resources: Vec<SchemaResource>,
    pub root_uri: String,
    pub schema_set_digest: Digest256,
    pub format_profile: FormatProfile,
    pub instance_sha256: Digest256,
}

impl RecordFamily {
    /// Shadow-only constructor. Its inline schema probe has no CPU deadline
    /// and must not be used to complete an audit rule.
    pub fn new<'a>(
        registry_raw: &[u8],
        contract_raw: &[u8],
        schemas: impl IntoIterator<Item = RecordSchema<'a>>,
        format_profile: FormatProfile,
    ) -> Result<Self, RecordRuleError> {
        Self::new_inner(
            registry_raw,
            contract_raw,
            schemas,
            format_profile,
            RecordFactBudget::laboratory_default(),
            None,
        )
    }

    /// Audit-consumable constructor only when the caller has checked that the
    /// exact registry schema verdict came from a deadline-bounded worker.
    pub(crate) fn new_with_bounded_registry<'a>(
        registry_raw: &[u8],
        contract_raw: &[u8],
        schemas: impl IntoIterator<Item = RecordSchema<'a>>,
        format_profile: FormatProfile,
        fact_budget: RecordFactBudget,
        registry_evidence: &BoundedSchemaVerdict,
    ) -> Result<Self, RecordRuleError> {
        Self::new_inner(
            registry_raw,
            contract_raw,
            schemas,
            format_profile,
            fact_budget,
            Some(registry_evidence),
        )
    }

    fn new_inner<'a>(
        registry_raw: &[u8],
        contract_raw: &[u8],
        schemas: impl IntoIterator<Item = RecordSchema<'a>>,
        format_profile: FormatProfile,
        fact_budget: RecordFactBudget,
        registry_evidence: Option<&BoundedSchemaVerdict>,
    ) -> Result<Self, RecordRuleError> {
        if fact_budget.max_facts == 0 || fact_budget.max_encoded_bytes == 0 {
            return Err(RecordRuleError::Budget {
                code: "record_fact_budget_zero",
            });
        }
        let registry = parse_object(registry_raw, MAX_RECORD_BYTES, "registry_json")?;
        let contract = parse_object(contract_raw, MAX_RECORD_BYTES, "registry_contract_json")?;
        let contract_uri = schema_uri(ENTITY_CONTRACT, &contract)?;
        let contract_probe = SchemaBackendProbe::new(
            [SchemaResource {
                uri: contract_uri.clone(),
                raw: contract_raw.to_vec(),
            }],
            format_profile,
        )
        .map_err(|error| schema_error(error, ENTITY_CONTRACT))?;
        let registry_valid = match registry_evidence {
            Some(evidence) => check_verdict(
                evidence,
                registry_raw,
                &contract_probe,
                &contract_uri,
                format_profile,
            )?,
            None => contract_probe
                .is_valid_raw(&contract_uri, registry_raw)
                .map_err(|error| schema_error(error, ENTITY_REGISTRY))?,
        };
        if !registry_valid {
            return Err(unsupported("registry_contract", ENTITY_REGISTRY));
        }
        let registry_version = positive_integer(&registry, "registry_version")
            .ok_or_else(|| unsupported("registry_version", ENTITY_REGISTRY))?
            .to_string();
        let registry_sha256 = Digest256::of_bytes(registry_raw).to_hex();
        let mut resources = BTreeMap::new();
        resources.insert(
            ENTITY_CONTRACT.to_owned(),
            (contract_uri, contract_raw.to_vec()),
        );
        let mut total_resource_bytes = contract_raw.len();
        for resource in schemas {
            total_resource_bytes = total_resource_bytes.checked_add(resource.raw.len()).ok_or(
                RecordRuleError::Budget {
                    code: "schema_total_bytes",
                },
            )?;
            if resources.len() >= MAX_SOURCE_RESOURCES
                || total_resource_bytes > MAX_SOURCE_RESOURCE_BYTES
            {
                return Err(RecordRuleError::Budget {
                    code: "schema_resource_budget",
                });
            }
            if !is_contract_path(resource.path) || resources.contains_key(resource.path) {
                return Err(unsupported("schema_path_or_duplicate", resource.path));
            }
            let parsed = parse_object(
                resource.raw,
                SchemaBackendProbe::MAX_RESOURCE_BYTES,
                "schema_json",
            )?;
            let uri = schema_uri(resource.path, &parsed)?;
            resources.insert(resource.path.to_owned(), (uri, resource.raw.to_vec()));
        }
        if !resources.contains_key(CORPUS_CONTRACT) {
            return Err(unsupported("missing_corpus_schema", CORPUS_CONTRACT));
        }
        let profiles = compile_profiles(&registry)?;
        Ok(Self {
            registry_sha256,
            registry_version,
            resources,
            profiles,
            compiled_routes: BTreeMap::new(),
            format_profile,
            fact_budget,
            inspected_members: 0,
            issue_count: 0,
            seen_profiles: BTreeSet::new(),
            native_packet_count: 0,
            native_packet_bytes: 0,
            fact_count: 0,
            fact_bytes: 0,
        })
    }

    pub fn registry_digest(&self) -> &str {
        &self.registry_sha256
    }

    /// Reuse this family's exact declared/native routing for current endpoint
    /// lookup. Other JSON members and native semantic packet history do not
    /// become current standalone owners merely because they contain an ID.
    pub fn classify_current_member(
        &self,
        path: &str,
        raw: &[u8],
    ) -> Result<Option<CurrentRecordCarrier>, RecordRuleError> {
        if !path.starts_with(SOURCE_HOME) || !path.ends_with(".json") {
            return Ok(None);
        }
        let basename = path.rsplit('/').next().unwrap_or("");
        if let Some(profile) = self
            .profiles
            .values()
            .find(|profile| profile.basename == basename)
        {
            if !valid_record_path(path, profile) {
                return Err(unsupported("record_path", path));
            }
            let value = parse_object(raw, MAX_RECORD_BYTES, "record_json")?;
            let version = field_str(&value, "schema_version", "record_schema_version")?;
            if !profile.routes.contains_key(version) {
                return Err(unsupported("unsupported_record_schema_version", path));
            }
            if value["record_type"].as_str() != Some(profile.kind.as_str()) {
                return Err(unsupported("record_type", path));
            }
            let id = field_str(&value, "record_id", "record_id")?;
            if !valid_record_id(id, &profile.id_prefix) {
                return Err(unsupported("record_id", path));
            }
            return Ok(Some(CurrentRecordCarrier {
                id: id.into(),
                kind: profile.kind.clone(),
            }));
        }
        // Native baseline traversal selects known basenames throughout the
        // source home. Its decoded-field loader is legacy ordinary JSON.
        if ![
            "agent.json",
            "place.json",
            "organization.json",
            "work.json",
            "expression.json",
            "edition.json",
            "collection.json",
            "item.json",
            "link.json",
            "artifact-witness.json",
            "composite-witness.json",
        ]
        .contains(&basename)
        {
            return Ok(None);
        }
        if raw.len() > MAX_RECORD_BYTES {
            return Err(RecordRuleError::Budget {
                code: "native_record_byte_budget",
            });
        }
        let value: Value =
            serde_json::from_slice(raw).map_err(|_| unsupported("native_record_json", path))?;
        let carrier = native_carrier(path, &value)?;
        if value["schema_version"].as_str() != Some(carrier.schema_version) {
            return Err(unsupported("native_schema_version", path));
        }
        let id = field_str(&value, carrier.id_field, "native_record_id")?;
        if !valid_record_id(id, &format!("tos.{}.", carrier.kind)) {
            return Err(unsupported("native_record_id", path));
        }
        if carrier.id_field == "record_id" && value["record_type"].as_str() != Some(carrier.kind) {
            return Err(unsupported("native_record_type", path));
        }
        Ok(Some(CurrentRecordCarrier {
            id: id.into(),
            kind: carrier.kind.into(),
        }))
    }

    pub fn registry_version(&self) -> &str {
        &self.registry_version
    }

    pub fn emit_registry_read(&self, sink: &mut impl RecordSink) -> Result<(), RecordRuleError> {
        emit(
            sink,
            RecordObservation::Registry {
                path: ENTITY_REGISTRY.to_owned(),
                version: self.registry_version.clone(),
                raw_sha256: self.registry_sha256.clone(),
            },
        )
    }

    /// The registry worker request is independent of profile compilation.
    pub(crate) fn registry_schema_plan(
        contract_raw: &[u8],
        registry_raw: &[u8],
        format_profile: FormatProfile,
    ) -> Result<(Vec<SchemaResource>, String, Digest256, Digest256), RecordRuleError> {
        let contract = parse_object(contract_raw, MAX_RECORD_BYTES, "registry_contract_json")?;
        let uri = schema_uri(ENTITY_CONTRACT, &contract)?;
        let resources = vec![SchemaResource {
            uri: uri.clone(),
            raw: contract_raw.to_vec(),
        }];
        let probe = SchemaBackendProbe::new(resources.clone(), format_profile)
            .map_err(|error| schema_error(error, ENTITY_CONTRACT))?;
        Ok((
            resources,
            uri,
            probe.schema_set_digest(),
            Digest256::of_bytes(registry_raw),
        ))
    }

    /// Prepares the exact route and common-schema resources for a worker. A
    /// malformed candidate has no plan and is still fed to `inspect_member`.
    pub(crate) fn member_schema_plan(
        &self,
        path: &str,
        raw: &[u8],
    ) -> Result<BoundedMemberSchemaPlan, RecordRuleError> {
        let basename = path.rsplit('/').next().unwrap_or("");
        let profile = self
            .profiles
            .values()
            .find(|profile| profile.basename == basename)
            .ok_or_else(|| unsupported("unrecognized_record_basename", path))?;
        if !valid_record_path(path, profile) {
            return Err(unsupported("record_path", path));
        }
        let record = parse_object(raw, MAX_RECORD_BYTES, "record_json")?;
        let schema_version = field_str(&record, "schema_version", "record_schema_version")?;
        let route = profile
            .routes
            .get(schema_version)
            .ok_or_else(|| unsupported("unsupported_record_schema_version", path))?;
        let resources = self.route_resources(route)?;
        let probe = SchemaBackendProbe::new(resources.clone(), self.format_profile)
            .map_err(|error| schema_error(error, &route.schema_ref))?;
        Ok(BoundedMemberSchemaPlan {
            route_uri: self.resources[&route.schema_ref].0.clone(),
            common_uri: COMMON_URI.to_owned(),
            schema_set_digest: probe.schema_set_digest(),
            format_profile: self.format_profile,
            instance_sha256: Digest256::of_bytes(raw),
            resources,
        })
    }

    /// Select one source-owned native adapter by exact metadata path and
    /// schema version. The caller uses these bytes in the bounded worker.
    pub(crate) fn native_schema_plan(
        &self,
        path: &str,
        raw: &[u8],
    ) -> Result<NativeSchemaPlan, RecordRuleError> {
        let record = parse_object(raw, MAX_RECORD_BYTES, "native_record_json")?;
        let carrier = native_carrier(path, &record)?;
        let (uri, schema_raw) = self
            .resources
            .get(carrier.schema_path)
            .ok_or_else(|| unsupported("missing_native_schema", carrier.schema_path))?;
        let resources = vec![SchemaResource {
            uri: uri.clone(),
            raw: schema_raw.clone(),
        }];
        let probe = SchemaBackendProbe::new(resources.clone(), self.format_profile)
            .map_err(|error| schema_error(error, carrier.schema_path))?;
        Ok(NativeSchemaPlan {
            resources,
            root_uri: uri.clone(),
            schema_set_digest: probe.schema_set_digest(),
            format_profile: self.format_profile,
            instance_sha256: Digest256::of_bytes(raw),
        })
    }

    /// Native ID carriers remain distinct from registry-declared standalone
    /// records. Packet bodies stay inside the auditor's protected sink.
    pub(crate) fn inspect_native_with_bounded_schema(
        &mut self,
        path: &str,
        raw: &[u8],
        evidence: &BoundedSchemaVerdict,
        sink: &mut impl RecordSink,
    ) -> Result<(), RecordRuleError> {
        let mut counted =
            AccountingSink::new(sink, self.fact_budget, self.fact_count, self.fact_bytes);
        let result = self.inspect_native_inner(path, raw, evidence, &mut counted);
        self.fact_count = counted.count;
        self.fact_bytes = counted.bytes;
        if counted.budget_failed {
            Err(RecordRuleError::Budget {
                code: "record_fact_budget",
            })
        } else {
            result
        }
    }

    fn inspect_native_inner(
        &mut self,
        path: &str,
        raw: &[u8],
        evidence: &BoundedSchemaVerdict,
        sink: &mut impl RecordSink,
    ) -> Result<(), RecordRuleError> {
        emit(
            sink,
            RecordObservation::ExactPath {
                path: path.to_owned(),
                raw_sha256: Digest256::of_bytes(raw).to_hex(),
            },
        )?;
        self.inspected_members += 1;
        if raw.len() > MAX_RECORD_BYTES {
            return self.issue(sink, path, "native_record_byte_budget");
        }
        let record = match parse_object(raw, MAX_RECORD_BYTES, "native_record_json") {
            Ok(record) => record,
            Err(_) => return self.issue(sink, path, "native_record_json"),
        };
        let carrier = native_carrier(path, &record)?;
        if carrier.semantic_packet {
            self.native_packet_count += 1;
            self.native_packet_bytes =
                self.native_packet_bytes
                    .checked_add(raw.len())
                    .ok_or(RecordRuleError::Budget {
                        code: "native_identity_byte_budget",
                    })?;
            if self.native_packet_count > 1024 || self.native_packet_bytes > 8_388_608 {
                return Err(RecordRuleError::Budget {
                    code: "native_identity_inventory_budget",
                });
            }
        }
        let (uri, schema_raw) = self
            .resources
            .get(carrier.schema_path)
            .ok_or_else(|| unsupported("missing_native_schema", carrier.schema_path))?;
        let probe = SchemaBackendProbe::new(
            [SchemaResource {
                uri: uri.clone(),
                raw: schema_raw.clone(),
            }],
            self.format_profile,
        )
        .map_err(|error| schema_error(error, carrier.schema_path))?;
        emit(
            sink,
            RecordObservation::Schema {
                path: carrier.schema_path.to_owned(),
                uri: uri.clone(),
                raw_sha256: Digest256::of_bytes(schema_raw).to_hex(),
            },
        )?;
        if !check_verdict(evidence, raw, &probe, uri, self.format_profile)? {
            self.issue(sink, path, "native_record_schema")?;
        }
        if record.get("schema_version").and_then(Value::as_str) != Some(carrier.schema_version) {
            return self.issue(sink, path, "native_schema_version");
        }
        if carrier.semantic_packet {
            let Some(entities) = record.get("entities").and_then(Value::as_array) else {
                return self.issue(sink, path, "native_entities");
            };
            for entity in entities {
                let Some(id) = entity.get("entity_id").and_then(Value::as_str) else {
                    self.issue(sink, path, "native_entity_id")?;
                    continue;
                };
                if !["occurrence", "lexeme", "sense", "sign", "concept"]
                    .iter()
                    .any(|kind| valid_record_id(id, &format!("tos.{kind}.")))
                {
                    self.issue(sink, path, "native_entity_id")?;
                }
                emit(
                    sink,
                    RecordObservation::NativeReservation {
                        id: id.to_owned(),
                        packet_path: path.to_owned(),
                        raw_sha256: Digest256::of_bytes(raw).to_hex(),
                    },
                )?;
            }
            return Ok(());
        }
        self.emit_foundation_refs(&record, path, sink)?;
        if matches!(
            carrier.kind,
            "agent"
                | "place"
                | "organization"
                | "work"
                | "expression"
                | "edition"
                | "collection"
                | "item"
                | "link"
        ) && record.get("record_type").and_then(Value::as_str) != Some(carrier.kind)
        {
            self.issue(sink, path, "native_record_type")?;
        }
        let Some(id) = record.get(carrier.id_field).and_then(Value::as_str) else {
            return self.issue(sink, path, "native_record_id");
        };
        if !valid_record_id(id, &format!("tos.{}.", carrier.kind)) {
            self.issue(sink, path, "native_record_id")?;
        }
        let Some(version) = positive_integer(&record, "record_version") else {
            return self.issue(sink, path, "native_record_version");
        };
        emit(
            sink,
            RecordObservation::IdOwner {
                id: id.to_owned(),
                kind: carrier.kind.to_owned(),
                path: path.to_owned(),
                version,
                raw_sha256: Digest256::of_bytes(raw).to_hex(),
            },
        )?;
        emit(
            sink,
            RecordObservation::IdKindOwner {
                kind: carrier.kind.to_owned(),
                id: id.to_owned(),
                path: path.to_owned(),
            },
        )?;
        if carrier.kind == "link" {
            if let Some(uri) = record.get("uri").and_then(Value::as_str) {
                emit(
                    sink,
                    RecordObservation::LinkUriOwner {
                        uri: uri.to_owned(),
                        id: id.to_owned(),
                        path: path.to_owned(),
                    },
                )?;
            }
            if let Some(target_path) = record.get("observation_ref").and_then(Value::as_str) {
                emit(
                    sink,
                    RecordObservation::Reference {
                        from_path: path.to_owned(),
                        target_path: target_path.to_owned(),
                        check: PathReferenceCheck::FileIfToS,
                    },
                )?;
            }
        }
        if carrier.kind == "expression" {
            self.emit_typed_ref(&record, path, "work_ref", "work", sink)?;
        } else if carrier.kind == "edition" {
            self.emit_typed_refs(
                &record,
                path,
                "embodies_expression_refs",
                "expression",
                sink,
            )?;
            self.emit_typed_ref(&record, path, "collection_ref", "collection", sink)?;
        }
        Ok(())
    }

    /// Shadow-only inline schema probe. It has no CPU deadline and cannot
    /// complete an audit rule. It still emits exact per-member observations.
    pub fn inspect_member(
        &mut self,
        path: &str,
        raw: &[u8],
        sink: &mut impl RecordSink,
    ) -> Result<(), RecordRuleError> {
        self.inspect_member_accounted(path, raw, sink, None)
    }

    /// Consumes two exact verdicts from the auditor's deadline-bounded worker.
    /// The caller must account for both responses and verify worker provenance.
    pub(crate) fn inspect_member_with_bounded_schema(
        &mut self,
        path: &str,
        raw: &[u8],
        evidence: &BoundedMemberSchemaEvidence,
        sink: &mut impl RecordSink,
    ) -> Result<(), RecordRuleError> {
        self.inspect_member_accounted(path, raw, sink, Some(evidence))
    }

    fn inspect_member_accounted(
        &mut self,
        path: &str,
        raw: &[u8],
        sink: &mut impl RecordSink,
        evidence: Option<&BoundedMemberSchemaEvidence>,
    ) -> Result<(), RecordRuleError> {
        let mut counted =
            AccountingSink::new(sink, self.fact_budget, self.fact_count, self.fact_bytes);
        let result = self.inspect_member_inner(path, raw, &mut counted, evidence);
        self.fact_count = counted.count;
        self.fact_bytes = counted.bytes;
        if counted.budget_failed {
            Err(RecordRuleError::Budget {
                code: "record_fact_budget",
            })
        } else {
            result
        }
    }

    fn inspect_member_inner(
        &mut self,
        path: &str,
        raw: &[u8],
        sink: &mut impl RecordSink,
        evidence: Option<&BoundedMemberSchemaEvidence>,
    ) -> Result<(), RecordRuleError> {
        emit(
            sink,
            RecordObservation::ExactPath {
                path: path.to_owned(),
                raw_sha256: Digest256::of_bytes(raw).to_hex(),
            },
        )?;
        self.inspected_members += 1;
        if raw.len() > MAX_RECORD_BYTES {
            return self.issue(sink, path, "record_byte_budget");
        }
        let basename = path.rsplit('/').next().unwrap_or("");
        let Some(kind) = self
            .profiles
            .values()
            .find(|profile| profile.basename == basename)
            .map(|profile| profile.kind.clone())
        else {
            return Err(unsupported("unrecognized_record_basename", path));
        };
        let profile = self.profiles[&kind].clone();
        if !valid_record_path(path, &profile) {
            return self.issue(sink, path, "record_path");
        }
        let record = match parse_object(raw, MAX_RECORD_BYTES, "record_json") {
            Ok(value) => value,
            Err(RecordRuleError::Budget { .. }) => {
                return self.issue(sink, path, "record_byte_budget");
            }
            Err(_) => return self.issue(sink, path, "record_json"),
        };
        let Some(schema_version) = record.get("schema_version").and_then(Value::as_str) else {
            return self.issue(sink, path, "record_schema_version");
        };
        if !profile.routes.contains_key(schema_version) {
            return Err(unsupported("unsupported_record_schema_version", path));
        }
        self.seen_profiles.insert(kind.clone());
        emit(
            sink,
            RecordObservation::Profile {
                path: path.to_owned(),
                kind: kind.clone(),
                profile_version: profile.version,
                schema_version: schema_version.to_owned(),
            },
        )?;
        let route_key = (kind.clone(), schema_version.to_owned());
        if !self.compiled_routes.contains_key(&route_key) {
            if self.compiled_routes.len() >= MAX_COMPILED_ROUTES {
                return Err(RecordRuleError::Budget {
                    code: "compiled_route_budget",
                });
            }
            let probe = self.compile_route(&profile, schema_version, sink)?;
            self.compiled_routes.insert(route_key.clone(), probe);
        }
        let probe = &self.compiled_routes[&route_key];
        let route = &profile.routes[schema_version];
        let schema_uri = &self.resources[&route.schema_ref].0;
        let (schema_ok, common_ok) = match evidence {
            Some(evidence) => {
                if evidence.route.worker_protocol_id != evidence.common.worker_protocol_id
                    || evidence.route.worker_binary_digest != evidence.common.worker_binary_digest
                {
                    return Err(unsupported("schema_worker_mismatch", path));
                }
                (
                    check_verdict(&evidence.route, raw, probe, schema_uri, self.format_profile)?,
                    check_verdict(
                        &evidence.common,
                        raw,
                        probe,
                        COMMON_URI,
                        self.format_profile,
                    )?,
                )
            }
            None => (
                probe
                    .is_valid_raw(schema_uri, raw)
                    .map_err(|error| schema_error(error, path))?,
                probe
                    .is_valid_raw(COMMON_URI, raw)
                    .map_err(|error| schema_error(error, path))?,
            ),
        };
        if !schema_ok || !common_ok {
            self.issue(sink, path, "record_schema")?;
        }
        let Some(record_type) = record.get("record_type").and_then(Value::as_str) else {
            return self.issue(sink, path, "record_type");
        };
        if record_type != kind
            || !matches!(
                record.get("visibility").and_then(Value::as_str),
                Some("public" | "public_metadata_only")
            )
        {
            self.issue(sink, path, "record_profile_or_visibility")?;
        }
        let Some(id) = record.get("record_id").and_then(Value::as_str) else {
            return self.issue(sink, path, "record_id");
        };
        if !valid_record_id(id, &profile.id_prefix)
            || !matches!(
                record.get("identity_status").and_then(Value::as_str),
                Some("provisional" | "verified" | "disputed" | "superseded")
            )
            || !record
                .get("preferred_label")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
        {
            self.issue(sink, path, "record_identity_metadata")?;
        }
        let Some(version) = positive_integer(&record, "record_version") else {
            return self.issue(sink, path, "record_version");
        };
        emit(
            sink,
            RecordObservation::IdOwner {
                id: id.to_owned(),
                kind: kind.clone(),
                path: path.to_owned(),
                version,
                raw_sha256: Digest256::of_bytes(raw).to_hex(),
            },
        )?;
        emit(
            sink,
            RecordObservation::IdKindOwner {
                kind: kind.clone(),
                id: id.to_owned(),
                path: path.to_owned(),
            },
        )?;
        self.emit_foundation_refs(&record, path, sink)?;
        Ok(())
    }

    fn emit_foundation_refs(
        &mut self,
        record: &Value,
        path: &str,
        sink: &mut impl RecordSink,
    ) -> Result<(), RecordRuleError> {
        for field in ["source_refs", "source_record_refs", "receipt_refs"] {
            if let Some(values) = record.get(field).and_then(Value::as_array) {
                for value in values {
                    self.emit_path_ref(value, path, PathReferenceCheck::RepoExistsIfToS, sink)?;
                }
            }
        }
        for field in [
            "rights_ref",
            "provenance_ref",
            "forensic_report_ref",
            "resource_inventory_ref",
            "generated_from_manifest_ref",
            "item_manifest_ref",
        ] {
            if let Some(value) = record.get(field) {
                self.emit_path_ref(value, path, PathReferenceCheck::RepoExistsIfToS, sink)?;
            }
        }
        Ok(())
    }

    fn emit_path_ref(
        &mut self,
        value: &Value,
        path: &str,
        check: PathReferenceCheck,
        sink: &mut impl RecordSink,
    ) -> Result<(), RecordRuleError> {
        if let Some(target_path) = value.as_str() {
            emit(
                sink,
                RecordObservation::Reference {
                    from_path: path.to_owned(),
                    target_path: target_path.to_owned(),
                    check,
                },
            )
        } else {
            self.issue(sink, path, "reference_type")
        }
    }

    fn emit_typed_ref(
        &mut self,
        record: &Value,
        path: &str,
        field: &str,
        expected_kind: &'static str,
        sink: &mut impl RecordSink,
    ) -> Result<(), RecordRuleError> {
        if let Some(value) = record.get(field) {
            if let Some(target_id) = value.as_str() {
                emit(
                    sink,
                    RecordObservation::RecordIdReference {
                        from_path: path.to_owned(),
                        target_id: target_id.to_owned(),
                        expected_kind,
                    },
                )?;
            } else {
                self.issue(sink, path, "record_id_reference_type")?;
            }
        }
        Ok(())
    }

    fn emit_typed_refs(
        &mut self,
        record: &Value,
        path: &str,
        field: &str,
        expected_kind: &'static str,
        sink: &mut impl RecordSink,
    ) -> Result<(), RecordRuleError> {
        if let Some(values) = record.get(field).and_then(Value::as_array) {
            for value in values {
                if let Some(target_id) = value.as_str() {
                    emit(
                        sink,
                        RecordObservation::RecordIdReference {
                            from_path: path.to_owned(),
                            target_id: target_id.to_owned(),
                            expected_kind,
                        },
                    )?;
                } else {
                    self.issue(sink, path, "record_id_reference_type")?;
                }
            }
        }
        Ok(())
    }

    fn compile_route(
        &self,
        profile: &Profile,
        schema_version: &str,
        sink: &mut impl RecordSink,
    ) -> Result<SchemaBackendProbe, RecordRuleError> {
        let route = &profile.routes[schema_version];
        let resources = self.route_resources(route)?;
        for (path, (uri, raw)) in self.resources.iter().filter(|(path, _)| {
            path.as_str() == CORPUS_CONTRACT
                || path.as_str() == route.schema_ref
                || route.schema_dependencies.contains(path)
        }) {
            emit(
                sink,
                RecordObservation::Schema {
                    path: path.clone(),
                    uri: uri.clone(),
                    raw_sha256: Digest256::of_bytes(raw).to_hex(),
                },
            )?;
        }
        SchemaBackendProbe::new(resources, self.format_profile)
            .map_err(|error| schema_error(error, &route.schema_ref))
    }

    fn route_resources(&self, route: &Route) -> Result<Vec<SchemaResource>, RecordRuleError> {
        let mut refs = BTreeSet::from([CORPUS_CONTRACT.to_owned(), route.schema_ref.clone()]);
        refs.extend(route.schema_dependencies.iter().cloned());
        let corpus_uri = &self
            .resources
            .get(CORPUS_CONTRACT)
            .ok_or_else(|| unsupported("missing_corpus_schema", CORPUS_CONTRACT))?
            .0;
        let mut resources = Vec::new();
        for path in refs {
            let (uri, raw) = self
                .resources
                .get(&path)
                .ok_or_else(|| unsupported("undeclared_schema_resource", &path))?;
            resources.push(SchemaResource {
                uri: uri.clone(),
                raw: raw.clone(),
            });
        }
        resources.push(SchemaResource {
            uri: COMMON_URI.to_owned(),
            raw: common_schema(corpus_uri),
        });
        Ok(resources)
    }

    fn issue(
        &mut self,
        sink: &mut impl RecordSink,
        path: &str,
        code: &'static str,
    ) -> Result<(), RecordRuleError> {
        self.issue_count += 1;
        emit(
            sink,
            RecordObservation::Issue {
                path: path.to_owned(),
                code,
            },
        )
    }

    /// The report is family-local and deliberately cannot claim admission.
    pub fn finish(self) -> RecordFamilyReport {
        let enumerated_profile_ids = self
            .profiles
            .values()
            .map(|profile| format!("{}@{}", profile.kind, profile.version))
            .collect();
        let skipped_profile_ids = self
            .profiles
            .values()
            .filter(|profile| !self.seen_profiles.contains(&profile.kind))
            .map(|profile| format!("{}@{}", profile.kind, profile.version))
            .collect();
        RecordFamilyReport {
            registry_version: self.registry_version,
            registry_sha256: self.registry_sha256,
            inspected_members: self.inspected_members,
            issue_count: self.issue_count,
            emitted_member_fact_count: self.fact_count,
            emitted_member_fact_bytes: self.fact_bytes,
            enumerated_profile_ids,
            skipped_profile_ids,
            incomplete: true,
        }
    }
}

fn emit(sink: &mut impl RecordSink, event: RecordObservation) -> Result<(), RecordRuleError> {
    sink.push(event)
        .map_err(|detail| RecordRuleError::Sink { detail })
}

fn check_verdict(
    verdict: &BoundedSchemaVerdict,
    raw: &[u8],
    probe: &SchemaBackendProbe,
    root_uri: &str,
    format_profile: FormatProfile,
) -> Result<bool, RecordRuleError> {
    if verdict.instance_sha256 != Digest256::of_bytes(raw)
        || verdict.schema_set_digest != probe.schema_set_digest()
        || verdict.format_profile != format_profile
        || verdict.root_uri != root_uri
        || verdict.worker_protocol_id.is_empty()
    {
        return Err(unsupported("bounded_schema_evidence_mismatch", root_uri));
    }
    Ok(verdict.valid)
}

fn parse_object(raw: &[u8], max: usize, code: &'static str) -> Result<Value, RecordRuleError> {
    let value = published_value(raw, max).map_err(|error| match error {
        SchemaProbeError::BudgetExceeded => RecordRuleError::Budget { code },
        _ => unsupported(code, "strict published JSON"),
    })?;
    if !value.is_object() {
        return Err(unsupported(code, "JSON root is not an object"));
    }
    Ok(value)
}

fn schema_uri(path: &str, schema: &Value) -> Result<String, RecordRuleError> {
    let uri = schema
        .get("$id")
        .and_then(Value::as_str)
        .ok_or_else(|| unsupported("schema_id", path))?;
    // Preserve the exact authored resource identity. Contract paths locate
    // pinned bytes; maintained schema IDs also use public/legacy URI homes.
    // The existing selected backend owns URI resolution and schema validity.
    // Retain its HTTPS, fragment-free resource profile without inventing a
    // filesystem-path-to-ID convention or a URI host allowlist here.
    if !uri.starts_with("https://") || uri.contains('#') {
        return Err(unsupported("schema_id", path));
    }
    Ok(uri.to_owned())
}

fn is_contract_path(path: &str) -> bool {
    let Some(name) = path.strip_prefix("ToS/contracts/") else {
        return false;
    };
    name.ends_with(".schema.json")
        && !name.contains('/')
        && !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
        })
}

fn unsupported(code: &'static str, detail: &str) -> RecordRuleError {
    RecordRuleError::Unsupported {
        code,
        detail: detail.to_owned(),
    }
}

fn schema_error(error: SchemaProbeError, path: &str) -> RecordRuleError {
    match error {
        SchemaProbeError::BudgetExceeded => RecordRuleError::Budget {
            code: "schema_budget",
        },
        _ => unsupported("schema_backend_or_resource", path),
    }
}

fn positive_integer(object: &Value, field: &str) -> Option<u64> {
    object.get(field)?.as_u64().filter(|value| *value > 0)
}

fn source_kind(kind: &str) -> bool {
    let mut parts = kind.split('-');
    parts.next().is_some_and(valid_kind_segment)
        && parts.all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn valid_kind_segment(part: &str) -> bool {
    let mut bytes = part.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn valid_record_id(id: &str, prefix: &str) -> bool {
    let Some(suffix) = id.strip_prefix(prefix) else {
        return false;
    };
    !suffix.is_empty()
        && suffix.split(['.', '-']).all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn valid_record_path(path: &str, profile: &Profile) -> bool {
    if !path.starts_with(SOURCE_HOME) || path.starts_with('/') {
        return false;
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.iter().any(|part| {
        part.is_empty()
            || *part == "."
            || *part == ".."
            || matches!(
                *part,
                "catalog" | "payload" | "local-content" | "owner-local"
            )
    }) || parts.last().copied() != Some(profile.basename.as_str())
    {
        return false;
    }
    if profile.kind == "composite" {
        return parts.len() >= 7 && path.starts_with("ToS/source-witnesses/scholarly-composites/");
    }
    true
}

fn native_carrier(path: &str, record: &Value) -> Result<NativeCarrier, RecordRuleError> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 3
        || parts[0..2] != ["ToS", "source-witnesses"]
        || parts.iter().any(|part| {
            part.is_empty()
                || *part == "."
                || *part == ".."
                || matches!(
                    *part,
                    "catalog" | "payload" | "local-content" | "owner-local"
                )
        })
    {
        return Err(unsupported("native_owner_path", path));
    }
    let basename = parts.last().copied().unwrap_or("");
    if basename.starts_with("semantic-annotation") && basename.ends_with(".json") {
        return Ok(NativeCarrier {
            kind: "semantic-packet",
            schema_path: "ToS/contracts/semantic-annotation-packet-v2.schema.json",
            schema_version: "tos_semantic_annotation_packet_v2",
            id_field: "",
            semantic_packet: true,
        });
    }
    if parts.len() < 4 {
        return Err(unsupported("native_owner_path", path));
    }
    let carrier = match (parts[2], basename) {
        (_, "agent.json") => (
            "agent",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        (_, "place.json") => (
            "place",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        (_, "organization.json") => (
            "organization",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        (_, "work.json") => ("work", CORPUS_CONTRACT, "tos_corpus_record_v1", "record_id"),
        (_, "expression.json") => (
            "expression",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        (_, "edition.json") => (
            "edition",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        (_, "collection.json") => (
            "collection",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        (_, "item.json") => ("item", CORPUS_CONTRACT, "tos_corpus_record_v1", "record_id"),
        ("links", "link.json") => (
            "link",
            "ToS/contracts/source-link.schema.json",
            "tos_source_link_v1",
            "record_id",
        ),
        ("artifacts", "artifact-witness.json") => {
            match record.get("schema_version").and_then(Value::as_str) {
                Some("tos_artifact_source_witness_v1") => (
                    "artifact",
                    "ToS/contracts/artifact-source-witness.schema.json",
                    "tos_artifact_source_witness_v1",
                    "artifact_id",
                ),
                Some("tos_artifact_source_witness_v2") => (
                    "artifact",
                    "ToS/contracts/artifact-source-witness-v2.schema.json",
                    "tos_artifact_source_witness_v2",
                    "artifact_id",
                ),
                _ => return Err(unsupported("native_artifact_schema_version", path)),
            }
        }
        ("scholarly-composites", "composite-witness.json") => (
            "composite",
            "ToS/contracts/scholarly-composite-witness.schema.json",
            "tos_scholarly_composite_witness_v1",
            "composite_id",
        ),
        _ => return Err(unsupported("unrecognized_native_carrier", path)),
    };
    Ok(NativeCarrier {
        kind: carrier.0,
        schema_path: carrier.1,
        schema_version: carrier.2,
        id_field: carrier.3,
        semantic_packet: false,
    })
}

fn common_schema(corpus_uri: &str) -> Vec<u8> {
    let fields = [
        "preferred_label",
        "variant_labels",
        "field_languages",
        "identity_status",
        "source_refs",
        "external_identifiers",
        "same_as_posture",
        "record_version",
        "notes",
    ];
    let properties: serde_json::Map<String, Value> = fields
        .into_iter()
        .map(|field| {
            (
                field.to_owned(),
                json!({"$ref": format!("{corpus_uri}#/properties/{field}")}),
            )
        })
        .collect();
    serde_json::to_vec(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": COMMON_URI,
        "type": "object",
        "required": ["preferred_label", "identity_status", "source_refs", "external_identifiers", "same_as_posture", "record_version"],
        "properties": properties
    })).expect("static common schema serializes")
}

fn compile_entities(registry: &Value) -> Result<BTreeMap<&str, Entity<'_>>, RecordRuleError> {
    let entries = registry
        .get("types")
        .and_then(Value::as_array)
        .ok_or_else(|| unsupported("registry_types", ENTITY_REGISTRY))?;
    let mut entities = BTreeMap::new();
    for entry in entries {
        let id = field_str(entry, "type_id", "type_identity")?;
        let parents = field_array_refs(entry, "parent_type_ids", "type_parent")?;
        let role = field_str(entry, "object_role", "type_role")?;
        let abstract_type = entry
            .get("abstract")
            .and_then(Value::as_bool)
            .ok_or_else(|| unsupported("type_abstract", &id))?;
        let mappings = entry
            .get("source_mappings")
            .and_then(Value::as_array)
            .ok_or_else(|| unsupported("type_mappings", &id))?
            .iter()
            .map(|mapping| {
                Ok((
                    field_str(mapping, "source_graph", "mapping_graph")?,
                    field_str(mapping, "source_kind_id", "mapping_kind")?,
                ))
            })
            .collect::<Result<Vec<_>, RecordRuleError>>()?;
        if entities
            .insert(
                id,
                Entity {
                    parents,
                    role,
                    abstract_type,
                    mappings,
                    profile: entry.get("source_record_profile"),
                },
            )
            .is_some()
        {
            return Err(unsupported("duplicate_type_id", &id));
        }
    }
    Ok(entities)
}

fn compile_profiles(registry: &Value) -> Result<BTreeMap<String, Profile>, RecordRuleError> {
    let entities = compile_entities(registry)?;
    let mut profiles = BTreeMap::new();
    let mut seen = BTreeMap::<(&str, String), String>::new();
    for (type_id, entity) in &entities {
        let Some(value) = &entity.profile else {
            continue;
        };
        let kind = field_str(value, "record_type", "profile_kind")?.to_owned();
        let reader = field_str(value, "reader", "profile_reader")?.to_owned();
        let version = positive_integer(value, "profile_version")
            .ok_or_else(|| unsupported("profile_version", &kind))?;
        let id_prefix = field_str(value, "id_prefix", "profile_prefix")?.to_owned();
        let basename = field_str(value, "source_basename", "profile_basename")?.to_owned();
        let catalog_filename = field_str(value, "catalog_filename", "profile_catalog")?.to_owned();
        if !source_kind(&kind)
            || id_prefix != format!("tos.{kind}.")
            || basename != format!("{kind}.json")
            || !matches!(
                reader.as_str(),
                "corpus-metadata-v1" | "semantic-metadata-v1"
            )
        {
            return Err(unsupported("profile_identity_or_reader", &kind));
        }
        let retained_composite = kind == "composite"
            && *type_id == "tos.entity.composite"
            && value.get("retained_native_adapter").and_then(Value::as_str)
                == Some("scholarly-composite-v1")
            && reader == "corpus-metadata-v1"
            && catalog_filename == "composites.jsonl";
        let reserved = [
            "agent",
            "place",
            "organization",
            "work",
            "expression",
            "edition",
            "collection",
            "item",
            "link",
            "artifact",
            "composite",
        ];
        if !retained_composite
            && (reserved.contains(&kind.as_str())
                || reserved.iter().any(|name| {
                    basename == format!("{name}.json")
                        || catalog_filename == format!("{name}s.jsonl")
                })
                || catalog_filename == "claims.jsonl")
        {
            return Err(unsupported("profile_reserved_collision", &kind));
        }
        if value.get("retained_native_adapter").is_some() && !retained_composite {
            return Err(unsupported("profile_native_adapter", &kind));
        }
        if value.get("native_binding_adapter").is_some()
            && !(reader == "semantic-metadata-v1"
                && value.get("native_binding_adapter").and_then(Value::as_str)
                    == Some("source-text-unit-v1"))
        {
            return Err(unsupported("profile_text_adapter", &kind));
        }
        let authored_sign =
            kind == "sign" && *type_id == "tos.entity.sign" && reader == "semantic-metadata-v1";
        if (value.get("creation_gate").is_some() && !authored_sign)
            || (authored_sign
                && value.get("creation_gate").and_then(Value::as_str) != Some("sign-promotion-v1"))
            || ((kind == "sign" || *type_id == "tos.entity.sign") && !authored_sign)
        {
            return Err(unsupported("profile_sign_gate", &kind));
        }
        if value.get("identity_proposal_adapter").is_some()
            && !(reader == "semantic-metadata-v1"
                && entity.role == "semantic"
                && value
                    .get("identity_proposal_adapter")
                    .and_then(Value::as_str)
                    == Some("exact-semantic-metadata-v1")
                && value.get("graph_layer").and_then(Value::as_str) == Some("source-profile")
                && !["claim", "literal", "temporal-assertion"].contains(&kind.as_str()))
        {
            return Err(unsupported("profile_identity_adapter", &kind));
        }
        let (role, ancestor) = if reader == "corpus-metadata-v1" {
            ("identity", "tos.entity.identity")
        } else {
            ("semantic", "tos.entity.semantic-object")
        };
        if entity.abstract_type
            || entity.role != role
            || *type_id == ancestor
            || !has_ancestor(&entities, type_id, ancestor)?
        {
            return Err(unsupported("profile_type_ancestry", &kind));
        }
        for graph in ["source-claims", "source-navigation"] {
            if entity
                .mappings
                .iter()
                .filter(|(g, k)| *g == graph && *k == kind)
                .count()
                != 1
                || entities.iter().any(|(other_id, other)| {
                    other_id != type_id
                        && other
                            .mappings
                            .iter()
                            .any(|(g, k)| *g == graph && *k == kind)
                })
            {
                return Err(unsupported("profile_mapping_owner", &kind));
            }
        }
        for (key, value) in [
            ("id_prefix", &id_prefix),
            ("source_basename", &basename),
            ("catalog_filename", &catalog_filename),
        ] {
            if seen.insert((key, value.clone()), kind.clone()).is_some() {
                return Err(unsupported("duplicate_profile_field", &kind));
            }
        }
        let mut routes = BTreeMap::new();
        for route in value
            .get("schemas")
            .and_then(Value::as_array)
            .ok_or_else(|| unsupported("profile_routes", &kind))?
        {
            let schema_version = field_str(route, "schema_version", "route_version")?.to_owned();
            let schema_ref = field_str(route, "schema_ref", "route_schema")?.to_owned();
            let dependencies = field_array_str(route, "schema_dependencies", "route_dependencies")?;
            if !is_contract_path(&schema_ref)
                || dependencies
                    .iter()
                    .any(|dependency| !is_contract_path(dependency))
                || routes
                    .insert(
                        schema_version.clone(),
                        Route {
                            schema_ref,
                            schema_dependencies: dependencies,
                        },
                    )
                    .is_some()
            {
                return Err(unsupported("profile_route_collision", &kind));
            }
        }
        if profiles
            .insert(
                kind.clone(),
                Profile {
                    kind,
                    version,
                    reader,
                    id_prefix,
                    basename,
                    catalog_filename,
                    routes,
                },
            )
            .is_some()
        {
            return Err(unsupported("duplicate_profile_kind", type_id));
        }
    }
    Ok(profiles)
}

fn has_ancestor<'a>(
    entities: &BTreeMap<&'a str, Entity<'a>>,
    start: &'a str,
    ancestor: &str,
) -> Result<bool, RecordRuleError> {
    let mut found = false;
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    let mut stack = vec![(start, false)];
    while let Some((id, leaving)) = stack.pop() {
        if leaving {
            active.remove(&id);
            done.insert(id);
            continue;
        }
        if active.contains(&id) {
            return Err(unsupported("type_cycle", &id));
        }
        if done.contains(&id) {
            continue;
        }
        let entity = entities
            .get(id)
            .ok_or_else(|| unsupported("unknown_parent_type", &id))?;
        if id == ancestor {
            found = true;
        }
        active.insert(id);
        stack.push((id, true));
        for parent in entity.parents.iter().rev() {
            stack.push((*parent, false));
        }
    }
    Ok(found)
}

fn field_str<'a>(
    value: &'a Value,
    field: &str,
    code: &'static str,
) -> Result<&'a str, RecordRuleError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| unsupported(code, field))
}

fn field_array_refs<'a>(
    value: &'a Value,
    field: &str,
    code: &'static str,
) -> Result<Vec<&'a str>, RecordRuleError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| unsupported(code, field))?
        .iter()
        .map(|item| item.as_str().ok_or_else(|| unsupported(code, field)))
        .collect()
}
fn field_array_str(
    value: &Value,
    field: &str,
    code: &'static str,
) -> Result<Vec<String>, RecordRuleError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| unsupported(code, field))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| unsupported(code, field))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[derive(Default)]
    struct Events(Vec<RecordObservation>);
    impl RecordSink for Events {
        fn push(&mut self, event: RecordObservation) -> Result<(), String> {
            self.0.push(event);
            Ok(())
        }
    }

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    fn source(path: &str) -> Vec<u8> {
        std::fs::read(root().join(path)).unwrap()
    }

    fn family() -> RecordFamily {
        let registry = source(ENTITY_REGISTRY);
        let contract = source(ENTITY_CONTRACT);
        let registry_value: Value = serde_json::from_slice(&registry).unwrap();
        let mut schema_paths = BTreeSet::from([
            CORPUS_CONTRACT.to_owned(),
            "ToS/contracts/source-link.schema.json".to_owned(),
            "ToS/contracts/artifact-source-witness-v2.schema.json".to_owned(),
        ]);
        for profile in registry_value["types"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|entry| {
                entry
                    .get("source_record_profile")
                    .and_then(|profile| profile.get("schemas"))
                    .and_then(Value::as_array)
            })
        {
            for route in profile {
                schema_paths.insert(route["schema_ref"].as_str().unwrap().to_owned());
                for dependency in route["schema_dependencies"].as_array().unwrap() {
                    schema_paths.insert(dependency.as_str().unwrap().to_owned());
                }
            }
        }
        let files: Vec<_> = schema_paths
            .into_iter()
            .map(|path| {
                let raw = source(&path);
                (path, raw)
            })
            .collect();
        RecordFamily::new(
            &registry,
            &contract,
            files.iter().map(|(path, raw)| RecordSchema { path, raw }),
            FormatProfile::LegacyPythonObserved20260923,
        )
        .unwrap()
    }

    const RECORD: &str =
        "ToS/source-witnesses/research-corpora/foundation-source-routes/research-corpus.json";

    #[test]
    fn actual_declared_record_emits_exact_inputs_and_id_owner_without_admission() {
        let mut rule = family();
        let mut events = Events::default();
        rule.emit_registry_read(&mut events).unwrap();
        rule.inspect_member(RECORD, &source(RECORD), &mut events)
            .unwrap();
        let report = rule.finish();
        assert!(report.incomplete);
        assert_eq!(report.issue_count, 0);
        assert_eq!(report.inspected_members, 1);
        assert!(
            report
                .enumerated_profile_ids
                .contains(&"research-corpus@1".to_owned())
        );
        assert!(events.0.iter().any(|event| matches!(event,
            RecordObservation::IdOwner { id, version: 2, .. } if id.starts_with("tos.research-corpus."))));
        assert!(events.0.iter().any(|event| matches!(event,
            RecordObservation::Reference { target_path, .. } if target_path.starts_with("ToS/review-ledger/"))));
    }

    #[test]
    fn wrong_path_and_strict_json_mutations_have_coded_issues() {
        let mut rule = family();
        let mut events = Events::default();
        rule.inspect_member(
            "ToS/source-witnesses/catalog/research-corpus.json",
            &source(RECORD),
            &mut events,
        )
        .unwrap();
        let duplicate = br#"{"record_id":"one","record_\u0069d":"two"}"#;
        rule.inspect_member(RECORD, duplicate, &mut events).unwrap();
        let report = rule.finish();
        assert_eq!(report.issue_count, 2);
        assert!(events.0.iter().any(|event| matches!(
            event,
            RecordObservation::Issue {
                code: "record_path",
                ..
            }
        )));
        assert!(events.0.iter().any(|event| matches!(
            event,
            RecordObservation::Issue {
                code: "record_json",
                ..
            }
        )));
    }

    #[test]
    fn absent_route_refuses_instead_of_selecting_nearest_version() {
        let mut value: Value = serde_json::from_slice(&source(RECORD)).unwrap();
        value["schema_version"] = Value::String("tos_research_corpus_record_v999".into());
        let mut rule = family();
        let mut events = Events::default();
        let error = rule
            .inspect_member(RECORD, &serde_json::to_vec(&value).unwrap(), &mut events)
            .unwrap_err();
        assert!(matches!(
            error,
            RecordRuleError::Unsupported {
                code: "unsupported_record_schema_version",
                ..
            }
        ));
        assert!(matches!(
            events.0.first(),
            Some(RecordObservation::ExactPath { .. })
        ));
    }

    #[test]
    fn duplicate_global_id_is_emitted_twice_for_auditor_conflict_check() {
        let mut rule = family();
        let mut events = Events::default();
        let raw = source(RECORD);
        rule.inspect_member(RECORD, &raw, &mut events).unwrap();
        rule.inspect_member(
            "ToS/source-witnesses/research-corpora/other/research-corpus.json",
            &raw,
            &mut events,
        )
        .unwrap();
        let owners = events
            .0
            .iter()
            .filter(|event| matches!(event, RecordObservation::IdOwner { .. }))
            .count();
        assert_eq!(owners, 2);
        assert!(rule.finish().incomplete);
    }

    fn synthetic_verdict(
        raw_digest: Digest256,
        schema_set_digest: Digest256,
        profile: FormatProfile,
        root: &str,
    ) -> BoundedSchemaVerdict {
        BoundedSchemaVerdict {
            instance_sha256: raw_digest,
            schema_set_digest,
            format_profile: profile,
            root_uri: root.to_owned(),
            worker_protocol_id: "test-only-worker-protocol".to_owned(),
            worker_binary_digest: Digest256::from_hex(&"a".repeat(64)).unwrap(),
            valid: true,
        }
    }

    #[test]
    fn bounded_member_evidence_is_bound_to_exact_bytes_and_both_roots() {
        let mut rule = family();
        let raw = source(RECORD);
        let plan = rule.member_schema_plan(RECORD, &raw).unwrap();
        let evidence = BoundedMemberSchemaEvidence {
            route: synthetic_verdict(
                plan.instance_sha256,
                plan.schema_set_digest,
                plan.format_profile,
                &plan.route_uri,
            ),
            common: synthetic_verdict(
                plan.instance_sha256,
                plan.schema_set_digest,
                plan.format_profile,
                &plan.common_uri,
            ),
        };
        let mut events = Events::default();
        rule.inspect_member_with_bounded_schema(RECORD, &raw, &evidence, &mut events)
            .unwrap();
        assert!(
            events
                .0
                .iter()
                .any(|event| matches!(event, RecordObservation::IdOwner { .. }))
        );
        let mut wrong = evidence;
        wrong.common.instance_sha256 = Digest256::from_hex(&"b".repeat(64)).unwrap();
        assert!(matches!(
            rule.inspect_member_with_bounded_schema(RECORD, &raw, &wrong, &mut events),
            Err(RecordRuleError::Unsupported {
                code: "bounded_schema_evidence_mismatch",
                ..
            })
        ));
    }

    #[test]
    fn native_artifact_uses_physical_id_and_exact_source_schema() {
        const ARTIFACT: &str = "ToS/source-witnesses/artifacts/old-babylonian/susa/hammurabi-stele-sb-8/artifact-witness.json";
        let mut rule = family();
        let raw = source(ARTIFACT);
        let plan = rule.native_schema_plan(ARTIFACT, &raw).unwrap();
        assert!(
            plan.root_uri
                .ends_with("artifact-source-witness-v2.schema.json")
                || plan
                    .root_uri
                    .ends_with("artifact-source-witness.schema.json")
        );
        let evidence = synthetic_verdict(
            plan.instance_sha256,
            plan.schema_set_digest,
            plan.format_profile,
            &plan.root_uri,
        );
        let mut events = Events::default();
        rule.inspect_native_with_bounded_schema(ARTIFACT, &raw, &evidence, &mut events)
            .unwrap();
        assert!(events.0.iter().any(|event| matches!(event,
            RecordObservation::IdOwner { id, .. } if id.starts_with("tos.artifact."))));
        const NESTED: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/editions/chemnitz-schmeitzner-1884-part-3";
        for (path, kind) in [
            (format!("{NESTED}/edition.json"), "edition"),
            (
                format!("{NESTED}/items/dta-sbb-corrected-tei-p5/item.json"),
                "item",
            ),
        ] {
            let raw = source(&path);
            let carrier = rule.classify_current_member(&path, &raw).unwrap().unwrap();
            assert_eq!(carrier.kind, kind);
            assert!(carrier.id.starts_with(&format!("tos.{kind}.")));
            assert!(rule.native_schema_plan(&path, &raw).is_ok());
            let mut malformed: Value = serde_json::from_slice(&raw).unwrap();
            malformed["record_type"] = json!("work");
            assert!(
                rule.classify_current_member(&path, &serde_json::to_vec(&malformed).unwrap())
                    .is_err()
            );
        }
        assert!(
            rule.classify_current_member(
                "ToS/contracts/corpus-record.schema.json",
                &source(CORPUS_CONTRACT)
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn sorted_global_join_separates_duplicates_native_reservations_and_wrong_kind() {
        let agent: Value = serde_json::from_slice(&source(
            "ToS/source-witnesses/agents/erasmus-of-rotterdam/agent.json",
        ))
        .unwrap();
        let agent_id = agent["record_id"].as_str().unwrap().to_owned();
        let mut events = Events::default();
        let duplicates = RecordGlobalJoin::check_id_collisions(
            vec![
                GlobalIdFact {
                    id: agent_id.clone(),
                    kind: "agent".into(),
                    path: "agent-a".into(),
                    carrier: IdCarrier::Standalone,
                },
                GlobalIdFact {
                    id: agent_id.clone(),
                    kind: "agent".into(),
                    path: "agent-b".into(),
                    carrier: IdCarrier::Standalone,
                },
            ],
            RecordFactBudget::laboratory_default(),
            &mut events,
        )
        .unwrap();
        assert_eq!(duplicates, 1);
        assert!(events.0.iter().any(|event| matches!(
            event,
            RecordObservation::Issue {
                code: "duplicate_record_id",
                ..
            }
        )));
        let limited = RecordGlobalJoin::check_id_collisions(
            vec![
                GlobalIdFact {
                    id: "tos.work.a".into(),
                    kind: "work".into(),
                    path: "a".into(),
                    carrier: IdCarrier::Standalone,
                },
                GlobalIdFact {
                    id: "tos.work.b".into(),
                    kind: "work".into(),
                    path: "b".into(),
                    carrier: IdCarrier::Standalone,
                },
            ],
            RecordFactBudget {
                max_facts: 1,
                max_encoded_bytes: 1_000,
            },
            &mut Events::default(),
        );
        assert!(matches!(
            limited,
            Err(RecordRuleError::Budget {
                code: "global_fact_budget"
            })
        ));
        let unsorted = RecordGlobalJoin::check_id_collisions(
            vec![
                GlobalIdFact {
                    id: "tos.work.z".into(),
                    kind: "work".into(),
                    path: "z".into(),
                    carrier: IdCarrier::Standalone,
                },
                GlobalIdFact {
                    id: "tos.work.a".into(),
                    kind: "work".into(),
                    path: "a".into(),
                    carrier: IdCarrier::Standalone,
                },
            ],
            RecordFactBudget::laboratory_default(),
            &mut Events::default(),
        );
        assert!(matches!(
            unsorted,
            Err(RecordRuleError::Unsupported {
                code: "id_facts_unsorted",
                ..
            })
        ));

        let mut events = Events::default();
        let native_collisions = RecordGlobalJoin::check_id_collisions(
            vec![
                GlobalIdFact {
                    id: "tos.sign.test".into(),
                    kind: "sign".into(),
                    path: "packet-v1".into(),
                    carrier: IdCarrier::NativePacket,
                },
                GlobalIdFact {
                    id: "tos.sign.test".into(),
                    kind: "sign".into(),
                    path: "packet-v2".into(),
                    carrier: IdCarrier::NativePacket,
                },
                GlobalIdFact {
                    id: "tos.sign.test".into(),
                    kind: "sign".into(),
                    path: "standalone".into(),
                    carrier: IdCarrier::Standalone,
                },
            ],
            RecordFactBudget::laboratory_default(),
            &mut events,
        )
        .unwrap();
        assert_eq!(native_collisions, 1);
        assert!(events.0.iter().any(|event| matches!(
            event,
            RecordObservation::Issue {
                code: "native_record_id_collision",
                ..
            }
        )));

        let mut events = Events::default();
        let issues = RecordGlobalJoin::check_typed_references(
            vec![GlobalIdFact {
                id: agent_id.clone(),
                kind: "agent".into(),
                path: "agent-a".into(),
                carrier: IdCarrier::Standalone,
            }],
            vec![TypedIdRefFact {
                target_id: agent_id,
                expected_kind: "work".into(),
                from_path: "expression".into(),
            }],
            RecordFactBudget::laboratory_default(),
            &mut events,
        )
        .unwrap();
        assert_eq!(issues, 1);
        assert!(events.0.iter().any(|event| matches!(
            event,
            RecordObservation::Issue {
                code: "record_reference_wrong_kind",
                ..
            }
        )));

        let link: Value = serde_json::from_slice(&source(
            "ToS/source-witnesses/links/cdli/cdlb-2006-1/article/link.json",
        ))
        .unwrap();
        let uri = link["uri"].as_str().unwrap().to_owned();
        let mut events = Events::default();
        let uri_issues = RecordGlobalJoin::check_link_uri_collisions(
            vec![
                LinkUriFact {
                    uri: uri.clone(),
                    id: "tos.link.one".into(),
                    path: "link-a".into(),
                },
                LinkUriFact {
                    uri,
                    id: "tos.link.two".into(),
                    path: "link-b".into(),
                },
            ],
            RecordFactBudget::laboratory_default(),
            &mut events,
        )
        .unwrap();
        assert_eq!(uri_issues, 1);
        assert!(events.0.iter().any(|event| matches!(
            event,
            RecordObservation::Issue {
                code: "duplicate_link_uri",
                ..
            }
        )));
    }

    #[test]
    fn source_link_emits_uri_and_file_reference_facts() {
        const LINK: &str = "ToS/source-witnesses/links/cdli/cdlb-2006-1/article/link.json";
        let mut rule = family();
        let raw = source(LINK);
        let plan = rule.native_schema_plan(LINK, &raw).unwrap();
        let evidence = synthetic_verdict(
            plan.instance_sha256,
            plan.schema_set_digest,
            plan.format_profile,
            &plan.root_uri,
        );
        let mut events = Events::default();
        rule.inspect_native_with_bounded_schema(LINK, &raw, &evidence, &mut events)
            .unwrap();
        assert!(events.0.iter().any(|event| matches!(event,
            RecordObservation::LinkUriOwner { uri, .. } if uri == "https://cdli.earth/articles/cdlb/2006-1")));
        assert!(events.0.iter().any(|event| matches!(
            event,
            RecordObservation::Reference {
                check: PathReferenceCheck::FileIfToS,
                ..
            }
        )));
        assert!(events.0.iter().any(|event| matches!(event,
            RecordObservation::IdKindOwner { kind, .. } if kind == "link")));
    }

    #[test]
    fn local_claim_constructor_preserves_all_declared_routes_and_owner_fences() {
        let entities: Value = serde_json::from_slice(&source(ENTITY_REGISTRY)).unwrap();
        let relations: Value = serde_json::from_slice(&source(LOCAL_CLAIM_REGISTRY)).unwrap();
        let limits = crate::item_rules::ItemLimits {
            max_member_bytes: MAX_RECORD_BYTES,
            max_total_bytes: 32 * 1_048_576,
            max_state_bytes: 64 * 1_048_576,
            max_issues: 64,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let base_state = crate::record_biblio_cut::decoded_state(&entities)
            .unwrap()
            .checked_add(crate::record_biblio_cut::decoded_state(&relations).unwrap())
            .unwrap();
        let routes =
            compile_local_claim_routes(&entities, &relations, base_state, limits, &cancel).unwrap();
        let count: usize = relations["relations"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e.get("source_claim_profile"))
            .map(|p| p["schemas"].as_array().unwrap().len())
            .sum();
        assert_eq!(routes.len(), count);
        assert!(routes.contains_key(&(
            "subject_identity_transition_proposal".into(),
            "tos_subject_identity_transition_claim_v1".into()
        )));
        drop(routes);
        let mut duplicated = relations.clone();
        let entry = duplicated["relations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e.get("source_claim_profile").is_some())
            .unwrap()
            .clone();
        duplicated["relations"].as_array_mut().unwrap().push(entry);
        assert!(matches!(
            compile_local_claim_routes(
                &entities,
                &duplicated,
                base_state
                    .checked_add(crate::record_biblio_cut::decoded_state(&duplicated).unwrap())
                    .unwrap(),
                limits,
                &cancel
            ),
            Err(LocalClaimCompileError::Rule(
                "claim-relation-id-duplicate",
                _
            ))
        ));
        drop(duplicated);
        let mut changed = entities.clone();
        let entry = changed["types"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|e| {
                e["source_record_profile"]
                    .get("identity_proposal_adapter")
                    .is_some()
            })
            .unwrap();
        entry["source_record_profile"]["graph_layer"] = json!("invented");
        assert!(matches!(
            compile_local_claim_routes(
                &changed,
                &relations,
                base_state
                    .checked_add(crate::record_biblio_cut::decoded_state(&changed).unwrap())
                    .unwrap(),
                limits,
                &cancel
            ),
            Err(LocalClaimCompileError::Rule(
                "claim-semantic-identity-adapter",
                _
            ))
        ));
        drop(changed);
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(matches!(
            compile_local_claim_routes(&entities, &relations, base_state, limits, &cancel),
            Err(LocalClaimCompileError::Refusal(_))
        ));
    }

    #[test]
    fn local_claim_shared_value_laws_keep_topology_and_exact_basis_local() {
        let exact =
            |id: &str| json!({"id":id,"version":1,"digest":format!("sha256:{}","a".repeat(64))});
        let mut proposal = json!({"claim_id":"tos.claim.proposal","subject_ref":"tos.work.a","supersedes_claim_ref":null,
            "object":{"operation":"merge","members":["tos.work.a","tos.work.b","tos.work.c"],
                "predecessors":[exact("tos.work.a"),exact("tos.work.b")],"successors":[exact("tos.work.c")],
                "mapping":[{"predecessor":"tos.work.a","successor":"tos.work.c"},{"predecessor":"tos.work.b","successor":"tos.work.c"}],
                "supersedes_proposal":null,"unresolved_links":[]}});
        assert!(local_proposal_participants(&proposal).is_ok());
        proposal["object"]["mapping"].as_array_mut().unwrap().pop();
        assert!(local_proposal_participants(&proposal).is_err());
        assert!(!local_exact_ref(
            &json!({"id":"tos.work.a","version":true,"digest":format!("sha256:{}","a".repeat(64))}),
            false
        ));
        assert!(!local_tos_id("tos.work.bad..suffix", false));
        let limits = crate::item_rules::ItemLimits {
            max_member_bytes: MAX_RECORD_BYTES,
            max_total_bytes: 32 * 1_048_576,
            max_state_bytes: 64 * 1_048_576,
            max_issues: 64,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut order = json!({"subject_ref":"tos.collection.one","object":{"kind":"collection-member-order",
            "members":["tos.work.a","tos.work.b"],"ordering":{"mode":"total","precedes":[["tos.work.a","tos.work.b"]]},
            "collection_version":exact("tos.collection.one"),"membership_versions":[exact("tos.claim.a"),exact("tos.claim.b")]}});
        assert!(
            local_scoped_member_structure(&order, limits, &cancel)
                .unwrap()
                .is_ok()
        );
        order["object"]["ordering"]["precedes"] = json!([]);
        assert_eq!(
            local_scoped_member_structure(&order, limits, &cancel).unwrap(),
            Err("claim-structure-total-incomparable")
        );
        order["object"]["ordering"] = json!({"mode":"partial","precedes":[["tos.work.a","tos.work.b"],["tos.work.b","tos.work.a"]]});
        assert_eq!(
            local_scoped_member_structure(&order, limits, &cancel).unwrap(),
            Err("claim-structure-cycle")
        );
        order["object"]["ordering"] = json!({"mode":"unordered","precedes":[]});
        order["object"]["membership_versions"][1] = exact("tos.claim.a");
        assert_eq!(
            local_scoped_member_structure(&order, limits, &cancel).unwrap(),
            Err("claim-collection-order-exact-basis")
        );
    }
}

// Local human-forms Claim route: SourceClaimProfiles.validate(objects=None).
// This deliberately performs no endpoint, review, history or canon joins.
const LOCAL_CLAIM_REGISTRY: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const LOCAL_CLAIM_CONTRACT: &str = "ToS/contracts/semantic-relation-type-registry.schema.json";
const LOCAL_CLAIM_BASE: &str = "ToS/contracts/source-claim-record.schema.json";
const LOCAL_TEMPORAL: &str = "ToS/contracts/historical-claim.schema.json";
const LOCAL_STRUCTURED: &str = "ToS/contracts/source-structured-value.schema.json";
const LOCAL_MEMBERS: &str = "ToS/contracts/scoped-member-structure.schema.json";
const LOCAL_DOCUMENT: &str = "ToS/contracts/document-catalogue-claim.schema.json";
const LOCAL_DISPLAY: &str = "ToS/contracts/claim-display-fields.schema.json";

/// Observed local mechanics only. An empty issue list carries no permission,
/// endpoint closure, semantic assessment, review or source admission.
#[derive(Debug, Clone)]
pub struct SourceClaimLocalReport {
    pub source_revision: tos_foundation::SourceRevision,
    pub source_input_sha256: Digest256,
    pub decoded_input_sha256: Digest256,
    pub execution_binding: crate::source_cut::CutExecutionBinding,
    pub dependency_digests: BTreeMap<String, Digest256>,
    /// Maintained input_digests insertion order: successful first reads only.
    /// Each path appears exactly once and owns its digest in the map above.
    pub dependency_order: Vec<String>,
    pub issues: Vec<crate::relation_rules::RelationIssue>,
    pub schema_receipts: Vec<crate::source_cut::CutSchemaReceipt>,
    // Private running logical slots/payload total; Arc aliases own no second tree.
    logical_state: usize,
    workspace: usize,
}

/// Candidate-fenced local Claim result. Its input identity, membership and
/// prepared diagnostics worker stay typed; this carries no `SourceRevision`
/// and is only local-form verification evidence.
#[derive(Debug, Clone)]
pub struct CandidateSourceClaimLocalReport<I> {
    pub input_identity: I,
    pub current_membership: tos_source_store::SourceMembershipV1,
    pub source_input_sha256: Digest256,
    pub decoded_input_sha256: Digest256,
    pub prepared_execution_binding: crate::source_cut::CutPreparedSchemaExecutionBinding,
    pub schema_set_sha256: Digest256,
    pub contract_selection_sha256: Digest256,
    pub dependency_digests: BTreeMap<String, Digest256>,
    pub dependency_order: Vec<String>,
    /// Bytes read from selected current dependencies; excludes the caller's
    /// already-borrowed Claim member.
    pub dependency_bytes_read: u64,
    pub issues: Vec<crate::relation_rules::RelationIssue>,
    pub diagnostics_cost: CandidateLocalClaimDiagnosticsCost,
    logical_state: usize,
}

impl<I> CandidateSourceClaimLocalReport<I> {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn logical_state_bytes(&self) -> usize {
        self.logical_state
    }
}

/// Exact totals from the complete candidate diagnostics-v2 exchanges used by
/// one local Claim validation. Each value is summed from its actual validated
/// `CandidateCutSchemaDiagnostic`; incomplete exchanges refuse the operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CandidateLocalClaimDiagnosticsCost {
    pub completed_exchanges: u64,
    pub issue_count: u64,
    pub schema_resource_bytes: u64,
    pub schema_resource_buffer_bytes: usize,
    pub input_instance_bytes: u64,
    pub input_instance_buffer_bytes: usize,
    pub input_metadata_bytes: usize,
    pub request_bytes: u64,
    pub request_buffer_bytes: usize,
    pub response_bytes: u64,
    pub response_buffer_bytes: usize,
    pub worker_cpu_micros: u64,
    pub retained_state_bytes: usize,
    pub accounted_state_bytes: usize,
}

impl CandidateLocalClaimDiagnosticsCost {
    fn include(
        &mut self,
        diagnostic: &crate::source_cut::SchemaDiagnosticResult,
    ) -> Result<(), crate::item_rules::ItemRefusal> {
        use crate::item_rules::ItemRefusal;
        let add_u64 = |left: u64, right: usize| {
            left.checked_add(u64::try_from(right).map_err(|_| crate::item_budget_origin!())?)
                .ok_or(crate::item_budget_origin!())
        };
        self.completed_exchanges = self
            .completed_exchanges
            .checked_add(1)
            .ok_or(crate::item_budget_origin!())?;
        self.issue_count = self
            .issue_count
            .checked_add(diagnostic.report().issues.len() as u64)
            .ok_or(crate::item_budget_origin!())?;
        self.schema_resource_bytes = add_u64(
            self.schema_resource_bytes,
            diagnostic.schema_resource_bytes(),
        )?;
        self.schema_resource_buffer_bytes = self
            .schema_resource_buffer_bytes
            .checked_add(diagnostic.schema_resource_buffer_bytes())
            .ok_or(crate::item_budget_origin!())?;
        self.input_instance_bytes =
            add_u64(self.input_instance_bytes, diagnostic.input_instance_bytes())?;
        self.input_instance_buffer_bytes = self
            .input_instance_buffer_bytes
            .checked_add(diagnostic.input_instance_buffer_bytes())
            .ok_or(crate::item_budget_origin!())?;
        self.input_metadata_bytes = self
            .input_metadata_bytes
            .checked_add(diagnostic.input_metadata_bytes())
            .ok_or(crate::item_budget_origin!())?;
        self.request_bytes = add_u64(self.request_bytes, diagnostic.request_bytes())?;
        self.request_buffer_bytes = self
            .request_buffer_bytes
            .checked_add(diagnostic.request_buffer_bytes())
            .ok_or(crate::item_budget_origin!())?;
        self.response_bytes = add_u64(self.response_bytes, diagnostic.response_bytes())?;
        self.response_buffer_bytes = self
            .response_buffer_bytes
            .checked_add(diagnostic.response_buffer_bytes())
            .ok_or(crate::item_budget_origin!())?;
        self.worker_cpu_micros = self
            .worker_cpu_micros
            .checked_add(diagnostic.worker_cpu_micros())
            .ok_or(crate::item_budget_origin!())?;
        self.retained_state_bytes = self
            .retained_state_bytes
            .checked_add(diagnostic.retained_state_bytes())
            .ok_or(crate::item_budget_origin!())?;
        self.accounted_state_bytes = self
            .accounted_state_bytes
            .checked_add(diagnostic.accounted_state_bytes())
            .ok_or(crate::item_budget_origin!())?;
        Ok(())
    }
}

#[derive(Debug)]
struct LocalClaimReportState {
    source_input_sha256: Digest256,
    decoded_input_sha256: Digest256,
    dependency_digests: BTreeMap<String, Digest256>,
    dependency_order: Vec<String>,
    issues: Vec<crate::relation_rules::RelationIssue>,
    logical_state: usize,
    workspace: usize,
}

trait LocalClaimCurrentSource {
    fn read_member(
        &self,
        path: &str,
        max_bytes: usize,
        remaining_state_bytes: usize,
        limits: crate::item_rules::ItemLimits,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<u8>, crate::item_rules::ItemRefusal>;
}

struct CutLocalClaimSource<'a>(&'a tos_source_store::CorpusCutReader);

impl LocalClaimCurrentSource for CutLocalClaimSource<'_> {
    fn read_member(
        &self,
        path: &str,
        max_bytes: usize,
        _remaining_state_bytes: usize,
        limits: crate::item_rules::ItemLimits,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<u8>, crate::item_rules::ItemRefusal> {
        use crate::item_rules::ItemRefusal;
        let relative = tos_foundation::RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("local Claim dependency path".into()))?;
        let member = self
            .0
            .read_member(
                self.0.current().revision(),
                &relative,
                max_bytes as u64,
                limits.deadline,
                cancelled,
            )
            .map_err(|e| {
                use tos_source_store::StoreErrorCode;
                match e.code {
                    StoreErrorCode::BudgetExceeded => ItemRefusal::BudgetCheck {
                        check: "local Claim selected dependency reader budget",
                        used: None,
                        limit: None,
                    },
                    StoreErrorCode::UnsupportedFormat | StoreErrorCode::UnsupportedPlatform => {
                        ItemRefusal::Unsupported(e.to_string())
                    }
                    _ => ItemRefusal::Source(e.to_string()),
                }
            })?;
        Ok(member.raw)
    }
}

struct CandidateLocalClaimSource<'a>(&'a dyn crate::record_biblio_cut::SourceCutInput);

impl LocalClaimCurrentSource for CandidateLocalClaimSource<'_> {
    fn read_member(
        &self,
        path: &str,
        max_bytes: usize,
        remaining_state_bytes: usize,
        limits: crate::item_rules::ItemLimits,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<u8>, crate::item_rules::ItemRefusal> {
        use crate::item_rules::ItemRefusal;
        use tos_source_store::SourcePresenceV1;
        let selection = self.0.record_selection();
        if selection
            .as_ref()
            .is_some_and(|selection| !selection.contains_member(path))
            && path != ENTITY_REGISTRY
            && path != LOCAL_CLAIM_REGISTRY
            && !(path.starts_with("ToS/contracts/") && path.ends_with(".schema.json"))
        {
            return Err(ItemRefusal::Source(
                "local Claim dependency is outside selected record closure".into(),
            ));
        }
        if self.0.path_presence(path, limits.deadline, cancelled)? != Some(SourcePresenceV1::File) {
            return Err(ItemRefusal::Source(format!(
                "local Claim selected dependency absent from candidate: {path}"
            )));
        }
        let mut selected = None;
        self.0.with_current_member(
            path,
            max_bytes,
            limits.deadline,
            cancelled,
            &mut |meta, bytes| {
                if meta.path != path {
                    return Err(ItemRefusal::Source(
                        "local Claim candidate member path changed".into(),
                    ));
                }
                let size =
                    usize::try_from(meta.size_bytes).map_err(|_| crate::item_budget_origin!())?;
                if size > max_bytes || size != bytes.len() {
                    return Err(ItemRefusal::Source(
                        "local Claim candidate member size changed".into(),
                    ));
                }
                if let Some(selection) = selection
                    .as_ref()
                    .filter(|selection| selection.contains_member(path))
                {
                    selection.verify_metadata_member(path, bytes)?;
                }
                let copy_state = size
                    .checked_add(std::mem::size_of::<Vec<u8>>())
                    .ok_or(crate::item_budget_origin!())?;
                if copy_state > remaining_state_bytes {
                    return Err(ItemRefusal::BudgetCheck {
                        check: "local Claim candidate member copy",
                        used: Some(copy_state as u64),
                        limit: Some(remaining_state_bytes as u64),
                    });
                }
                let mut owned = Vec::with_capacity(size);
                owned.extend_from_slice(bytes);
                selected = Some(owned);
                Ok(())
            },
        )?;
        selected.ok_or_else(|| {
            ItemRefusal::Source("local Claim candidate member visitor omitted bytes".into())
        })
    }
}

trait LocalClaimSchemaWorker {
    fn contract_digest(&self, contract: &str) -> Option<Digest256>;
    fn check_reusing_scalar(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, crate::item_rules::ItemRefusal>;
}

impl<S: CutSchemaExecutor + CutSchemaReceiptRange> LocalClaimSchemaWorker for S {
    fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        CutSchemaReceiptRange::contract_digest(self, contract)
    }

    fn check_reusing_scalar(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, crate::item_rules::ItemRefusal> {
        CutSchemaExecutor::check_reusing_scalar(self, path, raw, contract, deadline, cancelled)
    }
}

struct CandidateLocalClaimSchemaWorker<'a, I> {
    worker: &'a mut crate::source_cut::CandidateCutWorkerSchemaExecutor<I>,
    input_identity: I,
    schema_set_sha256: Digest256,
    contract_selection_sha256: Digest256,
    prepared_execution: crate::source_cut::CutPreparedSchemaExecutionBinding,
    cost: CandidateLocalClaimDiagnosticsCost,
}

impl<I: Copy + Eq> LocalClaimSchemaWorker for CandidateLocalClaimSchemaWorker<'_, I> {
    fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        self.worker.contract_digest(contract)
    }

    fn check_reusing_scalar(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, crate::item_rules::ItemRefusal> {
        use crate::item_rules::ItemRefusal;
        let diagnostic = self
            .worker
            .check_diagnostics_v2(path, raw, contract, deadline, cancelled)?;
        let result = diagnostic.result();
        if diagnostic.input_identity() != &self.input_identity
            || diagnostic.schema_set_sha256() != self.schema_set_sha256
            || diagnostic.contract_selection_sha256() != self.contract_selection_sha256
            || diagnostic.prepared_execution_binding() != self.prepared_execution
            || diagnostic.profile() != self.prepared_execution.schema_profile
            || result.path() != path
            || result.contract() != contract
            || result.source_raw_sha256() != Digest256::of_bytes(raw)
        {
            return Err(ItemRefusal::Source(
                "local Claim candidate diagnostics binding differs".into(),
            ));
        }
        self.cost.include(result)?;
        if result.is_valid() {
            Ok(true)
        } else if result.is_invalid() {
            Ok(false)
        } else {
            Err(ItemRefusal::Unsupported(
                "local Claim candidate schema diagnostics are incomplete".into(),
            ))
        }
    }
}

struct LocalClaimRoute {
    relation: usize,
    schema: usize,
}
type LocalClaimInputs = BTreeMap<String, (std::sync::Arc<Value>, Vec<u8>)>;
fn local_claim_state_check(
    state: usize,
    extra: usize,
    limits: crate::item_rules::ItemLimits,
) -> Result<(), crate::item_rules::ItemRefusal> {
    let used = state.checked_add(extra);
    if used.is_none_or(|n| n > limits.max_state_bytes) {
        return Err(crate::item_rules::ItemRefusal::BudgetCheck {
            check: "local Claim simultaneous logical state",
            used: used.map(|n| n as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    Ok(())
}
fn local_route_state(
    routes: &BTreeMap<(String, String), LocalClaimRoute>,
) -> Result<usize, crate::item_rules::ItemRefusal> {
    routes
        .iter()
        .try_fold(
            std::mem::size_of_val(routes),
            |sum, ((predicate, version), _)| {
                sum.checked_add(
                    std::mem::size_of::<((String, String), LocalClaimRoute)>()
                        + predicate.len()
                        + version.len(),
                )
            },
        )
        .ok_or(crate::item_budget_origin!())
}

/// Execute the source owner's complete local Claim forms route against the
/// same immutable current cut and the maintained disposable schema worker.
/// The selected Claim bytes are bound separately from their decoded instance.
///
/// Candidate callers use `validate_source_claim_from_input`; its opaque input
/// identity remains distinct from the cut-only `SourceRevision` route below.
pub fn validate_source_claim_from_input<I: Copy + Eq>(
    input: &dyn crate::record_biblio_cut::SourceCutInputWithIdentity<I>,
    records: &crate::source_foundation_records::SourceFoundationRecordsStreamedReport<'_, I>,
    selected_claim_raw: &[u8],
    worker: &mut crate::source_cut::CandidateCutWorkerSchemaExecutor<I>,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<CandidateSourceClaimLocalReport<I>, crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    local_claim_checkpoint(limits, cancelled)?;
    if input.input_identity() != records.input_identity()
        || input.input_identity() != worker.input_identity()
    {
        return Err(ItemRefusal::Source(
            "local Claim candidate input identity differs".into(),
        ));
    }
    let candidate_schema = records.candidate_schema_identity().ok_or_else(|| {
        ItemRefusal::Unsupported("local Claim requires a candidate Records schema binding".into())
    })?;
    let prepared = worker.prepared_execution_binding();
    if worker.profile() != FormatProfile::LegacyPythonObserved20260923
        || candidate_schema.profile() != worker.profile()
        || candidate_schema.schema_set_digest() != worker.schema_set_digest()
        || candidate_schema.contract_selection_digest() != worker.contract_selection_digest()
        || candidate_schema.prepared_execution_binding() != prepared
        || candidate_schema.selected_resource_count() != worker.source_resource_count() as u64
        || candidate_schema.selected_resource_bytes() != worker.schema_bytes() as u64
    {
        return Err(ItemRefusal::Source(
            "local Claim candidate schema binding differs from Records".into(),
        ));
    }
    if selected_claim_raw.len() > limits.max_member_bytes.min(MAX_RECORD_BYTES) {
        return Err(ItemRefusal::BudgetCheck {
            check: "local Claim raw member bytes",
            used: Some(selected_claim_raw.len() as u64),
            limit: Some(limits.max_member_bytes.min(MAX_RECORD_BYTES) as u64),
        });
    }
    if selected_claim_raw.len() as u64 > limits.max_total_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "local Claim initial read bytes",
            used: Some(selected_claim_raw.len() as u64),
            limit: Some(limits.max_total_bytes),
        });
    }
    let (claim, claim_state) = crate::record_biblio_cut::bounded_decoded_state(
        selected_claim_raw,
        tos_foundation::JsonLimits::default(),
        limits.max_state_bytes,
        limits.deadline,
        cancelled,
    )?;
    crate::record_biblio_cut::decoded_wire_size(
        &claim,
        limits
            .max_state_bytes
            .checked_sub(claim_state)
            .ok_or(crate::item_budget_origin!())?,
    )?;
    let decoded = serde_json::to_vec(&claim)
        .map_err(|_| ItemRefusal::Unsupported("local Claim decoded serialization".into()))?;
    let logical_state = claim_state
        .checked_add(std::mem::size_of::<CandidateSourceClaimLocalReport<I>>())
        .and_then(|n| n.checked_add(std::mem::size_of::<LocalClaimInputs>()))
        .ok_or(crate::item_budget_origin!())?;
    local_claim_state_check(
        logical_state,
        decoded
            .len()
            .checked_add(std::mem::size_of::<Vec<u8>>())
            .ok_or(crate::item_budget_origin!())?,
        limits,
    )?;
    let mut report = LocalClaimReportState {
        source_input_sha256: Digest256::of_bytes(selected_claim_raw),
        decoded_input_sha256: Digest256::of_bytes(&decoded),
        dependency_digests: BTreeMap::new(),
        dependency_order: Vec::new(),
        issues: Vec::new(),
        logical_state,
        workspace: 0,
    };
    drop(decoded);
    let mut schema_worker = CandidateLocalClaimSchemaWorker {
        worker,
        input_identity: *input.input_identity(),
        schema_set_sha256: candidate_schema.schema_set_digest(),
        contract_selection_sha256: candidate_schema.contract_selection_digest(),
        prepared_execution: prepared,
        cost: CandidateLocalClaimDiagnosticsCost::default(),
    };
    let source = CandidateLocalClaimSource(input.source_input());
    let mut bytes = selected_claim_raw.len() as u64;
    validate_source_claim_local_core(
        &source,
        &claim,
        selected_claim_raw,
        std::mem::size_of::<CandidateSourceClaimLocalReport<I>>(),
        &mut schema_worker,
        limits,
        cancelled,
        &mut bytes,
        &mut report,
    )?;
    drop(claim);
    let diagnostics_cost = std::mem::take(&mut schema_worker.cost);
    drop(schema_worker);
    Ok(CandidateSourceClaimLocalReport {
        input_identity: *input.input_identity(),
        current_membership: *records.source_membership(),
        source_input_sha256: report.source_input_sha256,
        decoded_input_sha256: report.decoded_input_sha256,
        prepared_execution_binding: prepared,
        schema_set_sha256: candidate_schema.schema_set_digest(),
        contract_selection_sha256: candidate_schema.contract_selection_digest(),
        dependency_digests: report.dependency_digests,
        dependency_order: report.dependency_order,
        dependency_bytes_read: bytes
            .checked_sub(selected_claim_raw.len() as u64)
            .ok_or(crate::item_budget_origin!())?,
        issues: report.issues,
        diagnostics_cost,
        logical_state: report.logical_state,
    })
}

pub fn validate_source_claim_from_cut<S: CutSchemaExecutor + CutSchemaReceiptRange>(
    cut: &tos_source_store::CorpusCutReader,
    selected_claim_raw: &[u8],
    worker: &mut S,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<SourceClaimLocalReport, crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    local_claim_checkpoint(limits, cancelled)?;
    let revision = cut.current().revision();
    let binding = worker.execution_binding();
    if worker.source_revision() != revision || binding.source_revision != revision {
        return Err(ItemRefusal::Source(
            "local Claim worker/cut revision differs".into(),
        ));
    }
    if binding.schema_profile != FormatProfile::LegacyPythonObserved20260923 {
        return Err(ItemRefusal::Unsupported(
            "local Claim requires the observed Python FormatChecker profile".into(),
        ));
    }
    if selected_claim_raw.len() > limits.max_member_bytes.min(MAX_RECORD_BYTES) {
        return Err(ItemRefusal::BudgetCheck {
            check: "local Claim raw member bytes",
            used: Some(selected_claim_raw.len() as u64),
            limit: Some(limits.max_member_bytes.min(MAX_RECORD_BYTES) as u64),
        });
    }
    if selected_claim_raw.len() as u64 > limits.max_total_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "local Claim initial read bytes",
            used: Some(selected_claim_raw.len() as u64),
            limit: Some(limits.max_total_bytes),
        });
    }
    // This consumer receives the native owner's decoded JSON transport. A
    // representation Rust cannot retain is Unsupported, never invalid source.
    let (claim, claim_state) = crate::record_biblio_cut::bounded_decoded_state(
        selected_claim_raw,
        tos_foundation::JsonLimits::default(),
        limits.max_state_bytes,
        limits.deadline,
        cancelled,
    )?;
    crate::record_biblio_cut::decoded_wire_size(
        &claim,
        limits
            .max_state_bytes
            .checked_sub(claim_state)
            .ok_or(crate::item_budget_origin!())?,
    )?;
    let decoded = serde_json::to_vec(&claim)
        .map_err(|_| ItemRefusal::Unsupported("local Claim decoded serialization".into()))?;
    let logical_state = claim_state
        .checked_add(std::mem::size_of::<SourceClaimLocalReport>())
        .and_then(|n| n.checked_add(std::mem::size_of::<LocalClaimInputs>()))
        .ok_or(crate::item_budget_origin!())?;
    local_claim_state_check(
        logical_state,
        decoded.len() + std::mem::size_of::<Vec<u8>>(),
        limits,
    )?;
    if !worker.receipt_range_supported() {
        return Err(ItemRefusal::Unsupported(
            "local Claim report requires legacy schema receipts".into(),
        ));
    }
    let first_receipt = worker.receipt_count();
    let mut report = LocalClaimReportState {
        source_input_sha256: Digest256::of_bytes(selected_claim_raw),
        decoded_input_sha256: Digest256::of_bytes(&decoded),
        dependency_digests: BTreeMap::new(),
        dependency_order: Vec::new(),
        issues: Vec::new(),
        logical_state,
        workspace: 0,
    };
    drop(decoded);
    let mut bytes = selected_claim_raw.len() as u64;
    let source = CutLocalClaimSource(cut);
    validate_source_claim_local_core(
        &source,
        &claim,
        selected_claim_raw,
        std::mem::size_of::<SourceClaimLocalReport>(),
        worker,
        limits,
        cancelled,
        &mut bytes,
        &mut report,
    )?;
    drop(claim);
    // Only this invocation's concrete executed receipts are exposed.
    let receipt_end = worker.receipt_count();
    let receipts = crate::source_cut::collect_schema_receipt_range(
        worker,
        first_receipt,
        receipt_end,
        limits
            .max_state_bytes
            .checked_sub(report.logical_state)
            .ok_or(crate::item_budget_origin!())?,
        limits.deadline,
        cancelled,
    )?;
    let receipt_state = receipts
        .iter()
        .try_fold(0usize, |sum, r| {
            sum.checked_add(
                std::mem::size_of::<crate::source_cut::CutSchemaReceipt>()
                    + r.path.len()
                    + r.contract.len(),
            )
        })
        .ok_or(crate::item_budget_origin!())?;
    local_claim_state_check(report.logical_state, receipt_state, limits)?;
    report.logical_state = report
        .logical_state
        .checked_add(receipt_state)
        .ok_or(crate::item_budget_origin!())?;
    Ok(SourceClaimLocalReport {
        source_revision: revision,
        source_input_sha256: report.source_input_sha256,
        decoded_input_sha256: report.decoded_input_sha256,
        execution_binding: binding,
        dependency_digests: report.dependency_digests,
        dependency_order: report.dependency_order,
        issues: report.issues,
        schema_receipts: receipts,
        logical_state: report.logical_state,
        workspace: report.workspace,
    })
}

/// Validate a selected prepared `source-claims.jsonl` carrier using the same
/// native route compiler and maintained local Claim profile predicates as the
/// source-cut validator. This checks local row shape/schema only; endpoints,
/// history, review and admission remain outside this transfer preflight. The
/// caller supplies rooted reads for local dependencies; one bounded cache pins
/// each dependency's bytes for this entire validation operation.
pub fn validate_acquisition_source_claims(
    source_ref: &str,
    raw: &[u8],
    dependency_reader: &mut impl FnMut(&str, usize) -> Result<Vec<u8>, String>,
) -> Result<Vec<(usize, String)>, String> {
    use crate::{FormatProfile, SchemaBackendProbe, SchemaResource};
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    const MAX_CARRIER_BYTES: usize = 16 * 1024 * 1024;
    const MAX_ROW_BYTES: usize = 1024 * 1024;

    if raw.len() > MAX_CARRIER_BYTES {
        return Err(format!(
            "selected source Claim carrier exceeds 16 MiB: {source_ref}"
        ));
    }
    let path = tos_foundation::RelativePath::parse(source_ref).map_err(|_| {
        format!("selected source Claim path is outside its metadata home: {source_ref}")
    })?;
    if !tos_source_store::is_authored_source_path_v1(source_ref)
        || source_ref.split('/').any(|part| {
            matches!(
                part,
                "catalog" | "payload" | "local-content" | "owner-local"
            )
        })
        || path.as_str().rsplit('/').next() != Some("source-claims.jsonl")
    {
        return Err(format!(
            "selected source Claim path is outside its metadata home: {source_ref}"
        ));
    }

    const MAX_DEPENDENCY_CACHE_BYTES: usize =
        SchemaBackendProbe::MAX_TOTAL_BYTES + 2 * MAX_RECORD_BYTES;
    const MAX_DEPENDENCY_CACHE_RESOURCES: usize = SchemaBackendProbe::MAX_RESOURCES + 2;

    fn read_local(
        cache: &mut BTreeMap<String, Vec<u8>>,
        reader: &mut impl FnMut(&str, usize) -> Result<Vec<u8>, String>,
        reference: &str,
        cap: usize,
    ) -> Result<Vec<u8>, String> {
        if !is_contract_path(reference)
            && !reference.starts_with("ToS/doctrine/semantic-interchange/")
        {
            return Err(format!(
                "local Claim dependency path is unsupported: {reference}"
            ));
        }
        if let Some(bytes) = cache.get(reference) {
            if bytes.len() > cap {
                return Err(format!("local Claim dependency exceeds bound: {reference}"));
            }
            return Ok(bytes.clone());
        }
        if cache.len() >= MAX_DEPENDENCY_CACHE_RESOURCES {
            return Err("local Claim dependency cache resource count exceeds native bound".into());
        }
        let bytes = reader(reference, cap)?;
        if bytes.len() > cap {
            return Err(format!("local Claim dependency exceeds bound: {reference}"));
        }
        let used = cache
            .values()
            .try_fold(bytes.len(), |sum, existing| sum.checked_add(existing.len()))
            .ok_or("local Claim dependency cache byte count overflow")?;
        if used > MAX_DEPENDENCY_CACHE_BYTES {
            return Err("local Claim dependency cache bytes exceed native bound".into());
        }
        cache.insert(reference.to_owned(), bytes.clone());
        Ok(bytes)
    }

    fn read_value(
        cache: &mut BTreeMap<String, Vec<u8>>,
        reader: &mut impl FnMut(&str, usize) -> Result<Vec<u8>, String>,
        reference: &str,
        cap: usize,
    ) -> Result<Value, String> {
        let bytes = read_local(cache, reader, reference, cap)?;
        crate::published_value(&bytes, cap).map_err(|error| {
            format!("local Claim dependency is not strict JSON: {reference}: {error:?}")
        })
    }

    fn refs(value: &Value, output: &mut Vec<String>) {
        let mut stack = vec![value];
        while let Some(current) = stack.pop() {
            match current {
                Value::Object(object) => {
                    for (key, child) in object {
                        if matches!(key.as_str(), "$ref" | "$dynamicRef") {
                            if let Some(reference) = child.as_str() {
                                output.push(reference.to_owned());
                            }
                        }
                        stack.push(child);
                    }
                }
                Value::Array(values) => stack.extend(values),
                _ => {}
            }
        }
    }

    fn dependency(reference: &str, current: &str) -> Result<Option<String>, String> {
        let base = reference.split('#').next().unwrap_or("");
        if base.is_empty() {
            return Ok(None);
        }
        let path = if let Some(value) = base.strip_prefix("https://tree-of-sophia.local/") {
            value.to_owned()
        } else if let Some(value) = base.strip_prefix("https://treeofsophia.local/") {
            value.to_owned()
        } else if base.contains("://") || base.starts_with('/') {
            return Err(format!(
                "local Claim schema dependency is outside ToS contracts: {reference}"
            ));
        } else {
            let parent = current
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("");
            if parent.is_empty() {
                base.to_owned()
            } else {
                format!("{parent}/{base}")
            }
        };
        let parsed = tos_foundation::RelativePath::parse(&path)
            .map_err(|_| format!("unsafe local Claim schema dependency: {reference}"))?;
        if !is_contract_path(parsed.as_str()) {
            return Err(format!(
                "local Claim schema dependency leaves contract home: {reference}"
            ));
        }
        Ok(Some(path))
    }

    fn make_probe(
        cache: &mut BTreeMap<String, Vec<u8>>,
        reader: &mut impl FnMut(&str, usize) -> Result<Vec<u8>, String>,
        roots: &[String],
    ) -> Result<(SchemaBackendProbe, BTreeMap<String, String>), String> {
        let mut pending = roots.to_vec();
        let mut seen = BTreeSet::new();
        let mut resources = Vec::new();
        let mut uri_by_path = BTreeMap::new();
        let mut total = 0usize;
        while let Some(current_path) = pending.pop() {
            if !seen.insert(current_path.clone()) {
                continue;
            }
            if seen.len() > SchemaBackendProbe::MAX_RESOURCES {
                return Err("local Claim schema resource count exceeds native bound".into());
            }
            let raw = read_local(
                cache,
                reader,
                &current_path,
                SchemaBackendProbe::MAX_RESOURCE_BYTES,
            )?;
            total = total
                .checked_add(raw.len())
                .filter(|n| *n <= SchemaBackendProbe::MAX_TOTAL_BYTES)
                .ok_or("local Claim schema bytes exceed native bound")?;
            let schema = crate::published_value(&raw, SchemaBackendProbe::MAX_RESOURCE_BYTES)
                .map_err(|e| {
                    format!("local Claim schema is not strict JSON: {current_path}: {e:?}")
                })?;
            let uri = schema_uri(&current_path, &schema)
                .map_err(|e| format!("local Claim schema identity: {e:?}"))?;
            uri_by_path.insert(current_path.clone(), uri.clone());
            let mut dependencies = Vec::new();
            refs(&schema, &mut dependencies);
            for reference in dependencies {
                if let Some(path) = dependency(&reference, &current_path)? {
                    pending.push(path);
                }
            }
            resources.push(SchemaResource { uri, raw });
        }
        let probe = SchemaBackendProbe::new(resources, FormatProfile::LegacyPythonObserved20260923)
            .map_err(|e| format!("local Claim schema set is invalid: {e:?}"))?;
        probe
            .compile_all()
            .map_err(|e| format!("local Claim schema compilation failed: {e:?}"))?;
        Ok((probe, uri_by_path))
    }

    fn check_declared_schema_refs(
        cache: &mut BTreeMap<String, Vec<u8>>,
        reader: &mut impl FnMut(&str, usize) -> Result<Vec<u8>, String>,
        roots: &[String],
        uris: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        let allowed = roots
            .iter()
            .filter_map(|root| uris.get(root).map(String::as_str))
            .collect::<BTreeSet<_>>();
        for path in roots {
            let schema = read_value(cache, reader, path, SchemaBackendProbe::MAX_RESOURCE_BYTES)?;
            local_schema_walk(&schema, true, &mut |key, value| {
                if matches!(key, "$ref" | "$dynamicRef") {
                    let reference = value.as_str().ok_or_else(|| {
                        crate::item_rules::ItemRefusal::Unsupported(
                            "local Claim schema ref representation".into(),
                        )
                    })?;
                    let base = reference.split('#').next().unwrap_or("");
                    if !base.is_empty() && !allowed.contains(base) {
                        return Err(crate::item_rules::ItemRefusal::Unsupported(format!(
                            "local Claim undeclared schema dependency: {path}: {reference}"
                        )));
                    }
                }
                Ok(())
            })
            .map_err(|error| format!("local Claim schema dependency check failed: {error:?}"))?;
        }
        Ok(())
    }

    // This is the exact local no-format constructor closure already used by
    // the source-cut Claim route; malformed/changed registries fail closed.
    let mut dependency_cache = BTreeMap::<String, Vec<u8>>::new();
    let entity_raw = read_value(
        &mut dependency_cache,
        dependency_reader,
        ENTITY_REGISTRY,
        MAX_RECORD_BYTES,
    )?;
    let relation_raw = read_value(
        &mut dependency_cache,
        dependency_reader,
        LOCAL_CLAIM_REGISTRY,
        MAX_RECORD_BYTES,
    )?;
    let registry_roots = [ENTITY_CONTRACT.to_owned(), LOCAL_CLAIM_CONTRACT.to_owned()];
    for contract in &registry_roots {
        let schema = read_value(
            &mut dependency_cache,
            dependency_reader,
            contract,
            SchemaBackendProbe::MAX_RESOURCE_BYTES,
        )?;
        local_registry_schema(&schema).map_err(|error| {
            format!("local Claim registry schema changed its supported closure: {error:?}")
        })?;
    }
    let (registry_probe, registry_uris) =
        make_probe(&mut dependency_cache, dependency_reader, &registry_roots)?;
    check_declared_schema_refs(
        &mut dependency_cache,
        dependency_reader,
        &registry_roots,
        &registry_uris,
    )?;
    for (registry, contract) in [
        (&entity_raw, ENTITY_CONTRACT),
        (&relation_raw, LOCAL_CLAIM_CONTRACT),
    ] {
        let uri = registry_uris
            .get(contract)
            .ok_or("local Claim registry schema absent")?;
        if !registry_probe
            .is_valid_value(uri, registry)
            .map_err(|e| format!("local Claim registry schema execution failed: {e:?}"))?
        {
            return Err(format!(
                "local Claim registry violates its contract: {contract}"
            ));
        }
    }
    drop(registry_probe);
    drop(registry_uris);
    let limits = crate::item_rules::ItemLimits {
        max_member_bytes: MAX_ROW_BYTES,
        max_total_bytes: MAX_CARRIER_BYTES as u64 + 64 * 1024 * 1024,
        max_state_bytes: 128 * 1024 * 1024,
        max_issues: 1,
        deadline: Instant::now() + Duration::from_secs(60),
    };
    let cancelled = AtomicBool::new(false);
    let routes = compile_local_claim_routes(&entity_raw, &relation_raw, 0, limits, &cancelled)
        .map_err(|error| format!("local Claim relation profile is invalid: {error:?}"))?;

    let mut rows = Vec::new();
    let mut route_probes =
        BTreeMap::<(String, String, bool), (SchemaBackendProbe, BTreeMap<String, String>)>::new();
    let mut line_number = 1usize;
    let mut start = 0usize;
    let mut index = 0usize;
    while index <= raw.len() {
        if index != raw.len() && raw[index] != b'\n' && raw[index] != b'\r' {
            index += 1;
            continue;
        }
        let mut line = &raw[start..index];
        if line.len() > MAX_ROW_BYTES {
            return Err(format!(
                "selected source Claim row exceeds 1 MiB: {source_ref}:{line_number}"
            ));
        }
        if line.last() == Some(&b'\r') {
            line = &line[..line.len() - 1];
        }
        if !std::str::from_utf8(line).is_ok_and(|text| text.trim().is_empty()) {
            let claim = crate::published_value(line, MAX_ROW_BYTES)
                .map_err(|e| format!("selected source Claim row is not strict JSON: {source_ref}:{line_number}: {e:?}"))?;
            if !claim.is_object() {
                return Err(format!(
                    "selected source Claim row must be an object: {source_ref}:{line_number}"
                ));
            }
            let predicate = local_string(&claim, "predicate").ok_or_else(|| format!("selected source Claim requires a string predicate and version: {source_ref}:{line_number}"))?;
            let version = local_string(&claim, "schema_version").ok_or_else(|| format!("selected source Claim requires a string predicate and version: {source_ref}:{line_number}"))?;
            let route = routes.get(&(predicate.to_owned(), version.to_owned()))
                .ok_or_else(|| format!("selected source Claim has unrecognized predicate/version: {source_ref}:{line_number}"))?;
            let profile = &relation_raw["relations"][route.relation]["source_claim_profile"];
            let schema_route = &profile["schemas"][route.schema];
            let reader = local_string(profile, "reader")
                .ok_or("local Claim compiled route has no reader")?;
            let selected_schema = local_string(schema_route, "schema_ref")
                .ok_or("local Claim selected schema is missing")?;
            let mut schema_roots = vec![
                "ToS/contracts/claim-packet.schema.json".to_owned(),
                "ToS/contracts/knowledge-assessment.schema.json".to_owned(),
                LOCAL_CLAIM_BASE.to_owned(),
                selected_schema.to_owned(),
            ];
            schema_roots.extend(
                local_strings(schema_route, "schema_dependencies")
                    .into_iter()
                    .map(str::to_owned),
            );
            if local_is_temporal(reader) {
                schema_roots.push(
                    if reader == "document-catalogue-temporal-v1" {
                        LOCAL_DOCUMENT
                    } else {
                        LOCAL_TEMPORAL
                    }
                    .to_owned(),
                );
            }
            if local_is_structured(reader) {
                schema_roots.extend([CORPUS_CONTRACT.to_owned(), LOCAL_STRUCTURED.to_owned()]);
            }
            let scoped = local_string(&profile["object_reference_set"], "structure_adapter")
                == Some("scoped-members-v1");
            if scoped {
                schema_roots.push(LOCAL_MEMBERS.to_owned());
            }
            let qualifiers = &claim["qualifiers"];
            let display = qualifiers["display_fields"].is_object()
                && local_string(&qualifiers["display_fields"], "schema_version")
                    == Some("tos_claim_display_fields_v1");
            if display {
                schema_roots.extend([CORPUS_CONTRACT.to_owned(), LOCAL_DISPLAY.to_owned()]);
            }
            schema_roots.sort();
            schema_roots.dedup();
            let probe_key = (predicate.to_owned(), version.to_owned(), display);
            if !route_probes.contains_key(&probe_key) {
                if route_probes.len() >= MAX_COMPILED_ROUTES * 2 {
                    return Err("local Claim schema route count exceeds native bound".into());
                }
                let (probe, uris) =
                    make_probe(&mut dependency_cache, dependency_reader, &schema_roots)?;
                check_declared_schema_refs(
                    &mut dependency_cache,
                    dependency_reader,
                    &schema_roots,
                    &uris,
                )?;
                route_probes.insert(probe_key.clone(), (probe, uris));
            }
            let (probe, uris) = route_probes
                .get(&probe_key)
                .ok_or("local Claim schema route absent")?;
            let valid = |path: &str, value: &Value| -> Result<bool, String> {
                let uri = uris
                    .get(path)
                    .ok_or_else(|| format!("local Claim schema absent: {path}"))?;
                probe
                    .is_valid_value(uri, value)
                    .map_err(|e| format!("local Claim schema execution failed: {e:?}"))
            };
            if !matches!(
                local_string(&claim, "visibility"),
                Some("public" | "public_metadata_only")
            ) || local_string(&claim, "claim_type") != Some("relation")
                || local_string(&claim, "subject_ref").is_none()
                || !(if local_is_temporal(reader) || local_is_structured(reader) {
                    claim["object"].is_object()
                } else {
                    claim["object"].is_string()
                })
                || local_string(&claim, "assertion_layer").is_none_or(|layer| {
                    !local_strings(profile, "assertion_layers").contains(&layer)
                })
                || claim["claim_id"] == claim["subject_ref"]
                || claim["claim_id"] == claim["object"]
            {
                return Err(format!(
                    "selected source Claim identity, endpoints or layer violate its profile: {source_ref}:{line_number}"
                ));
            }
            if !valid(selected_schema, &claim)? || !valid(LOCAL_CLAIM_BASE, &claim)? {
                return Err(format!(
                    "selected source Claim violates its exact schema or shared record contract: {source_ref}:{line_number}"
                ));
            }
            if local_is_temporal(reader) {
                let (schema, definition) = if reader == "document-catalogue-temporal-v1" {
                    (LOCAL_DOCUMENT, "documentDate")
                } else {
                    (LOCAL_TEMPORAL, "historicalDate")
                };
                let root = format!(
                    "{}#/$defs/{definition}",
                    uris.get(schema)
                        .ok_or("local Claim temporal schema absent")?
                );
                if !probe
                    .is_valid_value(&root, &claim["object"])
                    .map_err(|e| format!("local Claim temporal schema execution failed: {e:?}"))?
                {
                    return Err(format!(
                        "selected source Claim violates temporal value contract: {source_ref}:{line_number}"
                    ));
                }
            }
            if local_is_structured(reader)
                && (!valid(LOCAL_STRUCTURED, &claim["object"])?
                    || claim["object"]["kind"] != profile["value_kind"])
            {
                return Err(format!(
                    "selected source Claim violates structured value contract: {source_ref}:{line_number}"
                ));
            }
            if display && !valid(LOCAL_DISPLAY, &qualifiers)? {
                return Err(format!(
                    "selected source Claim display fields violate their contract: {source_ref}:{line_number}"
                ));
            }
            if let Some(role) = local_document_role(predicate) {
                let attribution = &qualifiers["catalogue_attribution"];
                if local_string(attribution, "field_role") != Some(role)
                    || !claim["evidence_refs"]
                        .as_array()
                        .is_some_and(|items| items.contains(&attribution["evidence_ref"]))
                    || predicate == "document_catalogue_date"
                        && attribution["source_wording"] != claim["object"]["source_wording"]
                {
                    return Err(format!(
                        "selected source Claim document attribution is invalid: {source_ref}:{line_number}"
                    ));
                }
            }
            if local_is_proposal(reader) {
                local_proposal_participants(&claim).map_err(|code| format!("selected source Claim violates proposal structure ({code}): {source_ref}:{line_number}"))?;
            } else if reader == "structured-reference-value-v1" {
                let constraint = &profile["object_reference_set"];
                let members = claim["object"]["members"].as_array();
                let valid_members = members.is_some_and(|members| {
                    let ids = members.iter().filter_map(Value::as_str).collect::<Vec<_>>();
                    constraint["min_items"]
                        .as_u64()
                        .zip(constraint["max_items"].as_u64())
                        .is_some_and(|(min, max)| {
                            min <= members.len() as u64 && members.len() as u64 <= max
                        })
                        && ids.len() == members.len()
                        && ids.iter().all(|id| local_tos_id(id, false))
                        && ids.iter().copied().collect::<BTreeSet<_>>().len() == members.len()
                        && !members.contains(&claim["claim_id"])
                        && (constraint["subject_is_member"] != true
                            || members.contains(&claim["subject_ref"]))
                });
                if !valid_members {
                    return Err(format!(
                        "selected source Claim reference members violate profile: {source_ref}:{line_number}"
                    ));
                }
            }
            if scoped {
                if !valid(LOCAL_MEMBERS, &claim["object"])? {
                    return Err(format!(
                        "selected source Claim violates scoped-member schema: {source_ref}:{line_number}"
                    ));
                }
                if let Err(code) = local_scoped_member_structure(&claim, limits, &cancelled)
                    .map_err(|error| {
                        format!("selected source Claim structure check unavailable: {error:?}")
                    })?
                {
                    return Err(format!(
                        "selected source Claim violates scoped-member structure ({code}): {source_ref}:{line_number}"
                    ));
                }
            }
            let claim_id = local_string(&claim, "claim_id").ok_or_else(|| {
                format!("selected source Claim ID is missing: {source_ref}:{line_number}")
            })?;
            rows.push((line_number, claim_id.to_owned()));
        }
        if index == raw.len() {
            break;
        }
        if raw[index] == b'\r' && raw.get(index + 1) == Some(&b'\n') {
            index += 1;
        }
        index += 1;
        start = index;
        line_number += 1;
    }
    Ok(rows)
}

fn local_claim_checkpoint(
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<(), crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(ItemRefusal::Source("local Claim cancelled".into()));
    }
    if std::time::Instant::now() >= limits.deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}
fn local_claim_issue(
    report: &mut LocalClaimReportState,
    limits: crate::item_rules::ItemLimits,
    code: &'static str,
    location: &str,
) -> Result<(), crate::item_rules::ItemRefusal> {
    if report.issues.len() >= limits.max_issues {
        return Err(crate::item_rules::ItemRefusal::BudgetCheck {
            check: "local Claim issue count",
            used: (report.issues.len() as u64).checked_add(1),
            limit: Some(limits.max_issues as u64),
        });
    }
    let amount = std::mem::size_of::<crate::relation_rules::RelationIssue>()
        .checked_add(location.len())
        .ok_or(crate::item_budget_origin!())?;
    local_claim_state_check(
        report.logical_state,
        report
            .workspace
            .checked_add(amount)
            .ok_or(crate::item_budget_origin!())?,
        limits,
    )?;
    report.logical_state = report
        .logical_state
        .checked_add(amount)
        .ok_or(crate::item_budget_origin!())?;
    report.issues.push(crate::relation_rules::RelationIssue {
        code,
        location: location.to_owned(),
    });
    Ok(())
}
fn local_claim_input(
    source: &dyn LocalClaimCurrentSource,
    path: &str,
    _claim: &Value,
    extra: usize,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
    bytes: &mut u64,
    inputs: &mut LocalClaimInputs,
    report: &mut LocalClaimReportState,
) -> Result<(), crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    local_claim_checkpoint(limits, cancelled)?;
    if inputs.contains_key(path) {
        return Ok(());
    }
    if inputs.len() >= MAX_SOURCE_RESOURCES {
        return Err(ItemRefusal::BudgetCheck {
            check: "local Claim source resource count",
            used: (inputs.len() as u64).checked_add(1),
            limit: Some(MAX_SOURCE_RESOURCES as u64),
        });
    }
    let remaining_state_bytes = limits
        .max_state_bytes
        .checked_sub(report.logical_state)
        .and_then(|n| n.checked_sub(extra))
        .ok_or(crate::item_budget_origin!())?;
    let raw = source.read_member(
        path,
        limits.max_member_bytes.min(MAX_RECORD_BYTES),
        remaining_state_bytes,
        limits,
        cancelled,
    )?;
    let next_bytes = bytes.checked_add(raw.len() as u64);
    *bytes =
        next_bytes
            .filter(|n| *n <= limits.max_total_bytes)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "local Claim selected dependency read bytes",
                used: next_bytes,
                limit: Some(limits.max_total_bytes),
            })?;
    local_claim_state_check(
        report.logical_state,
        extra
            .checked_add(raw.len())
            .ok_or(crate::item_budget_origin!())?,
        limits,
    )?;
    let available = limits
        .max_state_bytes
        .checked_sub(report.logical_state)
        .and_then(|n| n.checked_sub(extra))
        .and_then(|n| n.checked_sub(raw.len()))
        .ok_or(crate::item_budget_origin!())?;
    let (value, value_state) = crate::record_biblio_cut::bounded_decoded_state(
        &raw,
        tos_foundation::JsonLimits::default(),
        available,
        limits.deadline,
        cancelled,
    )?;
    if !value.is_object() {
        return Err(ItemRefusal::Unsupported(format!(
            "local Claim dependency must be an object: {path}"
        )));
    }
    // Price this newly owned tree once, before its three owned path entries.
    let added = value_state
        .checked_add(raw.len())
        .and_then(|n| {
            n.checked_add(std::mem::size_of::<(
                String,
                (std::sync::Arc<Value>, Vec<u8>),
            )>())
        })
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
        .and_then(|n| {
            n.checked_add(
                std::mem::size_of::<(String, Digest256)>() + std::mem::size_of::<String>(),
            )
        })
        .and_then(|n| n.checked_add(path.len().checked_mul(3)?))
        .ok_or(crate::item_budget_origin!())?;
    local_claim_state_check(
        report.logical_state,
        extra
            .checked_add(added)
            .ok_or(crate::item_budget_origin!())?,
        limits,
    )?;
    report.logical_state = report
        .logical_state
        .checked_add(added)
        .ok_or(crate::item_budget_origin!())?;
    report
        .dependency_digests
        .insert(path.into(), Digest256::of_bytes(&raw));
    inputs.insert(path.into(), (std::sync::Arc::new(value), raw));
    report.dependency_order.push(path.into());
    local_claim_state_check(report.logical_state, extra, limits)?;
    Ok(())
}

// Inspect schema locations only; arbitrary const/enum/example values remain
// data. Nested resource identities would need the source owner's exact resolver
// semantics and therefore retain Unsupported in this fixed local adapter.
fn local_schema_walk(
    value: &Value,
    root: bool,
    visit: &mut impl FnMut(&str, &Value) -> Result<(), crate::item_rules::ItemRefusal>,
) -> Result<(), crate::item_rules::ItemRefusal> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    for (key, item) in object {
        if key == "$id" && !root {
            return Err(crate::item_rules::ItemRefusal::Unsupported(
                "local Claim nested schema identity".into(),
            ));
        }
        visit(key, item)?;
        match key.as_str() {
            "$defs" | "definitions" | "properties" | "patternProperties" | "dependentSchemas" => {
                if let Some(children) = item.as_object() {
                    for child in children.values() {
                        local_schema_walk(child, false, visit)?;
                    }
                }
            }
            "allOf" | "anyOf" | "oneOf" | "prefixItems" => {
                if let Some(children) = item.as_array() {
                    for child in children {
                        local_schema_walk(child, false, visit)?;
                    }
                }
            }
            "items"
            | "contains"
            | "additionalProperties"
            | "unevaluatedProperties"
            | "unevaluatedItems"
            | "propertyNames"
            | "not"
            | "if"
            | "then"
            | "else" => local_schema_walk(item, false, visit)?,
            _ => {}
        }
    }
    Ok(())
}
fn local_registry_schema(schema: &Value) -> Result<(), crate::item_rules::ItemRefusal> {
    local_schema_walk(schema, true, &mut |key, value| {
        if key == "format"
            || (matches!(key, "$ref" | "$dynamicRef")
                && value.as_str().is_none_or(|s| !s.starts_with('#')))
        {
            return Err(crate::item_rules::ItemRefusal::Unsupported("local Claim changed registry constructor schema requires an exact no-format closure".into()));
        }
        Ok(())
    })
}

#[derive(Debug)]
enum LocalClaimCompileError {
    Rule(&'static str, String),
    Refusal(crate::item_rules::ItemRefusal),
}
impl From<RecordRuleError> for LocalClaimCompileError {
    fn from(error: RecordRuleError) -> Self {
        match error {
            RecordRuleError::Unsupported { code, detail } => Self::Rule(code, detail),
            other => Self::Refusal(crate::item_rules::ItemRefusal::Unsupported(format!(
                "local Claim registry: {other:?}"
            ))),
        }
    }
}
fn local_route_failure(code: &'static str, detail: &str) -> LocalClaimCompileError {
    LocalClaimCompileError::Rule(code, detail.into())
}
fn local_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
fn local_strings<'a>(value: &'a Value, key: &str) -> Vec<&'a str> {
    value[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}
fn local_specific(id: &str) -> bool {
    !matches!(
        id,
        "tos.entity.thing"
            | "tos.entity.identity"
            | "tos.entity.semantic-object"
            | "tos.entity.unmapped"
            | "tos.entity.unresolved-endpoint"
    )
}
fn local_semantic_eligible(entity: &Entity) -> bool {
    let Some(p) = entity.profile.as_ref() else {
        return false;
    };
    let Some(kind) = local_string(p, "record_type") else {
        return false;
    };
    !entity.abstract_type
        && entity.role == "semantic"
        && local_string(p, "reader") == Some("semantic-metadata-v1")
        && local_string(p, "identity_proposal_adapter") == Some("exact-semantic-metadata-v1")
        && local_string(p, "graph_layer") == Some("source-profile")
        && source_kind(kind)
        && !matches!(kind, "claim" | "literal" | "temporal-assertion")
        && local_string(p, "id_prefix") == Some(format!("tos.{kind}.").as_str())
        && local_string(p, "source_basename") == Some(format!("{kind}.json").as_str())
        && p["schemas"].as_array().is_some_and(|s| !s.is_empty())
        && ["source-claims", "source-navigation"].iter().all(|graph| {
            entity
                .mappings
                .iter()
                .filter(|(g, k)| *g == *graph && *k == kind)
                .count()
                == 1
        })
}
fn local_is_proposal(reader: &str) -> bool {
    matches!(reader, "identity-transition-v1" | "identity-transition-v2")
}
fn local_is_structured(reader: &str) -> bool {
    matches!(
        reader,
        "structured-value-v1"
            | "structured-reference-value-v1"
            | "identity-transition-v1"
            | "identity-transition-v2"
    )
}
fn local_is_temporal(reader: &str) -> bool {
    matches!(
        reader,
        "historical-temporal-v1" | "document-catalogue-temporal-v1"
    )
}
fn local_document_role(predicate: &str) -> Option<&'static str> {
    match predicate {
        "document_catalogue_date" => Some("assigned-date"),
        "document_catalogue_origin" => Some("origin"),
        "document_catalogue_destination" => Some("destination"),
        _ => None,
    }
}
fn local_constructor_state(
    entities: &Value,
    relations: &Value,
) -> Result<usize, crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    let mut state = std::mem::size_of::<BTreeMap<&str, Entity<'_>>>()
        + std::mem::size_of::<BTreeMap<&str, &str>>()
        + std::mem::size_of::<BTreeSet<&str>>()
        + std::mem::size_of::<BTreeMap<&str, usize>>()
        + 2 * std::mem::size_of::<BTreeSet<&str>>()
        + std::mem::size_of::<Vec<(&str, bool)>>();
    let mut parents = 0usize;
    let mut nodes = 0usize;
    for entry in entities["types"].as_array().into_iter().flatten() {
        nodes = nodes.checked_add(1).ok_or(crate::item_budget_origin!())?;
        let p = entry["parent_type_ids"].as_array().map_or(0, Vec::len);
        let m = entry["source_mappings"].as_array().map_or(0, Vec::len);
        parents = parents.checked_add(p).ok_or(crate::item_budget_origin!())?;
        // Entity table, its two borrowed Vecs, and the kind-owner index.
        state = state
            .checked_add(std::mem::size_of::<(&str, Entity<'_>)>())
            .and_then(|n| n.checked_add(p.checked_mul(std::mem::size_of::<&str>())?))
            .and_then(|n| n.checked_add(m.checked_mul(2 * std::mem::size_of::<(&str, &str)>())?))
            .ok_or(crate::item_budget_origin!())?;
    }
    // One ancestry walk at a time: active/done node sets and entering/leaving
    // stack frames. Each graph edge can enqueue one parent frame.
    state = state
        .checked_add(
            nodes
                .checked_mul(2 * std::mem::size_of::<&str>())
                .ok_or(crate::item_budget_origin!())?,
        )
        .and_then(|n| {
            n.checked_add(
                nodes
                    .checked_add(parents)?
                    .checked_mul(std::mem::size_of::<(&str, bool)>())?,
            )
        })
        .ok_or(crate::item_budget_origin!())?;
    let mut views = 0usize;
    for entry in relations["relations"].as_array().into_iter().flatten() {
        let mappings = entry["source_mappings"].as_array().map_or(0, Vec::len);
        state = state
            .checked_add(std::mem::size_of::<&str>())
            .and_then(|n| {
                n.checked_add(mappings.checked_mul(std::mem::size_of::<(&str, usize)>())?)
            })
            .ok_or(crate::item_budget_origin!())?;
        let count = [
            &entry["domain_type_ids"],
            &entry["range_type_ids"],
            &entry["source_claim_profile"]["assertion_layers"],
            &entry["source_claim_profile"]["object_reference_set"]["member_type_ids"],
        ]
        .iter()
        .try_fold(mappings, |sum, value| {
            sum.checked_add(value.as_array().map_or(0, Vec::len))
        })
        .ok_or(crate::item_budget_origin!())?;
        views = views.max(count);
    }
    state
        .checked_add(
            views
                .checked_mul(std::mem::size_of::<&Value>())
                .ok_or(crate::item_budget_origin!())?,
        )
        .ok_or(crate::item_budget_origin!())
}

fn compile_local_claim_routes(
    entities_raw: &Value,
    relations: &Value,
    base_state: usize,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<BTreeMap<(String, String), LocalClaimRoute>, LocalClaimCompileError> {
    let constructor_state = local_constructor_state(entities_raw, relations)
        .map_err(LocalClaimCompileError::Refusal)?;
    local_claim_state_check(base_state, constructor_state, limits)
        .map_err(LocalClaimCompileError::Refusal)?;
    let entities = compile_entities(entities_raw)?;
    let mut kinds = BTreeMap::new();
    for (id, e) in &entities {
        local_claim_checkpoint(limits, cancelled).map_err(LocalClaimCompileError::Refusal)?;
        if e.profile
            .as_ref()
            .is_some_and(|p| p.get("identity_proposal_adapter").is_some())
            && !local_semantic_eligible(e)
        {
            return Err(local_route_failure("claim-semantic-identity-adapter", id));
        }
        for (graph, kind) in &e.mappings {
            if *graph == "source-claims" && kinds.insert(*kind, *id).is_some() {
                return Err(local_route_failure("claim-source-kind-owner", kind));
            }
        }
    }
    let entries = relations["relations"]
        .as_array()
        .ok_or_else(|| local_route_failure("claim-relation-registry", "relations"))?;
    let mut ids = BTreeSet::new();
    let mut owners = BTreeMap::<&str, usize>::new();
    for (index, entry) in entries.iter().enumerate() {
        let id = field_str(entry, "relation_type_id", "claim-relation-id")?;
        if !ids.insert(id) {
            return Err(local_route_failure("claim-relation-id-duplicate", id));
        }
        for mapping in entry["source_mappings"].as_array().into_iter().flatten() {
            if local_string(mapping, "source_graph") == Some("source-claims")
                && local_string(mapping, "scope") == Some("claim-predicate")
            {
                let predicate =
                    field_str(mapping, "source_predicate_id", "claim-predicate-mapping")?;
                // Same-entry duplicates are caught by its exact mapping count.
                if owners
                    .insert(predicate, index)
                    .is_some_and(|prior| prior != index)
                {
                    // Only predicates with a declared local profile are used.
                    if entries.iter().any(|e| {
                        e.get("source_claim_profile").is_some()
                            && e["source_mappings"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .any(|m| {
                                    local_string(m, "source_graph") == Some("source-claims")
                                        && local_string(m, "scope") == Some("claim-predicate")
                                        && local_string(m, "source_predicate_id") == Some(predicate)
                                })
                    }) {
                        return Err(local_route_failure("claim-predicate-owner", predicate));
                    }
                }
            }
        }
    }
    let mut routes = BTreeMap::new();
    let mut route_bytes = 0usize;
    for (relation_index, entry) in entries.iter().enumerate() {
        local_claim_checkpoint(limits, cancelled).map_err(LocalClaimCompileError::Refusal)?;
        let Some(profile) = entry.get("source_claim_profile") else {
            continue;
        };
        let reader = field_str(profile, "reader", "claim-reader")?;
        let mappings: Vec<_> = entry["source_mappings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| {
                local_string(m, "source_graph") == Some("source-claims")
                    && local_string(m, "scope") == Some("claim-predicate")
            })
            .collect();
        if mappings.len() != 1
            || entry["abstract"] != false
            || local_string(entry, "assertion_mode") != Some("reified-claim")
            || entry["evidence_required"] != true
        {
            return Err(local_route_failure(
                "claim-profile-reified-evidence",
                "source_claim_profile",
            ));
        }
        let predicate = field_str(mappings[0], "source_predicate_id", "claim-predicate")?;
        let domain = field_array_refs(entry, "domain_type_ids", "claim-domain")?;
        let range = field_array_refs(entry, "range_type_ids", "claim-range")?;
        let schemas = profile["schemas"]
            .as_array()
            .ok_or_else(|| local_route_failure("claim-profile-schemas", predicate))?;
        let exact_schema = |version: &str, path: &str| {
            schemas.len() == 1
                && local_string(&schemas[0], "schema_version") == Some(version)
                && local_string(&schemas[0], "schema_ref") == Some(path)
        };
        let layers = local_strings(profile, "assertion_layers");
        if local_document_role(predicate).is_some() || reader == "document-catalogue-temporal-v1" {
            let date = predicate == "document_catalogue_date";
            if local_document_role(predicate).is_none()
                || reader
                    != if date {
                        "document-catalogue-temporal-v1"
                    } else {
                        "identity-relation-v1"
                    }
                || domain != ["tos.entity.document"]
                || range
                    != [if date {
                        "tos.entity.temporal-assertion"
                    } else {
                        "tos.entity.place"
                    }]
                || layers != ["bibliographic_assertion"]
                || !exact_schema("tos_document_catalogue_claim_v1", LOCAL_DOCUMENT)
            {
                return Err(local_route_failure(
                    "claim-document-exact-profile",
                    predicate,
                ));
            }
        }
        let mut semantic_endpoint = false;
        for (is_domain, values) in [(true, &domain), (false, &range)] {
            for type_id in values {
                if local_is_proposal(reader) && is_domain {
                    let (expected_domain, expected_predicate, version, path) =
                        if reader == "identity-transition-v1" {
                            (
                                vec!["tos.entity.identity"],
                                "identity_transition_proposal",
                                "tos_source_identity_transition_claim_v1",
                                "ToS/contracts/source-identity-transition-claim.schema.json",
                            )
                        } else {
                            (
                                vec!["tos.entity.identity", "tos.entity.semantic-object"],
                                "subject_identity_transition_proposal",
                                "tos_subject_identity_transition_claim_v1",
                                "ToS/contracts/subject-identity-transition-claim.schema.json",
                            )
                        };
                    if domain != expected_domain
                        || predicate != expected_predicate
                        || layers != ["identity_assertion"]
                        || !exact_schema(version, path)
                    {
                        return Err(local_route_failure(
                            "claim-proposal-exact-profile",
                            predicate,
                        ));
                    }
                    continue;
                }
                if !local_specific(type_id) {
                    return Err(local_route_failure("claim-specific-endpoint", type_id));
                }
                // Existing ancestry traversal detects missing types and active
                // cycles even when the requested ancestor has already appeared.
                let identity = has_ancestor(&entities, type_id, "tos.entity.identity")?;
                let semantic = has_ancestor(&entities, type_id, "tos.entity.semantic-object")?;
                if local_is_temporal(reader) {
                    let expected = if is_domain {
                        if reader == "document-catalogue-temporal-v1" {
                            "tos.entity.document"
                        } else {
                            "tos.entity.historical-situation"
                        }
                    } else {
                        "tos.entity.temporal-assertion"
                    };
                    if !has_ancestor(&entities, type_id, expected)? {
                        return Err(local_route_failure("claim-temporal-family", type_id));
                    }
                } else if local_is_structured(reader) {
                    if is_domain {
                        if !identity && !semantic {
                            return Err(local_route_failure("claim-value-subject-family", type_id));
                        }
                    } else {
                        let e = &entities[*type_id];
                        if range.len() != 1
                            || *type_id == "tos.entity.literal"
                            || !has_ancestor(&entities, type_id, "tos.entity.literal")?
                            || e.abstract_type
                            || e.role != "literal"
                            || local_string(profile, "value_kind")
                                .and_then(|kind| kinds.get(kind).copied())
                                != Some(*type_id)
                        {
                            return Err(local_route_failure("claim-value-concrete-range", type_id));
                        }
                    }
                } else if reader == "identity-relation-v1" {
                    if !identity {
                        return Err(local_route_failure(
                            "claim-identity-relation-family",
                            type_id,
                        ));
                    }
                } else {
                    semantic_endpoint |= semantic;
                    if !identity && !semantic {
                        return Err(local_route_failure(
                            "claim-semantic-relation-family",
                            type_id,
                        ));
                    }
                }
            }
        }
        if reader == "semantic-relation-v1" && !semantic_endpoint {
            return Err(local_route_failure(
                "claim-semantic-endpoint-required",
                predicate,
            ));
        }
        if reader == "structured-reference-value-v1" {
            let members = &profile["object_reference_set"];
            let types = field_array_refs(members, "member_type_ids", "claim-member-types")?;
            if (members.get("basis_adapter").is_some()
                || local_string(profile, "value_kind") == Some("collection-member-order"))
                && (local_string(members, "basis_adapter")
                    != Some("collection-membership-versions-v1")
                    || local_string(members, "structure_adapter") != Some("scoped-members-v1")
                    || local_string(profile, "value_kind") != Some("collection-member-order")
                    || predicate != "collection_member_order"
                    || domain != ["tos.entity.collection"]
                    || types != ["tos.entity.work"])
            {
                return Err(local_route_failure(
                    "claim-collection-order-exact-profile",
                    predicate,
                ));
            }
            if members["min_items"].as_u64() > members["max_items"].as_u64() {
                return Err(local_route_failure("claim-member-bounds", predicate));
            }
            for id in types {
                if !local_specific(&id)
                    || (!has_ancestor(&entities, &id, "tos.entity.identity")?
                        && !has_ancestor(&entities, &id, "tos.entity.semantic-object")?)
                {
                    return Err(local_route_failure("claim-member-specific-family", &id));
                }
            }
        }
        for (schema_index, schema) in schemas.iter().enumerate() {
            if routes.len() >= MAX_COMPILED_ROUTES {
                return Err(LocalClaimCompileError::Refusal(
                    crate::item_rules::ItemRefusal::BudgetCheck {
                        check: "local Claim compiled route count",
                        used: (routes.len() as u64).checked_add(1),
                        limit: Some(MAX_COMPILED_ROUTES as u64),
                    },
                ));
            }
            let version = field_str(schema, "schema_version", "claim-schema-version")?;
            let size = std::mem::size_of::<((String, String), LocalClaimRoute)>()
                .checked_add(predicate.len())
                .and_then(|n| n.checked_add(version.len()))
                .ok_or(LocalClaimCompileError::Refusal(
                    crate::item_budget_origin!(),
                ))?;
            let next_route_bytes = route_bytes.checked_add(size);
            route_bytes = next_route_bytes.ok_or(LocalClaimCompileError::Refusal(
                crate::item_budget_origin!(),
            ))?;
            local_claim_state_check(
                base_state,
                constructor_state.checked_add(route_bytes).ok_or(
                    LocalClaimCompileError::Refusal(crate::item_budget_origin!()),
                )?,
                limits,
            )
            .map_err(LocalClaimCompileError::Refusal)?;
            if routes
                .insert(
                    (predicate.into(), version.into()),
                    LocalClaimRoute {
                        relation: relation_index,
                        schema: schema_index,
                    },
                )
                .is_some()
            {
                return Err(local_route_failure(
                    "claim-schema-route-duplicate",
                    predicate,
                ));
            }
        }
    }
    Ok(routes)
}

/// Read-only Work/Expression descriptors from the same compiled Claim routes.
/// Registry schema verdicts belong to the caller's bounded source-cut worker.
pub fn work_expression_source_descriptors(
    entities_raw: &[u8],
    relations_raw: &[u8],
    corpus_raw: &[u8],
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Value, crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    local_claim_checkpoint(limits, cancelled)?;
    let mut state = 0usize;
    let mut decode = |raw: &[u8]| {
        if raw.len() > limits.max_member_bytes {
            return Err(crate::item_budget_origin!());
        }
        let (value, used) = crate::record_biblio_cut::bounded_decoded_state(
            raw,
            tos_foundation::JsonLimits::default(),
            limits
                .max_state_bytes
                .checked_sub(state)
                .ok_or(crate::item_budget_origin!())?,
            limits.deadline,
            cancelled,
        )?;
        state = state
            .checked_add(used)
            .ok_or(crate::item_budget_origin!())?;
        Ok(value)
    };
    let total = entities_raw
        .len()
        .checked_add(relations_raw.len())
        .and_then(|n| n.checked_add(corpus_raw.len()))
        .ok_or(crate::item_budget_origin!())?;
    if total as u64 > limits.max_total_bytes {
        return Err(crate::item_budget_origin!());
    }
    let entities = decode(entities_raw)?;
    let relations = decode(relations_raw)?;
    let corpus = decode(corpus_raw)?;
    let routes = compile_local_claim_routes(&entities, &relations, state, limits, cancelled)
        .map_err(|error| match error {
            LocalClaimCompileError::Refusal(reason) => reason,
            LocalClaimCompileError::Rule(code, detail) => {
                ItemRefusal::Source(format!("{code}: {detail}"))
            }
        })?;
    state = state
        .checked_add(local_route_state(&routes)?)
        .ok_or(crate::item_budget_origin!())?;
    local_claim_state_check(state, 0, limits)?;
    let route = routes
        .get(&(
            "has_expression".into(),
            "tos_source_relation_claim_v1".into(),
        ))
        .ok_or_else(|| {
            ItemRefusal::Unsupported("Work/Expression Claim profile route absent".into())
        })?;
    let relation = &relations["relations"][route.relation];
    let profile = &relation["source_claim_profile"];
    let schema = &profile["schemas"][route.schema];
    let version = corpus["properties"]["schema_version"]["const"]
        .as_str()
        .ok_or_else(|| ItemRefusal::Source("corpus schema version declaration absent".into()))?;
    let mut result = serde_json::Map::new();
    for kind in ["work", "expression"] {
        local_claim_checkpoint(limits, cancelled)?;
        let entry = entities["types"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| {
                entry["source_mappings"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|mapping| {
                        local_string(mapping, "source_graph") == Some("source-claims")
                            && local_string(mapping, "source_kind_id") == Some(kind)
                    })
            })
            .ok_or_else(|| ItemRefusal::Source(format!("{kind} source mapping absent")))?;
        result.insert(
            kind.into(),
            json!({"type_id":entry["type_id"],"record_type":kind,
            "schema_ref":"ToS/contracts/corpus-record.schema.json","schema_version":version,
            "source_basename":format!("{kind}.json")}),
        );
    }
    result.insert(
        "has_expression".into(),
        json!({"relation_type_id":relation["relation_type_id"],
        "predicate":"has_expression","reader":profile["reader"],"schema_ref":schema["schema_ref"],
        "schema_version":schema["schema_version"],"assertion_layers":profile["assertion_layers"],
        "source_basename":"source-claims.jsonl"}),
    );
    let result = Value::Object(result);
    crate::record_biblio_cut::decoded_wire_size(
        &result,
        limits
            .max_state_bytes
            .checked_sub(state)
            .ok_or(crate::item_budget_origin!())?,
    )?;
    local_claim_checkpoint(limits, cancelled)?;
    Ok(result)
}

fn validate_source_claim_local_core(
    source: &dyn LocalClaimCurrentSource,
    claim: &Value,
    selected_claim_raw: &[u8],
    retained_header_state_bytes: usize,
    worker: &mut impl LocalClaimSchemaWorker,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
    bytes: &mut u64,
    report: &mut LocalClaimReportState,
) -> Result<(), crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    let mut inputs = LocalClaimInputs::new();
    for path in [
        LOCAL_CLAIM_REGISTRY,
        LOCAL_CLAIM_CONTRACT,
        ENTITY_REGISTRY,
        ENTITY_CONTRACT,
    ] {
        local_claim_input(
            source,
            path,
            claim,
            0,
            limits,
            cancelled,
            bytes,
            &mut inputs,
            report,
        )?;
    }
    // Constructor uses no FormatChecker. Current exact contracts are local,
    // format-free schemas. Refuse a changed contract requiring another profile
    // rather than silently making source registry validation stricter.
    for path in [LOCAL_CLAIM_CONTRACT, ENTITY_CONTRACT] {
        local_registry_schema(&inputs[path].0)?;
        if worker.contract_digest(path) != report.dependency_digests.get(path).copied() {
            return Err(ItemRefusal::Source(
                "local Claim registry contract worker digest differs".into(),
            ));
        }
    }
    for (path, schema) in [
        (LOCAL_CLAIM_REGISTRY, LOCAL_CLAIM_CONTRACT),
        (ENTITY_REGISTRY, ENTITY_CONTRACT),
    ] {
        if !worker.check_reusing_scalar(
            path,
            &inputs[path].1,
            schema,
            limits.deadline,
            cancelled,
        )? {
            local_claim_issue(report, limits, "claim-registry-schema", path)?;
        }
    }
    if report.issues.is_empty() {
        match compile_local_claim_routes(
            &inputs[ENTITY_REGISTRY].0,
            &inputs[LOCAL_CLAIM_REGISTRY].0,
            report.logical_state,
            limits,
            cancelled,
        ) {
            Ok(routes) => {
                let result = validate_local_claim_shape(
                    source,
                    claim,
                    selected_claim_raw,
                    &routes,
                    worker,
                    limits,
                    cancelled,
                    bytes,
                    &mut inputs,
                    report,
                );
                report.workspace = 0;
                result?;
            }
            Err(LocalClaimCompileError::Rule(code, detail)) => {
                local_claim_issue(report, limits, code, &detail)?
            }
            Err(LocalClaimCompileError::Refusal(error)) => return Err(error),
        }
    }
    local_claim_checkpoint(limits, cancelled)?;
    drop(inputs);
    let mut retained = retained_header_state_bytes;
    for path in report.dependency_digests.keys() {
        retained = retained
            .checked_add(std::mem::size_of::<(String, Digest256)>() + path.len())
            .ok_or(crate::item_budget_origin!())?;
    }
    for path in &report.dependency_order {
        retained = retained
            .checked_add(std::mem::size_of::<String>() + path.len())
            .ok_or(crate::item_budget_origin!())?;
    }
    for issue in &report.issues {
        retained = retained
            .checked_add(
                std::mem::size_of::<crate::relation_rules::RelationIssue>() + issue.location.len(),
            )
            .ok_or(crate::item_budget_origin!())?;
    }
    report.logical_state = retained;
    Ok(())
}

fn local_claim_resources(
    source: &dyn LocalClaimCurrentSource,
    paths: &[String],
    claim: &Value,
    route_state: usize,
    worker: &impl LocalClaimSchemaWorker,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
    bytes: &mut u64,
    inputs: &mut BTreeMap<String, (std::sync::Arc<Value>, Vec<u8>)>,
    report: &mut LocalClaimReportState,
) -> Result<(), crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    let mut uris = BTreeSet::new();
    let path_state = paths
        .iter()
        .try_fold(std::mem::size_of::<Vec<String>>(), |n, path| {
            n.checked_add(std::mem::size_of::<String>() + path.len())
        })
        .ok_or(crate::item_budget_origin!())?;
    let mut uri_state = std::mem::size_of_val(&uris);
    for path in paths {
        if !is_contract_path(path) {
            return Err(ItemRefusal::Unsupported(format!(
                "local Claim non-contract schema path: {path}"
            )));
        }

        let extra = route_state
            .checked_add(path_state)
            .and_then(|n| n.checked_add(uri_state))
            .ok_or(crate::item_budget_origin!())?;
        local_claim_input(
            source, path, claim, extra, limits, cancelled, bytes, inputs, report,
        )?;
        let (schema, raw) = &inputs[path];
        let uri = schema_uri(path, schema)
            .map_err(|e| ItemRefusal::Unsupported(format!("local Claim schema identity: {e:?}")))?;
        if uri != format!("https://tree-of-sophia.local/{path}")
            && uri != format!("https://treeofsophia.local/{path}")
        {
            return Err(ItemRefusal::Unsupported(format!(
                "local Claim schema identity differs from owner path: {path}"
            )));
        }
        if worker.contract_digest(path) != Some(Digest256::of_bytes(raw)) {
            return Err(ItemRefusal::Source(format!(
                "local Claim schema worker digest differs: {path}"
            )));
        }
        uri_state = uri_state
            .checked_add(std::mem::size_of::<String>() + uri.len())
            .ok_or(crate::item_budget_origin!())?;
        local_claim_state_check(
            report.logical_state,
            route_state
                .checked_add(path_state)
                .and_then(|n| n.checked_add(uri_state))
                .ok_or(crate::item_budget_origin!())?,
            limits,
        )?;
        if !uris.insert(uri) {
            return Err(ItemRefusal::Unsupported(
                "duplicate local Claim schema resource identity".into(),
            ));
        }
    }
    // from_cut has already prepared these exact raw resources with this same
    // backend/profile. Recheck selected fixity above and local dependency scope
    // below instead of cloning and parsing another identical schema closure.
    for path in paths {
        local_claim_checkpoint(limits, cancelled)?;
        let schema = &inputs[path].0;
        local_schema_walk(schema, true, &mut |key, value| {
            if matches!(key, "$ref" | "$dynamicRef") {
                let reference = value.as_str().ok_or_else(|| {
                    ItemRefusal::Unsupported("local Claim schema ref representation".into())
                })?;
                let base = reference.split('#').next().unwrap_or("");
                if !base.is_empty() && !uris.contains(base) {
                    return Err(ItemRefusal::Unsupported(format!(
                        "local Claim undeclared schema dependency: {path}: {reference}"
                    )));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_local_claim_shape(
    source: &dyn LocalClaimCurrentSource,
    claim: &Value,
    raw: &[u8],
    routes: &BTreeMap<(String, String), LocalClaimRoute>,
    worker: &mut impl LocalClaimSchemaWorker,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
    bytes: &mut u64,
    inputs: &mut BTreeMap<String, (std::sync::Arc<Value>, Vec<u8>)>,
    report: &mut LocalClaimReportState,
) -> Result<(), crate::item_rules::ItemRefusal> {
    use crate::item_rules::ItemRefusal;
    let route_state = local_route_state(routes)?;
    report.workspace = route_state;
    local_claim_state_check(report.logical_state, route_state, limits)?;
    if !claim.is_object()
        || !matches!(
            local_string(claim, "visibility"),
            Some("public" | "public_metadata_only")
        )
    {
        return local_claim_issue(report, limits, "claim-public-visibility", "claim");
    }
    let (Some(predicate), Some(version)) = (
        local_string(claim, "predicate"),
        local_string(claim, "schema_version"),
    ) else {
        return local_claim_issue(report, limits, "claim-string-predicate-version", "claim");
    };
    let Some(route) = routes
        .iter()
        .find(|((p, v), _)| p == predicate && v == version)
        .map(|(_, route)| route)
    else {
        return local_claim_issue(
            report,
            limits,
            "claim-unrecognized-predicate-version",
            predicate,
        );
    };
    // The immutable input map owns each registry tree once. This Arc only
    // keeps that same tree borrowed while more selected dependencies are read.
    let registry = std::sync::Arc::clone(&inputs[LOCAL_CLAIM_REGISTRY].0);
    let profile = &registry["relations"][route.relation]["source_claim_profile"];
    let schema = &profile["schemas"][route.schema];
    let reader = local_string(profile, "reader")
        .ok_or_else(|| ItemRefusal::Unsupported("local Claim compiled reader".into()))?;
    let value_route = local_is_temporal(reader) || local_is_structured(reader);
    if local_string(claim, "claim_type") != Some("relation")
        || local_string(claim, "subject_ref").is_none()
        || !(if value_route {
            claim["object"].is_object()
        } else {
            claim["object"].is_string()
        })
        || local_string(claim, "assertion_layer")
            .is_none_or(|layer| !local_strings(profile, "assertion_layers").contains(&layer))
        || claim["claim_id"] == claim["subject_ref"]
        || claim["claim_id"] == claim["object"]
    {
        return local_claim_issue(report, limits, "claim-identity-endpoint-layer", predicate);
    }
    let selected = local_string(schema, "schema_ref")
        .ok_or_else(|| ItemRefusal::Unsupported("local Claim selected schema".into()))?;
    let scoped = local_string(&profile["object_reference_set"], "structure_adapter")
        == Some("scoped-members-v1");
    let mut paths: Vec<String> = [
        "ToS/contracts/claim-packet.schema.json",
        "ToS/contracts/knowledge-assessment.schema.json",
        LOCAL_CLAIM_BASE,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if local_is_temporal(reader) {
        paths.push(LOCAL_TEMPORAL.into());
    }
    if local_is_structured(reader) {
        paths.extend([CORPUS_CONTRACT.into(), LOCAL_STRUCTURED.into()]);
    }
    if scoped {
        paths.push(LOCAL_MEMBERS.into());
    }
    paths.extend(
        local_strings(schema, "schema_dependencies")
            .into_iter()
            .map(str::to_owned),
    );
    paths.push(selected.into());
    let mut unique = BTreeSet::new();
    paths.retain(|path| unique.insert(path.clone()));
    let paths_state = paths
        .iter()
        .try_fold(std::mem::size_of_val(&paths), |n, path| {
            n.checked_add(std::mem::size_of::<String>() + path.len())
        })
        .ok_or(crate::item_budget_origin!())?;
    let unique_state = unique
        .iter()
        .try_fold(std::mem::size_of_val(&unique), |n, path| {
            n.checked_add(std::mem::size_of::<String>() + path.len())
        })
        .ok_or(crate::item_budget_origin!())?;
    local_claim_state_check(
        report.logical_state,
        route_state
            .checked_add(paths_state)
            .and_then(|n| n.checked_add(unique_state))
            .ok_or(crate::item_budget_origin!())?,
        limits,
    )?;
    drop(unique);
    local_claim_resources(
        source,
        &paths,
        claim,
        route_state,
        worker,
        limits,
        cancelled,
        bytes,
        inputs,
        report,
    )?;
    drop(paths);
    for contract in [selected, LOCAL_CLAIM_BASE] {
        if !worker.check_reusing_scalar(
            "selected Claim",
            raw,
            contract,
            limits.deadline,
            cancelled,
        )? {
            local_claim_issue(
                report,
                limits,
                "claim-exact-schema-or-shared-record",
                contract,
            )?;
        }
    }
    if !report.issues.is_empty() {
        return Ok(());
    }
    crate::record_biblio_cut::decoded_wire_size(
        &claim["object"],
        limits
            .max_state_bytes
            .checked_sub(report.logical_state)
            .and_then(|n| n.checked_sub(route_state))
            .ok_or(crate::item_budget_origin!())?,
    )?;
    let object_raw = serde_json::to_vec(&claim["object"])
        .map_err(|_| ItemRefusal::Unsupported("local Claim object serialization".into()))?;
    report.workspace = route_state
        .checked_add(std::mem::size_of::<Vec<u8>>() + object_raw.len())
        .ok_or(crate::item_budget_origin!())?;
    local_claim_state_check(report.logical_state, report.workspace, limits)?;
    if local_is_temporal(reader) {
        let root = if reader == "document-catalogue-temporal-v1" {
            format!("{LOCAL_DOCUMENT}#/$defs/documentDate")
        } else {
            format!("{LOCAL_TEMPORAL}#/$defs/historicalDate")
        };
        if !worker.check_reusing_scalar(
            "selected Claim/object",
            &object_raw,
            &root,
            limits.deadline,
            cancelled,
        )? {
            local_claim_issue(report, limits, "claim-shared-temporal-value", &root)?;
        }
    }
    if local_is_structured(reader) {
        if !worker.check_reusing_scalar(
            "selected Claim/object",
            &object_raw,
            LOCAL_STRUCTURED,
            limits.deadline,
            cancelled,
        )? || claim["object"]["kind"] != profile["value_kind"]
        {
            local_claim_issue(report, limits, "claim-shared-value-kind", predicate)?;
        }
    }
    if !report.issues.is_empty() {
        return Ok(());
    }
    let qualifiers = &claim["qualifiers"];
    if qualifiers["display_fields"].is_object()
        && local_string(&qualifiers["display_fields"], "schema_version")
            == Some("tos_claim_display_fields_v1")
    {
        let display_paths = vec![CORPUS_CONTRACT.into(), LOCAL_DISPLAY.into()];
        local_claim_resources(
            source,
            &display_paths,
            claim,
            report.workspace,
            worker,
            limits,
            cancelled,
            bytes,
            inputs,
            report,
        )?;
        let display_path_state = display_paths
            .iter()
            .try_fold(std::mem::size_of_val(&display_paths), |n, path| {
                n.checked_add(std::mem::size_of::<String>() + path.len())
            })
            .ok_or(crate::item_budget_origin!())?;
        report.workspace = report
            .workspace
            .checked_add(display_path_state)
            .ok_or(crate::item_budget_origin!())?;
        crate::record_biblio_cut::decoded_wire_size(
            qualifiers,
            limits
                .max_state_bytes
                .checked_sub(report.logical_state)
                .and_then(|n| n.checked_sub(report.workspace))
                .ok_or(crate::item_budget_origin!())?,
        )?;
        let qualifier_raw = serde_json::to_vec(qualifiers)
            .map_err(|_| ItemRefusal::Unsupported("local Claim qualifiers serialization".into()))?;
        report.workspace = route_state
            .checked_add(std::mem::size_of::<Vec<u8>>() + object_raw.len())
            .and_then(|n| n.checked_add(display_path_state))
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<u8>>() + qualifier_raw.len()))
            .ok_or(crate::item_budget_origin!())?;
        local_claim_state_check(report.logical_state, report.workspace, limits)?;
        if !worker.check_reusing_scalar(
            "selected Claim/qualifiers",
            &qualifier_raw,
            LOCAL_DISPLAY,
            limits.deadline,
            cancelled,
        )? {
            local_claim_issue(report, limits, "claim-display-fields", predicate)?;
        }
    }
    report.workspace = route_state
        .checked_add(std::mem::size_of::<Vec<u8>>() + object_raw.len())
        .ok_or(crate::item_budget_origin!())?;
    if let Some(role) = local_document_role(predicate) {
        let attribution = &qualifiers["catalogue_attribution"];
        if local_string(attribution, "field_role") != Some(role)
            || !claim["evidence_refs"]
                .as_array()
                .is_some_and(|refs| refs.contains(&attribution["evidence_ref"]))
        {
            local_claim_issue(report, limits, "claim-document-field-evidence", predicate)?;
        }
        if predicate == "document_catalogue_date"
            && attribution["source_wording"] != claim["object"]["source_wording"]
        {
            local_claim_issue(report, limits, "claim-document-source-wording", predicate)?;
        }
    }
    if local_is_proposal(reader) {
        let value = &claim["object"];
        let left = value["predecessors"].as_array().map_or(0, Vec::len);
        let right = value["successors"].as_array().map_or(0, Vec::len);
        let members = value["members"].as_array().map_or(0, Vec::len);
        let edges = value["mapping"].as_array().map_or(0, Vec::len);
        let participants = left
            .checked_add(right)
            .ok_or(crate::item_budget_origin!())?;
        let scratch = participants
            .checked_mul(std::mem::size_of::<&str>())
            .and_then(|n| n.checked_add(participants.checked_mul(std::mem::size_of::<&str>())?))
            .and_then(|n| n.checked_add(members.checked_mul(std::mem::size_of::<&str>())?))
            .and_then(|n| {
                n.checked_add(
                    left.checked_mul(right)?
                        .checked_add(edges)?
                        .checked_mul(std::mem::size_of::<(&str, &str)>())?,
                )
            })
            .ok_or(crate::item_budget_origin!())?;
        local_claim_state_check(
            report.logical_state,
            report
                .workspace
                .checked_add(scratch)
                .ok_or(crate::item_budget_origin!())?,
            limits,
        )?;
        if let Err(code) = local_proposal_participants(claim) {
            local_claim_issue(report, limits, code, predicate)?;
        }
    } else if reader == "structured-reference-value-v1" {
        let constraint = &profile["object_reference_set"];
        let members = claim["object"]["members"].as_array();
        let scratch = members
            .map_or(0, Vec::len)
            .checked_mul(std::mem::size_of::<&str>() + std::mem::size_of::<&str>())
            .ok_or(crate::item_budget_origin!())?;
        local_claim_state_check(
            report.logical_state,
            report
                .workspace
                .checked_add(scratch)
                .ok_or(crate::item_budget_origin!())?,
            limits,
        )?;
        let valid = members.is_some_and(|members| {
            let ids: Vec<_> = members.iter().filter_map(Value::as_str).collect();
            constraint["min_items"]
                .as_u64()
                .zip(constraint["max_items"].as_u64())
                .is_some_and(|(min, max)| {
                    min <= members.len() as u64 && members.len() as u64 <= max
                })
                && ids.len() == members.len()
                && ids.iter().all(|id| local_tos_id(id, false))
                && ids.iter().copied().collect::<BTreeSet<_>>().len() == members.len()
                && !members.contains(&claim["claim_id"])
                && (constraint["subject_is_member"] != true
                    || members.contains(&claim["subject_ref"]))
        });
        if !valid {
            local_claim_issue(report, limits, "claim-reference-value-members", predicate)?;
        }
    }
    if scoped {
        if !worker.check_reusing_scalar(
            "selected Claim/object",
            &object_raw,
            LOCAL_MEMBERS,
            limits.deadline,
            cancelled,
        )? {
            local_claim_issue(report, limits, "claim-shared-member-structure", predicate)?;
        } else {
            let mut scoped_limits = limits;
            scoped_limits.max_state_bytes = limits
                .max_state_bytes
                .checked_sub(report.logical_state)
                .and_then(|n| n.checked_sub(report.workspace))
                .ok_or(crate::item_budget_origin!())?;
            if let Err(code) = local_scoped_member_structure(claim, scoped_limits, cancelled)? {
                local_claim_issue(report, limits, code, predicate)?;
            }
        }
    }
    report.workspace = route_state
        .checked_add(std::mem::size_of::<Vec<u8>>() + object_raw.len())
        .ok_or(crate::item_budget_origin!())?;
    local_claim_state_check(report.logical_state, report.workspace, limits)?;
    Ok(())
}

fn local_tos_id(id: &str, claim: bool) -> bool {
    if claim {
        return valid_record_id(id, "tos.claim.");
    }
    let Some(rest) = id.strip_prefix("tos.") else {
        return false;
    };
    let Some((kind, _)) = rest.split_once('.') else {
        return false;
    };
    let mut chars = kind.bytes();
    matches!(chars.next(), Some(b'a'..=b'z'))
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && valid_record_id(id, &format!("tos.{kind}."))
}
fn local_exact_ref(value: &Value, claim: bool) -> bool {
    value.as_object().is_some_and(|o| {
        o.len() == 3
            && o.contains_key("id")
            && o.contains_key("version")
            && o.contains_key("digest")
    }) && local_string(value, "id").is_some_and(|id| local_tos_id(id, claim))
        && value["version"]
            .as_u64()
            .is_some_and(|n| (1..=9_007_199_254_740_991).contains(&n))
        && local_string(value, "digest")
            .and_then(|s| s.strip_prefix("sha256:"))
            .is_some_and(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            })
}
fn local_proposal_participants(claim: &Value) -> Result<(), &'static str> {
    let object = &claim["object"];
    let left = object["predecessors"]
        .as_array()
        .ok_or("claim-proposal-participant-shape")?;
    let right = object["successors"]
        .as_array()
        .ok_or("claim-proposal-participant-shape")?;
    let members = object["members"]
        .as_array()
        .ok_or("claim-proposal-participant-shape")?;
    let mapping = object["mapping"]
        .as_array()
        .ok_or("claim-proposal-participant-shape")?;
    if [left, right]
        .iter()
        .any(|r| !(1..=8).contains(&r.len()) || r.iter().any(|v| !local_exact_ref(v, false)))
        || !(3..=9).contains(&members.len())
        || members.iter().any(|v| !v.is_string())
        || !(2..=8).contains(&mapping.len())
        || mapping.iter().any(|edge| {
            !edge.as_object().is_some_and(|o| {
                o.len() == 2
                    && o.get("predecessor").is_some_and(Value::is_string)
                    && o.get("successor").is_some_and(Value::is_string)
            })
        })
    {
        return Err("claim-proposal-participant-shape");
    }
    let previous = object
        .get("supersedes_proposal")
        .ok_or("claim-proposal-explicit-predecessor")?;
    let unresolved = object["unresolved_links"]
        .as_array()
        .ok_or("claim-proposal-related-claims")?;
    if unresolved.len() > 32
        || unresolved
            .iter()
            .any(|v| !v.is_object() || !local_exact_ref(&v["claim"], true))
        || (!previous.is_null() && !local_exact_ref(previous, true))
    {
        return Err("claim-proposal-related-claims");
    }
    let participants: Vec<_> = left
        .iter()
        .chain(right)
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    let distinct: BTreeSet<_> = participants.iter().copied().collect();
    let declared: BTreeSet<_> = members.iter().map(|v| v.as_str().unwrap()).collect();
    if distinct.len() != participants.len()
        || members.len() != participants.len()
        || declared != distinct
        || !left.iter().any(|v| v["id"] == claim["subject_ref"])
        || participants
            .iter()
            .any(|id| Some(*id) == local_string(claim, "claim_id"))
    {
        return Err("claim-proposal-frozen-union");
    }
    if !matches!(
        (local_string(object, "operation"), left.len(), right.len()),
        (Some("merge"), 2..=8, 1) | (Some("split"), 1, 2..=8)
    ) {
        return Err("claim-proposal-topology");
    }
    let expected: BTreeSet<_> = left
        .iter()
        .flat_map(|old| {
            right
                .iter()
                .map(move |new| (old["id"].as_str().unwrap(), new["id"].as_str().unwrap()))
        })
        .collect();
    let actual: BTreeSet<_> = mapping
        .iter()
        .map(|e| {
            (
                e["predecessor"].as_str().unwrap(),
                e["successor"].as_str().unwrap(),
            )
        })
        .collect();
    if actual != expected || mapping.len() != expected.len() {
        return Err("claim-proposal-complete-mapping");
    }
    if claim.get("supersedes_claim_ref").unwrap_or(&Value::Null)
        != previous.get("id").unwrap_or(&Value::Null)
    {
        return Err("claim-proposal-succession-navigation");
    }
    if (!previous.is_null() && previous["id"] == claim["claim_id"])
        || unresolved
            .iter()
            .any(|v| v["claim"]["id"] == claim["claim_id"])
    {
        return Err("claim-proposal-self-related-claim");
    }
    Ok(())
}
fn local_scoped_member_structure(
    claim: &Value,
    limits: crate::item_rules::ItemLimits,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Result<(), &'static str>, crate::item_rules::ItemRefusal> {
    let object = &claim["object"];
    let count = object["members"].as_array().map_or(0, Vec::len);
    let edges_count = object["ordering"]["precedes"]
        .as_array()
        .map_or(0, Vec::len);
    let versions = object["membership_versions"].as_array().map_or(0, Vec::len);
    let member_slots = std::mem::size_of::<&str>()
        + std::mem::size_of::<(&str, BTreeSet<&str>)>()
        + std::mem::size_of::<(&str, usize)>()
        + std::mem::size_of::<&str>();
    let scratch = count
        .checked_mul(member_slots)
        .and_then(|n| n.checked_add(edges_count.checked_mul(std::mem::size_of::<&str>())?))
        .and_then(|n| n.checked_add(versions.checked_mul(std::mem::size_of::<&str>())?))
        .ok_or(crate::item_budget_origin!())?;
    if scratch > limits.max_state_bytes {
        return Err(crate::item_rules::ItemRefusal::BudgetCheck {
            check: "local Claim scoped member logical indexes",
            used: Some(scratch as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    let members: BTreeSet<_> = local_strings(object, "members").into_iter().collect();
    if local_string(claim, "subject_ref").is_some_and(|s| members.contains(s)) {
        return Ok(Err("claim-structure-subject-is-member"));
    }
    let edges = object["ordering"]["precedes"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if members.len() > 128 || edges.len() > 8128 {
        return Err(crate::item_budget_origin!());
    }
    let mode = local_string(&object["ordering"], "mode");
    if mode == Some("unordered") && !edges.is_empty() {
        return Ok(Err("claim-structure-unordered-edges"));
    }
    let mut outgoing: BTreeMap<_, BTreeSet<&str>> =
        members.iter().map(|m| (*m, BTreeSet::new())).collect();
    let mut degree: BTreeMap<_, usize> = members.iter().map(|m| (*m, 0)).collect();
    for edge in edges {
        local_claim_checkpoint(limits, cancelled)?;
        let Some((before, after)) = edge
            .as_array()
            .filter(|e| e.len() == 2)
            .and_then(|e| e[0].as_str().zip(e[1].as_str()))
        else {
            return Ok(Err("claim-structure-edge-shape"));
        };
        if !members.contains(before) || !members.contains(after) {
            return Ok(Err("claim-structure-edge-outside-members"));
        }
        // Schema uniqueItems already rejects duplicate edges; retaining the
        // set mirrors Python's degree accounting after exact schema success.
        if outgoing.get_mut(before).unwrap().insert(after) {
            *degree.get_mut(after).unwrap() += 1;
        }
    }
    let mut ready: Vec<_> = degree
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(m, _)| *m)
        .collect();
    let mut visited = 0;
    while !ready.is_empty() {
        local_claim_checkpoint(limits, cancelled)?;
        if mode == Some("total") && ready.len() != 1 {
            return Ok(Err("claim-structure-total-incomparable"));
        }
        let member = ready.pop().unwrap();
        visited += 1;
        for next in &outgoing[member] {
            let n = degree.get_mut(next).unwrap();
            *n -= 1;
            if *n == 0 {
                ready.push(*next);
            }
        }
    }
    if visited != members.len() {
        return Ok(Err("claim-structure-cycle"));
    }
    if local_string(object, "kind") == Some("collection-member-order") {
        let collection = &object["collection_version"];
        let versions = object["membership_versions"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let unique: BTreeSet<_> = versions
            .iter()
            .filter_map(|v| local_string(v, "id"))
            .collect();
        if !local_exact_ref(collection, false)
            || collection["id"] != claim["subject_ref"]
            || !local_string(collection, "id").is_some_and(|id| id.starts_with("tos.collection."))
            || versions.len() != members.len()
            || unique.len() != versions.len()
            || versions.iter().any(|v| !local_exact_ref(v, true))
        {
            return Ok(Err("claim-collection-order-exact-basis"));
        }
    }
    Ok(Ok(()))
}
