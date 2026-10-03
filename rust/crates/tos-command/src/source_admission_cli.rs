//! Maintained corpus-admit entry over the actual native foundation validator.
//! The batch names bytes and a validator identity; it never selects code.
use crate::source_admission::{AdmissionBatch, invalid};
use crate::source_admission_candidate::Candidate;
use crate::source_admission_spooled_candidate::{SpoolCandidate, SpooledPublicationReceipt};
use crate::source_admission_store::{AdmissionStore, StreamedPublicationRead};
use crate::source_current_cut::{
    foundation_command::SelectedOutput, foundation_entry::FoundationBootstrapClock,
};
use crate::source_foundation_admission::{
    NativeSourceValidator, PreparedAdmissionExecution, PreparedSpooledExecution,
};
use serde_json::json;
use std::fs::File;
use std::{
    cell::Cell,
    ffi::OsString,
    fmt,
    io::{self, Write},
    os::unix::ffi::OsStrExt,
    path::{Component, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32},
    },
    time::Instant,
};
use tos_foundation::{Digest256, SourceRevision};

pub const HELP: &str = "usage: tos-native-owner-command corpus-admit --store PATH --batch PATH --input-root PATH --grammar-root PATH --invocation PATH [--payload-source-root PATH] [--historical-capture PATH --historical-root PATH]...\n       tos-native-owner-command corpus-admit --validator-identity --grammar-root PATH --invocation PATH [validation selections]\n\nAdmit exact proposed source bytes through the selected complete native validator.\nThe invocation selects finite operation resources and pinned workers. No semantic admission or rights change is granted.\n";

struct Arguments {
    store: Option<PathBuf>,
    batch: Option<PathBuf>,
    input: Option<PathBuf>,
    validator: Vec<OsString>,
    identity_only: bool,
    help: bool,
}

/// The accepted pointer may already have advanced when post-publication
/// custody or empty-workspace cleanup refuses. Retain the exact revision and
/// manifest allocation through the outer bounded refusal path.
pub(crate) struct PublicationCommittedRefusal {
    pub(crate) phase: &'static str,
    pub(crate) revision: Digest256,
    pub(crate) manifest_sha256: Digest256,
    pub(crate) batch_sha256: Digest256,
    pub(crate) validator_sha256: Digest256,
    _custody: Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
    cause: io::Error,
}

impl fmt::Debug for PublicationCommittedRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationCommittedRefusal")
            .field("phase", &self.phase)
            .field("revision", &self.revision.to_hex())
            .field("manifest_sha256", &self.manifest_sha256.to_hex())
            .field("batch_sha256", &self.batch_sha256.to_hex())
            .field("validator_sha256", &self.validator_sha256.to_hex())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for PublicationCommittedRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.phase)
    }
}

impl std::error::Error for PublicationCommittedRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

fn publication_committed_refusal(
    publication: &SpooledPublicationReceipt,
    phase: &'static str,
    cause: io::Error,
) -> io::Error {
    publication_committed_refusal_parts(
        phase,
        publication.revision.0,
        publication.manifest_sha256,
        publication.fence.batch_sha256,
        publication.fence.validator_sha256,
        Some(Arc::clone(&publication.persistent_manifest_custody)),
        cause,
    )
}

fn publication_committed_refusal_parts(
    phase: &'static str,
    revision: Digest256,
    manifest_sha256: Digest256,
    batch_sha256: Digest256,
    validator_sha256: Digest256,
    custody: Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
    cause: io::Error,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::Other,
        PublicationCommittedRefusal {
            phase,
            revision,
            manifest_sha256,
            batch_sha256,
            validator_sha256,
            _custody: custody,
            cause,
        },
    )
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
    run_with_cancel_owner(args, cancelled, None, git_signal, stdout, stderr)
}

