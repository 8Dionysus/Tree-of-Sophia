//! Maintained corpus-admit entry over the actual native foundation validator.
//! The batch names bytes and a validator identity; it never selects code.
use crate::source_admission::{AdmissionBatch, active, invalid};
use crate::source_admission_candidate::Candidate;
use crate::source_admission_spooled_candidate::{SpoolCandidate, SpooledPublicationReceipt};
use crate::source_admission_store::{AdmissionStore, StreamedPublicationRead};
use crate::source_current_cut::{
    foundation_command::SelectedOutput, foundation_entry::FoundationBootstrapClock,
};
use crate::source_foundation_admission::{
    NativeSourceValidator, NativeSpoolRefusal, PreparedAdmissionExecution, PreparedSpooledExecution,
};
use serde_json::json;
use std::fs::File;
use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    fmt,
    io::{self, Write},
    os::unix::ffi::OsStrExt,
    os::unix::fs::MetadataExt,
    path::{Component, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI32},
    },
    time::Instant,
};
use tos_foundation::{Digest256, SourceRevision};

pub const HELP: &str = "usage: tos-native-owner-command corpus-admit --store PATH --batch PATH --input-root PATH --grammar-root PATH --invocation PATH [--payload-source-root PATH] [--historical-capture PATH --historical-root PATH]...\n       tos-native-owner-command corpus-admit --store PATH --input-root PATH --fresh-record-owner ABSOLUTE_OWNER_CONFIG --fresh-record-id ID --fresh-record-transaction ID --fresh-record-base REVISION_SHA256 --grammar-root PATH --invocation PATH\n       tos-native-owner-command corpus-admit --store PATH --input-root PATH --initial-cut [--indexed-input-root PATH] --grammar-root PATH --invocation PATH\n       tos-native-owner-command corpus-admit --store PATH --input-root PATH --source-transition-base ORIGINAL_REVISION_SHA256 --grammar-root PATH --invocation PATH\n       tos-native-owner-command corpus-admit --validator-identity --grammar-root PATH --invocation PATH [--validation-profile ID] [--record-selection-manifest PATH] [validation selections]\n\nAdmit exact proposed source bytes through the selected complete native validator.\nThe invocation selects finite operation resources and pinned workers. No semantic admission or rights change is granted.\n";
pub const AUTHORED_BOOTSTRAP_HELP: &str = "usage: tos-native-owner-command authored-bootstrap --authored-bootstrap-owner ABSOLUTE_OWNER_CONFIG --store PATH --batch PATH --input-root PATH --grammar-root PATH --invocation PATH\n\nValidate the complete native-v4 candidate and publish its exact technical metadata bootstrap in the protected new private source root. The owner configuration pins the initial candidate and selects the fixed new metadata receipt; the existing transaction owner issues the ready epoch. An epoch-bound catalogue must subsequently complete under held source/currentness fences.\n";

struct FreshRevisionSelection {
    owner_configuration: PathBuf,
    record_id: String,
    transaction_id: String,
    original_base: SourceRevision,
}

struct Arguments {
    store: Option<PathBuf>,
    authored_bootstrap_owner: Option<PathBuf>,
    fresh_revision: Option<FreshRevisionSelection>,
    source_transition_base: Option<SourceRevision>,
    initial_cut: bool,
    batch: Option<PathBuf>,
    input: Option<PathBuf>,
    indexed_input_root: Option<PathBuf>,
    validator: Vec<OsString>,
    identity_only: bool,
    help: bool,
}

enum SpooledExecutionOutcome {
    Bootstrap {
        receipt: serde_json::Value,
    },
    Published {
        receipt: serde_json::Value,
        publication: SpooledPublicationReceipt,
        case: Option<io::Result<crate::source_admission_v2_case::V2CaseOutcome>>,
    },
    Recovered {
        accepted: crate::source_admission_v2_reader::AcceptedV2Publication,
        case: Option<io::Result<crate::source_admission_v2_case::V2CaseOutcome>>,
    },
}

fn recovered_publication_refusal(
    accepted: &crate::source_admission_v2_reader::AcceptedV2Publication,
    phase: &'static str,
    cause: io::Error,
) -> io::Error {
    io::Error::other(PublicationCommittedRefusal {
        phase,
        revision: accepted.revision.0,
        manifest_sha256: None,
        source_artifact: Some(accepted.source_artifact.clone()),
        rootset_sha256: None,
        history_proof_rootset_sha256: Some(accepted.history_proof_rootset_sha256),
        batch_sha256: accepted.batch_sha256,
        validator_sha256: accepted.validator_sha256,
        _custody: None,
        cause,
    })
}

/// The accepted pointer may already have advanced when post-publication
/// custody or empty-workspace cleanup refuses. Retain the exact revision and
/// source-record allocation through the outer bounded refusal path.
pub(crate) struct PublicationCommittedRefusal {
    pub(crate) phase: &'static str,
    pub(crate) revision: Digest256,
    pub(crate) manifest_sha256: Option<Digest256>,
    pub(crate) source_artifact:
        Option<crate::source_admission_segment_v2::SourceRevisionArtifactV2>,
    pub(crate) rootset_sha256: Option<Digest256>,
    pub(crate) history_proof_rootset_sha256: Option<Digest256>,
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
            .field("manifest_sha256", &self.manifest_sha256.map(|d| d.to_hex()))
            .field("source_artifact", &self.source_artifact)
            .field("rootset_sha256", &self.rootset_sha256.map(|d| d.to_hex()))
            .field(
                "history_proof_rootset_sha256",
                &self.history_proof_rootset_sha256.map(|d| d.to_hex()),
            )
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
        publication.source_artifact.clone(),
        publication.rootset_sha256,
        publication.fence.batch_sha256,
        publication.fence.validator_sha256,
        Some(Arc::clone(&publication.persistent_manifest_custody)),
        cause,
    )
}

