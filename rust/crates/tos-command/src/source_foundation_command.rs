//! Native foundation CLI boundary over the authenticated whole-operation
//! controller. Only complete owner/diagnostic evidence reaches normal output.

use super::foundation_bootstrap::FoundationBootstrapInputs;
use super::foundation_entry::{FoundationBootstrapClock, parse_launch_arguments, read_invocation};
use super::foundation_execution_limits::{
    FoundationCharge, FoundationPhaseReservation, FoundationPhaseUse,
};
use super::foundation_output::SourceFoundationOutputOutcome;
use super::{foundation_bootstrap_config, foundation_cli, foundation_orchestrator};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use std::cell::Cell;
use std::ffi::OsString;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Instant;

pub(crate) struct SelectedOutput<'a, W: ?Sized> {
    pub(crate) writer: &'a mut W,
    pub(crate) bytes: &'a Cell<usize>,
    pub(crate) stopped: &'a Cell<bool>,
    pub(crate) max_bytes: usize,
    pub(crate) deadline: Instant,
    pub(crate) cancelled: &'a AtomicBool,
}

impl<W: Write + ?Sized> SelectedOutput<'_, W> {
    fn active(&self) -> io::Result<()> {
        if self.stopped.get()
            || self.cancelled.load(Ordering::Relaxed)
            || Instant::now() >= self.deadline
        {
            self.stopped.set(true);
            Err(io::Error::other("foundation output stopped"))
        } else {
            Ok(())
        }
    }
}

impl<W: Write + ?Sized> Write for SelectedOutput<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.active()?;
        if bytes.len() > self.max_bytes.saturating_sub(self.bytes.get()) {
            self.stopped.set(true);
            return Err(io::Error::other("foundation output byte limit"));
        }
        let written = match self.writer.write(bytes) {
            Ok(written) if written <= bytes.len() && (written != 0 || bytes.is_empty()) => written,
            _ => {
                self.stopped.set(true);
                return Err(io::Error::other("foundation output failed"));
            }
        };
        let total = self
            .bytes
            .get()
            .checked_add(written)
            .filter(|total| *total <= self.max_bytes)
            .ok_or_else(|| {
                self.stopped.set(true);
                io::Error::other("foundation output byte limit")
            })?;
        self.bytes.set(total);
        self.active()?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.active()?;
        if self.writer.flush().is_err() {
            self.stopped.set(true);
            return Err(io::Error::other("foundation output failed"));
        }
        self.active()
    }
}

