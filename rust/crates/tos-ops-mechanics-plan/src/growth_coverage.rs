//! Growth completion is an owner assessment, not a count of test modules.
use serde::Deserialize;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

pub(crate) const CONTRACT: &str =
    "mechanics/growth-cycle/parts/branch-growth-cycle/docs/native-behavior-coverage.json";
const MAX_CONTRACT_BYTES: u64 = 1_048_576;

#[derive(Deserialize)]
struct Coverage {
    schema_version: String,
    whole_route_status: String,
    maintained_route: String,
    bounded_native_route: String,
    reference_route: String,
    assessment_route: String,
}

fn load(root: &Path) -> io::Result<Coverage> {
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

pub fn require_whole_route(root: &Path) -> io::Result<()> {
    let coverage = load(root)?;
    match coverage.whole_route_status.as_str() {
        "accepted" => Err(io::Error::other(
            "Growth owner assessment is accepted but no whole native execution route is wired",
        )),
        "incomplete" => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "whole native Growth behavior remains incomplete; maintained source checks: {}; bounded native mechanics: {}; explicit reference comparison: {}; owner assessment: {}",
                coverage.maintained_route,
                coverage.bounded_native_route,
                coverage.reference_route,
                coverage.assessment_route,
            ),
        )),
        _ => Err(io::Error::other("unknown Growth whole-route assessment")),
    }
}
