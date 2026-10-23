use super::{
    codec::{self, closed, field, text},
    record::NativeTsRecord,
    session::{NativeTsSession, push},
};
use crate::{
    CheckedTsOutput, DomainError, DomainLimits, OutputCheckLimits, ResolvedRecord, TsCodomain,
    TsOutputStore, TsOutputValue, TsProfileBody, check_ts_output,
};
use std::cell::RefCell;
use tos_foundation::{
    CheckedOperationBinding, CutPreparedSchemaExecutionBinding, Digest256, ExactRecordRef,
    JsonDocument, JsonValue, SourceRevision,
};
type Result<T> = std::result::Result<T, DomainError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTsSelection {
    pub selection_ref: ExactRecordRef,
    pub source_revision: SourceRevision,
    pub input_identity: Digest256,
    pub entity_registry_ref: ExactRecordRef,
    pub role_registry_ref: ExactRecordRef,
    pub schema_execution: CutPreparedSchemaExecutionBinding,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeTsRole {
    Definition,
    ValueProfile,
    OperationResult,
}
impl NativeTsRole {
    pub fn schema_resource(self) -> &'static str {
        match self {
            Self::Definition => {
                "https://tree-of-sophia.local/ToS/contracts/ts-native-definition.schema.json"
            }
            Self::ValueProfile => {
                "https://tree-of-sophia.local/ToS/contracts/ts-native-value-profile.schema.json"
            }
            Self::OperationResult => {
                "https://tree-of-sophia.local/ToS/contracts/ts-native-operation-result.schema.json"
            }
        }
    }
}
#[derive(Clone, Copy)]
pub struct NativeTsRequest<'a> {
    pub exact: &'a ExactRecordRef,
    pub role: NativeTsRole,
}
/// Actual owner-derived route data, never an authority token or request flag.
pub struct NativeTsDescriptor<'a> {
    pub record_type: &'a str,
    pub schema_version: &'a str,
    pub schema_resource: &'a str,
}
pub struct SelectedNativeTsRecord<'a> {
    pub document: &'a JsonDocument,
    pub selection: &'a NativeTsSelection,
    pub descriptor: NativeTsDescriptor<'a>,
}
/// The real implementation holds selected native cut/input, schema worker,
/// metadata/history route and access/currentness frames. resolve executes the
/// actual registered schema and required membership/exact history. All physical
/// I/O/schema work uses its original host budget/deadline and the supplied TS
/// sub-budget. No callback bool, source-list completeness or issuer is accepted.
/// Missing real registration/frames must refuse; no fabricated owner is provided.
pub trait NativeTsSourceOwner {
    fn verify_current(
        &self,
        selection: &NativeTsSelection,
        budget: &mut NativeTsSession<'_>,
    ) -> Result<()>;
    fn resolve<'a>(
        &'a self,
        selection: &NativeTsSelection,
        request: NativeTsRequest<'_>,
        budget: &mut NativeTsSession<'_>,
    ) -> Result<SelectedNativeTsRecord<'a>>;
}
pub struct NativeTsDefinition<'a> {
    pub definition_kind: &'a str,
    pub domain: &'a str,
    pub rule: &'a str,
    pub premises: Vec<&'a str>,
}
enum Body<'a> {
    Definition(NativeTsDefinition<'a>),
    Profile(TsProfileBody),
    Value {
        codomain: TsCodomain,
        value: TsOutputValue,
    },
}
struct Entry<'a> {
    source: NativeTsRecord<'a>,
    body: Body<'a>,
}
pub struct NativeTsStore<'a> {
    owner: &'a dyn NativeTsSourceOwner,
    selection: &'a NativeTsSelection,
    entries: Vec<Entry<'a>>,
    budget: &'a RefCell<NativeTsSession<'a>>,
}
fn definition<'a>(v: &'a JsonValue, s: &mut NativeTsSession<'_>) -> Result<NativeTsDefinition<'a>> {
    closed(v, &["definition_kind", "specification"], s)?;
    let definition_kind = text(field(v, "definition_kind", s)?, s)?;
    if definition_kind != "bounded-ts-rule-v1" {
        return Err(DomainError::Unsupported("native TS definition kind"));
    }
    let specification = field(v, "specification", s)?;
    closed(specification, &["domain", "rule", "premises"], s)?;
    let domain = text(field(specification, "domain", s)?, s)?;
    let rule = text(field(specification, "rule", s)?, s)?;
    let supported = match domain {
        "rational-linear-order" => matches!(
            rule,
            "exact-rational-arithmetic"
                | "span-membership-meet"
                | "allen-endpoint-order"
                | "dense-q-order-conjunction"
        ),
        "rational-cartesian-2d" => rule == "rectangle-point-topology",
        _ => false,
    };
    if !supported {
        return Err(DomainError::Unsupported("native TS definition/rule"));
    }
    let raw = field(specification, "premises", s)?
        .as_array()
        .ok_or(DomainError::Invalid("native TS premise list"))?;
    if raw.is_empty() {
        return Err(DomainError::Invalid("native TS empty premises"));
    }
    let mut premises = Vec::new();
    for p in raw {
        let p = text(p, s)?;
        if p.is_empty() {
            return Err(DomainError::Invalid("native TS empty premise"));
        }
        push(s, &mut premises, p)?;
    }
    Ok(NativeTsDefinition {
        definition_kind,
        domain,
        rule,
        premises,
    })
}
fn selection_matches(
    a: &NativeTsSelection,
    b: &NativeTsSelection,
    s: &mut NativeTsSession<'_>,
) -> Result<()> {
    for r in [
        &a.selection_ref,
        &b.selection_ref,
        &a.entity_registry_ref,
        &b.entity_registry_ref,
        &a.role_registry_ref,
        &b.role_registry_ref,
    ] {
        s.exact(r)?;
    }
    if a != b {
        return Err(DomainError::Incompatible(
            "native TS selected source/schema binding",
        ));
    }
    Ok(())
}
struct Wire<'a> {
    id: &'a str,
    version: u64,
    digest: Digest256,
}
fn wire<'a>(v: &'a JsonValue, s: &mut NativeTsSession<'_>) -> Result<Wire<'a>> {
    closed(v, &["id", "version", "digest"], s)?;
    let id = text(field(v, "id", s)?, s)?;
    let number = match field(v, "version", s)? {
        JsonValue::Number(n) if n.kind == tos_foundation::JsonNumberKind::Int => n,
        _ => return Err(DomainError::Invalid("native TS wire version")),
    };
    s.work(number.lexeme.len())?;
    if number.lexeme.is_empty()
        || number.lexeme.starts_with('0')
        || !number.lexeme.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(DomainError::Invalid("native TS wire version grammar"));
    }
    let version = number
        .lexeme
        .parse::<u64>()
        .map_err(|_| DomainError::NumericRange)?;
    let digest = Digest256::from_prefixed(text(field(v, "digest", s)?, s)?)
        .map_err(|_| DomainError::Invalid("native TS wire digest"))?;
    Ok(Wire {
        id,
        version,
        digest,
    })
}
impl<'a> NativeTsStore<'a> {
    /// This explicit list is a cache request, not a complete candidate/world/
    /// source universe. Unknown exact references never become proved absence.
    pub fn prepare(
        owner: &'a dyn NativeTsSourceOwner,
        selection: &'a NativeTsSelection,
        requests: &'a [NativeTsRequest<'a>],
        session: &'a RefCell<NativeTsSession<'a>>,
    ) -> Result<Self> {
        // The host retains the original ledger even if preparation fails.
        let mut budget = session
            .try_borrow_mut()
            .map_err(|_| DomainError::ResourceLimit)?;
        budget.check()?;
        owner.verify_current(selection, &mut budget)?;
        let mut entries: Vec<Entry<'a>> = Vec::new();
        for request in requests {
            budget.item()?;
            budget.exact(request.exact)?;
            for e in &entries {
                budget.exact(e.source.exact())?;
                if e.source.exact() == request.exact {
                    return Err(DomainError::Invalid("duplicate native TS exact request"));
                }
            }
            owner.verify_current(selection, &mut budget)?;
            let selected: SelectedNativeTsRecord<'a> =
                owner.resolve(selection, *request, &mut budget)?;
            selection_matches(selection, selected.selection, &mut budget)?;
            let d = selected.descriptor;
            budget.work(
                d.schema_resource
                    .len()
                    .checked_add(d.schema_version.len())
                    .and_then(|n| n.checked_add(d.record_type.len()))
                    .ok_or(DomainError::ResourceLimit)?,
            )?;
            if d.schema_resource != request.role.schema_resource() {
                return Err(DomainError::Incompatible(
                    "native TS registered schema resource/role",
                ));
            }
            let source = budget.bind(selected.document, request.exact)?;
            let root = source.document().root();
            if text(field(root, "schema_version", &mut budget)?, &mut budget)? != d.schema_version
                || text(field(root, "record_type", &mut budget)?, &mut budget)? != d.record_type
            {
                return Err(DomainError::Incompatible(
                    "native TS actual Record descriptor",
                ));
            }
            let raw_body = field(root, "body", &mut budget)?;
            // First map: native whole Record -> decoded /body.
            let body = match request.role {
                NativeTsRole::Definition => Body::Definition(definition(raw_body, &mut budget)?),
                NativeTsRole::ValueProfile => {
                    Body::Profile(codec::decode_profile_body(raw_body, &mut budget)?)
                }
                NativeTsRole::OperationResult => {
                    let (codomain, value) = codec::decode_result_body(raw_body, &mut budget)?;
                    Body::Value { codomain, value }
                }
            };
            push(&mut budget, &mut entries, Entry { source, body })?;
            owner.verify_current(selection, &mut budget)?;
        }
        owner.verify_current(selection, &mut budget)?;
        drop(budget);
        Ok(Self {
            owner,
            selection,
            entries,
            budget: session,
        })
    }
    fn find(
        &self,
        reference: &JsonValue,
        role: NativeTsRole,
        s: &mut NativeTsSession<'_>,
    ) -> Result<usize> {
        let w = wire(reference, s)?;
        for (i, e) in self.entries.iter().enumerate() {
            let r = e.source.exact();
            s.exact(r)?;
            if w.id != r.id.as_str() || w.version != r.version.get() {
                continue;
            }
            if w.digest != r.digest {
                return Err(DomainError::Incompatible(
                    "native TS requested exact digest",
                ));
            }
            let actual = match &e.body {
                Body::Definition(_) => NativeTsRole::Definition,
                Body::Profile(_) => NativeTsRole::ValueProfile,
                Body::Value { .. } => NativeTsRole::OperationResult,
            };
            if actual != role {
                return Err(DomainError::Incompatible("native TS exact record role"));
            }
            return Ok(i);
        }
        Err(DomainError::NeedsBinding(
            "native TS exact record not prepared",
        ))
    }
    fn query(
        &self,
        reference: &JsonValue,
        role: NativeTsRole,
        limits: DomainLimits,
    ) -> Result<usize> {
        let mut b = self
            .budget
            .try_borrow_mut()
            .map_err(|_| DomainError::ResourceLimit)?;
        b.call(limits, |s| {
            self.owner.verify_current(self.selection, s)?;
            let i = self.find(reference, role, s)?;
            self.owner.verify_current(self.selection, s)?;
            Ok(i)
        })
    }
    fn fence(&self) -> Result<()> {
        let mut b = self
            .budget
            .try_borrow_mut()
            .map_err(|_| DomainError::ResourceLimit)?;
        self.owner.verify_current(self.selection, &mut b)
    }
    /// Current-use wrapper fences the ENTIRE K0/value/profile check as well as
    /// both individual resolutions. Historical immutable borrows are no grant.
    pub fn check_output<'s>(
        &'s self,
        run: &CheckedOperationBinding<'_>,
        expected: TsCodomain,
        limits: OutputCheckLimits,
    ) -> Result<CheckedTsOutput<'s>> {
        self.fence()?;
        self.budget
            .try_borrow_mut()
            .map_err(|_| DomainError::ResourceLimit)?
            .reserve_checker(limits.local)?;
        let out = check_ts_output(run, Some(self), expected, limits)?;
        self.fence()?;
        Ok(out)
    }
    pub fn native_record(
        &self,
        reference: &JsonValue,
        role: NativeTsRole,
        limits: DomainLimits,
    ) -> Result<&JsonDocument> {
        let i = self.query(reference, role, limits)?;
        Ok(self.entries[i].source.document())
    }
    pub fn definition(
        &self,
        reference: &JsonValue,
        limits: DomainLimits,
    ) -> Result<&NativeTsDefinition<'a>> {
        let i = self.query(reference, NativeTsRole::Definition, limits)?;
        match &self.entries[i].body {
            Body::Definition(v) => Ok(v),
            _ => Err(DomainError::Invalid("native TS definition index")),
        }
    }
}
impl TsOutputStore for NativeTsStore<'_> {
    fn resolve_value<'a>(
        &'a self,
        reference: &JsonValue,
        limits: DomainLimits,
    ) -> Result<ResolvedRecord<'a, TsOutputValue>> {
        let i = self.query(reference, NativeTsRole::OperationResult, limits)?;
        let e = &self.entries[i];
        match &e.body {
            Body::Value { codomain, value } => {
                if codec::value_codomain(value) != *codomain {
                    return Err(DomainError::Incompatible("native TS decoded codomain"));
                }
                Ok(ResolvedRecord {
                    exact: e.source.exact(),
                    body: value,
                })
            }
            _ => Err(DomainError::Invalid("native TS result index")),
        }
    }
    fn resolve_profile<'a>(
        &'a self,
        reference: &JsonValue,
        limits: DomainLimits,
    ) -> Result<ResolvedRecord<'a, TsProfileBody>> {
        let i = self.query(reference, NativeTsRole::ValueProfile, limits)?;
        let e = &self.entries[i];
        match &e.body {
            Body::Profile(v) => Ok(ResolvedRecord {
                exact: e.source.exact(),
                body: v,
            }),
            _ => Err(DomainError::Invalid("native TS profile index")),
        }
    }
}
