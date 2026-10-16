//! Standalone native source-witness catalog candidate and parity routes.
//!
//! `build` uses the protected full-foundation route and emits one bounded
//! framed candidate only after its source, repository, worker and metadata
//! fences complete. `check` is the same protected full-foundation comparison
//! without candidate bytes. Neither operation writes the source checkout.

use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_current_cut::foundation_catalog;
use crate::source_current_cut::foundation_command;
use crate::source_current_cut::foundation_command::SourceFoundationCatalogueObservationLimits;
use std::cell::RefCell;
use std::ffi::OsString;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, AtomicI32};
use tos_compiler::knowledge_stage::KnowledgeStage;
use tos_compiler::source_witness_catalog::{
    ColdSourceCatalogReceipt, SourceCatalogLimits, SourceCatalogSink,
    render_cold_source_witness_catalog,
};
use tos_compiler::{Error as CompilerError, Result as CompilerResult};
use tos_foundation::{Digest256, JsonLimits};

const HELP: &str = "usage: tos-native-owner-command source-catalog check --repo-root ABS --invocation ABS\n       tos-native-owner-command source-catalog build --repo-root ABS --invocation ABS --max-candidate-bytes N --max-stage-read-bytes N --max-state-bytes N\n\n`check` runs the protected full source-witness foundation audit and generated-catalog comparison. `build` requires its owned-cold foundation mode and writes a bounded `TOS_SOURCE_CATALOG_CANDIDATE_V1` frame stream to stdout after all terminal source and metadata fences pass. The stream contains path, byte length, SHA-256, and exact candidate bytes for every generated catalog file, including the manifest. Both routes need the private-tmpfs issuer ticket; neither modifies the source tree. The three build limits are finite caller reservations, and `max-state-bytes` must cover the derived peak bound for the selected source-catalog profile.\n";

const CANDIDATE_MAGIC: &[u8] = b"TOS_SOURCE_CATALOG_CANDIDATE_V1\n";
const MAX_ARGUMENTS: usize = 64;
const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
const MAX_CANDIDATE_BYTES: usize = 128 * 1024 * 1024;
const MAX_REPORT_BYTES: usize = 1024 * 1024;
const MAX_PATH_BYTES: usize = 4096;
const MANIFEST_PATH: &str = "ToS/source-witnesses/catalog/catalog.manifest.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    Build,
    Check,
    Help,
}

struct Arguments {
    operation: Operation,
    foundation: Vec<OsString>,
    max_candidate_bytes: Option<usize>,
    max_stage_read_bytes: Option<u64>,
    max_state_bytes: Option<usize>,
}

