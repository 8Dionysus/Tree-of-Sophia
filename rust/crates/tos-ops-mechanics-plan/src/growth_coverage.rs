//! Growth completion is an owner assessment, not a count of test modules.
use serde::Deserialize;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

pub(crate) const CONTRACT: &str =
    "mechanics/growth-cycle/parts/branch-growth-cycle/docs/native-behavior-coverage.json";
const MAX_CONTRACT_BYTES: u64 = 1_048_576;

#[derive(Deserialize)]
pub(crate) struct Coverage {
    schema_version: String,
    #[serde(default)]
    native_execution_sequence: Option<String>,
    #[serde(default)]
    pub(crate) native_test_routes: Vec<crate::growth_native_plan::TestRoute>,
}

pub(crate) fn load(root: &Path) -> io::Result<Coverage> {
    let path = root.join(CONTRACT);
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err(io::Error::other(
            "Growth coverage contract must be a regular file",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_CONTRACT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONTRACT_BYTES {
        return Err(io::Error::other(
            "Growth coverage contract exceeds its byte bound",
        ));
    }
    let coverage: Coverage = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if coverage.schema_version != "tos_growth_native_coverage_v1" {
        return Err(io::Error::other("unsupported Growth coverage contract"));
    }
    Ok(coverage)
}

fn selected_steps(root: &Path) -> io::Result<Vec<crate::validation_lanes::BudgetedCommandStep>> {
    let coverage = load(root)?;
    let sequence = coverage.native_execution_sequence.as_deref().filter(|name| !name.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported,
            "Growth has no source-owned native execution sequence; use explicit reference comparison or bounded contracts"))?;
    let steps = crate::validation_lanes::command_sequence_with_budgets(root, sequence)?;
    if !steps.iter().any(|((_, argv), _)| {
        argv.first()
            .is_some_and(|arg| arg == crate::growth_native_plan::CLASS_SEQUENCE)
    }) || !steps.iter().any(|((_, argv), _)| {
        argv.iter().any(|arg| arg == "--no-run")
            && argv.iter().any(|arg| arg == "--message-format=json")
    }) {
        return Err(io::Error::other(
            "Growth native execution sequence lacks current-product preparation or owned class execution",
        ));
    }
    crate::growth_native_plan::expand_steps(root, &steps)?;
    Ok(steps)
}

/// A standalone mechanics fixture with no Growth owner retains its ordinary
/// executor contract. The Growth home itself requires native planning, even
/// when its contract is missing and no retired Python tests remain.
pub fn uses_native_route(root: &Path, mechanics: &crate::Plan) -> io::Result<bool> {
    match fs::symlink_metadata(root.join(CONTRACT)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let owner = CONTRACT
                .split_once("/parts/")
                .ok_or_else(|| io::Error::other("Growth contract owner route missing"))?
                .0;
            match fs::symlink_metadata(root.join(owner)) {
                Ok(_) => Ok(true),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(mechanics
                    .commands
                    .iter()
                    .any(|command| command.home == owner)),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

/// Validate an actual executable owner binding, never an acceptance flag.
pub fn require_whole_route(root: &Path) -> io::Result<()> {
    selected_steps(root).map(|_| ())
}

/// Keep mechanics commands in authored order and append the native Growth
/// pipeline. One executor owns the combined lane wall deadline.
pub fn whole_steps(
    root: &Path,
    mechanics: &crate::Plan,
) -> io::Result<Vec<crate::validation_lanes::BudgetedCommandStep>> {
    let mut steps: Vec<_> = mechanics
        .commands
        .iter()
        .map(|command| {
            (
                (
                    format!("{}: {}", command.home, command.kind),
                    command.argv.clone(),
                ),
                None,
            )
        })
        .collect();
    steps.extend(selected_steps(root)?);
    Ok(steps)
}
