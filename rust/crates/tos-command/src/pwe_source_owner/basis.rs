//! Ordered exact closure shared by preparation/validation and the native host.
//! Physical content is deduplicated only by the complete exact ref; logical
//! slots, roles, indices and multiplicity are retained in their original order.
use super::{
    ExactDependencyReader, HeldFrames, MAX_RECORDS, RoleCache, invalid, refuse, unsupported,
};
use crate::source_command::SourceCommandResult;
use serde_json::{Value, json};
use tos_compiler::pwe_native::{PreparedAccount, ResolvedBasis, Session, SuppliedExact};

struct Content {
    reference: Value,
    value: Value,
    identity: &'static str,
    version: &'static str,
}
pub(crate) struct NativeBasis {
    contents: Vec<Content>,
    occurrences: Vec<usize>,
}
impl NativeBasis {
    pub(crate) fn capture<R: ExactDependencyReader>(
        frames: &mut HeldFrames<'_, '_>,
        resolver: &mut R,
        roles: &mut RoleCache,
        session: &mut Session<'_>,
        account: &PreparedAccount<'_>,
    ) -> SourceCommandResult<Self> {
        frames.verify()?;
        let slots = session
            .exact_basis(account)
            .map_err(|_| unsupported("PWE exact basis requires all declared bindings"))?;
        if slots.len() > MAX_RECORDS {
            return Err(unsupported("PWE ordered exact basis slot cap"));
        }
        let mut out = Self {
            contents: Vec::new(),
            occurrences: Vec::new(),
        };
        frames.retain(
            slots
                .len()
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or(unsupported("PWE slot reservation overflow"))?,
        )?;
        out.occurrences
            .try_reserve_exact(slots.len())
            .map_err(|_| unsupported("PWE slot reservation"))?;
        let mut previous: Option<(&str, usize, usize, Value)> = None;
        for slot in slots {
            frames.budget.charge_work(1)?;
            frames.retain(
                slot.reference
                    .id()
                    .len()
                    .checked_add(512)
                    .ok_or(unsupported("PWE exact reference holder overflow"))?,
            )?;
            let reference = json!({"id":slot.reference.id(),"version":slot.reference.version(),"digest":slot.reference.digest().to_prefixed()});
            // Formal record slots use only C4's private-constructed result.
            // Kind-registry slots and other dependencies retain the maintained
            // source reader port. Role/order/multiplicity are never discarded.
            let formal = if slot.inner % 2 == 1 {
                match slot.role {
                    "scope" | "scope_ref" => Some(crate::source_native_roles::NativeRole::Scope),
                    "world" | "context_ref" => {
                        Some(crate::source_native_roles::NativeRole::Context)
                    }
                    "signature_ref" => Some(crate::source_native_roles::NativeRole::Signature),
                    "predicate_ref" => Some(crate::source_native_roles::NativeRole::Predicate),
                    "qualification_profile_ref" => {
                        Some(crate::source_native_roles::NativeRole::QualificationProfile)
                    }
                    "criterion" => Some(crate::source_native_roles::NativeRole::CriterionPolicy),
                    _ => None,
                }
            } else {
                None
            };
            if formal.is_none() {
                if let Some(index) = out.contents.iter().position(|c| c.reference == reference) {
                    // One authenticated exact source body may serve repeated
                    // logical occurrences. The occurrence itself is retained;
                    // Session::resolve_basis checks every occurrence again.
                    previous = Some((slot.role, slot.outer, slot.inner, reference));
                    out.occurrences.push(index);
                    continue;
                }
            }
            let (value, identity, version) = if let Some(role) = formal {
                let (last_role, last_outer, last_inner, last_ref) = previous
                    .as_ref()
                    .ok_or(invalid("PWE formal slot kind binding absent"))?;
                if *last_role != slot.role
                    || *last_outer != slot.outer
                    || last_inner.checked_add(1) != Some(slot.inner)
                {
                    return Err(invalid("PWE formal slot kind/record order differs"));
                }
                let typed = json!({"kind_binding":last_ref,"record":&reference});
                (
                    roles.value(frames, &typed, role)?,
                    "record_id",
                    "record_version",
                )
            } else {
                let record = resolver.basis(&slot, frames)?;
                record.verify(&reference, frames)?;
                let identity = record.identity_field;
                let version = record.version_field;
                (record.into_serde(frames)?, identity, version)
            };
            // Exact identity includes version+digest. Same-ID other versions are
            // retained separately; none is rejected or replaced by latest.
            let index = out.contents.iter().position(|c| c.reference == reference);
            previous = Some((slot.role, slot.outer, slot.inner, reference.clone()));
            let index = match index {
                Some(index) => {
                    let old = &out.contents[index];
                    if old.value != value || old.identity != identity || old.version != version {
                        return Err(invalid(
                            "PWE repeated exact dependency content/route differs",
                        ));
                    }
                    index
                }
                None => {
                    frames.retain(
                        std::mem::size_of::<Content>()
                            .checked_add(256)
                            .ok_or(unsupported("PWE exact cache reservation"))?,
                    )?;
                    out.contents
                        .try_reserve_exact(1)
                        .map_err(|_| unsupported("PWE exact cache reservation"))?;
                    out.contents.push(Content {
                        reference,
                        value,
                        identity,
                        version,
                    });
                    out.contents.len() - 1
                }
            };
            out.occurrences.push(index);
        }
        frames.verify()?;
        Ok(out)
    }
    /// The pure metadata facade is not native authority. Its exact record and
    /// registry must both occur in this authenticated native dependency closure.
    pub(super) fn check_metadata(
        &self,
        frames: &mut HeldFrames<'_, '_>,
        metadata: &[tos_compiler::pwe_native::CheckedMetadata<'_>],
    ) -> SourceCommandResult<()> {
        for item in metadata {
            let work = frames
                .encoded_len(item.value())?
                .checked_add(256)
                .and_then(|n| n.checked_mul(self.contents.len().checked_add(1)?))
                .ok_or(unsupported("PWE metadata comparison work overflow"))?;
            frames.budget.charge_work(work)?;
            let r = item.reference();
            let registry = item.registry();
            let matches = |c: &Content, r: tos_compiler::pwe_native::PweRef<'_>| {
                c.reference.get("id").and_then(Value::as_str) == Some(r.id())
                    && c.reference.get("version").and_then(Value::as_u64) == Some(r.version())
                    && c.reference
                        .get("digest")
                        .and_then(Value::as_str)
                        .and_then(|d| tos_foundation::Digest256::from_prefixed(d).ok())
                        == Some(r.digest())
            };
            if !self
                .contents
                .iter()
                .any(|c| matches(c, r) && c.value == *item.value())
                || !self.contents.iter().any(|c| {
                    matches(c, registry)
                        && c.identity == "registry_id"
                        && c.version == "registry_version"
                })
            {
                return Err(unsupported(
                    "PWE metadata/registry is not in native exact dependency closure",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn pins(&self, frames: &mut HeldFrames<'_, '_>) -> SourceCommandResult<Vec<Value>> {
        frames.retain(
            self.occurrences
                .len()
                .checked_mul(std::mem::size_of::<Value>())
                .ok_or(unsupported("PWE basis pins holder overflow"))?,
        )?;
        let mut out = Vec::new();
        out.try_reserve_exact(self.occurrences.len())
            .map_err(|_| unsupported("PWE basis pins reservation"))?;
        for index in &self.occurrences {
            let reference = &self.contents[*index].reference;
            frames.reserve_domain_copy(reference)?;
            out.push(reference.clone());
        }
        Ok(out)
    }
    pub(super) fn matches_pins(
        pins: &[Value],
        basis: &ResolvedBasis<'_>,
        frames: &mut HeldFrames<'_, '_>,
    ) -> SourceCommandResult<bool> {
        for reference in pins {
            let bytes = frames.encoded_len(reference)?;
            frames.budget.charge_work(bytes)?;
        }
        // ResolvedBasis has private construction. Its native digest checker has
        // already checked each full supplied value against this exact pin. We
        // compare ordered pins, not cloned bodies or a bare-ID map.
        Ok(basis.slots().len() == pins.len()
            && basis.slots().iter().zip(pins).all(|(slot, reference)| {
                reference.get("id").and_then(Value::as_str) == Some(slot.slot.reference.id())
                    && reference.get("version").and_then(Value::as_u64)
                        == Some(slot.slot.reference.version())
                    && reference
                        .get("digest")
                        .and_then(Value::as_str)
                        .and_then(|d| tos_foundation::Digest256::from_prefixed(d).ok())
                        == Some(slot.slot.reference.digest())
            }))
    }
    /// Session::resolve_basis performs its existing exact record check again.
    /// Cache order must equal extractor order; no bare-ID map is substituted.
    pub(crate) fn bind<'v>(
        &'v self,
        session: &mut Session<'_>,
        account: &PreparedAccount<'v>,
    ) -> tos_compiler::pwe_native::Result<ResolvedBasis<'v>> {
        let mut index = 0;
        let result = session.resolve_basis(account, |requested| {
            let slot = *self
                .occurrences
                .get(index)
                .ok_or_else(|| refuse(invalid("PWE basis occurrence absent")))?;
            index += 1;
            let content = self
                .contents
                .get(slot)
                .ok_or_else(|| refuse(invalid("PWE basis content absent")))?;
            if content.reference.get("id").and_then(Value::as_str) != Some(requested.id())
                || content.reference.get("version").and_then(Value::as_u64)
                    != Some(requested.version())
                || content
                    .reference
                    .get("digest")
                    .and_then(Value::as_str)
                    .and_then(|d| tos_foundation::Digest256::from_prefixed(d).ok())
                    != Some(requested.digest())
            {
                return Err(tos_compiler::pwe_native::Error::ExactMismatch);
            }
            Ok(SuppliedExact {
                value: &content.value,
                identity_field: content.identity,
                version_field: content.version,
            })
        })?;
        if index != self.occurrences.len() {
            return Err(tos_compiler::pwe_native::Error::ExactMismatch);
        }
        Ok(result)
    }
}
