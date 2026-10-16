//! Apply real complete diagnostics to the maintained laboratory report.
//! Schema validity and schema-or-semantic control rejection are distinct.

use serde_json::Value;
use tos_validation::source_foundation_labs::SourceFoundationLabResult;
use tos_validation::source_foundation_schema::SourceFoundationSchemaReport;

pub(crate) struct ResolvedFoundationLab {
    pub report: Value,
    pub issues: Vec<(String, String)>,
    /// Additional logical state reserved for this resolution, beyond the
    /// already admitted source DTO, decoded report and diagnostics report.
    pub additional_state_bytes: usize,
}

/// Retain the actual complete diagnostics alongside the presentation. An
/// owner-only missing-file route has no schema checks and launches no worker.
pub(crate) struct EvaluatedFoundationLab {
    pub resolved: ResolvedFoundationLab,
    pub diagnostics: Option<SourceFoundationSchemaReport>,
    pub input_workspace_bytes: usize,
}
pub(crate) enum FoundationLabEvaluationError {
    Refused(&'static str),
    IncompleteSchema {
        report: SourceFoundationSchemaReport,
        reason: tos_validation::source_foundation_schema::SourceFoundationSchemaFailure,
    },
}

/// Execute the actual lab schema requests, then apply their exact bound
/// control semantics. All DTO instances remain borrowed through execution;
/// neither a bool substitute nor an invented diagnostics receipt is used.
pub(crate) fn evaluate(
    lab: SourceFoundationLabResult,
    schema_set: &tos_validation::source_foundation_schema::SourceFoundationSchemaSet,
    worker: &tos_validation::executor::ExactWorkerIdentity,
    schema_limits: tos_validation::source_foundation_schema::SourceFoundationSchemaLimits,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
    max_issues: usize,
    max_output_bytes: usize,
    workspace: usize,
) -> Result<EvaluatedFoundationLab, FoundationLabEvaluationError> {
    use tos_validation::source_foundation_schema::{
        SourceFoundationSchemaInput, SourceFoundationSchemaOutcome,
        evaluate_source_foundation_schema_checks,
    };
    let fail = FoundationLabEvaluationError::Refused;
    if !lab.unimplemented.is_empty() {
        return Err(fail("foundation laboratory owner coverage incomplete"));
    }
    if std::time::Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err(fail("foundation laboratory deadline or cancellation"));
    }
    if lab.schema_checks.is_empty() {
        let bytes = lab
            .ordered_issues
            .iter()
            .try_fold(0usize, |n, (path, message)| {
                n.checked_add(path.len())
                    .and_then(|n| n.checked_add(message.len()))
            })
            .ok_or_else(|| fail("foundation laboratory issue byte overflow"))?;
        if lab.ordered_issues.len() > max_issues || bytes > max_output_bytes {
            return Err(fail("foundation laboratory issue limits"));
        }
        return Ok(EvaluatedFoundationLab {
            resolved: ResolvedFoundationLab {
                report: lab.report,
                issues: lab.ordered_issues,
                additional_state_bytes: 0,
            },
            diagnostics: None,
            input_workspace_bytes: 0,
        });
    }
    let input_workspace = lab
        .schema_checks
        .len()
        .checked_mul(std::mem::size_of::<SourceFoundationSchemaInput<'_>>())
        .filter(|n| *n <= workspace)
        .ok_or_else(|| fail("foundation laboratory schema input workspace"))?;
    let checks: Vec<_> = lab
        .schema_checks
        .iter()
        .map(|check| SourceFoundationSchemaInput {
            location: &check.location,
            contract: &check.contract,
            decoded_instance: &check.instance,
        })
        .collect();
    let diagnostics = match evaluate_source_foundation_schema_checks(
        schema_set,
        worker,
        &checks,
        schema_limits,
        deadline,
        cancelled,
    ) {
        SourceFoundationSchemaOutcome::Complete(report) => report,
        SourceFoundationSchemaOutcome::Incomplete { report, reason } => {
            return Err(FoundationLabEvaluationError::IncompleteSchema { report, reason });
        }
    };
    drop(checks);
    let resolved = resolve(
        lab,
        &diagnostics,
        0,
        max_issues,
        max_output_bytes,
        workspace - input_workspace,
    )
    .map_err(fail)?;
    Ok(EvaluatedFoundationLab {
        resolved,
        diagnostics: Some(diagnostics),
        input_workspace_bytes: input_workspace,
    })
}

pub(crate) fn resolve(
    mut lab: SourceFoundationLabResult,
    diagnostics: &SourceFoundationSchemaReport,
    first_check: usize,
    max_issues: usize,
    max_output_bytes: usize,
    workspace: usize,
) -> Result<ResolvedFoundationLab, &'static str> {
    if !lab.unimplemented.is_empty() || !diagnostics.is_complete() {
        return Err("foundation laboratory evaluation incomplete");
    }
    let end = first_check
        .checked_add(lab.schema_checks.len())
        .ok_or("foundation laboratory schema count overflow")?;
    let selected = diagnostics
        .checks
        .get(first_check..end)
        .ok_or("foundation laboratory schema checks absent")?;
    let all_schema_issues = selected
        .iter()
        .try_fold(0usize, |n, check| n.checked_add(check.issues.len()))
        .ok_or("foundation laboratory schema issue count overflow")?;
    let mut count = lab.ordered_issues.len();
    let mut bytes = 0usize;
    let mut additional = 0usize;
    for (location, message) in &lab.ordered_issues {
        add(&mut bytes, location.len())?;
        add(&mut bytes, message.len())?;
    }
    let mut prior_ordinal = 0;
    for (request, check) in lab.schema_checks.iter().zip(selected) {
        if request.before_issue < prior_ordinal
            || request.before_issue > lab.ordered_issues.len()
            || check.location != request.location
            || check.contract != request.contract
        {
            return Err("foundation laboratory schema binding/order");
        }
        prior_ordinal = request.before_issue;
        let valid = check.diagnostic.is_valid();
        let control = request.negative_control.is_some();
        let mismatch = if control {
            let expected = request
                .expected_rejected
                .ok_or("foundation laboratory control expectation absent")?;
            expected != (!valid || request.semantic_rejected.unwrap_or(false))
        } else {
            request
                .expected_valid
                .is_some_and(|expected| expected != valid)
        };
        if !control {
            for issue in &check.issues {
                add(&mut count, 1)?;
                let message_bytes = request
                    .schema_message_prefix
                    .as_ref()
                    .map_or(0, String::len)
                    .checked_add(issue.message.len())
                    .ok_or("foundation laboratory issue overflow")?;
                add(&mut bytes, issue.location.len())?;
                add(&mut bytes, message_bytes)?;
                add(&mut additional, issue.location.len())?;
                add(&mut additional, message_bytes)?;
            }
        }
        if mismatch {
            let message = request
                .mismatch_message
                .as_ref()
                .ok_or("foundation laboratory mismatch message absent")?;
            add(&mut count, 1)?;
            add(&mut bytes, request.location.len())?;
            add(&mut bytes, message.len())?;
            add(&mut additional, request.location.len())?;
            add(&mut additional, message.len())?;
        }
        if let Some(slot) = &request.report_slot {
            if lab.report.pointer(slot).is_none() {
                return Err("foundation laboratory verdict slot absent");
            }
            // The longest control literal is not_rejected; normal verdicts
            // replace a Value in place without a heap string.
            add(
                &mut additional,
                if control { "not_rejected".len() } else { 0 },
            )?;
        }
        if let Some(slot) = &request.rejection_reasons_slot {
            let reasons = lab
                .report
                .pointer(slot)
                .and_then(Value::as_array)
                .ok_or("foundation laboratory reasons slot absent")?;
            if reasons.iter().any(|reason| !reason.is_string()) {
                return Err("foundation laboratory reasons shape");
            }
            let slots = reasons
                .len()
                .checked_add(all_schema_issues)
                .and_then(|n| n.checked_mul(std::mem::size_of::<Value>()))
                .ok_or("foundation laboratory reasons state overflow")?;
            // Admit a replacement vector before moving existing strings;
            // no clone of owner-authored reasons is needed.
            add(&mut additional, slots)?;
            for issue in &check.issues {
                add(&mut additional, issue.message.len())?;
            }
        }
    }
    add(
        &mut additional,
        count
            .checked_mul(std::mem::size_of::<(String, String)>())
            .ok_or("foundation laboratory output state overflow")?,
    )?;
    if count > max_issues || bytes > max_output_bytes || additional > workspace {
        return Err("foundation laboratory resolution limits");
    }
    let mut issues = Vec::with_capacity(count);
    let mut direct = std::mem::take(&mut lab.ordered_issues).into_iter();
    let mut ordinal = 0;
    for (request, check) in lab.schema_checks.iter().zip(selected) {
        while ordinal < request.before_issue {
            issues.push(
                direct
                    .next()
                    .ok_or("foundation laboratory issue placement absent")?,
            );
            ordinal += 1;
        }
        let valid = check.diagnostic.is_valid();
        let control = request.negative_control.is_some();
        let rejected = !valid || request.semantic_rejected.unwrap_or(false);
        if let Some(slot) = &request.report_slot {
            *lab.report
                .pointer_mut(slot)
                .ok_or("foundation laboratory verdict slot changed")? = if control {
                Value::String(if rejected { "rejected" } else { "not_rejected" }.into())
            } else {
                Value::Bool(valid)
            };
        }
        if let Some(slot) = &request.rejection_reasons_slot {
            let target = lab
                .report
                .pointer_mut(slot)
                .ok_or("foundation laboratory reasons slot changed")?;
            let old = std::mem::replace(target, Value::Null);
            let Value::Array(old) = old else {
                return Err("foundation laboratory reasons slot changed");
            };
            let mut reasons = Vec::with_capacity(old.len() + check.issues.len());
            reasons.extend(old);
            reasons.extend(
                check
                    .issues
                    .iter()
                    .map(|issue| Value::String(issue.message.into())),
            );
            reasons.sort_unstable_by(|a, b| a.as_str().cmp(&b.as_str()));
            reasons.dedup();
            *target = Value::Array(reasons);
        }
        if !control {
            for issue in &check.issues {
                let prefix = request.schema_message_prefix.as_deref().unwrap_or("");
                let mut message = String::with_capacity(prefix.len() + issue.message.len());
                message.push_str(prefix);
                message.push_str(issue.message);
                issues.push((issue.location.clone(), message));
            }
        }
        let mismatch = if control {
            request
                .expected_rejected
                .is_some_and(|expected| expected != rejected)
        } else {
            request
                .expected_valid
                .is_some_and(|expected| expected != valid)
        };
        if mismatch {
            issues.push((
                request.location.clone(),
                request
                    .mismatch_message
                    .as_ref()
                    .ok_or("foundation laboratory mismatch message changed")?
                    .clone(),
            ));
        }
    }
    issues.extend(direct);
    Ok(ResolvedFoundationLab {
        report: lab.report,
        issues,
        additional_state_bytes: additional,
    })
}

fn add(total: &mut usize, amount: usize) -> Result<(), &'static str> {
    *total = total
        .checked_add(amount)
        .ok_or("foundation laboratory resolution overflow")?;
    Ok(())
}