fn publication_committed_refusal_parts(
    phase: &'static str,
    revision: Digest256,
    manifest_sha256: Option<Digest256>,
    source_artifact: Option<crate::source_admission_segment_v2::SourceRevisionArtifactV2>,
    rootset_sha256: Option<Digest256>,
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
            source_artifact,
            rootset_sha256,
            history_proof_rootset_sha256: None,
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
        authored_bootstrap_owner: None,
        fresh_revision: None,
        source_transition_base: None,
        initial_cut: false,
        batch: None,
        input: None,
        indexed_input_root: None,
        validator: Vec::new(),
        identity_only: false,
        help: false,
    };
    let mut fresh_owner = None;
    let mut fresh_record = None;
    let mut fresh_transaction = None;
    let mut fresh_base = None;
    let mut grammar = None;
    let mut invocation = None;
    let mut validation_profile = None;
    let mut record_selection_manifest = None;
    let mut payload = None;
    let mut captures = Vec::new();
    let mut roots = Vec::new();
    let mut position = 0;
    while position < args.len() {
        let option = args[position]
            .to_str()
            .ok_or_else(|| invalid("corpus admission option must be UTF-8"))?;
        position += 1;
        if option == "--validation-profile" || option.starts_with("--validation-profile=") {
            let id = if let Some(id) = option.strip_prefix("--validation-profile=") {
                OsString::from(id)
            } else {
                let id = args
                    .get(position)
                    .ok_or_else(|| invalid("corpus admission validation profile requires an ID"))?;
                position += 1;
                id.clone()
            };
            // The software-owner catalog validates this ID inside the same
            // immutable Foundation launch used for identity and admission.
            if validation_profile.replace(id).is_some() {
                return Err(invalid("duplicate corpus admission validation profile"));
            }
            continue;
        }
        if option == "--record-selection-manifest"
            || option.starts_with("--record-selection-manifest=")
        {
            let selected = if let Some(value) = option.strip_prefix("--record-selection-manifest=")
            {
                path(&OsString::from(value))?
            } else {
                let value = args
                    .get(position)
                    .ok_or_else(|| invalid("record selection manifest requires a path"))?;
                position += 1;
                path(value)?
            };
            if record_selection_manifest.replace(selected).is_some() {
                return Err(invalid("duplicate record selection manifest"));
            }
            continue;
        }
        match option {
            "--help" | "-h" => {
                result.help = true;
                return Ok(result);
            }
            "--validator-identity" => {
                result.identity_only = true;
                continue;
            }
            "--initial-cut" => {
                if result.initial_cut {
                    return Err(invalid("duplicate initial source cut selector"));
                }
                result.initial_cut = true;
                continue;
            }
            "--source-transition-base" => {
                let value = args
                    .get(position)
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| invalid("source transition base requires bounded UTF-8 text"))?;
                position += 1;
                if value.len() != 64
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(invalid(
                        "source transition requires exact lowercase original base SHA256",
                    ));
                }
                if result
                    .source_transition_base
                    .replace(SourceRevision(Digest256::from_hex(value).map_err(invalid)?))
                    .is_some()
                {
                    return Err(invalid("duplicate source transition original base"));
                }
                continue;
            }
            "--fresh-record-id" | "--fresh-record-transaction" | "--fresh-record-base" => {
                let value = args
                    .get(position)
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| {
                        invalid("fresh revision selection requires bounded UTF-8 text")
                    })?;
                position += 1;
                match option {
                    "--fresh-record-base" => {
                        if value.len() != 64
                            || !value
                                .bytes()
                                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                        {
                            return Err(invalid(
                                "fresh revision requires exact lowercase original base SHA256",
                            ));
                        }
                        if fresh_base
                            .replace(SourceRevision(Digest256::from_hex(value).map_err(invalid)?))
                            .is_some()
                        {
                            return Err(invalid("duplicate fresh revision original base"));
                        }
                    }
                    _ => {
                        if value.is_empty() || value.len() > 256 {
                            return Err(invalid(
                                "fresh revision owner selector exceeds text profile",
                            ));
                        }
                        let slot = if option == "--fresh-record-id" {
                            &mut fresh_record
                        } else {
                            &mut fresh_transaction
                        };
                        if slot.replace(value.to_owned()).is_some() {
                            return Err(invalid("duplicate fresh revision owner selector"));
                        }
                    }
                }
                continue;
            }
            "--store"
            | "--fresh-record-owner"
            | "--authored-bootstrap-owner"
            | "--batch"
            | "--input-root"
            | "--indexed-input-root"
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
            "--store"
            | "--authored-bootstrap-owner"
            | "--batch"
            | "--input-root"
            | "--indexed-input-root"
            | "--grammar-root"
            | "--payload-source-root" => {
                let selected = match option {
                    "--store" => &mut result.store,
                    "--authored-bootstrap-owner" => &mut result.authored_bootstrap_owner,
                    "--batch" => &mut result.batch,
                    "--input-root" => &mut result.input,
                    "--indexed-input-root" => &mut result.indexed_input_root,
                    "--grammar-root" => &mut grammar,
                    "--payload-source-root" => &mut payload,
                    _ => unreachable!(),
                };
                if selected.replace(value).is_some() {
                    return Err(invalid(format!("duplicate corpus admission {option}")));
                }
            }
            "--fresh-record-owner" => {
                if fresh_owner.replace(value).is_some() {
                    return Err(invalid("duplicate fresh revision protected owner"));
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
    if fresh_owner.is_some()
        || fresh_record.is_some()
        || fresh_transaction.is_some()
        || fresh_base.is_some()
    {
        result.fresh_revision = Some(FreshRevisionSelection {
            owner_configuration: fresh_owner
                .ok_or_else(|| invalid("fresh revision requires --fresh-record-owner"))?,
            record_id: fresh_record
                .ok_or_else(|| invalid("fresh revision requires --fresh-record-id"))?,
            transaction_id: fresh_transaction
                .ok_or_else(|| invalid("fresh revision requires --fresh-record-transaction"))?,
            original_base: fresh_base
                .ok_or_else(|| invalid("fresh revision requires --fresh-record-base"))?,
        });
        if result.batch.is_some()
            || result.identity_only
            || result.initial_cut
            || result.source_transition_base.is_some()
            || result.authored_bootstrap_owner.is_some()
        {
            return Err(invalid(
                "fresh revision cannot also select JSON batch, identity-only or authored bootstrap",
            ));
        }
    }
    if result.initial_cut
        && (result.batch.is_some()
            || result.identity_only
            || result.authored_bootstrap_owner.is_some()
            || result.fresh_revision.is_some()
            || result.source_transition_base.is_some())
    {
        return Err(invalid(
            "initial source cut cannot select JSON batch, identity-only, authored bootstrap, or fresh revision",
        ));
    }
    let selected_scope = validation_profile
        .as_ref()
        .map(|id| {
            let id = id
                .to_str()
                .ok_or_else(|| invalid("validation profile must be UTF-8"))?;
            crate::source_current_cut::foundation_cli::select_validation_profile(Some(id))
                .map(|profile| profile.scope)
                .map_err(invalid)
        })
        .transpose()?;
    let generated_profile = selected_scope == Some(tos_validation::source_foundation_default_rules::SourceFoundationDefaultRuleScope::SelectedGeneratedRecordClosure);
    if generated_profile && result.indexed_input_root.is_none() {
        return Err(invalid(
            "generated record closure requires indexed input root",
        ));
    }
    if selected_scope == Some(tos_validation::source_foundation_default_rules::SourceFoundationDefaultRuleScope::SelectedRecordClosure) && result.indexed_input_root.is_some() {
        return Err(invalid("finite record closure cannot select generated indexed input"));
    }
    if result.indexed_input_root.is_some()
        && !result.initial_cut
        && !(result.identity_only && generated_profile)
    {
        return Err(invalid(
            "indexed input requires the explicit initial source cut selector",
        ));
    }
    if result.source_transition_base.is_some()
        && (result.batch.is_some()
            || result.identity_only
            || result.initial_cut
            || result.fresh_revision.is_some()
            || result.authored_bootstrap_owner.is_some())
    {
        return Err(invalid(
            "source transition cannot also select JSON batch, identity-only, initial cut, fresh revision, or authored bootstrap",
        ));
    }
    if captures.len() != roots.len() {
        return Err(invalid(
            "each historical capture requires its exact restored root",
        ));
    }
    let grammar = grammar.ok_or_else(|| invalid("corpus admission requires --grammar-root"))?;
    let invocation = invocation.ok_or_else(|| invalid("corpus admission requires --invocation"))?;
    if result.identity_only {
        if result.store.is_some()
            || result.batch.is_some()
            || result.input.is_some()
            || result.source_transition_base.is_some()
        {
            return Err(invalid(
                "validator identity selection cannot also admit a batch",
            ));
        }
    } else if result.store.is_none()
        || (result.batch.is_none()
            && result.fresh_revision.is_none()
            && result.source_transition_base.is_none()
            && !result.initial_cut)
        || result.input.is_none()
    {
        return Err(invalid(
            "corpus admission requires --store, --batch or --initial-cut and --input-root",
        ));
    }
    result.validator.extend([
        OsString::from("--repo-root"),
        grammar.into_os_string(),
        OsString::from("--invocation"),
        invocation.into_os_string(),
    ]);
    if let Some(id) = validation_profile {
        result
            .validator
            .extend([OsString::from("--validation-profile"), id]);
    }
    if let Some(manifest) = record_selection_manifest {
        result.validator.extend([
            OsString::from("--record-selection-manifest"),
            manifest.into_os_string(),
        ]);
    }
    if generated_profile {
        result.validator.extend([
            OsString::from("--indexed-input-root"),
            result
                .indexed_input_root
                .as_ref()
                .expect("generated input selected")
                .as_os_str()
                .to_owned(),
        ]);
    }
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
#[derive(Debug)]
struct RetainedNativeSpoolRefusal {
    refusal: NativeSpoolRefusal,
    _custody: Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
}
impl std::fmt::Display for RetainedNativeSpoolRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt::Display::fmt(&self.refusal, f)
    }
}
impl std::error::Error for RetainedNativeSpoolRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.refusal)
    }
}

pub fn run(
    args: &[OsString],
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<i32> {
    run_with_cancel_owner(args, cancelled, None, git_signal, stdout, stderr, false)
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
        false,
    )
}

/// Explicit production bootstrap entry sharing the native validator, clock,
/// cancellation, finite invocation and refusal output with corpus admission.
pub fn run_authored_bootstrap_shared_cancel(
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
        true,
    )
}

