//! D4 supplied metadata route and one exact-basis adapter. No new source reader,
//! occurrence transport, source membership proof or registry writer is added.
use super::{
    basis::{Error, PweRef, Result, Session},
    qualification::{BasisSlot, PreparedAccount},
};
use serde_json::Value;
use tos_foundation::Digest256;

fn metadata_sum(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Error::ResourceLimit)
}
fn metadata_product(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b).ok_or(Error::ResourceLimit)
}

#[cfg(test)]
mod metadata_admission_tests {
    use super::*;
    use crate::pwe_native::{Limits, SourceAdmission};
    use serde_json::json;
    use std::{
        cell::Cell,
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    #[test]
    fn same_session_descriptor_uses_maintained_routes_and_prepaid_prefixes() {
        const I: usize = 33_554_432;
        struct Original {
            read: Cell<usize>,
            live: Cell<usize>,
            reject_route_prefix: Cell<bool>,
        }
        impl SourceAdmission for Original {
            fn inspect(&self, n: usize) -> Result<()> {
                if n == 4096 && self.reject_route_prefix.replace(false) {
                    return Err(Error::Upstream("original descriptor prefix refusal"));
                }
                self.read
                    .set(self.read.get().checked_sub(n).ok_or(Error::ResourceLimit)?);
                Ok(())
            }
            fn retain(&self, n: usize) -> Result<()> {
                self.live
                    .set(self.live.get().checked_sub(n).ok_or(Error::ResourceLimit)?);
                Ok(())
            }
        }
        let original = Original {
            read: Cell::new(I),
            live: Cell::new(I),
            reject_route_prefix: Cell::new(false),
        };
        let cancelled = AtomicBool::new(false);
        let limits = Limits {
            max_input_bytes: I,
            max_work: I,
            max_nodes: 1_000_000,
            max_refs: 4096,
            max_depth: 96,
            max_items: 4096,
            max_state_bytes: I,
            deadline: Instant::now() + Duration::from_secs(30),
        };
        let mut s = Session::new_source(limits, &cancelled, &original).unwrap();
        // This is a resource-test projection of existing registry rows, with
        // its own TEST identity. It is never a native registered authority.
        let mut entities: Value = serde_json::from_str(include_str!(
            "../../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json"
        ))
        .unwrap();
        entities["registry_id"] = json!("test.pwe.metadata-resource-projection");
        entities.as_object_mut().unwrap().retain(|key, _| {
            matches!(
                key.as_str(),
                "schema_version" | "registry_id" | "registry_version" | "types"
            )
        });
        entities["types"].as_array_mut().unwrap().retain(|ty| {
            ty["source_mappings"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["source_graph"] == "source-navigation"
                        && matches!(
                            row["source_kind_id"].as_str(),
                            Some("agent" | "artifact" | "link" | "language")
                        )
                })
            })
        });
        let registry_ref = json!({"id":entities["registry_id"], "version":entities["registry_version"],
            "digest":s.digest(&entities).unwrap().to_prefixed()});
        let registry = s.registry(&entities, &registry_ref).unwrap();
        let cases = [
            (
                "agent",
                "ToS/source-witnesses/agents/friedrich-ritschl/agent.json",
                include_str!(
                    "../../../../../ToS/source-witnesses/agents/friedrich-ritschl/agent.json"
                ),
                "record_id",
                "ToS/contracts/corpus-record.schema.json",
            ),
            (
                "artifact",
                "ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645/artifact-witness.json",
                include_str!(
                    "../../../../../ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645/artifact-witness.json"
                ),
                "artifact_id",
                "ToS/contracts/artifact-source-witness-v2.schema.json",
            ),
            (
                "link",
                "ToS/source-witnesses/links/cdli/cdlb-2006-1/article/link.json",
                include_str!(
                    "../../../../../ToS/source-witnesses/links/cdli/cdlb-2006-1/article/link.json"
                ),
                "record_id",
                "ToS/contracts/source-link.schema.json",
            ),
            (
                "language",
                "ToS/source-witnesses/languages/sumerian/language.json",
                include_str!(
                    "../../../../../ToS/source-witnesses/languages/sumerian/language.json"
                ),
                "record_id",
                "ToS/contracts/linguistic-description-record.schema.json",
            ),
        ];
        for (kind, path, raw, identity, schema) in cases {
            let record: Value = serde_json::from_str(raw).unwrap();
            // Caller-supplied test catalog projection, not an accepted source
            // catalog row or source membership/currentness receipt.
            let digest = s.digest(&record).unwrap();
            let reference = json!({"id":record[identity], "version":record["record_version"], "digest":digest.to_prefixed()});
            let entry = json!({"record_type":kind, "record_id":record[identity],
                "source_record_ref":path, "source_schema_ref":schema, "record_sha256":digest.to_hex()});
            let expected = crate::source_bibliographic::supplied_metadata_descriptor(
                &entry,
                &record,
                &entities,
                crate::source_bibliographic::BibliographicDocumentLimits {
                    input_document_bytes: I,
                    output_row_bytes: I,
                },
            )
            .unwrap();
            let before = s.charges();
            let before_read = original.read.get();
            let actual = s.metadata(&entry, &record, &reference, &registry).unwrap();
            assert_eq!(actual.descriptor(), &expected);
            assert!(s.charges().2 > before.2 && s.charges().4 > before.4);
            assert!(original.read.get() < before_read);
        }
        let entry = json!({"record_type":"agent", "source_record_ref":"ToS/source-witnesses/agents/a/agent.json"});
        let record = json!({"schema_version":"tos_corpus_record_v1"});
        let before = s.charges();
        original.reject_route_prefix.set(true);
        assert_eq!(
            s.prepay_metadata_route(&entry, &record, &entities),
            Err(Error::Upstream("original descriptor prefix refusal"))
        );
        assert!(s.charges().4 > before.4); // prior state debit is retained
        let before_retry = s.charges();
        s.prepay_metadata_route(&entry, &record, &entities).unwrap();
        assert!(s.charges().2 > before_retry.2 && s.charges().4 > before_retry.4);
        let invalid = json!({"record_type":"agent", "source_record_ref":"ToS/private/agent.json"});
        assert!(matches!(
            s.source_metadata_descriptor(&invalid, &record, &entities),
            Err(Error::Unsupported(_))
        ));
        let mut exhausted_limits = limits;
        exhausted_limits.max_work = 1;
        let mut exhausted = Session::new_source(exhausted_limits, &cancelled, &original).unwrap();
        assert_eq!(
            exhausted.prepay_metadata_route(&entry, &record, &entities),
            Err(Error::ResourceLimit)
        );
        let spent = exhausted.charges();
        assert!(spent.4 > 0);
        assert_eq!(
            exhausted.prepay_metadata_route(&entry, &record, &entities),
            Err(Error::ResourceLimit)
        );
        assert!(exhausted.charges().4 > spent.4); // no refill on failure
    }
}

