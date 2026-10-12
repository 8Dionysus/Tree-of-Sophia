//! Text-owned capability for committing complete private packages through the
//! durable private-owner broker. Preparation stays in the maintained Text
//! entries; this layer binds their immutable output to one selected PG readset.

use crate::durable_private_packages::{
    PrivateOwnerPackageCapabilityV1, PrivateOwnerPackageFamilyV1, PrivateOwnerPackageReplayV1,
    PrivateOwnerPackageSnapshotV1, PrivateOwnerPackageWriteV1, PrivateOwnerScopeV1,
    PrivateOwnerSourceBindingV1, capability_seal, package_digest, private_owner_dependency_digest,
    private_owner_readset_digest,
};
use crate::source_command::{self as cmd, SourceCommandResult};
use crate::source_text_layer_derived_entry::PreparedNativeDerivedTextPackage;
use crate::source_text_layer_entry::PreparedNativeInitialTextPackage;
use crate::source_text_unit_entry::PreparedNativeTextUnitPackage;
use crate::{DurableError, DurableResult};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::Digest256;
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};

pub(crate) enum PreparedPrivateTextPackageV1 {
    Initial(PreparedNativeInitialTextPackage),
    Derived(PreparedNativeDerivedTextPackage),
    TextUnit(PreparedNativeTextUnitPackage),
}

/// Bind the bounded identity predicates used by a private Text preparation.
/// The public digest carries the selected-source fence and exact public
/// token observations; the private side contributes its owner-scope audit
/// generation and the precise target/ancestry exclusions used by the broker.
pub(crate) fn private_text_identity_predicate_digest(
    public_snapshot: Digest256,
    private_audit_generation: u64,
    requested: &BTreeSet<String>,
    excluded_target: &str,
    ancestry: &BTreeMap<String, Digest256>,
) -> DurableResult<Digest256> {
    if tos_foundation::RelativePath::parse(excluded_target).is_err()
        || !excluded_target.starts_with("ToS/source-witnesses/owner-local/")
        || requested.iter().any(|token| token.is_empty())
        || ancestry.keys().any(|path| {
            tos_foundation::RelativePath::parse(path).is_err()
                || !path.starts_with("ToS/source-witnesses/owner-local/")
        })
    {
        return Err(DurableError::Invalid(
            "private Text identity predicate selection",
        ));
    }
    let tokens = tos_foundation::JsonValue::Array(
        requested.iter().map(|token| cmd::string(token)).collect(),
    );
    let ancestry = cmd::object(
        ancestry
            .iter()
            .map(|(path, digest)| (path.as_str(), cmd::string(&digest.to_prefixed())))
            .collect(),
    );
    let value = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_private_text_identity_predicate_snapshot_v1"),
        ),
        (
            "public_snapshot_sha256",
            cmd::string(&public_snapshot.to_prefixed()),
        ),
        (
            "private_audit_generation",
            cmd::number(private_audit_generation),
        ),
        ("requested_tokens", tokens),
        ("excluded_target", cmd::string(excluded_target)),
        ("allowed_private_ancestry", ancestry),
    ]);
    let raw = cmd::canonical(&value).map_err(DurableError::Source)?;
    Ok(Digest256::of_bytes(&raw))
}

impl PreparedPrivateTextPackageV1 {
    fn source_path(&self) -> &str {
        match self {
            Self::Initial(value) => value.source_path(),
            Self::Derived(value) => value.source_path(),
            Self::TextUnit(value) => value.source_path(),
        }
    }

    fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        match self {
            Self::Initial(value) => value.files(),
            Self::Derived(value) => value.files(),
            Self::TextUnit(value) => value.files(),
        }
    }

    pub(crate) fn replayed(&self) -> bool {
        match self {
            Self::Initial(value) => value.replayed(),
            Self::Derived(value) => value.replayed(),
            Self::TextUnit(value) => value.replayed(),
        }
    }

    fn receipt(&self) -> &tos_foundation::JsonValue {
        match self {
            Self::Initial(value) => value.receipt(),
            Self::Derived(value) => value.receipt(),
            Self::TextUnit(value) => value.receipt(),
        }
    }

    fn owner_configuration(&self) -> &str {
        match self {
            Self::Initial(value) => value.owner_configuration(),
            Self::Derived(value) => value.owner_configuration(),
            Self::TextUnit(value) => value.owner_configuration(),
        }
    }

    fn owner_context_binding(&self) -> Digest256 {
        match self {
            Self::Initial(value) => value.owner_context_binding(),
            Self::Derived(value) => value.owner_context_binding(),
            Self::TextUnit(value) => value.owner_context_binding(),
        }
    }

    fn request_digest(&self) -> Digest256 {
        match self {
            Self::Initial(value) => value.request_digest(),
            Self::Derived(value) => value.request_digest(),
            Self::TextUnit(value) => value.request_digest(),
        }
    }

    fn verify_current(
        &self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        exclude_target: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        match self {
            Self::Initial(value) => {
                value.verify_current(software, components, exclude_target, deadline, cancelled)
            }
            Self::Derived(value) => {
                value.verify_current(software, components, exclude_target, deadline, cancelled)
            }
            Self::TextUnit(value) => {
                value.verify_current(software, components, exclude_target, deadline, cancelled)
            }
        }
    }
}

