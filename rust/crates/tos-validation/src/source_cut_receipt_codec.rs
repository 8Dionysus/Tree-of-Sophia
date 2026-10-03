//! Versioned binary codecs for the opt-in source-cut receipt spool.
//!
//! This module is included beneath `source_cut::receipt_spool`, so it can
//! preserve the private diagnostic fields without widening their API.

use super::super::*;

const RECEIPT_MAGIC: &[u8; 4] = b"TSRC";
const DIAGNOSTIC_MAGIC: &[u8; 4] = b"TSDC";
const CODEC_VERSION: u8 = 1;

const RECEIPT_HEADER_BYTES: usize = 4 + 1;
const RECEIPT_FIXED_BYTES: usize = RECEIPT_HEADER_BYTES + (7 * 32) + 3;
const BATCH_BINDING_BYTES: usize = (5 * 32) + 1 + 8 + 8 + 32;

// These mirror the limits applied to diagnostic units before the worker
// accepts them. Keep this codec bounded even if it is asked to decode a
// damaged or externally modified spool row.
const MAX_UNIT_MEMBER_ID_BYTES: usize = 512;
const MAX_UNIT_RELATIVE_PATH_BYTES: usize = 4096;
const MAX_UNIT_ROOT_URI_BYTES: usize = 4096;
const MAX_DIAGNOSTIC_KEYWORD_BYTES: usize = 64;

pub(super) fn encoded_scalar_receipt_len(path: &str, contract: &str) -> Result<usize, ItemRefusal> {
    let path_bytes = encoded_string_len(path.len())?;
    let contract_bytes = encoded_string_len(contract.len())?;
    let total = RECEIPT_FIXED_BYTES
        .checked_add(path_bytes)
        .and_then(|bytes| bytes.checked_add(contract_bytes))
        .ok_or(ItemRefusal::Budget)?;
    ensure_sqlite_length(total)?;
    Ok(total)
}

pub(in crate::source_cut) fn encoded_receipt_len(
    receipt: &CutSchemaReceipt,
) -> Result<usize, ItemRefusal> {
    let total = encoded_scalar_receipt_len(&receipt.path, &receipt.contract)?
        .checked_add(if receipt.batch.is_some() {
            BATCH_BINDING_BYTES
        } else {
            0
        })
        .ok_or(ItemRefusal::Budget)?;
    ensure_sqlite_length(total)?;
    Ok(total)
}

pub(super) fn diagnostic_row_encoded_upper_bound(
    path_len: usize,
    contract_len: usize,
) -> Result<usize, ItemRefusal> {
    let top_level_strings = encoded_string_len(path_len)?
        .checked_add(encoded_string_len(contract_len)?)
        .ok_or(ItemRefusal::Budget)?;
    let unit_strings = (3 * 4usize)
        .checked_add(MAX_UNIT_MEMBER_ID_BYTES)
        .and_then(|bytes| bytes.checked_add(MAX_UNIT_RELATIVE_PATH_BYTES))
        .and_then(|bytes| bytes.checked_add(MAX_UNIT_ROOT_URI_BYTES))
        .ok_or(ItemRefusal::Budget)?;

    // The issue payload is the same bounded representation used by the
    // diagnostics protocol. Reserving its whole cap also covers report
    // framing and the fields retained here from the verified report.
    let report_and_issues = 2usize
        .checked_add(4 * 32)
        .and_then(|bytes| bytes.checked_add(4 + 4 + 2 + 4))
        .and_then(|bytes| bytes.checked_add(1 + 1 + 8 + 1 + 32 + 32 + 4))
        .and_then(|bytes| bytes.checked_add(schema_diagnostics::MAX_REPORT_BYTES_PER_UNIT as usize))
        .ok_or(ItemRefusal::Budget)?;

    // Six checkpoint digests, profile/count/byte counters, and the largest
    // legal optional exceptional-usage vectors.
    let checkpoint = (6 * 32usize)
        .checked_add(1 + 8 + 8 + 8)
        .and_then(|bytes| bytes.checked_add(1 + 8))
        .and_then(|bytes| bytes.checked_add(1 + 9 * 8))
        .and_then(|bytes| bytes.checked_add(1 + 9 * 8))
        .ok_or(ItemRefusal::Budget)?;

    let fixed = (RECEIPT_HEADER_BYTES)
        .checked_add(4 * 32) // source revision and three source/caps digests
        .and_then(|bytes| bytes.checked_add(checkpoint))
        .and_then(|bytes| bytes.checked_add(8 + 2 * 32)) // unit ordinal and digests
        .and_then(|bytes| bytes.checked_add(unit_strings))
        .and_then(|bytes| bytes.checked_add(report_and_issues))
        .and_then(|bytes| bytes.checked_add(12 * 8)) // every byte/cpu/state counter
        .ok_or(ItemRefusal::Budget)?;

    let total = fixed
        .checked_add(top_level_strings)
        .ok_or(ItemRefusal::Budget)?;
    ensure_sqlite_length(total)?;
    Ok(total)
}

