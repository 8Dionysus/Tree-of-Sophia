//! Existing owner-local initial TextLayer preparation: selected delegation,
//! exact metadata/rights, one acquired EPUB member, and pure XHTML text.
//! Publication and retained package authority are separate later stages.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_sign_native::{
    NativeInput, ResolvedInitialTextSource, resolve_initial_owner_text_source,
};
use crate::source_text_layer_payload::{AcquiredMember, read_acquired_epub_member};
use crate::source_text_layer_xml::{extract_xhtml_text, validate_extraction_profile};
use crate::source_text_owner::{OwnerTextContext, OwnerTextInitialLayerSelection};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};
use tos_validation::source_cut::CutWorkerSchemaExecutor;

const MAX_METADATA_BYTES: usize = 8_388_608;
const MAX_METADATA_FILE_BYTES: usize = 1_048_576;

pub(crate) struct PreparedInitialLayerText {
    pub(crate) text: String,
    pub(crate) member: AcquiredMember,
    pub(crate) source: ResolvedInitialTextSource,
}

pub(crate) fn recheck_selected_inputs(
    context: &OwnerTextContext,
    inputs: &[NativeInput],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut remaining = MAX_METADATA_BYTES;
    for input in inputs {
        active(deadline, cancelled)?;
        let bytes = context.read(
            &input.reference,
            remaining.min(MAX_METADATA_FILE_BYTES),
            deadline,
            cancelled,
        )?;
        remaining = remaining
            .checked_sub(bytes.len())
            .ok_or(SourceCommandError::Unsupported(
                "native TextLayer selected input reread budget",
            ))?;
        if Digest256::of_bytes(&bytes) != input.raw_sha256 {
            return Err(SourceCommandError::Conflict(
                "native TextLayer metadata, schema or rights changed after payload read",
            ));
        }
    }
    context.snapshot(deadline, cancelled)?;
    Ok(())
}

pub(crate) fn prepare_initial_text(
    context: &mut OwnerTextContext,
    worker: &mut CutWorkerSchemaExecutor,
    grant: &OwnerTextInitialLayerSelection,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedInitialLayerText> {
    let selector = validate_extraction_profile(
        cmd::field(&grant.config, "selector")?,
        cmd::field(&grant.config, "policy")?,
    )?;
    let source = resolve_initial_owner_text_source(context, worker, grant, deadline, cancelled)?;
    let member =
        read_acquired_epub_member(context, grant, &source.payload_entry, deadline, cancelled)?;
    let text = extract_xhtml_text(&member.raw, selector, deadline, cancelled)?;
    if text.len() as u64 > cmd::integer(cmd::field(&grant.config, "limits")?, "max_output_bytes")? {
        return Err(SourceCommandError::Unsupported(
            "native TextLayer delegated output byte budget",
        ));
    }
    recheck_selected_inputs(context, &source.inputs, deadline, cancelled)?;
    Ok(PreparedInitialLayerText {
        text,
        member,
        source,
    })
}

pub(crate) fn selected_initial_inputs(prepared: &PreparedInitialLayerText) -> Vec<JsonValue> {
    prepared
        .source
        .inputs
        .iter()
        .map(|input| {
            JsonValue::Array(vec![
                cmd::string(&input.reference),
                cmd::string(input.category),
                cmd::string(&input.raw_sha256.to_hex()),
            ])
        })
        .collect()
}
