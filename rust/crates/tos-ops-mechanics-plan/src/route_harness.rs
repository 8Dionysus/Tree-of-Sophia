//! Actual declared-task route consumer. Results describe route shape only;
//! authored meaning, model behavior and owner acceptance remain elsewhere.
use crate::executor::{Limits, capture_ci_git};
use crate::route_cards::{
    OutputBudget, RouteSources, render_currentness, sha256_bytes, whitespace_tokens,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::sync::atomic::AtomicI32;
use std::time::{Duration, Instant};
use tos_foundation::python_lower_unicode16_v1;

const INVENTORY: &str = "docs/validation/agents_route_inventory.json";
const LANES: &str = "docs/validation/validation_lanes.json";
const MAX_TASKS: usize = 4096;
const MAX_ITEMS: usize = 4096;
const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
// Charge cached text work too, rather than letting repeated tasks multiply
// an otherwise bounded input into unbounded lower/search/token scan work.
const MAX_SCAN_BYTES: usize = 512 * 1024 * 1024;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn field<'a>(value: &'a Value, key: &str) -> io::Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| invalid(format!("missing harness field: {key}")))
}
fn string<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    field(value, key)?
        .as_str()
        .filter(|s| s.len() <= MAX_TEXT_BYTES)
        .ok_or_else(|| invalid(format!("harness {key} must be bounded text")))
}
fn strings<'a>(value: &'a Value, key: &str) -> io::Result<Vec<&'a str>> {
    let items = field(value, key)?
        .as_array()
        .filter(|v| v.len() <= MAX_ITEMS)
        .ok_or_else(|| invalid(format!("harness {key} must be a bounded list")))?;
    items
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|s| s.len() <= MAX_TEXT_BYTES)
                .ok_or_else(|| invalid(format!("harness {key} must contain bounded text")))
        })
        .collect()
}
fn number(value: &Value, key: &str) -> io::Result<f64> {
    field(value, key)?
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| invalid(format!("harness {key} must be finite numeric")))
}
fn numeric_text(value: &Value) -> io::Result<String> {
    if !value.is_number() {
        return Err(invalid("harness budget must be numeric"));
    }
    Ok(render_currentness(value)?.trim_end_matches('\n').to_owned())
}
fn lower(text: &str) -> io::Result<String> {
    python_lower_unicode16_v1(text, MAX_TEXT_BYTES, MAX_TEXT_BYTES * 3, MAX_TEXT_BYTES * 3)
        .map_err(|e| invalid(e.to_string()))
}
fn missing(sources: &mut RouteSources, values: &[&str]) -> io::Result<Vec<String>> {
    let mut absent = Vec::new();
    for value in values {
        if !sources.is_file(value)? {
            absent.push((*value).to_owned());
        }
    }
    Ok(absent)
}
fn charge(scanned: &mut usize, bytes: usize) -> io::Result<()> {
    *scanned = scanned
        .checked_add(bytes)
        .filter(|n| *n <= MAX_SCAN_BYTES)
        .ok_or_else(|| invalid("harness aggregate text scan budget exceeded"))?;
    Ok(())
}
fn tokens(sources: &mut RouteSources, paths: &[&str], scanned: &mut usize) -> io::Result<usize> {
    let mut count = 0usize;
    for path in paths {
        if let Some(text) = sources.text(path)? {
            charge(scanned, text.len())?;
            count = count
                .checked_add(whitespace_tokens(&text))
                .ok_or_else(|| invalid("harness token count overflow"))?;
        }
    }
    Ok(count)
}
pub fn task_prompt_digest(inventory: &Value) -> io::Result<String> {
    let tasks = field(inventory, "task_routes")?
        .as_array()
        .filter(|v| v.len() <= MAX_TASKS)
        .ok_or_else(|| invalid("harness task_routes must be a bounded array"))?;
    let mut payload = String::new();
    for (index, task) in tasks.iter().enumerate() {
        let id = string(task, "id")?;
        let prompt = string(task, "prompt")?;
        let added = id
            .len()
            .checked_add(prompt.len() + 1 + usize::from(index > 0))
            .ok_or_else(|| invalid("harness prompt digest overflow"))?;
        if payload
            .len()
            .checked_add(added)
            .is_none_or(|n| n > MAX_TEXT_BYTES)
        {
            return Err(invalid("harness aggregate prompt byte budget exceeded"));
        }
        if index > 0 {
            payload.push('\n');
        }
        payload.push_str(id);
        payload.push('\n');
        payload.push_str(prompt);
    }
    Ok(sha256_bytes(payload.as_bytes()))
}