fn parse(args: &[OsString]) -> SourceCommandResult<Arguments> {
    if args.len() > MAX_ARGUMENTS
        || args
            .iter()
            .try_fold(0usize, |total, arg| {
                total.checked_add(arg.as_os_str().as_bytes().len())
            })
            .is_none_or(|bytes| bytes > MAX_ARGUMENT_BYTES)
    {
        return Err(SourceCommandError::Invalid(
            "source-catalog argument bounds",
        ));
    }
    let Some(operation) = args.first().and_then(|arg| arg.to_str()) else {
        return Err(SourceCommandError::Invalid("source-catalog operation"));
    };
    if matches!(operation, "--help" | "-h")
        || (args.len() == 2 && matches!(args[1].to_str(), Some("--help" | "-h")))
    {
        return Ok(Arguments {
            operation: Operation::Help,
            foundation: Vec::new(),
            max_candidate_bytes: None,
            max_stage_read_bytes: None,
            max_state_bytes: None,
        });
    }
    let operation = match operation {
        "build" => Operation::Build,
        "check" => Operation::Check,
        _ => return Err(SourceCommandError::Invalid("source-catalog operation")),
    };
    let mut foundation = Vec::with_capacity(args.len());
    let mut repo_root = false;
    let mut invocation = false;
    let mut max_candidate_bytes = None;
    let mut max_stage_read_bytes = None;
    let mut max_state_bytes = None;
    let mut index = 1;
    while index < args.len() {
        let key = args[index]
            .to_str()
            .ok_or(SourceCommandError::Invalid("source-catalog option UTF-8"))?;
        match key {
            "--repo-root" | "--invocation" => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.is_empty())
                    .ok_or(SourceCommandError::Invalid("source-catalog option value"))?;
                let path = value.to_str().ok_or(SourceCommandError::Invalid(
                    "source-catalog selected path UTF-8",
                ))?;
                if !std::path::Path::new(path).is_absolute() {
                    return Err(SourceCommandError::Denied(
                        "source-catalog selected paths must be absolute",
                    ));
                }
                if key == "--repo-root" {
                    if repo_root {
                        return Err(SourceCommandError::Invalid(
                            "duplicate source-catalog repository root",
                        ));
                    }
                    repo_root = true;
                } else {
                    if invocation {
                        return Err(SourceCommandError::Invalid(
                            "duplicate source-catalog invocation",
                        ));
                    }
                    invocation = true;
                }
                foundation.push(args[index].clone());
                foundation.push(value.clone());
                index += 2;
            }
            "--max-candidate-bytes" if operation == Operation::Build => {
                if max_candidate_bytes.is_some() {
                    return Err(SourceCommandError::Invalid(
                        "duplicate source-catalog candidate limit",
                    ));
                }
                let value = args.get(index + 1).and_then(|value| value.to_str()).ok_or(
                    SourceCommandError::Invalid("source-catalog candidate limit"),
                )?;
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| SourceCommandError::Invalid("source-catalog candidate limit"))?;
                if parsed == 0 || parsed > MAX_CANDIDATE_BYTES {
                    return Err(SourceCommandError::Denied(
                        "source-catalog candidate limit outside fixed range",
                    ));
                }
                max_candidate_bytes = Some(parsed);
                index += 2;
            }
            "--max-stage-read-bytes" if operation == Operation::Build => {
                if max_stage_read_bytes.is_some() {
                    return Err(SourceCommandError::Invalid(
                        "duplicate source-catalog stage read limit",
                    ));
                }
                let value = args.get(index + 1).and_then(|value| value.to_str()).ok_or(
                    SourceCommandError::Invalid("source-catalog stage read limit"),
                )?;
                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| SourceCommandError::Invalid("source-catalog stage read limit"))?;
                if parsed == 0 {
                    return Err(SourceCommandError::Denied(
                        "source-catalog stage read limit must be positive",
                    ));
                }
                max_stage_read_bytes = Some(parsed);
                index += 2;
            }
            "--max-state-bytes" if operation == Operation::Build => {
                if max_state_bytes.is_some() {
                    return Err(SourceCommandError::Invalid(
                        "duplicate source-catalog state limit",
                    ));
                }
                let value = args
                    .get(index + 1)
                    .and_then(|value| value.to_str())
                    .ok_or(SourceCommandError::Invalid("source-catalog state limit"))?;
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| SourceCommandError::Invalid("source-catalog state limit"))?;
                if parsed == 0 {
                    return Err(SourceCommandError::Denied(
                        "source-catalog state limit must be positive",
                    ));
                }
                max_state_bytes = Some(parsed);
                index += 2;
            }
            _ => return Err(SourceCommandError::Invalid("unknown source-catalog option")),
        }
    }
    if !repo_root || !invocation {
        return Err(SourceCommandError::Denied(
            "source-catalog requires explicit repository root and protected invocation",
        ));
    }
    if operation == Operation::Build
        && (max_candidate_bytes.is_none()
            || max_stage_read_bytes.is_none()
            || max_state_bytes.is_none())
    {
        return Err(SourceCommandError::Denied(
            "source-catalog build requires explicit candidate, stage-read, and state limits",
        ));
    }
    if operation == Operation::Check
        && (max_candidate_bytes.is_some()
            || max_stage_read_bytes.is_some()
            || max_state_bytes.is_some())
    {
        return Err(SourceCommandError::Invalid(
            "source-catalog check does not accept build reservations",
        ));
    }
    Ok(Arguments {
        operation,
        foundation,
        max_candidate_bytes,
        max_stage_read_bytes,
        max_state_bytes,
    })
}

