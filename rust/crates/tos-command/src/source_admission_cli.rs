//! Maintained corpus-admit entry over the actual native foundation validator.
//! The batch names bytes and a validator identity; it never selects code.
use crate::source_admission::{AdmissionBatch, invalid};
use crate::source_admission_candidate::Candidate;
use crate::source_admission_store::AdmissionStore;
use crate::source_current_cut::{
    foundation_command::SelectedOutput, foundation_entry::FoundationBootstrapClock,
};
use crate::source_foundation_admission::NativeSourceValidator;
use serde_json::json;
use std::{
    cell::Cell,
    ffi::OsString,
    io::{self, Write},
    os::unix::ffi::OsStrExt,
    path::{Component, PathBuf},
    sync::atomic::{AtomicBool, AtomicI32},
    time::Instant,
};

pub const HELP: &str = "usage: tos-native-owner-command corpus-admit --store PATH --batch PATH --input-root PATH --grammar-root PATH --invocation PATH [--payload-source-root PATH] [--historical-capture PATH --historical-root PATH]...\n       tos-native-owner-command corpus-admit --validator-identity --grammar-root PATH --invocation PATH [validation selections]\n\nAdmit exact proposed source bytes through the selected complete native validator.\nThe invocation selects finite operation resources and pinned workers. No semantic admission or rights change is granted.\n";

struct Arguments {
    store: Option<PathBuf>,
    batch: Option<PathBuf>,
    input: Option<PathBuf>,
    validator: Vec<OsString>,
    identity_only: bool,
    help: bool,
}
fn path(value: &OsString) -> io::Result<PathBuf> {
    if value.is_empty() {
        return Err(invalid("empty corpus admission path"));
    }
    let path = std::path::absolute(value)?;
    if path
        .components()
        .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(invalid("corpus admission path must be normalized"));
    }
    Ok(path)
}
fn parse(args: &[OsString]) -> io::Result<Arguments> {
    // Same bounded bootstrap argument envelope as the native FND entry.
    if args.len() > 64
        || args
            .iter()
            .try_fold(0usize, |sum, arg| {
                sum.checked_add(arg.as_os_str().as_bytes().len())
            })
            .is_none_or(|n| n > 32 * 1024)
    {
        return Err(invalid("corpus admission argument limits exceeded"));
    }
    let mut result = Arguments {
        store: None,
        batch: None,
        input: None,
        validator: Vec::new(),
        identity_only: false,
        help: false,
    };
    let mut grammar = None;
    let mut invocation = None;
    let mut payload = None;
    let mut captures = Vec::new();
    let mut roots = Vec::new();
    let mut position = 0;
    while position < args.len() {
        let option = args[position]
            .to_str()
            .ok_or_else(|| invalid("corpus admission option must be UTF-8"))?;
        position += 1;
        match option {
            "--help" | "-h" => {
                result.help = true;
                return Ok(result);
            }
            "--validator-identity" => {
                result.identity_only = true;
                continue;
            }
            "--store"
            | "--batch"
            | "--input-root"
            | "--grammar-root"
            | "--invocation"
            | "--payload-source-root"
            | "--historical-capture"
            | "--historical-root" => (),
            _ => return Err(invalid("unknown corpus admission option")),
        }
        let value = path(
            args.get(position)
                .ok_or_else(|| invalid("corpus admission option requires a path"))?,
        )?;
        position += 1;
        match option {
            "--store" | "--batch" | "--input-root" | "--grammar-root" | "--payload-source-root" => {
                let selected = match option {
                    "--store" => &mut result.store,
                    "--batch" => &mut result.batch,
                    "--input-root" => &mut result.input,
                    "--grammar-root" => &mut grammar,
                    _ => &mut payload,
                };
                if selected.replace(value).is_some() {
                    return Err(invalid(format!("duplicate corpus admission {option}")));
                }
            }
            "--historical-capture" => captures.push(value),
            "--historical-root" => roots.push(value),
            "--invocation" => {
                if invocation.replace(value).is_some() {
                    return Err(invalid("duplicate corpus admission invocation"));
                }
            }
            _ => unreachable!(),
        }
    }
    if captures.len() != roots.len() {
        return Err(invalid(
            "each historical capture requires its exact restored root",
        ));
    }
    let grammar = grammar.ok_or_else(|| invalid("corpus admission requires --grammar-root"))?;
    let invocation = invocation.ok_or_else(|| invalid("corpus admission requires --invocation"))?;
    if result.identity_only {
        if result.store.is_some() || result.batch.is_some() || result.input.is_some() {
            return Err(invalid(
                "validator identity selection cannot also admit a batch",
            ));
        }
    } else if result.store.is_none() || result.batch.is_none() || result.input.is_none() {
        return Err(invalid(
            "corpus admission requires --store, --batch and --input-root",
        ));
    }
    result.validator.extend([
        OsString::from("--repo-root"),
        grammar.into_os_string(),
        OsString::from("--invocation"),
        invocation.into_os_string(),
    ]);
    if let Some(payload) = payload {
        result.validator.extend([
            OsString::from("--payload-source-root"),
            payload.into_os_string(),
        ]);
    }
    // Preserve the explicit selection for the adapter. Unsupported historical
    // verification is refused there before candidate preparation.
    for (capture, root) in captures.into_iter().zip(roots) {
        result.validator.extend([
            OsString::from("--historical-capture"),
            capture.into_os_string(),
            OsString::from("--historical-root"),
            root.into_os_string(),
        ]);
    }
    Ok(result)
}