/// At most two 10s Git captures, each capped at 4MiB combined stdout/stderr,
/// within the reader's 30s operation deadline. Supervisor failures propagate;
/// ordinary Git nonzero status preserves Python's conservative working-tree.
pub fn current_ref(root: &Path, cancel: &AtomicI32) -> io::Result<String> {
    let limits = Limits {
        command_wall: Duration::from_secs(10),
        lane_wall: Duration::from_secs(10),
        cleanup_grace: Duration::from_secs(1),
        output_bytes: 4 * 1024 * 1024,
    };
    let capture = |args: &[&str]| {
        capture_ci_git(
            root,
            std::iter::once("git")
                .chain(args.iter().copied())
                .map(str::to_owned)
                .collect(),
            limits,
            cancel,
        )
    };
    let (status, output, _) = capture(&["status", "--porcelain=v1", "--untracked-files=all"])?;
    if status != 0
        || !String::from_utf8(output)
            .map_err(|_| invalid("non-UTF8 Git status"))?
            .trim()
            .is_empty()
    {
        return Ok("working-tree".into());
    }
    let (status, output, _) = capture(&["rev-parse", "HEAD"])?;
    if status != 0 {
        return Ok("working-tree".into());
    }
    let output = String::from_utf8(output).map_err(|_| invalid("non-UTF8 Git ref"))?;
    Ok(output.trim().to_owned())
}
pub fn requested_source_ref(observed: &str, requested: Option<&str>) -> String {
    if observed == "working-tree" {
        return observed.into();
    }
    requested
        .filter(|s| !s.is_empty())
        .unwrap_or(observed)
        .into()
}