/// Run one explicit catalog operation. The foundation owner keeps the actual
/// source cut, pinned schema worker, TMPFS ticket and final source/epoch fences.
pub fn run(
    args: &[OsString],
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> SourceCommandResult<i32> {
    let parsed = parse(args)?;
    if parsed.operation == Operation::Help {
        stdout
            .write_all(HELP.as_bytes())
            .and_then(|_| stdout.flush())
            .map_err(|_| SourceCommandError::Denied("source-catalog help output failed"))?;
        return Ok(0);
    }
    if parsed.operation == Operation::Check {
        let published_root = std::cell::Cell::new(false);
        let mut report = BoundedReport::new(MAX_REPORT_BYTES);
        let result = foundation_command::run_with_owned_catalogue_observation(
            &parsed.foundation,
            cancelled,
            git_signal,
            &mut report,
            stderr,
            SourceFoundationCatalogueObservationLimits {
                max_stage_read_bytes: 0,
                max_state_bytes: 1024,
            },
            |_, _, _, origin| {
                if origin.as_str() != "published_root" || published_root.replace(true) {
                    return Err(CompilerError::Invalid(
                        "source-catalog check requires published-root foundation invocation",
                    ));
                }
                Ok(())
            },
        );
        return match result {
            Ok(code) => {
                stdout
                    .write_all(&report.bytes)
                    .and_then(|_| stdout.flush())
                    .map_err(|_| {
                        SourceCommandError::Denied("source-catalog report output failed")
                    })?;
                if code == 0 && !published_root.get() {
                    return Err(SourceCommandError::Denied(
                        "source-catalog check callback absent after complete foundation audit",
                    ));
                }
                Ok(code)
            }
            // Foundation owns refusal formatting and bounded stderr output.
            Err(_) => Ok(2),
        };
    }

    let max_candidate_bytes = parsed
        .max_candidate_bytes
        .ok_or(SourceCommandError::Invalid(
            "source-catalog candidate limit absent",
        ))?;
    let max_stage_read_bytes = parsed
        .max_stage_read_bytes
        .ok_or(SourceCommandError::Invalid(
            "source-catalog stage read limit absent",
        ))?;
    let max_state_bytes = parsed.max_state_bytes.ok_or(SourceCommandError::Invalid(
        "source-catalog state limit absent",
    ))?;
    let candidate = RefCell::new(None::<Vec<u8>>);
    let mut report = BoundedReport::new(MAX_REPORT_BYTES);
    let result = foundation_command::run_with_owned_catalogue_observation(
        &parsed.foundation,
        cancelled,
        git_signal,
        &mut report,
        stderr,
        SourceFoundationCatalogueObservationLimits {
            max_stage_read_bytes,
            max_state_bytes,
        },
        |stage, receipt, limits, origin| {
            if origin.as_str() != "cold_generated" {
                return Err(CompilerError::Invalid(
                    "source-catalog build requires owned-cold foundation invocation",
                ));
            }
            let required_state_bytes = minimum_state_bytes(max_candidate_bytes, limits)?;
            ensure_state_reservation(max_state_bytes, required_state_bytes)?;
            let json = JsonLimits::new(
                limits.max_output_row_bytes,
                96,
                manifest_entry_budget(limits)?,
                4096,
            )
            .map_err(|_| CompilerError::Budget("source-catalog manifest limits"))?;
            let manifest = foundation_catalog::published_manifest(None, receipt, json)?;
            let mut sink =
                CandidateFrameSink::new(max_candidate_bytes, limits, manifest, &receipt.manifest)?;
            render_cold_source_witness_catalog(stage, receipt, limits, &mut sink)?;
            let bytes = sink.finish()?;
            if candidate.replace(Some(bytes)).is_some() {
                return Err(CompilerError::Invalid(
                    "source-catalog candidate callback repeated",
                ));
            }
            Ok(())
        },
    );
    match result {
        Ok(0) => {
            let bytes = candidate.into_inner().ok_or(SourceCommandError::Denied(
                "source-catalog candidate absent after complete foundation audit",
            ))?;
            stdout
                .write_all(&bytes)
                .and_then(|_| stdout.flush())
                .map_err(|_| {
                    SourceCommandError::Denied("source-catalog candidate output failed")
                })?;
            Ok(0)
        }
        Ok(code) => {
            stderr
                .write_all(&report.bytes)
                .and_then(|_| stderr.flush())
                .map_err(|_| SourceCommandError::Denied("source-catalog report output failed"))?;
            Ok(code)
        }
        // The foundation command owns its bounded refusal output; do not
        // print its internal reason a second time at the binary boundary.
        Err(_) => Ok(2),
    }
}

struct BoundedReport {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl BoundedReport {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
        }
    }
}