pub(super) fn encode_receipt(receipt: &CutSchemaReceipt) -> Result<Vec<u8>, ItemRefusal> {
    let capacity = encoded_receipt_len(receipt)?;
    ensure_sqlite_length(capacity)?;
    let mut writer = Writer::with_capacity(capacity)?;
    writer.header(RECEIPT_MAGIC)?;
    writer.string(&receipt.path)?;
    writer.string(&receipt.contract)?;
    writer.digest(receipt.source_revision.0)?;
    writer.digest(receipt.source_raw_sha256)?;
    writer.digest(receipt.decoded_instance_sha256)?;
    writer.digest(receipt.execution.worker_sha256)?;
    writer.digest(receipt.execution.request_sha256)?;
    writer.digest(receipt.execution.schema_set_sha256)?;
    writer.digest(receipt.execution.instance_sha256)?;
    writer.profile(receipt.execution.profile)?;
    writer.bool(receipt.valid)?;
    match receipt.batch {
        None => writer.u8(0)?,
        Some(binding) => {
            writer.u8(1)?;
            write_batch_checkpoint(&mut writer, binding.checkpoint)?;
            writer.u64(binding.ordinal)?;
            writer.digest(binding.unit_sha256)?;
        }
    }
    writer.finish(capacity)
}

pub(super) fn decode_receipt(bytes: &[u8]) -> Result<CutSchemaReceipt, ItemRefusal> {
    let mut reader = Reader::new(bytes);
    reader.header(RECEIPT_MAGIC)?;
    let path = reader.string(u32::MAX as usize)?;
    let contract = reader.string(u32::MAX as usize)?;
    let source_revision = SourceRevision(reader.digest()?);
    let source_raw_sha256 = reader.digest()?;
    let decoded_instance_sha256 = reader.digest()?;
    let execution = ExecutionIdentity {
        worker_sha256: reader.digest()?,
        request_sha256: reader.digest()?,
        schema_set_sha256: reader.digest()?,
        instance_sha256: reader.digest()?,
        profile: reader.profile()?,
    };
    let valid = reader.bool()?;
    let batch = match reader.u8()? {
        0 => None,
        1 => Some(CutBatchBinding {
            checkpoint: read_batch_checkpoint(&mut reader)?,
            ordinal: reader.u64()?,
            unit_sha256: reader.digest()?,
        }),
        _ => return Err(invalid_payload()),
    };
    reader.finish()?;
    Ok(CutSchemaReceipt {
        path,
        contract,
        source_revision,
        source_raw_sha256,
        decoded_instance_sha256,
        execution,
        valid,
        batch,
    })
}