impl Session<'_> {
    // Resource-only trace of the pinned RecordRoute::derive. It never returns
    // a route, descriptor or semantic selection. The maintained owner below
    // still decides the result. Two lookup fees pay this trace AND that call.
    fn metadata_foreign_get<'v>(&mut self, v: &'v Value, key: &str) -> Result<Option<&'v Value>> {
        self.admit_source_prefix(0, 2)?;
        let Some(object) = v.as_object() else {
            return Ok(None);
        };
        for actual in object.keys() {
            let fee = metadata_sum(metadata_sum(actual.len(), key.len())?, 1)?;
            self.admit_source_prefix(metadata_product(fee, 2)?, 2)?;
        }
        Ok(object.get(key))
    }
    fn metadata_foreign_text<'v>(&mut self, v: &'v Value, key: &str) -> Result<Option<&'v str>> {
        let value = self.metadata_foreign_get(v, key)?;
        let Some(text) = value.and_then(Value::as_str) else {
            return Ok(None);
        };
        self.admit_source_prefix(metadata_product(text.len(), 2)?, 2)?;
        Ok((!text.is_empty()).then_some(text))
    }
    fn metadata_foreign_is(&mut self, v: Option<&Value>, expected: &str) -> Result<bool> {
        self.admit_source_prefix(0, 2)?;
        let Some(actual) = v.and_then(Value::as_str) else {
            return Ok(false);
        };
        self.admit_source_prefix(
            metadata_product(metadata_sum(actual.len(), expected.len())?, 2)?,
            2,
        )?;
        Ok(actual == expected)
    }
    // UTF8/number-spelling bytes plus one unit per value/key. Metadata lengths
    // are borrowed; contents/visits are paid before each recursive inspection.
    // The caller reserves the entire recursion frame envelope before entry.
    fn metadata_extent(&mut self, v: &Value, depth: usize) -> Result<usize> {
        self.admit_source_prefix(0, 1)?;
        if depth > self.limits.max_depth {
            return Err(Error::ResourceLimit);
        }
        let mut units = 1;
        match v {
            Value::Null | Value::Bool(_) => {}
            Value::Number(number) => {
                let n = number.as_str().len();
                self.admit_source_prefix(n, 1)?;
                units = metadata_sum(units, n)?;
            }
            Value::String(text) => {
                self.admit_source_prefix(text.len(), 1)?;
                units = metadata_sum(units, text.len())?;
            }
            Value::Array(values) => {
                for value in values {
                    units = metadata_sum(units, self.metadata_extent(value, depth + 1)?)?;
                }
            }
            Value::Object(values) => {
                for (key, value) in values {
                    self.admit_source_prefix(key.len(), 1)?;
                    units = metadata_sum(units, metadata_sum(key.len(), 1)?)?;
                    units = metadata_sum(units, self.metadata_extent(value, depth + 1)?)?;
                }
            }
        }
        Ok(units)
    }
    fn prepay_metadata_route(
        &mut self,
        entry: &Value,
        record: &Value,
        entities: &Value,
    ) -> Result<()> {
        use std::mem::size_of;
        let slot = metadata_sum(
            size_of::<Value>(),
            metadata_sum(size_of::<(String, Value)>(), 4 * size_of::<usize>())?,
        )?;
        self.state(metadata_sum(
            metadata_product(metadata_product(self.limits.max_depth + 1, slot)?, 8)?,
            8192,
        )?)?;
        // Fixed fields/literals and the bounded descriptor map/route headers.
        self.admit_source_prefix(4096, 32)?;
        let Some(kind) = self.metadata_foreign_text(entry, "record_type")? else {
            return Ok(());
        };
        let Some(reference) = self.metadata_foreign_text(entry, "source_record_ref")? else {
            return Ok(());
        };
        // Kind dispatch precedes the owner's required types-array check, so
        // this reservation must also precede our corresponding early exit.
        self.admit_source_prefix(
            metadata_sum(
                metadata_product(kind.len(), 32)?,
                metadata_product(reference.len(), 512)?,
            )?,
            32,
        )?;
        let Some(types) = self
            .metadata_foreign_get(entities, "types")?
            .and_then(Value::as_array)
        else {
            return Ok(());
        };
        self.admit_source_prefix(0, metadata_product(types.len(), 2)?)?;
        // Both collect vectors and the complete split path may grow. Reserve
        // four slots per possible member before any maintained allocation.
        self.state(metadata_sum(
            metadata_product(metadata_product(types.len(), size_of::<&Value>())?, 4)?,
            metadata_sum(
                metadata_product(
                    metadata_product(metadata_sum(reference.len(), 1)?, size_of::<&str>())?,
                    4,
                )?,
                3 * size_of::<Vec<&Value>>(),
            )?,
        )?)?;
        let mut owner = None;
        let mut matches = 0usize;
        for ty in types {
            let mappings = self
                .metadata_foreign_get(ty, "source_mappings")?
                .and_then(Value::as_array);
            let mut selected = false;
            if let Some(mappings) = mappings {
                self.admit_source_prefix(0, metadata_product(mappings.len(), 2)?)?;
                for mapping in mappings {
                    let graph = self.metadata_foreign_get(mapping, "source_graph")?;
                    let source_kind = self.metadata_foreign_get(mapping, "source_kind_id")?;
                    let graph_matches = self.metadata_foreign_is(graph, "source-navigation")?;
                    let kind_matches = self.metadata_foreign_is(source_kind, kind)?;
                    selected |= graph_matches && kind_matches;
                }
            }
            if selected {
                matches = metadata_sum(matches, 1)?;
                owner = Some(ty);
            }
        }
        // On these exits the real owner rejects before any remaining work.
        if matches != 1 {
            return Ok(());
        }
        let owner = owner.ok_or(Error::Upstream("metadata resource trace"))?;
        if self.metadata_foreign_text(owner, "type_id")?.is_none() {
            return Ok(());
        }
        let null = Value::Null;
        let version = self
            .metadata_foreign_get(record, "schema_version")?
            .unwrap_or(&null);
        // Native witness dispatch and final descriptor access can both occur.
        let _ = self.metadata_foreign_get(record, "schema_version")?;
        let _ = self.metadata_foreign_get(entry, "source_schema_ref")?;
        let entry_units = self.metadata_extent(entry, 0)?;
        let owner_units = self.metadata_extent(owner, 0)?;
        let version_units = self.metadata_extent(version, 0)?;
        let retained = metadata_sum(metadata_sum(entry_units, owner_units)?, version_units)?;
        self.state(metadata_product(
            metadata_product(metadata_sum(retained, 64)?, slot)?,
            4,
        )?)?;
        self.admit_source_prefix(metadata_product(metadata_sum(retained, 64)?, 128)?, 16)?;
        // schema_version can be an arbitrary supplied JSON value. The owner
        // clones it twice; reserve a quadratic comparison/insertion envelope
        // rather than assuming the schema worker already proved a string.
        self.admit_source_prefix(
            metadata_product(
                metadata_product(
                    metadata_sum(version_units, 1)?,
                    metadata_sum(version_units, 1)?,
                )?,
                16,
            )?,
            2,
        )?;
        let native_witness = kind == "artifact"
            || kind == "composite" && reference.ends_with("/composite-witness.json");
        let native_corpus = [
            "agent",
            "place",
            "organization",
            "work",
            "expression",
            "edition",
            "collection",
            "item",
        ]
        .contains(&kind);
        if !native_witness && kind != "link" && !native_corpus {
            let Some(profile) = self.metadata_foreign_get(owner, "source_record_profile")? else {
                return Ok(());
            };
            for key in [
                "record_type",
                "id_prefix",
                "source_basename",
                "catalog_filename",
            ] {
                let _ = self.metadata_foreign_get(profile, key)?;
            }
            let Some(schemas) = self
                .metadata_foreign_get(profile, "schemas")?
                .and_then(Value::as_array)
            else {
                return Ok(());
            };
            self.state(metadata_sum(
                metadata_product(metadata_product(schemas.len(), size_of::<&Value>())?, 4)?,
                size_of::<Vec<&Value>>(),
            )?)?;
            self.admit_source_prefix(0, metadata_product(schemas.len(), 2)?)?;
            for schema in schemas {
                let selected_version = self
                    .metadata_foreign_get(schema, "schema_version")?
                    .unwrap_or(&null);
                let _ = self.metadata_foreign_get(record, "schema_version")?;
                let _ = self.metadata_foreign_get(schema, "schema_ref")?;
                let candidate_units = self.metadata_extent(selected_version, 0)?;
                let pair = metadata_product(
                    metadata_sum(candidate_units, 1)?,
                    metadata_sum(version_units, 1)?,
                )?;
                self.admit_source_prefix(metadata_product(pair, 8)?, 1)?;
            }
        }
        self.check()
    }
    fn source_metadata_descriptor(
        &mut self,
        entry: &Value,
        record: &Value,
        entities: &Value,
    ) -> Result<Value> {
        self.source_allowance()?;
        // Same facade grammar and codec composition, with each actual codec
        // charged prospectively by THIS Session. No opaque outer encode calls.
        for value in [entry, record, entities] {
            let _ = self.source_canonical(value)?;
        }
        self.prepay_metadata_route(entry, record, entities)?;
        let result = crate::source_bibliographic_versions::supplied_metadata_descriptor(
            entry, record, entities,
        )
        .map_err(|_| Error::Unsupported("maintained metadata owner route"));
        // The finite pure owner call was fully prepaid. Preserve its error;
        // never release successful content after the original cutoff expires.
        let checked = self.check();
        let descriptor = match (result, checked) {
            (Err(error), _) | (Ok(_), Err(error)) => return Err(error),
            (Ok(value), Ok(())) => value,
        };
        let _ = self.source_canonical(&descriptor)?;
        Ok(descriptor)
    }
}

