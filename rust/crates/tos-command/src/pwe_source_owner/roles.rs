//! Actual C4 role cache and a separate exact non-formal source reader port.
//! C4 owns role registration/history; PWE checks application body semantics.
use super::{HeldFrames, MAX_BYTES, canonical, exact_match, invalid, unsupported};
use crate::source_command::{SourceCommandResult, SourceDependency};
use serde_json::Value;
use tos_compiler::pwe_native::BasisSlot;
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
    fn basis(
        &mut self,
        slot: &BasisSlot<'_>,
        frames: &mut HeldFrames<'_, '_>,
    ) -> SourceCommandResult<ResolvedDependency>;
}
impl ResolvedDependency {
    pub(super) fn verify(
        &self,
        reference: &Value,
        frames: &mut HeldFrames<'_, '_>,
    ) -> SourceCommandResult<()> {
        if self.dependencies.is_empty() || self.dependencies.len() > super::MAX_RECORDS {
            return Err(unsupported("PWE resolved record derivation closure"));
        }
        let mut origin_present = false;
        for dependency in &self.dependencies {
            let raw = frames.selected_bytes(dependency.path.as_str())?;
            if Digest256::of_bytes(&raw) != dependency.raw_sha256 {
                return Err(crate::source_command::SourceCommandError::Conflict(
                    "PWE exact derivation source bytes changed",
                ));
            }
            origin_present |= dependency.path.as_str() == self.source_path;
        }
        if !origin_present {
            return Err(invalid("PWE record origin not in derivation closure"));
        }
        frames
            .whole
            .try_borrow()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .check_live(MAX_BYTES)?;
        exact_match(
            reference,
            &self.document,
            self.identity_field,
            self.version_field,
        )?;
        frames.check_schema(&self.source_path, &self.document, &self.schema_path)?;
        crate::source_creation_store::active(frames.deadline, frames.cancelled)
    }
    pub(super) fn into_serde(self, frames: &mut HeldFrames<'_, '_>) -> SourceCommandResult<Value> {
        let raw = canonical(&self.document)?;
        // Conservative logical conversion holder reservation; no RSS theorem.
        // The original native document is still held during conversion.
        let fee = raw
            .len()
            .checked_mul(64)
            .and_then(|n| n.checked_add(256))
            .ok_or(unsupported("PWE conversion state overflow"))?;
        frames.retain(fee)?;
        if raw.len() > MAX_BYTES {
            return Err(unsupported("PWE record conversion byte cap"));
        }
        serde_json::from_slice(&raw).map_err(|_| invalid("PWE native-to-domain conversion"))
    }
}

use crate::source_native_roles::{NativeRole, NativeRoleRecord, RoleRegistrySelection};
use tos_foundation::{ExactRecordRef, RecordVersion, StableId};
pub(crate) fn exact(value: &Value) -> SourceCommandResult<ExactRecordRef> {
    let o = value
        .as_object()
        .filter(|o| o.len() == 3)
        .ok_or(invalid("PWE exact reference shape"))?;
    Ok(ExactRecordRef {
        id: StableId::parse(
            o.get("id")
                .and_then(Value::as_str)
                .ok_or(invalid("PWE exact ID"))?,
        )
        .map_err(|_| invalid("PWE exact ID"))?,
        version: RecordVersion::new(
            o.get("version")
                .and_then(Value::as_u64)
                .ok_or(invalid("PWE exact version"))?,
        )
        .map_err(|_| invalid("PWE exact version"))?,
        digest: Digest256::from_prefixed(
            o.get("digest")
                .and_then(Value::as_str)
                .ok_or(invalid("PWE exact digest"))?,
        )
        .map_err(|_| invalid("PWE exact digest"))?,
    })
}
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
    ) -> SourceCommandResult<Self> {
        if selection.path != super::registry::BINDINGS {
            return Err(invalid("PWE selected role registry path"));
        }
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
        let (record, kind) = if wire.get("record").is_some() {
            if wire.as_object().map(|o| o.len()) != Some(2) {
                return Err(invalid("PWE TypedRef shape"));
            }
            (exact(&wire["record"])?, exact(&wire["kind_binding"])?)
        } else {
            (exact(wire)?, self.kind_binding.clone())
        };
        if kind != self.kind_binding {
            return Err(crate::source_command::SourceCommandError::Conflict(
                "PWE formal kind registry pin differs",
            ));
        }
        let work = super::encoded_len(wire)?
            .checked_add(256)
            .and_then(|n| n.checked_mul(self.records.len().checked_add(1)?))
            .ok_or(unsupported("PWE role cache comparison work overflow"))?;
        frames.budget.charge_work(work)?;
        let index = if let Some(i) = self
            .records
            .iter()
            .position(|r| r.role() == role && r.exact() == &record && r.kind_binding() == &kind)
        {
            i
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