pub fn evaluate_task(
    sources: &mut RouteSources,
    inventory: &Value,
    task: &Value,
    discovered: &BTreeSet<String>,
    lanes: &BTreeSet<String>,
    include_timing: bool,
) -> io::Result<Value> {
    evaluate_task_with_cost(
        sources,
        inventory,
        task,
        discovered,
        lanes,
        include_timing,
        &mut 0,
        &mut OutputBudget::new(),
    )
}
fn evaluate_task_with_cost(
    sources: &mut RouteSources,
    inventory: &Value,
    task: &Value,
    discovered: &BTreeSet<String>,
    lanes: &BTreeSet<String>,
    include_timing: bool,
    scanned: &mut usize,
    output_budget: &mut OutputBudget,
) -> io::Result<Value> {
    sources.check()?;
    // Before any amplified task/list clones, reserve the selected task fields
    // up to three times (declared + repeated missing/owner fields), plus fixed
    // generated field/number/indent overhead. This conservative escaped-size
    // preflight also bounds construction, before the renderer's exact cap.
    output_budget.reserve(4096)?;
    for _ in 0..3 {
        output_budget.value(task)?;
    }
    let started = Instant::now();
    let id = string(task, "id")?;
    let target = string(task, "target")?;
    let owner = string(task, "owner_route")?;
    let stack = sources.target_stack(target, discovered)?;
    for path in &stack {
        output_budget.text(path)?;
    }
    let owner_exists = sources.is_file(owner)?;
    let target_exists = sources.is_file(target)?;
    let owner_in_stack = stack.iter().any(|p| p == owner);
    let handoff = owner_exists && !owner_in_stack;
    let mut route_text = String::new();
    let mut present_texts = 0usize;
    for path in stack
        .iter()
        .map(String::as_str)
        .chain(handoff.then_some(owner))
    {
        if let Some(text) = sources.text(path)? {
            let separator = usize::from(present_texts > 0);
            if route_text
                .len()
                .checked_add(text.len() + separator)
                .is_none_or(|n| n > MAX_TEXT_BYTES)
            {
                return Err(invalid("harness aggregate route text byte budget exceeded"));
            }
            if present_texts > 0 {
                route_text.push('\n');
            }
            route_text.push_str(&text);
            present_texts += 1;
        }
    }
    charge(scanned, route_text.len())?;
    let route_text = lower(&route_text)?;
    let mut missing_markers = |key: &str| -> io::Result<Vec<String>> {
        let mut missing = Vec::new();
        for marker in strings(task, key)? {
            sources.check()?;
            charge(scanned, marker.len())?;
            charge(scanned, route_text.len())?;
            if !route_text.contains(&lower(marker)?) {
                missing.push(marker.to_owned());
            }
        }
        Ok(missing)
    };
    let required = missing_markers("required_markers")?;
    let boundary = missing_markers("boundary_markers")?;
    let validation_paths = strings(task, "validation_paths")?;
    let completion_paths = strings(task, "completion_evidence")?;
    let extra_paths = strings(task, "on_demand_surfaces")?;
    let validation_lanes = strings(task, "validation_lanes")?;
    let absent_validation = missing(sources, &validation_paths)?;
    let absent_completion = missing(sources, &completion_paths)?;
    let absent_extra = missing(sources, &extra_paths)?;
    let unknown_lanes: Vec<_> = validation_lanes
        .iter()
        .filter(|lane| !lanes.contains(**lane))
        .copied()
        .collect();
    let stack_refs: Vec<_> = stack.iter().map(String::as_str).collect();
    let inherited = tokens(sources, &stack_refs, scanned)?;
    let additional = tokens(sources, &extra_paths, scanned)?;
    let handoff_tokens = if handoff {
        tokens(sources, &[owner], scanned)?
    } else {
        0
    };
    let budget = field(inventory, "context_budget")?;
    let max_inherited = budget
        .get("task_overrides")
        .and_then(|v| v.get(id))
        .unwrap_or(field(budget, "inherited_stack_max_tokens")?);
    let max_inherited_number = max_inherited
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| invalid("harness inherited budget must be finite numeric"))?;
    output_budget.value(max_inherited)?;
    output_budget.value(field(budget, "additional_context_max_tokens")?)?;
    output_budget.value(field(budget, "owner_handoff_max_tokens")?)?;
    let coverage = if completion_paths.is_empty() {
        0.0
    } else {
        (completion_paths.len() - absent_completion.len()) as f64 / completion_paths.len() as f64
    };
    let mut violations = Vec::new();
    if inherited as f64 > max_inherited_number {
        violations.push(format!(
            "inherited_context_tokens>{}",
            numeric_text(max_inherited)?
        ));
    }
    if additional as f64 > number(budget, "additional_context_max_tokens")? {
        violations.push(format!(
            "additional_context_tokens>{}",
            numeric_text(field(budget, "additional_context_max_tokens")?)?
        ));
    }
    if handoff_tokens as f64 > number(budget, "owner_handoff_max_tokens")? {
        violations.push(format!(
            "owner_handoff_context_tokens>{}",
            numeric_text(field(budget, "owner_handoff_max_tokens")?)?
        ));
    }
    // Python compares len(stack)-1 before clamping the reported owner_hops.
    if stack.len() as f64 - 1.0 > number(budget, "owner_hops_max")? {
        violations
            .push("owner_hops>".to_owned() + &numeric_text(field(budget, "owner_hops_max")?)?);
    }
    if required.len() as f64 > number(budget, "missing_task_law_max")? {
        violations.push("missing_task_specific_law".into());
    }
    if boundary.len() as f64 > number(budget, "boundary_deviation_max")? {
        violations.push("boundary_deviation".into());
    }
    if coverage < number(budget, "completion_coverage_min")? {
        violations.push("completion_coverage".into());
    }
    sources.check()?;
    let timing = if include_timing {
        Some((started.elapsed().as_secs_f64() * 1_000_000.0).round_ties_even() / 1000.0)
    } else {
        None
    };
    let success = required.is_empty()
        && boundary.is_empty()
        && absent_validation.is_empty()
        && unknown_lanes.is_empty()
        && absent_completion.is_empty()
        && absent_extra.is_empty()
        && target_exists
        && owner_exists
        && violations.is_empty();
    Ok(json!({
        "id":id,"prompt":string(task,"prompt")?,"target":target,"owner_route":owner,"route_success":success,
        "inherited_context_tokens":inherited,"additional_context_tokens":additional,"owner_handoff_context_tokens":handoff_tokens,
        "owner_hops":stack.len().saturating_sub(1),"time_to_owner_ms":timing,
        "route_resolution_measurement":if include_timing {"Wall-clock duration of this harness lookup, measured for the current run."} else {"deterministic route lookup; timing omitted from canonical result"},
        "inheritance_stack":stack,"owner_handoff":{"path":owner,"exists":owner_exists,"in_inheritance_stack":owner_in_stack,"whitespace_tokens":handoff_tokens},
        "declared_extra_reads":{"count":extra_paths.len(),"paths":extra_paths,"missing_paths":absent_extra,"behavioral_extra_reads":null},
        "selected_validation":{"lanes":validation_lanes,"unknown_lanes":unknown_lanes,"paths":validation_paths},
        "boundary_deviation":{"count":boundary.len(),"missing_markers":boundary},
        "missing_task_specific_law":{"count":required.len(),"missing_markers":required},"completion_coverage":coverage,
        "completion_evidence":{"paths":completion_paths,"missing_paths":absent_completion},
        "target_exists":target_exists,"owner_exists":owner_exists,"owner_in_inheritance_stack":owner_in_stack,
        "budget":{"inherited_stack_max_tokens":max_inherited,"additional_context_max_tokens":field(budget,"additional_context_max_tokens")?,"owner_handoff_max_tokens":field(budget,"owner_handoff_max_tokens")?,"violations":violations},
        "handoff_routes":strings(task,"handoff_routes")?
    }))
}