pub(super) fn encode_diagnostic(diagnostic: &CutSchemaDiagnostic) -> Result<Vec<u8>, ItemRefusal> {
    if !diagnostic.unit.report.is_well_formed()
        || diagnostic.unit.member_id.is_empty()
        || diagnostic.unit.member_id.len() > MAX_UNIT_MEMBER_ID_BYTES
        || diagnostic.unit.relative_path.is_empty()
        || diagnostic.unit.relative_path.len() > MAX_UNIT_RELATIVE_PATH_BYTES
        || diagnostic.unit.root_uri.len() > MAX_UNIT_ROOT_URI_BYTES
    {
        return Err(invalid_payload());
    }
    let capacity = encoded_diagnostic_len(diagnostic)?;
    if capacity
        > diagnostic_row_encoded_upper_bound(diagnostic.path.len(), diagnostic.contract.len())?
    {
        return Err(ItemRefusal::Budget);
    }
    let mut writer = Writer::with_capacity(capacity)?;
    writer.header(DIAGNOSTIC_MAGIC)?;
    writer.string(&diagnostic.path)?;
    writer.string(&diagnostic.contract)?;
    writer.digest(diagnostic.source_revision.0)?;
    writer.digest(diagnostic.source_raw_sha256)?;
    writer.digest(diagnostic.decoded_instance_sha256)?;
    writer.digest(diagnostic.aggregate_caps_sha256)?;
    write_diagnostics_checkpoint(&mut writer, diagnostic.checkpoint)?;
    writer.u64(diagnostic.unit.ordinal)?;
    writer.string(&diagnostic.unit.member_id)?;
    writer.string(&diagnostic.unit.relative_path)?;
    writer.string(&diagnostic.unit.root_uri)?;
    writer.digest(diagnostic.unit.raw_sha256)?;
    writer.digest(diagnostic.unit.unit_sha256)?;
    write_report(&mut writer, &diagnostic.unit.report)?;
    writer.usize(diagnostic.schema_resource_bytes)?;
    writer.usize(diagnostic.schema_resource_buffer_bytes)?;
    writer.usize(diagnostic.input_instance_bytes)?;
    writer.usize(diagnostic.input_instance_buffer_bytes)?;
    writer.usize(diagnostic.input_metadata_bytes)?;
    writer.usize(diagnostic.request_bytes)?;
    writer.usize(diagnostic.request_buffer_bytes)?;
    writer.usize(diagnostic.response_bytes)?;
    writer.usize(diagnostic.response_buffer_bytes)?;
    writer.u64(diagnostic.worker_cpu_micros)?;
    writer.usize(diagnostic.retained_state_bytes)?;
    writer.usize(diagnostic.accounted_state_bytes)?;
    writer.finish(capacity)
}

fn encoded_diagnostic_len(diagnostic: &CutSchemaDiagnostic) -> Result<usize, ItemRefusal> {
    let top_level_strings = encoded_string_len(diagnostic.path.len())?
        .checked_add(encoded_string_len(diagnostic.contract.len())?)
        .ok_or(ItemRefusal::Budget)?;
    let member_id = encoded_string_len(diagnostic.unit.member_id.len())?;
    let relative_path = encoded_string_len(diagnostic.unit.relative_path.len())?;
    let root_uri = encoded_string_len(diagnostic.unit.root_uri.len())?;
    let unit_strings = member_id
        .checked_add(relative_path)
        .and_then(|bytes| bytes.checked_add(root_uri))
        .ok_or(ItemRefusal::Budget)?;
    let report = encoded_report_len(&diagnostic.unit.report)?;
    let checkpoint = encoded_diagnostics_checkpoint_len(diagnostic.checkpoint)?;
    let total = RECEIPT_HEADER_BYTES
        .checked_add(top_level_strings)
        .and_then(|bytes| bytes.checked_add(4 * 32))
        .and_then(|bytes| bytes.checked_add(checkpoint))
        .and_then(|bytes| bytes.checked_add(8 + 2 * 32))
        .and_then(|bytes| bytes.checked_add(unit_strings))
        .and_then(|bytes| bytes.checked_add(report))
        .and_then(|bytes| bytes.checked_add(12 * 8))
        .ok_or(ItemRefusal::Budget)?;
    ensure_sqlite_length(total)?;
    Ok(total)
}

fn encoded_diagnostics_checkpoint_len(
    checkpoint: SchemaDiagnosticsCheckpoint,
) -> Result<usize, ItemRefusal> {
    let worker_cpu = if checkpoint.worker_cpu_micros.is_some() {
        1 + 8
    } else {
        1
    };
    let exceptional_remaining = optional_exceptional_usage_len(checkpoint.exceptional_remaining);
    let exceptional_usage = optional_exceptional_usage_len(checkpoint.exceptional_usage);
    (6 * 32usize)
        .checked_add(1 + 8 + 8 + 8)
        .and_then(|bytes| bytes.checked_add(worker_cpu))
        .and_then(|bytes| bytes.checked_add(exceptional_remaining))
        .and_then(|bytes| bytes.checked_add(exceptional_usage))
        .ok_or(ItemRefusal::Budget)
}

fn optional_exceptional_usage_len(usage: Option<crate::executor::ExceptionalSchemaUsage>) -> usize {
    if usage.is_some() { 1 + 9 * 8 } else { 1 }
}