fn run_with_cancel_owner(
    args: &[OsString],
    cancelled: &AtomicBool,
    cancel_owner: Option<&Arc<AtomicBool>>,
    git_signal: &AtomicI32,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    authored_bootstrap: bool,
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
        authored_bootstrap,
    );
    match result {
        Ok(code) => Ok(code),
        Err(error) => {
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
            if let Some(packet) = error.get_ref()
            .and_then(|source| source.downcast_ref::<crate::source_creation_store::authored_catalogue_bootstrap::BootstrapRefusal>())
            .and_then(|refusal| refusal.packet())
        {
            if writeln!(output, "{}", packet).and_then(|_| output.flush()).is_err() {
                let cause = error.into_inner().expect("typed bootstrap refusal retains owner");
                let refusal = cause.downcast::<crate::source_creation_store::authored_catalogue_bootstrap::BootstrapRefusal>()
                    .expect("checked bootstrap refusal type");
                return Err(io::Error::other((*refusal).with_output_refused()));
            }
        } else if let Some(committed) = error
            .get_ref()
            .and_then(|source| source.downcast_ref::<PublicationCommittedRefusal>())
        {
            let detail = committed
                .phase
                .strip_prefix("publication committed; ")
                .unwrap_or(committed.phase);
            let printed = match (
                committed.rootset_sha256,
                &committed.source_artifact,
                committed.manifest_sha256,
                committed.history_proof_rootset_sha256,
            ) {
                (Some(rootset), Some(record), _, _) => writeln!(
                    output,
                    "Native corpus V2 revision {} was committed (rootset {}, record {} {} in {}), but {}; restore by that exact revision digest.",
                    committed.revision.to_hex(),
                    rootset.to_hex(),
                    record.format(),
                    record.sha256().to_hex(),
                    record.filename(),
                    detail
                ),
                (Some(rootset), None, _, _) => writeln!(
                    output,
                    "Native corpus V2 revision {} was committed (rootset {}, source record unavailable), but {}; restore by that exact revision digest.",
                    committed.revision.to_hex(),
                    rootset.to_hex(),
                    detail
                ),
                (None, Some(record), _, Some(history_proof)) => writeln!(
                    output,
                    "Native corpus V2 revision {} was previously accepted (record {} {} in {}, history proof rootset {}), but {}; restore by that exact revision digest.",
                    committed.revision.to_hex(),
                    record.format(),
                    record.sha256().to_hex(),
                    record.filename(),
                    history_proof.to_hex(),
                    detail
                ),
                (None, _, Some(manifest), _) => writeln!(
                    output,
                    "Native corpus revision {} was committed (manifest {}), but {}; restore by that exact revision digest.",
                    committed.revision.to_hex(),
                    manifest.to_hex(),
                    detail
                ),
                _ => writeln!(
                    output,
                    "Native corpus revision {} was committed, but {}; restore by that exact revision digest.",
                    committed.revision.to_hex(),
                    detail
                ),
            };
            let _ = printed.and_then(|_| output.flush());
        } else if let Some(refusal) = error.get_ref()
            .and_then(|cause| cause.downcast_ref::<NativeSpoolRefusal>().or_else(|| cause.downcast_ref::<RetainedNativeSpoolRefusal>().map(|owner| &owner.refusal)))
        {
            // Stream the fixed-size diagnostic through the selected output
            // ledger; no independent buffer, retry writer or output allowance.
            let emitted = serde_json::to_writer(&mut output, &refusal.packet())
                .map_err(io::Error::other)
                .and_then(|_| writeln!(output))
                .and_then(|_| output.flush());
            if emitted.is_err() {
                let cause = error.into_inner().expect("checked typed spool refusal");
                return match cause.downcast::<NativeSpoolRefusal>() {
                    Ok(refusal) => Err(io::Error::other(refusal.with_output_refused())),
                    Err(cause) => {
                        let owner = *cause.downcast::<RetainedNativeSpoolRefusal>().expect("checked retained spool owner");
                        Err(io::Error::other(RetainedNativeSpoolRefusal {
                            refusal: owner.refusal.with_output_refused(),
                            _custody: owner._custody,
                        }))
                    }
                };
            }
        } else {
            let public_phase = error
                .get_ref()
                .and_then(|source| {
                    source.downcast_ref::<
                        crate::source_foundation_admission::NativeValidationRefusal,
                    >()
                })
                .map(|refusal| refusal.0.clone())
                .unwrap_or_else(|| crate::source_command::public_io_reason(&error));
            let _ = writeln!(
                output,
                "Native corpus admission refused during {}: {}",
                phase.get(),
                public_phase
            )
            .and_then(|_| output.flush());
        }
            Err(error)
        }
    }
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
    authored_bootstrap: bool,
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
        output.write_all(if authored_bootstrap {
            AUTHORED_BOOTSTRAP_HELP.as_bytes()
        } else {
            HELP.as_bytes()
        })?;
        output.flush()?;
        return Ok(0);
    }
    if args.authored_bootstrap_owner.is_some() != authored_bootstrap {
        return Err(invalid(
            "authored bootstrap requires its explicit command and protected owner",
        ));
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
        if args.authored_bootstrap_owner.is_some() {
            return Err(invalid(
                "authored bootstrap cannot be a validator identity request",
            ));
        }
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
        PreparedAdmissionExecution::Resident => {
            if args.initial_cut {
                return Err(invalid(
                    "initial source cut requires the protected spooled V2 route",
                ));
            }
            if args.fresh_revision.is_some() {
                return Err(invalid(
                    "fresh record revision requires the protected spooled V2 source route",
                ));
            }
            if args.source_transition_base.is_some() {
                return Err(invalid(
                    "source transition requires the protected spooled V2 source route",
                ));
            }
            if args.authored_bootstrap_owner.is_some() {
                return Err(invalid(
                    "authored bootstrap requires genuine native-v4 completion",
                ));
            }
        }
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
    let result = run_spooled_inner(
        args, validator, &resources, cancelled, store_path, identity, phase,
    );
    match result {
        Ok(SpooledExecutionOutcome::Recovered { accepted, case }) => {
            // Recover authenticated prior admission without a new completion,
            // CAS or historical pointer/allocation claim. A selected fresh
            // read/copy case retains its own actual target allocation custody.
            let (case_result, target_custody) = match case {
                None => (Ok(None), None),
                Some(Err(error)) => (Err(error), None),
                Some(Ok(outcome)) => (outcome.result.map(Some), Some(outcome.custody)),
            };
            let accounting = validator.account_spooled_external_io();
            let store = validator.verify_store_authority(store_path);
            drop(resources.workspace);
            let cleanup =
                validator.cleanup_spooled_workspace(&resources.workspace_root, cancelled.as_ref());
            for (result, failed_phase) in [
                (
                    cleanup,
                    "prior accepted publication; workspace cleanup failed",
                ),
                (
                    accounting,
                    "prior accepted publication; terminal IO accounting failed",
                ),
                (
                    store,
                    "prior accepted publication; store custody recheck failed",
                ),
            ] {
                if let Err(error) = result {
                    phase.set(failed_phase);
                    return Err(recovered_publication_refusal(
                        &accepted,
                        failed_phase,
                        retain_v2_allocation_custody(error, target_custody.clone()),
                    ));
                }
            }
            let mut receipt = recovered_receipt(&accepted);
            if let Some(accountant) = &resources.v2_allocation_accountant {
                receipt["source_new_allocated_bytes"] = json!(accountant.actual_allocated_bytes());
            }
            match case_result {
                Ok(Some(case)) => receipt["v2_case"] = case_receipt(&case),
                Ok(None) => {}
                Err(error) => {
                    phase.set("prior accepted publication; selected V2 read/restore case refused");
                    return Err(recovered_publication_refusal(
                        &accepted,
                        "prior accepted publication; selected V2 read/restore case refused",
                        retain_v2_allocation_custody(error, target_custody.clone()),
                    ));
                }
            }
            if let Err(error) = validator.write_receipt(&receipt, stdout) {
                phase.set("prior accepted publication; bounded receipt output failed");
                return Err(recovered_publication_refusal(
                    &accepted,
                    "prior accepted publication; bounded receipt output failed",
                    retain_v2_allocation_custody(error, target_custody.clone()),
                ));
            }
            Ok(0)
        }
        Ok(SpooledExecutionOutcome::Published {
            mut receipt,
            publication,
            case,
        }) => {
            // The publication result is real store state. Preserve its exact
            // source-record allocation custody through cleanup and bounded output.
            let (case_result, target_custody) = match case {
                None => (Ok(None), None),
                Some(Err(error)) => (Err(error), None),
                Some(Ok(outcome)) => (outcome.result.map(Some), Some(outcome.custody)),
            };
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
                    retain_v2_allocation_custody(error, target_custody.clone()),
                ));
            }
            if let Err(error) = accounting {
                phase.set("publication committed; terminal IO accounting failed");
                return Err(publication_committed_refusal(
                    &publication,
                    "publication committed; terminal IO accounting failed",
                    retain_v2_allocation_custody(error, target_custody.clone()),
                ));
            }
            if let Err(error) = store {
                phase.set("publication committed; store custody recheck failed");
                return Err(publication_committed_refusal(
                    &publication,
                    "publication committed; store custody recheck failed",
                    retain_v2_allocation_custody(error, target_custody.clone()),
                ));
            }
            if let Some(accountant) = &resources.v2_allocation_accountant {
                // This is measured new allocation, excluding retained baseline;
                // the terminal owner must account the resulting named store.
                receipt["source_new_allocated_bytes"] = json!(accountant.actual_allocated_bytes());
            }
            match case_result {
                Ok(Some(case)) => {
                    receipt["v2_case"] = case_receipt(&case);
                }
                Ok(None) => {}
                Err(error) => {
                    phase.set("publication committed; selected V2 read/restore case refused");
                    return Err(publication_committed_refusal(
                        &publication,
                        "publication committed; selected V2 read/restore case refused",
                        retain_v2_allocation_custody(error, target_custody.clone()),
                    ));
                }
            }
            if let Err(error) = validator.write_receipt(&receipt, stdout) {
                phase.set("publication committed; bounded receipt output failed");
                return Err(publication_committed_refusal(
                    &publication,
                    "publication committed; bounded receipt output failed",
                    retain_v2_allocation_custody(error, target_custody.clone()),
                ));
            }
            drop(publication);
            Ok(0)
        }
        Ok(SpooledExecutionOutcome::Bootstrap { receipt }) => {
            // Authored bootstrap publishes only its real metadata transaction.
            // It does not advance the auxiliary admission-store current pointer.
            let accounting = validator.account_spooled_external_io();
            let store = validator.verify_store_authority(store_path);
            drop(resources.workspace);
            let cleanup =
                validator.cleanup_spooled_workspace(&resources.workspace_root, cancelled.as_ref());
            let terminal = accounting
                .and(store)
                .and(cleanup)
                .and_then(|_| validator.write_receipt(&receipt, stdout));
            if let Err(error) = terminal {
                return Err(io::Error::other(
                    crate::source_creation_store::authored_catalogue_bootstrap::BootstrapRefusal::after_publication(receipt, error),
                ));
            }
            Ok(0)
        }
        Err(error) => {
            let primary_phase = phase.get();
            let primary_io = validator.spooled_invocation_io_snapshot();
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
                        refusal.source_artifact.clone(),
                        refusal.rootset_sha256,
                        refusal.batch_sha256,
                        refusal.validator_sha256,
                        refusal.persistent_manifest_custody.clone(),
                    )
                });
            let accounting = validator.account_spooled_external_io();
            drop(resources.workspace);
            let cleanup =
                validator.cleanup_spooled_workspace(&resources.workspace_root, cancelled.as_ref());
            if error.get_ref().is_some_and(|source| source.is::<crate::source_creation_store::authored_catalogue_bootstrap::BootstrapRefusal>()) {
                let cause = error.into_inner().expect("typed bootstrap error contains its owner cause");
                let refusal = cause.downcast::<crate::source_creation_store::authored_catalogue_bootstrap::BootstrapRefusal>()
                    .expect("checked bootstrap error type");
                return Err(io::Error::other((*refusal).with_terminal_checks(accounting.is_ok(), cleanup.is_ok())));
            }
            if let Some((
                revision,
                manifest,
                source_artifact,
                rootset,
                batch,
                validator_sha,
                custody,
            )) = committed
            {
                if let Err(cleanup_error) = cleanup {
                    phase.set("publication committed; isolated workspace cleanup failed");
                    return Err(publication_committed_refusal_parts(
                        "publication committed; isolated workspace cleanup failed",
                        revision,
                        manifest,
                        source_artifact.clone(),
                        rootset,
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
                        source_artifact.clone(),
                        rootset,
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
                    source_artifact,
                    rootset,
                    batch,
                    validator_sha,
                    custody,
                    error,
                ));
            }
            let terminal_io = validator.spooled_invocation_io_snapshot();
            let (primary_io, terminal_io) = match (primary_io, terminal_io) {
                (Ok(primary), Ok(terminal)) => (primary, terminal),
                // Mandatory accounting and cleanup above still run. Preserve
                // the primary cause; never label a partial census as exact.
                _ => return Err(error),
            };
            let refusal = NativeSpoolRefusal::retain(
                error,
                primary_phase,
                primary_io,
                terminal_io,
                accounting.is_err(),
                cleanup.is_err(),
            );
            match resources.v2_allocation_accountant.as_ref() {
                Some(accountant) => Err(io::Error::other(RetainedNativeSpoolRefusal {
                    refusal,
                    _custody: Some(accountant.custody_reservation()),
                })),
                None => Err(io::Error::other(refusal)),
            }
        }
    }
}

