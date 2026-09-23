//! Source-declared record profile checks for a future full auditor.
//!
//! This module reports observations and ID owners; it never returns an
//! admission result. The caller must prove complete member enumeration,
//! native-adapter coverage, reference closure, history and catalog parity.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tos_foundation::Digest256;

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

/// One exact verdict returned by a separately bounded schema worker. The
/// auditor owns worker execution, timeout, process identity and complete
/// response accounting; a value alone is never proof of those properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundedSchemaVerdict {
    pub instance_sha256: String,
    pub schema_set_digest: String,
    pub format_profile_id: String,
    pub root_uri: String,
    pub worker_protocol_id: String,
    pub worker_binary_digest: String,
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
    pub schema_set_digest: String,
    pub format_profile_id: String,
    pub instance_sha256: String,
}

/// Exact source-owned resource bytes. The URI is read from `$id`, not chosen by
/// an instance record. Neither this list nor registry data can supply code.
#[derive(Debug, Clone)]
pub struct RecordSchema<'a> {
    pub path: &'a str,
    pub raw: &'a [u8],
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
    },
    IdOwner {
        id: String,
        path: String,
        version: u64,
        raw_sha256: String,
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

/// A sink may spill sorted ID owners and reference queries. Its error must
/// prevent the caller from treating a truncated output as complete.
pub trait RecordSink {
    fn push(&mut self, observation: RecordObservation) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFamilyReport {
    pub registry_version: String,
    pub registry_sha256: String,
    pub inspected_members: u64,
    pub issue_count: u64,
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

struct Entity {
    parents: Vec<String>,
    role: String,
    abstract_type: bool,
    mappings: Vec<(String, String)>,
    profile: Option<Value>,
}

pub struct RecordFamily {
    registry_sha256: String,
    registry_version: String,
    resources: BTreeMap<String, (String, Vec<u8>)>,
    profiles: BTreeMap<String, Profile>,
    compiled_routes: BTreeMap<(String, String), SchemaBackendProbe>,
    format_profile: FormatProfile,
    inspected_members: u64,
    issue_count: u64,
    seen_profiles: BTreeSet<String>,
    native_packet_count: usize,
    native_packet_bytes: usize,
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
    pub schema_set_digest: String,
    pub format_profile_id: String,
    pub instance_sha256: String,
}

impl RecordFamily {
    /// Shadow-only constructor. Its inline schema probe has no CPU deadline
    /// and must not be used to complete an audit rule.
    pub fn new(
        registry_raw: &[u8],
        contract_raw: &[u8],
        schemas: impl IntoIterator<Item = RecordSchema<'_>>,
        format_profile: FormatProfile,
    ) -> Result<Self, RecordRuleError> {
        Self::new_inner(registry_raw, contract_raw, schemas, format_profile, None)
    }

    /// Audit-consumable constructor only when the caller has checked that the
    /// exact registry schema verdict came from a deadline-bounded worker.
    pub(crate) fn new_with_bounded_registry(
        registry_raw: &[u8],
        contract_raw: &[u8],
        schemas: impl IntoIterator<Item = RecordSchema<'_>>,
        format_profile: FormatProfile,
        registry_evidence: &BoundedSchemaVerdict,
    ) -> Result<Self, RecordRuleError> {
        Self::new_inner(
            registry_raw,
            contract_raw,
            schemas,
            format_profile,
            Some(registry_evidence),
        )
    }

    fn new_inner(
        registry_raw: &[u8],
        contract_raw: &[u8],
        schemas: impl IntoIterator<Item = RecordSchema<'_>>,
        format_profile: FormatProfile,
        registry_evidence: Option<&BoundedSchemaVerdict>,
    ) -> Result<Self, RecordRuleError> {
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
            inspected_members: 0,
            issue_count: 0,
            seen_profiles: BTreeSet::new(),
            native_packet_count: 0,
            native_packet_bytes: 0,
        })
    }

    pub fn registry_digest(&self) -> &str {
        &self.registry_sha256
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
    ) -> Result<(Vec<SchemaResource>, String, String, String), RecordRuleError> {
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
            probe.schema_set_digest().to_hex(),
            Digest256::of_bytes(registry_raw).to_hex(),
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
            schema_set_digest: probe.schema_set_digest().to_hex(),
            format_profile_id: self.format_profile.id().to_owned(),
            instance_sha256: Digest256::of_bytes(raw).to_hex(),
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
            schema_set_digest: probe.schema_set_digest().to_hex(),
            format_profile_id: self.format_profile.id().to_owned(),
            instance_sha256: Digest256::of_bytes(raw).to_hex(),
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
                path: path.to_owned(),
                version,
                raw_sha256: Digest256::of_bytes(raw).to_hex(),
            },
        )
    }

    /// Shadow-only inline schema probe. It has no CPU deadline and cannot
    /// complete an audit rule. It still emits exact per-member observations.
    pub fn inspect_member(
        &mut self,
        path: &str,
        raw: &[u8],
        sink: &mut impl RecordSink,
    ) -> Result<(), RecordRuleError> {
        self.inspect_member_inner(path, raw, sink, None)
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
        self.inspect_member_inner(path, raw, sink, Some(evidence))
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
                path: path.to_owned(),
                version,
                raw_sha256: Digest256::of_bytes(raw).to_hex(),
            },
        )?;
        for field in ["source_refs", "source_record_refs", "receipt_refs"] {
            if let Some(values) = record.get(field).and_then(Value::as_array) {
                for target in values.iter().filter_map(Value::as_str) {
                    emit(
                        sink,
                        RecordObservation::Reference {
                            from_path: path.to_owned(),
                            target_path: target.to_owned(),
                        },
                    )?;
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
            if let Some(target) = record.get(field).and_then(Value::as_str) {
                emit(
                    sink,
                    RecordObservation::Reference {
                        from_path: path.to_owned(),
                        target_path: target.to_owned(),
                    },
                )?;
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
    if verdict.instance_sha256 != Digest256::of_bytes(raw).to_hex()
        || verdict.schema_set_digest != probe.schema_set_digest().to_hex()
        || verdict.format_profile_id != format_profile.id()
        || verdict.root_uri != root_uri
        || verdict.worker_protocol_id.is_empty()
        || verdict.worker_binary_digest.len() != 64
        || !verdict
            .worker_binary_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
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
    if uri != format!("https://tree-of-sophia.local/{path}")
        && uri != format!("https://treeofsophia.local/{path}")
    {
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
        ("agents", "agent.json") => (
            "agent",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        ("places", "place.json") => (
            "place",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        ("organizations", "organization.json") => (
            "organization",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        ("works", "work.json") => ("work", CORPUS_CONTRACT, "tos_corpus_record_v1", "record_id"),
        ("expressions", "expression.json") => (
            "expression",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        ("editions", "edition.json") => (
            "edition",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        ("collections", "collection.json") => (
            "collection",
            CORPUS_CONTRACT,
            "tos_corpus_record_v1",
            "record_id",
        ),
        ("items", "item.json") => ("item", CORPUS_CONTRACT, "tos_corpus_record_v1", "record_id"),
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

fn compile_profiles(registry: &Value) -> Result<BTreeMap<String, Profile>, RecordRuleError> {
    let entries = registry
        .get("types")
        .and_then(Value::as_array)
        .ok_or_else(|| unsupported("registry_types", ENTITY_REGISTRY))?;
    let mut entities = BTreeMap::new();
    for entry in entries {
        let id = field_str(entry, "type_id", "type_identity")?.to_owned();
        let parents = field_array_str(entry, "parent_type_ids", "type_parent")?;
        let role = field_str(entry, "object_role", "type_role")?.to_owned();
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
                    field_str(mapping, "source_graph", "mapping_graph")?.to_owned(),
                    field_str(mapping, "source_kind_id", "mapping_kind")?.to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, RecordRuleError>>()?;
        if entities
            .insert(
                id.clone(),
                Entity {
                    parents,
                    role,
                    abstract_type,
                    mappings,
                    profile: entry.get("source_record_profile").cloned(),
                },
            )
            .is_some()
        {
            return Err(unsupported("duplicate_type_id", &id));
        }
    }
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
            && type_id == "tos.entity.composite"
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
            kind == "sign" && type_id == "tos.entity.sign" && reader == "semantic-metadata-v1";
        if (value.get("creation_gate").is_some() && !authored_sign)
            || (authored_sign
                && value.get("creation_gate").and_then(Value::as_str) != Some("sign-promotion-v1"))
            || ((kind == "sign" || type_id == "tos.entity.sign") && !authored_sign)
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
            || type_id == ancestor
            || !has_ancestor(&entities, type_id, ancestor)?
        {
            return Err(unsupported("profile_type_ancestry", &kind));
        }
        for graph in ["source-claims", "source-navigation"] {
            if entity
                .mappings
                .iter()
                .filter(|(g, k)| g == graph && k == &kind)
                .count()
                != 1
                || entities.iter().any(|(other_id, other)| {
                    other_id != type_id
                        && other.mappings.iter().any(|(g, k)| g == graph && k == &kind)
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

fn has_ancestor(
    entities: &BTreeMap<String, Entity>,
    start: &str,
    ancestor: &str,
) -> Result<bool, RecordRuleError> {
    let mut found = false;
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    let mut stack = vec![(start.to_owned(), false)];
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
            .get(&id)
            .ok_or_else(|| unsupported("unknown_parent_type", &id))?;
        if id == ancestor {
            found = true;
        }
        active.insert(id.clone());
        stack.push((id, true));
        for parent in entity.parents.iter().rev() {
            stack.push((parent.clone(), false));
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
        let contract_dir = root().join("ToS/contracts");
        let files: Vec<_> = std::fs::read_dir(contract_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|item| item.file_name().to_string_lossy().ends_with(".schema.json"))
            .map(|item| {
                let name = item.file_name().to_string_lossy().into_owned();
                (
                    format!("ToS/contracts/{name}"),
                    std::fs::read(item.path()).unwrap(),
                )
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
        raw_digest: &str,
        schema_set_digest: &str,
        profile: &str,
        root: &str,
    ) -> BoundedSchemaVerdict {
        BoundedSchemaVerdict {
            instance_sha256: raw_digest.to_owned(),
            schema_set_digest: schema_set_digest.to_owned(),
            format_profile_id: profile.to_owned(),
            root_uri: root.to_owned(),
            worker_protocol_id: "test-only-worker-protocol".to_owned(),
            worker_binary_digest: "a".repeat(64),
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
                &plan.instance_sha256,
                &plan.schema_set_digest,
                &plan.format_profile_id,
                &plan.route_uri,
            ),
            common: synthetic_verdict(
                &plan.instance_sha256,
                &plan.schema_set_digest,
                &plan.format_profile_id,
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
        wrong.common.instance_sha256 = "b".repeat(64);
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
            &plan.instance_sha256,
            &plan.schema_set_digest,
            &plan.format_profile_id,
            &plan.root_uri,
        );
        let mut events = Events::default();
        rule.inspect_native_with_bounded_schema(ARTIFACT, &raw, &evidence, &mut events)
            .unwrap();
        assert!(events.0.iter().any(|event| matches!(event,
            RecordObservation::IdOwner { id, .. } if id.starts_with("tos.artifact."))));
    }
}