/// Bind one complete owner-prepared package to the exact current PG readset.
/// The capability's private fields prevent the dispatcher from substituting
/// arbitrary bytes after Text semantics and recovery checks have completed.
pub(crate) struct PrivateTextPackageCapabilityV1<'a> {
    write: PrivateOwnerPackageWriteV1,
    prepared: PreparedPrivateTextPackageV1,
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

impl<'a> PrivateTextPackageCapabilityV1<'a> {
    pub(crate) fn from_prepared(
        prepared: PreparedPrivateTextPackageV1,
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
                "private Text request bytes differ from prepared request",
            ));
        }
        if prepared.owner_context_binding() != scope.context_binding {
            return Err(DurableError::Conflict(
                "private Text context binding differs from owner scope",
            ));
        }
        let source_path = prepared.source_path();
        let (target_ref, member) = source_path
            .rsplit_once('/')
            .ok_or(DurableError::Invalid("private Text target path"))?;
        if !source_path.starts_with("ToS/source-witnesses/owner-local/")
            || member.is_empty()
            || tos_foundation::RelativePath::parse(target_ref).is_err()
            || prepared.files().is_empty()
            || expected_current.as_ref().is_some_and(|current| {
                current.scope != scope
                    || current.target_ref != target_ref
                    || current.family != PrivateOwnerPackageFamilyV1::Text
                    || current.private_audit_generation
                        != source_binding.private_owner_audit_generation
            })
        {
            return Err(DurableError::Conflict(
                "private Text package target or current snapshot differs",
            ));
        }
        let owner_configuration_digest =
            Digest256::from_prefixed(prepared.owner_configuration())
                .map_err(|_| DurableError::Invalid("private Text owner configuration digest"))?;
        let response_bytes = cmd::canonical(prepared.receipt()).map_err(DurableError::Source)?;
        let write = PrivateOwnerPackageWriteV1 {
            scope,
            family: PrivateOwnerPackageFamilyV1::Text,
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

impl capability_seal::Sealed for PrivateTextPackageCapabilityV1<'_> {}

impl PrivateOwnerPackageCapabilityV1 for PrivateTextPackageCapabilityV1<'_> {
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
                "private Text replay state differs from owner recovery",
            ));
        }
        if replay.is_none() && current != self.write.expected_current.as_ref() {
            return Err(DurableError::Conflict(
                "private Text current package changed",
            ));
        }
        if let Some(current) = current {
            if current.scope != self.write.scope
                || current.target_ref != self.write.target_ref
                || current.family != PrivateOwnerPackageFamilyV1::Text
            {
                return Err(DurableError::Conflict(
                    "private Text current package identity changed",
                ));
            }
        }
        if let Some(replay) = replay {
            if current.map(|snapshot| snapshot.package_digest) != Some(replay.package_digest)
                || replay.package_digest != package_digest(self.prepared.files())?
                || replay.response_bytes != self.write.response_bytes
            {
                return Err(DurableError::Conflict(
                    "private Text replay receipt differs from prepared package",
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
                "private Text invocation changed during commit",
            ));
        }
        if self
            .runtime
            .current_digest(self.deadline, self.cancelled)
            .map_err(DurableError::Source)?
            != self.executable_digest
        {
            return Err(DurableError::Conflict(
                "private Text executable changed during commit",
            ));
        }
        self.prepared
            .verify_current(
                self.software,
                self.components,
                self.write.expected_current.is_some(),
                self.deadline,
                self.cancelled,
            )
            .map_err(DurableError::Source)
    }
}

pub(crate) fn private_text_source_binding(
    readset: &crate::source_cohort::revision::SourceRevisionReadSet,
    dependency_members: &BTreeMap<String, Vec<u8>>,
    private_owner_audit_generation: u64,
) -> DurableResult<PrivateOwnerSourceBindingV1> {
    let selected = readset.selection();
    let dependencies = dependency_members
        .iter()
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect::<Vec<_>>();
    Ok(PrivateOwnerSourceBindingV1 {
        head_seq: selected.head_seq,
        source_epoch: selected.epoch,
        source_generation: selected.source_generation,
        source_revision: selected.cohort.initial_revision().0,
        source_audit_generation: selected.audit_generation,
        rights_version: selected.rights_version,
        rule_version: selected.rule_version,
        contract_digest: selected.contract,
        job_id: selected.job_id.clone(),
        job_fence: selected.job_fence,
        readset_digest: private_owner_readset_digest(readset)?,
        dependency_digest: private_owner_dependency_digest(&dependencies)?,
        private_owner_audit_generation,
    })
}