pub struct RegistryDescriptor<'v> {
    pub(super) value: &'v Value,
    pub(super) reference: PweRef<'v>,
}
impl RegistryDescriptor<'_> {
    pub fn reference(&self) -> PweRef<'_> {
        self.reference
    }
}
/// Route/exact-content checked, NOT source/schema/admission authenticated. The
/// supplied metadata facade deliberately has the same boundary.
pub struct CheckedMetadata<'v> {
    value: &'v Value,
    reference: PweRef<'v>,
    registry: PweRef<'v>,
    descriptor: Value,
}
impl<'v> CheckedMetadata<'v> {
    pub fn value(&self) -> &'v Value {
        self.value
    }
    pub fn reference(&self) -> PweRef<'v> {
        self.reference
    }
    pub fn registry(&self) -> PweRef<'v> {
        self.registry
    }
    pub fn descriptor(&self) -> &Value {
        &self.descriptor
    }
}
/// A borrowed value selected by an actual source owner. This is a transport
/// input, not a receipt. Callers retain selected-cut/current source fences.
pub struct SuppliedExact<'v> {
    pub value: &'v Value,
    pub identity_field: &'v str,
    pub version_field: &'v str,
}
pub struct ResolvedSlot<'v> {
    pub slot: BasisSlot<'v>,
    pub value: &'v Value,
}
pub struct ResolvedBasis<'v> {
    pub(super) account: &'v Value,
    pub(super) slots: Vec<ResolvedSlot<'v>>,
}
impl<'v> ResolvedBasis<'v> {
    pub fn slots(&self) -> &[ResolvedSlot<'v>] {
        &self.slots
    }
}