/// Consumes only the authentic completion's prepared numeric profile. The
/// protected existing invocation selects all target/cold-workspace slices.
struct V2AllocationCustodyRefusal {
    _custody: Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
    cause: io::Error,
}
impl fmt::Debug for V2AllocationCustodyRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("V2AllocationCustodyRefusal")
            .finish_non_exhaustive()
    }
}
impl fmt::Display for V2AllocationCustodyRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("V2 allocation terminal refusal")
    }
}
impl std::error::Error for V2AllocationCustodyRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
fn retain_v2_allocation_custody(
    cause: io::Error,
    custody: Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
) -> io::Error {
    io::Error::new(
        cause.kind(),
        V2AllocationCustodyRefusal {
            _custody: custody,
            cause,
        },
    )
}

fn run_selected_v2_case(
    selected: &crate::source_current_cut::foundation_entry::FoundationV2CaseSelection,
    profile: &crate::source_foundation_admission::NativeSegmentV2Budget,
    resources: &PreparedSpooledExecution,
    source: &std::path::Path,
    revision: SourceRevision,
    deadline: std::time::Instant,
    cancelled: &Arc<AtomicBool>,
    caller_retained_state_bytes: usize,
    work: crate::source_admission::AdmissionWorkBudget,
) -> io::Result<crate::source_admission_v2_case::V2CaseOutcome> {
    use crate::source_admission_v2_backup_restore::V2ImageLimits;
    use crate::source_admission_v2_case::{
        V2CaseLimits, cold_case_phase_overhead_bytes, read_backup_restore_case_with_cold_spill_at,
    };
    use crate::source_admission_v2_reader::V2PointReadLimits;
    use crate::source_admission_v2_seen_pack::{V2SeenPackSpillRequest, V2SeenPackSpillRequests};
    let source_root = resources
        .v2_source_root
        .as_ref()
        .ok_or_else(|| invalid("selected V2 case original source root absent"))?;
    if source_root.path != source {
        return Err(invalid("selected V2 case source root binding differs"));
    }
    let target_root = resources
        .v2_target_root
        .as_ref()
        .ok_or_else(|| invalid("selected V2 case original target root absent"))?;
    let target_relative = selected
        .target
        .strip_prefix(&target_root.path)
        .ok()
        .and_then(|relative| relative.to_str())
        .ok_or_else(|| invalid("selected V2 case target root binding differs"))?;
    let target_relative = tos_foundation::RelativePath::parse(target_relative).map_err(invalid)?;
    let phase_state = selected
        .state_bytes
        .checked_sub(cold_case_phase_overhead_bytes())
        .and_then(|bytes| bytes.checked_sub(caller_retained_state_bytes))
        .filter(|n| *n > 0 && *n <= profile.max_working_state_bytes)
        .ok_or_else(|| invalid("V2 selected case original state slice differs"))?;
    let mut pointer = resources.candidate_limits.candidate.reader;
    pointer.max_manifest_bytes = pointer.max_manifest_bytes.min(selected.pointer_bytes);
    pointer.json.max_bytes = pointer.json.max_bytes.min(selected.pointer_bytes);
    pointer.max_selected_object_bytes = pointer
        .max_selected_object_bytes
        .min(selected.object_bytes as u64);
    let mut image_tree = profile.tree_limits;
    image_tree.max_nodes = selected.tree_nodes;
    image_tree.max_total_bytes = selected.tree_bytes;
    image_tree.max_rows = selected.tree_rows;
    let mut point_tree = image_tree;
    point_tree.max_nodes = selected.point_tree_nodes;
    point_tree.max_total_bytes = selected.point_tree_bytes;
    let segment = tos_segment_store::SegmentLimits {
        max_segment_bytes: profile.max_allocated_bytes,
        max_frame_bytes: profile
            .max_frame_bytes
            .min(selected.object_bytes as u64)
            .max(1),
        max_frames: u32::try_from(profile.tree_limits.max_nodes.min(u32::MAX as u64))
            .map_err(|_| invalid("V2 case segment frame range"))?
            .max(1),
        max_journal_bytes: profile
            .max_working_state_bytes
            .min(4 * 1024 * 1024)
            .max(128),
    };
    let limits = V2CaseLimits {
        point: V2PointReadLimits {
            pointer,
            segment,
            tree: point_tree,
            max_object_bytes: selected.object_bytes,
            max_state_bytes: phase_state,
            caller_retained_state_bytes: 0,
        },
        image: V2ImageLimits {
            reader: pointer,
            segment,
            tree: image_tree,
            max_history_roots: selected.history_roots,
            max_files: selected.files,
            max_directories: selected.directories,
            max_depth: selected.depth,
            max_state_bytes: phase_state,
            max_new_allocated_bytes: selected.target_store_bytes,
            max_compatibility_manifest_bytes: selected.snapshot_bytes,
            allocation_unit_bytes: selected.allocation_unit_bytes,
        },
        max_total_tree_nodes: selected
            .point_tree_nodes
            .checked_mul(2)
            .and_then(|n| n.checked_add(selected.tree_nodes))
            .ok_or_else(|| invalid("V2 case whole node partition overflow"))?,
        max_total_tree_bytes: selected
            .point_tree_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(selected.tree_bytes))
            .ok_or_else(|| invalid("V2 case whole byte partition overflow"))?,
        max_state_bytes: selected.state_bytes,
    };
    let request = |workspace: File| {
        let mut caps = resources.request.limits;
        caps.main_logical_bytes = caps.main_logical_bytes.min(selected.aux_bytes);
        caps.main_allocated_bytes = caps.main_allocated_bytes.min(selected.aux_bytes);
        caps.temp_db_logical_bytes = caps.temp_db_logical_bytes.min(selected.aux_bytes);
        caps.temp_db_allocated_bytes = caps.temp_db_allocated_bytes.min(selected.aux_bytes);
        caps.main_journal_logical_bytes = caps.main_journal_logical_bytes.min(selected.aux_bytes);
        caps.main_journal_allocated_bytes =
            caps.main_journal_allocated_bytes.min(selected.aux_bytes);
        caps.temp_journal_logical_bytes = caps.temp_journal_logical_bytes.min(selected.aux_bytes);
        caps.temp_journal_allocated_bytes =
            caps.temp_journal_allocated_bytes.min(selected.aux_bytes);
        caps.other_aux_aggregate_logical_bytes = caps
            .other_aux_aggregate_logical_bytes
            .min(selected.aux_bytes);
        caps.other_aux_aggregate_allocated_bytes = caps
            .other_aux_aggregate_allocated_bytes
            .min(selected.aux_bytes);
        V2SeenPackSpillRequest {
            workspace,
            request: tos_source_store::PinnedSqliteAuxRequest {
                limits: caps,
                io_budget: profile.io.clone(),
                space_budget: resources.request.space_budget.clone(),
                deadline,
                cancelled: cancelled.clone(),
            },
            cache_bytes: selected.sqlite_cache_bytes,
            sqlite_native_overhead_bytes: selected.sqlite_native_overhead_bytes,
        }
    };
    // Each scope creates fresh unnamed inodes and a distinct VFS context in
    // this already-held private workspace. No source/target rows are shared.
    let paired = V2SeenPackSpillRequests {
        source: request(resources.workspace.try_clone()?),
        target: request(resources.workspace.try_clone()?),
    };
    read_backup_restore_case_with_cold_spill_at(
        source,
        &source_root.held,
        source_root.identity,
        &target_root.held,
        &target_root.path,
        target_root.identity,
        &selected.target,
        &target_relative,
        revision,
        &selected.identity,
        &selected.member_path,
        limits,
        profile.io.clone(),
        &profile.allocation_space,
        &resources.request.space_budget,
        paired,
        work,
        deadline,
        cancelled.clone(),
    )
}

