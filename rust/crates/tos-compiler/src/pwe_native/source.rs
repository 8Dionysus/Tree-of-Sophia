//! D4 supplied metadata route and one exact-basis adapter. No new source reader,
//! occurrence transport, source membership proof or registry writer is added.
use super::{
    basis::{Error, PweRef, Result, Session},
    qualification::{BasisSlot, PreparedAccount},
};
use serde_json::Value;
use tos_foundation::Digest256;

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
        for value in [entry, record, reference, registry.value] {
            self.admit(value)?;
        }
        let r = self.exact(reference)?;
        // Prepay the maintained owner's canonical helper holders/work, including
        // repeated calls. Source/schema parsing is independently caller-owned.
        for value in [entry, record, registry.value] {
            self.digest(value)?;
        }
        let limits = crate::source_bibliographic::BibliographicDocumentLimits {
            input_document_bytes: self.limits.max_input_bytes,
            output_row_bytes: self.limits.max_input_bytes,
        };
        let descriptor = crate::source_bibliographic::supplied_metadata_descriptor(
            entry,
            record,
            registry.value,
            limits,
        )
        .map_err(|_| Error::Unsupported("maintained metadata owner route"))?;
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