/// A refusal is returned to the outer entry, never converted to an accepted
/// receipt or a legacy fallback. The real validator owns final bounded output.
pub fn run(
    args: &[OsString],
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<i32> {
    let clock = FoundationBootstrapClock::begin()
        .map_err(|error| invalid(format!("admission bootstrap: {error:?}")))?;
    let bytes = Cell::new(0);
    let stopped = Cell::new(false);
    let output_cap = Cell::new(4096);
    let output_deadline = Cell::new(clock.hard_deadline());
    let phase = Cell::new("command arguments");
    let result = run_selected(
        args,
        cancelled,
        git_signal,
        stdout,
        clock,
        &bytes,
        &stopped,
        &output_cap,
        &output_deadline,
        &phase,
    );
    if let Err(error) = &result {
        let mut output = SelectedOutput {
            writer: stderr,
            bytes: &bytes,
            stopped: &stopped,
            max_bytes: output_cap.get(),
            deadline: output_deadline.get(),
            cancelled,
        };
        // Public context is an operation/contract label, never document bytes
        // or a worker diagnostic. A failed/exhausted writer cannot print again.
        let public_phase = error
            .get_ref()
            .and_then(|source| {
                source.downcast_ref::<crate::source_foundation_admission::NativeValidationRefusal>()
            })
            .map_or(phase.get(), |refusal| refusal.0);
        let _ = writeln!(
            output,
            "Native corpus admission refused during {}",
            public_phase
        )
        .and_then(|_| output.flush());
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run_selected(
    args: &[OsString],
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
    stdout: &mut dyn Write,
    clock: FoundationBootstrapClock,
    bytes: &Cell<usize>,
    stopped: &Cell<bool>,
    output_cap: &Cell<usize>,
    output_deadline: &Cell<Instant>,
    phase: &Cell<&'static str>,
) -> io::Result<i32> {
    let args = parse(args)?;
    if args.help {
        let mut output = SelectedOutput {
            writer: stdout,
            bytes,
            stopped,
            max_bytes: output_cap.get(),
            deadline: output_deadline.get(),
            cancelled,
        };
        output.write_all(HELP.as_bytes())?;
        output.flush()?;
        return Ok(0);
    }
    phase.set("protected invocation and validator identity");
    let mut validator = NativeSourceValidator::prepare(
        clock,
        &args.validator,
        cancelled,
        git_signal,
        |cap, deadline| {
            output_cap.set(usize::try_from(cap).unwrap_or(usize::MAX));
            output_deadline.set(deadline);
        },
    )?;
    let mut selected_output = SelectedOutput {
        writer: stdout,
        bytes,
        stopped,
        max_bytes: output_cap.get(),
        deadline: output_deadline.get(),
        cancelled,
    };
    let stdout: &mut dyn Write = &mut selected_output;
    let identity = validator.identity();
    if args.identity_only {
        phase.set("validator identity final custody");
        validator.finalize_without_evaluation()?;
        validator.write_receipt(
            &json!({"schema_version":"tos_native_source_validator_identity_v1",
            "validator_sha256":identity.to_hex(),"semantic_admission":false,"rights_change":false}),
            stdout,
        )?;
        return Ok(0);
    }
    phase.set("persistent store write authority");
    let store_path = args.store.as_deref().unwrap();
    validator.bind_store_authority(store_path)?;
    phase.set("admission resource profile");
    let mut limits = validator.candidate_limits()?.validate()?;
    if limits.admission.max_batch_bytes as u64 > limits.max_read_bytes {
        return Err(invalid(
            "batch read envelope exceeds remaining operation bytes",
        ));
    }
    let deadline = validator.deadline();
    phase.set("exact batch and input-root selection");
    let batch = AdmissionBatch::read(
        args.batch.as_deref().unwrap(),
        args.input.as_deref().unwrap(),
        limits.bounded_batch_limits()?,
        deadline,
        cancelled,
    )?;
    if batch.validator_sha256 != identity {
        return Err(invalid(
            "batch validator identity does not match selected native program and grammar",
        ));
    }
    // Batch IO and candidate preparation share the remaining FND ledger.
    // Reserve the bytes already read before allowing candidate object IO;
    // account_candidate later charges this batch exactly once in that ledger.
    limits.max_read_bytes = limits
        .max_read_bytes
        .checked_sub(batch.bytes_read())
        .filter(|remaining| *remaining > 0)
        .ok_or_else(|| invalid("batch exhausted operation read budget"))?;
    // No object store or pointer exists merely because a batch parses.
    phase.set("candidate store and member preparation");
    let store = AdmissionStore::create(store_path, deadline, cancelled)?;
    let candidate = Candidate::prepare(&store, batch, identity, limits, deadline, cancelled)?;
    validator.account_candidate(&candidate)?;
    let mut unchanged = false;
    let manifest = match candidate.unchanged()? {
        Some(manifest) => {
            unchanged = true;
            manifest
        }
        None => {
            phase.set("complete candidate foundation and index validation");
            let validated = validator.validate(&candidate)?;
            validator.verify_store_authority(store_path)?;
            // This transition accounts all FND work and tightens candidate
            // publication limits to the remaining original operation budget.
            validator.account_candidate(&candidate)?;
            phase.set("candidate publication and accepted-pointer transaction");
            candidate.publish(validated.into_index())?
        }
    };
    validator.account_candidate(&candidate)?;
    phase.set("manifest and final custody");
    validator.account_manifest(&candidate, &manifest)?;
    if unchanged {
        validator.finalize_without_evaluation()?;
        validator.account_candidate(&candidate)?;
    }
    validator.verify_store_authority(store_path)?;
    phase.set("admission receipt");
    let receipt = candidate.receipt(&manifest)?;
    validator.write_receipt(&receipt, stdout)?;
    Ok(0)
}