impl Session<'_> {
    pub fn registry<'v>(
        &mut self,
        value: &'v Value,
        reference: &'v Value,
    ) -> Result<RegistryDescriptor<'v>> {
        self.admit(value)?;
        self.admit(reference)?;
        let r = self.exact(reference)?;
        self.record_matches(value, r, "registry_id", "registry_version")?;
        let schema = self.str_field(value, "schema_version")?;
        if schema != "tos_semantic_entity_type_registry_v1" {
            return Err(Error::Unsupported("entity registry schema"));
        }
        let xs = self.field(value, "types")?;
        self.array(xs)?;
        Ok(RegistryDescriptor {
            value,
            reference: r,
        })
    }
    pub fn metadata<'v>(
        &mut self,
        entry: &Value,
        record: &'v Value,
        reference: &'v Value,
        registry: &RegistryDescriptor<'v>,
    ) -> Result<CheckedMetadata<'v>> {
        self.check()?;
        for value in [entry, record, reference, registry.value] {
            self.admit(value)?;
        }
        let r = self.exact(reference)?;
        let descriptor = if self.source_mode() {
            self.source_metadata_descriptor(entry, record, registry.value)?
        } else {
            // Preserve the supplied-value facade and its existing envelope.
            for value in [entry, record, registry.value] {
                self.digest(value)?;
            }
            let limits = crate::source_bibliographic::BibliographicDocumentLimits {
                input_document_bytes: self.limits.max_input_bytes,
                output_row_bytes: self.limits.max_input_bytes,
            };
            crate::source_bibliographic::supplied_metadata_descriptor(
                entry,
                record,
                registry.value,
                limits,
            )
            .map_err(|_| Error::Unsupported("maintained metadata owner route"))?
        };
        self.admit(&descriptor)?;
        self.digest(&descriptor)?;
        let identity = self.str_field(&descriptor, "identity_field")?;
        self.record_matches(record, r, identity, "record_version")?;
        let catalog_id = self.str_field(entry, "record_id")?;
        if !self.same_text(catalog_id, r.id)? {
            return Err(Error::ExactMismatch);
        }
        let hex = self.str_field(entry, "record_sha256")?;
        if Digest256::from_hex(hex).map_err(|_| Error::Malformed("catalog digest"))? != r.digest {
            return Err(Error::ExactMismatch);
        }
        let es = self.field(entry, "source_schema_ref")?;
        let ds = self.field(&descriptor, "source_schema_ref")?;
        if !self.equal(es, ds)? {
            return Err(Error::ExactMismatch);
        }
        let kind = self.str_field(&descriptor, "record_type")?;
        if self.source_mode() {
            // The two prefix operations inspect the already selected strings
            // again; descriptor construction is not credit for later work.
            self.admit_source_prefix(
                r.id.len()
                    .checked_add(kind.len())
                    .and_then(|n| n.checked_add(5))
                    .and_then(|n| n.checked_mul(2))
                    .ok_or(Error::ResourceLimit)?,
                2,
            )?;
        }
        let prefix_ok =
            r.id.strip_prefix("tos.")
                .and_then(|x| x.strip_prefix(kind))
                .is_some_and(|x| x.starts_with('.'));
        if !prefix_ok {
            return Err(Error::Malformed("metadata kind/id"));
        }
        if self.str_field(&descriptor, "adapter")? != "native-witness" {
            let actual = self.str_field(record, "record_type")?;
            if !self.same_text(kind, actual)? {
                return Err(Error::ExactMismatch);
            }
        }
        Ok(CheckedMetadata {
            value: record,
            reference: r,
            registry: registry.reference,
            descriptor,
        })
    }
    /// All-or-error exact resolution in occurrence order. The callback performs
    /// owner route, schema/currentness and selected source-cut resolution using
    /// its own nonresettable source budget; this function rechecks every exact
    /// record with the shared codec. No occurrence namespace is interpreted here.
    pub fn resolve_basis<'v, F>(
        &mut self,
        p: &PreparedAccount<'v>,
        mut owner: F,
    ) -> Result<ResolvedBasis<'v>>
    where
        F: FnMut(PweRef<'v>) -> Result<SuppliedExact<'v>>,
    {
        let slots = self.exact_basis(p)?;
        let mut out = self.vec(slots.len())?;
        for slot in slots {
            self.check()?;
            let supplied = owner(slot.reference)?;
            self.check()?;
            self.admit(supplied.value)?;
            self.record_matches(
                supplied.value,
                slot.reference,
                supplied.identity_field,
                supplied.version_field,
            )?;
            out.push(ResolvedSlot {
                slot,
                value: supplied.value,
            });
        }
        Ok(ResolvedBasis {
            account: p.value,
            slots: out,
        })
    }
    /// Endpoint/member coherence is checked before any resolver can return a
    /// semantic account. Whole TypedRefs, rather than bare IDs, are compared.
    pub fn check_endpoints(&mut self, p: &PreparedAccount<'_>) -> Result<()> {
        self.admit(p.value)?;
        let e = self.field(p.value, "endpoints")?;
        let endpoints = self.array(e)?;
        let m = self.field(p.value, "members")?;
        let members = self.array(m)?;
        if endpoints.len() != members.len() {
            return Err(Error::Malformed("members/endpoints size"));
        }
        for (i, endpoint) in endpoints.iter().enumerate() {
            let (_, r) = self.typed(endpoint)?;
            for other in &endpoints[..i] {
                let (_, o) = self.typed(other)?;
                if self.same_text(r.id, o.id)? {
                    return Err(Error::Malformed("ambiguous endpoint ID"));
                }
            }
            let mut found = false;
            for member in members {
                let text = self.text(member)?;
                if self.same_text(text, r.id)? {
                    found = true;
                }
            }
            if !found {
                return Err(Error::Malformed("members/endpoints ID set"));
            }
        }
        let subject = self.field(p.value, "subject")?;
        self.require_endpoint(endpoints, subject)?;
        match p.kind {
            super::Kind::Identity => {
                let l = self.field(p.value, "left")?;
                let r = self.field(p.value, "right")?;
                let (_, lr) = self.typed(l)?;
                let (_, rr) = self.typed(r)?;
                if self.same_text(lr.id, rr.id)? || endpoints.len() != 2 {
                    return Err(Error::Malformed("two distinct correspondence subjects"));
                }
                self.require_endpoint(endpoints, l)?;
                self.require_endpoint(endpoints, r)?;
            }
            super::Kind::Name => {
                let b = self.field(p.value, "named_bearer")?;
                if self.str_field(b, "state")? == "bound" {
                    let b = self.field(b, "value")?;
                    self.require_endpoint(endpoints, b)?;
                }
            }
            super::Kind::Responsibility => {
                let a = self.field(p.value, "assignees")?;
                match self.str_field(a, "mode")? {
                    "single" | "identified-anonymous" => {
                        let b = self.field(a, "bearer")?;
                        self.require_endpoint(endpoints, b)?;
                    }
                    "joint" => {
                        let b = self.field(a, "bearers")?;
                        for x in self.array(b)? {
                            self.require_endpoint(endpoints, x)?;
                        }
                    }
                    "alternatives" => {
                        let b = self.field(a, "groups")?;
                        for g in self.array(b)? {
                            for x in self.array(g)? {
                                self.require_endpoint(endpoints, x)?;
                            }
                        }
                    }
                    "unestablished" => {}
                    _ => return Err(Error::Malformed("assignees")),
                }
            }
        }
        Ok(())
    }
    fn require_endpoint(&mut self, endpoints: &[Value], value: &Value) -> Result<()> {
        for e in endpoints {
            if self.equal(e, value)? {
                return Ok(());
            }
        }
        Err(Error::Malformed("missing exact endpoint"))
    }
    /// G06--G11 use the maintained metadata descriptors, never a submitted kind
    /// label or the prefix of a person/work ID. Cross-level policies remain a
    /// separate explicitly unsupported binding in this exact first consumer.
    pub fn check_role_matrix(
        &mut self,
        p: &PreparedAccount<'_>,
        metadata: &[CheckedMetadata<'_>],
    ) -> Result<()> {
        self.check_endpoints(p)?;
        if metadata.len() > self.limits.max_items {
            return Err(Error::ResourceLimit);
        }
        let subject = self.field(p.value, "subject")?;
        let subject_kind = self.endpoint_kind(subject, metadata)?;
        let endpoints_value = self.field(p.value, "endpoints")?;
        let endpoints = self.array(endpoints_value)?;
        for endpoint in endpoints {
            self.endpoint_kind(endpoint, metadata)?;
        }
        match p.kind {
            super::Kind::Name => {
                let assignment = self.str_field(p.value, "assignment")?;
                let role = self.str_field(p.value, "subject_role")?;
                if !matches!(
                    (assignment, role),
                    ("name-used-by", "named-referent") | ("signature-in", "containing-object")
                ) {
                    return Err(Error::QualificationMismatch("name assignment subject role"));
                }
                let b = self.field(p.value, "named_bearer")?;
                let b = self.bound(b, "named bearer role")?;
                let k = self.endpoint_kind(b, metadata)?;
                if !matches!(
                    k,
                    "agent"
                        | "organization"
                        | "textual-persona"
                        | "collective"
                        | "work"
                        | "expression"
                        | "edition"
                        | "item"
                        | "artifact"
                        | "file"
                        | "document"
                        | "collection"
                        | "composite"
                        | "textual-fragment"
                        | "quotation-passage"
                ) {
                    return Err(Error::NeedsBinding("named bearer identity role"));
                }
                let function = self.str_field(p.value, "name_function")?;
                if function == "collective-pseudonym" && k != "collective" {
                    return Err(Error::NeedsBinding("collective bearer policy"));
                }
                if assignment == "name-used-by" && !self.equal(subject, b)? {
                    return Err(Error::QualificationMismatch(
                        "named-referent subject/bearer",
                    ));
                }
                if assignment == "signature-in"
                    && !matches!(
                        subject_kind,
                        "work"
                            | "expression"
                            | "edition"
                            | "item"
                            | "artifact"
                            | "file"
                            | "document"
                            | "composite"
                            | "textual-fragment"
                            | "quotation-passage"
                    )
                {
                    return Err(Error::NeedsBinding("signature containing-object role"));
                }
                for endpoint in endpoints {
                    if !self.equal(endpoint, subject)? && !self.equal(endpoint, b)? {
                        return Err(Error::Malformed("extra name participant"));
                    }
                }
            }
            super::Kind::Responsibility => {
                let role = self.str_field(p.value, "role")?;
                let supported = match role {
                    "author" => matches!(subject_kind, "work" | "document"),
                    "translator" | "expression-editor" => subject_kind == "expression",
                    "edition-editor" => subject_kind == "edition",
                    "compiler" => matches!(subject_kind, "composite" | "collection"),
                    "copyist" => matches!(subject_kind, "artifact" | "item"),
                    "speaker" | "addressee" => subject_kind == "document",
                    "rights-holder" => true,
                    // No publication/provision-activity route is fabricated by
                    // treating an Edition as that activity.
                    "publisher" => {
                        return Err(Error::NeedsBinding("publication/provision activity role"));
                    }
                    _ => false,
                };
                if !supported {
                    return Err(Error::NeedsBinding("responsibility subject role"));
                }
                let mode = self.str_field(p.value, "attribution_mode")?;
                let assignees = self.field(p.value, "assignees")?;
                for endpoint in endpoints {
                    let assigned = self.is_assignee(assignees, endpoint)?;
                    if !assigned {
                        if self.equal(endpoint, subject)? {
                            continue;
                        }
                        return Err(Error::Malformed("extra responsibility participant"));
                    }
                    let k = self.endpoint_kind(endpoint, metadata)?;
                    let allowed = if mode == "historical-responsibility" {
                        matches!(k, "agent" | "organization")
                    } else {
                        matches!(
                            k,
                            "agent" | "organization" | "collective" | "textual-persona"
                        )
                    };
                    if !allowed {
                        return Err(Error::NeedsBinding("responsibility bearer role"));
                    }
                }
            }
            super::Kind::Identity => {
                let level = self.str_field(p.value, "identity_level")?;
                let kind = match level {
                    "historical-person" => "agent",
                    other => other,
                };
                for endpoint in endpoints {
                    let k = self.endpoint_kind(endpoint, metadata)?;
                    if !self.same_text(k, kind)? {
                        return Err(Error::NeedsBinding("identity level/cross-profile policy"));
                    }
                }
            }
        }
        Ok(())
    }
    fn is_assignee(&mut self, a: &Value, v: &Value) -> Result<bool> {
        match self.str_field(a, "mode")? {
            "single" | "identified-anonymous" => {
                let b = self.field(a, "bearer")?;
                self.equal(b, v)
            }
            "joint" => {
                let b = self.field(a, "bearers")?;
                for x in self.array(b)? {
                    if self.equal(x, v)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            "alternatives" => {
                let b = self.field(a, "groups")?;
                for g in self.array(b)? {
                    for x in self.array(g)? {
                        if self.equal(x, v)? {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }
            "unestablished" => Ok(false),
            _ => Err(Error::Malformed("assignees")),
        }
    }
    fn endpoint_kind<'m>(
        &mut self,
        v: &Value,
        metadata: &'m [CheckedMetadata<'_>],
    ) -> Result<&'m str> {
        let (kind, record) = self.typed(v)?;
        let mut selected = None;
        for m in metadata {
            self.tick(1)?;
            if m.reference == record && m.registry == kind {
                if selected.is_some() {
                    return Err(Error::Malformed("ambiguous metadata descriptor"));
                }
                selected = Some(m);
            }
        }
        let m = selected.ok_or(Error::NeedsBinding(
            "exact endpoint metadata owner descriptor",
        ))?;
        self.str_field(&m.descriptor, "record_type")
    }
    /// A source Claim must be independently schema/route checked by its owner.
    /// This bridge checks the selected wrapper content against the candidate;
    /// it does not accept a predicate string as registration evidence.
    pub fn check_claim_content(&mut self, p: &PreparedAccount<'_>, claim: &Value) -> Result<()> {
        self.admit(claim)?;
        self.check_endpoints(p)?;
        let pred = self.str_field(claim, "predicate")?;
        if !self.same_text(pred, p.kind.outer_predicate())? {
            return Err(Error::QualificationMismatch("outer account predicate"));
        }
        let object = self.field(claim, "object")?;
        if !self.equal(object, p.value)? {
            return Err(Error::ExactMismatch);
        }
        let subject = self.field(p.value, "subject")?;
        let (_, r) = self.typed(subject)?;
        let cs = self.str_field(claim, "subject_ref")?;
        if !self.same_text(cs, r.id)? {
            return Err(Error::QualificationMismatch("outer subject"));
        }
        Ok(())
    }
}