// A narrower execution profile derived from the already selected invocation.
// All scans share its original IO, AUX storage, state, deadline and cancellation.
fn select_fresh_revision_profile(
    limits: crate::source_admission_spooled_candidate::SpoolLimits,
) -> io::Result<crate::source_admission_fresh_revision::FreshRevisionProfile> {
    use crate::source_admission_fresh_revision::FreshRevisionProfile;
    use crate::source_admission_source_census::SourceCensusLimits;
    use crate::source_work_transaction::RecordRevisionInspectionLimits;
    let original = limits.candidate;
    let state = original.max_state_bytes / 3;
    let members = u64::try_from(original.admission.max_members)
        .map_err(|_| invalid("fresh source member ceiling range"))?;
    let directories = members
        .checked_mul(16)
        .ok_or_else(|| invalid("fresh source directory ceiling overflow"))?
        .min(original.max_read_bytes / 4096);
    let manifest = (state / 256).min(original.admission.max_batch_bytes);
    let blobs = state / 128;
    let side = (state / 256).min(original.admission.max_batch_bytes);
    let inspection = RecordRevisionInspectionLimits {
        max_manifest_bytes: manifest,
        max_files: 3,
        max_side_bytes: side,
        max_total_side_bytes: side,
        max_total_blob_bytes: blobs,
    };
    let census = SourceCensusLimits {
        max_files: members,
        max_directories: directories,
        max_entries: original.max_read_bytes,
        max_path_bytes: 4096,
        max_depth: 128,
        max_member_bytes: original.admission.max_member_bytes,
        max_source_bytes: original.max_read_bytes,
        state_slice_bytes: 1, // The owner selector computes its checked minimum.
    };
    FreshRevisionProfile::from_original_limits(
        state,
        inspection,
        census,
        limits.bounded_batch_limits()?,
        limits.sqlite_cache_bytes,
        limits
            .max_row_state_bytes
            .checked_sub(limits.sqlite_cache_bytes)
            .ok_or_else(|| invalid("fresh SQL original native-state slice underflow"))?,
        original.max_read_bytes,
    )
}

fn select_initial_cut_profile(
    limits: crate::source_admission_spooled_candidate::SpoolLimits,
    indexed_input: bool,
) -> io::Result<crate::source_admission_initial_cut::InitialCutProfile> {
    use crate::source_admission_initial_cut::InitialCutProfile;
    use crate::source_admission_source_census::SourceCensusLimits;
    let original = limits.candidate;
    let admission = limits.bounded_batch_limits()?;
    let files = u64::try_from(admission.max_members)
        .map_err(|_| invalid("initial source file ceiling range"))?;
    let directories = files
        .checked_mul(16)
        .ok_or_else(|| invalid("initial source directory ceiling overflow"))?
        .min(original.max_read_bytes / 4096);
    let entries = files
        .checked_add(directories)
        .ok_or_else(|| invalid("initial source entry ceiling overflow"))?;
    let source_bytes = admission.max_source_bytes.min(original.max_read_bytes);
    let census = SourceCensusLimits {
        max_files: files,
        max_directories: directories,
        max_entries: entries,
        max_path_bytes: 4096.min(admission.max_batch_bytes),
        max_depth: 128,
        max_member_bytes: admission.max_member_bytes.min(source_bytes),
        max_source_bytes: source_bytes,
        state_slice_bytes: 1, // The owner selector computes its checked minimum.
    };
    InitialCutProfile::from_original_limits(
        original.max_state_bytes / 3,
        census,
        admission,
        limits.sqlite_cache_bytes,
        limits
            .max_row_state_bytes
            .checked_sub(limits.sqlite_cache_bytes)
            .ok_or_else(|| invalid("initial SQL original native-state slice underflow"))?,
        indexed_input,
        original.max_read_bytes,
    )
}

fn select_source_transition_profile(
    limits: crate::source_admission_spooled_candidate::SpoolLimits,
) -> io::Result<crate::source_admission_source_transition::SourceTransitionProfile> {
    use crate::source_admission_source_census::SourceCensusLimits;
    use crate::source_admission_source_transition::SourceTransitionProfile;
    let original = limits.candidate;
    let admission = limits.bounded_batch_limits()?;
    let files = u64::try_from(admission.max_members)
        .map_err(|_| invalid("source transition file ceiling range"))?;
    let directories = files
        .checked_mul(16)
        .ok_or_else(|| invalid("source transition directory ceiling overflow"))?
        .min(original.max_read_bytes / 4096);
    let entries = files
        .checked_add(directories)
        .ok_or_else(|| invalid("source transition entry ceiling overflow"))?;
    let source_bytes = admission.max_source_bytes.min(original.max_read_bytes);
    let census = SourceCensusLimits {
        max_files: files,
        max_directories: directories,
        max_entries: entries,
        max_path_bytes: 4096.min(admission.max_batch_bytes),
        max_depth: 128,
        max_member_bytes: admission.max_member_bytes.min(source_bytes),
        max_source_bytes: source_bytes,
        state_slice_bytes: 1, // The owner selector computes its checked minimum.
    };
    SourceTransitionProfile::from_original_limits(
        original.max_state_bytes / 3,
        census,
        admission,
        admission.max_members,
        limits.sqlite_cache_bytes,
        limits
            .max_row_state_bytes
            .checked_sub(limits.sqlite_cache_bytes)
            .ok_or_else(|| invalid("source transition SQL native-state slice underflow"))?,
        original.max_read_bytes,
    )
}