pub fn build_result(root: &Path, include_timing: bool, cancel: &AtomicI32) -> io::Result<Value> {
    let mut sources = RouteSources::new(root)?;
    let inventory = sources.inventory()?;
    let inventory_hash = sha256_bytes(&sources.bytes(INVENTORY)?);
    let discovered = sources.discover(&inventory)?.into_iter().collect();
    let manifest: Value = serde_json::from_str(
        &sources
            .text(LANES)?
            .ok_or_else(|| invalid("missing validation lane manifest"))?,
    )
    .map_err(|e| invalid(format!("invalid validation lane manifest: {e}")))?;
    let lanes = manifest
        .get("lanes")
        .and_then(Value::as_object)
        .map(|v| v.keys().cloned().collect())
        .unwrap_or_default();
    let digest = task_prompt_digest(&inventory)?;
    let task_routes = field(&inventory, "task_routes")?
        .as_array()
        .ok_or_else(|| invalid("task_routes must be an array"))?;
    let mut tasks = Vec::new();
    let mut scanned = 0;
    let mut output_budget = OutputBudget::new();
    output_budget.reserve(4096)?;
    for task in task_routes {
        tasks.push(evaluate_task_with_cost(
            &mut sources,
            &inventory,
            task,
            &discovered,
            &lanes,
            include_timing,
            &mut scanned,
            &mut output_budget,
        )?);
    }
    let successful = tasks.iter().filter(|t| t["route_success"] == true).count();
    let source_ref = current_ref(root, cancel)?;
    output_budget.text(&source_ref)?;
    sources.check()?;
    Ok(
        json!({"schema_version":"tos_agents_route_harness_result_v1",
        "harness":"Deterministic evaluation of declared agent routes and route-card structure.",
        "source_ref":source_ref,"inventory_ref":INVENTORY,"inventory_sha256":inventory_hash,"task_prompt_digest":digest,
        "task_count":tasks.len(),"route_success_count":successful,
        "route_success_rate":if tasks.is_empty() {0.0} else {successful as f64 / tasks.len() as f64},
        "behavioral_model_runs":0,"behavioral_claim":null,"tasks":tasks}),
    )
}
/// Same canonical JSON grammar as route currentness, with one trailing LF.
pub fn render_result(value: &Value) -> io::Result<String> {
    render_currentness(value)
}