fn encoded_report_len(report: &schema_diagnostics::Report) -> Result<usize, ItemRefusal> {
    if !report.is_well_formed() {
        return Err(invalid_payload());
    }
    report.issues.iter().try_fold(
        2usize
            .checked_add(4 * 32)
            .and_then(|bytes| bytes.checked_add(4 + 4 + 2 + 4))
            .and_then(|bytes| bytes.checked_add(1 + 1 + 8 + 1 + 32 + 32 + 4))
            .ok_or(ItemRefusal::Budget)?,
        |total, issue| {
            total
                .checked_add(encoded_issue_len(issue)?)
                .ok_or(ItemRefusal::Budget)
        },
    )
}

fn encoded_issue_len(issue: &schema_diagnostics::Issue) -> Result<usize, ItemRefusal> {
    let instance_path = encoded_path_len(&issue.instance_path)?;
    let schema_keyword = encoded_string_len(issue.schema_keyword.len())?;
    let schema_path = encoded_path_len(&issue.schema_path)?;
    instance_path
        .checked_add(schema_keyword)
        .and_then(|bytes| bytes.checked_add(2))
        .and_then(|bytes| bytes.checked_add(schema_path))
        .and_then(|bytes| bytes.checked_add(1))
        .ok_or(ItemRefusal::Budget)
}

fn encoded_path_len(path: &[schema_diagnostics::PathSegment]) -> Result<usize, ItemRefusal> {
    u16::try_from(path.len()).map_err(|_| ItemRefusal::Budget)?;
    path.iter().try_fold(2usize, |total, segment| {
        let segment_len = match segment {
            schema_diagnostics::PathSegment::Property(value) => {
                1usize.checked_add(encoded_string_len(value.len())?)
            }
            schema_diagnostics::PathSegment::Index(_) => Some(1 + 8),
        }
        .ok_or(ItemRefusal::Budget)?;
        total.checked_add(segment_len).ok_or(ItemRefusal::Budget)
    })
}

pub(super) fn decode_diagnostic(bytes: &[u8]) -> Result<CutSchemaDiagnostic, ItemRefusal> {
    let mut reader = Reader::new(bytes);
    reader.header(DIAGNOSTIC_MAGIC)?;
    let path = reader.string(u32::MAX as usize)?;
    let contract = reader.string(u32::MAX as usize)?;
    let source_revision = SourceRevision(reader.digest()?);
    let source_raw_sha256 = reader.digest()?;
    let decoded_instance_sha256 = reader.digest()?;
    let aggregate_caps_sha256 = reader.digest()?;
    let checkpoint = read_diagnostics_checkpoint(&mut reader)?;
    let ordinal = reader.u64()?;
    let member_id = reader.string(MAX_UNIT_MEMBER_ID_BYTES)?;
    let relative_path = reader.string(MAX_UNIT_RELATIVE_PATH_BYTES)?;
    let root_uri = reader.string(MAX_UNIT_ROOT_URI_BYTES)?;
    if member_id.is_empty() || relative_path.is_empty() {
        return Err(invalid_payload());
    }
    let raw_sha256 = reader.digest()?;
    let unit_sha256 = reader.digest()?;
    let report = read_report(&mut reader)?;
    let schema_resource_bytes = reader.usize()?;
    let schema_resource_buffer_bytes = reader.usize()?;
    let input_instance_bytes = reader.usize()?;
    let input_instance_buffer_bytes = reader.usize()?;
    let input_metadata_bytes = reader.usize()?;
    let request_bytes = reader.usize()?;
    let request_buffer_bytes = reader.usize()?;
    let response_bytes = reader.usize()?;
    let response_buffer_bytes = reader.usize()?;
    let worker_cpu_micros = reader.u64()?;
    let retained_state_bytes = reader.usize()?;
    let accounted_state_bytes = reader.usize()?;
    reader.finish()?;
    Ok(CutSchemaDiagnostic {
        path,
        contract,
        source_revision,
        source_raw_sha256,
        decoded_instance_sha256,
        aggregate_caps_sha256,
        checkpoint,
        unit: SchemaDiagnosticUnit {
            ordinal,
            member_id,
            relative_path,
            root_uri,
            raw_sha256,
            unit_sha256,
            report,
        },
        schema_resource_bytes,
        schema_resource_buffer_bytes,
        input_instance_bytes,
        input_instance_buffer_bytes,
        input_metadata_bytes,
        request_bytes,
        request_buffer_bytes,
        response_bytes,
        response_buffer_bytes,
        worker_cpu_micros,
        retained_state_bytes,
        accounted_state_bytes,
    })
}