fn open_source_transition_root(
    path: &std::path::Path,
    io: &tos_source_store::PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<(File, (u64, u64))> {
    let path_bytes = path.as_os_str().as_bytes();
    let mut components = path.components();
    let count = components.clone().count();
    if !path.is_absolute()
        || path_bytes.is_empty()
        || path_bytes.len() > 4096
        || count > 128
        || components.clone().any(|part| {
            matches!(
                part,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(invalid(
            "source transition input root is not bounded and normalized",
        ));
    }
    let mut guard = 4096u64;
    for part in components {
        let bytes = match part {
            Component::Normal(name) => name.as_bytes().len(),
            Component::RootDir => 0,
            _ => return Err(invalid("source transition input root component differs")),
        };
        let component =
            u64::try_from(bytes).map_err(|_| invalid("source transition input component range"))?;
        guard = guard
            .checked_add(component)
            .and_then(|n| n.checked_add(4097))
            .ok_or_else(|| invalid("source transition input root metadata bound overflow"))?;
    }
    io.charge_read_upper_bound(guard).map_err(invalid)?;
    active(deadline, cancel)?;
    let root = tos_fd_open::open_absolute_directory(path).map_err(invalid)?;
    let metadata = root.metadata()?;
    if !metadata.is_dir() {
        return Err(invalid("source transition input root is not a directory"));
    }
    Ok((root, (metadata.dev(), metadata.ino())))
}

fn run_spooled_inner(
    args: &Arguments,
    validator: &mut NativeSourceValidator<'_>,
    resources: &PreparedSpooledExecution,
    cancelled: &Arc<AtomicBool>,
    store_path: &std::path::Path,
    identity: Digest256,
    phase: &Cell<&'static str>,
) -> io::Result<SpooledExecutionOutcome> {
    if args.authored_bootstrap_owner.is_some() && resources.v2_allocation_accountant.is_some() {
        return Err(invalid(
            "authored bootstrap does not publish a segment V2 source cut",
        ));
    }
    if args.fresh_revision.is_some()
        && (resources.v2_allocation_accountant.is_none()
            || resources.v2_source_root.is_none()
            || resources.v2_base_read_limits.is_none())
    {
        return Err(invalid(
            "fresh record revision requires existing protected V2 allocation and held base reader",
        ));
    }
    if args.initial_cut
        && (resources.v2_allocation_accountant.is_none() || resources.v2_source_root.is_none())
    {
        return Err(invalid(
            "initial source cut requires the protected V2 allocation and held source root",
        ));
    }
    if args.source_transition_base.is_some()
        && (resources.v2_allocation_accountant.is_none()
            || resources.v2_source_root.is_none()
            || resources.v2_base_read_limits.is_none())
    {
        return Err(invalid(
            "source transition requires the protected V2 allocation and held original reader",
        ));
    }
    let deadline = validator.deadline();
    let mut limits = resources.candidate_limits;
    let request = &resources.request;
    if request.deadline != deadline || !Arc::ptr_eq(&request.cancelled, cancelled) {
        return Err(invalid("spooled entry resource identity changed"));
    }

    let initial_profile = if args.initial_cut {
        Some(select_initial_cut_profile(
            limits,
            args.indexed_input_root.is_some(),
        )?)
    } else {
        None
    };
    let transition_profile = if args.source_transition_base.is_some() {
        Some(select_source_transition_profile(limits)?)
    } else {
        None
    };
    let mut initial_store = if initial_profile.is_some() {
        let source_root = resources
            .v2_source_root
            .as_ref()
            .ok_or_else(|| invalid("initial V2 source root absent"))?;
        if source_root.path != store_path {
            return Err(invalid("initial selected V2 source root binding differs"));
        }
        let accountant = resources
            .v2_allocation_accountant
            .as_ref()
            .ok_or_else(|| invalid("initial V2 original allocation owner absent"))?;
        Some(AdmissionStore::create_or_open_v2_at_named(
            store_path,
            &source_root.held,
            accountant.clone(),
            deadline,
            &request.cancelled,
        )?)
    } else {
        None
    };
    // Parse once under the protected shared physical budget and check the
    // selected validator before the store can create any namespace.
    let fresh_profile = if args.fresh_revision.is_some() {
        Some(select_fresh_revision_profile(limits)?)
    } else {
        None
    };
    // Reserve the whole preparation envelope before reading owner configuration
    // or constructing Work/census state. It stays on the original ledger.
    let fresh_filesystem = if let (Some(selection), Some(profile)) =
        (&args.fresh_revision, fresh_profile)
    {
        validator
            .reserve_spooled_external_state(profile.max_state_slice_bytes, &request.io_budget)?;
        let root = args.input.as_deref().unwrap();
        let component_bytes = selection
            .owner_configuration
            .as_os_str()
            .as_bytes()
            .len()
            .checked_add(root.as_os_str().as_bytes().len())
            .ok_or_else(|| invalid("fresh owner selection path bound overflow"))?;
        let owner_read = u64::try_from(component_bytes)
            .map_err(|_| invalid("fresh owner selection path range"))?
            .checked_mul(4096)
            .and_then(|n| n.checked_add(1_048_576 + 65_536))
            .ok_or_else(|| invalid("fresh owner selection read bound overflow"))?;
        request
            .io_budget
            .charge_read_upper_bound(owner_read)
            .map_err(invalid)?;
        let (filesystem, configuration_raw) =
            crate::source_creation_store::CreationFilesystem::select_protected_native_owner_at_root(
                &selection.owner_configuration, root, deadline, &request.cancelled,
            ).map_err(|_| invalid("fresh protected source owner selection refused"))?;
        drop(configuration_raw);
        Some(filesystem)
    } else {
        None
    };
    let mut initial_prepared = None;
    let mut fresh_prepared = None;
    let mut fresh_reader = None;
    let mut transition_prepared = None;
    let mut transition_reader = None;
    let mut transition_root = None;
    let mut transition_root_identity = None;
    phase.set("native-v4 batch parsing");
    let batch = if let Some(profile) = initial_profile {
        phase.set("native-v4 initial source cut preparation");
        let source_root = resources
            .v2_source_root
            .as_ref()
            .ok_or_else(|| invalid("initial V2 source root absent"))?;
        if source_root.path != store_path {
            return Err(invalid("initial selected V2 source root binding differs"));
        }
        let accountant = resources
            .v2_allocation_accountant
            .as_ref()
            .ok_or_else(|| invalid("initial V2 original allocation owner absent"))?;
        let store = initial_store
            .as_ref()
            .ok_or_else(|| invalid("initial V2 store absent"))?;
        let indexed_input = if let Some(named_root) = args.indexed_input_root.as_deref() {
            let original = resources
                .v2_base_read_limits
                .ok_or_else(|| invalid("indexed input requires the selected V2 read profile"))?;
            // The base reader authenticates metadata only and deliberately has
            // a one-byte payload bound. Indexed source bytes instead inherit
            // the initial-cut census bound selected by this invocation.
            let max_input_bytes = usize::try_from(profile.census.max_member_bytes)
                .map_err(|_| invalid("indexed input member bound exceeds range"))?;
            let max_members = profile.census.max_files.min(original.tree.max_rows);
            let max_objects = max_members
                .min(original.tree.max_rows)
                .min(original.tree.max_nodes);
            let caller_live_state_bytes = profile
                .census_state_bytes
                .checked_add(profile.update_rows_state_bytes)
                .and_then(|bytes| bytes.checked_add(profile.batch_builder_state_bytes))
                .and_then(|bytes| bytes.checked_add(profile.store_namespace_state_bytes))
                .and_then(|bytes| bytes.checked_add(profile.retained_fence_state_bytes))
                .and_then(|bytes| bytes.checked_add(profile.sqlite_cache_bytes))
                .and_then(|bytes| bytes.checked_add(profile.sqlite_native_overhead_bytes))
                .ok_or_else(|| invalid("indexed input retained state bound overflow"))?;
            let max_working_state_bytes =
                profile.max_state_slice_bytes.min(original.max_state_bytes);
            let mut member_tree = original.tree;
            member_tree.max_rows = member_tree.max_rows.min(max_members);
            member_tree.max_key_bytes =
                member_tree.max_key_bytes.min(profile.census.max_path_bytes);
            member_tree.max_value_bytes = member_tree.max_value_bytes.min(44);
            let mut object_tree = original.tree;
            object_tree.max_rows = object_tree.max_rows.min(max_objects);
            object_tree.max_key_bytes = object_tree.max_key_bytes.min(32);
            object_tree.max_value_bytes = object_tree.max_value_bytes.min(76);
            Some(crate::source_admission_indexed_input::IndexedInputRequestV1 {
                held_declaration: validator.take_indexed_input_declaration_v1()?,
                named_root: named_root.to_path_buf(),
                segment_limits: original.segment,
                max_profile_bytes: max_input_bytes.min(
                    crate::source_admission_indexed_input::PROFILE_SIDECAR_MAX_BYTES_V1,
                ),
                max_dependency_closure_bytes: max_input_bytes.min(
                    crate::source_admission_indexed_input::DEPENDENCY_CLOSURE_MAX_BYTES_V1,
                ),
                reader_limits: crate::source_admission_indexed_input::IndexedInputLimitsV1 {
                    member_tree,
                    packed_objects: crate::source_admission_packed_objects::PackedObjectLimitsV2 {
                        segment_limits: original.segment,
                        tree_limits: object_tree,
                        max_working_state_bytes,
                        caller_live_state_bytes,
                        max_work_units: profile.max_work_units,
                        max_objects,
                        max_delta_rows: max_objects,
                        max_pack_frames: original.segment.max_frames.min(
                            crate::source_admission_packed_objects::MAX_PACKED_OBJECT_FRAMES_V2,
                        ),
                    },
                    max_members,
                    max_member_bytes: profile.census.max_member_bytes,
                    max_source_bytes: profile.census.max_source_bytes,
                    max_descriptor_bytes: (12 * 1024).min(max_input_bytes),
                    caller_retained_state_bytes: caller_live_state_bytes,
                },
            })
        } else {
            None
        };
        let mut prepared = crate::source_admission_initial_cut::prepare_initial_cut(
            args.input.as_deref().unwrap(),
            validator,
            store,
            limits.candidate.reader,
            accountant,
            profile,
            resources.workspace.try_clone()?,
            tos_source_store::PinnedSqliteAuxRequest {
                limits: request.limits,
                io_budget: request.io_budget.clone(),
                space_budget: request.space_budget.clone(),
                deadline: request.deadline,
                cancelled: request.cancelled.clone(),
            },
            indexed_input,
            deadline,
            &request.cancelled,
        )?;
        let batch = prepared.take_batch()?;
        initial_prepared = Some(prepared);
        limits.candidate.max_state_bytes = limits
            .candidate
            .max_state_bytes
            .min(validator.candidate_limits()?.max_state_bytes);
        batch
    } else if let (Some(original_base), Some(profile)) =
        (args.source_transition_base, transition_profile)
    {
        let source_root = resources
            .v2_source_root
            .as_ref()
            .ok_or_else(|| invalid("source transition original V2 root absent"))?;
        if source_root.path != store_path {
            return Err(invalid("source transition V2 source root binding differs"));
        }
        let accountant = resources
            .v2_allocation_accountant
            .as_ref()
            .ok_or_else(|| invalid("source transition original V2 allocation owner absent"))?;
        validator
            .reserve_spooled_external_state(profile.max_state_slice_bytes, &request.io_budget)?;
        let mut read_limits = resources
            .v2_base_read_limits
            .ok_or_else(|| invalid("source transition original V2 read profile absent"))?;
        read_limits.max_state_bytes = read_limits
            .max_state_bytes
            .min(validator.candidate_limits()?.max_state_bytes);
        let mut reader = crate::source_admission_v2_reader::V2ReadSession::open_at_named_with_io(
            store_path,
            &source_root.held,
            read_limits,
            accountant.io_budget().clone(),
            deadline,
            request.cancelled.clone(),
        )?;
        let (root, root_identity) = open_source_transition_root(
            args.input
                .as_deref()
                .ok_or_else(|| invalid("source transition input root absent"))?,
            &request.io_budget,
            deadline,
            &request.cancelled,
        )?;
        transition_root = Some(root);
        transition_root_identity = Some(root_identity);
        let root = transition_root
            .as_ref()
            .ok_or_else(|| invalid("source transition held input root absent"))?;
        let mut prepared = crate::source_admission_source_transition::prepare_source_transition(
            &*validator,
            &mut reader,
            original_base,
            root,
            profile,
            resources.workspace.try_clone()?,
            tos_source_store::PinnedSqliteAuxRequest {
                limits: request.limits,
                io_budget: request.io_budget.clone(),
                space_budget: request.space_budget.clone(),
                deadline: request.deadline,
                cancelled: request.cancelled.clone(),
            },
            identity,
            deadline,
            &request.cancelled,
        )?;
        let batch = prepared.take_batch()?;
        transition_prepared = Some(prepared);
        transition_reader = Some(reader);
        limits.candidate.max_state_bytes = limits
            .candidate
            .max_state_bytes
            .min(validator.candidate_limits()?.max_state_bytes);
        batch
    } else if let (Some(selection), Some(profile), Some(filesystem)) = (
        &args.fresh_revision,
        fresh_profile,
        fresh_filesystem.as_ref(),
    ) {
        let source_root = resources
            .v2_source_root
            .as_ref()
            .ok_or_else(|| invalid("fresh V2 source root absent"))?;
        if source_root.path != store_path {
            return Err(invalid("fresh selected V2 source root binding differs"));
        }
        let accountant = resources
            .v2_allocation_accountant
            .as_ref()
            .ok_or_else(|| invalid("fresh V2 original allocation owner absent"))?;
        let mut read_limits = resources
            .v2_base_read_limits
            .ok_or_else(|| invalid("fresh V2 original read profile absent"))?;
        // The Work/census envelope is already reserved outside this reader;
        // its own retained/transient allocations use only remaining headroom.
        read_limits.max_state_bytes = read_limits
            .max_state_bytes
            .min(validator.candidate_limits()?.max_state_bytes);
        let mut reader = crate::source_admission_v2_reader::V2ReadSession::open_at_named_with_io(
            store_path,
            &source_root.held,
            read_limits,
            accountant.io_budget().clone(),
            deadline,
            request.cancelled.clone(),
        )?;
        let mut prepared = crate::source_admission_fresh_revision::prepare_record_revision(
            filesystem,
            &*validator,
            &mut reader,
            selection.original_base,
            &selection.transaction_id,
            &selection.record_id,
            profile,
            resources.workspace.try_clone()?,
            tos_source_store::PinnedSqliteAuxRequest {
                limits: request.limits,
                io_budget: request.io_budget.clone(),
                space_budget: request.space_budget.clone(),
                deadline: request.deadline,
                cancelled: request.cancelled.clone(),
            },
            identity,
            deadline,
            &request.cancelled,
        )?;
        let batch = prepared.take_batch()?;
        fresh_prepared = Some(prepared);
        fresh_reader = Some(reader);
        // The operation's ledger, not the stale pre-preparation cap, owns the
        // candidate headroom while the source/Work fence remains live.
        limits.candidate.max_state_bytes = limits
            .candidate
            .max_state_bytes
            .min(validator.candidate_limits()?.max_state_bytes);
        batch
    } else {
        AdmissionBatch::read_budgeted(
            args.batch.as_deref().unwrap(),
            args.input.as_deref().unwrap(),
            limits.bounded_batch_limits()?,
            deadline,
            &request.cancelled,
            &request.io_budget,
        )?
    };
    if batch.validator_sha256 != identity {
        return Err(invalid(
            "batch validator identity does not match selected native program and grammar",
        ));
    }

    phase.set("native-v4 store creation");
    let store = match initial_store.take() {
        Some(store) => store,
        None => match &resources.v2_allocation_accountant {
            Some(accountant) => {
                let source_root = resources
                    .v2_source_root
                    .as_ref()
                    .ok_or_else(|| invalid("selected V2 original source root absent"))?;
                if source_root.path != store_path {
                    return Err(invalid("selected V2 source root binding differs"));
                }
                AdmissionStore::create_or_open_v2_at_named(
                    store_path,
                    &source_root.held,
                    accountant.clone(),
                    deadline,
                    &request.cancelled,
                )?
            }
            None => AdmissionStore::create(store_path, deadline, &request.cancelled)?,
        },
    };
    let pointer_io = resources
        .v2_allocation_accountant
        .as_ref()
        .map_or(&request.io_budget, |owner| owner.io_budget());
    // Output selection does not change the format of an authenticated base.
    // Keep V1 imports on their maintained streamed reader; only a genuine V2
    // selector can provide a V2 session or an accepted-batch history proof.
    let selected_format = if resources.v2_allocation_accountant.is_some() {
        store
            .current_selection(
                limits.candidate.reader,
                deadline,
                &request.cancelled,
                Some(pointer_io),
            )?
            .map(|selection| selection.format)
    } else {
        None
    };
    let mut probed_v2 = None;
    if selected_format == Some(tos_source_store::CorpusPointerFormat::V2) {
        let source_root = resources
            .v2_source_root
            .as_ref()
            .ok_or_else(|| invalid("selected V2 original source root absent"))?;
        let mut read_limits = resources
            .v2_base_read_limits
            .ok_or_else(|| invalid("selected V2 base read limits absent"))?;
        if args.initial_cut
            || args.fresh_revision.is_some()
            || args.source_transition_base.is_some()
        {
            read_limits.max_state_bytes = read_limits
                .max_state_bytes
                .min(validator.candidate_limits()?.max_state_bytes);
        }
        let preopened = transition_reader.take().or_else(|| fresh_reader.take());
        let mut reader = match preopened {
            Some(reader) => reader,
            None => crate::source_admission_v2_reader::V2ReadSession::open_at_named_with_io(
                store_path,
                &source_root.held,
                read_limits,
                pointer_io.clone(),
                deadline,
                request.cancelled.clone(),
            )?,
        };
        if let Some(accepted) = reader.find_accepted_batch(
            batch.batch_sha256,
            batch.base_revision.map(SourceRevision),
            identity,
            resources.manifest_limits.max_manifest_bytes,
        )? {
            if let Some(prepared) = fresh_prepared.as_mut() {
                prepared.verify_retry_outcome(&mut reader)?;
            }
            if let Some(prepared) = transition_prepared.as_mut() {
                prepared.verify_lookup_fence(&mut reader)?;
            }
            drop(fresh_prepared);
            drop(initial_prepared);
            drop(transition_prepared);
            drop(transition_root);
            drop(reader);
            let case_work = resources
                .v2_case
                .as_ref()
                .map(|_| batch.admission_work_budget());
            drop(batch);
            drop(store); // The case reopens only from the original held root.
            let case = resources.v2_case.as_ref().map(|selection| {
                validator
                    .prepared_v2_read_case_profile()
                    .and_then(|profile| {
                        let work = case_work
                            .ok_or_else(|| invalid("selected V2 case work meter absent"))??;
                        run_selected_v2_case(
                            selection,
                            profile,
                            resources,
                            store_path,
                            accepted.revision,
                            deadline,
                            cancelled,
                            std::mem::size_of::<
                                crate::source_admission_v2_reader::AcceptedV2Publication,
                            >(),
                            work,
                        )
                    })
            });
            return Ok(SpooledExecutionOutcome::Recovered { accepted, case });
        }
        if fresh_prepared.as_ref().is_some_and(|prepared|
            prepared.disposition() == crate::source_admission_fresh_revision::FreshRevisionDisposition::RetryLookupOnly)
        {
            return Err(invalid("fresh retry has no exact accepted V2 batch outcome"));
        }
        if let Some(prepared) = transition_prepared.as_mut() {
            prepared.verify_original_base_after_lookup_miss(&mut reader)?;
        }
        probed_v2 = Some(RefCell::new(reader));
    }
    if fresh_reader.is_some() || transition_reader.is_some() {
        return Err(invalid(
            "selected source lost its authenticated V2 selector",
        ));
    }
    if let Some(prepared) = initial_prepared.as_mut() {
        let accountant = resources
            .v2_allocation_accountant
            .as_ref()
            .ok_or_else(|| invalid("initial V2 original allocation owner absent"))?;
        prepared.verify_initial_miss_baseline(
            validator,
            &store,
            limits.candidate.reader,
            accountant,
        )?;
    }
    phase.set("native-v4 current revision check");
    store.check_current_budgeted(
        batch.base_revision,
        limits.candidate.reader,
        deadline,
        &request.cancelled,
        pointer_io,
    )?;
    phase.set("native-v4 base cut selection");
    let base = match (
        batch.base_revision,
        resources.v2_allocation_accountant.is_none()
            || selected_format == Some(tos_source_store::CorpusPointerFormat::V1),
    ) {
        (Some(revision), true) => {
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
        _ => None,
    };
    // Read-only limits are selected by the protected invocation. This borrows
    // authenticated held V2 rows before validation; it grants no publication
    // authority and uses the original shared source IO ledger.
    let base_v2 = if batch.base_revision.is_some()
        && resources.v2_allocation_accountant.is_some()
        && selected_format == Some(tos_source_store::CorpusPointerFormat::V2)
    {
        let source_root = resources
            .v2_source_root
            .as_ref()
            .ok_or_else(|| invalid("selected V2 original source root absent"))?;
        let read_limits = resources
            .v2_base_read_limits
            .ok_or_else(|| invalid("selected V2 base read limits absent"))?;
        match probed_v2.take() {
            Some(reader) => Some(reader),
            None => Some(RefCell::new(
                crate::source_admission_v2_reader::V2ReadSession::open_at_named_with_io(
                    store_path,
                    &source_root.held,
                    read_limits,
                    pointer_io.clone(),
                    deadline,
                    request.cancelled.clone(),
                )?,
            )),
        }
    } else {
        None
    };
    let candidate_workspace = resources.workspace.try_clone()?;
    let candidate_request = tos_source_store::PinnedSqliteAuxRequest {
        limits: request.limits,
        io_budget: request.io_budget.clone(),
        space_budget: request.space_budget.clone(),
        deadline: request.deadline,
        cancelled: request.cancelled.clone(),
    };
    phase.set("native-v4 candidate preparation");
    let candidate = SpoolCandidate::prepare_selected_batch_with_v2_base(
        &store,
        batch,
        identity,
        base.as_ref(),
        base_v2.as_ref(),
        resources
            .v2_allocation_accountant
            .as_ref()
            .map(|owner| owner.io_budget()),
        candidate_workspace,
        candidate_request,
        limits,
        deadline,
        request.cancelled.clone(),
    )?;
    phase.set("native-v4 candidate accounting");
    validator.account_spooled_candidate(&candidate)?;
    phase.set("native-v4 whole foundation validation");
    let index = validator.validate_spooled(&candidate, resources.index_limits)?;
    phase.set("native-v4 final store authority");
    validator.verify_store_authority(store_path)?;

    phase.set("native-v4 authored bootstrap publication");
    if let Some(owner) = &args.authored_bootstrap_owner {
        let output_cap = validator.remaining_output_bytes()?;
        let mut fence = || {
            validator
                .finalize_without_evaluation()
                .and_then(|_| validator.verify_store_authority(store_path))
                .map_err(|_| {
                    crate::source_command::SourceCommandError::Conflict(
                        "authored bootstrap protected native invocation changed",
                    )
                })
        };
        let bootstrap = crate::source_creation_store::authored_catalogue_bootstrap::publish(
            owner,
            &candidate,
            &index,
            resources.candidate_limits,
            resources.index_limits,
            output_cap,
            deadline,
            cancelled,
            &mut fence,
        )
        .map_err(|refusal| io::Error::other(refusal.with_candidate_io(candidate.io_snapshot())))?;
        let receipt = bootstrap
            .value()
            .map_err(|error| invalid(format!("authored bootstrap encoding refused: {error:?}")))?;
        drop(index);
        drop(candidate);
        drop(base);
        drop(base_v2);
        return Ok(SpooledExecutionOutcome::Bootstrap { receipt });
    }

    phase.set("native-v4 publication cut preparation");
    let case_profile = if let Some(selection) = &resources.v2_case {
        let target_root = resources
            .v2_target_root
            .as_ref()
            .ok_or_else(|| invalid("selected V2 case original target root absent"))?;
        if selection.target == target_root.path || !selection.target.starts_with(&target_root.path)
        {
            return Err(invalid("selected V2 case target root binding differs"));
        }
        // Resolve the selected tuple from the genuinely completed index before
        // publication. A path may own several identities; seek one bounded row
        // at a time and refuse once the ordered cursor passes the requested ID.
        let mut after = None;
        loop {
            let found = index.identity_for_path_after(&selection.member_path, after.as_deref())?;
            match found {
                Some(id) if id == selection.identity => break,
                Some(id) if id < selection.identity => after = Some(id),
                _ => return Err(invalid("selected V2 case identity/member tuple is absent")),
            }
        }
        Some(
            index
                .segment_v2_budget()
                .cloned()
                .ok_or_else(|| invalid("selected V2 case lacks genuine completion profile"))?,
        )
    } else {
        None
    };
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
    phase.set("native-v4 corpus publication");
    let publication = if let Some(prepared) = initial_prepared.as_mut() {
        phase.set("native-v4 initial source terminal fence");
        let accountant = resources
            .v2_allocation_accountant
            .as_ref()
            .ok_or_else(|| invalid("initial V2 original allocation owner absent"))?;
        let mut fence = || {
            prepared.verify_before_publish(
                validator,
                &store,
                limits.candidate.reader,
                accountant,
            )?;
            validator.account_spooled_external_io()
        };
        candidate.publish_validated_initial_checked(
            &index,
            resources.manifest_limits,
            resources.max_manifest_allocated_bytes,
            streamed,
            &mut fence,
        )?
    } else if let Some(prepared) = transition_prepared.as_mut() {
        phase.set("native-v4 source transition terminal fence");
        let base = base_v2
            .as_ref()
            .ok_or_else(|| invalid("source transition original V2 session absent"))?;
        let root = transition_root
            .as_ref()
            .ok_or_else(|| invalid("source transition held input root absent"))?;
        let root_identity = transition_root_identity
            .ok_or_else(|| invalid("source transition root identity absent"))?;
        let root_name = args
            .input
            .as_deref()
            .ok_or_else(|| invalid("source transition named input root absent"))?;
        let root_io = request.io_budget.clone();
        let root_cancel = request.cancelled.clone();
        let mut fence = || {
            prepared.verify_before_publish(&mut base.borrow_mut())?;
            crate::source_admission_v2_backup_restore::verify_named_root(
                root_name,
                root,
                root_identity,
                &root_io,
                deadline,
                &root_cancel,
            )?;
            validator.account_spooled_external_io()
        };
        candidate.publish_validated_successor_checked(
            &index,
            resources.manifest_limits,
            resources.max_manifest_allocated_bytes,
            streamed,
            &mut fence,
        )?
    } else {
        if let Some(prepared) = fresh_prepared.as_mut() {
            phase.set("native-v4 fresh source terminal fence");
            let base = base_v2
                .as_ref()
                .ok_or_else(|| invalid("fresh terminal original V2 session absent"))?;
            prepared.verify_before_publish(&mut base.borrow_mut())?;
            validator.account_spooled_external_io()?;
        }
        candidate.publish_validated(
            &index,
            resources.manifest_limits,
            resources.max_manifest_allocated_bytes,
            streamed,
        )?
    };
    // Keep receipt JSON out of the cold phase; the fixed publication tuple
    // remains live and is debited from the same original case state below.
    let case_work = resources
        .v2_case
        .as_ref()
        .map(|_| candidate.admission_work_budget());
    drop(index);
    drop(candidate);
    drop(fresh_prepared);
    drop(initial_prepared);
    drop(transition_prepared);
    drop(transition_root);
    drop(base);
    drop(base_v2);
    drop(store); // Publication custody survives; no duplicate store stays live.
    let case = resources.v2_case.as_ref().map(|selection| {
        let work = case_work.ok_or_else(|| invalid("selected V2 case work meter absent"))??;
        run_selected_v2_case(
            selection,
            case_profile
                .as_ref()
                .ok_or_else(|| invalid("selected V2 case completion profile disappeared"))?,
            resources,
            store_path,
            publication.revision,
            deadline,
            cancelled,
            std::mem::size_of::<SpooledPublicationReceipt>()
                .checked_add(std::mem::size_of::<
                    Option<crate::source_foundation_admission::NativeSegmentV2Budget>,
                >())
                .ok_or_else(|| invalid("V2 case retained publication/profile state overflow"))?,
            work,
        )
    });
    let receipt = spooled_receipt(&publication);
    Ok(SpooledExecutionOutcome::Published {
        receipt,
        publication,
        case,
    })
}

fn case_receipt(case: &crate::source_admission_v2_case::V2CaseReceipt) -> serde_json::Value {
    json!({
        "observed_revision":case.observed_revision.0.to_hex(),
        "observed_path":case.observed_path.as_str(),
        "observed_sha256":case.observed_sha256.to_hex(),
        "observed_bytes":case.observed_bytes,
        "target_allocated_bytes":case.image.allocated_bytes,
        "copied_files":case.image.copied_files,
        "copied_directories":case.image.copied_directories,
        "tree_read_nodes":case.image.tree_read_nodes,
        "semantic_admission":false,"rights_change":false
    })
}

fn recovered_receipt(
    accepted: &crate::source_admission_v2_reader::AcceptedV2Publication,
) -> serde_json::Value {
    let artifact = &accepted.source_artifact;
    let mut source_record = json!({
        "format":artifact.format(), "sha256":artifact.sha256().to_hex(),
        "file":artifact.filename()
    });
    if let Some(bytes) = artifact.bytes() {
        source_record["bytes"] = json!(bytes);
    }
    json!({
        "schema_version":"tos_corpus_admission_receipt_v1",
        "recovered_accepted_result":true,
        "batch_sha256":accepted.batch_sha256.to_hex(),
        "revision":accepted.revision.0.to_hex(),
        "base_revision":accepted.base_revision.map(|revision| revision.0.to_hex()),
        "validator_sha256":accepted.validator_sha256.to_hex(),
        "members":accepted.member_count,
        "identities":accepted.identity_count,
        "source_bytes":accepted.source_bytes,
        "source_record":source_record,
        "history_proof_rootset_sha256":accepted.history_proof_rootset_sha256.to_hex(),
        "semantic_admission":false,"rights_change":false
    })
}

fn spooled_receipt(publication: &SpooledPublicationReceipt) -> serde_json::Value {
    let fence = publication.fence;
    let mut receipt = json!({
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
    });
    if let Some(rootset) = publication.rootset_sha256 {
        receipt["rootset_sha256"] = json!(rootset.to_hex());
        if let Some(record) = &publication.source_artifact {
            let mut value = json!({
                "format":record.format(), "sha256":record.sha256().to_hex(),
                "file":record.filename()
            });
            if let Some(bytes) = record.bytes() {
                value["bytes"] = json!(bytes);
            }
            receipt["source_record"] = value;
        }
    }
    receipt
}

#[cfg(test)]
mod validation_profile_forwarding_tests {
    use super::*;

    fn identity_args(extra: &[&str]) -> Vec<OsString> {
        [
            "--validator-identity",
            "--grammar-root",
            "/grammar",
            "--invocation",
            "/invocation",
        ]
        .into_iter()
        .chain(extra.iter().copied())
        .map(OsString::from)
        .collect()
    }

    #[test]
    fn profile_selection_reaches_the_same_foundation_launch() {
        let default = parse(&identity_args(&[])).unwrap();
        assert!(
            !default
                .validator
                .iter()
                .any(|arg| arg == "--validation-profile")
        );
        for extra in [
            vec!["--validation-profile", "selected-source-closure"],
            vec!["--validation-profile=selected-source-closure"],
        ] {
            let selected = parse(&identity_args(&extra)).unwrap();
            assert_eq!(
                &selected.validator[selected.validator.len() - 2..],
                &[
                    OsString::from("--validation-profile"),
                    OsString::from("selected-source-closure")
                ]
            );
        }

        for extra in [
            vec!["--record-selection-manifest", "/selection.json"],
            vec!["--record-selection-manifest=/selection.json"],
        ] {
            let selected = parse(&identity_args(&extra)).unwrap();
            assert_eq!(
                &selected.validator[selected.validator.len() - 2..],
                &[
                    OsString::from("--record-selection-manifest"),
                    OsString::from("/selection.json")
                ]
            );
        }
        let generated = parse(&identity_args(&[
            "--validation-profile=selected-generated-record-closure",
            "--record-selection-manifest=/selection.json",
            "--indexed-input-root",
            "/indexed",
        ]))
        .unwrap();
        assert_eq!(
            generated.indexed_input_root.as_deref(),
            Some(std::path::Path::new("/indexed"))
        );
        assert!(generated.validator.windows(2).any(|pair| pair
            == [
                OsString::from("--indexed-input-root"),
                OsString::from("/indexed")
            ]));
        assert!(parse(&identity_args(&["--indexed-input-root", "/indexed"])).is_err());
        assert!(
            parse(&identity_args(&[
                "--validation-profile=selected-record-closure",
                "--record-selection-manifest=/selection.json",
                "--indexed-input-root",
                "/indexed",
            ]))
            .is_err()
        );
        assert!(parse(&identity_args(&["--record-selection-manifest"])).is_err());
        assert!(
            parse(&identity_args(&[
                "--record-selection-manifest=/selection.json",
                "--record-selection-manifest=/other.json"
            ]))
            .is_err()
        );
        assert!(parse(&identity_args(&["--validation-profile"])).is_err());
        assert!(
            parse(&identity_args(&[
                "--validation-profile",
                "full-audit",
                "--validation-profile=selected-source-closure"
            ]))
            .is_err()
        );
    }
}