impl Write for BoundedReport {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|next| *next <= self.max_bytes)
            .ok_or_else(|| io::Error::other("source-catalog report byte limit"))?;
        self.bytes
            .try_reserve_exact(next - self.bytes.len())
            .map_err(|_| io::Error::other("source-catalog report memory limit"))?;
        if self.bytes.capacity() > self.max_bytes {
            return Err(io::Error::other("source-catalog report capacity limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct CandidateFrameSink<'a> {
    bytes: Vec<u8>,
    current_path: Option<String>,
    current_file: Vec<u8>,
    expected_manifest: Vec<u8>,
    source_manifest: &'a serde_json::Value,
    max_bytes: usize,
    max_file_bytes: usize,
    max_files: usize,
    max_rows: u64,
    max_addressed_row_bytes: usize,
    file_count: usize,
    addressed_rows: u64,
    manifest_seen: bool,
}

impl<'a> CandidateFrameSink<'a> {
    fn new(
        max_bytes: usize,
        limits: SourceCatalogLimits,
        expected_manifest: Vec<u8>,
        source_manifest: &'a serde_json::Value,
    ) -> CompilerResult<Self> {
        if expected_manifest.len() > limits.max_output_row_bytes
            || expected_manifest.len() > max_bytes
            || expected_manifest.capacity() > limits.max_output_row_bytes
            || !expected_manifest.ends_with(b"\n")
        {
            return Err(CompilerError::Budget(
                "source-catalog manifest output limit",
            ));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(CANDIDATE_MAGIC.len())
            .map_err(|_| CompilerError::Budget("source-catalog candidate state"))?;
        bytes.extend_from_slice(CANDIDATE_MAGIC);
        if bytes.capacity() > max_bytes {
            return Err(CompilerError::Budget("source-catalog candidate state"));
        }
        Ok(Self {
            bytes,
            current_path: None,
            current_file: Vec::new(),
            expected_manifest,
            source_manifest,
            max_bytes,
            max_file_bytes: limits.max_file_bytes,
            max_files: usize::try_from(limits.max_files)
                .unwrap_or(usize::MAX)
                .saturating_add(1),
            max_rows: limits.max_rows,
            max_addressed_row_bytes: limits.max_output_row_bytes,
            file_count: 0,
            addressed_rows: 0,
            manifest_seen: false,
        })
    }

    fn append_frame(&mut self, path: &str, content: &[u8], digest: &str) -> CompilerResult<()> {
        if self.file_count >= self.max_files
            || path.is_empty()
            || path.len() > MAX_PATH_BYTES
            || !valid_catalog_path(path)
            || u32::try_from(path.len()).is_err()
            || u64::try_from(content.len()).is_err()
        {
            return Err(CompilerError::Budget(
                "source-catalog candidate file bounds",
            ));
        }
        let computed = Digest256::of_bytes(content).to_hex();
        if computed != digest {
            return Err(CompilerError::Invalid("source-catalog candidate digest"));
        }
        let frame_bytes = 4usize
            .checked_add(8)
            .and_then(|n| n.checked_add(64))
            .and_then(|n| n.checked_add(path.len()))
            .and_then(|n| n.checked_add(content.len()))
            .and_then(|n| self.bytes.len().checked_add(n))
            .filter(|n| *n <= self.max_bytes)
            .ok_or(CompilerError::Budget("source-catalog candidate byte limit"))?;
        self.bytes
            .try_reserve_exact(frame_bytes - self.bytes.len())
            .map_err(|_| CompilerError::Budget("source-catalog candidate state"))?;
        if self.bytes.capacity() > self.max_bytes {
            return Err(CompilerError::Budget("source-catalog candidate state"));
        }
        self.bytes
            .extend_from_slice(&(path.len() as u32).to_be_bytes());
        self.bytes
            .extend_from_slice(&(content.len() as u64).to_be_bytes());
        self.bytes.extend_from_slice(digest.as_bytes());
        self.bytes.extend_from_slice(path.as_bytes());
        self.bytes.extend_from_slice(content);
        self.file_count += 1;
        Ok(())
    }

    fn finish(mut self) -> CompilerResult<Vec<u8>> {
        if self.current_path.is_some() || !self.manifest_seen {
            return Err(CompilerError::Invalid(
                "source-catalog candidate incomplete",
            ));
        }
        let final_bytes = self
            .bytes
            .len()
            .checked_add(4)
            .filter(|n| *n <= self.max_bytes)
            .ok_or(CompilerError::Budget("source-catalog candidate byte limit"))?;
        self.bytes
            .try_reserve_exact(final_bytes - self.bytes.len())
            .map_err(|_| CompilerError::Budget("source-catalog candidate state"))?;
        if self.bytes.capacity() > self.max_bytes {
            return Err(CompilerError::Budget("source-catalog candidate state"));
        }
        self.bytes.extend_from_slice(&0u32.to_be_bytes());
        Ok(self.bytes)
    }
}

impl<'a> SourceCatalogSink for CandidateFrameSink<'a> {
    fn begin_file(&mut self, path: &str) -> CompilerResult<()> {
        if self.current_path.is_some() || self.manifest_seen || !valid_catalog_path(path) {
            return Err(CompilerError::Invalid(
                "source-catalog candidate file order",
            ));
        }
        self.current_path = Some(path.to_owned());
        self.current_file.clear();
        Ok(())
    }

    fn file_bytes(&mut self, bytes: &[u8]) -> CompilerResult<()> {
        let Some(path) = self.current_path.as_deref() else {
            return Err(CompilerError::Invalid(
                "source-catalog candidate file absent",
            ));
        };
        let frame_overhead = 4usize
            .checked_add(8)
            .and_then(|n| n.checked_add(64))
            .and_then(|n| n.checked_add(path.len()))
            .ok_or(CompilerError::Budget("source-catalog candidate file limit"))?;
        let content_cap = self
            .max_bytes
            .checked_sub(self.bytes.len())
            .and_then(|n| n.checked_sub(frame_overhead))
            .ok_or(CompilerError::Budget("source-catalog candidate file limit"))?;
        let next = self
            .current_file
            .len()
            .checked_add(bytes.len())
            .filter(|next| *next <= content_cap && *next <= self.max_file_bytes)
            .ok_or(CompilerError::Budget("source-catalog candidate file limit"))?;
        self.current_file
            .try_reserve_exact(next - self.current_file.len())
            .map_err(|_| CompilerError::Budget("source-catalog candidate state"))?;
        if self.current_file.capacity() > self.max_file_bytes {
            return Err(CompilerError::Budget("source-catalog candidate state"));
        }
        self.current_file.extend_from_slice(bytes);
        Ok(())
    }

    fn end_file(&mut self, path: &str, sha256: &str) -> CompilerResult<()> {
        let opened = self.current_path.take().ok_or(CompilerError::Invalid(
            "source-catalog candidate file absent",
        ))?;
        if opened != path {
            return Err(CompilerError::Invalid(
                "source-catalog candidate path changed",
            ));
        }
        let file = std::mem::take(&mut self.current_file);
        self.append_frame(path, &file, sha256)?;
        Ok(())
    }

    fn addressed_row(&mut self, collection: &str, row: &[u8]) -> CompilerResult<()> {
        if !matches!(collection, "records" | "claims" | "slots")
            || row.len() > self.max_addressed_row_bytes
        {
            return Err(CompilerError::Invalid("source-catalog addressed row"));
        }
        self.addressed_rows = self
            .addressed_rows
            .checked_add(1)
            .filter(|n| *n <= self.max_rows.saturating_add(1))
            .ok_or(CompilerError::Budget("source-catalog addressed row count"))?;
        Ok(())
    }

    fn manifest(&mut self, manifest: &serde_json::Value) -> CompilerResult<()> {
        if self.current_path.is_some() || self.manifest_seen {
            return Err(CompilerError::Invalid("source-catalog manifest order"));
        }
        // The selected cold renderer forwards this exact sealed receipt value.
        // Pointer identity prevents a copied or substituted manifest without
        // allocating a second JSON tree proportional to its contents.
        if !std::ptr::eq(self.source_manifest, manifest) {
            return Err(CompilerError::Invalid(
                "source-catalog published manifest differs from receipt",
            ));
        }
        let raw = std::mem::take(&mut self.expected_manifest);
        let digest = Digest256::of_bytes(&raw).to_hex();
        self.append_frame(MANIFEST_PATH, &raw, &digest)?;
        self.manifest_seen = true;
        Ok(())
    }
}

fn valid_catalog_path(path: &str) -> bool {
    path.len() <= MAX_PATH_BYTES
        && path.starts_with("ToS/source-witnesses/catalog/")
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && path
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'\\')
}

fn minimum_state_bytes(
    max_candidate_bytes: usize,
    limits: SourceCatalogLimits,
) -> CompilerResult<usize> {
    // The published manifest builder holds the encoded result and bounded
    // per-field typed/encoded copies. `max_files` also caps JSON tree entries.
    let manifest_working = limits
        .max_output_row_bytes
        .checked_mul(3)
        .and_then(|n| {
            manifest_entry_budget(limits)
                .ok()?
                .checked_mul(512)
                .and_then(|entries| n.checked_add(entries))
        })
        .ok_or(CompilerError::Budget("source-catalog manifest state bound"))?;
    max_candidate_bytes
        .checked_add(limits.max_file_bytes)
        .and_then(|n| n.checked_add(manifest_working))
        .and_then(|n| n.checked_add(MAX_REPORT_BYTES))
        .and_then(|n| n.checked_add(MAX_PATH_BYTES))
        .and_then(|n| n.checked_add(std::mem::size_of::<CandidateFrameSink<'_>>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<Option<Vec<u8>>>()))
        .ok_or(CompilerError::Budget("source-catalog callback state bound"))
}

fn manifest_entry_budget(limits: SourceCatalogLimits) -> CompilerResult<usize> {
    usize::try_from(limits.max_files)
        .ok()
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_add(128))
        .ok_or(CompilerError::Budget("source-catalog manifest entry bound"))
}

fn ensure_state_reservation(
    max_state_bytes: usize,
    required_state_bytes: usize,
) -> CompilerResult<()> {
    if max_state_bytes < required_state_bytes {
        return Err(CompilerError::Budget(
            "source-catalog callback state reservation below derived peak",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(values: &[&str]) -> Vec<OsString> {
        values.iter().map(|value| OsString::from(*value)).collect()
    }

    fn limits() -> SourceCatalogLimits {
        SourceCatalogLimits {
            max_files: 16,
            max_rows: 16,
            max_file_bytes: 1024,
            max_row_bytes: 1024,
            max_contract_bytes: 1024,
            max_output_row_bytes: 1024,
        }
    }

    #[test]
    fn candidate_frame_keeps_exact_file_bytes_and_manifest() {
        let manifest = b"{}\n".to_vec();
        let value: serde_json::Value = serde_json::from_slice(b"{}\n").unwrap();
        let mut sink = CandidateFrameSink::new(4096, limits(), manifest, &value).unwrap();
        let path = "ToS/source-witnesses/catalog/agents.jsonl";
        let bytes = b"{\"agent_id\":\"a\"}\n";
        let digest = Digest256::of_bytes(bytes).to_hex();
        sink.begin_file(path).unwrap();
        sink.file_bytes(bytes).unwrap();
        sink.end_file(path, &digest).unwrap();
        sink.addressed_row("records", b"{}").unwrap();
        // The actual published manifest is supplied through foundation's
        // serializer; tests keep the callback's structural comparison simple.
        sink.manifest(&value).unwrap();
        let frame = sink.finish().unwrap();
        assert!(frame.starts_with(CANDIDATE_MAGIC));
        assert!(frame.windows(bytes.len()).any(|window| window == bytes));
        assert!(
            frame
                .windows(MANIFEST_PATH.len())
                .any(|window| window == MANIFEST_PATH.as_bytes())
        );
        assert!(frame.ends_with(&0u32.to_be_bytes()));
    }

    #[test]
    fn candidate_frame_rejects_digest_drift_and_oversize() {
        let value: serde_json::Value = serde_json::from_slice(b"{}\n").unwrap();
        let mut sink = CandidateFrameSink::new(256, limits(), b"{}\n".to_vec(), &value).unwrap();
        sink.begin_file("ToS/source-witnesses/catalog/agents.jsonl")
            .unwrap();
        sink.file_bytes(b"changed").unwrap();
        assert!(
            sink.end_file("ToS/source-witnesses/catalog/agents.jsonl", &"0".repeat(64))
                .is_err()
        );

        let value: serde_json::Value = serde_json::from_slice(b"{}\n").unwrap();
        let mut sink = CandidateFrameSink::new(64, limits(), b"{}\n".to_vec(), &value).unwrap();
        sink.begin_file("ToS/source-witnesses/catalog/agents.jsonl")
            .unwrap();
        assert!(sink.file_bytes(&vec![b'x'; 65]).is_err());
    }

    #[test]
    fn candidate_build_requires_derived_state_reservation() {
        let required = minimum_state_bytes(1024, limits()).unwrap();
        assert!(required > 1024);
        assert!(ensure_state_reservation(1024, required).is_err());
        assert!(ensure_state_reservation(required, required).is_ok());
    }

    #[test]
    fn build_requires_explicit_finite_stage_and_state_reservations() {
        assert!(
            parse(&words(&[
                "build",
                "--repo-root",
                "/repo",
                "--invocation",
                "/protected/request.json",
                "--max-candidate-bytes",
                "4096",
            ]))
            .is_err()
        );
        let parsed = parse(&words(&[
            "build",
            "--repo-root",
            "/repo",
            "--invocation",
            "/protected/request.json",
            "--max-candidate-bytes",
            "4096",
            "--max-stage-read-bytes",
            "1048576",
            "--max-state-bytes",
            "8388608",
        ]))
        .unwrap();
        assert_eq!(parsed.max_candidate_bytes, Some(4096));
        assert_eq!(parsed.max_stage_read_bytes, Some(1_048_576));
        assert_eq!(parsed.max_state_bytes, Some(8_388_608));
    }
}
