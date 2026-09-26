//! Source proposal -> exact candidate-cut validation -> explicit commit gate.
//!
//! Staging a candidate uses the existing authored source snapshot carrier. A
//! scoped report, schema receipt or selected byte cut cannot activate this
//! coordinator's canonical writer.

use crate::source_command::{
    CommandContext, PreparedCommand, SourceCommandError, SourceCommandResult,
};
use std::sync::atomic::AtomicBool;
use tos_foundation::{Digest256, RelativePath};
use tos_source_store::CorpusCutReader;
use tos_validation::operation::{
    BoundOperation, OperationChange, OperationLimits, OperationProposal, OperationRefusal,
    bind_operation_from_cut,
};

#[derive(Debug)]
pub enum SourceOperationError {
    Command(SourceCommandError),
    Validation(OperationRefusal),
    MissingFullSourceAdmission,
}

/// Private constructor: binds the exact command-produced raw delta to VAL's
/// complete candidate carrier traversal. This still has no production issuer.
pub struct BoundSourceCommand {
    command: PreparedCommand,
    binding: BoundOperation,
}
impl BoundSourceCommand {
    pub fn command(&self) -> &PreparedCommand {
        &self.command
    }
    pub fn binding(&self) -> &BoundOperation {
        &self.binding
    }
    pub fn commit(&self) -> Result<(), SourceOperationError> {
        // General ToS source closure/global invariants, private complete VAL
        // attestation, and an atomic fresh protected owner fence are absent.
        // The synthetic PG attestation API must never fill those positions.
        Err(SourceOperationError::MissingFullSourceAdmission)
    }
}

pub fn bind_selected_candidate(
    context: &CommandContext,
    command: PreparedCommand,
    candidate: &CorpusCutReader,
    protected_configuration_locator: RelativePath,
    limits: OperationLimits,
    cancelled: &AtomicBool,
) -> Result<BoundSourceCommand, SourceOperationError> {
    let checked: SourceCommandResult<_> = context.plan(
        &command.handler_id,
        command.response.clone(),
        command.changes.clone(),
        command.replayed,
    );
    let checked = checked.map_err(SourceOperationError::Command)?;
    if checked != command {
        return Err(SourceOperationError::Command(SourceCommandError::Conflict(
            "command proposal differs from exact request/configuration/source inputs",
        )));
    }
    if command.changes.is_empty() || command.replayed {
        return Err(SourceOperationError::Command(SourceCommandError::Invalid(
            "read-only or replay result has no new candidate delta",
        )));
    }
    let proposal = OperationProposal {
        handler_id: command.handler_id.clone(),
        operation: command.operation.clone(),
        base_revision: command.base_revision,
        candidate_revision: candidate.current().revision(),
        request_raw: context.request_raw.clone(),
        request_canonical_sha256: command.request_canonical_sha256,
        configuration_path: protected_configuration_locator,
        configuration_raw: context.configuration_raw.clone(),
        configuration_raw_sha256: command.configuration_raw_sha256,
        configuration_canonical_sha256: command.configuration_canonical_sha256,
        changes: command
            .changes
            .iter()
            .map(|change| OperationChange {
                path: change.path.clone(),
                before: change.before,
                after: change.after.as_ref().map(|raw| Digest256::of_bytes(raw)),
            })
            .collect(),
    };
    let binding = bind_operation_from_cut(candidate, &proposal, limits, cancelled)
        .map_err(SourceOperationError::Validation)?;
    Ok(BoundSourceCommand { command, binding })
}