/// Continue one selected CLI invocation with the caller's cancellation and
/// read-only Git termination controls. No phase replaces either signal.
/// Repository, worker and private-stage selection come from the protected
/// invocation and explicit CLI arguments, never the current directory.
pub fn run(
    args: &[OsString],
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Result<i32> {
    run_with_observation(args, cancelled, git_signal, stdout, stderr, None)
}

/// Additional callback peak state (including retained output and closure) and
/// cumulative private-stage reads. The caller enforces these finite bounds;
/// the native owner reserves and charges them against the original invocation.
/// No callback worker, filesystem mutation or publication is admitted here.
#[derive(Clone, Copy, Debug, Default)]
pub struct SourceFoundationCatalogueObservationLimits {
    pub max_stage_read_bytes: u64,
    pub max_state_bytes: usize,
}

/// Observe one genuine rendered/validated catalog in its borrowed cold stage.
/// Keep results private until Ok(0) AND callback invocation are confirmed.
/// Terminal source/root/epoch fences run afterward; custody ends on return.
pub fn run_with_owned_catalogue_observation(
    args: &[OsString],
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    limits: SourceFoundationCatalogueObservationLimits,
    observe: impl FnOnce(
        &mut tos_compiler::knowledge_stage::KnowledgeStage<'_>,
        &tos_compiler::source_witness_catalog::ColdSourceCatalogReceipt,
        tos_compiler::source_witness_catalog::SourceCatalogLimits,
    ) -> tos_compiler::Result<()>,
) -> Result<i32> {
    let callback_state = std::mem::size_of_val(&observe)
        .checked_add(std::mem::size_of::<
            foundation_orchestrator::OwnedCatalogueObservation<'_>,
        >())
        .ok_or(Error::Unsupported(
            "catalogue observation closure state overflow",
        ))?;
    if limits.max_state_bytes < callback_state || limits.max_stage_read_bytes == u64::MAX {
        return Err(Error::Unsupported(
            "catalogue observation finite resource bounds",
        ));
    }
    run_with_observation(
        args,
        cancelled,
        git_signal,
        stdout,
        stderr,
        Some(foundation_orchestrator::OwnedCatalogueObservation {
            limits,
            callback: Box::new(observe),
            retained_closure_state_bytes: callback_state,
        }),
    )
}

fn run_with_observation(
    args: &[OsString],
    cancelled: &AtomicBool,
    git_signal: &AtomicI32,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    observation: Option<foundation_orchestrator::OwnedCatalogueObservation<'_>>,
) -> Result<i32> {
    let clock = FoundationBootstrapClock::begin()?;
    // Before a protected invocation is decoded, only bounded bootstrap help
    // or static refusal text can be emitted. Once decoded, its exact output
    // ceiling and deadline govern every normal and failure write together.
    let bytes = Cell::new(0);
    let stopped = Cell::new(false);
    let output_cap = Cell::new(4096);
    let output_deadline = Cell::new(clock.hard_deadline());
    let result = run_selected(
        args,
        cancelled,
        git_signal,
        stdout,
        stderr,
        clock,
        &bytes,
        &stopped,
        &output_cap,
        &output_deadline,
        observation,
    );
    if let Err(error) = &result {
        let reason = match error {
            Error::Invalid(reason) | Error::Denied(reason) | Error::Unsupported(reason) => *reason,
            _ => "foundation selected input refused",
        };
        let mut output = SelectedOutput {
            writer: stderr,
            bytes: &bytes,
            stopped: &stopped,
            max_bytes: output_cap.get(),
            deadline: output_deadline.get(),
            cancelled,
        };
        // A stopped/exhausted/failed output cannot escape through a second
        // unbounded printer. The original typed failure remains authoritative.
        let _ = writeln!(
            output,
            "Source-witness foundation evaluation incomplete: {reason}"
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
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    clock: FoundationBootstrapClock,
    bytes: &Cell<usize>,
    stopped: &Cell<bool>,
    output_cap: &Cell<usize>,
    output_deadline: &Cell<Instant>,
    observation: Option<foundation_orchestrator::OwnedCatalogueObservation<'_>>,
) -> Result<i32> {
    let launch = parse_launch_arguments(args)?;
    if launch.arguments.help {
        let mut output = SelectedOutput {
            writer: stdout,
            bytes,
            stopped,
            max_bytes: output_cap.get(),
            deadline: output_deadline.get(),
            cancelled,
        };
        output
            .write_all(foundation_cli::HELP.as_bytes())
            .and_then(|_| output.flush())
            .map_err(|_| Error::Denied("foundation stdout failed"))?;
        return Ok(0);
    }
    let mut invocation = read_invocation(&clock, &launch, cancelled)?;
    if let Some(observation) = &observation {
        // The callback closure is already held during bootstrap. Charge its
        // actual conservative state alongside the existing invocation input;
        // the later observation reservation separately covers its output peak.
        invocation.cost.retained_input_bytes = invocation
            .cost
            .retained_input_bytes
            .checked_add(observation.retained_closure_state_bytes)
            .ok_or(Error::Unsupported(
                "catalogue observation retained startup state overflow",
            ))?;
        invocation.cost.peak_state_upper_bound_bytes = invocation
            .cost
            .peak_state_upper_bound_bytes
            .checked_add(observation.retained_closure_state_bytes)
            .ok_or(Error::Unsupported(
                "catalogue observation bootstrap peak overflow",
            ))?;
    }
    let deadline = invocation.deadline();
    let max_output_bytes = usize::try_from(invocation.budgets.max_output_bytes)
        .map_err(|_| Error::Unsupported("foundation output byte range"))?;
    output_cap.set(max_output_bytes);
    output_deadline.set(deadline);
    // This source route is the whole default connector. Selected lab-only
    // entry completion remains a distinct connection obligation; default
    // evaluation cannot stand in for a lab report or its control semantics.
    if launch.arguments.selected_lab.is_some() {
        return Err(Error::Unsupported(
            "foundation selected lab entry is not connected",
        ));
    }
    let config = foundation_bootstrap_config::from_invocation(&invocation)?;
    let mut inputs = FoundationBootstrapInputs::prepare(clock, launch, invocation, config)
        .map_err(foundation_orchestrator::FoundationOrchestratorError::from)
        .map_err(|error| Error::Denied(error.public_reason()))?;
    let outcome = inputs
        .with_initial_snapshots(git_signal, |view, payloads| {
            if let Some(observation) = observation {
                foundation_orchestrator::evaluate_with_owned_catalogue_observation(
                    view,
                    payloads,
                    observation,
                )
            } else {
                foundation_orchestrator::evaluate(view, payloads)
            }
        })
        .map_err(|error| Error::Denied(error.public_reason()))?;
    match outcome {
        SourceFoundationOutputOutcome::Complete(assembled) => {
            let ticket = inputs
                .remaining_budget
                .begin_window("cli-output", FoundationPhaseReservation::default())?;
            let admitted_output = ticket.remaining().output_bytes.min(max_output_bytes);
            output_cap.set(admitted_output);
            let rendered = (|| {
                // This is an output admission check using the same formatter,
                // not a claim that counting emitted any physical bytes.
                foundation_cli::measure_result(
                    &inputs.launch.arguments,
                    None,
                    &assembled.issues,
                    admitted_output,
                    deadline,
                    cancelled,
                )
                .map_err(Error::Denied)?;
                let mut selected_stdout = SelectedOutput {
                    writer: stdout,
                    bytes,
                    stopped,
                    max_bytes: admitted_output,
                    deadline,
                    cancelled,
                };
                let mut selected_stderr = SelectedOutput {
                    writer: stderr,
                    bytes,
                    stopped,
                    max_bytes: admitted_output,
                    deadline,
                    cancelled,
                };
                let code = foundation_cli::write_result(
                    &inputs.launch.arguments,
                    None,
                    &assembled.issues,
                    admitted_output,
                    &mut selected_stdout,
                    &mut selected_stderr,
                )
                .map_err(Error::Denied)?;
                selected_stdout
                    .flush()
                    .map_err(|_| Error::Denied("foundation stdout failed"))?;
                selected_stderr
                    .flush()
                    .map_err(|_| Error::Denied("foundation stderr failed"))?;
                Ok(code)
            })();
            match rendered {
                Ok(code) => {
                    inputs.remaining_budget.complete_window(
                        ticket,
                        FoundationPhaseUse {
                            output_bytes: FoundationCharge::measured(bytes.get()),
                            ..FoundationPhaseUse::default()
                        },
                    )?;
                    Ok(code)
                }
                Err(error) => {
                    // A writer failure may hide a partial physical write.
                    // Admit its original output upper bound, poison and stop;
                    // never turn unknown partial consumption into zero.
                    let _ = inputs.remaining_budget.fail_window(
                        ticket,
                        FoundationPhaseUse {
                            output_bytes: FoundationCharge::admitted_upper_bound(admitted_output),
                            ..FoundationPhaseUse::default()
                        },
                    );
                    Err(error)
                }
            }
        }
        SourceFoundationOutputOutcome::Incomplete { .. } => {
            Err(Error::Unsupported("foundation owner evaluation incomplete"))
        }
        SourceFoundationOutputOutcome::Refused { reason, .. } => Err(Error::Denied(reason)),
    }
}
