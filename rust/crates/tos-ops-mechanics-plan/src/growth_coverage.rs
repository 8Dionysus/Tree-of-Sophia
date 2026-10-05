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
    reference_discovery: Option<ReferenceDiscovery>,
    #[serde(default)]
    pub(crate) native_test_routes: Vec<crate::growth_native_plan::TestRoute>,
}

#[derive(Deserialize)]
struct ReferenceDiscovery {
    root: String,
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

fn selected_steps(
    root: &Path,
    python: &str,
) -> io::Result<Vec<crate::validation_lanes::BudgetedCommandStep>> {
    let coverage = load(root)?;
    let sequence = coverage.native_execution_sequence.as_deref().filter(|name| !name.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported,
            "Growth has no source-owned native execution sequence; use explicit reference comparison or bounded contracts"))?;
    let steps = crate::validation_lanes::command_sequence_with_budgets(root, sequence, python)?;
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
/// executor contract. A present Growth source contract (even malformed), or
/// the retained owner's actual unittest home, requires native Growth planning.
pub fn uses_native_route(root: &Path, mechanics: &crate::Plan) -> io::Result<bool> {
    match fs::symlink_metadata(root.join(CONTRACT)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let owner = CONTRACT
                .split_once("/parts/")
                .ok_or_else(|| io::Error::other("Growth contract owner route missing"))?
                .0;
            Ok(mechanics
                .commands
                .iter()
                .any(|command| command.kind == "unittest" && command.home == owner))
        }
        Err(error) => Err(error),
    }
}

/// Validate an actual executable owner binding, never an acceptance flag.
pub fn require_whole_route(root: &Path) -> io::Result<()> {
    selected_steps(root, "python").map(|_| ())
}

/// Keep other mechanics and Growth builders/validators in authored order;
/// replace only the owner's retained unittest comparison with its real native
/// workspace pipeline. One executor owns the combined lane wall deadline.
pub fn whole_steps(
    root: &Path,
    python: &str,
    mechanics: &crate::Plan,
) -> io::Result<Vec<crate::validation_lanes::BudgetedCommandStep>> {
    let coverage = load(root)?;
    let discovery = coverage
        .reference_discovery
        .ok_or_else(|| io::Error::other("Growth reference discovery owner is missing"))?;
    let tests = tos_foundation::RelativePath::parse(&discovery.root).map_err(io::Error::other)?;
    let owner = Path::new(tests.as_str())
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| io::Error::other("Growth reference test root has no owner home"))?;
    let mut steps: Vec<_> = mechanics
        .commands
        .iter()
        .filter(|command| !(command.kind == "unittest" && command.home == owner))
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
    steps.extend(selected_steps(root, python)?);
    Ok(steps)
}