fn write_batch_checkpoint(
    writer: &mut Writer,
    checkpoint: BatchCoverageCheckpoint,
) -> Result<(), ItemRefusal> {
    writer.digest(checkpoint.worker_sha256)?;
    writer.digest(checkpoint.request_sha256)?;
    writer.profile(checkpoint.profile)?;
    writer.digest(checkpoint.schema_set_sha256)?;
    writer.digest(checkpoint.ordered_manifest_sha256)?;
    writer.u64(checkpoint.completed_count)?;
    writer.digest(checkpoint.result_stream_sha256)
}

fn read_batch_checkpoint(reader: &mut Reader<'_>) -> Result<BatchCoverageCheckpoint, ItemRefusal> {
    Ok(BatchCoverageCheckpoint {
        worker_sha256: reader.digest()?,
        request_sha256: reader.digest()?,
        profile: reader.profile()?,
        schema_set_sha256: reader.digest()?,
        ordered_manifest_sha256: reader.digest()?,
        completed_count: reader.u64()?,
        result_stream_sha256: reader.digest()?,
    })
}

fn write_diagnostics_checkpoint(
    writer: &mut Writer,
    checkpoint: SchemaDiagnosticsCheckpoint,
) -> Result<(), ItemRefusal> {
    writer.digest(checkpoint.worker_sha256)?;
    writer.digest(checkpoint.request_sha256)?;
    writer.profile(checkpoint.profile)?;
    writer.digest(checkpoint.schema_set_sha256)?;
    writer.digest(checkpoint.ordered_manifest_sha256)?;
    writer.digest(checkpoint.caps_sha256)?;
    writer.u64(checkpoint.completed_count)?;
    writer.digest(checkpoint.result_stream_sha256)?;
    writer.u64(checkpoint.worker_request_bytes)?;
    writer.u64(checkpoint.worker_response_bytes)?;
    match checkpoint.worker_cpu_micros {
        None => writer.u8(0)?,
        Some(value) => {
            writer.u8(1)?;
            writer.u64(value)?;
        }
    }
    write_optional_exceptional_usage(writer, checkpoint.exceptional_remaining)?;
    write_optional_exceptional_usage(writer, checkpoint.exceptional_usage)
}

fn read_diagnostics_checkpoint(
    reader: &mut Reader<'_>,
) -> Result<SchemaDiagnosticsCheckpoint, ItemRefusal> {
    let worker_sha256 = reader.digest()?;
    let request_sha256 = reader.digest()?;
    let profile = reader.profile()?;
    let schema_set_sha256 = reader.digest()?;
    let ordered_manifest_sha256 = reader.digest()?;
    let caps_sha256 = reader.digest()?;
    let completed_count = reader.u64()?;
    let result_stream_sha256 = reader.digest()?;
    let worker_request_bytes = reader.u64()?;
    let worker_response_bytes = reader.u64()?;
    let worker_cpu_micros = match reader.u8()? {
        0 => None,
        1 => Some(reader.u64()?),
        _ => return Err(invalid_payload()),
    };
    let exceptional_remaining = read_optional_exceptional_usage(reader)?;
    let exceptional_usage = read_optional_exceptional_usage(reader)?;
    Ok(SchemaDiagnosticsCheckpoint {
        worker_sha256,
        request_sha256,
        profile,
        schema_set_sha256,
        ordered_manifest_sha256,
        caps_sha256,
        completed_count,
        result_stream_sha256,
        worker_request_bytes,
        worker_response_bytes,
        worker_cpu_micros,
        exceptional_remaining,
        exceptional_usage,
    })
}

fn write_optional_exceptional_usage(
    writer: &mut Writer,
    usage: Option<crate::executor::ExceptionalSchemaUsage>,
) -> Result<(), ItemRefusal> {
    let Some(usage) = usage else {
        return writer.u8(0);
    };
    writer.u8(1)?;
    writer.u64(usage.schema_scan_work)?;
    writer.u64(usage.schema_scan_bytes)?;
    writer.u64(usage.pattern_compile_count)?;
    writer.u64(usage.pattern_bytes)?;
    writer.u64(usage.evaluation_work)?;
    writer.u64(usage.evaluation_bytes)?;
    writer.u64(usage.reference_steps)?;
    writer.u64(usage.regex_checks)?;
    writer.u64(usage.regex_bytes)
}

