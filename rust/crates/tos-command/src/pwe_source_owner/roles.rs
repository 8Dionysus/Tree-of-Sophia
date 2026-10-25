//! Actual C4 role cache and a separate exact non-formal source reader port.
//! C4 owns role registration/history; PWE checks application body semantics.
use super::{HeldFrames, invalid, unsupported};
use crate::source_command::{SourceCommandResult, SourceDependency};
use serde_json::Value;
use tos_compiler::pwe_native::{BasisSlot, Session};
use tos_foundation::{Digest256, JsonValue};

pub(crate) struct ResolvedDependency {
    pub document: JsonValue,
    pub source_path: String,
    pub schema_path: String,
    pub identity_field: &'static str,
    pub version_field: &'static str,
    pub dependencies: Vec<SourceDependency>,
}
/// Exact non-formal source dependencies only: source Claims, metadata,
/// registries and SVB occurrences. The common source owner supplies this port.
/// Formal roles MUST pass through source_native_roles, not through this port.
pub(crate) trait ExactDependencyReader {
    /// Recheck the ACTUAL retained source/history/catalogue/publication
    /// closure through its native owner. No successful default or caller bool.
    /// Borrow the caller's SAME original source Session; neither method may
    /// create a replacement Session/payer or retain this mutable loan.
    fn verify_closure(
        &mut self,
        resolved: &ResolvedDependency,
        frames: &mut HeldFrames<'_, '_>,
        session: &mut Session<'_>,
    ) -> SourceCommandResult<()>;
    fn basis(
        &mut self,
        slot: &BasisSlot<'_>,
        frames: &mut HeldFrames<'_, '_>,
        session: &mut Session<'_>,
    ) -> SourceCommandResult<ResolvedDependency>;
}
impl ResolvedDependency {
    /// Content/schema checks only. Native entry points must first call the
    /// required owner verify_closure; the observed list is not completeness.
    pub(super) fn verify_contents(
        &self,
        reference: &Value,
        frames: &mut HeldFrames<'_, '_>,
    ) -> SourceCommandResult<()> {
        if self.dependencies.is_empty() || self.dependencies.len() > super::MAX_RECORDS {
            return Err(unsupported("PWE resolved record derivation closure"));
        }
        let mut origin_present = false;
        for dependency in &self.dependencies {
            origin_present |= frames.text_equal(dependency.path.as_str(), &self.source_path)?;
        }
        if !origin_present {
            return Err(invalid("PWE record origin not in derivation closure"));
        }
        frames.exact_match(
            reference,
            &self.document,
            self.identity_field,
            self.version_field,
        )?;
        frames.check_schema(&self.source_path, &self.document, &self.schema_path)?;
        crate::source_creation_store::active(frames.deadline, frames.cancelled)
    }
    pub(super) fn into_serde(self, frames: &mut HeldFrames<'_, '_>) -> SourceCommandResult<Value> {
        frames.domain_value(&self.document)
    }
}

use crate::source_native_roles::{NativeRole, NativeRoleRecord, RoleRegistrySelection};
use tos_foundation::ExactRecordRef;
/// Cache of private-constructed common results, keyed by full exact refs and
/// role. No descriptor, request or manifest can construct NativeRoleRecord.
pub(crate) struct RoleCache {
    registry_exact: ExactRecordRef,
    kind_binding: ExactRecordRef,
    records: Vec<NativeRoleRecord>,
}
impl RoleCache {
    pub(crate) fn new(
        selection: RoleRegistrySelection<'_>,
        kind_binding: &ExactRecordRef,
        frames: &mut HeldFrames<'_, '_>,
    ) -> SourceCommandResult<Self> {
        if !frames.text_equal(selection.path, super::registry::BINDINGS)? {
            return Err(invalid("PWE selected role registry path"));
        }
        frames.inspect_access(
            selection
                .exact
                .id
                .as_str()
                .len()
                .checked_add(kind_binding.id.as_str().len())
                .ok_or(unsupported("PWE role pin copy size"))?,
            1,
        )?;
        frames.retain(
            selection
                .exact
                .id
                .as_str()
                .len()
                .checked_add(kind_binding.id.as_str().len())
                .and_then(|n| n.checked_add(256))
                .ok_or(unsupported("PWE role pin holders"))?,
        )?;
        Ok(Self {
            registry_exact: selection.exact.clone(),
            kind_binding: kind_binding.clone(),
            records: Vec::new(),
        })
    }
    pub(crate) fn value(
        &mut self,
        frames: &mut HeldFrames<'_, '_>,
        wire: &Value,
        role: NativeRole,
    ) -> SourceCommandResult<Value> {
        let record_wire = frames.get(wire, "record")?;
        let (record, kind) = if let Some(record_wire) = record_wire {
            if wire.as_object().map(|o| o.len()) != Some(2) {
                return Err(invalid("PWE TypedRef shape"));
            }
            let kind_wire = frames.field(wire, "kind_binding")?;
            (frames.exact(record_wire)?, frames.exact(kind_wire)?)
        } else {
            frames.inspect_access(self.kind_binding.id.as_str().len(), 1)?;
            frames.retain(
                self.kind_binding
                    .id
                    .as_str()
                    .len()
                    .checked_add(128)
                    .ok_or(unsupported("PWE role kind holder"))?,
            )?;
            (frames.exact(wire)?, self.kind_binding.clone())
        };
        if !frames.exact_equal(&kind, &self.kind_binding)? {
            return Err(crate::source_command::SourceCommandError::Conflict(
                "PWE formal kind registry pin differs",
            ));
        }
        let mut cached = None;
        for (index, candidate) in self.records.iter().enumerate() {
            frames.inspect_access(0, 1)?;
            if candidate.role() == role
                && frames.exact_equal(candidate.exact(), &record)?
                && frames.exact_equal(candidate.kind_binding(), &kind)?
            {
                cached = Some(index);
                break;
            }
        }
        let index = if let Some(index) = cached {
            index
        } else {
            if self.records.len() >= super::MAX_RECORDS {
                return Err(unsupported("PWE formal role cache cap"));
            }
            frames.retain(8192)?;
            let selected = frames.with_roles(
                RoleRegistrySelection {
                    path: super::registry::BINDINGS,
                    exact: &self.registry_exact,
                },
                |r| r.resolve(role, &kind, &record),
            )?;
            frames.retain(selected.evidence().returned_state_bytes)?;
            self.records
                .try_reserve_exact(1)
                .map_err(|_| unsupported("PWE formal role cache reservation"))?;
            self.records.push(selected);
            self.records.len() - 1
        };
        let selected = &self.records[index];
        if selected.source_revision() != frames.revision() {
            return Err(crate::source_command::SourceCommandError::Conflict(
                "PWE role cache source revision differs",
            ));
        }
        frames.domain_value(&selected.evidence().record)
    }
}
