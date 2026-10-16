//! Alignment-owned capability for committing one exact private native record
//! package through the durable private-owner broker.

use crate::durable_private_packages::{
    PrivateOwnerPackageCapabilityV1, PrivateOwnerPackageFamilyV1, PrivateOwnerPackageReplayV1,
    PrivateOwnerPackageSnapshotV1, PrivateOwnerPackageWriteV1, PrivateOwnerScopeV1,
    PrivateOwnerSourceBindingV1, capability_seal, package_digest,
};
use crate::source_command as cmd;
use crate::source_text_alignment_entry::PreparedNativeAlignmentPackage;
use crate::{DurableError, DurableResult};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};

/// Typed authority retained until the broker has checked the source and
/// private-owner fences under its commit lock.
pub(crate) struct PrivateAlignmentPackageCapabilityV1<'a> {
    write: PrivateOwnerPackageWriteV1,
    prepared: PreparedNativeAlignmentPackage,
    software: &'a SoftwareCaptureReader,
    components: &'a SoftwareComponentSelectionV1,
    invocation_path: &'a Path,
    invocation_raw: &'a [u8],
    uid: u32,
    runtime: &'a crate::source_serialization::ExecutableObservation,
    executable_digest: Digest256,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl<'a> PrivateAlignmentPackageCapabilityV1<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_prepared(
        prepared: PreparedNativeAlignmentPackage,
        scope: PrivateOwnerScopeV1,
        expected_current: Option<PrivateOwnerPackageSnapshotV1>,
        source_binding: PrivateOwnerSourceBindingV1,
        request: &tos_foundation::JsonValue,
        raw_request: &[u8],
        software: &'a SoftwareCaptureReader,
        components: &'a SoftwareComponentSelectionV1,
        invocation_path: &'a Path,
        invocation_raw: &'a [u8],
        uid: u32,
        runtime: &'a crate::source_serialization::ExecutableObservation,
        executable_digest: Digest256,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> DurableResult<Self> {
        let command_id = cmd::text(request, "command_id").map_err(DurableError::Source)?;
        let canonical_request = cmd::canonical(request).map_err(DurableError::Source)?;
        if Digest256::of_bytes(&canonical_request) != prepared.request_digest()
            || cmd::canonical(&cmd::parse(raw_request).map_err(DurableError::Source)?)
                .map_err(DurableError::Source)?
                != canonical_request
        {
            return Err(DurableError::Conflict(
                "private Alignment request bytes differ from prepared request",
            ));
        }
        if prepared.owner_context_binding() != scope.context_binding {
            return Err(DurableError::Conflict(
                "private Alignment context binding differs from owner scope",
            ));
        }
        let source_path = prepared.source_path();
        let (target_ref, member) = source_path
            .rsplit_once('/')
            .ok_or(DurableError::Invalid("private Alignment target path"))?;
        if !source_path.starts_with("ToS/source-witnesses/owner-local/")
            || member != "native-translation-alignment.v1.json"
            || RelativePath::parse(target_ref).is_err()
            || prepared.files().is_empty()
            || expected_current.as_ref().is_some_and(|current| {
                current.scope != scope
                    || current.target_ref != target_ref
                    || current.family != PrivateOwnerPackageFamilyV1::Alignment
                    || current.private_audit_generation
                        != source_binding.private_owner_audit_generation
            })
        {
            return Err(DurableError::Conflict(
                "private Alignment target or current snapshot differs",
            ));
        }
        let owner_configuration_digest =
            Digest256::from_prefixed(prepared.owner_configuration())
                .map_err(|_| DurableError::Invalid("private Alignment owner configuration"))?;
        let response_bytes = cmd::canonical(prepared.receipt()).map_err(DurableError::Source)?;
        let write = PrivateOwnerPackageWriteV1 {
            scope,
            family: PrivateOwnerPackageFamilyV1::Alignment,
            target_ref: target_ref.to_owned(),
            expected_current,
            members: prepared.files().clone(),
            archives: Vec::new(),
            command_id: command_id.to_owned(),
            request_digest: prepared.request_digest(),
            raw_request_digest: Digest256::of_bytes(raw_request),
            owner_configuration_digest,
            source_binding,
            response_bytes,
        };
        Ok(Self {
            write,
            prepared,
            software,
            components,
            invocation_path,
            invocation_raw,
            uid,
            runtime,
            executable_digest,
            deadline,
            cancelled,
        })
    }
}

impl capability_seal::Sealed for PrivateAlignmentPackageCapabilityV1<'_> {}

impl PrivateOwnerPackageCapabilityV1 for PrivateAlignmentPackageCapabilityV1<'_> {
    fn write(&self) -> &PrivateOwnerPackageWriteV1 {
        &self.write
    }

    fn revalidate(
        &mut self,
        current: Option<&PrivateOwnerPackageSnapshotV1>,
        replay: Option<&PrivateOwnerPackageReplayV1>,
    ) -> DurableResult<()> {
        if self.prepared.replayed() != replay.is_some() {
            return Err(DurableError::Conflict(
                "private Alignment replay state differs from owner recovery",
            ));
        }
        if replay.is_none() && current != self.write.expected_current.as_ref() {
            return Err(DurableError::Conflict(
                "private Alignment current package changed",
            ));
        }
        if let Some(current) = current {
            if current.scope != self.write.scope
                || current.target_ref != self.write.target_ref
                || current.family != PrivateOwnerPackageFamilyV1::Alignment
            {
                return Err(DurableError::Conflict(
                    "private Alignment package identity changed",
                ));
            }
        }
        if let Some(replay) = replay {
            if current.map(|snapshot| snapshot.package_digest) != Some(replay.package_digest)
                || replay.package_digest != package_digest(self.prepared.files())?
                || replay.response_bytes != self.write.response_bytes
            {
                return Err(DurableError::Conflict(
                    "private Alignment replay receipt differs from prepared package",
                ));
            }
        }
        let invocation = crate::source_text_owner::read_absolute(
            self.invocation_path,
            self.uid,
            true,
            self.invocation_raw.len(),
            self.deadline,
            self.cancelled,
        )
        .map_err(DurableError::Source)?;
        if invocation != self.invocation_raw {
            return Err(DurableError::Conflict(
                "private Alignment invocation changed during commit",
            ));
        }
        if self
            .runtime
            .current_digest(self.deadline, self.cancelled)
            .map_err(DurableError::Source)?
            != self.executable_digest
        {
            return Err(DurableError::Conflict(
                "private Alignment executable changed during commit",
            ));
        }
        self.prepared
            .verify_current(
                self.software,
                self.components,
                self.deadline,
                self.cancelled,
            )
            .map_err(DurableError::Source)
    }
}