fn read_optional_exceptional_usage(
    reader: &mut Reader<'_>,
) -> Result<Option<crate::executor::ExceptionalSchemaUsage>, ItemRefusal> {
    match reader.u8()? {
        0 => Ok(None),
        1 => Ok(Some(crate::executor::ExceptionalSchemaUsage {
            schema_scan_work: reader.u64()?,
            schema_scan_bytes: reader.u64()?,
            pattern_compile_count: reader.u64()?,
            pattern_bytes: reader.u64()?,
            evaluation_work: reader.u64()?,
            evaluation_bytes: reader.u64()?,
            reference_steps: reader.u64()?,
            regex_checks: reader.u64()?,
            regex_bytes: reader.u64()?,
        })),
        _ => Err(invalid_payload()),
    }
}

fn write_report(
    writer: &mut Writer,
    report: &schema_diagnostics::Report,
) -> Result<(), ItemRefusal> {
    if !report.is_well_formed() {
        return Err(invalid_payload());
    }
    writer.u16(report.protocol_version)?;
    writer.digest(report.worker_sha256)?;
    writer.digest(report.request_sha256)?;
    writer.digest(report.unit_sha256)?;
    writer.digest(report.schema_set_sha256)?;
    writer.u32(report.caps.max_issues_per_unit)?;
    writer.u32(report.caps.max_report_bytes_per_unit)?;
    writer.u16(report.caps.max_path_segments)?;
    writer.u32(report.caps.max_path_bytes)?;
    writer.u8(report.status as u8)?;
    writer.u8(report.failure as u8)?;
    writer.u64(report.total_issue_count)?;
    writer.bool(report.truncated)?;
    writer.digest(report.issues_sha256)?;
    writer.digest(report.report_sha256)?;
    writer.u32(u32::try_from(report.issues.len()).map_err(|_| ItemRefusal::Budget)?)?;
    for issue in &report.issues {
        write_issue(writer, issue)?;
    }
    Ok(())
}

fn read_report(reader: &mut Reader<'_>) -> Result<schema_diagnostics::Report, ItemRefusal> {
    let protocol_version = reader.u16()?;
    let worker_sha256 = reader.digest()?;
    let request_sha256 = reader.digest()?;
    let unit_sha256 = reader.digest()?;
    let schema_set_sha256 = reader.digest()?;
    let caps = schema_diagnostics::Caps {
        max_issues_per_unit: reader.u32()?,
        max_report_bytes_per_unit: reader.u32()?,
        max_path_segments: reader.u16()?,
        max_path_bytes: reader.u32()?,
    };
    if !caps.validate() {
        return Err(invalid_payload());
    }
    let status = schema_diagnostics::Status::from_wire(reader.u8()?).ok_or_else(invalid_payload)?;
    let failure =
        schema_diagnostics::Failure::from_wire(reader.u8()?).ok_or_else(invalid_payload)?;
    let total_issue_count = reader.u64()?;
    let truncated = reader.bool()?;
    let issues_sha256 = reader.digest()?;
    let report_sha256 = reader.digest()?;
    let issue_count = usize::try_from(reader.u32()?).map_err(|_| invalid_payload())?;
    if issue_count > caps.max_issues_per_unit as usize {
        return Err(invalid_payload());
    }
    let mut issues = Vec::new();
    issues
        .try_reserve_exact(issue_count)
        .map_err(|_| ItemRefusal::Budget)?;
    for _ in 0..issue_count {
        issues.push(read_issue(reader, caps)?);
    }
    let report = schema_diagnostics::Report {
        protocol_version,
        worker_sha256,
        request_sha256,
        unit_sha256,
        schema_set_sha256,
        caps,
        status,
        failure,
        total_issue_count,
        truncated,
        issues_sha256,
        report_sha256,
        issues,
    };
    if !report.is_well_formed() {
        return Err(invalid_payload());
    }
    Ok(report)
}