/// Native binary entry retaining its actual shared cancellation owner. The
/// borrowed compatibility entry above intentionally cannot select native-v4.
pub fn run_shared_cancel(
    args: &[OsString],
    cancelled: &Arc<AtomicBool>,
    git_signal: &AtomicI32,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<i32> {
    run_with_cancel_owner(
        args,
        cancelled.as_ref(),
        Some(cancelled),
        git_signal,
        stdout,
        stderr,
    )
}

fn run_with_cancel_owner(
    args: &[OsString],
    cancelled: &AtomicBool,
    cancel_owner: Option<&Arc<AtomicBool>>,
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
        cancel_owner,
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
        if let Some(committed) = error
            .get_ref()
            .and_then(|source| source.downcast_ref::<PublicationCommittedRefusal>())
        {
            let detail = committed
                .phase
                .strip_prefix("publication committed; ")
                .unwrap_or(committed.phase);
            let _ = writeln!(
                output,
                "Native corpus revision {} was committed (manifest {}), but {}; restore by that exact revision digest.",
                committed.revision.to_hex(),
                committed.manifest_sha256.to_hex(),
                detail
            )
            .and_then(|_| output.flush());
        } else {
            let public_phase = error
                .get_ref()
                .and_then(|source| {
                    source.downcast_ref::<
                        crate::source_foundation_admission::NativeValidationRefusal,
                    >()
                })
                .map_or(phase.get(), |refusal| refusal.0.as_str());
            let _ = writeln!(
                output,
                "Native corpus admission refused during {}",
                public_phase
            )
            .and_then(|_| output.flush());
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run_selected(
    args: &[OsString],
    cancelled: &AtomicBool,
    cancel_owner: Option<&Arc<AtomicBool>>,
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
    let select_output = |cap, deadline| {
        output_cap.set(usize::try_from(cap).unwrap_or(usize::MAX));
        output_deadline.set(deadline);
    };
    let mut validator = match cancel_owner {
        Some(cancel_owner) => NativeSourceValidator::prepare_shared_cancel(
            clock,
            &args.validator,
            cancel_owner,
            git_signal,
            select_output,
        )?,
        None => NativeSourceValidator::prepare(
            clock,
            &args.validator,
            cancelled,
            git_signal,
            select_output,
        )?,
    };
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
    phase.set("admission execution profile");
    let execution = validator.prepared_execution_resources()?;
    match execution {
        PreparedAdmissionExecution::Spooled(resources) => {
            phase.set("native-v4 spooled corpus admission");
            let Some(cancel_owner) = cancel_owner else {
                drop(resources.workspace);
                validator.cleanup_spooled_workspace(&resources.workspace_root, cancelled)?;
                return Err(invalid(
                    "native-v4 entry requires original shared cancellation",
                ));
            };
            return run_spooled(
                &args,
                &mut validator,
                resources,
                cancel_owner,
                store_path,
                identity,
                phase,
                stdout,
            );
        }
        PreparedAdmissionExecution::Resident => (),
    }
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

fn run_spooled(
    args: &Arguments,
    validator: &mut NativeSourceValidator<'_>,
    resources: PreparedSpooledExecution,
    cancelled: &Arc<AtomicBool>,
    store_path: &std::path::Path,
    identity: Digest256,
    phase: &Cell<&'static str>,
    stdout: &mut dyn Write,
) -> io::Result<i32> {
    let result = run_spooled_inner(args, validator, &resources, cancelled, store_path, identity);
    match result {
        Ok((receipt, publication)) => {
            // The publication result is real store state. Preserve its exact
            // manifest allocation custody through cleanup and bounded output.
            let accounting = validator.account_spooled_external_io();
            let store = validator.verify_store_authority(store_path);
            drop(resources.workspace);
            let cleanup =
                validator.cleanup_spooled_workspace(&resources.workspace_root, cancelled.as_ref());
            if let Err(error) = cleanup {
                phase.set("publication committed; isolated workspace cleanup failed");
                return Err(publication_committed_refusal(
                    &publication,
                    "publication committed; isolated workspace cleanup failed",
                    error,
                ));
            }
            if let Err(error) = accounting {
                phase.set("publication committed; terminal IO accounting failed");
                return Err(publication_committed_refusal(
                    &publication,
                    "publication committed; terminal IO accounting failed",
                    error,
                ));
            }
            if let Err(error) = store {
                phase.set("publication committed; store custody recheck failed");
                return Err(publication_committed_refusal(
                    &publication,
                    "publication committed; store custody recheck failed",
                    error,
                ));
            }
            if let Err(error) = validator.write_receipt(&receipt, stdout) {
                phase.set("publication committed; bounded receipt output failed");
                return Err(publication_committed_refusal(
                    &publication,
                    "publication committed; bounded receipt output failed",
                    error,
                ));
            }
            drop(publication);
            Ok(0)
        }
        Err(error) => {
            // Record attempted shared-ledger IO even on parse, identity, base,
            // native-kernel or publication refusal. Cleanup is exact and
            // empty-only; a replaced/nonempty workspace remains untouched.
            let committed = error
                .get_ref()
                .and_then(|source| {
                    source.downcast_ref::<
                        crate::source_admission_spooled_candidate::
                            SpooledPublicationCommittedRefusal,
                    >()
                })
                .map(|refusal| {
                    (
                        refusal.revision.0,
                        refusal.manifest_sha256,
                        refusal.batch_sha256,
                        refusal.validator_sha256,
                        refusal.persistent_manifest_custody.clone(),
                    )
                });
            let accounting = validator.account_spooled_external_io();
            drop(resources.workspace);
            let cleanup =
                validator.cleanup_spooled_workspace(&resources.workspace_root, cancelled.as_ref());
            if let Some((revision, manifest, batch, validator_sha, custody)) = committed {
                if let Err(cleanup_error) = cleanup {
                    phase.set("publication committed; isolated workspace cleanup failed");
                    return Err(publication_committed_refusal_parts(
                        "publication committed; isolated workspace cleanup failed",
                        revision,
                        manifest,
                        batch,
                        validator_sha,
                        custody,
                        cleanup_error,
                    ));
                }
                if let Err(accounting_error) = accounting {
                    phase.set("publication committed; terminal IO accounting failed");
                    return Err(publication_committed_refusal_parts(
                        "publication committed; terminal IO accounting failed",
                        revision,
                        manifest,
                        batch,
                        validator_sha,
                        custody,
                        accounting_error,
                    ));
                }
                phase.set("publication committed; post-CAS verification failed");
                return Err(publication_committed_refusal_parts(
                    "publication committed; post-CAS verification failed",
                    revision,
                    manifest,
                    batch,
                    validator_sha,
                    custody,
                    error,
                ));
            }
            match (cleanup, accounting) {
                (Err(cleanup_error), _) => Err(cleanup_error),
                (Ok(()), Err(accounting_error)) => Err(accounting_error),
                (Ok(()), Ok(())) => Err(error),
            }
        }
    }
}

fn run_spooled_inner(
    args: &Arguments,
    validator: &mut NativeSourceValidator<'_>,
    resources: &PreparedSpooledExecution,
    cancelled: &Arc<AtomicBool>,
    store_path: &std::path::Path,
    identity: Digest256,
) -> io::Result<(serde_json::Value, SpooledPublicationReceipt)> {
    let deadline = validator.deadline();
    let limits = resources.candidate_limits;
    let request = &resources.request;
    if request.deadline != deadline || !Arc::ptr_eq(&request.cancelled, cancelled) {
        return Err(invalid("spooled entry resource identity changed"));
    }

    // Parse once under the protected shared physical budget and check the
    // selected validator before the store can create any namespace.
    let batch = AdmissionBatch::read_budgeted(
        args.batch.as_deref().unwrap(),
        args.input.as_deref().unwrap(),
        limits.bounded_batch_limits()?,
        deadline,
        &request.cancelled,
        &request.io_budget,
    )?;
    if batch.validator_sha256 != identity {
        return Err(invalid(
            "batch validator identity does not match selected native program and grammar",
        ));
    }

    let store = AdmissionStore::create(store_path, deadline, &request.cancelled)?;
    store.check_current_budgeted(
        batch.base_revision,
        limits.candidate.reader,
        deadline,
        &request.cancelled,
        &request.io_budget,
    )?;
    let base = match batch.base_revision {
        Some(revision) => {
            let main = File::from(rustix::fs::openat(
                &resources.workspace,
                ".",
                rustix::fs::OFlags::TMPFILE
                    | rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::from_raw_mode(0o600),
            )?);
            let reader = store
                .reader(limits.candidate.reader)?
                .open_source_cut_streamed_budgeted(
                    SourceRevision(revision),
                    resources.streamed_cut_limits,
                    main,
                    request.io_budget.clone(),
                    request.space_budget.clone(),
                    resources.max_index_allocated_bytes,
                    deadline,
                    request.cancelled.clone(),
                )
                .map_err(invalid)?;
            Some(reader)
        }
        None => None,
    };
    let candidate_workspace = resources.workspace.try_clone()?;
    let candidate_request = tos_source_store::PinnedSqliteAuxRequest {
        limits: request.limits,
        io_budget: request.io_budget.clone(),
        space_budget: request.space_budget.clone(),
        deadline: request.deadline,
        cancelled: request.cancelled.clone(),
    };
    let candidate = SpoolCandidate::prepare_selected_batch(
        &store,
        batch,
        identity,
        base.as_ref(),
        candidate_workspace,
        candidate_request,
        limits,
        deadline,
        request.cancelled.clone(),
    )?;
    validator.account_spooled_candidate(&candidate)?;
    let index = validator.validate_spooled(&candidate, resources.index_limits)?;
    validator.verify_store_authority(store_path)?;

    let cut = candidate.create_streamed_cut_workspace_file()?;
    let streamed = StreamedPublicationRead {
        limits: resources.streamed_cut_limits,
        index_file: cut.main,
        io_budget: cut.io_budget,
        space_budget: cut.space_budget,
        max_index_allocated_bytes: resources.max_index_allocated_bytes,
        max_manifest_allocated_bytes: resources.max_manifest_allocated_bytes,
        cancelled: cut.cancelled,
    };
    let publication = candidate.publish_validated(
        &index,
        resources.manifest_limits,
        resources.max_manifest_allocated_bytes,
        streamed,
    )?;
    let receipt = spooled_receipt(&publication);
    drop(index);
    drop(candidate);
    drop(base);
    Ok((receipt, publication))
}

fn spooled_receipt(publication: &SpooledPublicationReceipt) -> serde_json::Value {
    let fence = publication.fence;
    json!({
        "schema_version":"tos_corpus_admission_receipt_v1",
        "batch_sha256":fence.batch_sha256.to_hex(),
        "revision":publication.revision.0.to_hex(),
        "base_revision":fence.base_revision.map(|revision| revision.0.to_hex()),
        "validator_sha256":fence.validator_sha256.to_hex(),
        "members":fence.membership.count,
        "identities":publication.identities,
        "source_bytes":fence.source_bytes,
        "semantic_admission":false,
        "rights_change":false
    })
}