fn write_issue(writer: &mut Writer, issue: &schema_diagnostics::Issue) -> Result<(), ItemRefusal> {
    write_path(writer, &issue.instance_path)?;
    writer.string(&issue.schema_keyword)?;
    writer.u16(issue.reason as u16)?;
    write_path(writer, &issue.schema_path)?;
    writer.u8(issue.compatibility_text.map_or(0, |value| value as u8))?;
    Ok(())
}

fn read_issue(
    reader: &mut Reader<'_>,
    caps: schema_diagnostics::Caps,
) -> Result<schema_diagnostics::Issue, ItemRefusal> {
    let instance_path = read_path(reader, caps)?;
    let schema_keyword = reader.string(MAX_DIAGNOSTIC_KEYWORD_BYTES)?;
    let reason =
        schema_diagnostics::Reason::from_wire(reader.u16()?).ok_or_else(invalid_payload)?;
    let schema_path = read_path(reader, caps)?;
    let compatibility_text = schema_diagnostics::CompatibilityText::from_wire(reader.u8()?)
        .ok_or_else(invalid_payload)?;
    let issue = schema_diagnostics::Issue {
        instance_path,
        schema_keyword,
        reason,
        schema_path,
        compatibility_text,
    };
    if issue.schema_keyword != issue.reason.schema_keyword() {
        return Err(invalid_payload());
    }
    Ok(issue)
}

fn write_path(
    writer: &mut Writer,
    path: &[schema_diagnostics::PathSegment],
) -> Result<(), ItemRefusal> {
    writer.u16(u16::try_from(path.len()).map_err(|_| ItemRefusal::Budget)?)?;
    for segment in path {
        match segment {
            schema_diagnostics::PathSegment::Property(value) => {
                writer.u8(0)?;
                writer.string(value)?;
            }
            schema_diagnostics::PathSegment::Index(value) => {
                writer.u8(1)?;
                writer.u64(*value)?;
            }
        }
    }
    Ok(())
}

fn read_path(
    reader: &mut Reader<'_>,
    caps: schema_diagnostics::Caps,
) -> Result<Vec<schema_diagnostics::PathSegment>, ItemRefusal> {
    let count = usize::from(reader.u16()?);
    if count > caps.max_path_segments as usize {
        return Err(invalid_payload());
    }
    let mut path = Vec::new();
    path.try_reserve_exact(count)
        .map_err(|_| ItemRefusal::Budget)?;
    let mut path_bytes = 0usize;
    for _ in 0..count {
        match reader.u8()? {
            0 => {
                let remaining = (caps.max_path_bytes as usize)
                    .checked_sub(path_bytes)
                    .ok_or_else(invalid_payload)?;
                let property = reader.string(remaining)?;
                path_bytes = path_bytes
                    .checked_add(property.len())
                    .filter(|bytes| *bytes <= caps.max_path_bytes as usize)
                    .ok_or_else(invalid_payload)?;
                path.push(schema_diagnostics::PathSegment::Property(property));
            }
            1 => {
                path_bytes = path_bytes
                    .checked_add(std::mem::size_of::<u64>())
                    .filter(|bytes| *bytes <= caps.max_path_bytes as usize)
                    .ok_or_else(invalid_payload)?;
                path.push(schema_diagnostics::PathSegment::Index(reader.u64()?));
            }
            _ => return Err(invalid_payload()),
        }
    }
    Ok(path)
}

fn encoded_string_len(len: usize) -> Result<usize, ItemRefusal> {
    u32::try_from(len).map_err(|_| ItemRefusal::Budget)?;
    len.checked_add(4).ok_or(ItemRefusal::Budget)
}

fn ensure_sqlite_length(len: usize) -> Result<(), ItemRefusal> {
    let len = u64::try_from(len).map_err(|_| ItemRefusal::Budget)?;
    if len > i64::MAX as u64 {
        return Err(ItemRefusal::Budget);
    }
    Ok(())
}

fn invalid_payload() -> ItemRefusal {
    ItemRefusal::Source("cut schema spool codec payload invalid".to_owned())
}

struct Writer {
    bytes: Vec<u8>,
    maximum: usize,
}

impl Writer {
    fn with_capacity(capacity: usize) -> Result<Self, ItemRefusal> {
        ensure_sqlite_length(capacity)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| ItemRefusal::Budget)?;
        Ok(Self {
            bytes,
            maximum: capacity,
        })
    }

    fn header(&mut self, magic: &[u8; 4]) -> Result<(), ItemRefusal> {
        self.extend(magic)?;
        self.u8(CODEC_VERSION)
    }

    fn string(&mut self, value: &str) -> Result<(), ItemRefusal> {
        self.u32(u32::try_from(value.len()).map_err(|_| ItemRefusal::Budget)?)?;
        self.extend(value.as_bytes())
    }

    fn digest(&mut self, value: Digest256) -> Result<(), ItemRefusal> {
        self.extend(value.as_bytes())
    }

    fn profile(&mut self, value: FormatProfile) -> Result<(), ItemRefusal> {
        self.u8(match value {
            FormatProfile::LegacyPythonObserved20260923 => 1,
            FormatProfile::AssertedSourceCandidateV1 => 2,
        })
    }

    fn bool(&mut self, value: bool) -> Result<(), ItemRefusal> {
        self.u8(u8::from(value))
    }

    fn usize(&mut self, value: usize) -> Result<(), ItemRefusal> {
        self.u64(u64::try_from(value).map_err(|_| ItemRefusal::Budget)?)
    }

    fn u8(&mut self, value: u8) -> Result<(), ItemRefusal> {
        self.extend(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), ItemRefusal> {
        self.extend(&value.to_be_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), ItemRefusal> {
        self.extend(&value.to_be_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), ItemRefusal> {
        self.extend(&value.to_be_bytes())
    }

    fn extend(&mut self, value: &[u8]) -> Result<(), ItemRefusal> {
        let next = self
            .bytes
            .len()
            .checked_add(value.len())
            .filter(|length| *length <= self.maximum)
            .ok_or(ItemRefusal::Budget)?;
        if next > self.bytes.capacity() {
            return Err(ItemRefusal::Budget);
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn finish(self, bound: usize) -> Result<Vec<u8>, ItemRefusal> {
        if self.bytes.len() != bound {
            return Err(ItemRefusal::Budget);
        }
        Ok(self.bytes)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn header(&mut self, magic: &[u8; 4]) -> Result<(), ItemRefusal> {
        if self.take(4)? != &magic[..] || self.u8()? != CODEC_VERSION {
            return Err(invalid_payload());
        }
        Ok(())
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ItemRefusal> {
        let end = self
            .offset
            .checked_add(len)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(invalid_payload)?;
        let result = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(result)
    }

    fn string(&mut self, maximum: usize) -> Result<String, ItemRefusal> {
        let len = usize::try_from(self.u32()?).map_err(|_| invalid_payload())?;
        if len > maximum {
            return Err(invalid_payload());
        }
        let raw = self.take(len)?;
        let value = std::str::from_utf8(raw).map_err(|_| invalid_payload())?;
        let mut result = String::new();
        result
            .try_reserve_exact(value.len())
            .map_err(|_| ItemRefusal::Budget)?;
        result.push_str(value);
        Ok(result)
    }

    fn digest(&mut self) -> Result<Digest256, ItemRefusal> {
        let mut bytes = [0u8; 32];
        let raw = self.take(32)?;
        bytes.copy_from_slice(raw);
        Ok(Digest256::from_bytes(bytes))
    }

    fn profile(&mut self) -> Result<FormatProfile, ItemRefusal> {
        match self.u8()? {
            1 => Ok(FormatProfile::LegacyPythonObserved20260923),
            2 => Ok(FormatProfile::AssertedSourceCandidateV1),
            _ => Err(invalid_payload()),
        }
    }

    fn bool(&mut self) -> Result<bool, ItemRefusal> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid_payload()),
        }
    }

    fn u8(&mut self) -> Result<u8, ItemRefusal> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ItemRefusal> {
        Ok(u16::from_be_bytes(
            self.take(2)?.try_into().map_err(|_| invalid_payload())?,
        ))
    }

    fn u32(&mut self) -> Result<u32, ItemRefusal> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().map_err(|_| invalid_payload())?,
        ))
    }

    fn u64(&mut self) -> Result<u64, ItemRefusal> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().map_err(|_| invalid_payload())?,
        ))
    }

    fn usize(&mut self) -> Result<usize, ItemRefusal> {
        usize::try_from(self.u64()?).map_err(|_| invalid_payload())
    }

    fn finish(&self) -> Result<(), ItemRefusal> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(invalid_payload())
        }
    }
}
